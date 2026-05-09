use super::CliAdapter;
use regex::Regex;
use std::path::Path;
use std::process::Command;

pub struct ClaudeCodeAdapter {
    bin: String,
    patterns: Vec<Regex>,
}

impl ClaudeCodeAdapter {
    pub fn new(bin: String) -> Self {
        let patterns = vec![
            // Bypass permissions confirmation
            Regex::new(r"Enter to confirm").unwrap(),
            Regex::new(r"Esc to cancel").unwrap(),
            // Trust prompt
            Regex::new(r"Yes, I trust this folder").unwrap(),
            Regex::new(r"trust this folder").unwrap(),
            // Yes/No prompts
            Regex::new(r"\(y/N\)").unwrap(),
            Regex::new(r"\(Y/n\)").unwrap(),
            Regex::new(r"\(y/n\)").unwrap(),
            // Choice prompts
            Regex::new(r"Enter your choice:").unwrap(),
            // Action prompts
            Regex::new(r"Overwrite\?").unwrap(),
            Regex::new(r"Continue\?").unwrap(),
            Regex::new(r"Confirm\?").unwrap(),
            Regex::new(r"Apply\?").unwrap(),
            Regex::new(r"Execute\?").unwrap(),
            Regex::new(r"Delete\?").unwrap(),
            Regex::new(r"Replace\?").unwrap(),
            Regex::new(r"Create\?").unwrap(),
            Regex::new(r"Edit\?").unwrap(),
            Regex::new(r"Run\?").unwrap(),
            // Numbered options
            Regex::new(r"^\s*\d+\.\s").unwrap(),
            Regex::new(r"\[\d+\]").unwrap(),
            // Claude Code prompt markers
            Regex::new(r"^\s*❯\s*").unwrap(),
            Regex::new(r"^\s*>\s*").unwrap(),
            // Permission prompts
            Regex::new(r"Allow\?").unwrap(),
            Regex::new(r"Permit\?").unwrap(),
            Regex::new(r"Skip permission").unwrap(),
        ];
        Self { bin, patterns }
    }
}

impl CliAdapter for ClaudeCodeAdapter {
    fn name(&self) -> &str {
        "claude"
    }

    fn spawn_cmd(&self, workdir: &Path) -> Command {
        let mut cmd = Command::new(&self.bin);
        cmd.current_dir(workdir);
        // Skip permissions to avoid interactive prompts
        cmd.arg("--dangerously-skip-permissions");
        cmd
    }

    fn input_prompt_patterns(&self) -> &[Regex] {
        &self.patterns
    }

    fn strip_output(&self, raw: &str) -> String {
        strip_ansi_escapes::strip_str(raw)
    }

    /// Auto-answer for known prompts
    fn auto_answer(&self, line: &str) -> Option<&str> {
        if line.contains("Enter to confirm") || line.contains("Esc to cancel") {
            Some("\n") // Press Enter to confirm
        } else if line.contains("Yes, I trust this folder") || line.contains("trust this folder") {
            Some("1\n") // Select option 1
        } else if line.contains("(y/N)") || line.contains("Overwrite?") {
            Some("y\n") // Yes
        } else {
            None
        }
    }
}
