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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_minimax_provider() {
        let provider = create_provider(
            "minimax",
            "test_api_key".to_string(),
            "test_model".to_string(),
            0.7,
        );
        assert!(provider.is_ok());
        assert_eq!(provider.unwrap().name(), "minimax");
    }

    #[test]
    fn test_create_provider_case_insensitive() {
        let provider = create_provider(
            "MINIMAX",
            "test_api_key".to_string(),
            "test_model".to_string(),
            0.7,
        );
        assert!(provider.is_ok());
    }

    #[test]
    fn test_create_unsupported_provider() {
        let provider = create_provider(
            "unknown",
            "test_api_key".to_string(),
            "test_model".to_string(),
            0.7,
        );
        assert!(provider.is_err());
        assert!(provider.unwrap_err().contains("Unsupported provider"));
    }
}
