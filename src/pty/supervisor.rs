use crate::adapters::CliAdapter;
use crate::session::recorder::Recorder;
use crate::session::state::SessionState;
use anyhow::Result;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{BufRead, BufReader, Write};
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

        let _child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut master_reader = pair.master.try_clone_reader()?;
        let mut master_writer = pair.master.take_writer()?;

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

        let state_reader = state.clone();
        let patterns: Vec<_> = adapter.input_prompt_patterns().to_vec();

        // stdout reader task (blocking IO in spawn_blocking)
        tokio::task::spawn_blocking(move || {
            let mut buf = String::new();
            let mut reader = BufReader::new(&mut master_reader);

            loop {
                buf.clear();
                match reader.read_line(&mut buf) {
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
                    Ok(_) => {
                        let line = buf.trim_end().to_string();
                        if line.is_empty() {
                            continue;
                        }

                        let line = line.replace('\r', "\n");
                        let clean = strip_ansi_escapes::strip_str(&line);

                        let rt = tokio::runtime::Handle::try_current();
                        if let Ok(handle) = rt {
                            // Check for input prompts
                            for pat in &patterns {
                                if pat.is_match(&clean) {
                                    let s = state_reader.clone();
                                    let otx = output_tx.clone();
                                    let clean_clone = clean.clone();
                                    handle.block_on(async move {
                                        let mut st = s.lock().await;
                                        let _ = otx.send(format!("⏸️ Waiting for input: {}", clean_clone.clone()));
                                        st.transition_to_waiting(clean_clone);
                                    });
                                    break;
                                }
                            }

                            // Record output
                            let rec = recorder.clone();
                            let otx = output_tx.clone();
                            handle.block_on(async move {
                                if let Err(e) = rec.record_event(session_id, "out", &clean).await {
                                    error!("Recorder error: {}", e);
                                }
                                let _ = otx.send(clean);
                            });
                        }
                    }
                    Err(e) => {
                        error!("PTY read error: {}", e);
                        break;
                    }
                }
            }

            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
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
