use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt, stream};
use launcher_core::{
    application::{
        error::PortError,
        ports::outbound::{DownloadTask, FileDownloader, PortResult, ProgressReporter},
    },
    domain::{InstallStep, Progress},
};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::{
    error::{from_io, from_reqwest},
    fs_util::{create_dir_all, file_matches, temp_sibling},
    http::HttpClient,
};

const CONCURRENCY: usize = 12;
const RETRIES: u32 = 3;
const BASE_BACKOFF: Duration = Duration::from_millis(500);

/// Downloads many files at once, verifying sha1 and skipping files that already verify.
/// The port does not know the install step, so each instance reports progress under a fixed step.
#[derive(Debug, Clone)]
pub struct ParallelDownloader {
    http: HttpClient,
    step: InstallStep,
    concurrency: usize,
    retries: u32,
    base_backoff: Duration,
}

impl ParallelDownloader {
    /// Reports progress as `DownloadingMinecraft`; use [`Self::with_step`] or [`Self::for_step`] for other steps.
    pub fn new(http: HttpClient) -> Self {
        Self {
            http,
            step: InstallStep::DownloadingMinecraft,
            concurrency: CONCURRENCY,
            retries: RETRIES,
            base_backoff: BASE_BACKOFF,
        }
    }

    pub fn for_step(http: HttpClient, step: InstallStep) -> Self {
        Self::new(http).with_step(step)
    }

    pub fn with_step(mut self, step: InstallStep) -> Self {
        self.step = step;
        self
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    pub fn with_backoff(mut self, base_backoff: Duration) -> Self {
        self.base_backoff = base_backoff;
        self
    }

    pub fn step(&self) -> InstallStep {
        self.step
    }

    async fn download_with_retries(&self, task: &DownloadTask) -> PortResult<()> {
        let mut attempt = 0;
        loop {
            match fetch_to_file(
                &self.http,
                &task.url,
                &task.destination,
                task.sha1.as_deref(),
                task.size,
            )
            .await
            {
                Ok(()) => return Ok(()),
                Err(PortError::Network(_) | PortError::Integrity(_)) if attempt < self.retries => {
                    tokio::time::sleep(self.base_backoff * 2u32.pow(attempt)).await;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

#[async_trait]
impl FileDownloader for ParallelDownloader {
    async fn download_all(
        &self,
        tasks: Vec<DownloadTask>,
        reporter: &dyn ProgressReporter,
    ) -> PortResult<()> {
        let mut seen = HashSet::new();
        let tasks: Vec<DownloadTask> = tasks
            .into_iter()
            .filter(|t| seen.insert(t.destination.clone()))
            .collect();
        let total = tasks.len() as u64;
        let done = AtomicU64::new(0);
        reporter.report(Progress::new(self.step, 0, total, ""));

        stream::iter(tasks.iter().map(Ok))
            .try_for_each_concurrent(self.concurrency, |task| {
                let done = &done;
                async move {
                    if !file_matches(&task.destination, task.sha1.as_deref(), task.size).await {
                        self.download_with_retries(task).await?;
                    }
                    let current = done.fetch_add(1, Ordering::SeqCst) + 1;
                    let name = task
                        .destination
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    reporter.report(Progress::new(self.step, current, total, name));
                    Ok::<(), PortError>(())
                }
            })
            .await
    }
}

/// Streams `url` into a temp file next to `destination`, verifies it and renames it into place.
pub(crate) async fn fetch_to_file(
    http: &HttpClient,
    url: &str,
    destination: &Path,
    sha1: Option<&str>,
    size: Option<u64>,
) -> PortResult<()> {
    if let Some(parent) = destination.parent() {
        create_dir_all(parent).await?;
    }
    let temp = temp_sibling(destination, ".part");
    let result = stream_verified(http, url, &temp, sha1, size).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
        return result;
    }
    tokio::fs::rename(&temp, destination)
        .await
        .map_err(|e| from_io(e, destination))
}

async fn stream_verified(
    http: &HttpClient,
    url: &str,
    temp: &Path,
    sha1: Option<&str>,
    size: Option<u64>,
) -> PortResult<()> {
    let response = http.send(http.client().get(url)).await?;
    let mut file = tokio::fs::File::create(temp)
        .await
        .map_err(|e| from_io(e, temp))?;
    let mut hasher = Sha1::new();
    let mut written = 0u64;
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(from_reqwest)?;
        hasher.update(&chunk);
        written += chunk.len() as u64;
        file.write_all(&chunk).await.map_err(|e| from_io(e, temp))?;
    }
    file.flush().await.map_err(|e| from_io(e, temp))?;
    drop(file);

    if let Some(expected) = size
        && expected != written
    {
        return Err(PortError::Integrity(format!(
            "{url}: expected {expected} bytes, got {written}"
        )));
    }
    if let Some(expected) = sha1 {
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(PortError::Integrity(format!(
                "{url}: sha1 {actual} does not match {expected}"
            )));
        }
    }
    Ok(())
}
