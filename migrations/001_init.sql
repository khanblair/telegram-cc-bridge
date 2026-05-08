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
