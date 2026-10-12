import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { LauncherGateway } from "../../application/LauncherGateway";
import { toAppError } from "../../domain/errors";
import type {
  AccountView,
  LauncherEvent,
  LauncherState,
  LauncherUpdateInfo,
  PackStatus,
  Settings,
} from "../../domain/types";

const EVENT_CHANNEL = "launcher-event";

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toAppError(error);
  }
}

export class TauriLauncherGateway implements LauncherGateway {
  getLauncherState() {
    return call<LauncherState>("get_launcher_state");
  }

  saveOfflineAccount(username: string) {
    return call<AccountView>("save_offline_account", { username });
  }

  saveSettings(settings: Settings) {
    return call<Settings>("save_settings", { settings });
  }

  checkPackUpdate() {
    return call<PackStatus>("check_pack_update");
  }

  installPack() {
    return call<void>("install_pack");
  }

  playGame() {
    return call<void>("play_game");
  }

  stopGame() {
    return call<void>("stop_game");
  }

  checkLauncherUpdate() {
    return call<LauncherUpdateInfo | null>("check_launcher_update");
  }

  installLauncherUpdate() {
    return call<void>("install_launcher_update");
  }

  onEvent(handler: (event: LauncherEvent) => void) {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    // `listen` resolves asynchronously; an unsubscribe that arrives first must still detach.
    void listen<LauncherEvent>(EVENT_CHANNEL, (event) => handler(event.payload)).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }
}
