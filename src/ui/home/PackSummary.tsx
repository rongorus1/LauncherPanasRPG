import { useLauncher } from "../../application/useLauncher";
import { UsernameForm } from "../account/UsernameForm";
import { CheckIcon, CircleNotchIcon, WifiSlashIcon } from "../icons";
import styles from "./PackSummary.module.css";

export function PackSummary() {
  const pack = useLauncher((s) => s.launcher?.pack ?? null);
  const hasAccount = useLauncher((s) => s.launcher?.account != null);

  if (!pack) return null;

  if (!hasAccount) {
    return (
      <section className={styles.summary} aria-labelledby="welcome-title">
        <h2 id="welcome-title" className={styles.welcome}>
          Elige tu nombre de jugador
        </h2>
        <p className={styles.lead}>Lo necesitas para entrar al mundo. Puedes cambiarlo cuando quieras.</p>
        <UsernameForm submitLabel="Guardar" layout="inline" />
      </section>
    );
  }

  const installed = pack.installed_version;

  return (
    <section className={styles.summary} aria-label="Estado del pack">
      {installed ? (
        <>
          <p className={styles.label}>Versión instalada</p>
          <p className={styles.version}>{installed}</p>
          <ul className={styles.meta}>
            {pack.minecraft_version ? <li>Minecraft {pack.minecraft_version}</li> : null}
            {pack.forge_version ? <li>Forge {pack.forge_version}</li> : null}
          </ul>
        </>
      ) : (
        <>
          <p className={styles.label}>Panas RPG</p>
          <p className={styles.version}>Sin instalar</p>
          <p className={styles.lead}>Se descargan Java, Minecraft, Forge y todos los mods del pack.</p>
        </>
      )}
      <UpdateLine />
    </section>
  );
}

function UpdateLine() {
  const check = useLauncher((s) => s.updateCheck);
  const latest = useLauncher((s) => s.launcher?.pack.latest_version ?? null);
  const updateAvailable = useLauncher((s) => s.launcher?.pack.update_available ?? false);
  const installed = useLauncher((s) => s.launcher?.pack.installed_version ?? null);

  let content = null;
  if (check === "checking") {
    content = (
      <span className={styles.status}>
        <CircleNotchIcon className={styles.spin} size={16} weight="bold" aria-hidden />
        Buscando actualizaciones
      </span>
    );
  } else if (check === "offline") {
    content = (
      <span className={styles.status}>
        <WifiSlashIcon size={16} weight="bold" aria-hidden />
        {installed
          ? "Sin conexión. No se pudo buscar actualizaciones, pero puedes jugar."
          : "Sin conexión. Conéctate a internet para instalar el pack."}
      </span>
    );
  } else if (updateAvailable && latest) {
    content = <span className={styles.tag}>Actualización disponible: {latest}</span>;
  } else if (check === "done" && installed) {
    content = (
      <span className={styles.status}>
        <CheckIcon size={16} weight="bold" aria-hidden />
        Tienes la última versión
      </span>
    );
  } else if (check === "done" && latest) {
    content = <span className={styles.status}>Última versión: {latest}</span>;
  }

  return (
    <p className={styles.update} role="status">
      {content}
    </p>
  );
}

export function PackSummarySkeleton() {
  return (
    <div className={styles.summary} aria-busy="true" aria-label="Cargando estado del pack">
      <span className={styles.bone} style={{ width: 120, height: 14 }} />
      <span className={styles.bone} style={{ width: 180, height: 44 }} />
      <span className={styles.bone} style={{ width: 240, height: 16 }} />
    </div>
  );
}
