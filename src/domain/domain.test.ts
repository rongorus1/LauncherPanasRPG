import { describe, expect, it } from "vitest";
import { errorCopy, isAppError, toAppError } from "./errors";
import { canPlayInstalled, primaryAction } from "./primaryAction";
import { formatCounter, isIndeterminate, overallFraction, stepLabel } from "./progress";
import { DEFAULT_SETTINGS, formatGb, gbToMb, snapGb, validateSettings } from "./settings";
import type { LauncherState } from "./types";
import { checkUsername } from "./username";

describe("checkUsername", () => {
  it("accepts 3 to 16 chars of letters, digits and underscore, trimmed", () => {
    expect(checkUsername("  Rongo_42 ")).toEqual({ ok: true, value: "Rongo_42" });
    expect(checkUsername("abc").ok).toBe(true);
    expect(checkUsername("a".repeat(16)).ok).toBe(true);
  });

  it("reports the specific problem", () => {
    expect(checkUsername("   ")).toEqual({ ok: false, problem: "empty" });
    expect(checkUsername("ab")).toEqual({ ok: false, problem: "too_short" });
    expect(checkUsername("a".repeat(17))).toEqual({ ok: false, problem: "too_long" });
    expect(checkUsername("Peña")).toEqual({ ok: false, problem: "invalid_chars" });
    expect(checkUsername("dos palabras")).toEqual({ ok: false, problem: "invalid_chars" });
  });
});

describe("validateSettings", () => {
  it("accepts the defaults", () => {
    expect(validateSettings(DEFAULT_SETTINGS)).toEqual({});
  });

  it("mirrors the backend bounds", () => {
    expect(validateSettings({ ...DEFAULT_SETTINGS, max_memory_mb: 512 }).max_memory_mb).toBeDefined();
    expect(validateSettings({ ...DEFAULT_SETTINGS, max_memory_mb: 70000 }).max_memory_mb).toBeDefined();
    expect(validateSettings({ ...DEFAULT_SETTINGS, min_memory_mb: 128 }).min_memory_mb).toBeDefined();
    expect(validateSettings({ ...DEFAULT_SETTINGS, min_memory_mb: 8192 }).min_memory_mb).toBe(
      "No puede superar la memoria máxima.",
    );
    expect(validateSettings({ ...DEFAULT_SETTINGS, window_width: 639 }).window_width).toBeDefined();
    expect(validateSettings({ ...DEFAULT_SETTINGS, window_height: 479 }).window_height).toBeDefined();
    expect(validateSettings({ ...DEFAULT_SETTINGS, max_memory_mb: Number.NaN }).max_memory_mb).toBeDefined();
  });

  it("converts and formats memory in GB", () => {
    expect(gbToMb(6.5)).toBe(6656);
    expect(snapGb(6.3)).toBe(6.5);
    expect(formatGb(6656)).toBe("6,5 GB");
  });
});

describe("progress", () => {
  it("labels steps in Spanish", () => {
    expect(stepLabel("downloading_mods")).toBe("Descargando mods");
    expect(stepLabel("preparing_java")).toBe("Preparando Java");
  });

  it("grows monotonically across steps", () => {
    const a = overallFraction({ step: "downloading_minecraft", current: 48, total: 48, message: "" });
    const b = overallFraction({ step: "installing_forge", current: 0, total: 0, message: "" });
    const c = overallFraction({ step: "downloading_mods", current: 10, total: 36, message: "" });
    expect(a).toBeLessThanOrEqual(b);
    expect(b).toBeLessThan(c);
    expect(overallFraction({ step: "finished", current: 0, total: 0, message: "" })).toBe(1);
  });

  it("treats total 0 as unknown", () => {
    const p = { step: "installing_forge" as const, current: 0, total: 0, message: "" };
    expect(isIndeterminate(p)).toBe(true);
    expect(formatCounter(p)).toBeNull();
    expect(formatCounter({ ...p, current: 1240, total: 3120 })).toBe("1240 de 3120");
  });
});

describe("errors", () => {
  it("normalizes anything thrown into an AppError", () => {
    expect(isAppError({ kind: "network", detail: "x" })).toBe(true);
    expect(isAppError({ kind: "nope", detail: "x" })).toBe(false);
    expect(toAppError(new Error("boom"))).toEqual({ kind: "internal", detail: "boom" });
    expect(toAppError("raw")).toEqual({ kind: "internal", detail: "raw" });
  });

  it("has Spanish copy for every kind", () => {
    expect(errorCopy("network").title).toMatch(/conexión/);
  });
});

describe("primaryAction", () => {
  const base: LauncherState = {
    account: { username: "Rongo", uuid: "u" },
    settings: DEFAULT_SETTINGS,
    pack: {
      installed_version: "1.6.2",
      minecraft_version: "1.20.1",
      forge_version: "47.4.10",
      latest_version: "1.7.0",
      update_available: true,
    },
    activity: "idle",
  };

  it("picks the button for each state", () => {
    expect(primaryAction(base)).toBe("update");
    expect(primaryAction({ ...base, pack: { ...base.pack, update_available: false } })).toBe("play");
    expect(primaryAction({ ...base, pack: { ...base.pack, installed_version: null } })).toBe("install");
    expect(primaryAction({ ...base, activity: "installing" })).toBe("installing");
    expect(primaryAction({ ...base, activity: "playing" })).toBe("stop");
  });

  it("allows playing the installed version while an update is pending", () => {
    expect(canPlayInstalled(base)).toBe(true);
    expect(canPlayInstalled({ ...base, activity: "playing" })).toBe(false);
  });
});
