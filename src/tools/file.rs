use super::traits::{Tool, ToolArgument, ToolDefinition, ToolResult};
use async_trait::async_trait;
use std::path::Path;

pub struct FileTool {
    allowed_directory: Option<String>,
}

impl FileTool {
    pub fn new() -> Self {
        Self {
            allowed_directory: None,
        }
    }

    pub fn with_directory(dir: String) -> Self {
        Self {
            allowed_directory: Some(dir),
        }
    }

    fn is_path_allowed(&self, path: &str) -> bool {
        if let Some(ref allowed) = self.allowed_directory {
            let path = Path::new(path);
            if let Ok(canonical) = path.canonicalize() {
                if let Ok(allowed_canonical) = Path::new(allowed).canonicalize() {
                    return canonical.starts_with(allowed_canonical);
                }
            }
        }
        true
    }
}

impl Default for FileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileTool {
    fn name(&self) -> &str {
        "file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file".to_string(),
            description: "Read or write files".to_string(),
            arguments: vec![
                ToolArgument {
                    name: "operation".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: "Operation: read, write, or exists".to_string(),
                },
                ToolArgument {
                    name: "path".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: "File path".to_string(),
                },
                ToolArgument {
                    name: "content".to_string(),
                    arg_type: "string".to_string(),
                    required: false,
                    description: "Content to write (for write operation)".to_string(),
                },
            ],
        }
    }

    async fn execute(&self, arguments: &str) -> Result<ToolResult, String> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| format!("Failed to parse arguments: {}", e))?;

        let operation = args["operation"]
            .as_str()
            .ok_or("Missing 'operation' parameter")?;
        
        let path = args["path"]
            .as_str()
            .ok_or("Missing 'path' parameter")?;

        if !self.is_path_allowed(path) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Path not in allowed directory".to_string()),
            });
        }

        match operation {
            "read" => {
                match tokio::fs::read_to_string(path).await {
                    Ok(content) => Ok(ToolResult {
                        success: true,
                        output: content,
                        error: None,
                    }),
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to read file: {}", e)),
                    }),
                }
            }
            "write" => {
                let content = args["content"]
                    .as_str()
                    .ok_or("Missing 'content' parameter for write operation")?;
                
                match tokio::fs::write(path, content).await {
                    Ok(_) => Ok(ToolResult {
                        success: true,
                        output: "File written successfully".to_string(),
                        error: None,
                    }),
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to write file: {}", e)),
                    }),
                }
            }
            "exists" => {
                let exists = Path::new(path).exists();
                Ok(ToolResult {
                    success: true,
                    output: exists.to_string(),
                    error: None,
                })
            }
            _ => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unknown operation: {}", operation)),
            }),
        }
    }
}
