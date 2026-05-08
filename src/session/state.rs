#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Idle,
    Running,
    WaitingForInput { prompt_snapshot: String },
    Stopped,
}

impl SessionState {
    pub fn transition_to_running(&mut self) {
        *self = SessionState::Running;
    }

    pub fn transition_to_waiting(&mut self, prompt: String) {
        *self = SessionState::WaitingForInput {
            prompt_snapshot: prompt,
        };
    }

    pub fn transition_to_idle(&mut self) {
        *self = SessionState::Idle;
    }

    pub fn transition_to_stopped(&mut self) {
        *self = SessionState::Stopped;
    }

    pub fn is_waiting_for_input(&self) -> bool {
        matches!(self, SessionState::WaitingForInput { .. })
    }
}
