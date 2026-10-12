import { describe, expect, it } from "vitest";
import {
  formatMb,
  formatUpdateProgress,
  isUpdateDownloaded,
  launcherUpdateBlockedReason,
  updateFraction,
} from "./launcherUpdate";

const MB = 1024 * 1024;

describe("launcher update", () => {
  it("formats bytes as MB with a Spanish decimal comma", () => {
    expect(formatMb(3.2 * MB)).toBe("3,2 MB");
    expect(formatUpdateProgress({ downloaded: 3.2 * MB, total: 8.4 * MB })).toBe("3,2 de 8,4 MB");
    expect(formatUpdateProgress({ downloaded: 3.2 * MB, total: null })).toBe("3,2 MB descargados");
  });

  it("only reports a fraction when the size is known", () => {
    expect(updateFraction({ downloaded: 5, total: 10 })).toBe(0.5);
    expect(updateFraction({ downloaded: 5, total: null })).toBeNull();
    expect(updateFraction({ downloaded: 50, total: 10 })).toBe(1);
    expect(isUpdateDownloaded({ downloaded: 10, total: 10 })).toBe(true);
    expect(isUpdateDownloaded({ downloaded: 10, total: null })).toBe(false);
  });

  it("is blocked while the pack installs or the game runs", () => {
    expect(launcherUpdateBlockedReason("idle")).toBeNull();
    expect(launcherUpdateBlockedReason("installing")).toMatch(/instalación/);
    expect(launcherUpdateBlockedReason("playing")).toMatch(/juego/);
  });
});
