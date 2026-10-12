use std::{process::Stdio, sync::Arc};

use async_trait::async_trait;
use launcher_core::{
    application::{
        error::PortError,
        events::LogStream,
        ports::outbound::{GameExit, GameLogSink, GameProcess, PortResult},
    },
    domain::LaunchPlan,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::{Mutex, oneshot},
};

use crate::error::from_io;

#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Spawns the game JVM and streams its output. Only one game runs at a time.
#[derive(Debug, Default, Clone)]
pub struct GameProcessRunner {
    kill: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl GameProcessRunner {
    pub fn new() -> Self {
        Self::default()
    }
}

fn forward_lines<R>(
    reader: R,
    stream: LogStream,
    sink: Arc<dyn GameLogSink>,
) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    // The game may print non UTF-8 bytes (e.g. Windows code pages).
                    let line = String::from_utf8_lossy(&buffer);
                    sink.line(stream, line.trim_end_matches(['\n', '\r']).to_owned());
                }
            }
        }
    })
}

#[async_trait]
impl GameProcess for GameProcessRunner {
    async fn run(&self, plan: &LaunchPlan, sink: Arc<dyn GameLogSink>) -> PortResult<GameExit> {
        let (kill_tx, kill_rx) = oneshot::channel();
        {
            let mut slot = self.kill.lock().await;
            if slot.as_ref().is_some_and(|tx| !tx.is_closed()) {
                return Err(PortError::Other("the game is already running".to_owned()));
            }
            *slot = Some(kill_tx);
        }

        let mut command = tokio::process::Command::new(&plan.java);
        command
            .args(&plan.jvm_args)
            .arg(&plan.main_class)
            .args(&plan.game_args)
            .current_dir(&plan.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.kill.lock().await.take();
                return Err(from_io(error, &plan.java));
            }
        };
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(forward_lines(stdout, LogStream::Stdout, sink.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(forward_lines(stderr, LogStream::Stderr, sink.clone()));
        }

        let status = tokio::select! {
            status = child.wait() => status,
            _ = kill_rx => {
                let _ = child.start_kill();
                child.wait().await
            }
        };
        self.kill.lock().await.take();
        // Child processes spawned by the game may keep the pipes open after it exits.
        for reader in readers {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), reader).await;
        }
        let status = status.map_err(|e| from_io(e, &plan.java))?;
        Ok(GameExit {
            code: status.code(),
        })
    }

    async fn stop(&self) -> PortResult<()> {
        if let Some(kill) = self.kill.lock().await.take() {
            let _ = kill.send(());
        }
        Ok(())
    }
}
