use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("invalid username: {0}")]
    InvalidUsername(String),
    #[error("invalid pack version: {0}")]
    InvalidPackVersion(String),
    #[error("invalid setting: {0}")]
    InvalidSetting(String),
    #[error("invalid version descriptor: {0}")]
    InvalidVersionDescriptor(String),
}
