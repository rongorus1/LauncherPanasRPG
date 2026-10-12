import type { LauncherState } from "./types";

export type PrimaryAction = "install" | "update" | "play" | "stop" | "installing";

export function primaryAction(state: LauncherState): PrimaryAction {
  if (state.activity === "installing") return "installing";
  if (state.activity === "playing") return "stop";
  if (state.pack.installed_version === null) return "install";
  if (state.pack.update_available) return "update";
  return "play";
}

/** Playing the installed version is still possible while an update is pending. */
export function canPlayInstalled(state: LauncherState): boolean {
  return state.activity === "idle" && state.pack.installed_version !== null;
}
