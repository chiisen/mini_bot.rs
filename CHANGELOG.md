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

---

*本文件最後更新於 2026-03-04*
