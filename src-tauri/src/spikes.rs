use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, State};

pub struct Beacon {
    port: u16,
    hits: Arc<AtomicU32>,
}

impl Beacon {
    pub fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let hits = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&hits);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                let mut buf = [0u8; 2048];
                if stream.read(&mut buf).unwrap_or(0) > 0 {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
            }
        });
        tracing::info!(target: "spike", port, "beacon listening on 127.0.0.1");
        Ok(Self { port, hits })
    }
}

const ATTACKS: &[(&str, &str)] = &[
    (
        "script-inline",
        "<script>parent.postMessage('ran:script-inline','*');new Image().src='http://127.0.0.1:{P}/b?script-inline'</script>",
    ),
    ("script-remote", "<script src=\"http://127.0.0.1:{P}/b?script-remote\"></script>"),
    ("img-remote", "<img src=\"http://127.0.0.1:{P}/b?img-remote\">"),
    ("img-onerror", "<img src=x onerror=\"parent.postMessage('ran:img-onerror','*')\">"),
    ("css-import", "<style>@import url(http://127.0.0.1:{P}/b?css-import);</style>"),
    (
        "css-background",
        "<div style=\"background:url(http://127.0.0.1:{P}/b?css-background);width:50px;height:50px\"></div>",
    ),
    ("link-stylesheet", "<link rel=stylesheet href=\"http://127.0.0.1:{P}/b?link-stylesheet\">"),
    ("meta-refresh", "<meta http-equiv=refresh content=\"0;url=http://127.0.0.1:{P}/b?meta-refresh\">"),
    (
        "form-submit",
        "<form id=f action=\"http://127.0.0.1:{P}/b?form-submit\" method=post><input name=x value=1></form><script>f.submit()</script>",
    ),
    ("iframe-remote", "<iframe src=\"http://127.0.0.1:{P}/b?iframe-remote\"></iframe>"),
    ("svg-onload", "<svg onload=\"parent.postMessage('ran:svg-onload','*')\"></svg>"),
    ("base-hijack", "<base href=\"http://127.0.0.1:{P}/b?base-hijack/\"><img src=\"relative.png\">"),
    (
        "object-embed",
        "<object data=\"http://127.0.0.1:{P}/b?object\"></object><embed src=\"http://127.0.0.1:{P}/b?embed\">",
    ),
    (
        "font-face",
        "<style>@font-face{font-family:x;src:url(http://127.0.0.1:{P}/b?font-face)}body{font-family:x}</style>text",
    ),
    ("top-navigation", "<script>top.location='http://127.0.0.1:{P}/b?top-navigation'</script>"),
    (
        "javascript-url",
        "<a id=a href=\"javascript:parent.postMessage('ran:javascript-url','*')\">x</a><script>a.click()</script>",
    ),
];

pub fn handle<R: tauri::Runtime>(
    ctx: &tauri::UriSchemeContext<'_, R>,
    request: &Request<Vec<u8>>,
) -> Option<Response<Cow<'static, [u8]>>> {
    let path = request.uri().path().strip_prefix("/spike/attack/")?;
    let (_, template) = path.parse::<usize>().ok().and_then(|i| ATTACKS.get(i))?;
    let port = ctx.app_handle().try_state::<Beacon>()?.port;
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>attack</title>{}",
        template.replace("{P}", &port.to_string())
    );
    let with_csp = request.uri().query().is_none_or(|q| !q.contains("csp=0"));

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store");
    if with_csp {
        response = response.header(header::CONTENT_SECURITY_POLICY, crate::protocol::MAIL_CSP);
    }
    response.body(Cow::Owned(body.into_bytes())).ok()
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct BeaconInfo {
    pub port: u16,
    pub hits: u32,
}

#[tauri::command]
#[specta::specta]
pub fn spike_attacks() -> Vec<String> {
    ATTACKS.iter().map(|(name, _)| (*name).to_string()).collect()
}

#[tauri::command]
#[specta::specta]
pub fn spike_beacon(beacon: State<'_, Beacon>) -> BeaconInfo {
    BeaconInfo { port: beacon.port, hits: beacon.hits.load(Ordering::SeqCst) }
}

#[tauri::command]
#[specta::specta]
pub fn spike_beacon_reset(beacon: State<'_, Beacon>) {
    beacon.hits.store(0, Ordering::SeqCst);
}

#[tauri::command]
#[specta::specta]
pub fn spike_report(name: String, payload: String) {
    tracing::info!(target: "spike", "SPIKE {name} {payload}");
}
