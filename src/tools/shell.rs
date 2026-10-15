use super::traits::{Tool, ToolArgument, ToolDefinition, ToolResult};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command as AsyncCommand};
use tokio::time::timeout;

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_SHELL_OUTPUT_BYTES: usize = 64 * 1024;

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

    fn is_command_allowed(&self, program: &str) -> bool {
        if self.allowed_commands.is_empty() || program.contains('/') || program.contains('\\') {
            return false;
        }
        self.allowed_commands
            .iter()
            .any(|allowed| program == allowed)
    }
}

fn parse_argv(command: &str) -> Result<Vec<String>, String> {
    if command.chars().any(|c| {
        matches!(
            c,
            ';' | '|' | '&' | '$' | '`' | '<' | '>' | '(' | ')' | '\n' | '\\'
        )
    }) {
        return Err("Shell metacharacters are not allowed".to_string());
    }

    let parts = split_argv(command)?;
    if parts.is_empty() {
        return Err("Missing command".to_string());
    }
    if parts[0].contains('/') || parts[0].contains('\\') {
        return Err("Command path is not allowed".to_string());
    }
    Ok(parts)
}

fn split_argv(command: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_token = false;
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {
                current.push(ch);
                in_token = true;
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                in_token = true;
            }
            None if ch.is_whitespace() => {
                if in_token {
                    parts.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            None => {
                current.push(ch);
                in_token = true;
            }
        }
    }

    if quote.is_some() {
        return Err("Unclosed quote".to_string());
    }
    if in_token {
        parts.push(current);
    }
    Ok(parts)
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
                description: "Program and arguments separated by spaces. The shell is not used."
                    .to_string(),
            }],
        }
    }

    async fn execute(&self, arguments: &str) -> Result<ToolResult, String> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| format!("Failed to parse arguments: {}", e))?;

        let command = args["command"]
            .as_str()
            .ok_or("Missing 'command' parameter")?;

        let argv = match parse_argv(command) {
            Ok(argv) => argv,
            Err(error) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(error),
                });
            }
        };

        if !self.is_command_allowed(&argv[0]) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Command '{}' not in allowlist", argv[0])),
            });
        }

        let timeout_duration = Duration::from_secs(self.timeout_secs);
        let program = argv[0].clone();
        let argv_rest = argv[1..].to_vec();

        let mut command = AsyncCommand::new(program);
        command
            .args(argv_rest)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // 子行程獨立 process group。回收只終止這個直接子行程；它再拉起的行程不在此列。
        #[cfg(unix)]
        command.process_group(0);

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Command execution failed: {error}")),
                });
            }
        };
        let mut child = ChildGuard { child: Some(child) };

        match timeout(
            timeout_duration,
            collect_output(child.child_mut(), MAX_SHELL_OUTPUT_BYTES),
        )
        .await
        {
            Ok(Ok(Collected::Finished {
                status,
                stdout,
                stderr,
            })) => {
                child.disarm();
                let stdout = String::from_utf8_lossy(&stdout).to_string();
                let stderr = String::from_utf8_lossy(&stderr).to_string();
                if status.success() {
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
            Ok(Ok(Collected::Capped { stdout })) => {
                child.disarm();
                Ok(ToolResult {
                    success: false,
                    output: String::from_utf8_lossy(&stdout).to_string(),
                    error: Some(format!(
                        "output exceeded {MAX_SHELL_OUTPUT_BYTES} bytes\n...[truncated]"
                    )),
                })
            }
            Ok(Err(error)) => {
                child.kill_and_wait().await;
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Command execution failed: {error}")),
                })
            }
            Err(_) => {
                child.kill_and_wait().await;
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "Command timed out after {} seconds",
                        self.timeout_secs
                    )),
                })
            }
        }
    }
}

enum Collected {
    Finished {
        status: std::process::ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    Capped {
        stdout: Vec<u8>,
    },
}

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child still running")
    }

    fn disarm(&mut self) {
        self.child = None;
    }

    async fn kill_and_wait(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
}

async fn collect_output(child: &mut Child, limit: usize) -> std::io::Result<Collected> {
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("stdout missing"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("stderr missing"))?;
    let mut out_buf = Vec::new();
    let mut err_buf = Vec::new();
    let mut left = limit;
    let mut out_open = true;
    let mut err_open = true;
    let mut out_chunk = [0_u8; 8192];
    let mut err_chunk = [0_u8; 8192];

    while out_open || err_open {
        if left == 0 {
            let _ = child.kill().await;
            return Ok(Collected::Capped { stdout: out_buf });
        }
        tokio::select! {
            biased;
            read = read_chunk(&mut stdout, &mut out_chunk), if out_open => {
                match read {
                    Ok(0) => out_open = false,
                    Ok(n) => {
                        let take = n.min(left);
                        out_buf.extend_from_slice(&out_chunk[..take]);
                        left -= take;
                        if take < n {
                            let _ = child.kill().await;
                            return Ok(Collected::Capped { stdout: out_buf });
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
            read = read_chunk(&mut stderr, &mut err_chunk), if err_open => {
                match read {
                    Ok(0) => err_open = false,
                    Ok(n) => {
                        let take = n.min(left);
                        err_buf.extend_from_slice(&err_chunk[..take]);
                        left -= take;
                        if take < n {
                            let _ = child.kill().await;
                            return Ok(Collected::Capped { stdout: out_buf });
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }

    let status = child.wait().await?;
    Ok(Collected::Finished {
        status,
        stdout: out_buf,
        stderr: err_buf,
    })
}

async fn read_chunk<R: AsyncRead + Unpin>(reader: &mut R, chunk: &mut [u8]) -> std::io::Result<usize> {
    reader.read(chunk).await
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
        assert!(!tool.is_command_allowed("rm"));
        assert!(!tool.is_command_allowed("/bin/ls"));
    }

    #[tokio::test]
    async fn test_shell_rejects_metacharacters() {
        let tool = ShellTool::with_allowed(vec!["echo".to_string()]);
        let result = tool
            .execute(r#"{"command": "echo hello; rm -rf /"}"#)
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_parse_argv_keeps_quoted_spaces() {
        let argv = parse_argv(r#"echo "a b""#).unwrap();
        assert_eq!(argv, vec!["echo", "a b"]);
        let argv = parse_argv("echo 'a b'").unwrap();
        assert_eq!(argv, vec!["echo", "a b"]);
    }

    #[tokio::test]
    async fn test_shell_echo_keeps_quoted_spaces() {
        let tool = ShellTool::with_allowed(vec!["echo".to_string()]);
        let result = tool
            .execute(r#"{"command": "echo \"a b\""}"#)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.output, "a b\n");
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

    fn perl_tool(timeout_secs: u64) -> ShellTool {
        ShellTool::with_config(vec!["perl".to_string()], timeout_secs)
    }

    fn write_script(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    fn process_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn shell_caps_stdout_and_kills_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("pid");
        let script = write_script(
            dir.path(),
            "out.pl",
            r#"open my $f, ">", $ARGV[0]; print $f $$; close $f; print "x" x 200000; sleep 30;"#,
        );
        let tool = perl_tool(10);
        let command = format!("perl {} {}", script.display(), pid_path.display());
        let started = std::time::Instant::now();
        let result = tool
            .execute(&serde_json::json!({ "command": command }).to_string())
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("output exceeded"));
        assert!(result.output.len() <= MAX_SHELL_OUTPUT_BYTES);
        assert!(started.elapsed() < Duration::from_secs(5));
        let pid: u32 = std::fs::read_to_string(&pid_path).unwrap().trim().parse().unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!process_alive(pid));
    }

    #[tokio::test]
    async fn shell_caps_stderr_and_kills_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("pid");
        let script = write_script(
            dir.path(),
            "err.pl",
            r#"open my $f, ">", $ARGV[0]; print $f $$; close $f; print STDERR "y" x 200000; sleep 30;"#,
        );
        let tool = perl_tool(10);
        let command = format!("perl {} {}", script.display(), pid_path.display());
        let result = tool
            .execute(&serde_json::json!({ "command": command }).to_string())
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("output exceeded"));
        let pid: u32 = std::fs::read_to_string(&pid_path).unwrap().trim().parse().unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!process_alive(pid));
    }

    #[tokio::test]
    async fn shell_timeout_kills_silent_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("pid");
        let script = write_script(
            dir.path(),
            "sleep.pl",
            r#"open my $f, ">", $ARGV[0]; print $f $$; close $f; sleep 30;"#,
        );
        let tool = perl_tool(1);
        let command = format!("perl {} {}", script.display(), pid_path.display());
        let result = tool
            .execute(&serde_json::json!({ "command": command }).to_string())
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("timed out"));
        let pid: u32 = std::fs::read_to_string(&pid_path).unwrap().trim().parse().unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!process_alive(pid));
    }

    #[tokio::test]
    async fn shell_cancel_kills_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("pid");
        let script = write_script(
            dir.path(),
            "sleep.pl",
            r#"open my $f, ">", $ARGV[0]; print $f $$; close $f; sleep 60;"#,
        );
        let tool = perl_tool(30);
        let command = format!("perl {} {}", script.display(), pid_path.display());
        let args = serde_json::json!({ "command": command }).to_string();
        let handle = tokio::spawn(async move { tool.execute(&args).await });
        let pid = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(text) = std::fs::read_to_string(&pid_path) {
                    if let Ok(pid) = text.trim().parse::<u32>() {
                        return pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("pid file");
        handle.abort();
        let _ = handle.await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!process_alive(pid));
    }
}
