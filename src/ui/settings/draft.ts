import { gbToMb, mbToGb, normalizeJavaPath } from "../../domain/settings";
import type { Settings } from "../../domain/types";

/** Text inputs keep what the player typed (including "6,5") until it is parsed. */
export interface SettingsDraft {
  maxGb: string;
  minGb: string;
  jvmArgs: string;
  javaPath: string;
  width: string;
  height: string;
  fullscreen: boolean;
  closeOnPlay: boolean;
}

const decimal = new Intl.NumberFormat("es-ES", { maximumFractionDigits: 2, useGrouping: false });

export function formatDecimal(n: number): string {
  return decimal.format(n);
}

export function parseDecimal(raw: string): number {
  const cleaned = raw.trim().replace(",", ".");
  if (cleaned === "" || !/^\d+(\.\d+)?$/.test(cleaned)) return Number.NaN;
  return Number(cleaned);
}

function parseInteger(raw: string): number {
  const cleaned = raw.trim();
  return /^\d+$/.test(cleaned) ? Number(cleaned) : Number.NaN;
}

export function toDraft(s: Settings): SettingsDraft {
  return {
    maxGb: formatDecimal(mbToGb(s.max_memory_mb)),
    minGb: formatDecimal(mbToGb(s.min_memory_mb)),
    jvmArgs: s.extra_jvm_args,
    javaPath: s.java_path ?? "",
    width: String(s.window_width),
    height: String(s.window_height),
    fullscreen: s.fullscreen,
    closeOnPlay: s.close_launcher_on_play,
  };
}

export function fromDraft(d: SettingsDraft): Settings {
  return {
    max_memory_mb: gbToMb(parseDecimal(d.maxGb)),
    min_memory_mb: gbToMb(parseDecimal(d.minGb)),
    extra_jvm_args: d.jvmArgs.trim(),
    java_path: normalizeJavaPath(d.javaPath),
    window_width: parseInteger(d.width),
    window_height: parseInteger(d.height),
    fullscreen: d.fullscreen,
    close_launcher_on_play: d.closeOnPlay,
  };
}
