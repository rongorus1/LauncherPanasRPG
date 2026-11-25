import { useEffect, useState } from "react";
import { useLauncher } from "../../application/useLauncher";
import { deviceDiagnostics, type DeviceDiagnostics } from "../../application/deviceDiagnostics";
import { errorCopy } from "../../domain/errors";
import { version } from "../../../package.json";
import btn from "../components/Button.module.css";
import s from "./HelpScreen.module.css";

interface Props { onOpenSettings: () => void; onOpenConsole: () => void; preview?: boolean }
interface Scan { installed_version: string | null; checked_files: number; missing_files: string[]; corrupt_archives: string[]; unchecked_files: string[]; status: string }
interface Backup { id: string; version: string; file_count: number }
const native = () => "__TAURI_INTERNALS__" in window;
async function invoke<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  const api = await import("@tauri-apps/api/core");
  return api.invoke<T>(name, args);
}

export function HelpScreen({ onOpenSettings, onOpenConsole, preview = false }: Props) {
  const snapshot = useLauncher(x => x);
  const recoveryAvailable = native() && /Windows/i.test(navigator.userAgent);
  const [hardware, setHardware] = useState<DeviceDiagnostics | null>(null);
  const [scan, setScan] = useState<Scan | null>(null);
  const [backups, setBackups] = useState<Backup[]>([]);
  const [selected, setSelected] = useState("");
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [copied, setCopied] = useState("");
  const [copyError, setCopyError] = useState(false);
  const failure = snapshot.loadError ?? (snapshot.notice?.type === "error" ? snapshot.notice.error : null);
  const info = failure ? errorCopy(failure.kind) : null;
  const state = snapshot.launcher;
  const incident = snapshot.loadError?.kind
    ?? (snapshot.notice?.type === "error" ? snapshot.notice.error.kind
      : snapshot.notice?.type === "game_crashed" ? "game_crashed"
      : snapshot.notice?.type === "install_failed" ? "install_failed" : "none");

  useEffect(() => {
    if (!native()) return;
    let active = true;
    void Promise.allSettled([deviceDiagnostics(), invoke<Backup[]>("list_backups")]).then(([device, copies]) => {
      if (!active) return;
      if (device.status === "fulfilled") setHardware(device.value);
      if (copies.status === "fulfilled") setBackups(copies.value);
    });
    return () => { active = false; };
  }, []);

  async function check() {
    if (!native()) return;
    setBusy(true); setMessage("Comprobando el equipo y los archivos administrados…");
    try {
      const [device, installation] = await Promise.all([deviceDiagnostics(), invoke<Scan>("scan_installation")]);
      setHardware(device); setScan(installation);
      setMessage("Comprobación terminada. Se revisó la estructura de los JAR, sin comparar hashes remotos.");
    } catch {
      setMessage("No se pudo completar la comprobación. Revisa la instalación y vuelve a intentar.");
    } finally { setBusy(false); }
  }

  async function createBackup() {
    if (!recoveryAvailable) return;
    if (state?.activity !== "idle") return;
    setBusy(true); setMessage("Creando copia local de los mods administrados…");
    try {
      const backup = await invoke<Backup>("create_backup");
      setBackups(await invoke<Backup[]>("list_backups"));
      setSelected(backup.id); setConfirm(false);
      setMessage(`Copia creada: ${backup.file_count} archivos administrados de ${backup.version}. Los mundos y ajustes quedan fuera.`);
    } catch { setMessage("No se pudo crear la copia."); }
    finally { setBusy(false); }
  }

  async function restore() {
    if (!recoveryAvailable || !selected || !confirm || state?.activity !== "idle") return;
    setBusy(true); setMessage("Verificando y restaurando la copia local…");
    try {
      const result = await invoke<{ restored_files: number; version: string }>("restore_backup", { id: selected });
      setConfirm(false);
      setScan(await invoke<Scan>("scan_installation"));
      setMessage(`Repuestos ${result.restored_files} mods faltantes o dañados de ${result.version}. Comprueba el estado antes de jugar.`);
    } catch {
      setMessage("No se completó la restauración. Cierra Minecraft y comprueba que la copia corresponda a esta versión.");
    } finally { setBusy(false); }
  }

  const summary = [
    "Panas RPG — resumen de soporte",
    native() ? "Origen: launcher y diagnóstico local" : "Origen: navegador; diagnóstico local no disponible",
    `Launcher: ${version}`,
    `Modpack instalado: ${scan ? scan.installed_version ?? "No detectado" : state?.pack.installed_version ?? "No detectado"}`,
    `Minecraft: ${state?.pack.minecraft_version ?? "No disponible"}`,
    `Forge: ${state?.pack.forge_version ?? "No disponible"}`,
    `RAM asignada: ${state ? `${state.settings.min_memory_mb}–${state.settings.max_memory_mb} MB` : "No disponible"}`,
    `Java: ${state ? state.settings.java_path ? "Personalizado" : "Administrado por el launcher" : "No disponible"}`,
    `Estado: ${state?.activity ?? "No disponible"}`,
    `RAM física: ${hardware?.total_memory_gb != null ? `${hardware.total_memory_gb} GB` : "Sin comprobar"}`,
    `Procesador: ${hardware?.processor_name ?? "Sin comprobar"}`,
    `Procesadores lógicos: ${hardware?.logical_cores ?? "Sin comprobar"}`,
    `GPU: ${hardware?.gpu_names.length ? hardware.gpu_names.join("; ") : "Sin comprobar"}`,
    `Mods JAR/ZIP revisados estructuralmente: ${scan?.checked_files ?? "Sin comprobar"}`,
    `Archivos faltantes: ${scan?.missing_files.length ?? "Sin comprobar"}`,
    `Archivos JAR dañados: ${scan?.corrupt_archives.length ?? "Sin comprobar"}`,
    `Archivos sin revisión estructural: ${scan?.unchecked_files.length ?? "Sin comprobar"}`,
    "Bibliotecas y configuraciones: no verificadas",
    "Hashes de la versión remota: no verificados",
    `Incidencia de sesión: ${incident}`,
  ].join("\n");

  async function copy() {
    try { await navigator.clipboard.writeText(summary); setCopied(summary); setCopyError(false); }
    catch { setCopyError(true); setCopied(""); }
  }

  return <div className={s.screen}><div className={s.content}>
    <header><h1>Ayuda</h1><p>Revisa tu instalación y encuentra el siguiente paso.</p></header>
    {preview && <p className={s.preview}>Esta vista en navegador no puede leer el hardware ni comprobar los archivos locales.</p>}
    <section className={s.section} aria-labelledby="help-state"><h2 id="help-state">Información del equipo y la instalación</h2>
      <dl className={s.facts}>
        <div><dt>Versión instalada</dt><dd>{scan ? scan.installed_version ?? "No detectada" : state?.pack.installed_version ?? "No detectada"}</dd></div>
        <div><dt>RAM asignada</dt><dd>{state ? `${state.settings.max_memory_mb / 1024} GB máximo` : "No disponible"}</dd></div>
        <div><dt>Memoria física</dt><dd>{hardware?.total_memory_gb != null ? `${hardware.total_memory_gb} GB` : "Sin comprobar"}</dd></div>
        <div><dt>Procesador</dt><dd>{hardware?.processor_name ?? "Sin comprobar"}</dd></div>
        <div><dt>Gráficos</dt><dd>{hardware?.gpu_names.join(", ") || "Sin comprobar"}</dd></div>
        <div><dt>JAR revisados</dt><dd>{scan?.checked_files ?? "Sin comprobar"}</dd></div>
        <div><dt>Faltantes / dañados</dt><dd>{scan ? `${scan.missing_files.length} / ${scan.corrupt_archives.length}` : "Sin comprobar"}</dd></div>
      </dl>
      <p className={s.hint}>Se comprueba la estructura y CRC de los mods JAR/ZIP listados en la instalación. No se verifican bibliotecas, configuraciones ni hashes remotos.</p>
      {info && <div className={s.incident} role="status"><strong>{info.title}</strong><p>{info.body}</p></div>}
      <div className={s.actions}><button className={`${btn.btn} ${btn.primary}`} disabled={!native() || busy} onClick={() => void check()}>Comprobar equipo y archivos</button><button className={`${btn.btn} ${btn.outline}`} disabled={!snapshot.launcher} onClick={onOpenSettings}>Abrir ajustes</button><button className={`${btn.btn} ${btn.outline}`} onClick={onOpenConsole}>Ver consola</button></div>
      {!native() && <p className={s.hint}>Abre el launcher instalado para comprobar tu equipo y los archivos. El navegador no tiene acceso a ellos.</p>}
    </section>
    <section className={s.section} aria-labelledby="help-solutions"><h2 id="help-solutions">Problemas frecuentes</h2>
      <details><summary>El juego no inicia o se cierra</summary><p>Revisa la consola y la ruta de Java en Ajustes. Si usas Java personalizado, comprueba que sea Java 17.</p></details>
      <details><summary>El juego va lento o falta memoria</summary><p>Revisa el perfil recomendado en Ajustes y conserva memoria para el sistema. Asignar más RAM no siempre mejora el rendimiento.</p></details>
      <details><summary>La descarga o instalación falla</summary><p>Comprueba la conexión y el espacio libre. Reintentar desde Inicio puede buscar una versión más reciente.</p></details>
      <details><summary>Quiero reparar o volver a una versión anterior</summary><p>La recuperación local repone únicamente mods faltantes o dañados desde una copia de la misma versión. No permite volver a otra versión. Mundos, bibliotecas y ajustes quedan fuera.</p>
        <section className={s.recovery} aria-label="Recuperación local">
          <strong>Recuperación desde copia local</strong>
          <p>Cierra Minecraft antes de crear o restaurar una copia. Solo se copian los mods administrados.</p>
          {state?.activity !== "idle" && <p className={s.hint}>Espera a que termine la instalación o cierra el juego para crear o restaurar una copia.</p>}
          <div className={s.actions}><button className={`${btn.btn} ${btn.outline}`} type="button" disabled={!native() || busy} onClick={() => void check()}>Comprobar archivos</button><button className={`${btn.btn} ${btn.outline}`} type="button" disabled={!recoveryAvailable || busy || state?.activity !== "idle" || !scan?.installed_version || scan.status === "issues"} onClick={() => void createBackup()}>Crear copia de mods</button></div>
          {scan && <p className={s.hint}>Revisados: {scan.checked_files}. Faltantes: {scan.missing_files.length}. JAR dañados: {scan.corrupt_archives.length}. Sin revisión estructural: {scan.unchecked_files.length}.</p>}
          <label className={s.recoveryChoice}>Copia disponible <select value={selected} disabled={busy || !backups.length} onChange={e => { setSelected(e.target.value); setConfirm(false); }}><option value="">Selecciona una copia</option>{backups.map(b => <option key={b.id} value={b.id}>{b.version} · {b.file_count} mods · {b.id}</option>)}</select></label>
          <button className={`${btn.btn} ${btn.outline}`} type="button" disabled={!recoveryAvailable || !selected || busy || state?.activity !== "idle"} onClick={() => setConfirm(true)}>Revisar restauración</button>
          {confirm && <div className={s.incident}><strong>Confirma la reposición de los mods</strong><p>Se verificará la copia y solo se repondrán mods faltantes o dañados si corresponde a la versión instalada. Cierra Minecraft antes de continuar. Los mundos, bibliotecas y ajustes quedan fuera.</p><div className={s.actions}><button className={`${btn.btn} ${btn.primary}`} type="button" disabled={!recoveryAvailable || busy || state?.activity !== "idle"} onClick={() => void restore()}>Reponer desde esta copia</button><button className={`${btn.btn} ${btn.outline}`} type="button" disabled={busy} onClick={() => setConfirm(false)}>Cancelar</button></div></div>}
          {!native() && <p className={s.hint}>Esta herramienta requiere el launcher instalado.</p>}
          {native() && !recoveryAvailable && <p className={s.hint}>La comprobación de archivos está disponible; crear copias y restaurar mods requiere Windows en esta versión.</p>}
        </section>
      </details>
    </section>
    <section className={s.section} aria-labelledby="help-report"><h2 id="help-report">Pedir ayuda</h2><p>Comprueba el equipo y los archivos, copia este resumen y explica qué ocurrió. No se envía automáticamente.</p><label className={s.hint} htmlFor="support-summary">Revisa el resumen antes de compartirlo. No incluye nombres de jugador, rutas ni registros.</label><textarea id="support-summary" className={s.report} readOnly value={summary} rows={16} spellCheck={false}/><div className={s.actions}><button className={`${btn.btn} ${btn.primary}`} onClick={() => void copy()}>Copiar resumen</button><span role="status">{copyError ? "No se pudo copiar. Selecciona el texto y cópialo manualmente." : copied === summary ? "Resumen copiado" : ""}</span></div><p className={s.hint}>Si compartes registros de la consola, revisa primero su contenido.</p></section>
    {message && <p className={s.recoveryStatus} role="status">{message}</p>}
  </div></div>;
}
