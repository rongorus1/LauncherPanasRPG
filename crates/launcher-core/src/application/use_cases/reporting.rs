use std::sync::Arc;

use crate::application::{
    events::{LauncherEvent, LogStream},
    ports::outbound::{EventPublisher, GameLogSink, ProgressReporter},
};
use crate::domain::Progress;

/// Turns progress reports into `LauncherEvent::InstallProgress`.
#[derive(Clone)]
pub struct EventProgressReporter {
    events: Arc<dyn EventPublisher>,
}

impl EventProgressReporter {
    pub fn new(events: Arc<dyn EventPublisher>) -> Self {
        Self { events }
    }
}

impl ProgressReporter for EventProgressReporter {
    fn report(&self, progress: Progress) {
        self.events
            .publish(LauncherEvent::InstallProgress(progress));
    }
}

/// Turns game output into `LauncherEvent::GameLog`.
pub struct EventLogSink {
    events: Arc<dyn EventPublisher>,
}

impl EventLogSink {
    pub fn new(events: Arc<dyn EventPublisher>) -> Self {
        Self { events }
    }
}

impl GameLogSink for EventLogSink {
    fn line(&self, stream: LogStream, line: String) {
        self.events.publish(LauncherEvent::GameLog { stream, line });
    }
}
