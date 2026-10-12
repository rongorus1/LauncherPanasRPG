import { useId, type ReactNode } from "react";
import { CaretDownIcon, WarningIcon, XIcon } from "../icons";
import styles from "./ErrorPanel.module.css";

interface Props {
  title: string;
  body: string;
  detail?: string;
  actions?: ReactNode;
  onDismiss?: () => void;
  tone?: "banner" | "page";
}

export function ErrorPanel({ title, body, detail, actions, onDismiss, tone = "banner" }: Props) {
  const titleId = useId();
  return (
    <section className={styles.panel} data-tone={tone} role="alert" aria-labelledby={titleId}>
      <WarningIcon className={styles.icon} size={22} weight="bold" aria-hidden />
      <div className={styles.content}>
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
        <p>{body}</p>
        {detail ? (
          <details className={styles.details}>
            <summary>
              <CaretDownIcon size={14} weight="bold" aria-hidden />
              Detalles
            </summary>
            <pre>{detail}</pre>
          </details>
        ) : null}
        {actions ? <div className={styles.actions}>{actions}</div> : null}
      </div>
      {onDismiss ? (
        <button type="button" className={styles.dismiss} onClick={onDismiss} aria-label="Cerrar aviso">
          <XIcon size={16} weight="bold" aria-hidden />
        </button>
      ) : null}
    </section>
  );
}
