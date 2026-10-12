//! Core of the Panas RPG launcher (hexagonal architecture).
//!
//! - `domain`: business rules and value objects. No I/O, no frameworks.
//! - `application`: use cases plus the ports they depend on (inbound and outbound).
//!
//! Adapters live elsewhere: `launcher-infrastructure` (outbound) and `src-tauri` (inbound + composition root).

pub mod application;
pub mod domain;
