use crate::adapters::CliAdapter;
use crate::session::recorder::Recorder;
use crate::session::state::SessionState;
use crate::pty::supervisor::PtySupervisor;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub struct SessionHandle {
    pub chat_id: i64,
    pub session_id: i64,
    pub state: Arc<Mutex<SessionState>>,
    pub supervisor: PtySupervisor,
    pub output_tx: broadcast::Sender<String>,
    pub adapter_name: String,
}

impl Clone for SessionHandle {
    fn clone(&self) -> Self {
        Self {
            chat_id: self.chat_id,
            session_id: self.session_id,
            state: self.state.clone(),
            supervisor: self.supervisor.clone(),
            output_tx: self.output_tx.clone(),
            adapter_name: self.adapter_name.clone(),
        }
    }
}

pub struct SessionManager {
    pub sessions: Arc<Mutex<HashMap<i64, SessionHandle>>>,
    pub recorder: Recorder,
}

impl SessionManager {
    pub fn new(recorder: Recorder) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            recorder,
        }
    }

    pub async fn spawn_session(
        &self,
        chat_id: i64,
        adapter: Arc<dyn CliAdapter>,
        workdir: &std::path::Path,
    ) -> Result<(SessionHandle, broadcast::Receiver<String>)> {
        let mut sessions = self.sessions.lock().await;

        if sessions.contains_key(&chat_id) {
            return Err(anyhow!(
                "Session already exists for chat {}. Use /reset to restart.",
                chat_id
            ));
        }

        let session_id = self
            .recorder
            .create_session(chat_id, adapter.name())
            .await?;

        let state = Arc::new(Mutex::new(SessionState::Idle));
        let (output_tx, output_rx) = broadcast::channel::<String>(256);

        let supervisor = PtySupervisor::spawn(
            adapter.clone(),
            workdir,
            state.clone(),
            self.recorder.clone(),
            session_id,
            output_tx.clone(),
        )?;

        let handle = SessionHandle {
            chat_id,
            session_id,
            state,
            supervisor,
            output_tx: output_tx.clone(),
            adapter_name: adapter.name().to_string(),
        };

        sessions.insert(chat_id, handle.clone());

        Ok((handle, output_rx))
    }

    pub async fn get_session(&self, chat_id: i64) -> Option<SessionHandle> {
        let sessions = self.sessions.lock().await;
        sessions.get(&chat_id).cloned()
    }

    pub async fn kill_session(&self, chat_id: i64) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(_handle) = sessions.remove(&chat_id) {
            // PTY will be dropped when handle is dropped, stopping the process
        }
        Ok(())
    }

    pub async fn reset_session(
        &self,
        chat_id: i64,
        adapter: Arc<dyn CliAdapter>,
        workdir: &std::path::Path,
    ) -> Result<(SessionHandle, broadcast::Receiver<String>)> {
        self.kill_session(chat_id).await.ok();
        self.spawn_session(chat_id, adapter, workdir).await
    }
}
