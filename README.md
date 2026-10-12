# Panas RPG Launcher

Offline launcher for the Panas RPG modpack (Minecraft 1.20.1, Forge 47.4.10). Tauri 2, Rust core with hexagonal architecture, React frontend.

```bash
pnpm install
pnpm tauri dev                      # app with hot reload
pnpm dev                            # UI only in the browser, in-memory gateway (?demo=fresh|installed|update|offline|broken|failing|crash)
pnpm tauri build --debug --no-bundle
```

Tests: `cargo test --workspace` (offline), `cargo test --workspace -- --ignored` (hits Mojang and Forge), `pnpm test`, `pnpm lint`, `pnpm typecheck`.

## CurseForge API key

Mods are downloaded through the CurseForge API. Get a free key at https://console.curseforge.com and provide it as `CURSEFORGE_API_KEY`, either at runtime or at build time (`CURSEFORGE_API_KEY=... pnpm tauri build`). Do not commit it.

For local development copy `.env.example` to `.env` and fill it in; the app reads it at startup (`.env` is git-ignored and release builds on the player machine do not read it). The value can be unquoted: the reader never expands `$`, which CurseForge keys contain.

## Architecture

See `docs/ARCHITECTURE.md`. Short version:

- `crates/launcher-core`: domain and application (use cases, ports). No I/O.
- `crates/launcher-infrastructure`: outbound adapters (CurseForge, Mojang, Forge, Java, filesystem, process).
- `src-tauri`: inbound adapter (Tauri commands) and composition root (`composition.rs`).
- `src`: frontend with the same split (domain, application, adapters, ui).

Player data lives in the app data directory (`~/.local/share/com.gatoartstudio.panasrpg.launcher` on Linux): `game/`, `instance/`, `runtimes/`, `settings.json`, `account.json`.
