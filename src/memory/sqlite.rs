use super::traits::{Memory, MemoryEntry};
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SqliteMemory {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteMemory {
    pub fn new(path: PathBuf) -> Result<Self, String> {
        let conn = Connection::open(&path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

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
        ).map_err(|e| format!("Failed to create table: {}", e))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category)",
            [],
        ).map_err(|e| format!("Failed to create index: {}", e))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key)",
            [],
        ).map_err(|e| format!("Failed to create index: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl Memory for SqliteMemory {
    async fn store(&self, entry: &MemoryEntry) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO memories (id, category, key, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                &entry.id,
                &entry.category,
                &entry.key,
                &entry.content,
                entry.created_at,
                entry.updated_at,
            ),
        ).map_err(|e| format!("Failed to store memory: {}", e))?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, category, key, content, created_at, updated_at FROM memories WHERE id = ?1"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let mut rows = stmt.query([id])
            .map_err(|e| format!("Failed to query: {}", e))?;

        if let Some(row) = rows.next().map_err(|e| format!("Failed to get row: {}", e))? {
            Ok(Some(MemoryEntry {
                id: row.get(0).map_err(|e| format!("Failed to get column: {}", e))?,
                category: row.get(1).map_err(|e| format!("Failed to get column: {}", e))?,
                key: row.get(2).map_err(|e| format!("Failed to get column: {}", e))?,
                content: row.get(3).map_err(|e| format!("Failed to get column: {}", e))?,
                created_at: row.get(4).map_err(|e| format!("Failed to get column: {}", e))?,
                updated_at: row.get(5).map_err(|e| format!("Failed to get column: {}", e))?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_by_category(&self, category: &str, limit: usize) -> Result<Vec<MemoryEntry>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, category, key, content, created_at, updated_at 
             FROM memories 
             WHERE category = ?1 
             ORDER BY updated_at DESC 
             LIMIT ?2"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let entries = stmt.query_map([category, &limit.to_string()], |row| {
            Ok(MemoryEntry {
                id: row.get(0)?,
                category: row.get(1)?,
                key: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        }).map_err(|e| format!("Failed to query: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect results: {}", e))?;

        Ok(entries)
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM memories WHERE id = ?1", [id])
            .map_err(|e| format!("Failed to delete memory: {}", e))?;
        Ok(())
    }

    async fn clear_category(&self, category: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM memories WHERE category = ?1", [category])
            .map_err(|e| format!("Failed to clear category: {}", e))?;
        Ok(())
    }
}
