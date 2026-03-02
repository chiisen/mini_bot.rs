//! Gateway module for MiniBot
//! 
//! Provides HTTP webhook server for external integrations.

mod handlers;

use crate::agent::Agent;
use crate::config::Config;
use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
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

pub async fn run(host: &str, port: u16) -> Result<()> {
    let config = load_config()?;
    let agent = Agent::new(config.clone())?;

    let state = GatewayState {
        agent: Arc::new(Mutex::new(agent)),
        config,
    };

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/webhook", post(webhook_handler))
        .route("/health", get(health_handler))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(30)))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    info!("Gateway server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn load_config() -> Result<Config> {
    let path = Config::default_path();
    
    if path.exists() {
        Config::load(&path).or_else(|_| Ok(Config::default()))
    } else {
        Ok(Config::default())
    }
}
