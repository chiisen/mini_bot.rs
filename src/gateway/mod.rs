//! Gateway module for MiniBot
//!
//! Provides HTTP webhook server for external integrations.

mod handlers;

use crate::agent::Agent;
use crate::config::Config;
use crate::i18n::tr_with_args;
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
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

pub use handlers::*;

pub struct GatewayState {
    pub agent: Arc<Mutex<Agent>>,
    pub config: Config,
}

impl Clone for GatewayState {
    fn clone(&self) -> Self {
        Self {
            agent: Arc::clone(&self.agent),
            config: self.config.clone(),
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

        let timestamps = requests.entry(key.to_string()).or_insert_with(Vec::new);
        timestamps.retain(|t| now.duration_since(*t) < window);

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

pub async fn run(host: &str, port: u16) -> Result<()> {
    let config = Config::load_or_default()?;
    let agent = Agent::new(config.clone())?;

    let state = GatewayState {
        agent: Arc::new(Mutex::new(agent)),
        config: config.clone(),
    };

    let (max_requests, window_secs) = config
        .gateway_security
        .as_ref()
        .map(|s| (s.rate_limit_requests, s.rate_limit_window_secs))
        .unwrap_or((10, 60));

    let rate_limiter = RateLimiter::new(max_requests, window_secs);

    let cors_layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/webhook", post(webhook_handler))
        .route("/health", get(health_handler))
        .layer(cors_layer)
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(30),
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
