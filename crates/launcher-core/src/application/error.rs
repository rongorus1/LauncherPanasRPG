use serde::Serialize;
use thiserror::Error;

use crate::domain::DomainError;

/// Failure reported by an outbound adapter. Adapters translate their library errors into these.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    #[error("network error: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("integrity check failed: {0}")]
    Integrity(String),
    #[error("i/o error: {0}")]
    Io(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorKind {
    InvalidInput,
    NoAccount,
    NotInstalled,
    Busy,
    Network,
    Integrity,
    Io,
    Internal,
}

/// Error returned by use cases. The kind lets the UI pick a message; `detail` is for logs.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[error("{kind:?}: {detail}")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub detail: String,
}

impl AppError {
    pub fn new(kind: AppErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl From<DomainError> for AppError {
    fn from(error: DomainError) -> Self {
        Self::new(AppErrorKind::InvalidInput, error.to_string())
    }
}

impl From<PortError> for AppError {
    fn from(error: PortError) -> Self {
        let kind = match &error {
            PortError::Network(_) => AppErrorKind::Network,
            PortError::Integrity(_) => AppErrorKind::Integrity,
            PortError::Io(_) | PortError::NotFound(_) => AppErrorKind::Io,
            PortError::Unsupported(_) | PortError::Other(_) => AppErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}
