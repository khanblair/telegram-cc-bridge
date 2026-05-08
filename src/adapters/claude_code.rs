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
            Regex::new(r"^>\s*").unwrap(),
            Regex::new(r"\(y/N\)") .unwrap(),
            Regex::new(r"\(Y/n\)").unwrap(),
            Regex::new(r"Enter your choice:").unwrap(),
            Regex::new(r"Overwrite\?").unwrap(),
            Regex::new(r"Continue\?").unwrap(),
            Regex::new(r"Confirm\?").unwrap(),
            Regex::new(r"\[\d+\]").unwrap(),
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
        cmd
    }

    fn input_prompt_patterns(&self) -> &[Regex] {
        &self.patterns
    }

    fn strip_output(&self, raw: &str) -> String {
        strip_ansi_escapes::strip_str(raw)
    }
}
