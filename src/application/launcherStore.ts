import { toAppError } from "../domain/errors";
import { launcherUpdateBlockedReason } from "../domain/launcherUpdate";
import type {
  AccountView,
  AppError,
  LauncherEvent,
  LauncherState,
  LauncherUpdateInfo,
  LogStream,
  Progress,
  Settings,
  UpdateProgress,
} from "../domain/types";
import type { LauncherGateway } from "./LauncherGateway";

export type RetryAction = "install" | "play" | "update_check";

export type Notice =
  | { type: "error"; error: AppError; retry: RetryAction | null }
  | { type: "install_failed"; detail: string }
  | { type: "game_crashed"; code: number };

export type UpdateCheck = "idle" | "checking" | "done" | "offline";

export type Pending = "play" | "stop" | null;

export interface LogLine {
  id: number;
  stream: LogStream;
  text: string;
}

export interface LauncherUpdateState {
  status: "none" | "available" | "downloading" | "failed";
  info: LauncherUpdateInfo | null;
  progress: UpdateProgress | null;
  error: AppError | null;
}

export interface AppSnapshot {
  phase: "loading" | "ready" | "failed";
  loadError: AppError | null;
  launcher: LauncherState | null;
  progress: Progress | null;
  updateCheck: UpdateCheck;
  pending: Pending;
  notice: Notice | null;
  logs: readonly LogLine[];
  launcherUpdate: LauncherUpdateState;
}

export type Result<T> = { ok: true; value: T } | { ok: false; error: AppError };

export interface LauncherStore {
  subscribe(listener: () => void): () => void;
  getSnapshot(): AppSnapshot;
  start(): () => void;
  load(): Promise<void>;
  checkUpdate(): Promise<void>;
  install(): Promise<void>;
  play(): Promise<void>;
  stop(): Promise<void>;
  retry(action: RetryAction): Promise<void>;
  saveAccount(username: string): Promise<Result<AccountView>>;
  saveSettings(settings: Settings): Promise<Result<Settings>>;
  dismissNotice(): void;
  clearLogs(): void;
  flushLogs(): void;
  checkLauncherUpdate(): Promise<void>;
  installLauncherUpdate(): Promise<void>;
}

export interface StoreOptions {
  /** Game output can arrive thousands of lines per second; lines are batched before rendering. */
  logFlushMs?: number;
  maxLogLines?: number;
  /** Where ignored launcher-update check failures go. */
  log?: (message: string) => void;
}

const INITIAL: AppSnapshot = {
  phase: "loading",
  loadError: null,
  launcher: null,
  progress: null,
  updateCheck: "idle",
  pending: null,
  notice: null,
  logs: [],
  launcherUpdate: { status: "none", info: null, progress: null, error: null },
};

export function createLauncherStore(gateway: LauncherGateway, options: StoreOptions = {}): LauncherStore {
  const logFlushMs = options.logFlushMs ?? 50;
  const maxLogLines = options.maxLogLines ?? 5000;
  const log = options.log ?? ((message: string) => console.info(message));

  let snapshot = INITIAL;
  const listeners = new Set<() => void>();
  let queued: LogLine[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  let nextLogId = 1;
  let installFailedByEvent = false;
  let stopRequested = false;

  function set(patch: Partial<AppSnapshot>) {
    snapshot = { ...snapshot, ...patch };
    for (const listener of listeners) listener();
  }

  function patchLauncher(patch: Partial<LauncherState>) {
    if (!snapshot.launcher) return;
    set({ launcher: { ...snapshot.launcher, ...patch } });
  }

  function patchUpdate(patch: Partial<LauncherUpdateState>) {
    set({ launcherUpdate: { ...snapshot.launcherUpdate, ...patch } });
  }

  function fail(error: AppError, retry: RetryAction | null) {
    set({ notice: { type: "error", error, retry } });
  }

  function flushLogs() {
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    if (queued.length === 0) return;
    const merged = snapshot.logs.concat(queued);
    queued = [];
    set({ logs: merged.length > maxLogLines ? merged.slice(merged.length - maxLogLines) : merged });
  }

  function queueLog(stream: LogStream, text: string) {
    queued.push({ id: nextLogId++, stream, text });
    if (flushTimer === null) flushTimer = setTimeout(flushLogs, logFlushMs);
  }

  async function refreshState() {
    try {
      const launcher = await gateway.getLauncherState();
      set({ launcher });
    } catch (error) {
      fail(toAppError(error), null);
    }
  }

  function handleEvent(event: LauncherEvent) {
    switch (event.type) {
      case "install_progress": {
        const { type: _type, ...progress } = event;
        set({ progress });
        if (snapshot.launcher?.activity !== "installing") patchLauncher({ activity: "installing" });
        break;
      }
      case "install_finished":
        set({ progress: null, notice: null });
        patchLauncher({ activity: "idle" });
        void refreshState();
        break;
      case "install_failed":
        installFailedByEvent = true;
        set({ progress: null });
        patchLauncher({ activity: "idle" });
        set({ notice: { type: "install_failed", detail: event.detail } });
        break;
      case "game_started":
        set({ pending: null });
        patchLauncher({ activity: "playing" });
        break;
      case "game_log":
        queueLog(event.stream, event.line);
        break;
      case "game_exited": {
        flushLogs();
        const userStopped = stopRequested;
        stopRequested = false;
        set({ pending: null });
        patchLauncher({ activity: "idle" });
        if (!userStopped && event.code !== null && event.code !== 0) {
          set({ notice: { type: "game_crashed", code: event.code } });
        }
        break;
      }
      case "update_progress":
        patchUpdate({
          status: "downloading",
          progress: { downloaded: event.downloaded, total: event.total },
          error: null,
        });
        break;
    }
  }

  let loadInFlight: Promise<void> | null = null;

  // Concurrent calls (StrictMode runs `start` twice in development) share one request.
  function load(): Promise<void> {
    loadInFlight ??= runLoad().finally(() => {
      loadInFlight = null;
    });
    return loadInFlight;
  }

  async function runLoad() {
    set({ phase: "loading", loadError: null });
    try {
      const launcher = await gateway.getLauncherState();
      set({ phase: "ready", launcher });
    } catch (error) {
      set({ phase: "failed", loadError: toAppError(error) });
      return;
    }
    // Runs alongside the pack check; it never blocks the screen.
    void checkLauncherUpdate();
    await checkUpdate();
  }

  async function checkLauncherUpdate() {
    try {
      const info = await gateway.checkLauncherUpdate();
      if (snapshot.launcherUpdate.status === "downloading") return;
      patchUpdate(info ? { status: "available", info, progress: null, error: null } : { status: "none", info: null });
    } catch (error) {
      const appError = toAppError(error);
      log(`launcher update check failed (${appError.kind}): ${appError.detail}`);
    }
  }

  async function installLauncherUpdate() {
    const { status, info } = snapshot.launcherUpdate;
    const activity = snapshot.launcher?.activity;
    if (!info || (status !== "available" && status !== "failed")) return;
    if (!activity || launcherUpdateBlockedReason(activity) !== null) return;
    patchUpdate({ status: "downloading", progress: null, error: null });
    try {
      await gateway.installLauncherUpdate();
    } catch (error) {
      patchUpdate({ status: "failed", error: toAppError(error) });
    }
  }

  async function checkUpdate() {
    if (!snapshot.launcher) return;
    set({ updateCheck: "checking" });
    try {
      const pack = await gateway.checkPackUpdate();
      patchLauncher({ pack });
      set({ updateCheck: pack.latest_version === null ? "offline" : "done" });
    } catch (error) {
      const appError = toAppError(error);
      if (appError.kind === "network") {
        set({ updateCheck: "offline" });
      } else {
        set({ updateCheck: "idle" });
        fail(appError, "update_check");
      }
    }
  }

  async function install() {
    const launcher = snapshot.launcher;
    if (!launcher || launcher.activity !== "idle") return;
    installFailedByEvent = false;
    set({ notice: null, progress: null });
    patchLauncher({ activity: "installing" });
    try {
      await gateway.installPack();
    } catch (error) {
      if (!installFailedByEvent) {
        set({ progress: null });
        patchLauncher({ activity: "idle" });
        fail(toAppError(error), "install");
      }
    }
  }

  async function play() {
    const launcher = snapshot.launcher;
    if (!launcher || launcher.activity !== "idle" || snapshot.pending) return;
    stopRequested = false;
    set({ pending: "play", notice: null });
    try {
      await gateway.playGame();
    } catch (error) {
      set({ pending: null });
      fail(toAppError(error), "play");
    }
  }

  async function stop() {
    if (snapshot.launcher?.activity !== "playing") return;
    stopRequested = true;
    set({ pending: "stop" });
    try {
      await gateway.stopGame();
    } catch (error) {
      stopRequested = false;
      set({ pending: null });
      fail(toAppError(error), null);
    }
  }

  async function saveAccount(username: string): Promise<Result<AccountView>> {
    try {
      const account = await gateway.saveOfflineAccount(username);
      patchLauncher({ account });
      return { ok: true, value: account };
    } catch (error) {
      return { ok: false, error: toAppError(error) };
    }
  }

  async function saveSettings(settings: Settings): Promise<Result<Settings>> {
    try {
      const saved = await gateway.saveSettings(settings);
      patchLauncher({ settings: saved });
      return { ok: true, value: saved };
    } catch (error) {
      return { ok: false, error: toAppError(error) };
    }
  }

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot: () => snapshot,
    start() {
      const unsubscribe = gateway.onEvent(handleEvent);
      if (snapshot.phase !== "ready") void load();
      return () => {
        unsubscribe();
        if (flushTimer !== null) clearTimeout(flushTimer);
        flushTimer = null;
      };
    },
    load,
    checkUpdate,
    install,
    play,
    stop,
    retry(action) {
      if (action === "install") return install();
      if (action === "play") return play();
      return checkUpdate();
    },
    saveAccount,
    saveSettings,
    dismissNotice: () => set({ notice: null }),
    clearLogs() {
      queued = [];
      set({ logs: [] });
    },
    flushLogs,
    checkLauncherUpdate,
    installLauncherUpdate,
  };
}
