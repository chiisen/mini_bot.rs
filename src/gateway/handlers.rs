//! Gateway request handlers

use crate::agent::{validate_session_id, Agent, SessionKind};
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
    if validate_session_id(&session_id).is_err() {
        error!("{}", tr("gateway.invalid_session_id"));
        return (
            StatusCode::BAD_REQUEST,
            Json(WebhookResponse {
                response: tr("gateway.invalid_session_id_public"),
                session_id: None,
            }),
        )
            .into_response();
    }
    let permit = match session_agent(&state, &session_id).await {
        Ok(permit) => permit,
        Err(SessionError::Capacity) => {
            error!("{}", tr("gateway.session_capacity"));
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(WebhookResponse {
                    response: tr("gateway.session_capacity_public"),
                    session_id: Some(session_id),
                }),
            )
                .into_response();
        }
        Err(SessionError::Internal(error)) => {
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

    let agent = permit.agent();
    let mut agent = agent.lock().await;
    let limit = crate::gateway::request_timeout(&state.config);

    let outcome = chat_within_limit(&mut agent, &payload.message, limit).await;
    drop(agent);
    permit.release().await;
    match outcome {
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

#[derive(Debug)]
enum SessionError {
    Capacity,
    Internal(String),
}

struct SessionPermit {
    agent: Arc<Mutex<Agent>>,
    sessions: Arc<Mutex<std::collections::HashMap<String, SessionSlot>>>,
    id: String,
    released: bool,
}

impl SessionPermit {
    fn agent(&self) -> Arc<Mutex<Agent>> {
        Arc::clone(&self.agent)
    }

    async fn release(mut self) {
        self.release_inner().await;
    }

    async fn release_inner(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let mut sessions = self.sessions.lock().await;
        if let Some(slot) = sessions.get_mut(&self.id) {
            slot.users = slot.users.saturating_sub(1);
        }
    }
}

impl Drop for SessionPermit {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let sessions = Arc::clone(&self.sessions);
        let id = self.id.clone();
        tokio::spawn(async move {
            let mut sessions = sessions.lock().await;
            if let Some(slot) = sessions.get_mut(&id) {
                slot.users = slot.users.saturating_sub(1);
            }
        });
    }
}

async fn session_agent(
    state: &GatewayState,
    session_id: &str,
) -> Result<SessionPermit, SessionError> {
    let wait = std::time::Duration::from_secs(5);
    let started = Instant::now();
    loop {
        if started.elapsed() > wait {
            return Err(SessionError::Capacity);
        }
        let create = {
            let mut sessions = state.sessions.lock().await;
            let mut loading = state.loading.lock().await;
            if let Some(slot) = sessions.get_mut(session_id) {
                slot.last_used = Instant::now();
                slot.users += 1;
                return Ok(SessionPermit {
                    agent: Arc::clone(&slot.agent),
                    sessions: Arc::clone(&state.sessions),
                    id: session_id.to_string(),
                    released: false,
                });
            }
            let mut should_create = false;
            if loading.contains(session_id) {
            } else if sessions.len() + loading.len() >= MAX_SESSIONS {
                let oldest = sessions
                    .iter()
                    .filter(|(_, slot)| slot.users == 0)
                    .min_by_key(|(_, slot)| slot.last_used)
                    .map(|(id, _)| id.clone());
                if let Some(id) = oldest {
                    sessions.remove(&id);
                    loading.insert(session_id.to_string());
                    should_create = true;
                } else {
                    return Err(SessionError::Capacity);
                }
            } else {
                loading.insert(session_id.to_string());
                should_create = true;
            }
            should_create
        };
        if !create {
            tokio::time::timeout(std::time::Duration::from_millis(50), state.session_ready.notified())
                .await
                .ok();
            continue;
        }

        let built = async {
            let mut agent = Agent::new(state.config.clone()).map_err(|error| error.to_string())?;
            if let Some(memory) = &state.memory {
                agent.bind_session(
                    Arc::clone(memory),
                    SessionKind::Gateway,
                    session_id.to_string(),
                );
                agent.load_history().await.map_err(|error| error.to_string())?;
            }
            Ok::<_, String>(Arc::new(Mutex::new(agent)))
        }
        .await;

        let mut sessions = state.sessions.lock().await;
        let mut loading = state.loading.lock().await;
        loading.remove(session_id);
        state.session_ready.notify_waiters();
        match built {
            Ok(agent) => {
                if let Some(existing) = sessions.get_mut(session_id) {
                    existing.last_used = Instant::now();
                    existing.users += 1;
                    return Ok(SessionPermit {
                        agent: Arc::clone(&existing.agent),
                        sessions: Arc::clone(&state.sessions),
                        id: session_id.to_string(),
                        released: false,
                    });
                }
                sessions.insert(
                    session_id.to_string(),
                    SessionSlot {
                        agent: Arc::clone(&agent),
                        last_used: Instant::now(),
                        users: 1,
                    },
                );
                return Ok(SessionPermit {
                    agent,
                    sessions: Arc::clone(&state.sessions),
                    id: session_id.to_string(),
                    released: false,
                });
            }
            Err(error) => return Err(SessionError::Internal(error)),
        }
    }
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
            loading: Arc::new(Mutex::new(std::collections::HashSet::new())),
            session_ready: Arc::new(tokio::sync::Notify::new()),
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
            let agent_ref = alpha.agent();
            let mut agent = agent_ref.lock().await;
            agent.push_history("user", "from-alpha");
            agent.save_history().await.unwrap();
        }
        let beta = session_agent(&state, "beta").await.unwrap();
        {
            let agent_ref = beta.agent();
            let mut agent = agent_ref.lock().await;
            agent.push_history("user", "from-beta");
            agent.save_history().await.unwrap();
        }

        let alpha_now = alpha.agent().lock().await.history_snapshot();
        let beta_now = beta.agent().lock().await.history_snapshot();
        assert!(alpha_now.iter().any(|message| message.content == "from-alpha"));
        assert!(alpha_now.iter().all(|message| message.content != "from-beta"));
        assert!(beta_now.iter().any(|message| message.content == "from-beta"));
        assert!(beta_now.iter().all(|message| message.content != "from-alpha"));

        let reloaded = state_with_memory(memory);
        let alpha_again = session_agent(&reloaded, "alpha").await.unwrap();
        let beta_again = session_agent(&reloaded, "beta").await.unwrap();
        let alpha_loaded = alpha_again.agent().lock().await.history_snapshot();
        let beta_loaded = beta_again.agent().lock().await.history_snapshot();
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
        first.bind_session(Arc::clone(&memory), SessionKind::Gateway, "kept".to_string());
        first.push_history("user", "remember-me");
        first.push_history("assistant", "stored");
        first.save_history().await.unwrap();

        let mut second = Agent::new(Config::default()).unwrap();
        second.bind_session(memory, SessionKind::Gateway, "kept".to_string());
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
            session_agent(&state, &format!("s{index}"))
                .await
                .unwrap()
                .release()
                .await;
        }
        {
            let mut sessions = state.sessions.lock().await;
            sessions.get_mut("s0").unwrap().last_used =
                Instant::now().checked_sub(Duration::from_secs(3600)).unwrap();
        }

        session_agent(&state, "newest").await.unwrap().release().await;
        let sessions = state.sessions.lock().await;
        assert_eq!(sessions.len(), MAX_SESSIONS);
        assert!(!sessions.contains_key("s0"));
        assert!(sessions.contains_key("s1"));
        assert!(sessions.contains_key("newest"));
    }

    #[tokio::test]
    async fn in_use_session_is_not_evicted_and_stays_unique() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let state = state_with_memory(memory);
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let state_for_busy = state.clone();
        let busy = tokio::spawn(async move {
            let permit = session_agent(&state_for_busy, "busy").await.unwrap();
            let agent = permit.agent();
            let _guard = agent.lock().await;
            let _ = release_rx.await;
            drop(_guard);
            permit.release().await;
            agent
        });
        let busy_agent = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(slot) = state.sessions.lock().await.get("busy") {
                    if slot.users > 0 {
                        return Arc::clone(&slot.agent);
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("busy session");

        for index in 0..MAX_SESSIONS - 1 {
            session_agent(&state, &format!("idle{index}"))
                .await
                .unwrap()
                .release()
                .await;
        }
        session_agent(&state, "newest").await.unwrap().release().await;
        {
            let sessions = state.sessions.lock().await;
            assert!(sessions.contains_key("busy"));
            assert_eq!(sessions.len(), MAX_SESSIONS);
        }
        let again = session_agent(&state, "busy").await.unwrap();
        assert!(Arc::ptr_eq(&again.agent(), &busy_agent));
        again.release().await;

        let mut held = Vec::new();
        for index in 0..MAX_SESSIONS {
            let id = format!("hold{index}");
            if state.sessions.lock().await.contains_key(&id) {
                continue;
            }
            match session_agent(&state, &id).await {
                Ok(permit) => held.push(permit),
                Err(SessionError::Capacity) => break,
                Err(SessionError::Internal(error)) => panic!("{error}"),
            }
        }
        let overflow = session_agent(&state, "overflow").await;
        assert!(matches!(overflow, Err(SessionError::Capacity)));
        drop(held);
        release_tx.send(()).unwrap();
        let _ = busy.await.unwrap();
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

    #[tokio::test]
    async fn invalid_session_id_is_rejected_before_create() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(SqliteMemory::new(dir.path().join("memory.db")).unwrap());
        let state = state_with_memory(memory);
        for session_id in ["a.b", "中文", &"z".repeat(241)] {
            let response = webhook_handler(
                State(state.clone()),
                Json(WebhookRequest {
                    message: "hi".to_string(),
                    session_id: Some(session_id.to_string()),
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        assert!(state.sessions.lock().await.is_empty());
    }
}
