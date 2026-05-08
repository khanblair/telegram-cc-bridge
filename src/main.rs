mod adapters;
mod bot;
mod config;
mod media;
mod pty;
mod session;

use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter("telegram_cc_bridge=info")
        .init();

    info!("Loading configuration...");
    let app_config = Arc::new(config::AppConfig::load()?);

    info!("Initializing database...");
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:bridge.db")
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let recorder = session::recorder::Recorder::new(pool);
    let session_manager = Arc::new(session::manager::SessionManager::new(recorder));

    info!("Starting Telegram CC Bridge...");
    bot::run_bot((*app_config).clone(), session_manager).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::adapters::{CliAdapter, ClaudeCodeAdapter, CodexAdapter, GeminiAdapter};
    use crate::bot::formatter::chunk_message;
    use crate::session::state::SessionState;

    #[test]
    fn test_session_state_transitions() {
        let mut state = SessionState::Idle;
        state.transition_to_running();
        assert!(matches!(state, SessionState::Running));

        state.transition_to_waiting("test prompt".to_string());
        assert!(matches!(state, SessionState::WaitingForInput { .. }));
        assert!(state.is_waiting_for_input());

        state.transition_to_idle();
        assert!(matches!(state, SessionState::Idle));

        state.transition_to_stopped();
        assert!(matches!(state, SessionState::Stopped));
    }

    #[test]
    fn test_chunk_message_short() {
        let text = "Hello, world!";
        let chunks = chunk_message(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_message_long() {
        let text = "a".repeat(5000);
        let chunks = chunk_message(&text);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 4096);
        }
    }

    #[test]
    fn test_chunk_message_empty() {
        let chunks = chunk_message("");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }

    #[test]
    fn test_claude_adapter_name() {
        let adapter = ClaudeCodeAdapter::new("claude".to_string());
        assert_eq!(adapter.name(), "claude");
    }

    #[test]
    fn test_claude_input_patterns() {
        let adapter = ClaudeCodeAdapter::new("claude".to_string());
        let patterns = adapter.input_prompt_patterns();
        assert!(!patterns.is_empty());

        let clean = adapter.strip_output("\x1b[31m(y/N)\x1b[0m");
        assert!(clean.contains("(y/N)"));
    }

    #[test]
    fn test_gemini_adapter_name() {
        let adapter = GeminiAdapter::new("gemini".to_string());
        assert_eq!(adapter.name(), "gemini");
    }

    #[test]
    fn test_codex_adapter_name() {
        let adapter = CodexAdapter::new("codex".to_string());
        assert_eq!(adapter.name(), "codex");
    }

    #[test]
    fn test_strip_ansi() {
        let adapter = ClaudeCodeAdapter::new("claude".to_string());
        let raw = "\x1b[1mHello\x1b[0m \x1b[32mWorld\x1b[0m";
        let clean = adapter.strip_output(raw);
        assert_eq!(clean, "Hello World");
    }
}
