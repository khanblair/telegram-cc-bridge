/// Telegram message limit is 4096 characters.
const MAX_LEN: usize = 4096;

/// Split a long message into chunks that fit within Telegram's limit,
/// trying to preserve code blocks when possible.
pub fn chunk_message(text: &str) -> Vec<String> {
    if text.len() <= MAX_LEN {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if line.len() > MAX_LEN {
            if !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
            let mut remaining = line;
            while remaining.len() > MAX_LEN {
                let (chunk, rest) = remaining.split_at(MAX_LEN);
                chunks.push(chunk.to_string());
                remaining = rest;
            }
            if !remaining.is_empty() {
                current.push_str(remaining);
                current.push('\n');
            }
        } else if current.len() + line.len() + 1 > MAX_LEN {
            chunks.push(current.clone());
            current.clear();
            current.push_str(line);
            current.push('\n');
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// A simple terminal screen buffer that interprets carriage returns
/// and ANSI escape sequences to extract visible text.
pub struct TerminalScreen {
    lines: Vec<String>,
    current_line: String,
}

impl TerminalScreen {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: String::new(),
        }
    }

    /// Process raw PTY output and extract visible text changes.
    /// Returns any completed lines that should be sent.
    pub fn process(&mut self, raw: &str) -> Vec<String> {
        let mut completed = Vec::new();
        let mut chars = raw.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Skip ANSI escape sequence
                if chars.peek() == Some(&'[') {
                    chars.next(); // skip '['
                    // Skip until we hit a letter (the command character)
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_alphabetic() {
                            chars.next();
                            break;
                        }
                        chars.next();
                    }
                }
            } else if ch == '\r' {
                // Carriage return: overwrite current line
                self.current_line.clear();
            } else if ch == '\n' {
                // Newline: finalize current line
                let line = std::mem::take(&mut self.current_line);
                let cleaned = clean_line(&line);
                if !cleaned.is_empty() {
                    completed.push(cleaned);
                }
            } else {
                self.current_line.push(ch);
            }
        }

        completed
    }

    /// Flush any remaining content in the current line.
    pub fn flush(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.current_line);
        let cleaned = clean_line(&line);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }
}

/// Clean a single line: replace box-drawing with spaces, collapse multiple spaces.
fn clean_line(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut prev_was_space = true; // skip leading spaces

    for ch in line.chars() {
        if is_box_drawing(ch) {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    // Trim trailing space
    result.trim_end().to_string()
}

fn is_box_drawing(c: char) -> bool {
    matches!(
        c,
        '─' | '│'
            | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼'
            | '╭' | '╮' | '╯' | '╰' | '╴' | '╶' | '╸' | '╹'
            | '▀' | '▄' | '█' | '▌' | '▐' | '░' | '▒' | '▓'
            | '▝' | '▗' | '▘' | '▙' | '▚' | '▛' | '▜' | '▞' | '▟'
            | '◜' | '◝' | '◞' | '◟' | '◢' | '◣' | '◤' | '◥'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_screen_basic() {
        let mut screen = TerminalScreen::new();
        let lines = screen.process("Hello world\n");
        assert_eq!(lines, vec!["Hello world"]);
    }

    #[test]
    fn test_terminal_screen_carriage_return() {
        let mut screen = TerminalScreen::new();
        let lines = screen.process("Loading\rDone!\n");
        assert_eq!(lines, vec!["Done!"]);
    }

    #[test]
    fn test_terminal_screen_box_drawing() {
        let mut screen = TerminalScreen::new();
        let lines = screen.process("│ctrl+g│to│edit│in│Vim│\n");
        assert_eq!(lines, vec!["ctrl+g to edit in Vim"]);
    }

    #[test]
    fn test_terminal_screen_collapse_spaces() {
        let mut screen = TerminalScreen::new();
        let lines = screen.process("Hello    world\n");
        assert_eq!(lines, vec!["Hello world"]);
    }
}
