use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub telegram: TelegramConfig,
    pub session: SessionConfig,
    pub adapters: AdaptersConfig,
    pub whisper: WhisperConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub whitelist: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionConfig {
    pub default_adapter: String,
    pub workdir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdaptersConfig {
    pub claude: Option<AdapterConfig>,
    pub gemini: Option<AdapterConfig>,
    pub codex: Option<AdapterConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdapterConfig {
    pub bin: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WhisperConfig {
    pub model: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::Environment::with_prefix("BRIDGE").separator("__"))
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
