# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- API Key 環境變數支援 (`MINIBOT_API_KEY`)
- File Tool 檔案大小限制 (預設 10MB)
- Shell Tool 命令執行超時 (預設 30 秒)
- Shell Tool 預設拒絕所有命令 (安全強化)
- Agent 整合 Config 中的 security.allowed_commands

### Changed
- Agent 工具迭代次數限制（使用 `max_tool_iterations` 配置）
- `--config-dir` 會載入該目錄的 `config.toml`；檔案存在但解析失敗時直接報錯
- 工具定義在 Agent 建立時組一次；工具輸出寫入歷史前截斷為 8KB
- Webhook 在 Agent 失敗時回 HTTP 500，回應正文不含內部錯誤細節
- MiniMax HTTP client 設定連線逾時 10 秒、請求逾時 60 秒
- 互動模式改為非阻塞讀取標準輸入
- Gateway 速率限制會清掉已無請求紀錄的 IP

---

*本文件最後更新於 2026-03-04*
