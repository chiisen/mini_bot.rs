mod minimax;
mod traits;

pub use minimax::MiniMaxProvider;
pub use traits::{Message, Provider, ProviderError, Response, ToolCall};

use std::sync::Arc;

pub fn create_provider(
    provider_type: &str,
    api_key: String,
    model: String,
    temperature: f64,
) -> Result<Arc<dyn Provider>, String> {
    match provider_type.to_lowercase().as_str() {
        "minimax" => Ok(Arc::new(MiniMaxProvider::new(api_key, model, temperature))),
        _ => Err(format!("Unsupported provider type: {}", provider_type)),
    }
}
