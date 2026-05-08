use crate::adapters::{CliAdapter, ClaudeCodeAdapter, CodexAdapter, GeminiAdapter};
use crate::bot::{formatter, Command};
use crate::config::AppConfig;
use crate::session::manager::SessionManager;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::ChatAction;
use teloxide::utils::command::BotCommands;
use tokio::sync::broadcast;
use tracing::error;

pub async fn handle_message(
    bot: Bot,
    msg: Message,
    config: AppConfig,
    session_manager: Arc<SessionManager>,
) -> Result<()> {
    let _chat_id = msg.chat.id.0;

    // Whitelist check
    if let Some(user) = &msg.from {
        if !config.telegram.whitelist.contains(&(user.id.0 as i64)) {
            bot.send_message(msg.chat.id, "Access denied.").await?;
            return Ok(());
        }
    }

    // Handle commands
    if let Some(text) = msg.text() {
        if let Ok(cmd) = Command::parse(text, "telegram_cc_bridge_bot") {
            return handle_command(bot, msg, cmd, config, session_manager).await;
        }
    }

    // Handle regular messages (text, voice, photo)
    handle_content(bot, msg, config, session_manager).await
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    config: AppConfig,
    session_manager: Arc<SessionManager>,
) -> Result<()> {
    let chat_id = msg.chat.id.0;

    match cmd {
        Command::Start => {
            let adapter = get_adapter(&config.session.default_adapter, &config)?;
            let workdir = PathBuf::from(&config.session.workdir);
            match session_manager.spawn_session(chat_id, adapter, &workdir).await {
                Ok((handle, mut output_rx)) => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "Session started with `{}` adapter.",
                            handle.adapter_name
                        ),
                    )
                    .await?;

                    // Spawn output listener with buffering and typing indicator
                    let bot2 = bot.clone();
                    let chat_id2 = msg.chat.id;
                    tokio::spawn(async move {
                        output_listener(bot2, chat_id2, output_rx).await;
                    });
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Error: {}", e))
                        .await?;
                }
            }
        }
        Command::Stop => {
            match session_manager.kill_session(chat_id).await {
                Ok(_) => {
                    bot.send_message(msg.chat.id, "Session stopped.").await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Error: {}", e))
                        .await?;
                }
            }
        }
        Command::Reset => {
            let adapter = get_adapter(&config.session.default_adapter, &config)?;
            let workdir = PathBuf::from(&config.session.workdir);
            match session_manager.reset_session(chat_id, adapter, &workdir).await {
                Ok((handle, mut output_rx)) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Session reset with `{}` adapter.", handle.adapter_name),
                    )
                    .await?;

                    let bot2 = bot.clone();
                    let chat_id2 = msg.chat.id;
                    tokio::spawn(async move {
                        output_listener(bot2, chat_id2, output_rx).await;
                    });
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Error: {}", e))
                        .await?;
                }
            }
        }
        Command::Use(adapter_name) => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "Adapter switch to `{}` will take effect on next /start or /reset.",
                    adapter_name
                ),
            )
            .await?;
        }
        Command::History(n_str) => {
            let limit: i64 = n_str.trim().parse().unwrap_or(20);
            if let Some(handle) = session_manager.get_session(chat_id).await {
                match session_manager
                    .recorder
                    .get_recent_events(handle.session_id, limit)
                    .await
                {
                    Ok(events) => {
                        let mut text = String::from("Recent events:\n");
                        for (ts, dir, content) in events {
                            text.push_str(&format!(
                                "`{}` *{}*: {}\n",
                                ts,
                                dir,
                                content.replace('`', "\\`")
                            ));
                        }
                        for chunk in formatter::chunk_message(&text) {
                            bot.send_message(msg.chat.id, chunk).await?;
                        }
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error: {}", e))
                            .await?;
                    }
                }
            } else {
                bot.send_message(msg.chat.id, "No active session.").await?;
            }
        }
        Command::Status => {
            if let Some(handle) = session_manager.get_session(chat_id).await {
                let state = handle.state.lock().await;
                let state_str = format!("{:?}", *state);
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Adapter: `{}`\nState: `{}`",
                        handle.adapter_name, state_str
                    ),
                )
                .await?;
            } else {
                bot.send_message(msg.chat.id, "No active session.").await?;
            }
        }
    }

    Ok(())
}

async fn handle_content(
    bot: Bot,
    msg: Message,
    _config: AppConfig,
    session_manager: Arc<SessionManager>,
) -> Result<()> {
    let chat_id = msg.chat.id.0;

    let handle = match session_manager.get_session(chat_id).await {
        Some(h) => h,
        None => {
            bot.send_message(msg.chat.id, "No active session. Use /start to begin.")
                .await?;
            return Ok(());
        }
    };

    // Check if waiting for input
    let is_waiting = {
        let state = handle.state.lock().await;
        state.is_waiting_for_input()
    };

    let input_text = if let Some(text) = msg.text() {
        text.to_string()
    } else if msg.voice().is_some() {
        bot.send_message(msg.chat.id, "Voice transcription not yet implemented.")
            .await?;
        return Ok(());
    } else if msg.photo().is_some() {
        bot.send_message(msg.chat.id, "Image passthrough not yet implemented.")
            .await?;
        return Ok(());
    } else {
        bot.send_message(msg.chat.id, "Unsupported message type.")
            .await?;
        return Ok(());
    };

    if is_waiting {
        // Direct stdin input
        if let Err(e) = handle.supervisor.write_stdin(input_text) {
            bot.send_message(msg.chat.id, format!("Error sending input: {}", e))
                .await?;
        }
        // Transition back to Running
        let mut state = handle.state.lock().await;
        state.transition_to_running();
    } else {
        // Treat as a new prompt
        if let Err(e) = handle.supervisor.write_stdin(input_text) {
            bot.send_message(msg.chat.id, format!("Error sending prompt: {}", e))
                .await?;
        }
    }

    Ok(())
}

fn get_adapter(name: &str, config: &AppConfig) -> Result<Arc<dyn CliAdapter + Send + Sync>> {
    match name {
        "claude" => {
            let bin = config
                .adapters
                .claude
                .as_ref()
                .map(|c| c.bin.clone())
                .unwrap_or_else(|| "claude".to_string());
            Ok(Arc::new(ClaudeCodeAdapter::new(bin)))
        }
        "gemini" => {
            let bin = config
                .adapters
                .gemini
                .as_ref()
                .map(|c| c.bin.clone())
                .unwrap_or_else(|| "gemini".to_string());
            Ok(Arc::new(GeminiAdapter::new(bin)))
        }
        "codex" => {
            let bin = config
                .adapters
                .codex
                .as_ref()
                .map(|c| c.bin.clone())
                .unwrap_or_else(|| "codex".to_string());
            Ok(Arc::new(CodexAdapter::new(bin)))
        }
        other => Err(anyhow::anyhow!("Unknown adapter: {}", other)),
    }
}

async fn output_listener(
    bot: Bot,
    chat_id: teloxide::types::ChatId,
    mut output_rx: broadcast::Receiver<String>,
) {
    let mut buffer = String::new();
    let mut typing_active = false;

    loop {
        match tokio::time::timeout(
            Duration::from_millis(1500),
            output_rx.recv(),
        )
        .await
        {
            Ok(Ok(line)) => {
                if !typing_active {
                    let _ = bot
                        .send_chat_action(chat_id, ChatAction::Typing)
                        .await;
                    typing_active = true;
                }
                let cleaned = formatter::clean_output(&line);
                if !cleaned.is_empty() {
                    buffer.push_str(&cleaned);
                    buffer.push('\n');
                }
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                if !buffer.is_empty() {
                    send_buffered(&bot, chat_id, &mut buffer).await;
                }
                break;
            }
            Err(_) => {
                if !buffer.is_empty() {
                    send_buffered(&bot, chat_id, &mut buffer).await;
                    typing_active = false;
                }
            }
        }
    }
}

async fn send_buffered(bot: &Bot, chat_id: teloxide::types::ChatId, buffer: &mut String) {
    let text = buffer.trim().to_string();
    buffer.clear();
    if text.is_empty() {
        return;
    }
    for chunk in formatter::chunk_message(&text) {
        if let Err(e) = bot.send_message(chat_id, chunk).await {
            error!("Failed to send message: {}", e);
        }
    }
}
