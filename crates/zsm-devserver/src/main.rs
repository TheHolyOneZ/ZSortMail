use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use zsm_app::{Core, CoreConfig, CoreEvent, EventSink, GoogleEndpoints};
use zsm_core::clock::{Clock, FixedClock, SystemClock};

struct BroadcastSink(broadcast::Sender<CoreEvent>);

impl EventSink for BroadcastSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.0.send(event);
    }
}

#[derive(Clone)]
struct Shared {
    window: WebviewWindow<tauri::test::MockRuntime>,
    core: Arc<Core>,
    events: broadcast::Sender<CoreEvent>,
    fake: Option<Arc<zsm_testkit::FakeGoogle>>,
    now: Option<i64>,
}

struct Options {
    port: u16,
    demo: bool,
    fake_google: bool,
    real_google: bool,
    people: usize,
    now: Option<i64>,
    tz: TimeZone,
    db: Option<PathBuf>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        port: 1421,
        demo: false,
        fake_google: false,
        real_google: false,
        people: 0,
        now: None,
        tz: TimeZone::system(),
        db: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--port" => options.port = value("--port")?.parse().map_err(|_| "--port must be a number")?,
            "--demo" => options.demo = true,
            "--fake-google" => options.fake_google = true,
            "--real-google" => options.real_google = true,
            "--people" => options.people = value("--people")?.parse().map_err(|_| "--people must be a number")?,
            "--now" => {
                let text = value("--now")?;
                let ts: Timestamp = text.parse().map_err(|e| format!("--now: {e}"))?;
                options.now = Some(ts.as_millisecond());
            }
            "--tz" => {
                let text = value("--tz")?;
                options.tz = if text.eq_ignore_ascii_case("utc") || text == "Z" {
                    TimeZone::UTC
                } else {
                    let hours: i8 = text
                        .trim_start_matches('+')
                        .split(':')
                        .next()
                        .and_then(|h| h.parse().ok())
                        .ok_or("--tz must look like +01:00")?;
                    TimeZone::fixed(Offset::from_hours(hours).map_err(|e| e.to_string())?)
                };
            }
            "--db" => options.db = Some(PathBuf::from(value("--db")?)),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(options)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let options = match parse_options() {
        Ok(o) => o,
        Err(message) => {
            eprintln!("zsm-devserver: {message}");
            std::process::exit(2);
        }
    };

    let (events, _) = broadcast::channel(256);
    let clock: Arc<dyn Clock> = match options.now {
        Some(ms) => Arc::new(FixedClock::new(ms)),
        None => Arc::new(SystemClock),
    };
    let fake = if options.fake_google {
        let fake = zsm_testkit::FakeGoogle::start("you@gmail.example").await;
        let now = options.now.unwrap_or_else(|| SystemClock.now_ms());
        fake.set_now(now);
        zsm_testkit::seed_sample(&fake, now);
        Some(fake)
    } else {
        None
    };
    let data_dir = std::env::temp_dir().join(format!("zsm-devserver-{}", options.port));
    if options.db.is_none() {
        let _ = std::fs::remove_dir_all(&data_dir);
    }
    let mut config = CoreConfig {
        db_path: options.db.clone(),
        data_dir: Some(data_dir),
        downloads_dir: Some(std::env::temp_dir().join(format!("zsm-devserver-{}-downloads", options.port))),
        clock,
        sink: Arc::new(BroadcastSink(events.clone())),
        coalesce: Duration::from_millis(50),
        timezone: options.tz.clone(),
        runtime: Some(tokio::runtime::Handle::current()),
        poll_interval: Duration::from_secs(if fake.is_some() { 2 } else { 20 }),
        ..CoreConfig::in_memory()
    };
    if let Some(fake) = &fake {
        config.google = GoogleEndpoints {
            auth_url: fake.auth_url(),
            token_url: fake.token_url(),
            gmail_base: fake.gmail_base(),
            people_base: fake.people_base(),
        };
    }
    if options.real_google {
        assert!(!options.fake_google, "--real-google and --fake-google exclude each other");
        let allow = match std::env::var("ZSM_WRITE_ALLOWLIST_FILE") {
            Ok(path) => std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read the allow-list file {path}: {e}"))
                .replace('\n', ","),
            Err(_) => std::env::var("ZSM_WRITE_ALLOWLIST").unwrap_or_default(),
        };
        assert!(
            !allow.trim().is_empty(),
            "--real-google needs ZSM_WRITE_ALLOWLIST_FILE or ZSM_WRITE_ALLOWLIST (the addresses that may be written to)"
        );
        tracing::warn!(
            "REAL GOOGLE. Changes are only sent for mail among {} listed addresses",
            allow.split(',').filter(|a| !a.trim().is_empty()).count()
        );
        config.quota_per_minute = 6000.0;
        config.write_allowlist =
            Some(allow.split(',').map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()).collect());
        config.secrets =
            zsm_app::platform_secrets(std::env::temp_dir().join(format!("zsm-devserver-{}-secrets", options.port)));
    }
    let core = Core::new(config).expect("open the store");
    if fake.is_some() {
        core.save_google_client("fake-client.apps.example", "fake-secret").expect("save the pretend client");
    }
    core.start_sync().expect("start syncing");
    core.start_scheduler();
    if options.demo {
        core.start_demo(options.people).expect("load the demo mailbox");
    }

    let app = zsortmail_lib::configure(mock_builder())
        .manage(Arc::clone(&core))
        .build(mock_context(noop_assets()))
        .expect("build the mock app");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default()).build().expect("mock window");
    let fake = fake.map(Arc::new);
    let shared = Shared { window, core, events, fake: fake.clone(), now: options.now };

    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ipc/{cmd}", post(ipc))
        .route("/events", get(stream_events))
        .route("/zsm/{*path}", get(zsm))
        .route("/dev/fake-mail", post(fake_mail))
        .route("/dev/sent", get(sent_mail))
        .with_state(shared);

    let address = SocketAddr::from(([127, 0, 0, 1], options.port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap_or_else(|e| {
        eprintln!("zsm-devserver: cannot listen on {address}: {e}");
        std::process::exit(1);
    });
    tracing::info!("zsm-devserver listening on http://{address} (demo: {})", options.demo);
    let _keep = (&app, &fake);
    axum::serve(listener, router).await.expect("serve");
}

async fn ipc(State(shared): State<Shared>, Path(cmd): Path<String>, body: Bytes) -> Response {
    let args: serde_json::Value = if body.is_empty() {
        serde_json::Value::Object(Default::default())
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("bad JSON: {e}")).into_response(),
        }
    };
    let window = shared.window.clone();
    let result = tokio::task::spawn_blocking(move || {
        get_ipc_response(
            &window,
            InvokeRequest {
                cmd,
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().expect("static url"),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<serde_json::Value>().unwrap_or(serde_json::Value::Null))
    })
    .await;
    match result {
        Ok(Ok(value)) => axum::Json(value).into_response(),
        Ok(Err(error)) => (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(error)).into_response(),
        Err(join) => (StatusCode::INTERNAL_SERVER_ERROR, join.to_string()).into_response(),
    }
}

async fn stream_events(
    State(shared): State<Shared>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(shared.events.subscribe()).filter_map(|item| {
        let event = item.ok()?;
        Some(Ok(Event::default().json_data(&event).ok()?))
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn zsm(State(shared): State<Shared>, Path(path): Path<String>, RawQuery(query): RawQuery) -> Response {
    let core = Arc::clone(&shared.core);
    let served = tokio::task::spawn_blocking(move || core.serve(&path, query.as_deref())).await;
    let Ok(served) = served else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut response = Response::builder()
        .status(StatusCode::from_u16(served.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, served.content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    for (name, value) in served.headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(name), HeaderValue::try_from(value)) {
            response = response.header(name, value);
        }
    }
    response.body(Body::from(served.body)).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn fake_mail(State(shared): State<Shared>, axum::Json(body): axum::Json<serde_json::Value>) -> Response {
    let Some(fake) = &shared.fake else {
        return (StatusCode::NOT_FOUND, "start the dev server with --fake-google").into_response();
    };
    let text = |key: &str| body[key].as_str().unwrap_or_default().to_string();
    let id = fake.add_message(zsm_testkit::MessageSpec {
        from: text("from"),
        to: vec!["you@gmail.example".into()],
        subject: text("subject"),
        text: Some(text("text")),
        date_ms: shared.now.unwrap_or_else(|| SystemClock.now_ms()) - 60_000,
        ..Default::default()
    });
    axum::Json(serde_json::json!({ "id": id })).into_response()
}

async fn sent_mail(State(shared): State<Shared>) -> Response {
    let Some(fake) = &shared.fake else {
        return (StatusCode::NOT_FOUND, "start the dev server with --fake-google").into_response();
    };
    let sent: Vec<String> = fake.sent().iter().map(|raw| String::from_utf8_lossy(raw).into_owned()).collect();
    axum::Json(sent).into_response()
}
