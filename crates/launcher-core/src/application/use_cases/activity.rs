use std::sync::{Arc, Mutex, MutexGuard};

use crate::application::{
    dto::Activity,
    error::{AppError, AppErrorKind},
};

/// What the launcher is doing right now, shared by every use case built by the composition root.
#[derive(Debug, Clone)]
pub struct ActivityTracker(Arc<Mutex<Activity>>);

impl Default for ActivityTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivityTracker {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Activity::Idle)))
    }

    pub fn current(&self) -> Activity {
        *self.lock()
    }

    /// Atomically moves from `Idle` to `activity`. The returned guard goes back to `Idle` when dropped.
    pub fn try_begin(&self, activity: Activity) -> Result<ActivityGuard, AppError> {
        let mut current = self.lock();
        if *current != Activity::Idle {
            return Err(AppError::new(
                AppErrorKind::Busy,
                format!("launcher is busy: {:?}", *current),
            ));
        }
        *current = activity;
        Ok(ActivityGuard {
            tracker: self.clone(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Activity> {
        // A poisoned lock only means a panic elsewhere; the enum inside is still valid.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug)]
#[must_use = "dropping the guard immediately ends the activity"]
pub struct ActivityGuard {
    tracker: ActivityTracker,
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        *self.tracker.lock() = Activity::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_activity_at_a_time_and_guard_resets_to_idle() {
        let tracker = ActivityTracker::new();
        let guard = tracker.try_begin(Activity::Installing).unwrap();
        assert_eq!(tracker.current(), Activity::Installing);
        assert_eq!(
            tracker.try_begin(Activity::Playing).unwrap_err().kind,
            AppErrorKind::Busy
        );
        drop(guard);
        assert_eq!(tracker.current(), Activity::Idle);
    }
}
