use crate::adapters::CliAdapter;
use crate::session::recorder::Recorder;
use crate::session::state::SessionState;
use crate::bot::formatter::TerminalScreen;
use anyhow::Result;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info};

#[derive(Clone)]
pub struct PtySupervisor {
    stdin_tx: mpsc::UnboundedSender<String>,
}

impl PtySupervisor {
    pub fn spawn(
        adapter: Arc<dyn CliAdapter>,
        workdir: &Path,
        state: Arc<Mutex<SessionState>>,
        recorder: Recorder,
        session_id: i64,
        output_tx: broadcast::Sender<String>,
    ) -> Result<Self> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(adapter.spawn_cmd(workdir).get_program());
        cmd.cwd(workdir);
        for arg in adapter.spawn_cmd(workdir).get_args() {
            cmd.arg(arg);
        }
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }

        let _child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut master_reader = pair.master.try_clone_reader()?;
        let mut master_writer = pair.master.take_writer()?;

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

        let state_reader = state.clone();
        let patterns: Vec<_> = adapter.input_prompt_patterns().to_vec();

        // stdout reader task (blocking IO in spawn_blocking)
        tokio::task::spawn_blocking(move || {
            let mut screen = TerminalScreen::new();
            let mut buf = [0u8; 4096];

            loop {
                match master_reader.read(&mut buf) {
                    Ok(0) => {
                        info!("PTY stdout closed for session {}", session_id);
                        let rt = tokio::runtime::Handle::try_current();
                        if let Ok(handle) = rt {
                            let s = state_reader.clone();
                            handle.block_on(async move {
                                let mut st = s.lock().await;
                                st.transition_to_stopped();
                            });
                        }
                        break;
                    }
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]);
                        let lines = screen.process(&chunk);

                        let rt = tokio::runtime::Handle::try_current();
                        if let Ok(handle) = rt {
                            for line in lines {
                                // Check for input prompts
                                for pat in &patterns {
                                    if pat.is_match(&line) {
                                        let s = state_reader.clone();
                                        let otx = output_tx.clone();
                                        let line_clone = line.clone();
                                        handle.block_on(async move {
                                            let mut st = s.lock().await;
                                            st.transition_to_waiting(line_clone.clone());
                                            let _ = otx.send(format!("⏸️ Waiting for input: {}", line_clone));
                                        });
                                        break;
                                    }
                                }

                                // Record output
                                let rec = recorder.clone();
                                let otx = output_tx.clone();
                                handle.block_on(async move {
                                    if let Err(e) = rec.record_event(session_id, "out", &line).await {
                                        error!("Recorder error: {}", e);
                                    }
                                    let _ = otx.send(line);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        error!("PTY read error: {}", e);
                        break;
                    }
                }
            }

            // Flush remaining content
            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
                if let Some(line) = screen.flush() {
                    let rec = recorder.clone();
                    let otx = output_tx.clone();
                    handle.block_on(async move {
                        if let Err(e) = rec.record_event(session_id, "out", &line).await {
                            error!("Recorder error: {}", e);
                        }
                        let _ = otx.send(line);
                    });
                }

                let s = state_reader.clone();
                handle.block_on(async move {
                    let mut st = s.lock().await;
                    st.transition_to_stopped();
                });
            }
        });

        // stdin writer task (blocking IO in spawn_blocking)
        tokio::task::spawn_blocking(move || {
            while let Some(input) = stdin_rx.blocking_recv() {
                if let Err(e) = master_writer.write_all(input.as_bytes()) {
                    error!("PTY write error: {}", e);
                    break;
                }
                if let Err(e) = master_writer.write_all(b"\n") {
                    error!("PTY write error: {}", e);
                    break;
                }
                if let Err(e) = master_writer.flush() {
                    error!("PTY flush error: {}", e);
                    break;
                }
            }
        });

        Ok(PtySupervisor { stdin_tx })
    }

    pub fn write_stdin(&self, input: String) -> Result<()> {
        self.stdin_tx
            .send(input)
            .map_err(|_| anyhow::anyhow!("stdin channel closed"))?;
        Ok(())
    }
}
