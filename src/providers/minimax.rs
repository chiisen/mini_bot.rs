use super::traits::{Message, Provider, ProviderError, Response, ToolCall};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct MiniMaxRequest {
    model: String,
    messages: Vec<Message>,
    tools: Option<Vec<serde_json::Value>>,
    temperature: f64,
}

#[derive(Debug, Deserialize)]
struct MiniMaxResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(rename = "total_tokens")]
    total_tokens: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Vec<ToolCallDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct ToolCallDelta {
    id: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize, Default)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

pub struct MiniMaxProvider {
    client: Client,
    api_key: String,
    model: String,
    temperature: f64,
    base_url: String,
}

impl MiniMaxProvider {
    pub fn new(api_key: String, model: String, temperature: f64) -> Self {
        let base_url = "https://api.minimax.chat/v1".to_string();
        Self {
            client: Client::new(),
            api_key,
            model,
            temperature,
            base_url,
        }
    }
}

#[async_trait]
impl Provider for MiniMaxProvider {
    fn name(&self) -> &str {
        "minimax"
    }

    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<Response, ProviderError> {
        let request = MiniMaxRequest {
            model: self.model.clone(),
            messages,
            tools,
            temperature: self.temperature,
        };

        let url = format!("{}/text/chatcompletion_v2", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::RequestFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let minimax_response: MiniMaxResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::ParseFailed(e.to_string()))?;

        let choice = minimax_response
            .choices
            .first()
            .ok_or_else(|| ProviderError::ParseFailed("No choices in response".to_string()))?;

        let tool_calls: Vec<ToolCall> = choice
            .message
            .tool_calls
            .iter()
            .filter_map(|tc| {
                Some(ToolCall {
                    name: tc.function.as_ref()?.name.clone()?,
                    arguments: tc.function.as_ref()?.arguments.clone()?,
                })
            })
            .collect();

        Ok(Response {
            message: Message {
                role: choice.message.role.clone(),
                content: choice.message.content.clone(),
            },
            tool_calls,
        })
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        let request = MiniMaxRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "ping".to_string(),
            }],
            tools: None,
            temperature: 0.0,
        };

        let url = format!("{}/text/chatcompletion_v2", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::RequestFailed(format!(
                "Health check failed: {}",
                response.status()
            )))
        }
    }
}
