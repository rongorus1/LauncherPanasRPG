import { useLauncher, useLauncherStore } from "../../application/useLauncher";
import { canPlayInstalled, primaryAction } from "../../domain/primaryAction";
import {
  formatCounter,
  formatPercent,
  isIndeterminate,
  overallFraction,
  stepLabel,
  stepNumber,
} from "../../domain/progress";
import type { Progress } from "../../domain/types";
import btn from "../components/Button.module.css";
import { ArrowsClockwiseIcon, CircleNotchIcon, DownloadSimpleIcon, PlayIcon, StopIcon } from "../icons";
import styles from "./PrimaryAction.module.css";

interface Props {
  onOpenAccount: () => void;
}

export function PrimaryAction({ onOpenAccount }: Props) {
  const store = useLauncherStore();
  const launcher = useLauncher((s) => s.launcher);
  const pending = useLauncher((s) => s.pending);
  const progress = useLauncher((s) => s.progress);

  if (!launcher) return null;
  const action = primaryAction(launcher);
  const account = launcher.account;

  if (action === "installing") {
    return (
      <div className={styles.area}>
        <InstallProgress progress={progress} />
      </div>
    );
  }

  if (action === "stop") {
    const stopping = pending === "stop";
    return (
      <div className={styles.area}>
        <button
          type="button"
          className={`${styles.main} ${styles.stop}`}
          onClick={() => void store.stop()}
          disabled={stopping}
        >
          {stopping ? <CircleNotchIcon className={styles.spin} size={24} weight="bold" aria-hidden /> : <StopIcon size={24} weight="fill" aria-hidden />}
          {stopping ? "Deteniendo" : "Detener"}
        </button>
        <p className={styles.sub}>Minecraft está en marcha{account ? ` como ${account.username}` : ""}.</p>
      </div>
    );
  }

  if (action === "install" || action === "update") {
    const isUpdate = action === "update";
    return (
      <div className={styles.area}>
        <button type="button" className={`${styles.main} ${styles.go}`} onClick={() => void store.install()}>
          {isUpdate ? (
            <ArrowsClockwiseIcon size={24} weight="bold" aria-hidden />
          ) : (
            <DownloadSimpleIcon size={24} weight="bold" aria-hidden />
          )}
          {isUpdate ? "Actualizar" : "Instalar"}
        </button>
        {isUpdate && account && canPlayInstalled(launcher) ? (
          <button
            type="button"
            className={`${btn.btn} ${btn.ghost} ${btn.small} ${styles.secondary}`}
            onClick={() => void store.play()}
            disabled={pending === "play"}
          >
            {pending === "play" ? "Iniciando" : `Jugar con la ${launcher.pack.installed_version}`}
          </button>
        ) : null}
      </div>
    );
  }

  const starting = pending === "play";
  return (
    <div className={styles.area}>
      <button
        type="button"
        className={`${styles.main} ${styles.go}`}
        onClick={() => (account ? void store.play() : onOpenAccount())}
        disabled={starting}
      >
        {starting ? (
          <CircleNotchIcon className={styles.spin} size={24} weight="bold" aria-hidden />
        ) : (
          <PlayIcon size={24} weight="fill" aria-hidden />
        )}
        {starting ? "Iniciando" : "Jugar"}
      </button>
      <p className={styles.sub}>
        {account ? `Entrarás como ${account.username}.` : "Elige un nombre de jugador para entrar."}
      </p>
    </div>
  );
}

function InstallProgress({ progress }: { progress: Progress | null }) {
  const label = progress ? stepLabel(progress.step) : "Preparando la instalación";
  const indeterminate = !progress || isIndeterminate(progress);
  const fraction = progress ? overallFraction(progress) : 0;
  const counter = progress ? formatCounter(progress) : null;
  const step = progress ? stepNumber(progress.step) : null;
  const valueText = [label, counter].filter(Boolean).join(", ");

  return (
    <div className={styles.progress}>
      <div className={styles.progressHead}>
        <span className={styles.stepLabel}>{label}</span>
        <span className={styles.percent} aria-hidden>
          {formatPercent(fraction)}
        </span>
      </div>
      <div
        className={styles.track}
        role="progressbar"
        aria-label="Progreso de la instalación"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(fraction * 100)}
        aria-valuetext={`${valueText}. ${formatPercent(fraction)} en total`}
        data-indeterminate={indeterminate || undefined}
      >
        <span className={styles.fill} style={{ transform: `scaleX(${fraction})` }} />
        {indeterminate ? <span className={styles.sweep} /> : null}
      </div>
      <div className={styles.progressFoot}>
        <span>{step && step.index <= step.count ? `Paso ${step.index} de ${step.count}` : ""}</span>
        <span className={styles.counter}>{counter}</span>
      </div>
      {progress?.message ? (
        <p className={styles.message} title={progress.message}>
          {progress.message}
        </p>
      ) : null}
    </div>
  );
}

export function PrimaryActionSkeleton() {
  return (
    <div className={styles.area} aria-hidden>
      <span className={styles.skeleton} />
    </div>
  );
}
