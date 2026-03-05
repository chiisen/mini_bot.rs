# Security Upgrade TODO - MiniBot.rs

> **狀態**: ✅ 所有安全修復已完成 (2026-03-04)

本文件規劃專案的安全性升級任務，所有 AI Agent 都應依此執行。

## 專案安全問題分析

### 🔴 高風險 (High Risk) - ✅ 已完成

#### 1. Shell Tool 命令注入漏洞
**位置**: `src/tools/shell.rs`

**問題**:
- 當 `allowed_commands` 為空時，預設允許所有命令 (第 23-25 行)
- allowlist 檢查僅使用 `starts_with()`，可被繞過 (第 28 行)
- 無命令執行超時或資源限制

**修復方向**:
- [x] 預設拒絕所有命令，而非允許
- [x] 實作更嚴格的命令驗證 (完整匹配)
- [x] 添加命令執行超時 (30 秒)
- [x] 整合 Config 中的 `security.allowed_commands`

**變更**:
- 新增 `ShellTool::with_config()` 方法接受命令列表和超時時間
- 空命令列表時預設返回 `false` (拒絕所有)
- 使用完整匹配而非 `starts_with()`
- 使用 `tokio::time::timeout` 實現 30 秒超時

---

#### 2. File Tool 路徑穿越漏洞
**位置**: `src/tools/file.rs`

**問題**:
- 當 `allowed_directory` 為 None 時，允許所有路徑 (第 24-34 行)
- `canonicalize()` 可被 symlink 繞過
- 無檔案大小限制

**修復方向**:
- [x] 預設設定工作目錄限制
- [x] 添加檔案大小上限 (10MB)
- [x] 整合 Config 中的 `security.workspace_only` 和 `security.allowed_roots`
- [x] 實作目錄越界檢查 (檢查 `..` 和 symlink)

**變更**:
- 新增 `MAX_FILE_SIZE` 常量 (10MB)
- 新增 `FileTool::with_config()` 方法
- 空 `allowed_directory` 時預設返回 `false` (拒絕所有)
- 新增 `..` 路徑成分檢查
- 新增檔案大小檢查

---

#### 3. Gateway 無認證保護
**位置**: `src/gateway/mod.rs`

**問題**:
- `/webhook` 端點無任何認證機制
- 無速率限制
- 無 CORS 配置

**修復方向**:
- [x] 添加 API Key 認證 (X-API-Key header)
- [x] 實作 CORS 設定
- [ ] 實作速率限制 (tower-http rate limit)
- [ ] 添加 IP 白名單功能

**變更**:
- 新增 `GatewaySecurityConfig` 配置結構
- 新增 `auth_middleware` 中間件驗證 API Key
- 使用 `tower-http` 的 `CorsLayer`
- 支援從環境變數 `MINIBOT_GATEWAY_API_KEY` 讀取

---

### 🟠 中風險 (Medium Risk) - ✅ 已完成

#### 4. API Key 明文儲存
**位置**: `src/config/mod.rs`, `src/providers/minimax.rs`

**問題**:
- API Key 以明文存儲在 config.toml
- 無環境變數支援

**修復方向**:
- [x] 優先從環境變數讀取 API Key
- [ ] 支援加密的 config 檔案
- [x] 添加 API Key 存在性檢查

**變更**:
- 新增 `get_api_key()` 方法，優先讀取環境變數
- 支援環境變數: `MINIMAX_API_KEY`, `MINIBOT_API_KEY`
- 在 `Config::load()` 中覆蓋 config 檔案的值

---

#### 5. Agent 工具迭代無限制
**位置**: `src/agent/mod.rs`

**問題**:
- Config 定義了 `max_tool_iterations: 100`，但未實際使用
- 無法防止無限工具呼叫迴圈

**修復方向**:
- [x] 在 `execute_tool` 前檢查迭代次數
- [ ] 添加最大執行時間限制

**變更**:
- 新增 `tool_iterations` 計數器欄位
- 在每次工具呼叫前檢查是否達到上限
- 達到上限時返回錯誤

---

#### 6. Memory 儲存無加密
**位置**: `src/memory/sqlite.rs`

**問題**:
- SQLite 資料庫無加密
- Session ID 未經 sanitization

**修復方向**:
- [x] 添加 SQL injection 防護 (使用參數化查詢 + 輸入驗證)
- [ ] 考慮添加 SQLite 加密 (sqlcipher) 或記錄加密

**變更**:
- 使用參數化查詢 (已實現)
- 新增 `validate_id()` 驗證 ID 格式
- 新增長度檢查

---

### 🟡 低風險 (Low Risk) - ✅ 已完成

#### 7. 依賴版本檢查
**位置**: `Cargo.toml`

**問題**:
- 需要確認所有依賴版本的安全性

**修復方向**:
- [x] 執行 `cargo audit` 檢查已知漏洞
- [x] 更新到安全的版本

**變更**:
- 新增 `regex` 依賴用於日誌 sanitization

---

#### 8. 日誌敏感資訊
**問題**:
- 可能會記錄 API Key 或敏感資料

**修復方向**:
- [x] 實作日誌 sanitization
- [x] 添加日誌級別過濾

**變更**:
- 新增 `sanitize_for_log()` 函數過濾敏感關鍵字
- 過濾關鍵字: `api_key`, `password`, `token`, `secret`, `credential`

---

## 剩餘待辦事項

所有安全修復已完成！

---

## 已完成的安全增強功能

### Agent 最大執行時間限制
- 新增 `max_execution_time_secs` 配置項 (預設 300 秒)
- 使用 `std::time::Instant` 追蹤執行時間
- 在工具呼叫前檢查是否超時

### 速率限制 (Rate Limiting)
- 使用記憶體雜湊表實現基於 IP 的速率限制
- 可配置請求數上限和時間窗口
- 返回 HTTP 429 Too Many Requests

### IP 白名單
- 在 `GatewaySecurityConfig` 中新增 `allowed_ips` 配置
- 在認證中介軟體中檢查客戶端 IP
- 返回 HTTP 403 Forbidden

### 加密 Config 檔案
- 新增 `src/config/crypto.rs` 加密模組
- 使用 AES-256-GCM 加密
- 環境變數 `MINIBOT_CONFIG_KEY` 提供加密金鑰
- 支援 `ENC:` 前綴標記加密字段

### SQLite 資料庫加密
- 在 `SqliteMemory` 中新增應用層加密
- 自動加密寫入的內容
- 自動解密讀取的內容
- 使用與 config 相同的加密金鑰

---

## 驗證方式

完成每個任務後，執行以下測試:

```bash
# 依賴安全檢查
cargo audit

# 單元測試
cargo test

# 整合測試
cargo test --test integration
```

---

## 參考資源

- [Rust 安全指南](https://anssi-fr.github.io/rust-secure-coding/)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [tower-http 安全 Middleware](https://docs.rs/tower-http/latest/tower_http/)

---

*本文件最後更新於 2026-03-04*
