use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub message: Message,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("API request failed: {0}")]
    RequestFailed(String),
    #[error("Response parse failed: {0}")]
    ParseFailed(String),
    #[error("Authentication failed: {0}")]
    #[allow(dead_code)]
    AuthenticationFailed(String),
    #[error("Quota exceeded: {0}")]
    #[allow(dead_code)]
    QuotaExceeded(String),
}

#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    #[allow(dead_code)]
    fn name(&self) -> &str;

    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<Response, ProviderError>;

    #[allow(dead_code)]
    async fn health_check(&self) -> Result<(), ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let message = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "Hello");
    }

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall {
            name: "shell".to_string(),
            arguments: r#"{"command": "ls"}"#.to_string(),
        };
        assert_eq!(tool_call.name, "shell");
    }

    #[test]
    fn test_response_creation() {
        let response = Response {
            message: Message {
                role: "assistant".to_string(),
                content: "Response content".to_string(),
            },
            tool_calls: vec![],
        };
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_provider_error_display() {
        let error = ProviderError::RequestFailed("connection timeout".to_string());
        assert!(error.to_string().contains("API request failed"));

        let error = ProviderError::ParseFailed("invalid json".to_string());
        assert!(error.to_string().contains("Response parse failed"));

        let error = ProviderError::AuthenticationFailed("invalid key".to_string());
        assert!(error.to_string().contains("Authentication failed"));

        let error = ProviderError::QuotaExceeded("limit reached".to_string());
        assert!(error.to_string().contains("Quota exceeded"));
    }
}
