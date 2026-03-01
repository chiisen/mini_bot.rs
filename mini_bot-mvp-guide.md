# MiniBot MVP 專案建置指南

## 📋 專案概述

### 什麼是 MiniBot？

MiniBot 是一個用 **100% Rust** 實作的高效能 AI Agent 執行框架。其設計目標是：

- **極低資源消耗**：< 5MB RAM 執行時期記憶體
- **快速啟動**：< 10ms 冷啟動時間
- **高度模組化**：透過 Trait 驅動的架構，所有元件皆可替換
- **跨平台**：支援 ARM、x86、RISC-V 架構
- **安全預設**：嚴格的沙箱隔離、明確的白名單

---

## 🎯 MVP 目標功能

本 MVP (Minimum Viable Product) 版本將實作以下核心功能：

### 1. 基礎 CLI 命令列介面
- `mini_bot.rs agent` - 啟動互動式對話
- `mini_bot.rs gateway` - 啟動 Webhook 閘道伺服器
- `mini_bot.rs daemon` - 啟動長期執行守護程序
- `mini_bot.rs config` - 配置管理

### 2. 單一 AI 模型供應商支援
- **MiniMax** 作為預設供應商（MiniMax M2.5 國際板）
- 簡化的 Provider Trait 實作
- 基本的對話歷史管理

### 3. 核心工具集
- **Shell Tool** - 執行系統命令
- **File Tool** - 檔案讀寫操作
- **Web Fetch Tool** - 擷取網頁內容

### 4. 基本記憶體系統
- SQLite 後端儲存對話歷史
- 簡化的 Memory Trait 實作

### 5. 最小安全機制
- 工作區域隔離（workspace-only 模式）
- 基本的命令白名單驗證

---

## 📁 MVP 專案目錄結構

```
mini_bot.rs/
├── Cargo.toml              # Rust 專案配置
├── rustfmt.toml            # 程式碼格式化配置
├── clippy.toml             # Linting 配置
├── src/
│   ├── main.rs             # 程式進入點
│   ├── lib.rs              # 模組匯出
│   ├── config/
│   │   ├── mod.rs          # 配置載入模組
│   │   └── schema.rs       # 配置結構定義
│   ├── agent/
│   │   ├── mod.rs          # Agent 核心模組
│   │   ├── loop.rs         # Agent 主迴圈
│   │   └── history.rs      # 對話歷史管理
│   ├── providers/
│   │   ├── mod.rs          # Provider 工廠
│   │   ├── traits.rs       # Provider Trait 定義
│   │   └── minimax.rs     # MiniMax 實作
│   ├── tools/
│   │   ├── mod.rs          # Tool 工廠
│   │   ├── traits.rs       # Tool Trait 定義
│   │   ├── shell.rs        # Shell 工具
│   │   └── file.rs         # 檔案工具
│   └── memory/
│       ├── mod.rs          # Memory 工廠
│       ├── traits.rs       # Memory Trait 定義
│       └── sqlite.rs       # SQLite 後端實作
└── README.md               # 專案說明文件
```

---

## 🔧 建置前置需求

### 軟體需求

| 軟體 | 版本需求 | 說明 |
|------|----------|------|
| **Rust** | 1.87+ | 編譯器 |
| **Cargo** | 內建於 Rust | 套件管理器 |
| **SQLite** | 3 | 資料庫（由 rusqlite 套件提供） |

### 安裝 Rust

```bash
# 方法 1：使用 rustup（推薦）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 方法 2：Windows 使用者
# 下載 rustup-init.exe 並執行
# https://rustup.rs

# 驗證安裝
rustc --version
cargo --version
```

---

## 📦 建立 MVP 專案步驟

### 步驟 1：建立新 Rust 專案

```bash
# 建立新專案目錄
mkdir mini_bot.rs
cd mini_bot.rs

# 初始化 Cargo 專案
cargo init --name mini_bot.rs --lib
```

### 步驟 2：設定 Cargo.toml

```toml
[package]
name = "mini_bot.rs"
version = "0.1.0"
edition = "2021"
authors = ["Your Name"]
license = "MIT OR Apache-2.0"
description = "MiniBot MVP - A minimal Rust AI Agent runtime"

# 最低 Rust 版本
rust-version = "1.87"

[dependencies]

# ============================================================
# CLI 命令列介面 - 使用 clap 框架
# ============================================================
clap = { version = "4.5", features = ["derive"] }

# ============================================================
# Async 執行期 - 使用 tokio
# 啟用功能：
# - rt-multi-thread: 多執行緒執行期
# - macros: 巨集支援 (#[tokio::main])
# - time: 時間相關功能
# - net: 網路功能
# - io-util: IO 工具
# - sync: 同步原語
# - process: 程序管理
# - io-std: 標準 IO
# - fs: 檔案系統
# - signal: 訊號處理
# ============================================================
tokio = { version = "1.42", default-features = false, features = ["rt-multi-thread", "macros", "time", "net", "io-util", "sync", "process", "io-std", "fs", "signal"] }

# ============================================================
# HTTP 用戶端 - 使用 reqwest
# 啟用功能：
# - json: JSON 支援
# - rustls-tls: Rust TLS 實作（無外部依賴）
# - blocking: 同步 HTTP 支援
# ============================================================
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }

# ============================================================
# 序列化/反序列化 - 使用 serde
# ============================================================
serde = { version = "1.0", default-features = false, features = ["derive"] }
serde_json = { version = "1.0", default-features = false, features = ["std"] }

# ============================================================
# 配置管理
# ============================================================
directories = "6.0"        # 取得系統目錄
toml = "1.0"               # TOML 解析

# ============================================================
# 日誌追蹤 - 使用 tracing
# ============================================================
tracing = { version = "0.1", default-features = false }
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "ansi", "env-filter", "chrono"] }

# ============================================================
# 錯誤處理
# ============================================================
anyhow = "1.0"             # 通用錯誤類型
thiserror = "2.0"          # 自訂錯誤類型

# ============================================================
# 記憶體/持久化 - 使用 SQLite
# rusqlite with "bundled" feature 會內建 SQLite
# ============================================================
rusqlite = { version = "0.37", features = ["bundled"] }

# ============================================================
# HTTP 伺服器 - 使用 axum
# ============================================================
axum = { version = "0.8", default-features = false, features = ["http1", "json", "tokio", "query", "ws", "macros"] }
tower = { version = "0.5", default-features = false }
tower-http = { version = "0.6", default-features = false, features = ["limit", "timeout"] }
http-body-util = "0.1"

# ============================================================
# 其他工具
# ============================================================
uuid = { version = "1.11", default-features = false, features = ["v4", "std"] }
async-trait = "0.1"        # 支援 async trait
parking_lot = "0.12"       # 高效 mutex

# ============================================================
# 發布配置 - 優化二進制大小
# ============================================================
[profile.release]
opt-level = "z"            # 優化大小
lto = "fat"                # 跨 crate 連結時優化
codegen-units = 1          # 序列化編碼以減少大小
strip = true               # 移除除錯符號
panic = "abort"            # 減少二進制大小
```

### 步驟 3：建立基礎模組結構

#### 3.1 建立 main.rs

```rust
//! MiniBot MVP - 程式入口點
//! 
//! 這是 MiniBot 應用程式的起點，負責：
//! 1. 解析命令列參數
//! 2. 初始化日誌系統
//! 3. 載入配置
//! 4. 路由到相應的子命令處理

// 引入所有必要的模組
mod agent;      // Agent 核心邏輯
mod config;     // 配置管理
mod providers; // AI 模型供應商
mod tools;      // 工具集合
mod memory;     // 記憶體系統

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

/// 命令列參數結構
#[derive(Parser, Debug)]
#[command(name = "mini_bot.rs")]
#[command(about = "MiniBot MVP - A minimal Rust AI Agent runtime", long_about = None)]
struct Cli {
    /// 指定配置目錄（可選）
    #[arg(long)]
    config_dir: Option<String>,

    /// 子命令
    #[command(subcommand)]
    command: Commands,
}

/// 可用的子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 啟動互動式 Agent 對話
    Agent {
        /// 單次訊息模式（不進入互動模式）
        #[arg(short, long)]
        message: Option<String>,
    },

    /// 啟動 Webhook Gateway 伺服器
    Gateway {
        /// 指定連接埠
        #[arg(short, long)]
        port: Option<u16>,

        /// 指定主機位址
        #[arg(long)]
        host: Option<String>,
    },

    /// 顯示版本資訊
    Version,
}

/// 主程式進入點
#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日誌系統
    // 使用環境變數 RUST_LOG 控制日誌級別
    // 預設為 info 級別
    let subscriber = fmt::Subscriber::builder()
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("設定日誌訂閱者失敗");

    // 解析命令列參數
    let cli = Cli::parse();

    // 處理子命令
    match cli.command {
        Commands::Agent { message } => {
            info!("啟動 MiniBot Agent...");
            agent::run(message).await?;
        }
        Commands::Gateway { port, host } => {
            let port = port.unwrap_or(3000);
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            info!("啟動 Gateway at {}:{}", host, port);
            gateway::run(&host, port).await?;
        }
        Commands::Version => {
            println!("MiniBot MVP v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
```

#### 3.2 建立 lib.rs

```rust
//! MiniBot MVP - 函式庫根模組
//! 
//! 本模組作為 MiniBot 的主要入口點，
//! 匯出所有公開的 API 供外部使用。

// 允許特定的 clippy 警告
#![warn(clippy::all, clippy::pedantic)]
// 禁止 unsafe code
#![forbid(unsafe_code)]

// 匯出配置模組
pub mod config;
// 匯出 Agent 模組
pub mod agent;
// 匯出 Providers 模組
pub mod providers;
// 匯出 Tools 模組
pub mod tools;
// 匯出 Memory 模組
pub mod memory;

// 重新匯出主要類型
pub use config::Config;
```

#### 3.3 建立 config/mod.rs（配置管理）

```rust
//! 配置管理模組
//! 
//! 負責載入、儲存和管理 MiniBot 的配置。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 主要的配置結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 配置版本
    pub version: String,
    
    /// 預設的 AI 模型供應商
    pub default_provider: String,
    
    /// 預設模型名稱
    pub default_model: String,
    
    /// API 金鑰
    pub api_key: String,
    
    /// Gateway 伺服器配置
    pub gateway: GatewayConfig,
    
    /// Agent 配置
    pub agent: AgentConfig,
    
    /// 安全性配置
    pub security: SecurityConfig,
}

/// Gateway 伺服器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// 監聽主機位址
    pub host: String,
    
    /// 監聽連接埠
    pub port: u16,
}

/// Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// 最大工具呼叫迭代次數
    pub max_tool_iterations: usize,
    
    /// 最大對話歷史訊息數
    pub max_history_messages: usize,
    
    /// 溫度參數（創造性程度）
    pub temperature: f64,
}

/// 安全性配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// 工作區域隔離模式
    pub workspace_only: bool,
    
    /// 允許的根目錄列表
    pub allowed_roots: Vec<String>,
    
    /// 允許的命令列表（白名單）
    pub allowed_commands: Vec<String>,
}

impl Default for Config {
    /// 預設配置
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            default_provider: "minimax".to_string(),
            default_model: "minimax-coding-plan/MiniMax-M2.5".to_string(),
            api_key: String::new(),
            gateway: GatewayConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            agent: AgentConfig {
                max_tool_iterations: 100,
                max_history_messages: 50,
                temperature: 0.7,
            },
            security: SecurityConfig {
                workspace_only: true,
                allowed_roots: vec![],
                allowed_commands: vec![],
            },
        }
    }
}

impl Config {
    /// 從檔案載入配置
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// 儲存配置到檔案
    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 取得配置目錄
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "minibot", "mini_bot_rs")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| {
                PathBuf::from(".")
            })
    }

    /// 取得預設配置路徑
    pub fn default_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }
}
```

### 步驟 4：建立 Agent 核心模組

#### 4.1 建立 providers/traits.rs（Provider Trait 定義）

```rust
//! AI 模型供應商 Trait 定義
//! 
//! 所有 AI 模型供應商都必須實作這個 Trait。
//! 這個設計允許輕鬆地替換不同的模型供應商。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 代表 AI 模型回應的結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 角色：user、assistant、system
    pub role: String,
    
    /// 訊息內容
    pub content: String,
}

/// 代表工具呼叫的結構
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具名稱
    pub name: String,
    
    /// 工具參數（JSON 格式）
    pub arguments: String,
}

/// 代表 AI 的完整回應
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// 回覆訊息
    pub message: Message,
    
    /// 工具呼叫列表（如果有的話）
    pub tool_calls: Vec<ToolCall>,
}

/// AI 模型供應商的錯誤類型
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// API 請求失敗
    #[error("API 請求失敗: {0}")]
    RequestFailed(String),
    
    /// API 回應解析失敗
    #[error("回應解析失敗: {0}")]
    ParseFailed(String),
    
    /// 認證失敗
    #[error("認證失敗: {0}")]
    AuthenticationFailed(String),
    
    /// 配額不足
    #[error("配額不足: {0}")]
    QuotaExceeded(String),
}

/// AI 模型供應商的 Trait
/// 
/// 所有的 AI 模型供應商（如 OpenAI、MiniMax、Anthropic）
/// 都必須實作這個 Trait。
#[async_trait]
pub trait Provider: Send + Sync {
    /// 取得供應商名稱
    fn name(&self) -> &str;

    /// 發送訊息並取得回應
    /// 
    /// # 參數
    /// - messages: 對話歷史
    /// - tools: 可用的工具列表（可選）
    /// 
    /// # 回傳
    /// - Ok(Response): AI 的回應
    /// - Err(ProviderError): 錯誤訊息
    async fn chat(
        &self, 
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<Response, ProviderError>;

    /// 檢查供應商是否可用
    async fn health_check(&self) -> Result<(), ProviderError>;
}
```

#### 4.2 建立 providers/mod.rs（Provider 工廠）

```rust
//! AI 模型供應商工廠模組
//! 
//! 負責建立和管理不同的 AI 模型供應商。

mod minimax;

pub use minimax::MiniMaxProvider;

use super::traits::Provider;
use std::sync::Arc;

/// 建立供應商實例
/// 
/// # 參數
/// - provider_type: 供應商類型 ("minimax", "openai", 等)
/// - api_key: API 金鑰
/// - model: 模型名稱
/// - temperature: 溫度參數
/// 
/// # 回傳
/// - Ok(Arc<dyn Provider>): 供應商實例
/// - Err(String): 錯誤訊息
pub fn create_provider(
    provider_type: &str,
    api_key: String,
    model: String,
    temperature: f64,
) -> Result<Arc<dyn Provider>, String> {
    match provider_type.to_lowercase().as_str() {
        "minimax" => Ok(Arc::new(MiniMaxProvider::new(api_key, model, temperature))),
        _ => Err(format!("不支援的供應商類型: {}", provider_type)),
    }
}
```

#### 4.3 建立 providers/minimax.rs（MiniMax 實作）

```rust
//! MiniMax AI 供應商實作
//! 
//! MiniMax 是一個中國 AI 模型供應商，提供 M2.5 等模型。
//! API 文件：https://platform.minimaxi.com/document/Gu...

use super::traits::{Message, Provider, ProviderError, Response, ToolCall};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// MiniMax API 的請求結構
#[derive(Debug, Serialize)]
struct MiniMaxRequest {
    /// 模型名稱
    model: String,
    
    /// 訊息列表
    messages: Vec<Message>,
    
    /// 工具定義（可選）
    tools: Option<Vec<serde_json::Value>>,
    
    /// 溫度參數
    temperature: f64,
}

/// MiniMax API 的回應結構
#[derive(Debug, Deserialize)]
struct MiniMaxResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

/// 回應中的選擇
#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

/// 使用量資訊
#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(rename = "total_tokens")]
    total_tokens: Option<i32>,
}

/// 回應訊息
#[derive(Debug, Deserialize)]
struct ResponseMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Vec<ToolCallDelta>,
}

/// 工具呼叫增量（用於追蹤）
#[derive(Debug, Deserialize, Default)]
struct ToolCallDelta {
    id: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
    function: Option<FunctionDelta>,
}

/// 函數呼叫增量
#[derive(Debug, Deserialize, Default)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

/// MiniMax 供應商實作
pub struct MiniMaxProvider {
    /// HTTP 用戶端
    client: Client,
    
    /// API 金鑰
    api_key: String,
    
    /// 模型名稱
    model: String,
    
    /// 預設溫度
    temperature: f64,
    
    /// API 端點（可根據模型調整）
    base_url: String,
}

impl MiniMaxProvider {
    /// 建立新的 MiniMax 供應商
    pub fn new(api_key: String, model: String, temperature: f64) -> Self {
        // MiniMax API 端點
        let base_url = "https://api.minimax.chat/v1".to_string();
        
        Self {
            client: Client::new(),
            api_key,
            model,
            temperature,
            base_url,
        }
    }
}

#[async_trait]
impl Provider for MiniMaxProvider {
    /// 取得供應商名稱
    fn name(&self) -> &str {
        "minimax"
    }

    /// 發送訊息並取得回應
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<Response, ProviderError> {
        // 建立請求
        let request = MiniMaxRequest {
            model: self.model.clone(),
            messages,
            tools,
            temperature: self.temperature,
        };

        // 發送請求到 MiniMax API
        let url = format!("{}/text/chatcompletion_v2", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        // 檢查 HTTP 狀態
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::RequestFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        // 解析回應
        let minimax_response: MiniMaxResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::ParseFailed(e.to_string()))?;

        // 取得第一個選擇
        let choice = minimax_response
            .choices
            .first()
            .ok_or_else(|| ProviderError::ParseFailed("No choices in response".to_string()))?;

        // 轉換工具呼叫
        let tool_calls: Vec<ToolCall> = choice
            .message
            .tool_calls
            .iter()
            .filter_map(|tc| {
                Some(ToolCall {
                    name: tc.function.as_ref()?.name.clone()?,
                    arguments: tc.function.as_ref()?.arguments.clone()?,
                })
            })
            .collect();

        Ok(Response {
            message: Message {
                role: choice.message.role.clone(),
                content: choice.message.content.clone(),
            },
            tool_calls,
        })
    }

    /// 健康檢查
    async fn health_check(&self) -> Result<(), ProviderError> {
        // 簡單的檢查：嘗試發送一個最小請求
        let request = MiniMaxRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "ping".to_string(),
            }],
            tools: None,
            temperature: 0.0,
        };

        let url = format!("{}/text/chatcompletion_v2", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::RequestFailed(format!(
                "Health check failed: {}",
                response.status()
            )))
        }
    }
}
```

### 步驟 5：建立 Tools 模組

#### 5.1 建立 tools/traits.rs（Tool Trait 定義）

```rust
//! 工具 Trait 定義
//! 
//! 所有工具都必須實作這個 Trait。
//! 工具是 Agent 與外部世界互動的介面。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 工具結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 是否成功
    pub success: bool,
    
    /// 輸出內容
    pub output: String,
    
    /// 錯誤訊息（如果有）
    pub error: Option<String>,
}

/// 工具參數
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolArgument {
    /// 參數名稱
    pub name: String,
    
    /// 參數類型
    pub arg_type: String,
    
    /// 是否必需
    pub required: bool,
    
    /// 描述
    pub description: String,
}

/// 工具定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名稱
    pub name: String,
    
    /// 工具描述
    pub description: String,
    
    /// 參數列表
    pub arguments: Vec<ToolArgument>,
}

/// 工具的 Trait
/// 
/// 所有工具（如 Shell、File、WebFetch）都必須實作這個 Trait。
#[async_trait]
pub trait Tool: Send + Sync {
    /// 取得工具名稱
    fn name(&self) -> &str;

    /// 取得工具定義（用於提供給 AI 模型）
    fn definition(&self) -> ToolDefinition;

    /// 執行工具
    /// 
    /// # 參數
    /// - arguments: 工具參數（JSON 格式的字串）
    /// 
    /// # 回傳
    /// - Ok(ToolResult): 執行結果
    /// - Err(String): 錯誤訊息
    async fn execute(&self, arguments: &str) -> Result<ToolResult, String>;
}
```

#### 5.2 建立 tools/shell.rs（Shell 工具）

```rust
//! Shell 工具實作
//! 
//! 這個工具允許 Agent 執行系統命令。
//! 注意：出於安全考量，應該限制可執行的命令。

use super::traits::{Tool, ToolArgument, ToolDefinition, ToolResult};
use async_trait::async_trait;
use std::process::Command;

/// Shell 工具
/// 
/// 允許執行系統命令的工具。
/// 在 production 環境中應該實施嚴格的白名單限制。
pub struct ShellTool {
    /// 允許的命令白名單（空表示允許所有）
    allowed_commands: Vec<String>,
}

impl ShellTool {
    /// 建立新的 Shell 工具
    pub fn new() -> Self {
        Self {
            allowed_commands: vec![],
        }
    }

    /// 建立帶有白名單的 Shell 工具
    pub fn with_allowed(commands: Vec<String>) -> Self {
        Self {
            allowed_commands: commands,
        }
    }

    /// 檢查命令是否允許執行
    fn is_command_allowed(&self, cmd: &str) -> bool {
        // 如果白名單為空，允許所有命令
        if self.allowed_commands.is_empty() {
            return true;
        }

        // 檢查命令是否在白名單中
        self.allowed_commands
            .iter()
            .any(|allowed| cmd.starts_with(allowed))
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    /// 工具名稱
    fn name(&self) -> &str {
        "shell"
    }

    /// 工具定義
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".to_string(),
            description: "執行系統命令".to_string(),
            arguments: vec![ToolArgument {
                name: "command".to_string(),
                arg_type: "string".to_string(),
                required: true,
                description: "要執行的命令".to_string(),
            }],
        }
    }

    /// 執行命令
    async fn execute(&self, arguments: &str) -> Result<ToolResult, String> {
        // 解析參數
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| format!("參數解析失敗: {}", e))?;

        let command = args["command"]
            .as_str()
            .ok_or("缺少 'command' 參數")?;

        // 安全檢查：檢查命令是否允許執行
        if !self.is_command_allowed(command) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("命令 '{}' 不在允許列表中", command)),
            });
        }

        // 在 Unix 系統上使用 sh -c，在 Windows 上使用 cmd /c
        #[cfg(unix)]
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output();

        #[cfg(windows)]
        let output = Command::new("cmd")
            .args(["/C", command])
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    Ok(ToolResult {
                        success: true,
                        output: stdout,
                        error: if stderr.is_empty() {
                            None
                        } else {
                            Some(stderr)
                        },
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: stdout,
                        error: Some(stderr),
                    })
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("命令執行失敗: {}", e)),
            }),
        }
    }
}
```

### 步驟 6：建立 Memory 模組

#### 6.1 建立 memory/sqlite.rs（SQLite 後端）

```rust
//! SQLite 記憶體後端實作
//! 
//! 使用 SQLite 儲存對話歷史和長期記憶。

use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;

/// 記憶體項目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 唯一識別碼
    pub id: String,
    
    /// 類別（如 "conversation", "system"）
    pub category: String,
    
    /// 鍵值
    pub key: String,
    
    /// 內容
    pub content: String,
    
    /// 建立時間戳
    pub created_at: i64,
    
    /// 更新時間戳
    pub updated_at: i64,
}

/// SQLite 記憶體儲存
pub struct SqliteMemory {
    /// 資料庫連線（使用 Arc 和 Mutex 確保執行緒安全）
    conn: Arc<Mutex<Connection>>,
}

impl SqliteMemory {
    /// 建立新的 SQLite 記憶體儲存
    pub fn new(path: PathBuf) -> SqliteResult<Self> {
        // 建立連線
        let conn = Connection::open(&path)?;

        // 初始化資料表
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
        )?;

        // 建立索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 儲存一個記憶體項目
    pub fn store(&self, entry: &MemoryEntry) -> SqliteResult<()> {
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
        )?;
        Ok(())
    }

    /// 取得一個記憶體項目
    pub fn get(&self, id: &str) -> SqliteResult<Option<MemoryEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, category, key, content, created_at, updated_at FROM memories WHERE id = ?1"
        )?;

        let mut rows = stmt.query([id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(MemoryEntry {
                id: row.get(0)?,
                category: row.get(1)?,
                key: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// 列出指定類别的記憶體
    pub fn list_by_category(&self, category: &str, limit: usize) -> SqliteResult<Vec<MemoryEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, category, key, content, created_at, updated_at 
             FROM memories 
             WHERE category = ?1 
             ORDER BY updated_at DESC 
             LIMIT ?2"
        )?;

        let entries = stmt
            .query_map([category, &limit.to_string()], |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    category: row.get(1)?,
                    key: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// 刪除一個記憶體項目
    pub fn delete(&self, id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM memories WHERE id = ?1", [id])?;
        Ok(())
    }

    /// 清除指定類别的所有記憶體
    pub fn clear_category(&self, category: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM memories WHERE category = ?1", [category])?;
        Ok(())
    }
}
```

### 步驟 7：建立 Agent 核心邏輯

#### 7.1 建立 agent/history.rs（對話歷史管理）

```rust
//! 對話歷史管理模組
//! 
//! 負責管理 Agent 的對話歷史紀錄。

use crate::providers::traits::Message;

/// 對話歷史
pub struct History {
    /// 訊息列表
    messages: Vec<Message>,
    
    /// 最大訊息數
    max_messages: usize,
}

impl History {
    /// 建立新的歷史管理器
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    /// 加入訊息
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        
        // 如果超過最大訊息數，移除最舊的訊息
        // 但保留第一個 system 訊息
        while self.messages.len() > self.max_messages {
            // 找到第一個非 system 訊息的位置
            if let Some(pos) = self.messages.iter().position(|m| m.role != "system") {
                if pos > 0 {
                    self.messages.remove(pos);
                }
            } else {
                break;
            }
        }
    }

    /// 取得所有訊息
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// 清空歷史
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
```

#### 7.2 建立 agent/mod.rs（Agent 核心）

```rust
//! Agent 核心模組
//! 
//! 負責管理 AI Agent 的主要邏輯流程：
//! 1. 維護對話歷史
//! 2. 與 AI 模型互動
//! 3. 執行工具並處理結果

mod history;

use crate::config::Config;
use crate::memory::sqlite::SqliteMemory;
use crate::providers::create_provider;
use crate::providers::traits::{Message, Provider, ToolCall};
use crate::tools::shell::ShellTool;
use crate::tools::traits::Tool;
use anyhow::Result;
use std::sync::Arc;

/// Agent 實例
pub struct Agent {
    /// AI 模型供應商
    provider: Arc<dyn Provider>,
    
    /// 可用工具
    tools: Vec<Arc<dyn Tool>>,
    
    /// 對話歷史
    history: history::History,
    
    /// 記憶體儲存
    memory: Option<SqliteMemory>,
    
    /// 配置
    config: Config,
}

impl Agent {
    /// 建立新的 Agent
    pub fn new(config: Config) -> Result<Self> {
        // 建立 Provider
        let provider = create_provider(
            &config.default_provider,
            config.api_key.clone(),
            config.default_model.clone(),
            config.agent.temperature,
        )?;

        // 建立工具列表
        let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(ShellTool::new())];

        // 建立記憶體儲存（可選）
        let memory = None; // 可以在這裡初始化 SQLite

        Ok(Self {
            provider,
            tools,
            history: history::History::new(config.agent.max_history_messages),
            memory,
            config,
        })
    }

    /// 執行單次對話
    pub async fn chat(&mut self, user_input: &str) -> Result<String> {
        // 將使用者訊息加入歷史
        self.history.add_message(Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        });

        // 準備工具定義
        let tool_definitions: Vec<serde_json::Value> = self
            .tools
            .iter()
            .map(|t| serde_json::to_value(t.definition()).unwrap())
            .collect();

        // 發送訊息給 AI
        let response = self
            .provider
            .chat(self.history.messages().to_vec(), Some(tool_definitions))
            .await?;

        // 將 AI 回應加入歷史
        self.history.add_message(response.message.clone());

        // 如果有工具呼叫，處理它們
        if !response.tool_calls.is_empty() {
            for tool_call in &response.tool_calls {
                let result = self.execute_tool(tool_call).await?;
                
                // 將工具結果加入歷史
                self.history.add_message(Message {
                    role: "tool".to_string(),
                    content: format!(
                        "Tool {} result: {}",
                        tool_call.name,
                        if result.success { result.output } else { result.error.unwrap_or_default() }
                    ),
                });
            }

            // 再次發送訊息給 AI（包含工具結果）
            let final_response = self
                .provider
                .chat(self.history.messages().to_vec(), None)
                .await?;

            return Ok(final_response.message.content);
        }

        Ok(response.message.content)
    }

    /// 執行工具
    async fn execute_tool(&self, tool_call: &ToolCall) -> Result<crate::tools::traits::ToolResult, String> {
        // 找到對應的工具
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == tool_call.name)
            .ok_or_else(|| format!("找不到工具: {}", tool_call.name))?;

        // 執行工具
        tool.execute(&tool_call.arguments).await
    }
}

/// 執行 Agent
pub async fn run(message: Option<String>) -> Result<()> {
    // 載入配置
    let config = load_config()?;

    // 建立 Agent
    let mut agent = Agent::new(config)?;

    if let Some(msg) = message {
        // 單次訊息模式
        let response = agent.chat(&msg).await?;
        println!("{}", response);
    } else {
        // 互動模式
        println!("MiniBot Agent 已啟動。輸入 'exit' 結束。");
        
        use std::io::{self, Write};
        loop {
            print!("> ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            let input = input.trim();
            if input == "exit" {
                break;
            }
            
            match agent.chat(input).await {
                Ok(response) => println!("{}", response),
                Err(e) => eprintln!("錯誤: {}", e),
            }
        }
    }

    Ok(())
}

/// 載入配置（簡化版本）
fn load_config() -> Result<Config> {
    let path = Config::default_path();
    
    if path.exists() {
        Config::load(&path).or_else(|_| Ok(Config::default()))
    } else {
        Ok(Config::default())
    }
}
```

---

## ✅ MVP 實現檢查清單

完成以下項目即完成 MVP：

- [ ] 建立 Cargo.toml 依賴配置
- [ ] 實作 CLI 框架（使用 clap）
- [ ] 實作配置管理系統
- [ ] 實作 MiniMax Provider
- [ ] 實作 Shell Tool
- [ ] 實作 File Tool
- [ ] 實作 SQLite Memory 後端
- [ ] 實作 Agent 主迴圈
- [ ] 實作 Gateway 伺服器（可選）
- [ ] 通過編譯和基本測試

---

## 📝 TODO List - 未來功能

以下是需要逐步添加的功能清單：

### 高優先順序

1. **更多的 AI Provider 支援**
   - [ ] Anthropic (Claude)
   - [ ] OpenAI
   - [ ] Google Gemini
   - [ ] Ollama (本地模型)

2. **更多的 Tools**
   - [ ] Web Fetch Tool
   - [ ] Web Search Tool
   - [ ] File Read/Write Tool
   - [ ] HTTP Request Tool

3. **Channels（通訊頻道）**
   - [ ] Telegram Channel
   - [ ] Discord Channel
   - [ ] Slack Channel

### 中優先順序

4. **安全性增強**
   - [ ] 工作區域隔離
   - [ ] 命令白名單
   - [ ] 敏感操作確認

5. **記憶體增強**
   - [ ] Markdown 記憶體後端
   - [ ] 向量搜尋

6. **長期執行**
   - [ ] Daemon 模式
   - [ ] Cron 排程

### 低優先順序

7. **硬體周邊支援**
   - [ ] STM32 Nucleo 支援
   - [ ] Raspberry Pi GPIO

8. **進階功能**
   - [ ] WASM 工具執行環境
   - [ ] MCP (Model Context Protocol) 支援
   - [ ] 多模態支援

---

## 🔗 相關資源

- [MiniMax API 文件](https://platform.minimaxi.com/)
- [Rust 官方文件](https://doc.rust-lang.org/)
- [Tokio 非同步執行期](https://tokio.rs)
- [Axum Web 框架](https://docs.rs/axum)
- [Serde 序列化](https://serde.rs/)
- [Clap CLI 框架](https://docs.rs/clap)

---

## 📄 授權

本專案使用 **MIT** 或 **Apache-2.0** 授權，詳見 LICENSE 文件。

---

*本文件最後更新於 2026-03-02*
