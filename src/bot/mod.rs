pub mod formatter;
pub mod handlers;

use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use crate::config::AppConfig;
use crate::session::manager::SessionManager;
use std::sync::Arc;
use tracing::info;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Telegram CC Bridge commands:")]
pub enum Command {
    #[command(description = "Open a new CLI session")]
    Start,
    #[command(description = "Kill the current session")]
    Stop,
    #[command(description = "Kill and restart a fresh session")]
    Reset,
    #[command(description = "Switch active adapter: /use claude | gemini | codex")]
    Use(String),
    #[command(description = "Show last n events (default 20)")]
    History(String),
    #[command(description = "Show current session state and adapter")]
    Status,
}

pub async fn run_bot(
    config: AppConfig,
    session_manager: Arc<SessionManager>,
) {
    let bot = Bot::new(&config.telegram.bot_token);

    info!("Starting Telegram bot...");

    let handler = Update::filter_message().endpoint(handlers::handle_message);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config, session_manager])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
