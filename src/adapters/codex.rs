use super::CliAdapter;
use regex::Regex;
use std::path::Path;
use std::process::Command;

pub struct CodexAdapter {
    bin: String,
    patterns: Vec<Regex>,
}

impl CodexAdapter {
    pub fn new(bin: String) -> Self {
        let patterns = vec![];
        Self { bin, patterns }
    }
}

impl CliAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
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
