# 程式碼自我升級功能評估

本文件只評估「程式能不能自己下載並換成新版本」。它不是設定系統的說明。改 `config.toml` 或環境變數之後，必須重新啟動 `agent` 或 `gateway`，執行中的行程不會自動載入新設定。

## 評估日期
2026-03-05。2026-10-14 更正「配置熱重載」的標題，避免把它讀成執行中會自動重載。

## 評估結論

**本專案目前不具備程式碼自我升級功能。** 沒有檢查新版本、下載執行檔或在執行中替換自己的程式。

---

## 評估範圍

### 1. 源代碼分析
搜索關鍵詞：
- `upgrade`, `self.update`, `auto.update`, `self.upgrade`
- `recompile`, `self.recompile`
- `self-update`, `selfupdate`
- `github release`, `latest version`
- `hot reload`, `watch`

**結果**: 無匹配內容

### 2. 依賴分析
檢查 `Cargo.toml`：
- 無 `self-update` 或類似 crate
- 無 GitHub API 客戶端依賴
- 無版本檢查相關依賴

### 3. 功能模組
| 模組 | 是否有升級功能 |
|------|---------------|
| Agent | ❌ 無 |
| Gateway | ❌ 無 |
| Config | ❌ 無 |
| Tools (Shell/File) | ❌ 無 |
| Memory | ❌ 無 |

---

## 現有類似功能

### 1. 設定變更需要重啟
- API key 可從環境變數讀取，優先於設定檔
- 沒有設定熱重載。改完設定檔或環境變數後，要重新執行命令才會生效

### 2. 依賴版本檢查
- 支援 `cargo audit` 檢查安全漏洞
- 需手動更新依賴

---

## 建議实现方案 (未來可選)

### 方案 A: GitHub Release (推薦)
使用 `reqwest` 檢查 GitHub Releases API，比較版本後下載新版本二進制檔案。

### 方案 B: Crates.io 版本檢查
使用 `crates.io` API 檢查最新版本。

### 方案 C: 自定義更新伺服器
架設更新伺服器發布更新資訊。

---

## 安全考量

若實現自我升級功能，需注意：
1. 驗證更新檔案簽名 (GPG/PGP)
2. 使用 HTTPS 傳輸
3. 實作回滾機制
4. 防止中間人攻擊 (MITM)

---

## 結論

MiniBot.rs 目前專注於作為輕量級 AI Agent 運行時，未包含自我升級機制。如有需要，可作為未來功能擴展。
