use std::{io, path::Path};

use launcher_core::application::error::PortError;

pub(crate) fn from_reqwest(error: reqwest::Error) -> PortError {
    let url = error.url().map(|u| u.to_string()).unwrap_or_default();
    if error.is_decode() {
        return PortError::Other(format!("invalid response from {url}: {error}"));
    }
    if let Some(status) = error.status() {
        return from_status(status, &url);
    }
    PortError::Network(format!("{url}: {error}"))
}

pub(crate) fn from_status(status: reqwest::StatusCode, url: &str) -> PortError {
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
        PortError::NotFound(format!("{url} ({status})"))
    } else if status.is_server_error()
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
    {
        PortError::Network(format!("{url} answered {status}"))
    } else {
        PortError::Other(format!("{url} answered {status}"))
    }
}

pub(crate) fn from_io(error: io::Error, path: &Path) -> PortError {
    let detail = format!("{}: {error}", path.display());
    match error.kind() {
        io::ErrorKind::NotFound => PortError::NotFound(detail),
        _ => PortError::Io(detail),
    }
}

pub(crate) fn from_zip(error: zip::result::ZipError, archive: &Path) -> PortError {
    match error {
        zip::result::ZipError::Io(io) => from_io(io, archive),
        other => PortError::Integrity(format!("invalid archive {}: {other}", archive.display())),
    }
}

pub(crate) fn from_json(error: serde_json::Error, source: &str) -> PortError {
    if error.is_io() {
        PortError::Io(format!("{source}: {error}"))
    } else {
        PortError::Other(format!("invalid JSON in {source}: {error}"))
    }
}

pub(crate) fn from_join(error: tokio::task::JoinError) -> PortError {
    PortError::Other(format!("background task failed: {error}"))
}
