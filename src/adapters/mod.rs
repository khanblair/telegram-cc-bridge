use regex::Regex;
use std::path::Path;
use std::process::Command;

pub trait CliAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn spawn_cmd(&self, workdir: &Path) -> Command;
    fn input_prompt_patterns(&self) -> &[Regex];
    fn strip_output(&self, raw: &str) -> String;

    /// Optional: auto-answer for known prompts (e.g., trust dialogs)
    fn auto_answer(&self, _line: &str) -> Option<&str> {
        None
    }
}

pub mod claude_code;
pub mod gemini;
pub mod codex;

pub use self::claude_code::ClaudeCodeAdapter;
pub use self::codex::CodexAdapter;
pub use self::gemini::GeminiAdapter;
