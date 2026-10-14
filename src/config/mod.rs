use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod crypto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub default_provider: String,
    pub default_model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
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
    #[serde(default = "default_max_history_chars")]
    pub max_history_chars: usize,
    pub temperature: f64,
    pub max_execution_time_secs: u64,
}

fn default_max_history_chars() -> usize {
    32_000
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
                max_history_chars: default_max_history_chars(),
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
    pub fn load_or_default() -> anyhow::Result<Self> {
        Self::load_or_default_from(&Self::default_path())
    }

    /// 檔案不存在時用預設值。檔案存在但讀取或解析失敗時回傳錯誤。
    pub fn load_or_default_from(path: &PathBuf) -> anyhow::Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            tracing::info!(
                "config file not found at {}, using defaults",
                path.display()
            );
            Ok(Self::default())
        }
    }

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

    /// 產生有效設定的文字報告。
    ///
    /// # Errors
    ///
    /// 設定檔存在但無法讀取或解析時回傳錯誤。
    pub fn show(path: &PathBuf) -> anyhow::Result<String> {
        let (file_exists, file_api_key, file_gateway_key) = read_file_secrets(path)?;
        let mut config = Self::load_or_default_from(path)?;
        config.api_key.clear();
        if let Some(security) = config.gateway_security.as_mut() {
            security.api_key.clear();
        }
        Ok(render_report(
            path,
            file_exists,
            &config,
            provider_key_source(
                env_non_empty("MINIMAX_API_KEY"),
                env_non_empty("MINIBOT_API_KEY"),
                &file_api_key,
            ),
            gateway_key_source(env_non_empty("MINIBOT_GATEWAY_API_KEY"), &file_gateway_key),
        ))
    }

    /// 產生不含金鑰的範例設定。
    ///
    /// # Errors
    ///
    /// 設定無法序列化時回傳錯誤。
    pub fn example_toml() -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(&Self::default())?)
    }

    /// 把範例設定寫到 `path`。檔案已存在時不覆寫。
    ///
    /// # Errors
    ///
    /// 目標已存在、目錄無法建立或寫入失敗時回傳錯誤。
    pub fn write_example(path: &PathBuf) -> anyhow::Result<()> {
        if path.exists() {
            anyhow::bail!("設定檔已存在：{}", path.display());
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, Self::example_toml()?)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn encrypt_value(value: &str) -> Result<String, String> {
        let key =
            crypto::get_encryption_key().ok_or("Encryption key not set (MINIBOT_CONFIG_KEY)")?;
        crypto::encrypt(value, &key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretSource {
    Missing,
    Environment(&'static str),
    File,
    FileEncrypted,
}

fn env_non_empty(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| !value.is_empty())
}

fn read_file_secrets(path: &PathBuf) -> anyhow::Result<(bool, String, String)> {
    if !path.exists() {
        return Ok((false, String::new(), String::new()));
    }
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    let gateway_key = config
        .gateway_security
        .map(|security| security.api_key)
        .unwrap_or_default();
    Ok((true, config.api_key, gateway_key))
}

#[must_use]
pub fn provider_key_source(minimax_env: bool, minibot_env: bool, file_value: &str) -> SecretSource {
    if minibot_env {
        SecretSource::Environment("MINIBOT_API_KEY")
    } else if minimax_env {
        SecretSource::Environment("MINIMAX_API_KEY")
    } else {
        file_secret_source(file_value)
    }
}

#[must_use]
pub fn gateway_key_source(gateway_env: bool, file_value: &str) -> SecretSource {
    if gateway_env {
        SecretSource::Environment("MINIBOT_GATEWAY_API_KEY")
    } else {
        file_secret_source(file_value)
    }
}

fn file_secret_source(file_value: &str) -> SecretSource {
    if file_value.is_empty() {
        SecretSource::Missing
    } else if file_value.starts_with("ENC:") {
        SecretSource::FileEncrypted
    } else {
        SecretSource::File
    }
}

fn source_label(source: SecretSource) -> String {
    match source {
        SecretSource::Missing => "未設定".to_string(),
        SecretSource::Environment(name) => format!("環境變數 {name}"),
        SecretSource::File => "設定檔".to_string(),
        SecretSource::FileEncrypted => "設定檔（已加密）".to_string(),
    }
}

fn list_label(values: &[String]) -> String {
    if values.is_empty() {
        "（無）".to_string()
    } else {
        values.join(", ")
    }
}

fn render_report(
    path: &std::path::Path,
    file_exists: bool,
    config: &Config,
    provider_key: SecretSource,
    gateway_key: SecretSource,
) -> String {
    let file_state = if file_exists {
        "存在"
    } else {
        "不存在，使用預設值"
    };
    let security = config.gateway_security.clone().unwrap_or_else(|| GatewaySecurityConfig {
        api_key: String::new(),
        rate_limit_requests: 10,
        rate_limit_window_secs: 60,
        allowed_origins: vec!["*".to_string()],
        allowed_ips: vec![],
    });
    let provider_label = source_label(provider_key);
    let gateway_label = source_label(gateway_key);
    format!(
        "\
設定檔：{path}（{file_state}）
provider：{provider}
model：{model}
provider_api_key：{provider_label}
gateway：{host}:{port}
gateway_api_key：{gateway_label}
rate_limit：{rate_requests} / {rate_window}s
allowed_origins：{origins}
allowed_ips：{ips}
max_tool_iterations：{iterations}
max_history_messages：{history_messages}
max_history_chars：{history_chars}
temperature：{temperature}
max_execution_time_secs：{execution_secs}
workspace_only：{workspace_only}
allowed_roots：{roots}
allowed_commands：{commands}
",
        path = path.display(),
        provider = config.default_provider,
        model = config.default_model,
        host = config.gateway.host,
        port = config.gateway.port,
        rate_requests = security.rate_limit_requests,
        rate_window = security.rate_limit_window_secs,
        origins = list_label(&security.allowed_origins),
        ips = list_label(&security.allowed_ips),
        iterations = config.agent.max_tool_iterations,
        history_messages = config.agent.max_history_messages,
        history_chars = config.agent.max_history_chars,
        temperature = config.agent.temperature,
        execution_secs = config.agent.max_execution_time_secs,
        workspace_only = config.security.workspace_only,
        roots = list_label(&config.security.allowed_roots),
        commands = list_label(&config.security.allowed_commands),
    )
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
    fn test_invalid_config_is_error() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.toml");
        std::fs::write(&path, "this is not toml {{{").unwrap();

        let err = Config::load_or_default_from(&path).unwrap_err();
        assert!(err.to_string().len() > 0);
    }

    #[test]
    fn test_missing_config_uses_default() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.toml");

        let loaded = Config::load_or_default_from(&path).unwrap();
        assert_eq!(loaded.version, Config::default().version);
    }

    #[test]
    fn test_config_dir() {
        let dir = Config::config_dir();
        assert!(dir.to_string_lossy().len() > 0);
    }

    #[test]
    fn provider_key_env_overrides_file_and_minibot_wins() {
        assert_eq!(
            provider_key_source(true, true, "file-secret"),
            SecretSource::Environment("MINIBOT_API_KEY")
        );
        assert_eq!(
            provider_key_source(true, false, "file-secret"),
            SecretSource::Environment("MINIMAX_API_KEY")
        );
        assert_eq!(
            provider_key_source(false, false, "file-secret"),
            SecretSource::File
        );
        assert_eq!(
            provider_key_source(false, false, "ENC:aabb"),
            SecretSource::FileEncrypted
        );
        assert_eq!(
            provider_key_source(false, false, ""),
            SecretSource::Missing
        );
        assert_eq!(
            gateway_key_source(true, "gateway-secret"),
            SecretSource::Environment("MINIBOT_GATEWAY_API_KEY")
        );
    }

    #[test]
    fn show_redacts_file_secrets() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
version = "1.0"
default_provider = "minimax"
default_model = "MiniMax-M2.5"
api_key = "provider-secret-value"

[gateway]
host = "127.0.0.1"
port = 3000

[gateway_security]
api_key = "gateway-secret-value"
rate_limit_requests = 3
rate_limit_window_secs = 15
allowed_origins = ["https://example.com"]
allowed_ips = []

[agent]
max_tool_iterations = 100
max_history_messages = 50
max_history_chars = 32000
temperature = 0.7
max_execution_time_secs = 300

[security]
workspace_only = true
allowed_roots = []
allowed_commands = []
"#,
        )
        .unwrap();

        let report = Config::show(&path).unwrap();
        assert!(!report.contains("provider-secret-value"));
        assert!(!report.contains("gateway-secret-value"));
        assert!(report.contains("rate_limit：3 / 15s"));
        assert!(report.contains("https://example.com"));
        assert!(report.contains(&path.display().to_string()));
    }

    #[test]
    fn example_omits_secrets_and_refuses_existing_file() {
        let example = Config::example_toml().unwrap();
        assert!(!example.contains("provider-secret-value"));
        assert!(example.contains("max_history_chars"));
        assert!(example.contains("max_execution_time_secs"));

        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("nested/config.toml");
        Config::write_example(&path).unwrap();
        let err = Config::write_example(&path).unwrap_err();
        assert!(err.to_string().contains("已存在"));
    }
}
