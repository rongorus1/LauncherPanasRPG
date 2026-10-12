import { useMemo, useState, type CSSProperties, type FormEvent } from "react";
import { useLauncher, useLauncherStore } from "../../application/useLauncher";
import { errorCopy } from "../../domain/errors";
import {
  MEMORY_MIN_MB,
  hasErrors,
  mbToGb,
  settingsEqual,
  snapGb,
  validateSettings,
  type SettingsErrors,
  type SettingsField,
} from "../../domain/settings";
import type { Settings } from "../../domain/types";
import btn from "../components/Button.module.css";
import field from "../components/Field.module.css";
import { FieldError } from "../components/FieldError";
import { Switch } from "../components/Switch";
import { CheckIcon, WarningIcon } from "../icons";
import { formatDecimal, fromDraft, parseDecimal, toDraft, type SettingsDraft } from "./draft";
import styles from "./SettingsScreen.module.css";

const SLIDER_MAX_GB = 32;

export function SettingsScreen() {
  const settings = useLauncher((s) => s.launcher?.settings ?? null);
  if (!settings) return null;
  // Remounting on every saved change resets the draft without syncing state in an effect.
  return <SettingsForm key={JSON.stringify(settings)} saved={settings} />;
}

type Status = { type: "idle" } | { type: "saved" } | { type: "error"; message: string };

function SettingsForm({ saved }: { saved: Settings }) {
  const store = useLauncherStore();
  const [draft, setDraft] = useState<SettingsDraft>(() => toDraft(saved));
  const [touched, setTouched] = useState<Partial<Record<SettingsField, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<Status>({ type: "idle" });

  const candidate = useMemo(() => fromDraft(draft), [draft]);
  const errors = useMemo(() => validateSettings(candidate), [candidate]);
  const dirty = !settingsEqual(candidate, saved);

  const visible: SettingsErrors = {};
  for (const key of Object.keys(errors) as SettingsField[]) {
    if (submitted || touched[key]) visible[key] = errors[key];
  }

  function update<K extends keyof SettingsDraft>(key: K, value: SettingsDraft[K]) {
    setDraft((d) => ({ ...d, [key]: value }));
    if (status.type !== "idle") setStatus({ type: "idle" });
  }

  const touch = (key: SettingsField) => () => setTouched((t) => (t[key] ? t : { ...t, [key]: true }));

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitted(true);
    if (hasErrors(errors)) {
      const first = event.currentTarget.querySelector<HTMLElement>('[aria-invalid="true"]');
      first?.focus();
      return;
    }
    setSaving(true);
    const result = await store.saveSettings(candidate);
    setSaving(false);
    if (!result.ok) {
      const copy = errorCopy(result.error.kind);
      setStatus({ type: "error", message: `${copy.title}. ${copy.body}` });
    }
  }

  const maxGbValue = parseDecimal(draft.maxGb);
  const sliderValue = Number.isFinite(maxGbValue)
    ? Math.min(SLIDER_MAX_GB, Math.max(mbToGb(MEMORY_MIN_MB), maxGbValue))
    : mbToGb(MEMORY_MIN_MB);

  const describe = (id: string, key: SettingsField) => (visible[key] ? `${id}-error ${id}-hint` : `${id}-hint`);

  return (
    <form className={styles.screen} onSubmit={submit} noValidate aria-labelledby="settings-title">
      <div className={styles.scroll}>
        <div className={styles.content}>
          <h1 id="settings-title" className={styles.title}>
            Ajustes
          </h1>

          <section className={styles.section} aria-labelledby="mem-title">
            <h2 id="mem-title">Memoria</h2>
            <div className={field.field}>
              <label className={field.label} htmlFor="max-mem">
                Memoria máxima
              </label>
              <div className={styles.memRow}>
                <input
                  type="range"
                  className={styles.slider}
                  min={mbToGb(MEMORY_MIN_MB)}
                  max={SLIDER_MAX_GB}
                  step={0.5}
                  value={sliderValue}
                  aria-label="Memoria máxima, control deslizante"
                  aria-valuetext={`${formatDecimal(sliderValue)} GB`}
                  style={{ "--fill": `${((sliderValue - 1) / (SLIDER_MAX_GB - 1)) * 100}%` } as CSSProperties}
                  onChange={(e) => update("maxGb", formatDecimal(snapGb(Number(e.target.value))))}
                />
                <div className={styles.unit}>
                  <input
                    id="max-mem"
                    className={`${field.input} ${styles.num}`}
                    inputMode="decimal"
                    value={draft.maxGb}
                    onChange={(e) => update("maxGb", e.target.value)}
                    onBlur={touch("max_memory_mb")}
                    aria-invalid={visible.max_memory_mb ? true : undefined}
                    aria-describedby={describe("max-mem", "max_memory_mb")}
                  />
                  <span aria-hidden>GB</span>
                </div>
              </div>
              <FieldError id="max-mem-error" message={visible.max_memory_mb} />
              <p id="max-mem-hint" className={field.hint}>
                Lo que puede usar el juego como mucho, en pasos de 0,5 GB. Deja memoria libre para el sistema.
              </p>
            </div>

            <div className={field.field}>
              <label className={field.label} htmlFor="min-mem">
                Memoria mínima
              </label>
              <div className={styles.unit}>
                <input
                  id="min-mem"
                  className={`${field.input} ${styles.num}`}
                  inputMode="decimal"
                  value={draft.minGb}
                  onChange={(e) => update("minGb", e.target.value)}
                  onBlur={touch("min_memory_mb")}
                  aria-invalid={visible.min_memory_mb ? true : undefined}
                  aria-describedby={describe("min-mem", "min_memory_mb")}
                />
                <span aria-hidden>GB</span>
              </div>
              <FieldError id="min-mem-error" message={visible.min_memory_mb} />
              <p id="min-mem-hint" className={field.hint}>
                Lo que se reserva al arrancar. No puede superar la máxima.
              </p>
            </div>
          </section>

          <section className={styles.section} aria-labelledby="java-title">
            <h2 id="java-title">Java</h2>
            <div className={field.field}>
              <label className={field.label} htmlFor="java-path">
                Ruta de Java (opcional)
              </label>
              <input
                id="java-path"
                className={`${field.input} ${field.mono}`}
                value={draft.javaPath}
                placeholder="Vacío: se usa el Java que descarga el launcher"
                spellCheck={false}
                autoComplete="off"
                onChange={(e) => update("javaPath", e.target.value)}
                onBlur={touch("java_path")}
                aria-invalid={visible.java_path ? true : undefined}
                aria-describedby={describe("java-path", "java_path")}
              />
              <FieldError id="java-path-error" message={visible.java_path} />
              <p id="java-path-hint" className={field.hint}>
                Ruta completa al ejecutable, por ejemplo /usr/lib/jvm/java-17/bin/java.
              </p>
            </div>
            <div className={field.field}>
              <label className={field.label} htmlFor="jvm-args">
                Argumentos JVM extra
              </label>
              <input
                id="jvm-args"
                className={`${field.input} ${field.mono}`}
                value={draft.jvmArgs}
                spellCheck={false}
                autoComplete="off"
                onChange={(e) => update("jvmArgs", e.target.value)}
                aria-describedby="jvm-args-hint"
              />
              <p id="jvm-args-hint" className={field.hint}>
                Separados por espacios. Si no sabes qué poner, déjalo vacío.
              </p>
            </div>
          </section>

          <section className={styles.section} aria-labelledby="window-title">
            <h2 id="window-title">Ventana del juego</h2>
            <div className={styles.pair}>
              <div className={field.field}>
                <label className={field.label} htmlFor="win-w">
                  Ancho
                </label>
                <div className={styles.unit}>
                  <input
                    id="win-w"
                    className={`${field.input} ${styles.num}`}
                    inputMode="numeric"
                    value={draft.width}
                    disabled={draft.fullscreen}
                    onChange={(e) => update("width", e.target.value)}
                    onBlur={touch("window_width")}
                    aria-invalid={visible.window_width ? true : undefined}
                    aria-describedby={describe("win-w", "window_width")}
                  />
                  <span aria-hidden>px</span>
                </div>
                <FieldError id="win-w-error" message={visible.window_width} />
                <p id="win-w-hint" className="visually-hidden">
                  Mínimo 640 píxeles.
                </p>
              </div>
              <div className={field.field}>
                <label className={field.label} htmlFor="win-h">
                  Alto
                </label>
                <div className={styles.unit}>
                  <input
                    id="win-h"
                    className={`${field.input} ${styles.num}`}
                    inputMode="numeric"
                    value={draft.height}
                    disabled={draft.fullscreen}
                    onChange={(e) => update("height", e.target.value)}
                    onBlur={touch("window_height")}
                    aria-invalid={visible.window_height ? true : undefined}
                    aria-describedby={describe("win-h", "window_height")}
                  />
                  <span aria-hidden>px</span>
                </div>
                <FieldError id="win-h-error" message={visible.window_height} />
                <p id="win-h-hint" className="visually-hidden">
                  Mínimo 480 píxeles.
                </p>
              </div>
            </div>
            <Switch
              label="Pantalla completa"
              hint="El juego ocupa toda la pantalla y se ignora el tamaño de ventana."
              checked={draft.fullscreen}
              onChange={(v) => update("fullscreen", v)}
            />
          </section>

          <section className={styles.section} aria-labelledby="launcher-title">
            <h2 id="launcher-title">Launcher</h2>
            <Switch
              label="Cerrar el launcher al jugar"
              hint="Libera memoria, pero no verás la consola mientras juegas."
              checked={draft.closeOnPlay}
              onChange={(v) => update("closeOnPlay", v)}
            />
          </section>
        </div>
      </div>

      <footer className={styles.footer}>
        <p className={styles.status} role="status">
          {status.type === "error" ? (
            <span className={styles.statusError}>
              <WarningIcon size={16} weight="bold" aria-hidden />
              {status.message}
            </span>
          ) : submitted && hasErrors(errors) ? (
            <span className={styles.statusError}>
              <WarningIcon size={16} weight="bold" aria-hidden />
              Corrige los campos marcados para guardar.
            </span>
          ) : dirty ? (
            "Tienes cambios sin guardar."
          ) : (
            <span className={styles.statusOk}>
              <CheckIcon size={16} weight="bold" aria-hidden />
              Todo guardado
            </span>
          )}
        </p>
        <div className={styles.actions}>
          <button
            type="button"
            className={`${btn.btn} ${btn.ghost}`}
            disabled={!dirty || saving}
            onClick={() => {
              setDraft(toDraft(saved));
              setTouched({});
              setSubmitted(false);
              setStatus({ type: "idle" });
            }}
          >
            Descartar
          </button>
          <button type="submit" className={`${btn.btn} ${btn.primary}`} disabled={!dirty || saving}>
            {saving ? "Guardando" : "Guardar ajustes"}
          </button>
        </div>
      </footer>
    </form>
  );
}
