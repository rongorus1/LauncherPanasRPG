# Architecture

Hexagonal (ports and adapters). Dependencies always point inward.

```
 Tauri commands (inbound adapter)        Outbound adapters
 src-tauri/src/adapters/inbound   --->   crates/launcher-infrastructure
            |                                   ^
            v                                   | implements
   inbound ports (traits)  --> use cases --> outbound ports (traits)
            \_______________ crates/launcher-core ______________/
                         application + domain
```

| Layer | Where | Rules |
|---|---|---|
| Domain | `crates/launcher-core/src/domain` | Pure types and rules. No I/O, no async, no frameworks. |
| Application | `crates/launcher-core/src/application` | Use cases, inbound ports (`ports/inbound.rs`), outbound ports (`ports/outbound.rs`), DTOs, events. Depends on domain only. |
| Outbound adapters | `crates/launcher-infrastructure` | reqwest, filesystem, zip, process. Implements `ports/outbound.rs`. Translates library errors into `PortError`. Never called from use cases directly. |
| Inbound adapter + composition root | `src-tauri` | Tauri commands call inbound ports. `composition.rs` is the only place that builds concrete adapters and injects them into use cases. |
| Frontend | `src/` | Same idea in TypeScript: domain types, a `LauncherGateway` port, a Tauri adapter and an in-memory adapter, UI on top. |

`launcher-core` must never depend on reqwest, tokio runtime features beyond what tests need, tauri or the filesystem.

## Domain language

- **Pack**: the modpack Panas RPG (CurseForge project `1493979`). A **release** is one published file; a **manifest** lists the Minecraft version, the Forge version and the mod files.
- **Instance**: the single game directory where the pack lives (mods, config, saves).
- **Offline account**: a username only. The UUID is derived from the name (`OfflinePlayer:<name>`).
- **Launch plan**: java, JVM args, main class and game args, built by merging the Forge descriptor with its vanilla parent.

## Flows

**Install or update** (`InstallPack`): latest release -> fetch pack (manifest + overrides in staging) -> resolve mod files -> ensure Java (Mojang runtime) -> vanilla descriptor -> install Forge (runs the official installer) -> merge descriptors -> download game files (client jar, libraries, assets) -> download mods -> prune stale mods -> apply overrides without touching player data -> save installed pack. Progress goes out as `LauncherEvent::InstallProgress`.

**Play** (`PlayGame`): requires an account and an installed pack -> ensure Java -> read installed descriptor -> build launch plan with the offline account (`--uuid`, `--username`, `--accessToken 0`, `--userType legacy`) -> run process, forwarding log lines as events -> `GameExited`.

## Tauri contract

Commands (all return `Result<T, AppError>`, `AppError = { kind, detail }`):

| Command | Args | Returns |
|---|---|---|
| `get_launcher_state` | none | `LauncherState` |
| `save_offline_account` | `username: string` | `AccountView` |
| `save_settings` | `settings: Settings` | `Settings` |
| `check_pack_update` | none | `PackStatus` |
| `install_pack` | none | `void` (progress via events) |
| `play_game` | none | `void` (logs via events) |
| `stop_game` | none | `void` |

Events: a single channel named `launcher-event`; payload is `LauncherEvent` serialized with `{ "type": "install_progress" | "install_finished" | "install_failed" | "game_started" | "game_log" | "game_exited", ... }` (see `application/events.rs`).

Serialization is snake_case everywhere (`max_memory_mb`, `installed_version`).

## Testing

- Domain: plain unit tests.
- Use cases: unit tests with in-memory fakes of the outbound ports (module `application::use_cases::testing`, `#[cfg(test)]`).
- Outbound adapters: integration tests against a local fake HTTP server and temp directories; network-hitting tests are `#[ignore]`.
- Frontend: the in-memory gateway lets the UI run in a plain browser (`pnpm dev`).
