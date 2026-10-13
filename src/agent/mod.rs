mod history;

use crate::config::Config;
use crate::i18n::{tr, tr_with_args};
use crate::memory::SqliteMemory;
use crate::providers::create_provider;
use crate::providers::{Message, Provider, ToolCall};
use crate::tools::{FileTool, ShellTool, Tool, ToolResult};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_TOOL_OUTPUT_CHARS: usize = 8 * 1024;

pub struct Agent {
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    history: history::History,
    #[allow(dead_code)]
    memory: Option<SqliteMemory>,
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

        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(ShellTool::with_config(
                config.security.allowed_commands.clone(),
                30,
            )),
            Arc::new(FileTool::with_config(
                if config.security.workspace_only {
                    config.security.allowed_roots.first().cloned()
                } else {
                    None
                },
                10 * 1024 * 1024,
            )),
        ];

        let tool_definitions = tools
            .iter()
            .map(|t| serde_json::to_value(t.definition()).unwrap_or(serde_json::Value::Null))
            .collect();

        Ok(Self {
            provider,
            tools,
            history: history::History::new(config.agent.max_history_messages),
            memory: None,
            config,
            tool_definitions,
            start_time: None,
        })
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
                    self.history.messages().to_vec(),
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

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}\n...[truncated]")
}

pub async fn run(message: Option<String>, config_path: &PathBuf) -> Result<()> {
    let config = Config::load_or_default_from(config_path)?;

    let mut agent = Agent::new(config)?;

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
