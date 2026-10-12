import { afterEach, describe, expect, it } from "vitest";
import { InMemoryLauncherGateway, type MemoryScenario } from "../adapters/memory/InMemoryLauncherGateway";
import { DEFAULT_SETTINGS } from "../domain/settings";
import { createLauncherStore, type AppSnapshot, type LauncherStore } from "./launcherStore";

let gateway: InMemoryLauncherGateway;
let stopStore: () => void = () => {};

async function setup(scenario: MemoryScenario): Promise<LauncherStore> {
  gateway = new InMemoryLauncherGateway({ pace: 0, ...scenario });
  const store = createLauncherStore(gateway);
  stopStore = store.start();
  await store.load();
  return store;
}

async function until(store: LauncherStore, check: (s: AppSnapshot) => boolean) {
  for (let i = 0; i < 200; i++) {
    if (check(store.getSnapshot())) return;
    await new Promise((r) => setTimeout(r, 0));
  }
  throw new Error("condition not reached");
}

afterEach(() => {
  stopStore();
  gateway.dispose();
});

describe("launcher store", () => {
  it("loads the state and checks for updates", async () => {
    const store = await setup({ username: "Rongo", installedVersion: "1.6.2" });
    const s = store.getSnapshot();
    expect(s.phase).toBe("ready");
    expect(s.updateCheck).toBe("done");
    expect(s.launcher?.pack.update_available).toBe(true);
    expect(s.launcher?.pack.latest_version).toBe("1.7.0");
  });

  it("marks the update check as offline on a network error but keeps the installed pack", async () => {
    const store = await setup({ username: "Rongo", installedVersion: "1.6.2", offline: true });
    const s = store.getSnapshot();
    expect(s.updateCheck).toBe("offline");
    expect(s.notice).toBeNull();
    expect(s.launcher?.pack.installed_version).toBe("1.6.2");
  });

  it("reports a failed load and recovers on retry", async () => {
    const store = await setup({ failLoads: 1 });
    expect(store.getSnapshot().phase).toBe("failed");
    expect(store.getSnapshot().loadError?.kind).toBe("io");
    await store.load();
    expect(store.getSnapshot().phase).toBe("ready");
  });

  it("installs the pack, tracking progress through events", async () => {
    const store = await setup({});
    const seen = new Set<string>();
    const unsubscribe = store.subscribe(() => {
      const step = store.getSnapshot().progress?.step;
      if (step) seen.add(step);
    });
    await store.install();
    await until(store, (s) => s.launcher?.pack.installed_version === "1.7.0");
    unsubscribe();
    expect(seen).toContain("downloading_mods");
    expect(store.getSnapshot().launcher?.activity).toBe("idle");
    expect(store.getSnapshot().progress).toBeNull();
  });

  it("shows a single install failure notice with a retry", async () => {
    const store = await setup({ failInstallAt: "downloading_mods" });
    await store.install();
    const s = store.getSnapshot();
    expect(s.notice).toEqual({ type: "install_failed", detail: expect.stringContaining("downloading_mods") });
    expect(s.launcher?.activity).toBe("idle");
  });

  it("refuses to play without an account", async () => {
    const store = await setup({ installedVersion: "1.7.0" });
    await store.play();
    const notice = store.getSnapshot().notice;
    expect(notice?.type === "error" && notice.error.kind).toBe("no_account");
  });

  it("plays, collects logs and stops without a crash notice", async () => {
    const store = await setup({ username: "Rongo", installedVersion: "1.7.0" });
    await store.play();
    expect(store.getSnapshot().launcher?.activity).toBe("playing");
    store.flushLogs();
    expect(store.getSnapshot().logs.some((l) => l.stream === "stderr")).toBe(true);
    await store.stop();
    expect(store.getSnapshot().launcher?.activity).toBe("idle");
    expect(store.getSnapshot().notice).toBeNull();
    store.clearLogs();
    expect(store.getSnapshot().logs).toHaveLength(0);
  });

  it("reports a crash when the game exits with an error code", async () => {
    const store = await setup({ username: "Rongo", installedVersion: "1.7.0", crashCode: 1 });
    await store.play();
    expect(store.getSnapshot().notice).toEqual({ type: "game_crashed", code: 1 });
    expect(store.getSnapshot().launcher?.activity).toBe("idle");
  });

  it("saves the account and the settings into the state", async () => {
    const store = await setup({});
    const account = await store.saveAccount("Nueva_Pana");
    expect(account.ok).toBe(true);
    expect(store.getSnapshot().launcher?.account?.username).toBe("Nueva_Pana");

    const bad = await store.saveAccount("x");
    expect(bad.ok === false && bad.error.kind).toBe("invalid_input");

    const saved = await store.saveSettings({ ...DEFAULT_SETTINGS, max_memory_mb: 8192 });
    expect(saved.ok).toBe(true);
    expect(store.getSnapshot().launcher?.settings.max_memory_mb).toBe(8192);
  });

  it("caps the log buffer", async () => {
    gateway = new InMemoryLauncherGateway({ pace: 0, username: "Rongo", installedVersion: "1.7.0", crashCode: 1 });
    const store = createLauncherStore(gateway, { maxLogLines: 3 });
    stopStore = store.start();
    await store.load();
    await store.play();
    const logs = store.getSnapshot().logs;
    expect(logs).toHaveLength(3);
    expect(logs[2].text).toMatch(/FATAL/);
  });
});
