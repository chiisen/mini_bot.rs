use super::traits::{Tool, ToolArgument, ToolDefinition, ToolResult};
use async_trait::async_trait;
use std::time::Duration;
use tokio::process::Command as AsyncCommand;
use tokio::time::timeout;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct ShellTool {
    allowed_commands: Vec<String>,
    timeout_secs: u64,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            allowed_commands: vec![],
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    #[allow(dead_code)]
    pub fn with_allowed(commands: Vec<String>) -> Self {
        Self {
            allowed_commands: commands,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    pub fn with_config(commands: Vec<String>, timeout_secs: u64) -> Self {
        Self {
            allowed_commands: commands,
            timeout_secs,
        }
    }

    fn is_command_allowed(&self, cmd: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return false;
        }
        self.allowed_commands.iter().any(|allowed| cmd == allowed)
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".to_string(),
            description: "Execute system command".to_string(),
            arguments: vec![ToolArgument {
                name: "command".to_string(),
                arg_type: "string".to_string(),
                required: true,
                description: "Command to execute".to_string(),
            }],
        }
    }

    async fn execute(&self, arguments: &str) -> Result<ToolResult, String> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| format!("Failed to parse arguments: {}", e))?;

        let command = args["command"]
            .as_str()
            .ok_or("Missing 'command' parameter")?;

        if !self.is_command_allowed(command) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Command '{}' not in allowlist", command)),
            });
        }

        let timeout_duration = Duration::from_secs(self.timeout_secs);

        let result = timeout(timeout_duration, async {
            #[cfg(unix)]
            {
                AsyncCommand::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .await
            }

            #[cfg(windows)]
            {
                AsyncCommand::new("cmd")
                    .args(["/C", command])
                    .output()
                    .await
            }
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    Ok(ToolResult {
                        success: true,
                        output: stdout,
                        error: if stderr.is_empty() {
                            None
                        } else {
                            Some(stderr)
                        },
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: stdout,
                        error: Some(stderr),
                    })
                }
            }
            Ok(Err(e)) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Command execution failed: {}", e)),
            }),
            Err(_) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Command timed out after {} seconds",
                    self.timeout_secs
                )),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_tool_default() {
        let tool = ShellTool::new();
        assert_eq!(tool.name(), "shell");
    }

    #[test]
    fn test_shell_tool_definition() {
        let tool = ShellTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "shell");
        assert_eq!(def.arguments.len(), 1);
        assert_eq!(def.arguments[0].name, "command");
    }

    #[test]
    fn test_command_allowlist_empty() {
        let tool = ShellTool::new();
        assert!(!tool.is_command_allowed("ls"));
    }

    #[test]
    fn test_command_allowlist_restricted() {
        let tool = ShellTool::with_allowed(vec!["ls".to_string(), "echo".to_string()]);
        assert!(tool.is_command_allowed("ls"));
        assert!(tool.is_command_allowed("echo"));
        assert!(!tool.is_command_allowed("ls -la"));
        assert!(!tool.is_command_allowed("echo hello"));
        assert!(!tool.is_command_allowed("rm -rf /"));
    }

    #[tokio::test]
    async fn test_shell_execute_echo() {
        let tool = ShellTool::with_allowed(vec!["echo".to_string()]);
        let result = tool.execute(r#"{"command": "echo hello"}"#).await;
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_shell_execute_invalid_args() {
        let tool = ShellTool::new();
        let result = tool.execute("invalid json").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_execute_missing_command() {
        let tool = ShellTool::new();
        let result = tool.execute(r#"{"other": "value"}"#).await;
        assert!(result.is_err());
    }
}
