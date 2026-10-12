import type { InstallStep, Progress } from "./types";

const LABELS: Record<InstallStep, string> = {
  checking_pack: "Comprobando el pack",
  preparing_java: "Preparando Java",
  downloading_minecraft: "Descargando Minecraft",
  installing_forge: "Instalando Forge",
  downloading_mods: "Descargando mods",
  applying_files: "Aplicando archivos",
  finished: "Instalación completa",
};

// Rough share of total install time per step, so the bar does not jump back at each step.
const WEIGHTS: ReadonlyArray<readonly [InstallStep, number]> = [
  ["checking_pack", 0.03],
  ["preparing_java", 0.12],
  ["downloading_minecraft", 0.3],
  ["installing_forge", 0.15],
  ["downloading_mods", 0.32],
  ["applying_files", 0.08],
];

const counterFormat = new Intl.NumberFormat("es-ES");
const percentFormat = new Intl.NumberFormat("es-ES", { style: "percent", maximumFractionDigits: 0 });

export function stepLabel(step: InstallStep): string {
  return LABELS[step];
}

export function isIndeterminate(progress: Progress): boolean {
  return progress.total <= 0 && progress.step !== "finished";
}

export function stepFraction(progress: Progress): number {
  if (progress.step === "finished") return 1;
  if (progress.total <= 0) return 0;
  return Math.min(1, Math.max(0, progress.current / progress.total));
}

/** Overall completion across all steps, between 0 and 1. */
export function overallFraction(progress: Progress): number {
  if (progress.step === "finished") return 1;
  let done = 0;
  for (const [step, weight] of WEIGHTS) {
    if (step === progress.step) return Math.min(1, done + weight * stepFraction(progress));
    done += weight;
  }
  return done;
}

/** "124 de 312", or `null` when the step has no known size. */
export function formatCounter(progress: Progress): string | null {
  if (progress.total <= 0) return null;
  return `${counterFormat.format(Math.min(progress.current, progress.total))} de ${counterFormat.format(progress.total)}`;
}

export function formatPercent(fraction: number): string {
  return percentFormat.format(fraction);
}

export function stepNumber(step: InstallStep): { index: number; count: number } {
  const index = WEIGHTS.findIndex(([s]) => s === step);
  return { index: index === -1 ? WEIGHTS.length : index + 1, count: WEIGHTS.length };
}
