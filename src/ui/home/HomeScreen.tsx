import { useLauncher } from "../../application/useLauncher";
import background from "../assets/background.webp";
import { ConsolePanel } from "../console/ConsolePanel";
import { HomeNotice } from "./HomeNotice";
import { LauncherUpdatePanel } from "./LauncherUpdatePanel";
import { PackSummary, PackSummarySkeleton } from "./PackSummary";
import { PrimaryAction, PrimaryActionSkeleton } from "./PrimaryAction";
import styles from "./HomeScreen.module.css";

interface Props {
  onOpenConsole: () => void;
  onOpenAccount: () => void;
}

export function HomeScreen({ onOpenConsole, onOpenAccount }: Props) {
  const phase = useLauncher((s) => s.phase);
  const playing = useLauncher((s) => s.launcher?.activity === "playing");

  return (
    <div className={styles.home}>
      <img className={styles.art} src={background} alt="" decoding="async" fetchPriority="high" />
      <div className={styles.shade} aria-hidden />
      <h1 className="visually-hidden">Panas RPG</h1>

      <div className={styles.corner}>
        <LauncherUpdatePanel />
      </div>

      <div className={styles.stack}>
        <div className={styles.notice}>
          <HomeNotice onOpenConsole={onOpenConsole} />
        </div>
        {playing ? (
          <div className={styles.console}>
            <ConsolePanel variant="docked" />
          </div>
        ) : null}
        <div className={styles.dock}>
          {phase === "ready" ? (
            <>
              <PackSummary />
              <PrimaryAction onOpenAccount={onOpenAccount} />
            </>
          ) : (
            <>
              <PackSummarySkeleton />
              <PrimaryActionSkeleton />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
