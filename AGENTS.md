# Agent rules

## Architecture

Hexagonal architecture, see `docs/ARCHITECTURE.md`. Keep dependencies pointing inward: `launcher-core` knows nothing about reqwest, Tauri or the filesystem. New side effects get an outbound port first, then an adapter.

## Code comments

- Write every code comment in English.
- Add a comment only when it is necessary: a non-obvious reason, a constraint, a workaround or a magic number. Do not narrate what the code says and do not add doc comments that repeat a name or a type.
- Product copy and UI strings stay in Spanish.

## Commits

English, `type: description`, no co-author trailer.
