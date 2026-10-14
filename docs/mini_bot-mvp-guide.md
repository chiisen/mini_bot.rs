# MiniBot MVP 專案建置指南

## 📋 專案概述

### 什麼是 MiniBot？

MiniBot 是一個用 **100% Rust** 實作的高效能 AI Agent 執行框架。其設計目標是：

- **小執行環境**：目標是執行時期記憶體小於 5MB、冷啟動小於 10ms。2026-10-14 以 release 的 `version` 量測，常駐記憶體約 7.5MB，未達到 5MB；暖機後啟動中位數約 3.9ms。詳見 README。
- **高度模組化**：透過 Trait 驅動的架構，所有元件皆可替換
- **跨平台**：支援 ARM、x86、RISC-V 架構
- **安全預設**：嚴格的沙箱隔離、明確的白名單

---

## 🎯 MVP 目標功能

本 MVP (Minimum Viable Product) 版本將實作以下核心功能：

### 1. 基礎 CLI 命令列介面
- `mini_bot.rs agent` - 啟動互動式對話
- `mini_bot.rs gateway` - 啟動 Webhook 閘道伺服器
- `mini_bot.rs config` - 查看有效設定，或寫出不含金鑰的範例

### 2. 單一 AI 模型供應商支援
- **MiniMax** 作為預設供應商（MiniMax M2.5 國際板）
- 簡化的 Provider Trait 實作
- 基本的對話歷史管理

### 3. 核心工具集
- **Shell Tool** - 只執行白名單中的程式名稱，不透過 shell
- **File Tool** - 檔案讀寫。預設限制在允許根目錄內
- **Web Fetch Tool** - 只抓取 http 與 https，並阻擋私有網段

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
├── Cargo.toml
├── README.md
├── config.example.toml
└── src/
    ├── main.rs
    ├── lib.rs
    ├── agent/          # history.rs、mod.rs
    ├── config/         # mod.rs、crypto.rs
    ├── gateway/        # mod.rs、handlers.rs
    ├── i18n/
    ├── memory/         # traits.rs、sqlite.rs
    ├── providers/      # traits.rs、minimax.rs
    └── tools/          # shell.rs、file.rs、web_fetch.rs
```

沒有 `config/schema.rs`、`agent/loop.rs`。Agent 主迴圈在 `src/agent/mod.rs`。使用方式以 README 為準。

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

## 程式在哪裡

這個專案已經建好。不要依早期草稿另起一個程式。

- 命令、設定與工具行為看 README 與 `config.example.toml`。
- 實作看 `src/`。本文件不再內嵌可複製的程式，避免和現況分叉。
- 測試用 `cargo test`。

## ✅ MVP 實現檢查清單

完成以下項目即完成 MVP：

- [x] 建立 Cargo.toml 依賴配置
- [x] 實作 CLI 框架（使用 clap）
- [x] 實作配置管理系統
- [x] 實作 MiniMax Provider
- [x] 實作 Shell Tool
- [x] 實作 File Tool
- [x] 實作 SQLite Memory 後端
- [x] 實作 Agent 主迴圈
- [x] 實作 Gateway 伺服器
- [x] 實作 Web Fetch
- [x] 實作 config 子命令
- [x] 通過編譯和基本測試

---

## 未來項目判斷

MVP 檢查清單都已完成。下面這些不是待辦，是對舊清單的取捨。目前產品是本機 MiniMax Agent，加上受限制的 Shell、檔案、Web Fetch，以及 HTTP webhook。

### 不需要為了這個產品做

- **Anthropic、OpenAI、Gemini、Ollama**：Provider trait 已能再加實作，但預設供應商就是 MiniMax。沒有第二個供應商的需求前不要做。
- **HTTP Request 工具**：和 Web Fetch 重疊。不受限的 HTTP 會繞過現在的私有網段檢查。
- **Telegram、Discord、Slack**：Gateway webhook 已是對外入口。各平台頻道是另一個產品。
- **Markdown 記憶體、向量搜尋**：對話歷史已存在 SQLite。沒有檢索需求前不要加第二套記憶體。
- **Daemon、Cron**：長駐用 `gateway`。沒有排程需求前不要加新命令。
- **STM32、Raspberry Pi GPIO、WASM 工具環境、多模態**：和這個 Agent 執行環境無關。

### 可以之後再做，現在沒有排程

- **敏感操作確認**：Shell 與檔案寫入目前沒有人工確認。這是安全缺口，但要先定哪些操作要停下來問。
- **Web Search**：和 Web Fetch 不同，需要搜尋供應商與 API key。沒有選定供應商前不做。
- **MCP**：只有在要接外部工具協定時才做。

### 已開 issue、尚未做

- [Gateway 常駐記憶體尚未量測](https://github.com/chiisen/mini_bot.rs/issues/14)。小於 5MB 的目標只量過 `version`。
- [`SqliteMemory::sanitize_string` 沒有人呼叫](https://github.com/chiisen/mini_bot.rs/issues/15)。`validate_id` 已檢查 id。

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

*本文件最後更新於 2026-10-14*
