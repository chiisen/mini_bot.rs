mod history;

use crate::config::Config;
use crate::i18n::{tr, tr_with_args};
use crate::memory::{Memory, MemoryEntry, SqliteMemory};
use crate::providers::create_provider;
use crate::providers::{Message, Provider, ToolCall};
use crate::tools::{FileTool, ShellTool, Tool, ToolResult, WebFetchTool};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_TOOL_OUTPUT_CHARS: usize = 8 * 1024;

pub struct Agent {
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    history: history::History,
    memory: Option<Arc<SqliteMemory>>,
    session: Option<BoundSession>,
    config: Config,
    tool_definitions: Vec<serde_json::Value>,
    start_time: Option<Instant>,
}

impl Agent {
    pub fn new(config: Config) -> Result<Self> {
        let provider = create_provider(
            &config.default_provider,
            config.get_api_key(),
            config.default_model.clone(),
            config.agent.temperature,
        )
        .map_err(anyhow::Error::msg)?;

        let file_root = if config.security.workspace_only {
            config.security.allowed_roots.first().cloned().or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|path| path.to_string_lossy().to_string())
            })
        } else {
            None
        };

        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(ShellTool::with_config(
                config.security.allowed_commands.clone(),
                30,
            )),
            Arc::new(FileTool::with_config(file_root, 10 * 1024 * 1024)),
            Arc::new(WebFetchTool::with_limits(
                config.security.web_fetch_timeout_secs,
                config.security.web_fetch_max_bytes,
                config.security.web_fetch_max_redirects,
            )),
        ];

        let tool_definitions = tools
            .iter()
            .map(|t| serde_json::to_value(t.definition()).unwrap_or(serde_json::Value::Null))
            .collect();

        Ok(Self {
            provider,
            tools,
            history: history::History::new(
                config.agent.max_history_messages,
                config.agent.max_history_chars,
            ),
            memory: None,
            session: None,
            config,
            tool_definitions,
            start_time: None,
        })
    }

    #[cfg(test)]
    pub fn set_provider(&mut self, provider: Arc<dyn Provider>) {
        self.provider = provider;
    }

    pub fn bind_session(
        &mut self,
        memory: Arc<SqliteMemory>,
        kind: SessionKind,
        session_id: String,
    ) {
        self.memory = Some(memory);
        self.session = Some(BoundSession { kind, id: session_id });
    }

    pub async fn load_history(&mut self) -> Result<()> {
        let (Some(memory), Some(session)) = (&self.memory, &self.session) else {
            return Ok(());
        };
        let id = session_memory_id(session.kind, &session.id).map_err(anyhow::Error::msg)?;
        let entry = if let Some(entry) = memory.get(&id).await.map_err(anyhow::Error::msg)? {
            Some(entry)
        } else if let Some(legacy) = legacy_session_memory_id(session.kind, &session.id) {
            memory.get(&legacy).await.map_err(anyhow::Error::msg)?
        } else {
            None
        };
        if let Some(entry) = entry {
            let messages: Vec<Message> = serde_json::from_str(&entry.content).unwrap_or_default();
            self.history.replace(messages);
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn push_history(&mut self, role: &str, content: &str) {
        self.history.add_message(Message {
            role: role.to_string(),
            content: content.to_string(),
        });
    }

    #[cfg(test)]
    pub fn history_snapshot(&self) -> Vec<Message> {
        self.history.messages().to_vec()
    }

    pub async fn save_history(&self) -> Result<()> {
        let (Some(memory), Some(session)) = (&self.memory, &self.session) else {
            return Ok(());
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        let id = session_memory_id(session.kind, &session.id).map_err(anyhow::Error::msg)?;
        let entry = MemoryEntry {
            id: id.clone(),
            category: "session".to_string(),
            key: session.id.clone(),
            content: serde_json::to_string(self.history.messages())?,
            created_at: now,
            updated_at: now,
        };
        memory.store(&entry).await.map_err(anyhow::Error::msg)?;
        if let Some(legacy) = legacy_session_memory_id(session.kind, &session.id) {
            if legacy != id {
                memory.delete(&legacy).await.map_err(anyhow::Error::msg)?;
            }
        }
        Ok(())
    }

    pub async fn chat(&mut self, user_input: &str) -> Result<String> {
        let max_time = Duration::from_secs(self.config.agent.max_execution_time_secs);
        self.start_time = Some(Instant::now());

        self.history.add_message(Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        });

        if let Some(start) = self.start_time {
            if start.elapsed() > max_time {
                return Err(anyhow::anyhow!(
                    "{}",
                    tr_with_args("agent.max_time_exceeded", &[&max_time.as_secs()])
                ));
            }
        }

        let max_iterations = self.config.agent.max_tool_iterations;
        let mut tool_iterations = 0;

        loop {
            if tool_iterations >= max_iterations {
                return Err(anyhow::anyhow!(
                    "{}",
                    tr_with_args("agent.max_iterations_exceeded", &[&max_iterations])
                ));
            }

            if let Some(start) = self.start_time {
                if start.elapsed() > max_time {
                    return Err(anyhow::anyhow!(
                        "{}",
                        tr_with_args("agent.max_time_exceeded", &[&max_time.as_secs()])
                    ));
                }
            }

            let response = self
                .provider
                .chat(
                    self.history.for_provider(),
                    Some(self.tool_definitions.clone()),
                )
                .await?;

            self.history.add_message(response.message.clone());

            if !response.tool_calls.is_empty() {
                tool_iterations += 1;

                for tool_call in &response.tool_calls {
                    let result = self
                        .execute_tool(tool_call)
                        .await
                        .map_err(anyhow::Error::msg)?;

                    let body = if result.success {
                        result.output
                    } else {
                        result.error.unwrap_or_default()
                    };
                    self.history.add_message(Message {
                        role: "tool".to_string(),
                        content: format!(
                            "Tool {} result: {}",
                            tool_call.name,
                            truncate_chars(&body, MAX_TOOL_OUTPUT_CHARS)
                        ),
                    });
                }
            } else {
                self.save_history().await?;
                return Ok(response.message.content);
            }
        }
    }

    async fn execute_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == tool_call.name)
            .ok_or_else(|| format!("Tool not found: {}", tool_call.name))?;

        tool.execute(&tool_call.arguments).await
    }
}

const MAX_SESSION_ID_LEN: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Cli,
    Gateway,
}

struct BoundSession {
    kind: SessionKind,
    id: String,
}

pub fn validate_session_id(session_id: &str) -> Result<(), String> {
    if session_id.is_empty() || session_id.len() > MAX_SESSION_ID_LEN {
        return Err("invalid session id".to_string());
    }
    if !session_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("invalid session id".to_string());
    }
    Ok(())
}

pub fn session_memory_id(kind: SessionKind, session_id: &str) -> Result<String, String> {
    validate_session_id(session_id)?;
    let prefix = match kind {
        SessionKind::Cli => "cli",
        SessionKind::Gateway => "gw",
    };
    Ok(format!("{prefix}_{session_id}"))
}

/// 舊版把合法 ID 存成 `sess_{id}`。`cli` 只屬於 CLI，Gateway 不讀那筆。
fn legacy_session_memory_id(kind: SessionKind, session_id: &str) -> Option<String> {
    match kind {
        SessionKind::Cli if session_id == "cli" => Some("sess_cli".to_string()),
        SessionKind::Gateway if session_id != "cli" => Some(format!("sess_{session_id}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Memory, MemoryEntry, SqliteMemory};
    use crate::providers::{ProviderError, Response, ToolCall};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct ScriptedProvider {
        step: AtomicUsize,
        delay: Duration,
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn name(&self) -> &str {
            "scripted"
        }

        async fn chat(
            &self,
            _messages: Vec<Message>,
            _tools: Option<Vec<serde_json::Value>>,
        ) -> Result<Response, ProviderError> {
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            let step = self.step.fetch_add(1, Ordering::SeqCst);
            if step == 0 {
                Ok(Response {
                    message: Message {
                        role: "assistant".to_string(),
                        content: "calling".to_string(),
                    },
                    tool_calls: vec![ToolCall {
                        name: "shell".to_string(),
                        arguments: r#"{"command":"echo hi"}"#.to_string(),
                    }],
                })
            } else {
                Ok(Response {
                    message: Message {
                        role: "assistant".to_string(),
                        content: "done".to_string(),
                    },
                    tool_calls: Vec::new(),
                })
            }
        }

        async fn health_check(&self) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn chat_runs_tool_then_returns_text_and_saves_history() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let mut agent = Agent::new(Config::default()).unwrap();
        agent.bind_session(Arc::clone(&memory), SessionKind::Gateway, "chat".to_string());
        agent.set_provider(Arc::new(ScriptedProvider {
            step: AtomicUsize::new(0),
            delay: Duration::ZERO,
        }));

        let reply = agent.chat("hello").await.unwrap();
        assert_eq!(reply, "done");
        let saved = agent.history_snapshot();
        assert!(saved.iter().any(|message| message.role == "tool"));
        assert!(saved.iter().any(|message| message.content == "done"));

        let mut restored = Agent::new(Config::default()).unwrap();
        restored.bind_session(memory, SessionKind::Gateway, "chat".to_string());
        restored.load_history().await.unwrap();
        let loaded = restored.history_snapshot();
        assert!(loaded.iter().any(|message| message.content == "done"));
        assert!(loaded.iter().any(|message| message.role == "tool"));
    }

    #[tokio::test]
    async fn chat_stops_at_tool_iteration_limit() {
        let mut config = Config::default();
        config.agent.max_tool_iterations = 1;
        let mut agent = Agent::new(config).unwrap();
        agent.set_provider(Arc::new(ScriptedProvider {
            step: AtomicUsize::new(0),
            delay: Duration::ZERO,
        }));

        let error = agent.chat("loop").await.unwrap_err();
        assert!(error.to_string().contains('1'));
    }

    #[tokio::test]
    async fn chat_stops_when_execution_time_is_exceeded() {
        let mut config = Config::default();
        config.agent.max_execution_time_secs = 1;
        config.agent.max_tool_iterations = 5;
        let mut agent = Agent::new(config).unwrap();
        agent.set_provider(Arc::new(ScriptedProvider {
            step: AtomicUsize::new(0),
            delay: Duration::from_millis(1100),
        }));

        let error = agent.chat("slow").await.unwrap_err();
        assert!(error.to_string().contains('1'));
    }

    async fn save_text(memory: &Arc<SqliteMemory>, kind: SessionKind, id: &str, text: &str) {
        let mut agent = Agent::new(Config::default()).unwrap();
        agent.bind_session(Arc::clone(memory), kind, id.to_string());
        agent.push_history("user", text);
        agent.save_history().await.unwrap();
    }

    #[tokio::test]
    async fn distinct_ids_do_not_share_storage() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let shared = "a".repeat(239);
        let left = format!("{shared}X");
        let right = format!("{shared}Y");
        save_text(&memory, SessionKind::Gateway, &left, "left").await;
        save_text(&memory, SessionKind::Gateway, &right, "right").await;

        let mut reloaded = Agent::new(Config::default()).unwrap();
        reloaded.bind_session(Arc::clone(&memory), SessionKind::Gateway, left);
        reloaded.load_history().await.unwrap();
        assert!(reloaded
            .history_snapshot()
            .iter()
            .any(|message| message.content == "left"));
        assert!(reloaded
            .history_snapshot()
            .iter()
            .all(|message| message.content != "right"));

        assert!(validate_session_id("a.b").is_err());
        assert!(validate_session_id("中文").is_err());
        assert!(validate_session_id(&"z".repeat(241)).is_err());
        assert!(memory.get("sess_ab").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn cli_and_gateway_keep_separate_records() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        save_text(&memory, SessionKind::Cli, "cli", "from-cli").await;
        save_text(&memory, SessionKind::Gateway, "cli", "from-gateway").await;

        let mut cli = Agent::new(Config::default()).unwrap();
        cli.bind_session(Arc::clone(&memory), SessionKind::Cli, "cli".to_string());
        cli.load_history().await.unwrap();
        assert!(cli
            .history_snapshot()
            .iter()
            .any(|message| message.content == "from-cli"));
        assert!(cli
            .history_snapshot()
            .iter()
            .all(|message| message.content != "from-gateway"));

        let mut gateway = Agent::new(Config::default()).unwrap();
        gateway.bind_session(memory, SessionKind::Gateway, "cli".to_string());
        gateway.load_history().await.unwrap();
        assert!(gateway
            .history_snapshot()
            .iter()
            .any(|message| message.content == "from-gateway"));
        assert!(gateway
            .history_snapshot()
            .iter()
            .all(|message| message.content != "from-cli"));
    }

    #[tokio::test]
    async fn legacy_session_keys_load_into_the_matching_namespace() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let now = 1_i64;
        for (id, content) in [("sess_alpha", "old-alpha"), ("sess_cli", "old-cli")] {
            memory
                .store(&MemoryEntry {
                    id: id.to_string(),
                    category: "session".to_string(),
                    key: id.to_string(),
                    content: serde_json::to_string(&vec![Message {
                        role: "user".to_string(),
                        content: content.to_string(),
                    }])
                    .unwrap(),
                    created_at: now,
                    updated_at: now,
                })
                .await
                .unwrap();
        }

        let mut gateway = Agent::new(Config::default()).unwrap();
        gateway.bind_session(Arc::clone(&memory), SessionKind::Gateway, "alpha".to_string());
        gateway.load_history().await.unwrap();
        assert!(gateway
            .history_snapshot()
            .iter()
            .any(|message| message.content == "old-alpha"));
        gateway.save_history().await.unwrap();
        assert!(memory.get("gw_alpha").await.unwrap().is_some());
        assert!(memory.get("sess_alpha").await.unwrap().is_none());

        let mut gateway_cli = Agent::new(Config::default()).unwrap();
        gateway_cli.bind_session(Arc::clone(&memory), SessionKind::Gateway, "cli".to_string());
        gateway_cli.load_history().await.unwrap();
        assert!(gateway_cli.history_snapshot().is_empty());

        let mut cli = Agent::new(Config::default()).unwrap();
        cli.bind_session(memory, SessionKind::Cli, "cli".to_string());
        cli.load_history().await.unwrap();
        assert!(cli
            .history_snapshot()
            .iter()
            .any(|message| message.content == "old-cli"));
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}\n...[truncated]")
}

pub async fn run(message: Option<String>, config_path: &PathBuf) -> Result<()> {
    let config = Config::load_or_default_from(config_path)?;

    let mut agent = Agent::new(config.clone())?;
    let memory_path = config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("memory.db");
    if let Some(parent) = memory_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(memory) = SqliteMemory::new(memory_path) {
        agent.bind_session(Arc::new(memory), SessionKind::Cli, "cli".to_string());
        agent.load_history().await?;
    }

    if let Some(msg) = message {
        let response = agent.chat(&msg).await?;
        println!("{}", response);
    } else {
        println!("{}", tr("agent.started"));

        use std::io::{self, Write};
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        loop {
            print!("> ");
            io::stdout().flush()?;

            let Some(line) = lines.next_line().await? else {
                break;
            };

            let input = line.trim();
            if input == "exit" {
                break;
            }

            match agent.chat(input).await {
                Ok(response) => println!("{}", response),
                Err(e) => eprintln!("{}", tr_with_args("error.prefix", &[&e])),
            }
        }
    }

    Ok(())
}
