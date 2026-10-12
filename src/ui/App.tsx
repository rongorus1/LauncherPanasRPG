import { Activity, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useLauncher, useLauncherStore } from "../application/useLauncher";
import { errorCopy } from "../domain/errors";
import { AccountDialog } from "./account/AccountDialog";
import logo from "./assets/logo-64.jpg";
import btn from "./components/Button.module.css";
import { ErrorPanel } from "./components/ErrorPanel";
import { ConsolePanel } from "./console/ConsolePanel";
import { HomeScreen } from "./home/HomeScreen";
import { ArrowClockwiseIcon, UserIcon } from "./icons";
import { SettingsScreen } from "./settings/SettingsScreen";
import styles from "./App.module.css";

type Tab = "home" | "settings" | "console";

const TABS: ReadonlyArray<{ id: Tab; label: string }> = [
  { id: "home", label: "Inicio" },
  { id: "settings", label: "Ajustes" },
  { id: "console", label: "Consola" },
];

export function App() {
  const store = useLauncherStore();
  const phase = useLauncher((s) => s.phase);
  const [tab, setTab] = useState<Tab>("home");
  const [accountOpen, setAccountOpen] = useState(false);

  useEffect(() => store.start(), [store]);

  return (
    <div className={styles.shell}>
      <NavBar tab={tab} onTab={setTab} onOpenAccount={() => setAccountOpen(true)} />
      <main className={styles.main}>
        {phase === "failed" ? (
          <LoadFailed />
        ) : (
          TABS.map(({ id }) => (
            <div
              key={id}
              id={`panel-${id}`}
              role="tabpanel"
              aria-labelledby={`tab-${id}`}
              className={styles.panel}
              hidden={tab !== id}
            >
              <Activity mode={tab === id ? "visible" : "hidden"}>
                {id === "home" ? (
                  <HomeScreen onOpenConsole={() => setTab("console")} onOpenAccount={() => setAccountOpen(true)} />
                ) : id === "settings" ? (
                  phase === "ready" ? <SettingsScreen /> : null
                ) : (
                  <div className={styles.consoleScreen}>
                    <ConsolePanel variant="full" />
                  </div>
                )}
              </Activity>
            </div>
          ))
        )}
      </main>
      <AccountDialog open={accountOpen} onClose={() => setAccountOpen(false)} />
    </div>
  );
}

interface NavProps {
  tab: Tab;
  onTab: (tab: Tab) => void;
  onOpenAccount: () => void;
}

function NavBar({ tab, onTab, onOpenAccount }: NavProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const playing = useLauncher((s) => s.launcher?.activity === "playing");

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const index = TABS.findIndex((t) => t.id === tab);
    let next: number;
    if (event.key === "ArrowRight") next = (index + 1) % TABS.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + TABS.length) % TABS.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = TABS.length - 1;
    else return;
    event.preventDefault();
    onTab(TABS[next].id);
    listRef.current?.querySelector<HTMLButtonElement>(`#tab-${TABS[next].id}`)?.focus();
  }

  return (
    <header className={styles.nav}>
      <div className={styles.brand}>
        <img src={logo} alt="" width={32} height={32} />
        <span>Panas RPG</span>
      </div>
      <div ref={listRef} role="tablist" aria-label="Secciones" className={styles.tabs} onKeyDown={onKeyDown}>
        {TABS.map(({ id, label }) => (
          <button
            key={id}
            id={`tab-${id}`}
            type="button"
            role="tab"
            className={styles.tab}
            aria-selected={tab === id}
            aria-controls={`panel-${id}`}
            tabIndex={tab === id ? 0 : -1}
            onClick={() => onTab(id)}
          >
            {label}
            {id === "console" && playing ? <span className={styles.live}>En vivo</span> : null}
          </button>
        ))}
      </div>
      <AccountChip onOpen={onOpenAccount} />
    </header>
  );
}

function AccountChip({ onOpen }: { onOpen: () => void }) {
  const phase = useLauncher((s) => s.phase);
  const username = useLauncher((s) => s.launcher?.account?.username ?? null);
  if (phase !== "ready") return <span className={styles.chipPlaceholder} aria-hidden />;
  return (
    <button
      type="button"
      className={styles.chip}
      onClick={onOpen}
      aria-label={username ? `Cuenta: ${username}. Cambiar nombre` : "Elegir nombre de jugador"}
    >
      <UserIcon size={16} weight="bold" aria-hidden />
      <span>{username ?? "Sin nombre"}</span>
    </button>
  );
}

function LoadFailed() {
  const store = useLauncherStore();
  const error = useLauncher((s) => s.loadError);
  if (!error) return null;
  const copy = errorCopy(error.kind);
  return (
    <div className={styles.failed}>
      <ErrorPanel
        tone="page"
        title="No se pudo abrir el launcher"
        body={`${copy.title}. ${copy.body}`}
        detail={error.detail}
        actions={
          <button type="button" className={`${btn.btn} ${btn.primary}`} onClick={() => void store.load()}>
            <ArrowClockwiseIcon size={16} weight="bold" aria-hidden />
            Reintentar
          </button>
        }
      />
    </div>
  );
}
