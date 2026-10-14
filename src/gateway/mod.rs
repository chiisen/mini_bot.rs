//! Gateway module for MiniBot
//!
//! Provides HTTP webhook server for external integrations.

mod handlers;

use crate::agent::Agent;
use crate::config::Config;
use crate::i18n::tr_with_args;
use crate::memory::SqliteMemory;
use anyhow::Result;
use axum::{
    body::Body,
    extract::Request,
    extract::{ConnectInfo, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use axum::http::HeaderValue;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

pub use handlers::*;

pub const MAX_SESSIONS: usize = 128;

pub(crate) fn request_timeout(config: &Config) -> Duration {
    Duration::from_secs(config.agent.max_execution_time_secs)
}

pub(crate) fn allows_any_origin(origins: Option<&[String]>) -> bool {
    match origins {
        None => true,
        Some(list) => list.is_empty() || list.iter().any(|origin| origin == "*"),
    }
}

pub(crate) fn cors_layer_for(origins: Option<&[String]>) -> CorsLayer {
    let layer = CorsLayer::new().allow_methods(Any).allow_headers(Any);
    if allows_any_origin(origins) {
        return layer.allow_origin(Any);
    }

    let allowed = origins
        .unwrap_or_default()
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok());
    layer.allow_origin(AllowOrigin::list(allowed))
}

pub struct SessionSlot {
    pub agent: Arc<Mutex<Agent>>,
    pub last_used: Instant,
}

pub struct GatewayState {
    pub sessions: Arc<Mutex<HashMap<String, SessionSlot>>>,
    pub config: Config,
    pub memory: Option<Arc<SqliteMemory>>,
}

impl Clone for GatewayState {
    fn clone(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
            config: self.config.clone(),
            memory: self.memory.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    pub async fn is_allowed(&self, key: &str) -> bool {
        let mut requests = self.requests.write().await;
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        requests.retain(|_, timestamps| {
            timestamps.retain(|t| now.duration_since(*t) < window);
            !timestamps.is_empty()
        });

        let timestamps = requests.entry(key.to_string()).or_insert_with(Vec::new);

        if timestamps.len() >= self.max_requests {
            return false;
        }

        timestamps.push(now);
        true
    }
}

async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let client_ip = addr.ip().to_string();

    if !limiter.is_allowed(&client_ip).await {
        let mut response = Response::new(Body::from("Too Many Requests"));
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return response;
    }

    next.run(request).await
}

async fn auth_middleware(
    State(state): State<GatewayState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if let Some(ref security) = state.config.gateway_security {
        if !security.allowed_ips.is_empty() {
            let client_ip = addr.ip().to_string();
            if !security.allowed_ips.contains(&client_ip) {
                let mut response = Response::new(Body::from("Forbidden: IP not allowed"));
                *response.status_mut() = StatusCode::FORBIDDEN;
                return response;
            }
        }

        if !security.api_key.is_empty() {
            let api_key = request
                .headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok());

            if api_key != Some(&security.api_key) {
                let mut response = Response::new(Body::from("Unauthorized"));
                *response.status_mut() = StatusCode::UNAUTHORIZED;
                return response;
            }
        }
    }

    next.run(request).await
}

pub async fn run(host: &str, port: u16, config_path: &PathBuf) -> Result<()> {
    let config = Config::load_or_default_from(config_path)?;
    let memory_path = config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("memory.db");
    if let Some(parent) = memory_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let memory = SqliteMemory::new(memory_path).map_err(anyhow::Error::msg)?;

    let state = GatewayState {
        sessions: Arc::new(Mutex::new(HashMap::new())),
        config: config.clone(),
        memory: Some(Arc::new(memory)),
    };

    let (max_requests, window_secs) = config
        .gateway_security
        .as_ref()
        .map(|s| (s.rate_limit_requests, s.rate_limit_window_secs))
        .unwrap_or((10, 60));

    let rate_limiter = RateLimiter::new(max_requests, window_secs);

    let cors_layer = cors_layer_for(
        config
            .gateway_security
            .as_ref()
            .map(|security| security.allowed_origins.as_slice()),
    );

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/webhook", post(webhook_handler))
        .route("/health", get(health_handler))
        .layer(cors_layer)
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            request_timeout(&config),
        ))
        .layer(TraceLayer::new_for_http())
        .route_layer(axum::middleware::from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_middleware,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    info!(
        "{}",
        tr_with_args("gateway.listening", &[&addr.to_string()])
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::get;
    use tower::ServiceExt;

    fn protected_app(config: Config, max_requests: usize) -> Router {
        let state = GatewayState {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            config,
            memory: None,
        };
        let limiter = RateLimiter::new(max_requests, 60);
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route_layer(axum::middleware::from_fn_with_state(
                limiter,
                rate_limit_middleware,
            ))
            .route_layer(axum::middleware::from_fn_with_state(state, auth_middleware))
    }

    fn config_with_security(api_key: &str, allowed_ips: Vec<String>, limit: usize) -> Config {
        let mut config = Config::default();
        config.gateway_security = Some(crate::config::GatewaySecurityConfig {
            api_key: api_key.to_string(),
            rate_limit_requests: limit,
            rate_limit_window_secs: 60,
            allowed_origins: vec!["*".to_string()],
            allowed_ips,
        });
        config
    }

    async fn status_from(app: Router, ip: [u8; 4], api_key: Option<&str>) -> StatusCode {
        use axum::extract::connect_info::MockConnectInfo;
        let mut request = Request::builder().uri("/health");
        if let Some(key) = api_key {
            request = request.header("x-api-key", key);
        }
        let response = app
            .layer(MockConnectInfo(SocketAddr::from((ip, 9))))
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        response.status()
    }

    #[tokio::test]
    async fn rejects_missing_or_wrong_api_key() {
        let config = config_with_security("secret", vec![], 10);
        let missing = status_from(protected_app(config.clone(), 10), [203, 0, 113, 1], None).await;
        let wrong = status_from(
            protected_app(config, 10),
            [203, 0, 113, 1],
            Some("nope"),
        )
        .await;
        assert_eq!(missing, StatusCode::UNAUTHORIZED);
        assert_eq!(wrong, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_ip_outside_allowlist() {
        let config = config_with_security("", vec!["203.0.113.9".to_string()], 10);
        let status = status_from(protected_app(config, 10), [203, 0, 113, 1], None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn allows_matching_ip_and_api_key() {
        let config = config_with_security("secret", vec!["203.0.113.9".to_string()], 10);
        let status = status_from(
            protected_app(config, 10),
            [203, 0, 113, 9],
            Some("secret"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn rate_limit_returns_too_many_requests() {
        let config = config_with_security("", vec![], 1);
        let app = protected_app(config, 1).layer(axum::extract::connect_info::MockConnectInfo(
            SocketAddr::from(([203, 0, 113, 5], 9)),
        ));
        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let second = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn http_timeout_follows_agent_limit() {
        let mut config = Config::default();
        config.agent.max_execution_time_secs = 12;
        assert_eq!(request_timeout(&config), Duration::from_secs(12));
    }

    #[test]
    fn specific_origins_do_not_use_any() {
        assert!(!allows_any_origin(Some(&[
            "https://example.com".to_string()
        ])));
        assert!(allows_any_origin(Some(&["*".to_string()])));
        assert!(allows_any_origin(Some(&[])));
        assert!(allows_any_origin(None));
    }

    #[tokio::test]
    async fn configured_origin_is_echoed() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(cors_layer_for(Some(&[
                "https://example.com".to_string()
            ])));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("origin", "https://example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "https://example.com"
        );
    }
}
