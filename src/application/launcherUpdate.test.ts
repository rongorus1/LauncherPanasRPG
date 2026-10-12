import { afterEach, describe, expect, it } from "vitest";
import { InMemoryLauncherGateway, MEMORY_SCENARIOS, type MemoryScenario } from "../adapters/memory/InMemoryLauncherGateway";
import { createLauncherStore, type AppSnapshot, type LauncherStore } from "./launcherStore";

let gateway: InMemoryLauncherGateway;
let stopStore: () => void = () => {};
let restarted = 0;
const logged: string[] = [];

async function setup(scenario: MemoryScenario): Promise<LauncherStore> {
  gateway = new InMemoryLauncherGateway({ ...scenario, pace: 0 }, () => restarted++);
  const store = createLauncherStore(gateway, { log: (m) => logged.push(m) });
  stopStore = store.start();
  await store.load();
  await until(store, (s) => s.updateCheck !== "checking");
  await store.checkLauncherUpdate();
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
  restarted = 0;
  logged.length = 0;
});

describe("launcher self-update", () => {
  it("stays quiet when the launcher is up to date", async () => {
    const store = await setup(MEMORY_SCENARIOS.installed);
    expect(store.getSnapshot().launcherUpdate.status).toBe("none");
  });

  it("ignores a failed check, only logging it", async () => {
    const store = await setup({ ...MEMORY_SCENARIOS["launcher-update"], offline: true });
    expect(store.getSnapshot().launcherUpdate.status).toBe("none");
    expect(store.getSnapshot().notice).toBeNull();
    expect(logged.some((m) => m.includes("network"))).toBe(true);
  });

  it("offers the new version and downloads it with progress before restarting", async () => {
    const store = await setup(MEMORY_SCENARIOS["launcher-update"]);
    expect(store.getSnapshot().launcherUpdate).toMatchObject({ status: "available", info: { version: "0.2.0" } });

    const totals: Array<number | null> = [];
    const unsubscribe = store.subscribe(() => {
      const p = store.getSnapshot().launcherUpdate.progress;
      if (p) totals.push(p.total);
    });
    void store.installLauncherUpdate();
    await until(store, () => restarted === 1);
    unsubscribe();

    const { status, progress } = store.getSnapshot().launcherUpdate;
    expect(status).toBe("downloading");
    expect(progress?.downloaded).toBe(progress?.total);
    expect(totals.length).toBeGreaterThan(10);
  });

  it("does not start while the game is running", async () => {
    const store = await setup(MEMORY_SCENARIOS["launcher-update"]);
    await store.play();
    expect(store.getSnapshot().launcher?.activity).toBe("playing");
    await store.installLauncherUpdate();
    expect(store.getSnapshot().launcherUpdate.status).toBe("available");
    await store.stop();
  });

  it("moves to failed on a download error and can retry", async () => {
    const store = await setup(MEMORY_SCENARIOS["launcher-update-failing"]);
    await store.installLauncherUpdate();
    const failed = store.getSnapshot().launcherUpdate;
    expect(failed.status).toBe("failed");
    expect(failed.error?.kind).toBe("network");

    await store.installLauncherUpdate();
    expect(store.getSnapshot().launcherUpdate.status).toBe("failed");
  });
});
