import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { useLauncher, useLauncherStore } from "../../application/useLauncher";
import btn from "../components/Button.module.css";
import { ArrowDownIcon, CaretDownIcon, CheckIcon, CopyIcon, TrashIcon } from "../icons";
import styles from "./ConsolePanel.module.css";

const STICK_THRESHOLD_PX = 24;
const countFormat = new Intl.NumberFormat("es-ES");

interface Props {
  variant: "docked" | "full";
}

export function ConsolePanel({ variant }: Props) {
  const store = useLauncherStore();
  const logs = useLauncher((s) => s.logs);
  const playing = useLauncher((s) => s.launcher?.activity === "playing");
  const [open, setOpen] = useState(true);
  const [following, setFollowing] = useState(true);
  const [copied, setCopied] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const followingRef = useRef(true);
  const bodyId = useId();

  useLayoutEffect(() => {
    const list = listRef.current;
    if (list && followingRef.current) list.scrollTop = list.scrollHeight;
  }, [logs, open]);

  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 1600);
    return () => clearTimeout(t);
  }, [copied]);

  function onScroll() {
    const list = listRef.current;
    if (!list) return;
    const atBottom = list.scrollHeight - list.scrollTop - list.clientHeight < STICK_THRESHOLD_PX;
    if (atBottom !== followingRef.current) {
      followingRef.current = atBottom;
      setFollowing(atBottom);
    }
  }

  function jumpToEnd() {
    const list = listRef.current;
    if (!list) return;
    followingRef.current = true;
    setFollowing(true);
    list.scrollTop = list.scrollHeight;
  }

  async function copy() {
    const text = logs.map((l) => l.text).join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  const expanded = variant === "full" || open;
  const title = (
    <>
      <span className={styles.title}>Consola</span>
      <span className={styles.count}>
        {logs.length === 1 ? "1 línea" : `${countFormat.format(logs.length)} líneas`}
      </span>
    </>
  );

  return (
    <section className={styles.panel} data-variant={variant} aria-label="Consola del juego">
      <header className={styles.bar}>
        {variant === "docked" ? (
          <button
            type="button"
            className={styles.toggle}
            aria-expanded={open}
            aria-controls={bodyId}
            onClick={() => setOpen((v) => !v)}
          >
            <CaretDownIcon className={styles.caret} size={14} weight="bold" aria-hidden />
            {title}
          </button>
        ) : (
          <h2 className={styles.heading}>{title}</h2>
        )}
        <div className={styles.tools}>
          {!following && expanded ? (
            <span className={styles.paused} role="status">
              Desplazamiento en pausa
            </span>
          ) : null}
          <button
            type="button"
            className={`${btn.btn} ${btn.ghost} ${btn.small}`}
            onClick={() => void copy()}
            disabled={logs.length === 0}
          >
            {copied ? <CheckIcon size={16} weight="bold" aria-hidden /> : <CopyIcon size={16} weight="bold" aria-hidden />}
            {copied ? "Copiado" : "Copiar"}
          </button>
          <button
            type="button"
            className={`${btn.btn} ${btn.ghost} ${btn.small}`}
            onClick={() => store.clearLogs()}
            disabled={logs.length === 0}
          >
            <TrashIcon size={16} weight="bold" aria-hidden />
            Limpiar
          </button>
        </div>
      </header>
      <div id={bodyId} className={styles.body} hidden={!expanded}>
        <div
          ref={listRef}
          className={styles.list}
          role="log"
          aria-label="Salida del juego"
          aria-live="off"
          tabIndex={0}
          onScroll={onScroll}
        >
          {logs.length === 0 ? (
            <p className={styles.empty}>
              {playing
                ? "Esperando la salida del juego."
                : "Aún no hay registros. Aparecerán aquí cuando inicies el juego."}
            </p>
          ) : (
            logs.map((line) => (
              <div key={line.id} className={styles.line} data-stream={line.stream}>
                {line.text}
              </div>
            ))
          )}
        </div>
        {!following ? (
          <button type="button" className={`${btn.btn} ${btn.outline} ${btn.small} ${styles.jump}`} onClick={jumpToEnd}>
            <ArrowDownIcon size={16} weight="bold" aria-hidden />
            Ir al final
          </button>
        ) : null}
      </div>
    </section>
  );
}
