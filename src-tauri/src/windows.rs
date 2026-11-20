use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Runtime, Url, WebviewWindow, WebviewWindowBuilder};

pub const MAIN_WINDOW: &str = "main";

fn is_app_url(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "zsm" | "about" | "blob" | "data" => true,
        "http" | "https" => {
            let host = url.host_str().unwrap_or_default();
            host == "tauri.localhost" || host == "zsm.localhost" || (cfg!(debug_assertions) && host == "localhost")
        }
        _ => false,
    }
}

fn open_in_browser(url: &Url) {
    if matches!(url.scheme(), "http" | "https" | "mailto") {
        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
    }
}

pub fn create_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_WINDOW)
        .cloned()
        .ok_or_else(|| tauri::Error::WindowNotFound)?;
    WebviewWindowBuilder::from_config(app, &config)?
        .disable_drag_drop_handler()
        .on_navigation(|url| {
            if is_app_url(url) {
                return true;
            }
            open_in_browser(url);
            false
        })
        .on_new_window(|url, _features| {
            open_in_browser(&url);
            NewWindowResponse::Deny
        })
        .build()
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    match create_main_window(app) {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(err) => tracing::error!(%err, "could not recreate the main window"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn the_app_may_load_its_own_pages_and_nothing_else() {
        for ok in [
            "tauri://localhost/",
            "http://tauri.localhost/index.html",
            "zsm://localhost/msg/1",
            "http://zsm.localhost/msg/1",
            "about:blank",
        ] {
            assert!(is_app_url(&url(ok)), "{ok}");
        }
        for bad in [
            "https://example.com/",
            "http://127.0.0.1:8080/",
            "file:///etc/passwd",
            "ftp://x.y/",
            "javascript:alert(1)",
            "https://tauri.localhost.evil.example/",
        ] {
            assert!(!is_app_url(&url(bad)), "{bad}");
        }
    }
}
