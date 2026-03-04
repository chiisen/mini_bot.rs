use super::traits::{Memory, MemoryEntry};
use crate::config::crypto;
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SqliteMemory {
    #[allow(dead_code)]
    conn: Arc<Mutex<Connection>>,
    encryption_key: Option<String>,
}

impl SqliteMemory {
    #[allow(dead_code)]
    pub fn new(path: PathBuf) -> Result<Self, String> {
        Self::new_with_key(path, None)
    }

    #[allow(dead_code)]
    pub fn new_with_key(path: PathBuf, key: Option<String>) -> Result<Self, String> {
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
            encryption_key: key.or_else(crypto::get_encryption_key),
        })
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

    fn sanitize_string(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
            .collect()
    }

    fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty() || id.len() > 255 {
            return Err("Invalid ID length".to_string());
        }
        if !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
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

        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO memories (id, category, key, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                &entry.id,
                &entry.category,
                &entry.key,
                &encrypted_content,
                entry.created_at,
                entry.updated_at,
            ),
        ).map_err(|e| format!("Failed to store memory: {}", e))?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, String> {
        Self::validate_id(id)?;

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, category, key, content, created_at, updated_at FROM memories WHERE id = ?1"
        ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let mut rows = stmt.query([id])
            .map_err(|e| format!("Failed to query: {}", e))?;

        if let Some(row) = rows.next().map_err(|e| format!("Failed to get row: {}", e))? {
            let content: String = row.get(3).map_err(|e| format!("Failed to get column: {}", e))?;
            let decrypted_content = self.decrypt_content(&content);
            
            Ok(Some(MemoryEntry {
                id: row.get(0).map_err(|e| format!("Failed to get column: {}", e))?,
                category: row.get(1).map_err(|e| format!("Failed to get column: {}", e))?,
                key: row.get(2).map_err(|e| format!("Failed to get column: {}", e))?,
                content: decrypted_content,
                created_at: row.get(4).map_err(|e| format!("Failed to get column: {}", e))?,
                updated_at: row.get(5).map_err(|e| format!("Failed to get column: {}", e))?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_by_category(&self, category: &str, limit: usize) -> Result<Vec<MemoryEntry>, String> {
        if category.is_empty() || category.len() > 255 {
            return Err("Invalid category length".to_string());
        }

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

        let decrypted_entries: Vec<MemoryEntry> = entries
            .into_iter()
            .map(|mut e| {
                e.content = self.decrypt_content(&e.content);
                e
            })
            .collect();

        Ok(decrypted_entries)
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        Self::validate_id(id)?;

        let conn = self.conn.lock();
        conn.execute("DELETE FROM memories WHERE id = ?1", [id])
            .map_err(|e| format!("Failed to delete memory: {}", e))?;
        Ok(())
    }

    async fn clear_category(&self, category: &str) -> Result<(), String> {
        if category.is_empty() || category.len() > 255 {
            return Err("Invalid category length".to_string());
        }

        let conn = self.conn.lock();
        conn.execute("DELETE FROM memories WHERE category = ?1", [category])
            .map_err(|e| format!("Failed to clear category: {}", e))?;
        Ok(())
    }
}
