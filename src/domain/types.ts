// Mirrors the Rust contract in crates/launcher-core (serde snake_case).

export type Activity = "idle" | "installing" | "playing";

export interface AccountView {
  username: string;
  uuid: string;
}

export interface Settings {
  max_memory_mb: number;
  min_memory_mb: number;
  extra_jvm_args: string;
  java_path: string | null;
  window_width: number;
  window_height: number;
  fullscreen: boolean;
  close_launcher_on_play: boolean;
}

export interface PackStatus {
  installed_version: string | null;
  minecraft_version: string | null;
  forge_version: string | null;
  /** `null` when the latest version could not be checked (offline). */
  latest_version: string | null;
  update_available: boolean;
}

export interface LauncherState {
  account: AccountView | null;
  settings: Settings;
  pack: PackStatus;
  activity: Activity;
}

export type InstallStep =
  | "checking_pack"
  | "preparing_java"
  | "downloading_minecraft"
  | "installing_forge"
  | "downloading_mods"
  | "applying_files"
  | "finished";

/** `total === 0` means the size of the step is unknown. */
export interface Progress {
  step: InstallStep;
  current: number;
  total: number;
  message: string;
}

export interface LauncherUpdateInfo {
  version: string;
  notes: string | null;
}

/** Bytes of the launcher installer; `total` is `null` when the server does not send a size. */
export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export type LogStream = "stdout" | "stderr";

export type LauncherEvent =
  | ({ type: "install_progress" } & Progress)
  | { type: "install_finished"; version: string }
  | { type: "install_failed"; detail: string }
  | { type: "game_started" }
  | { type: "game_log"; stream: LogStream; line: string }
  | { type: "game_exited"; code: number | null }
  | ({ type: "update_progress" } & UpdateProgress);

export type AppErrorKind =
  | "invalid_input"
  | "no_account"
  | "not_installed"
  | "busy"
  | "network"
  | "integrity"
  | "io"
  | "internal";

export interface AppError {
  kind: AppErrorKind;
  detail: string;
}
