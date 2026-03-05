//! Gateway request handlers

use crate::gateway::GatewayState;
use crate::i18n::tr_with_args;
use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

fn sanitize_for_log(s: &str) -> String {
    let sensitive_keys = ["api_key", "password", "token", "secret", "credential"];
    let mut result = s.to_string();

    for key in sensitive_keys {
        let pattern = format!(r#""{}"\s*:\s*"[^"]*"#, key);
        if let Ok(re) = regex::Regex::new(&pattern) {
            result = re
                .replace_all(&result, format!(r#""{}":"***"#, key))
                .to_string();
        }
    }

    result
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookRequest {
    pub message: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub response: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn root_handler() -> Json<&'static str> {
    Json("MiniBot Gateway v0.1.0")
}

pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn webhook_handler(
    State(state): State<GatewayState>,
    Json(payload): Json<WebhookRequest>,
) -> Json<WebhookResponse> {
    let sanitized_message = sanitize_for_log(&payload.message);
    info!(
        "{}",
        tr_with_args("gateway.webhook_received", &[&sanitized_message])
    );

    let session_id = payload
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut agent = state.agent.lock().await;

    match agent.chat(&payload.message).await {
        Ok(response) => {
            let sanitized_response = sanitize_for_log(&response);
            info!(
                "{}",
                tr_with_args("gateway.agent_response", &[&sanitized_response])
            );
            Json(WebhookResponse {
                response,
                session_id: Some(session_id),
            })
        }
        Err(e) => {
            error!("{}", tr_with_args("gateway.agent_error", &[&e.to_string()]));
            Json(WebhookResponse {
                response: tr_with_args("error.prefix", &[&e.to_string()]),
                session_id: Some(session_id),
            })
        }
    }
}
