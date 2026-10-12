import { useId } from "react";
import styles from "./Switch.module.css";

interface Props {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

export function Switch({ label, hint, checked, onChange }: Props) {
  const id = useId();
  return (
    <div className={styles.row}>
      <div className={styles.text}>
        <span id={`${id}-label`} className={styles.label}>
          {label}
        </span>
        {hint ? (
          <span id={`${id}-hint`} className={styles.hint}>
            {hint}
          </span>
        ) : null}
      </div>
      <button
        type="button"
        role="switch"
        className={styles.switch}
        aria-checked={checked}
        aria-labelledby={`${id}-label`}
        aria-describedby={hint ? `${id}-hint` : undefined}
        onClick={() => onChange(!checked)}
      >
        <span className={styles.thumb} />
      </button>
    </div>
  );
}
