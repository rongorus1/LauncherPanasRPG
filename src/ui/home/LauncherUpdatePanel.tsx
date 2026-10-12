import { useId, useRef, useState } from "react";
import { useLauncher, useLauncherStore } from "../../application/useLauncher";
import { errorCopy } from "../../domain/errors";
import {
  formatUpdateProgress,
  isUpdateDownloaded,
  launcherUpdateBlockedReason,
  updateFraction,
} from "../../domain/launcherUpdate";
import btn from "../components/Button.module.css";
import { ArrowClockwiseIcon, CaretDownIcon, RocketLaunchIcon, WarningIcon, XIcon } from "../icons";
import styles from "./LauncherUpdatePanel.module.css";

/** Self-update of the launcher app. Kept apart from the pack update, which lives in the bottom dock. */
export function LauncherUpdatePanel() {
  const store = useLauncherStore();
  const update = useLauncher((s) => s.launcherUpdate);
  const activity = useLauncher((s) => s.launcher?.activity ?? "idle");
  const [hidden, setHidden] = useState(false);
  const panelRef = useRef<HTMLElement>(null);
  const id = useId();

  if (update.status === "none" || !update.info) return null;
  if (hidden && update.status === "available") return null;

  const { info, progress } = update;
  const blocked = launcherUpdateBlockedReason(activity);

  function start() {
    // The button is replaced by the progress bar; keep focus inside the panel.
    panelRef.current?.focus();
    void store.installLauncherUpdate();
  }

  let body;
  if (update.status === "downloading") {
    const fraction = progress ? updateFraction(progress) : null;
    const restarting = isUpdateDownloaded(progress);
    body = (
      <>
        <p className={styles.line} role="status">
          {restarting ? "Instalando y reiniciando el launcher" : "Descargando la actualización del launcher"}
        </p>
        <div
          className={styles.track}
          role="progressbar"
          aria-labelledby={`${id}-title`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
          aria-valuetext={progress ? formatUpdateProgress(progress) : "Preparando la descarga"}
          data-indeterminate={fraction === null || undefined}
        >
          <span className={styles.fill} style={{ transform: `scaleX(${fraction ?? 0})` }} />
          {fraction === null ? <span className={styles.sweep} /> : null}
        </div>
        <p className={styles.meta}>{progress ? formatUpdateProgress(progress) : "Preparando la descarga"}</p>
      </>
    );
  } else if (update.status === "failed") {
    const copy = update.error ? errorCopy(update.error.kind) : null;
    body = (
      <>
        <p className={`${styles.line} ${styles.error}`} role="alert">
          <WarningIcon size={16} weight="bold" aria-hidden />
          No se pudo actualizar el launcher.
        </p>
        {copy ? <p className={styles.meta}>{copy.body}</p> : null}
        {update.error?.detail ? (
          <details className={styles.details}>
            <summary>
              <CaretDownIcon size={14} weight="bold" aria-hidden />
              Detalles
            </summary>
            <pre>{update.error.detail}</pre>
          </details>
        ) : null}
        <div className={styles.actions}>
          <button
            type="button"
            className={`${btn.btn} ${btn.outline} ${btn.small}`}
            onClick={start}
            disabled={blocked !== null}
            aria-describedby={blocked ? `${id}-blocked` : undefined}
          >
            <ArrowClockwiseIcon size={16} weight="bold" aria-hidden />
            Reintentar
          </button>
        </div>
        {blocked ? (
          <p id={`${id}-blocked`} className={styles.meta}>
            {blocked}
          </p>
        ) : null}
      </>
    );
  } else {
    body = (
      <>
        {/* Notes are dropped while blocked so the panel stays clear of the docked console. */}
        {info.notes && !blocked ? <p className={styles.notes}>{info.notes}</p> : null}
        <div className={styles.actions}>
          <button
            type="button"
            className={`${btn.btn} ${btn.outline} ${btn.small}`}
            onClick={start}
            disabled={blocked !== null}
            aria-describedby={blocked ? `${id}-blocked` : undefined}
          >
            <ArrowClockwiseIcon size={16} weight="bold" aria-hidden />
            Actualizar y reiniciar
          </button>
        </div>
        {blocked ? (
          <p id={`${id}-blocked`} className={styles.meta}>
            {blocked}
          </p>
        ) : null}
      </>
    );
  }

  return (
    <section
      ref={panelRef}
      className={styles.panel}
      data-status={update.status}
      aria-labelledby={`${id}-title`}
      tabIndex={-1}
    >
      <header className={styles.head}>
        <RocketLaunchIcon className={styles.icon} size={18} weight="bold" aria-hidden />
        <h2 id={`${id}-title`} className={styles.title}>
          Hay una versión nueva del launcher: {info.version}
        </h2>
        {update.status === "available" ? (
          <button
            type="button"
            className={styles.dismiss}
            onClick={() => setHidden(true)}
            aria-label="Ocultar aviso hasta el próximo arranque"
          >
            <XIcon size={14} weight="bold" aria-hidden />
          </button>
        ) : null}
      </header>
      <div className={styles.body}>
        {body}
      </div>
    </section>
  );
}
