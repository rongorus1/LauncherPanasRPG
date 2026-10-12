import { useLauncher, useLauncherStore } from "../../application/useLauncher";
import { errorCopy } from "../../domain/errors";
import btn from "../components/Button.module.css";
import { ErrorPanel } from "../components/ErrorPanel";
import { ArrowClockwiseIcon, TerminalWindowIcon } from "../icons";

const RETRY_LABEL = {
  install: "Reintentar la instalación",
  play: "Volver a intentarlo",
  update_check: "Buscar de nuevo",
} as const;

export function HomeNotice({ onOpenConsole }: { onOpenConsole: () => void }) {
  const store = useLauncherStore();
  const notice = useLauncher((s) => s.notice);
  if (!notice) return null;

  const dismiss = () => store.dismissNotice();

  if (notice.type === "game_crashed") {
    return (
      <ErrorPanel
        title="El juego se cerró de forma inesperada"
        body={`Minecraft terminó con el código ${notice.code}. En la consola tienes el registro completo.`}
        onDismiss={dismiss}
        actions={
          <button type="button" className={`${btn.btn} ${btn.outline} ${btn.small}`} onClick={onOpenConsole}>
            <TerminalWindowIcon size={16} weight="bold" aria-hidden />
            Ver consola
          </button>
        }
      />
    );
  }

  if (notice.type === "install_failed") {
    return (
      <ErrorPanel
        title="La instalación no terminó"
        body="Suele deberse a la conexión o al espacio en disco. Puedes volver a intentarlo."
        detail={notice.detail}
        onDismiss={dismiss}
        actions={<RetryButton label={RETRY_LABEL.install} onClick={() => void store.retry("install")} />}
      />
    );
  }

  const copy = errorCopy(notice.error.kind);
  const retry = notice.retry;
  return (
    <ErrorPanel
      title={copy.title}
      body={copy.body}
      detail={notice.error.detail}
      onDismiss={dismiss}
      actions={
        retry ? <RetryButton label={RETRY_LABEL[retry]} onClick={() => void store.retry(retry)} /> : null
      }
    />
  );
}

function RetryButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className={`${btn.btn} ${btn.outline} ${btn.small}`} onClick={onClick}>
      <ArrowClockwiseIcon size={16} weight="bold" aria-hidden />
      {label}
    </button>
  );
}
