use super::traits::{Tool, ToolArgument, ToolDefinition, ToolResult};
use async_trait::async_trait;
use reqwest::Url;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

const DEFAULT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_BYTES: u64 = 1024 * 1024;
const DEFAULT_MAX_REDIRECTS: usize = 3;

#[derive(Debug, Clone)]
pub struct WebFetchTool {
    timeout: Duration,
    max_bytes: u64,
    max_redirects: usize,
    block_private: bool,
    #[cfg(test)]
    loopback_ports_for_test: Vec<u16>,
}

impl WebFetchTool {
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_TIMEOUT_SECS,
            DEFAULT_MAX_BYTES,
            DEFAULT_MAX_REDIRECTS,
        )
    }

    #[must_use]
    pub fn with_limits(timeout_secs: u64, max_bytes: u64, max_redirects: usize) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_secs),
            max_bytes,
            max_redirects,
            block_private: true,
            #[cfg(test)]
            loopback_ports_for_test: Vec::new(),
        }
    }

    #[cfg(test)]
    fn allow_private(mut self) -> Self {
        self.block_private = false;
        self
    }

    #[cfg(test)]
    fn allow_loopback_port(mut self, port: u16) -> Self {
        self.loopback_ports_for_test.push(port);
        self
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

pub fn validate_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|error| format!("Invalid URL: {error}"))?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("Only http and https URLs are allowed".to_string()),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URLs with credentials are not allowed".to_string());
    }
    if url.host_str().is_none() {
        return Err("URL is missing a host".to_string());
    }
    Ok(url)
}

fn blocked_ip(ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        other => other,
    };
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 64
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || is_unique_local(v6)
                || is_link_local_v6(v6)
        }
    }
}

fn is_unique_local(ip: std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_link_local_v6(ip: std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn blocked_host_name(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local")
}

async fn ensure_public(url: &Url, allowed_loopback_ports: &[u16]) -> Result<(), String> {
    let host = url.host_str().ok_or("URL is missing a host")?;
    if blocked_host_name(host) {
        return Err("Private or local hosts are not allowed".to_string());
    }
    let port = url
        .port_or_known_default()
        .ok_or("URL is missing a port")?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        if blocked_ip(ip) && !loopback_port_allowed(ip, port, allowed_loopback_ports) {
            return Err("Private or local hosts are not allowed".to_string());
        }
        return Ok(());
    }
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| format!("Failed to resolve host: {error}"))?
        .collect();
    if addresses.is_empty() {
        return Err("Failed to resolve host".to_string());
    }
    if addresses.iter().any(|address| {
        blocked_ip(address.ip()) && !loopback_port_allowed(address.ip(), port, allowed_loopback_ports)
    }) {
        return Err("Private or local hosts are not allowed".to_string());
    }
    Ok(())
}

fn loopback_port_allowed(ip: IpAddr, port: u16, allowed_ports: &[u16]) -> bool {
    if !allowed_ports.contains(&port) {
        return false;
    }
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".to_string(),
            description: "Fetch the text content of an http or https URL".to_string(),
            arguments: vec![ToolArgument {
                name: "url".to_string(),
                arg_type: "string".to_string(),
                required: true,
                description: "Absolute http or https URL. Private networks are blocked."
                    .to_string(),
            }],
        }
    }

    async fn execute(&self, arguments: &str) -> Result<ToolResult, String> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|error| format!("Failed to parse arguments: {error}"))?;
        let raw = args["url"].as_str().ok_or("Missing 'url' parameter")?;
        match self.fetch(raw).await {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(error) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(error),
            }),
        }
    }
}

impl WebFetchTool {
    async fn fetch(&self, raw: &str) -> Result<String, String> {
        let mut url = validate_url(raw)?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(self.timeout)
            .user_agent("mini_bot.rs")
            .build()
            .map_err(|error| format!("Failed to build HTTP client: {error}"))?;

        for _ in 0..=self.max_redirects {
            if self.block_private {
                ensure_public(
                    &url,
                    #[cfg(test)]
                    &self.loopback_ports_for_test,
                    #[cfg(not(test))]
                    &[],
                )
                .await?;
            }
            let response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|error| format!("Request failed: {error}"))?;
            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or("Redirect is missing a location")?
                    .to_string();
                url = url
                    .join(&location)
                    .map_err(|error| format!("Invalid redirect location: {error}"))?;
                validate_url(url.as_str())?;
                continue;
            }
            if !status.is_success() {
                return Err(format!("HTTP status {status}"));
            }
            if let Some(length) = response.content_length() {
                if length > self.max_bytes {
                    return Err("Response exceeds the size limit".to_string());
                }
            }
            let mut body = Vec::new();
            let mut response = response;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| format!("Failed to read response: {error}"))?
            {
                if body.len() as u64 + chunk.len() as u64 > self.max_bytes {
                    return Err("Response exceeds the size limit".to_string());
                }
                body.extend_from_slice(&chunk);
            }
            return Ok(String::from_utf8_lossy(&body).into_owned());
        }
        Err("Too many redirects".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn rejects_non_http_and_local_targets() {
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("http://user:pass@example.com").is_err());
        assert!(validate_url("http://localhost/admin").is_err() || blocked_host_name("localhost"));
        assert!(blocked_ip("127.0.0.1".parse().unwrap()));
        assert!(blocked_ip("10.1.2.3".parse().unwrap()));
        assert!(blocked_ip("169.254.169.254".parse().unwrap()));
        assert!(blocked_ip("100.64.0.1".parse().unwrap()));
        assert!(blocked_ip("::1".parse().unwrap()));
        assert!(!blocked_ip("1.1.1.1".parse().unwrap()));
    }

    async fn serve(status: &str, extra_headers: &str, body: &[u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let status = status.to_string();
        let extra_headers = extra_headers.to_string();
        let body = body.to_vec();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 2048];
            let _ = socket.read(&mut buffer).await;
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n",
                body.len()
            );
            socket.write_all(header.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
        });
        port
    }

    #[tokio::test]
    async fn private_url_is_rejected_before_request() {
        let tool = WebFetchTool::new();
        let result = tool
            .execute(r#"{"url": "http://127.0.0.1/secret"}"#)
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Private"));
        let result = tool
            .execute(r#"{"url": "http://10.1.2.3/secret"}"#)
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Private"));
    }

    #[tokio::test]
    async fn fetches_text_and_limits_size() {
        let port = serve("200 OK", "", b"hello web").await;
        let tool = WebFetchTool::with_limits(2, 1024, 1).allow_private();
        let result = tool
            .execute(&format!(r#"{{"url": "http://127.0.0.1:{port}/"}}"#))
            .await
            .unwrap();
        assert!(result.success, "{result:?}");
        assert_eq!(result.output, "hello web");

        let port = serve("200 OK", "", b"abcdef").await;
        let tool = WebFetchTool::with_limits(2, 3, 0).allow_private();
        let result = tool
            .execute(&format!(r#"{{"url": "http://127.0.0.1:{port}/"}}"#))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("size"));
    }

    #[tokio::test]
    async fn redirect_to_unapproved_loopback_is_rejected() {
        let secret_port = serve("200 OK", "", b"secret").await;
        let redirect = format!("Location: http://127.0.0.1:{secret_port}/secret\r\n");
        let port = serve("302 Found", &redirect, b"").await;
        let tool = WebFetchTool::with_limits(2, 1024, 2).allow_loopback_port(port);
        let result = tool
            .execute(&format!(r#"{{"url": "http://127.0.0.1:{port}/"}}"#))
            .await
            .unwrap();
        assert!(!result.success, "{result:?}");
        assert!(result.error.unwrap().contains("Private"));
        assert_ne!(secret_port, port);
    }

    #[tokio::test]
    async fn follows_redirect_when_target_is_allowed() {
        let secret_port = serve("200 OK", "", b"secret").await;
        let redirect = format!("Location: http://127.0.0.1:{secret_port}/secret\r\n");
        let port = serve("302 Found", &redirect, b"").await;
        let tool = WebFetchTool::with_limits(2, 1024, 2)
            .allow_loopback_port(port)
            .allow_loopback_port(secret_port);
        let result = tool
            .execute(&format!(r#"{{"url": "http://127.0.0.1:{port}/"}}"#))
            .await
            .unwrap();
        assert!(result.success, "{result:?}");
        assert_eq!(result.output, "secret");
    }
}
