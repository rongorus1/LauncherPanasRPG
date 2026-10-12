//! Use cases implement the inbound ports by orchestrating domain rules and outbound ports.
//! One file per use case, each with unit tests that use in-memory fakes for the ports.

mod activity;
mod check_launcher_update;
mod check_pack_update;
mod get_launcher_state;
mod install_launcher_update;
mod install_pack;
mod play_game;
mod reporting;
mod save_offline_account;
mod save_settings;
mod stop_game;

#[cfg(test)]
pub(crate) mod testing;

pub use activity::{ActivityGuard, ActivityTracker};
pub use check_launcher_update::CheckLauncherUpdateUseCase;
pub use check_pack_update::CheckPackUpdateUseCase;
pub use get_launcher_state::GetLauncherStateUseCase;
pub use install_launcher_update::InstallLauncherUpdateUseCase;
pub use install_pack::{InstallPackDeps, InstallPackUseCase};
pub use play_game::{PlayGameDeps, PlayGameUseCase};
pub use reporting::{EventLogSink, EventProgressReporter};
pub use save_offline_account::SaveOfflineAccountUseCase;
pub use save_settings::SaveSettingsUseCase;
pub use stop_game::StopGameUseCase;
