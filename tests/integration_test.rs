use mini_bot_rs::tools::Tool;
use mini_bot_rs::Config;

#[test]
fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.version, "1.0");
    assert_eq!(config.default_provider, "minimax");
}

#[test]
fn test_config_gateway_default() {
    let config = Config::default();
    assert_eq!(config.gateway.host, "127.0.0.1");
    assert_eq!(config.gateway.port, 3000);
}

#[test]
fn test_config_agent_default() {
    let config = Config::default();
    assert_eq!(config.agent.max_tool_iterations, 100);
    assert_eq!(config.agent.max_history_messages, 50);
    assert_eq!(config.agent.temperature, 0.7);
}

#[test]
fn test_config_security_default() {
    let config = Config::default();
    assert!(config.security.workspace_only);
    assert!(config.security.allowed_roots.is_empty());
}

#[tokio::test]
async fn test_shell_tool_available() {
    use mini_bot_rs::tools::ShellTool;

    let tool = ShellTool::new();
    let result = tool.execute(r#"{"command": "echo test"}"#).await;
    assert!(result.is_ok());
}

#[test]
fn test_tools_module_exports() {
    use mini_bot_rs::tools::{FileTool, ShellTool, Tool};

    let shell = ShellTool::new();
    assert_eq!(shell.name(), "shell");

    let file = FileTool::new();
    assert_eq!(file.name(), "file");
}
