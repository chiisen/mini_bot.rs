//! Gateway request handlers

use crate::gateway::{GatewayState};
use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

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
    info!("Received webhook request: {:?}", payload.message);

    let session_id = payload.session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut agent = state.agent.lock().await;

    match agent.chat(&payload.message).await {
        Ok(response) => {
            info!("Agent response: {}", response);
            Json(WebhookResponse {
                response,
                session_id: Some(session_id),
            })
        }
        Err(e) => {
            error!("Agent error: {}", e);
            Json(WebhookResponse {
                response: format!("Error: {}", e),
                session_id: Some(session_id),
            })
        }
    }
}
