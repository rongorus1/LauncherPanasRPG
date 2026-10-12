import { useEffect, useRef } from "react";
import { useLauncher } from "../../application/useLauncher";
import { XIcon } from "../icons";
import styles from "./AccountDialog.module.css";
import { UsernameForm } from "./UsernameForm";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function AccountDialog({ open, onClose }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const account = useLauncher((s) => s.launcher?.account ?? null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      className={styles.dialog}
      aria-labelledby="account-title"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* Re-keyed on every open so the form starts clean, while staying mounted for the exit transition. */}
      <div key={open ? "open" : "closed"} className={styles.body}>
          <header className={styles.head}>
            <h2 id="account-title">{account ? "Cambiar nombre" : "Elige tu nombre"}</h2>
            <button type="button" className={styles.close} onClick={onClose} aria-label="Cerrar">
              <XIcon size={18} weight="bold" aria-hidden />
            </button>
          </header>
          <p className={styles.lead}>
            Cuenta sin conexión: solo hace falta un nombre. Se guarda en este equipo.
          </p>
          <UsernameForm
            initial={account?.username ?? ""}
            submitLabel="Guardar"
            onSaved={onClose}
            onCancel={onClose}
            autoFocus
          />
      </div>
    </dialog>
  );
}
