use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;
use zsm_app::{Notice, Platform};

use crate::windows::MAIN_WINDOW;

pub struct DesktopNotifications<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> Platform for DesktopNotifications<R> {
    fn notify(&self, notice: &Notice) {
        let focused = self.0.get_webview_window(MAIN_WINDOW).is_some_and(|w| w.is_focused().unwrap_or(false));
        if focused && !notice.urgent {
            return;
        }
        if let Err(err) = self.0.notification().builder().title(&notice.title).body(&notice.body).show() {
            tracing::warn!(%err, "the desktop would not show a notification");
        }
    }
}
