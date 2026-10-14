mod history;

use crate::config::Config;
use crate::i18n::{tr, tr_with_args};
use crate::memory::{Memory, MemoryEntry, SqliteMemory};
use crate::providers::create_provider;
use crate::providers::{Message, Provider, ToolCall};
use crate::tools::{FileTool, ShellTool, Tool, ToolResult};
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
    session_id: Option<String>,
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
            session_id: None,
            config,
            tool_definitions,
            start_time: None,
        })
    }

    pub fn bind_session(&mut self, memory: Arc<SqliteMemory>, session_id: String) {
        self.memory = Some(memory);
        self.session_id = Some(session_id);
    }

    pub async fn load_history(&mut self) -> Result<()> {
        let (Some(memory), Some(session_id)) = (&self.memory, &self.session_id) else {
            return Ok(());
        };
        let id = session_memory_id(session_id);
        if let Some(entry) = memory.get(&id).await.map_err(anyhow::Error::msg)? {
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
        let (Some(memory), Some(session_id)) = (&self.memory, &self.session_id) else {
            return Ok(());
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        let entry = MemoryEntry {
            id: session_memory_id(session_id),
            category: "session".to_string(),
            key: session_id.clone(),
            content: serde_json::to_string(self.history.messages())?,
            created_at: now,
            updated_at: now,
        };
        memory.store(&entry).await.map_err(anyhow::Error::msg)
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

fn session_memory_id(session_id: &str) -> String {
    let safe: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(240)
        .collect();
    format!("sess_{safe}")
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
        agent.bind_session(Arc::new(memory), "cli".to_string());
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
