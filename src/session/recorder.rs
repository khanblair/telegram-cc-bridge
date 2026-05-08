use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use anyhow::Result;

#[derive(Clone)]
pub struct Recorder {
    pool: SqlitePool,
}

impl Recorder {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_session(&self, chat_id: i64, adapter: &str) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO sessions (chat_id, adapter) VALUES (?1, ?2) RETURNING id"
        )
        .bind(chat_id)
        .bind(adapter)
        .fetch_one(&self.pool)
        .await?;

        let id: i64 = row.try_get("id")?;
        Ok(id)
    }

    pub async fn record_event(
        &self,
        session_id: i64,
        direction: &str,
        content: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO events (session_id, direction, content) VALUES (?1, ?2, ?3)"
        )
        .bind(session_id)
        .bind(direction)
        .bind(content)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_recent_events(
        &self,
        session_id: i64,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>> {
        let rows = sqlx::query(
            "SELECT ts, direction, content FROM events WHERE session_id = ?1 ORDER BY ts DESC LIMIT ?2"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            let ts: String = row.try_get("ts")?;
            let dir: String = row.try_get("direction")?;
            let content: String = row.try_get("content")?;
            events.push((ts, dir, content));
        }
        Ok(events)
    }
}
