import { useId, useState, type FormEvent } from "react";
import { useLauncherStore } from "../../application/useLauncher";
import { errorCopy } from "../../domain/errors";
import { USERNAME_MAX, checkUsername, usernameProblemMessage } from "../../domain/username";
import btn from "../components/Button.module.css";
import field from "../components/Field.module.css";
import { FieldError } from "../components/FieldError";
import styles from "./UsernameForm.module.css";

interface Props {
  initial?: string;
  submitLabel: string;
  onSaved?: () => void;
  onCancel?: () => void;
  autoFocus?: boolean;
  layout?: "inline" | "stacked";
}

export function UsernameForm({ initial = "", submitLabel, onSaved, onCancel, autoFocus, layout = "stacked" }: Props) {
  const store = useLauncherStore();
  const id = useId();
  const [value, setValue] = useState(initial);
  const [error, setError] = useState<string | undefined>();
  const [saving, setSaving] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const check = checkUsername(value);
    if (!check.ok) {
      setError(usernameProblemMessage(check.problem));
      return;
    }
    setSaving(true);
    const result = await store.saveAccount(check.value);
    setSaving(false);
    if (result.ok) {
      setError(undefined);
      onSaved?.();
    } else {
      const copy = errorCopy(result.error.kind);
      setError(result.error.kind === "invalid_input" ? "Ese nombre no es válido." : `${copy.title}. ${copy.body}`);
    }
  }

  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;

  return (
    <form className={styles.form} data-layout={layout} onSubmit={submit} noValidate>
      <div className={field.field}>
        <label className={field.label} htmlFor={`${id}-input`}>
          Nombre de jugador
        </label>
        <div className={styles.row}>
          <input
            id={`${id}-input`}
            className={field.input}
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              if (error) setError(undefined);
            }}
            maxLength={USERNAME_MAX + 8}
            autoComplete="off"
            autoCapitalize="off"
            spellCheck={false}
            autoFocus={autoFocus}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? `${errorId} ${hintId}` : hintId}
          />
          {layout === "inline" ? (
            <button type="submit" className={`${btn.btn} ${btn.primary}`} disabled={saving}>
              {saving ? "Guardando" : submitLabel}
            </button>
          ) : null}
        </div>
        <FieldError id={errorId} message={error} />
        <p id={hintId} className={field.hint}>
          De 3 a 16 caracteres: letras, números y guion bajo. Es el nombre que verán los demás.
        </p>
      </div>
      {layout === "stacked" ? (
        <div className={styles.actions}>
          {onCancel ? (
            <button type="button" className={`${btn.btn} ${btn.ghost}`} onClick={onCancel}>
              Cancelar
            </button>
          ) : null}
          <button type="submit" className={`${btn.btn} ${btn.primary}`} disabled={saving}>
            {saving ? "Guardando" : submitLabel}
          </button>
        </div>
      ) : null}
    </form>
  );
}
