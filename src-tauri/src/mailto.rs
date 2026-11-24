use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::windows;

pub const EVENT: &str = "zsm-mailto";

const LONGEST: usize = 8 * 1024;

static PENDING: Mutex<Option<String>> = Mutex::new(None);

pub fn find<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().map(|a| a.as_ref().trim().to_string()).find(|a| {
        a.len() > "mailto:".len()
            && a.len() <= LONGEST
            && a.get(.."mailto:".len()).is_some_and(|p| p.eq_ignore_ascii_case("mailto:"))
    })
}

pub fn hold(url: String) {
    if let Ok(mut pending) = PENDING.lock() {
        *pending = Some(url);
    }
}

pub fn take() -> Option<String> {
    PENDING.lock().ok().and_then(|mut pending| pending.take())
}

pub fn script() -> Option<String> {
    let url = take()?;
    let quoted = serde_json::to_string(&url).ok()?;
    Some(format!("window.__ZSM_MAILTO__ = {quoted};"))
}

pub fn deliver<R: Runtime>(app: &AppHandle<R>, url: &str) {
    if app.get_webview_window(windows::MAIN_WINDOW).is_some() {
        windows::show_main_window(app);
        let _ = app.emit(EVENT, url);
    } else {
        hold(url.to_string());
        windows::show_main_window(app);
    }
}

#[cfg(test)]
mod tests {
    use super::find;

    #[test]
    fn a_mailto_address_is_found_among_the_launch_arguments() {
        assert_eq!(
            find(["zsortmail", "mailto:a@b.example?subject=Hi"]).as_deref(),
            Some("mailto:a@b.example?subject=Hi")
        );
        assert_eq!(find(["MAILTO:a@b.example"]).as_deref(), Some("MAILTO:a@b.example"));
        assert_eq!(find(["--flag", "https://example.com", "mailto:"]), None);
        assert_eq!(find(["é"]), None);
        assert_eq!(find([format!("mailto:{}", "a".repeat(9000))]), None, "an absurdly long one is ignored");
    }
}
