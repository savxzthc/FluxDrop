use serde::Serialize;

#[cfg(not(test))]
use tauri::{AppHandle, Emitter};

#[cfg(not(test))]
pub type EventHandle = AppHandle;

#[cfg(test)]
pub type EventHandle = ();

#[cfg(not(test))]
pub fn emit_share_status<T: Serialize>(app: Option<&EventHandle>, event: &str, payload: &T) {
    if let Some(app) = app {
        let _ = app.emit(event, payload);
        crate::tray::update_for_event(app, event);
    }
}

#[cfg(test)]
pub fn emit_share_status<T: Serialize>(_app: Option<&EventHandle>, _event: &str, _payload: &T) {}
