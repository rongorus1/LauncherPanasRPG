use launcher_core::application::{events::LauncherEvent, ports::outbound::EventPublisher};
use tauri::{AppHandle, Emitter};

pub const EVENT_CHANNEL: &str = "launcher-event";

/// Delivers application events to the web view.
pub struct TauriEventPublisher {
    app: AppHandle,
}

impl TauriEventPublisher {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventPublisher for TauriEventPublisher {
    fn publish(&self, event: LauncherEvent) {
        if let Err(error) = self.app.emit(EVENT_CHANNEL, event) {
            eprintln!("could not emit launcher event: {error}");
        }
    }
}
