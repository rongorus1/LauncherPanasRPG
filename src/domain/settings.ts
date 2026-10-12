import type { Settings } from "./types";

export const MEMORY_MIN_MB = 1024;
export const MEMORY_MAX_MB = 65536;
export const MIN_MEMORY_FLOOR_MB = 256;
export const WINDOW_MIN_WIDTH = 640;
export const WINDOW_MIN_HEIGHT = 480;

export const DEFAULT_SETTINGS: Settings = {
  max_memory_mb: 6144,
  min_memory_mb: 2048,
  extra_jvm_args: "",
  java_path: null,
  window_width: 1280,
  window_height: 720,
  fullscreen: false,
  close_launcher_on_play: false,
};

export type SettingsField =
  | "max_memory_mb"
  | "min_memory_mb"
  | "java_path"
  | "window_width"
  | "window_height";

export type SettingsErrors = Partial<Record<SettingsField, string>>;

const gbFormat = new Intl.NumberFormat("es-ES", { maximumFractionDigits: 2 });

export function mbToGb(mb: number): number {
  return mb / 1024;
}

export function gbToMb(gb: number): number {
  return Math.round(gb * 1024);
}

/** Snaps a GB value to the slider grid (0.5 GB). */
export function snapGb(gb: number, step = 0.5): number {
  return Math.round(gb / step) * step;
}

export function formatGb(mb: number): string {
  return `${gbFormat.format(mbToGb(mb))} GB`;
}

function isWhole(n: number): boolean {
  return Number.isFinite(n) && Number.isInteger(n);
}

/** Same bounds as `Settings::validate` in launcher-core, with field-level messages for the form. */
export function validateSettings(s: Settings): SettingsErrors {
  const errors: SettingsErrors = {};

  if (!Number.isFinite(s.max_memory_mb)) {
    errors.max_memory_mb = "Escribe una cantidad de memoria.";
  } else if (s.max_memory_mb < MEMORY_MIN_MB || s.max_memory_mb > MEMORY_MAX_MB) {
    errors.max_memory_mb = `Debe estar entre ${formatGb(MEMORY_MIN_MB)} y ${formatGb(MEMORY_MAX_MB)}.`;
  }

  if (!Number.isFinite(s.min_memory_mb)) {
    errors.min_memory_mb = "Escribe una cantidad de memoria.";
  } else if (s.min_memory_mb < MIN_MEMORY_FLOOR_MB) {
    errors.min_memory_mb = `Debe ser al menos ${formatGb(MIN_MEMORY_FLOOR_MB)}.`;
  } else if (!errors.max_memory_mb && s.min_memory_mb > s.max_memory_mb) {
    errors.min_memory_mb = "No puede superar la memoria máxima.";
  }

  if (s.java_path !== null && s.java_path.trim() !== s.java_path) {
    errors.java_path = "Quita los espacios al principio o al final de la ruta.";
  }

  if (!isWhole(s.window_width) || s.window_width < WINDOW_MIN_WIDTH) {
    errors.window_width = `Mínimo ${WINDOW_MIN_WIDTH} píxeles.`;
  }
  if (!isWhole(s.window_height) || s.window_height < WINDOW_MIN_HEIGHT) {
    errors.window_height = `Mínimo ${WINDOW_MIN_HEIGHT} píxeles.`;
  }

  return errors;
}

export function hasErrors(errors: SettingsErrors): boolean {
  return Object.keys(errors).length > 0;
}

export function settingsEqual(a: Settings, b: Settings): boolean {
  return (
    a.max_memory_mb === b.max_memory_mb &&
    a.min_memory_mb === b.min_memory_mb &&
    a.extra_jvm_args === b.extra_jvm_args &&
    a.java_path === b.java_path &&
    a.window_width === b.window_width &&
    a.window_height === b.window_height &&
    a.fullscreen === b.fullscreen &&
    a.close_launcher_on_play === b.close_launcher_on_play
  );
}

/** Empty Java path means "use the bundled runtime", which the backend models as `null`. */
export function normalizeJavaPath(raw: string): string | null {
  return raw === "" ? null : raw;
}
