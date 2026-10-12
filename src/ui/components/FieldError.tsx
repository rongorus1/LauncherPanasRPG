import { WarningIcon } from "../icons";
import styles from "./Field.module.css";

export function FieldError({ id, message }: { id: string; message: string | undefined }) {
  if (!message) return null;
  return (
    <p id={id} className={styles.error}>
      <WarningIcon size={15} weight="bold" aria-hidden />
      {message}
    </p>
  );
}
