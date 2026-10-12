import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import type { LauncherGateway } from "./application/LauncherGateway";
import { createLauncherStore } from "./application/launcherStore";
import { LauncherStoreContext } from "./application/useLauncher";
import { App } from "./ui/App";
import "./ui/styles/global.css";

// Composition root: the only place that picks a concrete gateway.
async function createGateway(): Promise<LauncherGateway> {
  if ("__TAURI_INTERNALS__" in window) {
    const { TauriLauncherGateway } = await import("./adapters/tauri/TauriLauncherGateway");
    return new TauriLauncherGateway();
  }
  const { InMemoryLauncherGateway, MEMORY_SCENARIOS } = await import("./adapters/memory/InMemoryLauncherGateway");
  const name = new URLSearchParams(window.location.search).get("demo") ?? "fresh";
  // The simulated restart lands on the same pack state without the pending launcher update.
  const restart = () => window.location.replace("?demo=installed");
  return new InMemoryLauncherGateway(MEMORY_SCENARIOS[name] ?? MEMORY_SCENARIOS.fresh, restart);
}

void createGateway().then((gateway) => {
  const store = createLauncherStore(gateway);
  createRoot(document.getElementById("root") as HTMLElement).render(
    <StrictMode>
      <LauncherStoreContext.Provider value={store}>
        <App />
      </LauncherStoreContext.Provider>
    </StrictMode>,
  );
});
