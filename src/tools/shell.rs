use super::traits::{Tool, ToolArgument, ToolDefinition, ToolResult};
use async_trait::async_trait;
use std::process::Command;

pub struct ShellTool {
    allowed_commands: Vec<String>,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            allowed_commands: vec![],
        }
    }

    pub fn with_allowed(commands: Vec<String>) -> Self {
        Self { allowed_commands: commands }
    }

    fn is_command_allowed(&self, cmd: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return true;
        }
        self.allowed_commands
            .iter()
            .any(|allowed| cmd.starts_with(allowed))
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

        #[cfg(unix)]
        let output = Command::new("sh").arg("-c").arg(command).output();

        #[cfg(windows)]
        let output = Command::new("cmd").args(["/C", command]).output();

        match output {
            Ok(output) => {
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
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Command execution failed: {}", e)),
            }),
        }
    }
}
