use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MemoryEntry {
    pub id: String,
    pub category: String,
    pub key: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[async_trait]
#[allow(dead_code)]
pub trait Memory: Send + Sync {
    async fn store(&self, entry: &MemoryEntry) -> Result<(), String>;
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, String>;
    async fn list_by_category(
        &self,
        category: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, String>;
    async fn delete(&self, id: &str) -> Result<(), String>;
    async fn clear_category(&self, category: &str) -> Result<(), String>;
}
