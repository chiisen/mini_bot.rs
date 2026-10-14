//! Gateway request handlers

use crate::agent::Agent;
use crate::gateway::{GatewayState, SessionSlot, MAX_SESSIONS};
use crate::i18n::{tr, tr_with_args};
use axum::http::StatusCode;
use axum::{extract::State, response::IntoResponse, response::Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{error, info};

static SENSITIVE_LOG_RES: LazyLock<Vec<(regex::Regex, String)>> = LazyLock::new(|| {
    ["api_key", "password", "token", "secret", "credential"]
        .into_iter()
        .filter_map(|key| {
            let pattern = format!(r#""{}"\s*:\s*"[^"]*"#, key);
            regex::Regex::new(&pattern)
                .ok()
                .map(|re| (re, format!(r#""{}":"***""#, key)))
        })
        .collect()
});

fn sanitize_for_log(s: &str) -> String {
    let mut result = s.to_string();
    for (re, replacement) in SENSITIVE_LOG_RES.iter() {
        result = re.replace_all(&result, replacement.as_str()).to_string();
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
) -> impl IntoResponse {
    let sanitized_message = sanitize_for_log(&payload.message);
    info!(
        "{}",
        tr_with_args("gateway.webhook_received", &[&sanitized_message])
    );

    let session_id = payload
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let agent = match session_agent(&state, &session_id).await {
        Ok(agent) => agent,
        Err(error) => {
            error!("{}", tr_with_args("gateway.agent_error", &[&error]));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    response: tr("gateway.agent_error_public"),
                    session_id: Some(session_id),
                }),
            )
                .into_response();
        }
    };

    let mut agent = agent.lock().await;
    let limit = crate::gateway::request_timeout(&state.config);

    match chat_within_limit(&mut agent, &payload.message, limit).await {
        Ok(Ok(response)) => {
            let sanitized_response = sanitize_for_log(&response);
            info!(
                "{}",
                tr_with_args("gateway.agent_response", &[&sanitized_response])
            );
            (
                StatusCode::OK,
                Json(WebhookResponse {
                    response,
                    session_id: Some(session_id),
                }),
            )
                .into_response()
        }
        Ok(Err(e)) => {
            error!("{}", tr_with_args("gateway.agent_error", &[&e.to_string()]));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    response: tr("gateway.agent_error_public"),
                    session_id: Some(session_id),
                }),
            )
                .into_response()
        }
        Err(_) => {
            error!("{}", tr("gateway.request_timeout"));
            (
                StatusCode::REQUEST_TIMEOUT,
                Json(WebhookResponse {
                    response: tr("gateway.request_timeout_public"),
                    session_id: Some(session_id),
                }),
            )
                .into_response()
        }
    }
}

async fn chat_within_limit(
    agent: &mut Agent,
    message: &str,
    limit: std::time::Duration,
) -> Result<anyhow::Result<String>, tokio::time::error::Elapsed> {
    tokio::time::timeout(limit, agent.chat(message)).await
}

async fn session_agent(
    state: &GatewayState,
    session_id: &str,
) -> Result<Arc<Mutex<Agent>>, String> {
    let mut sessions = state.sessions.lock().await;
    if let Some(slot) = sessions.get_mut(session_id) {
        slot.last_used = Instant::now();
        return Ok(Arc::clone(&slot.agent));
    }

    while sessions.len() >= MAX_SESSIONS {
        let oldest = sessions
            .iter()
            .min_by_key(|(_, slot)| slot.last_used)
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            sessions.remove(&id);
        } else {
            break;
        }
    }

    let mut agent = Agent::new(state.config.clone()).map_err(|e| e.to_string())?;
    if let Some(memory) = &state.memory {
        agent.bind_session(Arc::clone(memory), session_id.to_string());
        agent.load_history().await.map_err(|e| e.to_string())?;
    }
    let agent = Arc::new(Mutex::new(agent));
    sessions.insert(
        session_id.to_string(),
        SessionSlot {
            agent: Arc::clone(&agent),
            last_used: Instant::now(),
        },
    );
    Ok(agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::gateway::GatewayState;
    use crate::memory::SqliteMemory;
    use std::collections::HashMap;
    use std::time::Duration;

    fn state_with_memory(memory: Arc<SqliteMemory>) -> GatewayState {
        GatewayState {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            config: Config::default(),
            memory: Some(memory),
        }
    }

    #[tokio::test]
    async fn two_sessions_keep_their_own_history() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let state = state_with_memory(Arc::clone(&memory));

        let alpha = session_agent(&state, "alpha").await.unwrap();
        {
            let mut agent = alpha.lock().await;
            agent.push_history("user", "from-alpha");
            agent.save_history().await.unwrap();
        }
        let beta = session_agent(&state, "beta").await.unwrap();
        {
            let mut agent = beta.lock().await;
            agent.push_history("user", "from-beta");
            agent.save_history().await.unwrap();
        }

        let alpha_now = alpha.lock().await.history_snapshot();
        let beta_now = beta.lock().await.history_snapshot();
        assert!(alpha_now.iter().any(|message| message.content == "from-alpha"));
        assert!(alpha_now.iter().all(|message| message.content != "from-beta"));
        assert!(beta_now.iter().any(|message| message.content == "from-beta"));
        assert!(beta_now.iter().all(|message| message.content != "from-alpha"));

        let reloaded = state_with_memory(memory);
        let alpha_again = session_agent(&reloaded, "alpha").await.unwrap();
        let beta_again = session_agent(&reloaded, "beta").await.unwrap();
        let alpha_loaded = alpha_again.lock().await.history_snapshot();
        let beta_loaded = beta_again.lock().await.history_snapshot();
        assert!(alpha_loaded.iter().any(|message| message.content == "from-alpha"));
        assert!(alpha_loaded.iter().all(|message| message.content != "from-beta"));
        assert!(beta_loaded.iter().any(|message| message.content == "from-beta"));
        assert!(beta_loaded.iter().all(|message| message.content != "from-alpha"));
    }

    #[tokio::test]
    async fn new_agent_loads_the_same_session_from_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let mut first = Agent::new(Config::default()).unwrap();
        first.bind_session(Arc::clone(&memory), "kept".to_string());
        first.push_history("user", "remember-me");
        first.push_history("assistant", "stored");
        first.save_history().await.unwrap();

        let mut second = Agent::new(Config::default()).unwrap();
        second.bind_session(memory, "kept".to_string());
        second.load_history().await.unwrap();
        let loaded = second.history_snapshot();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].content, "remember-me");
        assert_eq!(loaded[1].content, "stored");
    }

    #[tokio::test]
    async fn session_limit_drops_the_least_recently_used() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let state = state_with_memory(memory);

        for index in 0..MAX_SESSIONS {
            session_agent(&state, &format!("s{index}")).await.unwrap();
        }
        {
            let mut sessions = state.sessions.lock().await;
            sessions.get_mut("s0").unwrap().last_used =
                Instant::now().checked_sub(Duration::from_secs(3600)).unwrap();
        }

        session_agent(&state, "newest").await.unwrap();
        let sessions = state.sessions.lock().await;
        assert_eq!(sessions.len(), MAX_SESSIONS);
        assert!(!sessions.contains_key("s0"));
        assert!(sessions.contains_key("s1"));
        assert!(sessions.contains_key("newest"));
    }

    #[tokio::test]
    async fn timed_out_chat_does_not_keep_running() {
        use crate::providers::{Message, Provider, ProviderError, Response, ToolCall};
        use async_trait::async_trait;
        use std::sync::atomic::{AtomicBool, Ordering};

        #[derive(Debug)]
        struct HangProvider {
            finished: Arc<AtomicBool>,
        }

        #[async_trait]
        impl Provider for HangProvider {
            fn name(&self) -> &str {
                "hang"
            }

            async fn chat(
                &self,
                _messages: Vec<Message>,
                _tools: Option<Vec<serde_json::Value>>,
            ) -> Result<Response, ProviderError> {
                tokio::time::sleep(Duration::from_millis(200)).await;
                self.finished.store(true, Ordering::SeqCst);
                Ok(Response {
                    message: Message {
                        role: "assistant".to_string(),
                        content: "late".to_string(),
                    },
                    tool_calls: Vec::<ToolCall>::new(),
                })
            }

            async fn health_check(&self) -> Result<(), ProviderError> {
                Ok(())
            }
        }

        let finished = Arc::new(AtomicBool::new(false));
        let mut agent = Agent::new(Config::default()).unwrap();
        agent.set_provider(Arc::new(HangProvider {
            finished: Arc::clone(&finished),
        }));

        let result = chat_within_limit(&mut agent, "hi", Duration::from_millis(30)).await;
        assert!(result.is_err());
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(!finished.load(Ordering::SeqCst));
    }
}
