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
        // If a single line exceeds MAX_LEN, split it by character boundary
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
