import { useEffect, useMemo, useState, type CSSProperties, type FormEvent } from "react";
import { useLauncher, useLauncherStore } from "../../application/useLauncher";
import { deviceDiagnostics } from "../../application/deviceDiagnostics";
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
  const [totalRam, setTotalRam] = useState("");
  const [processor, setProcessor] = useState("");
  const [graphics, setGraphics] = useState<"integrated" | "dedicated" | "unknown">("unknown");
  const [gpuName, setGpuName] = useState("");
  const [profileMessage, setProfileMessage] = useState("");
  const [hardwareMessage, setHardwareMessage] = useState("Introduce los datos de tu equipo.");

  useEffect(() => {
    let active = true;
    void deviceDiagnostics().then(info => {
      if (!active || !info) return;
      if (info.total_memory_gb) setTotalRam(String(info.total_memory_gb));
      if (info.processor_name) setProcessor(info.processor_name);
      const gpu = info.gpu_names.find(name => /nvidia|geforce|radeon\s+rx/i.test(name));
      if (gpu) { setGraphics("dedicated"); setGpuName(gpu); }
      else if (info.gpu_names.length) setGraphics("integrated");
      setHardwareMessage("Hardware leído localmente. Revisa los datos antes de aplicar un perfil.");
    }).catch(() => { if (active) setHardwareMessage("No se pudo leer el hardware automáticamente. Puedes introducirlo manualmente."); });
    return () => { active = false; };
  }, []);

  const candidate = useMemo(() => fromDraft(draft), [draft]);
  const errors = useMemo(() => validateSettings(candidate), [candidate]);
  const dirty = !settingsEqual(candidate, saved);
  const ramTotal = Number(totalRam);
  const cpu = processor.toLowerCase();
  const intelGeneration = Number(cpu.match(/(?:core\s*i[3579]-?)(\d{4,5})/)?.[1]?.slice(0, -3) ?? 0);
  const ryzenGeneration = Number(cpu.match(/ryzen\s*[3579]\s*(\d{4})/)?.[1]?.[0] ?? 0);
  const recentCpu = intelGeneration >= 10 || ryzenGeneration >= 3 || /core\s*ultra/.test(cpu);
  const cpuTier = recentCpu && /ryzen\s*[79]|core\s*i[79]|core\s*ultra\s*[79]/.test(cpu) ? "high"
    : recentCpu || /ryzen\s*5|core\s*i5/.test(cpu) ? "balanced" : "modest";
  const gpu = gpuName.toLowerCase();
  const gpuTier = /rtx\s*[345]\d{3}|rx\s*[679]\d{3}|arc\s*b[57]/.test(gpu) ? "high"
    : /gtx\s*1[06]\d{2}|rx\s*[45]\d{3}|rtx\s*20\d{2}/.test(gpu) ? "balanced" : "unknown";
  const reserve = ramTotal >= 16 ? 5 : 4;
  const available = Number.isFinite(ramTotal) ? Math.max(0, ramTotal - reserve) : 0;
  const hasHardwareData = Number.isFinite(ramTotal) && ramTotal >= 2 && processor.trim().length >= 3;
  const recommendedTier = !hasHardwareData || available < 6 ? null : available < 8 || cpuTier === "modest"
    ? "modest"
    : cpuTier === "high" && available >= 10 && ramTotal >= 16 && graphics === "dedicated" && gpuTier === "high" ? "high" : "balanced";
  const tiers = [
    { id: "modest", name: "Equipo modesto", target: 6, detail: "Mínimo basado en el pico observado de 5,8 GB; cierra otras aplicaciones." },
    { id: "balanced", name: "Equilibrado", target: 8, detail: "Más margen para zonas cargadas y sesiones largas." },
    { id: "high", name: "Gama alta", target: 10, detail: "Más margen sin ocupar toda la RAM del equipo." },
  ] as const;
  function applyTier(target: number) {
    if (available < target) { setProfileMessage("La RAM disponible no alcanza para este perfil y para reservar memoria al sistema."); return; }
    update("maxGb", formatDecimal(target));
    update("minGb", "3");
    setProfileMessage(`Propuesta ${formatDecimal(target)} GB máximo y 3 GB iniciales. Revisa y guarda los ajustes para aplicarla.`);
  }

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
            <div className={styles.profileBox}>
              <div><h3>Perfil recomendado para tu equipo</h3><p>{hardwareMessage}</p></div>
              <div className={styles.profileInputs}>
                <label>Total de RAM (GB)<input className={field.input} type="number" min="2" max="256" step="0.5" value={totalRam} onChange={e => { setTotalRam(e.target.value); setProfileMessage(""); }} /></label>
                <label>Procesador actual<input className={field.input} type="text" value={processor} placeholder="Ej. Ryzen 5 5600X" onChange={e => { setProcessor(e.target.value); setProfileMessage(""); }} /></label>
                <label>Gráficos<select className={field.input} value={graphics} onChange={e => { setGraphics(e.target.value as typeof graphics); setProfileMessage(""); }}><option value="unknown">No sé / no indicado</option><option value="integrated">Integrados</option><option value="dedicated">Tarjeta dedicada</option></select></label>
              </div>
              {graphics === "dedicated" && <label className={styles.gpuName}>Nombre de la GPU dedicada<input className={field.input} type="text" value={gpuName} placeholder="Ej. GeForce RTX 3060" onChange={e => { setGpuName(e.target.value); setProfileMessage(""); }} /></label>}
              <p className={styles.profileHint}>{recommendedTier ? <>Recomendación: <strong>{tiers.find(t => t.id === recommendedTier)?.name}</strong>. Se reservan {reserve} GB para Windows y otras aplicaciones; {graphics === "integrated" ? "con gráficos integrados, usa ajustes visuales conservadores dentro del juego." : graphics === "dedicated" ? `GPU indicada: ${gpuName.trim() || "sin nombre"}. El perfil no cambia la configuración visual del juego.` : "sin datos de GPU, la recomendación se limita a memoria y procesador."}</> : hasHardwareData ? "No hay RAM suficiente para reservar memoria al sistema y cubrir el pico observado de 5,8 GB. Puedes ajustar la memoria manualmente, con riesgo de cierres por falta de memoria." : "Completa la RAM total y el procesador actual para estimar un perfil. La clasificación por nombre es orientativa."}</p>
              <div className={styles.profileCards}>{tiers.map(tier => {
                const canApply = hasHardwareData && available >= tier.target;
                return <article key={tier.id} className={styles.profileCard} data-recommended={tier.id === recommendedTier || undefined}>
                  <strong>{tier.name}{tier.id === recommendedTier ? " · sugerido" : ""}</strong><span>{!hasHardwareData ? "Completa los datos" : canApply ? `${formatDecimal(tier.target)} GB máximo` : "RAM insuficiente"}</span><small>{tier.detail}</small>
                  <button type="button" className={`${btn.btn} ${btn.outline}`} disabled={!canApply} onClick={() => applyTier(tier.target)}>Usar perfil</button>
                </article>;
              })}</div>
              <p className={styles.profileHint} role="status">{profileMessage || (hasHardwareData ? `Margen estimado tras reservar para el sistema: ${formatDecimal(available)} GB. Perfiles basados en un pico de RAM de 5,8 GB comunicado para Panas RPG. Aplicar cambia solo la memoria; CPU y GPU orientan la elección.` : "Introduce los datos para preparar una recomendación de RAM.")}</p>
            </div>
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
                Recomendado para los tres perfiles: dejarlo vacío. Java 17 ya usa G1; la memoria mínima y máxima se configuran arriba. Añade opciones avanzadas solo si has medido una mejora en tu equipo.
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
