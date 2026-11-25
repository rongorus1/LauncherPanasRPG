//! Outbound adapters. Implemented against the ports in `launcher_core::application::ports::outbound`.

pub mod curseforge;
pub mod download;
pub mod diagnostics;
mod error;
pub mod forge;
mod fs_util;
pub mod http;
pub mod java;
pub mod mojang;
mod platform;
pub mod process;
pub mod storage;

pub use curseforge::CurseForgePackSource;
pub use download::ParallelDownloader;
pub use diagnostics::FsLocalDiagnostics;
pub use forge::ForgeLoaderInstaller;
pub use http::HttpClient;
pub use java::MojangJavaProvider;
pub use mojang::MojangMinecraftSource;
pub use process::GameProcessRunner;
pub use storage::{FsInstanceStorage, JsonAccountRepository, JsonSettingsRepository};
