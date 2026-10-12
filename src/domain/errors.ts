import type { AppError, AppErrorKind } from "./types";

const KINDS: ReadonlySet<string> = new Set<AppErrorKind>([
  "invalid_input",
  "no_account",
  "not_installed",
  "busy",
  "network",
  "integrity",
  "io",
  "internal",
]);

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.kind === "string" && KINDS.has(v.kind) && typeof v.detail === "string";
}

/** Anything thrown across the gateway boundary becomes an `AppError`. */
export function toAppError(value: unknown): AppError {
  if (isAppError(value)) return value;
  if (value instanceof Error) return { kind: "internal", detail: value.message };
  if (typeof value === "string") return { kind: "internal", detail: value };
  return { kind: "internal", detail: JSON.stringify(value) ?? String(value) };
}

interface ErrorCopy {
  title: string;
  body: string;
}

const COPY: Record<AppErrorKind, ErrorCopy> = {
  invalid_input: {
    title: "Hay datos que no son válidos",
    body: "Revisa los valores e inténtalo de nuevo.",
  },
  no_account: {
    title: "Falta tu nombre de jugador",
    body: "Elige un nombre antes de jugar.",
  },
  not_installed: {
    title: "El pack no está instalado",
    body: "Instálalo para poder jugar.",
  },
  busy: {
    title: "El launcher está ocupado",
    body: "Espera a que termine la tarea en curso.",
  },
  network: {
    title: "Sin conexión con el servidor de descargas",
    body: "Comprueba tu conexión a internet y vuelve a intentarlo.",
  },
  integrity: {
    title: "Un archivo descargado está dañado",
    body: "Vuelve a intentarlo para descargarlo de nuevo.",
  },
  io: {
    title: "No se pudo leer o escribir en el disco",
    body: "Comprueba que queda espacio libre y que la carpeta del juego tiene permisos.",
  },
  internal: {
    title: "Algo ha fallado en el launcher",
    body: "Vuelve a intentarlo. Si se repite, copia los detalles y avisa al equipo.",
  },
};

export function errorCopy(kind: AppErrorKind): ErrorCopy {
  return COPY[kind];
}
