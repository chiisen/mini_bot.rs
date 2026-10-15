use super::traits::{Memory, MemoryEntry};
use crate::config::crypto;
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct SqliteMemory {
    #[allow(dead_code)]
    conn: Arc<Mutex<Connection>>,
    encryption_key: Option<String>,
    #[cfg(test)]
    block_for: Arc<std::sync::Mutex<Duration>>,
}

impl SqliteMemory {
    #[allow(dead_code)]
    pub fn new(path: PathBuf) -> Result<Self, String> {
        Self::new_with_key(path, None)
    }

    #[allow(dead_code)]
    pub fn new_with_key(path: PathBuf, key: Option<String>) -> Result<Self, String> {
        let conn =
            Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                key TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("Failed to create table: {}", e))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category)",
            [],
        )
        .map_err(|e| format!("Failed to create index: {}", e))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key)",
            [],
        )
        .map_err(|e| format!("Failed to create index: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            encryption_key: key.or_else(crypto::get_encryption_key),
            #[cfg(test)]
            block_for: Arc::new(std::sync::Mutex::new(Duration::ZERO)),
        })
    }

    #[cfg(test)]
    pub fn set_blocking_delay(&self, delay: Duration) {
        *self.block_for.lock().expect("delay") = delay;
    }

    async fn with_connection<T, F>(&self, job: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, String> + Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        #[cfg(test)]
        let block_for = Arc::clone(&self.block_for);
        tokio::task::spawn_blocking(move || {
            #[cfg(test)]
            {
                let delay = *block_for.lock().expect("delay");
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
            }
            let conn = conn.lock();
            job(&conn)
        })
        .await
        .map_err(|error| format!("SQLite worker failed: {error}"))?
    }

    fn encrypt_content(&self, content: &str) -> String {
        if let Some(ref key) = self.encryption_key {
            if let Ok(encrypted) = crypto::encrypt(content, key) {
                return format!("ENC:{}", encrypted);
            }
        }
        content.to_string()
    }

    fn decrypt_content(&self, content: &str) -> String {
        if content.starts_with("ENC:") {
            let encrypted = content.trim_start_matches("ENC:");
            if let Some(ref key) = self.encryption_key {
                if let Ok(decrypted) = crypto::decrypt(encrypted, key) {
                    return decrypted;
                }
            }
        }
        content.to_string()
    }

    fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty() || id.len() > 255 {
            return Err("Invalid ID length".to_string());
        }
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Invalid ID characters".to_string());
        }
        Ok(())
    }
}

#[async_trait]
impl Memory for SqliteMemory {
    async fn store(&self, entry: &MemoryEntry) -> Result<(), String> {
        Self::validate_id(&entry.id)?;

        let encrypted_content = self.encrypt_content(&entry.content);
        let id = entry.id.clone();
        let category = entry.category.clone();
        let key = entry.key.clone();
        let created_at = entry.created_at;
        let updated_at = entry.updated_at;

        self.with_connection(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO memories (id, category, key, content, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (id, category, key, encrypted_content, created_at, updated_at),
            )
            .map(|_| ())
            .map_err(|e| format!("Failed to store memory: {}", e))
        })
        .await
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, String> {
        Self::validate_id(id)?;
        let id = id.to_string();
        let row = self
            .with_connection(move |conn| {
                conn.query_row(
                    "SELECT id, category, key, content, created_at, updated_at FROM memories WHERE id = ?1",
                    [id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                        ))
                    },
                )
                .map(Some)
                .or_else(|error| {
                    if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                        Ok(None)
                    } else {
                        Err(format!("Failed to query: {error}"))
                    }
                })
            })
            .await?;
        Ok(row.map(|row| MemoryEntry {
            id: row.0,
            category: row.1,
            key: row.2,
            content: self.decrypt_content(&row.3),
            created_at: row.4,
            updated_at: row.5,
        }))
    }

    async fn list_by_category(
        &self,
        category: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, String> {
        if category.is_empty() || category.len() > 255 {
            return Err("Invalid category length".to_string());
        }

        let category = category.to_string();
        let entries = self
            .with_connection(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, category, key, content, created_at, updated_at
                         FROM memories
                         WHERE category = ?1
                         ORDER BY updated_at DESC
                         LIMIT ?2",
                    )
                    .map_err(|e| format!("Failed to prepare statement: {}", e))?;
                let entries = stmt
                    .query_map(rusqlite::params![category, limit as i64], |row| {
                        Ok(MemoryEntry {
                            id: row.get(0)?,
                            category: row.get(1)?,
                            key: row.get(2)?,
                            content: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        })
                    })
                    .map_err(|e| format!("Failed to query: {}", e))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| format!("Failed to collect results: {}", e))?;
                Ok(entries)
            })
            .await?;

        Ok(entries
            .into_iter()
            .map(|mut entry| {
                entry.content = self.decrypt_content(&entry.content);
                entry
            })
            .collect())
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        Self::validate_id(id)?;
        let id = id.to_string();
        self.with_connection(move |conn| {
            conn.execute("DELETE FROM memories WHERE id = ?1", [id])
                .map(|_| ())
                .map_err(|e| format!("Failed to delete memory: {}", e))
        })
        .await
    }

    async fn clear_category(&self, category: &str) -> Result<(), String> {
        if category.is_empty() || category.len() > 255 {
            return Err("Invalid category length".to_string());
        }

        let category = category.to_string();
        self.with_connection(move |conn| {
            conn.execute("DELETE FROM memories WHERE category = ?1", [category])
                .map(|_| ())
                .map_err(|e| format!("Failed to clear category: {}", e))
        })
        .await
    }
}


