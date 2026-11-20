use serde::{Deserialize, Serialize};
use tauri_specta::Event;
use zsm_app::{CoreEvent, EventSink};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct ZsmEvent(pub CoreEvent);

pub struct TauriSink<R: tauri::Runtime>(pub tauri::AppHandle<R>);

impl<R: tauri::Runtime> EventSink for TauriSink<R> {
    fn emit(&self, event: CoreEvent) {
        if let Err(err) = ZsmEvent(event).emit(&self.0) {
            tracing::warn!(%err, "could not deliver an event to the window");
        }
    }
}
