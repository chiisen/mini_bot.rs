mod history;

use crate::config::Config;
use crate::memory::SqliteMemory;
use crate::providers::create_provider;
use crate::providers::traits::{Message, Provider, ToolCall};
use crate::tools::{FileTool, ShellTool};
use crate::tools::traits::Tool;
use anyhow::Result;
use std::sync::Arc;

pub struct Agent {
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    history: history::History,
    memory: Option<SqliteMemory>,
    config: Config,
}

impl Agent {
    pub fn new(config: Config) -> Result<Self> {
        let provider = create_provider(
            &config.default_provider,
            config.api_key.clone(),
            config.default_model.clone(),
            config.agent.temperature,
        )?;

        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(ShellTool::new()),
            Arc::new(FileTool::new()),
        ];

        Ok(Self {
            provider,
            tools,
            history: history::History::new(config.agent.max_history_messages),
            memory: None,
            config,
        })
    }

    pub async fn chat(&mut self, user_input: &str) -> Result<String> {
        self.history.add_message(Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        });

        let tool_definitions: Vec<serde_json::Value> = self
            .tools
            .iter()
            .map(|t| serde_json::to_value(t.definition()).unwrap())
            .collect();

        let response = self
            .provider
            .chat(self.history.messages().to_vec(), Some(tool_definitions))
            .await?;

        self.history.add_message(response.message.clone());

        if !response.tool_calls.is_empty() {
            for tool_call in &response.tool_calls {
                let result = self.execute_tool(tool_call).await?;
                
                self.history.add_message(Message {
                    role: "tool".to_string(),
                    content: format!(
                        "Tool {} result: {}",
                        tool_call.name,
                        if result.success { result.output } else { result.error.unwrap_or_default() }
                    ),
                });
            }

            let final_response = self
                .provider
                .chat(self.history.messages().to_vec(), None)
                .await?;

            return Ok(final_response.message.content);
        }

        Ok(response.message.content)
    }

    async fn execute_tool(&self, tool_call: &ToolCall) -> Result<crate::tools::traits::ToolResult, String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == tool_call.name)
            .ok_or_else(|| format!("Tool not found: {}", tool_call.name))?;

        tool.execute(&tool_call.arguments).await
    }
}

pub async fn run(message: Option<String>) -> Result<()> {
    let config = load_config()?;

    let mut agent = Agent::new(config)?;

    if let Some(msg) = message {
        let response = agent.chat(&msg).await?;
        println!("{}", response);
    } else {
        println!("MiniBot Agent started. Type 'exit' to quit.");
        
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
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }

    Ok(())
}

fn load_config() -> Result<Config> {
    let path = Config::default_path();
    
    if path.exists() {
        Config::load(&path).or_else(|_| Ok(Config::default()))
    } else {
        Ok(Config::default())
    }
}
