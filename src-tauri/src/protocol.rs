use std::borrow::Cow;
use std::sync::Arc;

use tauri::Manager;
use tauri::http::{Request, Response, StatusCode, header};
use zsm_app::Core;

#[cfg(debug_assertions)]
pub const MAIL_CSP: &str = "default-src 'none'; img-src zsm: http://zsm.localhost data:; \
style-src 'unsafe-inline'; font-src zsm: http://zsm.localhost data:; base-uri 'none'; \
form-action 'none'; frame-src 'none'";

pub fn handle<R: tauri::Runtime>(
    ctx: tauri::UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    #[cfg(debug_assertions)]
    if let Some(response) = crate::spikes::handle(&ctx, &request) {
        return response;
    }

    let Some(core) = ctx.app_handle().try_state::<Arc<Core>>() else {
        return plain(StatusCode::SERVICE_UNAVAILABLE, "Starting up");
    };
    let served = core.serve(request.uri().path(), request.uri().query());
    let mut response = Response::builder()
        .status(StatusCode::from_u16(served.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, served.content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
    for (name, value) in served.headers {
        response = response.header(name, value);
    }
    response.body(Cow::Owned(served.body)).unwrap_or_else(|_| plain(StatusCode::INTERNAL_SERVER_ERROR, "Error"))
}

fn plain(status: StatusCode, body: &'static str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_SECURITY_POLICY, "default-src 'none'")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Borrowed(body.as_bytes()))
        .expect("static response is valid")
}
