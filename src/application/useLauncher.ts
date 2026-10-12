import { createContext, useContext, useSyncExternalStore } from "react";
import type { AppSnapshot, LauncherStore } from "./launcherStore";

export const LauncherStoreContext = createContext<LauncherStore | null>(null);

export function useLauncherStore(): LauncherStore {
  const store = useContext(LauncherStoreContext);
  if (!store) throw new Error("LauncherStoreContext is missing");
  return store;
}

/** Selectors must return primitives or references taken from the snapshot, never new objects. */
export function useLauncher<T>(selector: (snapshot: AppSnapshot) => T): T {
  const store = useLauncherStore();
  return useSyncExternalStore(store.subscribe, () => selector(store.getSnapshot()));
}
