# MiniBot.rs

一個用 **100% Rust** 實作的高效能 AI Agent 執行框架。

## 特性

- **小執行環境**：目標是執行時期記憶體小於 5MB、冷啟動小於 10ms。2026-10-14 在這台 Apple Silicon Mac 上，以 release 建置量過，兩項記憶體都高於 5MB：
  - `mini_bot_rs version`：執行檔約 4.6MB。`/usr/bin/time -l` 的 maximum resident set size 約 7.2–7.5MB，peak memory footprint 約 2.4MB。第一次啟動約 300ms，之後 10 次的中位數約 3.9ms。
  - `gateway` 聽在 `127.0.0.1:18765`、使用預設設定：`ps` 的 RSS 在剛開始聽 port 時約 6.1MB（6288KB），對 `GET /health` 之後約 7.2MB（7408KB）。量測當下該請求回 HTTP 500，因為當時 `axum::serve` 沒有帶上 `ConnectInfo`。這個問題已修好，實際連線現在會把客戶端位址傳給認證中介層。
- **高度模組化**：透過 Trait 驅動的架構，所有元件皆可替換
- **跨平台**：支援 ARM、x86、RISC-V 架構
- **安全預設**：Shell 只執行白名單中的程式名稱，檔案工具預設限制在允許根目錄內

對話時間主要花在呼叫 MiniMax 的網路往返，換語言不會縮短這一段。Rust 在這裡的作用是交出單一執行檔、讓長駐的 Gateway 沒有垃圾回收停頓，以及用型別約束 Shell 與檔案路徑的邊界。

Release 建置使用 `opt-level = "z"`、全程式 LTO 與 `strip = true`，目的是縮小執行檔，不是把執行速度拉到最快。

## MVP 功能

### CLI 命令列介面

```bash
mini_bot.rs agent      # 啟動互動式對話
mini_bot.rs gateway   # 啟動 Webhook 閘道伺服器
mini_bot.rs config    # 查看合併後的有效設定，或寫出範例
```

### 核心工具集

- **Shell Tool** - 只執行白名單中的程式名稱與參數，不透過 `sh -c`。單引號與雙引號內的空白會保留。含有分號、管線、重新導向、命令替換或換行的命令會拒絕。`allowed_commands` 為空時拒絕所有命令。
- **File Tool** - 檔案讀寫。`workspace_only = true` 時路徑必須在允許根目錄內；未設定 `allowed_roots` 時使用目前工作目錄。`workspace_only = false` 時不限制根目錄。
- **Web Fetch Tool** - 只抓取 `http` 與 `https` 的文字內容。不允許網址內的帳密，也不允許 localhost、私有網段、link-local 與 loopback。轉址會逐一檢查，不會跟著進到這些位址。預設逾時 10 秒、回應上限 1MB、最多 3 次轉址。

### Gateway 與對話歷史

- 每個 `session_id` 使用各自的 Agent。對話寫入該設定目錄的 `memory.db`，同一個 session 重啟後可載回，並受歷史筆數上限限制。記憶體中的 session 超過 128 個時，清掉最久未使用的項目。
- 送往模型的歷史超過 `agent.max_history_chars`（預設 32000）時，從最舊的非 system 訊息開始移除，並保留最新的使用者訊息與其後尚未完成的工具結果。
- HTTP 逾時跟隨 `agent.max_execution_time_secs`（預設 300 秒）。逾時時取消該次 `chat`，同一個 session 不會留下仍在執行的請求。
- CORS 讀取 `gateway_security.allowed_origins`。清單含 `*` 或為空時允許任意來源，否則只允許清單中的來源。

### 支援的 AI 模型供應商

- MiniMax (預設)
- 支援擴充其他供應商

## 專案結構

```
mini_bot.rs/
├── Cargo.toml              # Rust 專案配置
├── src/
│   ├── main.rs             # 程式進入點
│   ├── lib.rs              # 模組匯出
│   ├── config/             # 配置管理
│   ├── agent/              # Agent 核心
│   ├── providers/          # AI 模型供應商
│   ├── tools/              # 工具集合
│   ├── gateway/            # Webhook 閘道
│   └── memory/             # 記憶體系統
└── README.md
```

## 建置需求

| 軟體 | 版本需求 |
|------|----------|
| Rust | 1.87+    |
| Cargo | 內建於 Rust |
| SQLite | 3 |

## 安裝

```bash
# 建置專案
cargo build --release
```

## 使用方式

```bash
# 互動式 Agent 對話
cargo run -- agent

# 單次訊息模式
cargo run -- agent --message "Hello"

# 啟動 Gateway
cargo run -- gateway --port 3000

# 顯示版本
cargo run -- version

# 查看有效設定（不印出 API key，只顯示來源）
cargo run -- config

# 把不含金鑰的範例寫到指定路徑
cargo run -- config example --output ./config.toml
```

## 設定檔

`config` 用來確認程式實際採用的設定，不會改動既有檔案。

- `config` 與 `config show` 讀取設定檔，再套用環境變數，印出 provider、Gateway、歷史上限、檔案與 Shell 限制。API key 只顯示來源：`MINIBOT_API_KEY`、`MINIMAX_API_KEY`、`MINIBOT_GATEWAY_API_KEY`、設定檔、設定檔（已加密）或未設定。`MINIBOT_API_KEY` 優先於 `MINIMAX_API_KEY`，兩者都優先於設定檔。
- `config example` 印出不含金鑰的範例。加上 `--output` 時寫入該路徑；檔案已存在則拒絕覆寫。
- 設定檔不存在時，`show` 顯示內建預設值。Gateway 安全設定的區段名稱是 `[gateway_security]`。

建立 `config.toml` 檔案並放置於：

- **Windows**: `%APPDATA%\com.minibot.mini_bot_rs\config.toml`
- **macOS/Linux**: `~/.config/com.minibot.mini_bot_rs/config.toml`

或使用 `--config-dir` 參數指定配置目錄。

### 範例 config.toml

```toml
version = "1.0"
default_provider = "minimax"
default_model = "minimax-coding-plan/MiniMax-M2.5"
api_key = "YOUR_MINIMAX_API_KEY"

[gateway]
host = "127.0.0.1"
port = 3000

[gateway_security]
api_key = ""
rate_limit_requests = 10
rate_limit_window_secs = 60
allowed_origins = ["*"]
allowed_ips = []

[agent]
max_tool_iterations = 100
max_history_messages = 50
max_history_chars = 32000
temperature = 0.7
max_execution_time_secs = 300

[security]
workspace_only = true
allowed_roots = []
allowed_commands = []
web_fetch_timeout_secs = 10
web_fetch_max_bytes = 1048576
web_fetch_max_redirects = 3
```

`max_history_chars` 是送往模型前的歷史字元上限。`max_execution_time_secs` 同時是 Agent 單次執行上限與 Gateway HTTP 逾時。`allowed_origins` 為 `["*"]` 或空清單時允許任意來源。`workspace_only = false` 時檔案工具不檢查根目錄。`allowed_commands` 只比對程式名稱，不接受路徑。`web_fetch_timeout_secs`、`web_fetch_max_bytes` 與 `web_fetch_max_redirects` 限制 Web Fetch 的時間、回應大小與轉址次數。

## 測試

```bash
# 執行所有單元測試
cargo test

# 執行特定模組測試
cargo test --lib
cargo test --test integration

# 執行涵蓋率測試（需要先安裝 cargo-llvm-cov）
cargo install cargo-llvm-cov
cargo llvm-cov --lcov --output-path lcov.info
cargo llvm-cov --text
```

### 涵蓋率報告

涵蓋率報告會顯示各模組的測試覆蓋程度：
- `src/config/` - 配置管理
- `src/agent/` - Agent 核心（含歷史管理）
- `src/providers/` - AI 供應商
- `src/tools/` - 工具集合（Shell、File、Web Fetch）

## 授權

MIT OR Apache-2.0

<!-- [😸SAM] -->