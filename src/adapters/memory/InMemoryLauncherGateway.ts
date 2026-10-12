import type { LauncherGateway } from "../../application/LauncherGateway";
import { DEFAULT_SETTINGS, hasErrors, validateSettings } from "../../domain/settings";
import { checkUsername } from "../../domain/username";
import type {
  AccountView,
  AppError,
  InstallStep,
  LauncherEvent,
  LauncherState,
  LauncherUpdateInfo,
  PackStatus,
  Settings,
} from "../../domain/types";

export interface MemoryScenario {
  username?: string | null;
  installedVersion?: string | null;
  latestVersion?: string;
  offline?: boolean;
  /** Number of `getLauncherState` calls that fail before it starts working. */
  failLoads?: number;
  /** The install aborts with a network error when it reaches this step. */
  failInstallAt?: InstallStep | null;
  /** The game exits with this code on its own after its startup logs. */
  crashCode?: number | null;
  /** A newer launcher build is published. */
  launcherUpdate?: LauncherUpdateInfo | null;
  /** The launcher download breaks with a network error once it passes this share (0 to 1). */
  failLauncherUpdateAt?: number | null;
  /** Multiplies every simulated delay. Tests use 0. */
  pace?: number;
}

// Demo size of the launcher installer for the simulation only.
const LAUNCHER_INSTALLER_BYTES = 8_806_400;
const LAUNCHER_CHUNKS = 40;

// Demo numbers for the simulation only; real totals come from the backend.
const INSTALL_PLAN: ReadonlyArray<{ step: InstallStep; total: number; message: (i: number) => string }> = [
  { step: "checking_pack", total: 1, message: () => "Leyendo el manifiesto del pack" },
  { step: "preparing_java", total: 0, message: () => "Java 17 (runtime de Mojang)" },
  { step: "downloading_minecraft", total: 48, message: (i) => `libraries/org/lwjgl/lwjgl-${i}.jar` },
  { step: "installing_forge", total: 0, message: () => "Ejecutando el instalador de Forge 47.4.10" },
  { step: "downloading_mods", total: 36, message: (i) => `mods/mod-${String(i).padStart(3, "0")}.jar` },
  { step: "applying_files", total: 12, message: (i) => `config/archivo-${i}.toml` },
];

const STARTUP_LOG: ReadonlyArray<readonly ["stdout" | "stderr", string]> = [
  ["stdout", "[main/INFO] [cpw.mods.modlauncher.Launcher/MODLAUNCHER]: ModLauncher running: args [--username, ...]"],
  ["stdout", "[main/INFO] [net.minecraftforge.fml.loading.FMLLoader/CORE]: Forge 47.4.10 for Minecraft 1.20.1"],
  ["stderr", "[main/WARN] [mixin/]: Reference map 'example.refmap.json' could not be read. Continuing anyway"],
  ["stdout", "[Render thread/INFO] [minecraft/Minecraft]: Setting user"],
  ["stdout", "[Render thread/INFO] [net.minecraftforge.common.ForgeMod/FORGEMOD]: Forge mod loading, version 47.4.10"],
  ["stdout", "[Render thread/INFO] [minecraft/ReloadableResourceManager]: Reloading ResourceManager: vanilla, mod_resources"],
  ["stderr", "[Worker-Main-4/WARN] [minecraft/ModelBakery]: Unable to load model: 'minecraft:missing'"],
  ["stdout", "[Render thread/INFO] [minecraft/SoundEngine]: Sound engine started"],
];

const AMBIENT_LOG = [
  "[Server thread/INFO] [minecraft/MinecraftServer]: Saving chunks for level 'ServerLevel[Mundo]'/minecraft:overworld",
  "[Render thread/INFO] [minecraft/ChatComponent]: [CHAT] Has conseguido el logro [Hora de minar]",
  "[Server thread/INFO] [minecraft/MinecraftServer]: Jugador ha entrado en la partida",
  "[Render thread/INFO] [minecraft/AdvancementsScreen]: Abriendo pantalla de progresos",
];

function fakeUuid(name: string): string {
  let h = 2166136261;
  const out: string[] = [];
  for (let i = 0; i < 32; i++) {
    h ^= name.charCodeAt(i % name.length) + i;
    h = Math.imul(h, 16777619) >>> 0;
    out.push((h & 15).toString(16));
  }
  const s = out.join("");
  return `${s.slice(0, 8)}-${s.slice(8, 12)}-3${s.slice(13, 16)}-${s.slice(16, 20)}-${s.slice(20)}`;
}

function err(kind: AppError["kind"], detail: string): AppError {
  return { kind, detail };
}

/** Simulates the whole backend so the UI runs in a plain browser and in tests. */
export class InMemoryLauncherGateway implements LauncherGateway {
  private state: LauncherState;
  private readonly scenario: Required<MemoryScenario>;
  private readonly handlers = new Set<(event: LauncherEvent) => void>();
  private timers = new Set<ReturnType<typeof setTimeout>>();
  private gameTimer: ReturnType<typeof setInterval> | null = null;
  private loadFailuresLeft: number;

  /** Called when a launcher update finishes, standing in for the real process restart. */
  private readonly onRestart: () => void;

  constructor(scenario: MemoryScenario = {}, onRestart: () => void = () => {}) {
    this.onRestart = onRestart;
    this.scenario = {
      username: null,
      installedVersion: null,
      latestVersion: "1.7.0",
      offline: false,
      failLoads: 0,
      failInstallAt: null,
      crashCode: null,
      launcherUpdate: null,
      failLauncherUpdateAt: null,
      pace: 1,
      ...scenario,
    };
    this.loadFailuresLeft = this.scenario.failLoads;
    const { username, installedVersion } = this.scenario;
    this.state = {
      account: username ? { username, uuid: fakeUuid(username) } : null,
      settings: { ...DEFAULT_SETTINGS },
      pack: this.packStatus(installedVersion, null),
      activity: "idle",
    };
  }

  private packStatus(installed: string | null, latest: string | null): PackStatus {
    return {
      installed_version: installed,
      minecraft_version: installed ? "1.20.1" : null,
      forge_version: installed ? "47.4.10" : null,
      latest_version: latest,
      update_available: installed !== null && latest !== null && installed !== latest,
    };
  }

  private wait(ms: number): Promise<void> {
    if (this.scenario.pace === 0) return Promise.resolve();
    return new Promise((resolve) => {
      const t = setTimeout(() => {
        this.timers.delete(t);
        resolve();
      }, ms * this.scenario.pace);
      this.timers.add(t);
    });
  }

  private emit(event: LauncherEvent) {
    for (const handler of this.handlers) handler(event);
  }

  async getLauncherState(): Promise<LauncherState> {
    await this.wait(450);
    if (this.loadFailuresLeft > 0) {
      this.loadFailuresLeft--;
      throw err("io", "Permission denied (os error 13): ~/.local/share/panas-rpg/launcher.json");
    }
    return structuredClone(this.state);
  }

  async saveOfflineAccount(username: string): Promise<AccountView> {
    await this.wait(200);
    const check = checkUsername(username);
    if (!check.ok) throw err("invalid_input", `invalid username: ${username}`);
    this.state.account = { username: check.value, uuid: fakeUuid(check.value) };
    return { ...this.state.account };
  }

  async saveSettings(settings: Settings): Promise<Settings> {
    await this.wait(250);
    if (hasErrors(validateSettings(settings))) throw err("invalid_input", "invalid setting");
    this.state.settings = { ...settings };
    return { ...settings };
  }

  async checkPackUpdate(): Promise<PackStatus> {
    await this.wait(700);
    if (this.scenario.offline) {
      throw err("network", "error sending request for url (https://api.curseforge.com/v1/mods/1493979/files)");
    }
    this.state.pack = this.packStatus(this.state.pack.installed_version, this.scenario.latestVersion);
    return { ...this.state.pack };
  }

  async installPack(): Promise<void> {
    if (this.state.activity !== "idle") throw err("busy", "another task is running");
    if (this.scenario.offline) throw err("network", "dns error: failed to lookup address information");
    this.state.activity = "installing";
    for (const { step, total, message } of INSTALL_PLAN) {
      if (step === this.scenario.failInstallAt) {
        this.state.activity = "idle";
        const detail = `${step}: connection reset by peer (os error 104)`;
        this.emit({ type: "install_failed", detail });
        throw err("network", detail);
      }
      if (total === 0) {
        this.emit({ type: "install_progress", step, current: 0, total: 0, message: message(0) });
        await this.wait(1600);
        continue;
      }
      for (let i = 0; i <= total; i++) {
        this.emit({ type: "install_progress", step, current: i, total, message: message(i) });
        await this.wait(70);
      }
    }
    this.emit({ type: "install_progress", step: "finished", current: 1, total: 1, message: "" });
    const version = this.scenario.latestVersion;
    this.state.pack = this.packStatus(version, version);
    this.state.activity = "idle";
    this.emit({ type: "install_finished", version });
  }

  async playGame(): Promise<void> {
    if (this.state.activity !== "idle") throw err("busy", "another task is running");
    if (!this.state.account) throw err("no_account", "no offline account saved");
    if (!this.state.pack.installed_version) throw err("not_installed", "pack is not installed");
    await this.wait(900);
    this.state.activity = "playing";
    this.emit({ type: "game_started" });
    for (const [stream, text] of STARTUP_LOG) {
      this.emit({ type: "game_log", stream, line: `[${this.clock()}] ${text}` });
      await this.wait(120);
    }
    if (this.scenario.crashCode !== null) {
      this.emit({
        type: "game_log",
        stream: "stderr",
        line: `[${this.clock()}] [Render thread/FATAL]: Unreported exception thrown!`,
      });
      this.finishGame(this.scenario.crashCode);
      return;
    }
    let n = 0;
    this.gameTimer = setInterval(() => {
      this.emit({ type: "game_log", stream: "stdout", line: `[${this.clock()}] ${AMBIENT_LOG[n++ % AMBIENT_LOG.length]}` });
    }, 1100);
  }

  async stopGame(): Promise<void> {
    if (this.state.activity !== "playing") throw err("busy", "the game is not running");
    await this.wait(400);
    this.finishGame(null);
  }

  async checkLauncherUpdate(): Promise<LauncherUpdateInfo | null> {
    await this.wait(900);
    if (this.scenario.offline) throw err("network", "error sending request for url (https://github.com/.../latest.json)");
    return this.scenario.launcherUpdate ? { ...this.scenario.launcherUpdate } : null;
  }

  async installLauncherUpdate(): Promise<void> {
    if (this.state.activity !== "idle") throw err("busy", "cannot update the launcher while the pack installs or the game runs");
    if (!this.scenario.launcherUpdate) throw err("network", "no release published");
    const failAt = this.scenario.failLauncherUpdateAt;
    for (let i = 0; i <= LAUNCHER_CHUNKS; i++) {
      const downloaded = Math.round((LAUNCHER_INSTALLER_BYTES * i) / LAUNCHER_CHUNKS);
      if (failAt !== null && i / LAUNCHER_CHUNKS > failAt) {
        throw err("network", `download interrupted at ${downloaded} bytes: connection reset by peer (os error 104)`);
      }
      this.emit({ type: "update_progress", downloaded, total: LAUNCHER_INSTALLER_BYTES });
      await this.wait(90);
    }
    await this.wait(1200);
    this.onRestart();
    // The real command never returns: the process is replaced by the new build.
    return new Promise<void>(() => {});
  }

  private finishGame(code: number | null) {
    if (this.gameTimer !== null) clearInterval(this.gameTimer);
    this.gameTimer = null;
    this.state.activity = "idle";
    this.emit({ type: "game_exited", code });
  }

  private clock(): string {
    return new Date().toTimeString().slice(0, 8);
  }

  onEvent(handler: (event: LauncherEvent) => void) {
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
    };
  }

  dispose() {
    for (const t of this.timers) clearTimeout(t);
    this.timers.clear();
    if (this.gameTimer !== null) clearInterval(this.gameTimer);
  }
}

export const MEMORY_SCENARIOS: Record<string, MemoryScenario> = {
  fresh: {},
  installed: { username: "Rongo_42", installedVersion: "1.7.0" },
  update: { username: "Rongo_42", installedVersion: "1.6.2" },
  offline: { username: "Rongo_42", installedVersion: "1.6.2", offline: true },
  broken: { failLoads: 1 },
  failing: { username: "Rongo_42", failInstallAt: "downloading_mods" },
  crash: { username: "Rongo_42", installedVersion: "1.7.0", crashCode: 1 },
  "launcher-update": {
    username: "Rongo_42",
    installedVersion: "1.7.0",
    launcherUpdate: { version: "0.2.0", notes: "Arranque más rápido y mejor aviso cuando no hay conexión." },
  },
  "launcher-update-failing": {
    username: "Rongo_42",
    installedVersion: "1.7.0",
    launcherUpdate: { version: "0.2.0", notes: null },
    failLauncherUpdateAt: 0.4,
  },
};
