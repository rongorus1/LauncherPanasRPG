import type {
  AccountView,
  LauncherEvent,
  LauncherState,
  LauncherUpdateInfo,
  PackStatus,
  Settings,
} from "../domain/types";

/**
 * Outbound port to the launcher backend. Every method rejects with an `AppError`
 * (`{ kind, detail }`) when the backend refuses the request.
 */
export interface LauncherGateway {
  getLauncherState(): Promise<LauncherState>;
  saveOfflineAccount(username: string): Promise<AccountView>;
  saveSettings(settings: Settings): Promise<Settings>;
  checkPackUpdate(): Promise<PackStatus>;
  installPack(): Promise<void>;
  playGame(): Promise<void>;
  stopGame(): Promise<void>;
  /** `null` when the launcher is up to date. */
  checkLauncherUpdate(): Promise<LauncherUpdateInfo | null>;
  /** Downloads, installs and restarts the app, so it may never resolve. */
  installLauncherUpdate(): Promise<void>;
  onEvent(handler: (event: LauncherEvent) => void): () => void;
}
