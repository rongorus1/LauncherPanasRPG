use std::time::Duration;

use launcher_core::application::{error::PortError, ports::outbound::PortResult};
use reqwest::{RequestBuilder, Response};
use serde::de::DeserializeOwned;

use crate::error::{from_json, from_reqwest, from_status};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
// Per-read timeout instead of a total one: large downloads may take minutes but must keep flowing.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Shared HTTP client (rustls, connection pool). Cheap to clone.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    /// `app_version` ends up in the user agent: `PanasRPGLauncher/<app_version>`.
    pub fn new(app_version: &str) -> PortResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent(format!("PanasRPGLauncher/{app_version}"))
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|e| PortError::Other(format!("cannot build HTTP client: {e}")))?;
        Ok(Self { client })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Sends the request and maps non-success statuses to `PortError`.
    pub(crate) async fn send(&self, request: RequestBuilder) -> PortResult<Response> {
        let response = request.send().await.map_err(from_reqwest)?;
        ensure_success(response)
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(&self, url: &str) -> PortResult<T> {
        let response = self.send(self.client.get(url)).await?;
        read_json(response).await
    }
}

pub(crate) fn ensure_success(response: Response) -> PortResult<Response> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        Err(from_status(status, response.url().as_str()))
    }
}

pub(crate) async fn read_json<T: DeserializeOwned>(response: Response) -> PortResult<T> {
    let url = response.url().to_string();
    let bytes = response.bytes().await.map_err(from_reqwest)?;
    serde_json::from_slice(&bytes).map_err(|e| from_json(e, &url))
}
