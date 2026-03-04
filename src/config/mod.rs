use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

pub mod crypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub default_provider: String,
    pub default_model: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    pub gateway: GatewayConfig,
    pub gateway_security: Option<GatewaySecurityConfig>,
    pub agent: AgentConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySecurityConfig {
    pub api_key: String,
    pub rate_limit_requests: usize,
    pub rate_limit_window_secs: u64,
    pub allowed_origins: Vec<String>,
    pub allowed_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_tool_iterations: usize,
    pub max_history_messages: usize,
    pub temperature: f64,
    pub max_execution_time_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub workspace_only: bool,
    pub allowed_roots: Vec<String>,
    pub allowed_commands: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            default_provider: "minimax".to_string(),
            default_model: "minimax-coding-plan/MiniMax-M2.5".to_string(),
            api_key: String::new(),
            gateway: GatewayConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            gateway_security: Some(GatewaySecurityConfig {
                api_key: String::new(),
                rate_limit_requests: 10,
                rate_limit_window_secs: 60,
                allowed_origins: vec!["*".to_string()],
                allowed_ips: vec![],
            }),
            agent: AgentConfig {
                max_tool_iterations: 100,
                max_history_messages: 50,
                temperature: 0.7,
                max_execution_time_secs: 300,
            },
            security: SecurityConfig {
                workspace_only: true,
                allowed_roots: vec![],
                allowed_commands: vec![],
            },
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        if let Ok(env_api_key) = std::env::var("MINIMAX_API_KEY") {
            if !env_api_key.is_empty() {
                config.api_key = env_api_key;
            }
        }

        if let Ok(env_api_key) = std::env::var("MINIBOT_API_KEY") {
            if !env_api_key.is_empty() {
                config.api_key = env_api_key;
            }
        }

        if let Some(ref mut gateway_security) = config.gateway_security {
            if let Ok(env_gateway_key) = std::env::var("MINIBOT_GATEWAY_API_KEY") {
                if !env_gateway_key.is_empty() {
                    gateway_security.api_key = env_gateway_key;
                }
            }
        }

        if let Some(ref encryption_key) = crypto::get_encryption_key() {
            if !config.api_key.is_empty() && config.api_key.starts_with("ENC:") {
                let encrypted = config.api_key.trim_start_matches("ENC:");
                if let Ok(decrypted) = crypto::decrypt(encrypted, encryption_key) {
                    config.api_key = decrypted;
                }
            }

            if let Some(ref mut gateway_security) = config.gateway_security {
                if !gateway_security.api_key.is_empty()
                    && gateway_security.api_key.starts_with("ENC:")
                {
                    let encrypted = gateway_security.api_key.trim_start_matches("ENC:");
                    if let Ok(decrypted) = crypto::decrypt(encrypted, encryption_key) {
                        gateway_security.api_key = decrypted;
                    }
                }
            }
        }

        Ok(config)
    }

    pub fn get_api_key(&self) -> String {
        std::env::var("MINIMAX_API_KEY")
            .or_else(|_| std::env::var("MINIBOT_API_KEY"))
            .unwrap_or_else(|_| self.api_key.clone())
    }

    pub fn get_gateway_api_key(&self) -> String {
        std::env::var("MINIBOT_GATEWAY_API_KEY").unwrap_or_else(|_| {
            self.gateway_security
                .as_ref()
                .map(|s| s.api_key.clone())
                .unwrap_or_default()
        })
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "minibot", "mini_bot_rs")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn default_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    #[allow(dead_code)]
    pub fn encrypt_value(value: &str) -> Result<String, String> {
        let key =
            crypto::get_encryption_key().ok_or("Encryption key not set (MINIBOT_CONFIG_KEY)")?;
        crypto::encrypt(value, &key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.default_provider, "minimax");
        assert_eq!(config.gateway.port, 3000);
        assert_eq!(config.agent.temperature, 0.7);
    }

    #[test]
    fn test_config_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.toml");

        let config = Config::default();
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.version, config.version);
        assert_eq!(loaded.default_provider, config.default_provider);
    }

    #[test]
    fn test_config_dir() {
        let dir = Config::config_dir();
        assert!(dir.to_string_lossy().len() > 0);
    }
}
