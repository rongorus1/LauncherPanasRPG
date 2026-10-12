import type { Activity, UpdateProgress } from "./types";

const MB = 1024 * 1024;
const mbFormat = new Intl.NumberFormat("es-ES", { minimumFractionDigits: 1, maximumFractionDigits: 1 });

function mb(bytes: number): string {
  return mbFormat.format(Math.max(0, bytes) / MB);
}

export function formatMb(bytes: number): string {
  return `${mb(bytes)} MB`;
}

/** Between 0 and 1, or `null` when the download size is unknown. */
export function updateFraction(progress: UpdateProgress): number | null {
  if (progress.total === null || progress.total <= 0) return null;
  return Math.min(1, Math.max(0, progress.downloaded / progress.total));
}

/** "3,2 de 8,4 MB", or "3,2 MB descargados" without a known size. */
export function formatUpdateProgress(progress: UpdateProgress): string {
  if (progress.total === null || progress.total <= 0) return `${formatMb(progress.downloaded)} descargados`;
  return `${mb(Math.min(progress.downloaded, progress.total))} de ${formatMb(progress.total)}`;
}

export function isUpdateDownloaded(progress: UpdateProgress | null): boolean {
  return progress !== null && progress.total !== null && progress.total > 0 && progress.downloaded >= progress.total;
}

/** The backend refuses to replace itself while the pack installs or the game runs. */
export function launcherUpdateBlockedReason(activity: Activity): string | null {
  if (activity === "installing") return "Podrás actualizar cuando termine la instalación del pack.";
  if (activity === "playing") return "Podrás actualizar cuando cierres el juego.";
  return null;
}
