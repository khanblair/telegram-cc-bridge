# telegram-cc-bridge

A Rust-based bridge that lets you control Claude Code CLI (and other AI CLI tools) from Telegram. Send text, voice notes, and images from your phone — the bridge forwards them to the CLI, records everything, and alerts you when the CLI is waiting for your input.

---

## What it does

- **Send messages from Telegram** → forwarded as prompts to Claude Code running in a real terminal session
- **Voice notes** → transcribed via Whisper → sent as text to the CLI
- **Images** → downloaded and passed to the CLI as base64 attachments
- **Full session recording** → every byte in and out is logged to SQLite with timestamps
- **Input-pending detection** → when Claude Code asks you a question (confirmations, file edits, choices), you get an alert on Telegram and your reply goes straight to the CLI's stdin
- **Pluggable adapters** → swap in Gemini CLI or Codex CLI without touching the bot or session logic

---

## How it works

```
You (Telegram)
    │
    ▼
Bot handler          — authenticates your chat_id against a whitelist
    │
    ▼
Media pipeline       — transcribes voice, converts images to base64
    │
    ▼
Session manager      — one persistent PTY session per chat_id
    │
    ├──► PTY supervisor   — spawns `claude` in a real terminal (portable-pty)
    │         │
    │         ├── stdout → line-by-line reader
    │         │       ├── checks input_prompt_patterns()  ← per-adapter regex
    │         │       │       └── if match → state = WaitingForInput
    │         │       │                    → alert sent to Telegram
    │         │       └── all output → Recorder (SQLite)
    │         │
    │         └── stdin  ← your Telegram replies when state = WaitingForInput
    │
    └──► CLI adapter      — defines how to spawn the CLI and what its prompts look like
```

### The input-pending state machine

This is the most important part of the system. Each session tracks a state:

- **Idle** — session is open, no task running
- **Running** — CLI is actively doing work, output is streaming to you
- **WaitingForInput** — CLI has printed a prompt and is blocking on stdin
- **Stopped** — session was killed or crashed

Every line of stdout from the PTY is checked against the adapter's regex patterns. For Claude Code, these include patterns like `>`, `(y/N)`, `Enter your choice:`, `Overwrite?`, and similar interactive prompts. The moment one matches, Telegram gets an alert and your next message goes directly into the CLI's stdin rather than being treated as a new prompt.

### Session recording

Every event — both directions — is written to SQLite:

| Column | Description |
|---|---|
| `session_id` | links to the session row |
| `ts` | timestamp |
| `direction` | `in` (you → CLI) or `out` (CLI → you) |
| `content` | the raw text |

Use `/history` in Telegram to retrieve recent events from your current session.

---

## Tech stack

| Layer | Crate |
|---|---|
| Telegram bot | `grammers-rs` |
| Async runtime | `tokio` |
| PTY control | `portable-pty` |
| ANSI stripping | `strip-ansi-escapes` |
| Voice transcription | `whisper-rs` |
| Database | `sqlx` + SQLite |
| Config | `config` crate (TOML) |
| Serialization | `serde` + `serde_json` |
| Logging | `tracing` |

---

## Project structure

```
telegram-cc-bridge/
│
├── Cargo.toml
├── config/
│   └── default.toml            # bot token, whitelist, workdirs, adapter selection
│
├── migrations/
│   └── 001_init.sql            # sessions + events tables
│
└── src/
    ├── main.rs                 # tokio::main entry point, wires everything together
    │
    ├── bot/
    │   ├── mod.rs
    │   ├── handlers.rs         # on_message, on_voice, on_photo, on_command
    │   └── formatter.rs        # chunk long output to ≤4096 chars, wrap code blocks
    │
    ├── session/
    │   ├── mod.rs
    │   ├── manager.rs          # HashMap<chat_id, SessionHandle>, spawn/kill/lookup
    │   ├── state.rs            # SessionState enum + transition logic
    │   └── recorder.rs         # async sqlx writes for every PTY event
    │
    ├── pty/
    │   └── supervisor.rs       # portable-pty wrapper, stdout reader loop, stdin writer
    │
    ├── adapters/
    │   ├── mod.rs              # CliAdapter trait definition
    │   ├── claude_code.rs      # spawn cmd, input prompt regexes, output cleanup
    │   ├── gemini.rs           # stub — impl CliAdapter when ready
    │   └── codex.rs            # stub — impl CliAdapter when ready
    │
    └── media/
        ├── voice.rs            # download OGG → convert to WAV → Whisper API
        └── image.rs            # download file → base64 encode
```

### Key files explained

**`src/adapters/mod.rs`** — the trait every CLI adapter must implement:
```rust
pub trait CliAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn spawn_cmd(&self, workdir: &Path) -> Command;
    fn input_prompt_patterns(&self) -> &[Regex];
    fn strip_output(&self, raw: &str) -> String;
}
```
Adding Gemini CLI later means writing one new file that implements this trait. Nothing else changes.

**`src/session/state.rs`** — the state machine:
```rust
pub enum SessionState {
    Idle,
    Running,
    WaitingForInput { prompt_snapshot: String },
    Stopped,
}
```

**`src/pty/supervisor.rs`** — the PTY loop that drives everything:
```
spawn process via portable-pty
└── reader task: stdout → strip ANSI → check patterns → recorder → formatter → Telegram
└── writer task: tokio channel → PTY stdin (accepts input when WaitingForInput)
```

**`migrations/001_init.sql`**:
```sql
CREATE TABLE sessions (
    id         INTEGER PRIMARY KEY,
    chat_id    INTEGER NOT NULL,
    adapter    TEXT NOT NULL,
    started_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE events (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER REFERENCES sessions(id),
    ts         DATETIME DEFAULT CURRENT_TIMESTAMP,
    direction  TEXT CHECK(direction IN ('in', 'out')),
    content    TEXT NOT NULL
);
```

---

## Telegram commands

| Command | Description |
|---|---|
| `/start` | Open a new CLI session in the default workdir |
| `/stop` | Kill the current session |
| `/reset` | Kill and restart a fresh session |
| `/use claude` | Switch active adapter to Claude Code |
| `/use gemini` | Switch active adapter to Gemini CLI |
| `/history [n]` | Show last n events from the session log (default 20) |
| `/status` | Show current session state and active adapter |

---

## Configuration (`config/default.toml`)

```toml
[telegram]
bot_token = "YOUR_BOT_TOKEN"
whitelist = [123456789]        # allowed Telegram user IDs

[session]
default_adapter = "claude"
workdir = "/home/user/projects"

[adapters.claude]
bin = "claude"                 # path to claude binary

[adapters.gemini]
bin = "gemini"

[whisper]
model = "base.en"              # or use OpenAI API key instead
```

---

## Build order

1. Get `portable-pty` launching a plain `bash` session — verify stdin/stdout channels work
2. Implement `ClaudeCodeAdapter` with input prompt patterns — test with `cargo test`
3. Wire session manager + state machine + recorder — test with a CLI harness, no Telegram yet
4. Add Telegram bot layer — text messages only
5. Add input-pending detection and the alert → reply loop
6. Add voice note pipeline
7. Add image passthrough
8. Clean up the `CliAdapter` trait — now adding Gemini is just one new file