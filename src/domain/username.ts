export const USERNAME_MIN = 3;
export const USERNAME_MAX = 16;

const ALLOWED = /^[A-Za-z0-9_]+$/;

export type UsernameProblem = "empty" | "too_short" | "too_long" | "invalid_chars";

export type UsernameCheck =
  | { ok: true; value: string }
  | { ok: false; problem: UsernameProblem };

/** Same rule as `Username::parse` in launcher-core: trimmed, 3 to 16 chars of [A-Za-z0-9_]. */
export function checkUsername(raw: string): UsernameCheck {
  const value = raw.trim();
  if (value.length === 0) return { ok: false, problem: "empty" };
  if (!ALLOWED.test(value)) return { ok: false, problem: "invalid_chars" };
  if (value.length < USERNAME_MIN) return { ok: false, problem: "too_short" };
  if (value.length > USERNAME_MAX) return { ok: false, problem: "too_long" };
  return { ok: true, value };
}

const MESSAGES: Record<UsernameProblem, string> = {
  empty: "Escribe un nombre de jugador.",
  too_short: `Usa al menos ${USERNAME_MIN} caracteres.`,
  too_long: `Usa como máximo ${USERNAME_MAX} caracteres.`,
  invalid_chars: "Solo letras sin tilde, números y guion bajo (_).",
};

export function usernameProblemMessage(problem: UsernameProblem): string {
  return MESSAGES[problem];
}
