use reqwest::Url;
use reqwest::blocking::Client;
use serde_json::Value;

const ENV_ENABLE_SPACETIME_SYNC: &str = "OPENAGENTS_ENABLE_SPACETIME_SYNC";
const ENV_CONTROL_BASE_URL_KEYS: [&str; 3] = [
    "OA_CONTROL_HTTP_BASE_URL",
    "OA_CONTROL_BASE_URL",
    "OA_OPENAGENTS_CONTROL_BASE_URL",
];
const ENV_SPACETIME_BASE_URL_KEYS: [&str; 2] =
    ["OA_SPACETIME_HTTP_BASE_URL", "OA_SPACETIME_DEV_HTTP_BASE_URL"];
const ENV_SPACETIME_DATABASE_KEYS: [&str; 2] = ["OA_SPACETIME_DATABASE", "OA_SPACETIME_DEV_DATABASE"];

const LEGACY_SYNC_TOKEN_PATHS: [&str; 3] = [
    "/api/spacetime/token",
    "/api/v1/spacetime/token",
    "/api/v1/sync/token",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpacetimeSubscribeTarget {
    pub base_url: String,
    pub database: String,
    pub subscribe_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncTokenLease {
    pub token: String,
    pub transport: Option<String>,
    pub protocol_version: Option<String>,
    pub refresh_after_in_seconds: Option<u64>,
    pub refresh_after: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncBootstrapResult {
    pub control_token_endpoint: String,
    pub target: SpacetimeSubscribeTarget,
    pub token_lease: SyncTokenLease,
}

pub fn spacetime_sync_enabled_from_env() -> bool {
    std::env::var(ENV_ENABLE_SPACETIME_SYNC)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn resolve_control_base_url_from_env() -> Result<String, String> {
    resolve_env_any(
        &ENV_CONTROL_BASE_URL_KEYS,
        "missing control base url; set OA_CONTROL_HTTP_BASE_URL",
    )
}

pub fn resolve_subscribe_target_from_env() -> Result<SpacetimeSubscribeTarget, String> {
    let base_url = resolve_env_any(
        &ENV_SPACETIME_BASE_URL_KEYS,
        "missing spacetime base url; set OA_SPACETIME_HTTP_BASE_URL",
    )?;
    let database = resolve_env_any(
        &ENV_SPACETIME_DATABASE_KEYS,
        "missing spacetime database; set OA_SPACETIME_DATABASE",
    )?;
    resolve_subscribe_target(base_url.as_str(), database.as_str())
}

pub fn resolve_subscribe_target(
    spacetime_base_url: &str,
    database: &str,
) -> Result<SpacetimeSubscribeTarget, String> {
    let normalized_base = normalize_http_base_url(spacetime_base_url)?;
    let normalized_database = database.trim();
    if normalized_database.is_empty() {
        return Err("spacetime database must not be empty".to_string());
    }
    if normalized_database.contains('/') {
        return Err("spacetime database must not contain '/'".to_string());
    }

    let mut subscribe_url = Url::parse(normalized_base.as_str())
        .map_err(|error| format!("invalid spacetime base url: {error}"))?;
    subscribe_url.set_path(format!("/v1/database/{normalized_database}/subscribe").as_str());
    subscribe_url.set_query(None);
    subscribe_url.set_fragment(None);

    Ok(SpacetimeSubscribeTarget {
        base_url: normalized_base,
        database: normalized_database.to_string(),
        subscribe_url: subscribe_url.to_string(),
    })
}

pub fn canonical_sync_token_endpoint(control_base_url: &str) -> Result<Url, String> {
    let normalized_base = normalize_http_base_url(control_base_url)?;
    let mut url = Url::parse(normalized_base.as_str())
        .map_err(|error| format!("invalid control base url: {error}"))?;
    let current_path = url.path().trim_end_matches('/');
    if LEGACY_SYNC_TOKEN_PATHS
        .iter()
        .any(|legacy| current_path.ends_with(legacy))
    {
        return Err(format!(
            "legacy sync token endpoint path is not allowed: {}",
            url.path()
        ));
    }

    url.set_path("/api/sync/token");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

pub fn mint_sync_token_blocking(
    client: &Client,
    control_base_url: &str,
    bearer_auth: Option<&str>,
) -> Result<SyncTokenLease, String> {
    let endpoint = canonical_sync_token_endpoint(control_base_url)?;
    let mut request = client.post(endpoint.clone());
    if let Some(token) = bearer_auth {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .map_err(|error| format!("sync token mint request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|_| "<unreadable-body>".to_string());

    if !status.is_success() {
        return Err(format!(
            "sync token mint failed status={} body={}",
            status,
            truncate_body(body.as_str())
        ));
    }

    parse_sync_token_lease(body.as_str())
}

pub fn bootstrap_sync_session_from_env(
    client: &Client,
    bearer_auth: Option<&str>,
) -> Result<Option<SyncBootstrapResult>, String> {
    if !spacetime_sync_enabled_from_env() {
        return Ok(None);
    }

    let control_base_url = resolve_control_base_url_from_env()?;
    let target = resolve_subscribe_target_from_env()?;
    let result = bootstrap_sync_session(
        client,
        control_base_url.as_str(),
        target.base_url.as_str(),
        target.database.as_str(),
        bearer_auth,
    )?;
    Ok(Some(result))
}

pub fn bootstrap_sync_session(
    client: &Client,
    control_base_url: &str,
    spacetime_base_url: &str,
    database: &str,
    bearer_auth: Option<&str>,
) -> Result<SyncBootstrapResult, String> {
    let token_endpoint = canonical_sync_token_endpoint(control_base_url)?;
    let target = resolve_subscribe_target(spacetime_base_url, database)?;
    let token_lease = mint_sync_token_blocking(client, control_base_url, bearer_auth)?;
    Ok(SyncBootstrapResult {
        control_token_endpoint: token_endpoint.to_string(),
        target,
        token_lease,
    })
}

fn parse_sync_token_lease(raw: &str) -> Result<SyncTokenLease, String> {
    let payload: Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid token payload: {error}"))?;

    let token = payload
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "sync token response missing non-empty token".to_string())?
        .to_string();

    let transport = payload
        .get("transport")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    if let Some(value) = transport.as_deref()
        && value != "spacetime_ws"
    {
        return Err(format!("unexpected sync transport '{}'; expected spacetime_ws", value));
    }

    let protocol_version = payload
        .get("protocol_version")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    if let Some(value) = protocol_version.as_deref()
        && value != "spacetime.sync.v1"
    {
        return Err(format!(
            "unexpected sync protocol_version '{}'; expected spacetime.sync.v1",
            value
        ));
    }

    let refresh_after_in_seconds = payload.get("refresh_after_in").and_then(Value::as_u64);
    let refresh_after = payload
        .get("refresh_after")
        .and_then(Value::as_str)
        .map(|value| value.to_string());

    Ok(SyncTokenLease {
        token,
        transport,
        protocol_version,
        refresh_after_in_seconds,
        refresh_after,
    })
}

fn resolve_env_any(keys: &[&str], err: &str) -> Result<String, String> {
    for key in keys {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }
    Err(format!("{} (checked: {})", err, keys.join(", ")))
}

fn normalize_http_base_url(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("base_url must not be empty".to_string());
    }
    let parsed = Url::parse(trimmed).map_err(|error| format!("invalid base_url: {error}"))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported base_url scheme: {scheme}"));
    }
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn truncate_body(value: &str) -> String {
    const MAX: usize = 240;
    if value.len() <= MAX {
        return value.to_string();
    }
    let prefix = &value[..MAX];
    format!("{prefix}...")
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use reqwest::blocking::Client;

    use super::{
        SyncTokenLease, bootstrap_sync_session, canonical_sync_token_endpoint,
        mint_sync_token_blocking, resolve_subscribe_target,
    };

    fn run_single_response_server(response: &'static str) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should exist");

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept should succeed");
            let mut request = [0u8; 4096];
            let read = stream.read(&mut request).expect("read should succeed");
            let request_text = String::from_utf8_lossy(&request[..read]);
            assert!(
                request_text.starts_with("POST /api/sync/token HTTP/1.1"),
                "unexpected request line: {request_text}"
            );
            stream
                .write_all(response.as_bytes())
                .expect("response write should succeed");
        });

        addr
    }

    #[test]
    fn canonical_endpoint_rejects_legacy_paths() {
        let err = canonical_sync_token_endpoint("https://control.example.com/api/spacetime/token")
            .expect_err("legacy path must fail");
        assert!(err.contains("legacy sync token endpoint path"));
    }

    #[test]
    fn resolve_subscribe_target_builds_spacetime_subscribe_url() {
        let target = resolve_subscribe_target("https://sync.example.com/", "autopilot")
            .expect("target should resolve");
        assert_eq!(target.base_url, "https://sync.example.com");
        assert_eq!(
            target.subscribe_url,
            "https://sync.example.com/v1/database/autopilot/subscribe"
        );
    }

    #[test]
    fn mint_sync_token_hits_canonical_endpoint_and_parses_payload() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/json\r\n",
            "Content-Length: 88\r\n",
            "Connection: close\r\n\r\n",
            "{\"token\":\"sync-token\",\"transport\":\"spacetime_ws\",\"protocol_version\":\"spacetime.sync.v1\"}"
        );
        let addr: SocketAddr = run_single_response_server(response);
        let client = Client::builder().build().expect("client should build");

        let lease = mint_sync_token_blocking(&client, format!("http://{addr}").as_str(), None)
            .expect("token mint should succeed");
        assert_eq!(
            lease,
            SyncTokenLease {
                token: "sync-token".to_string(),
                transport: Some("spacetime_ws".to_string()),
                protocol_version: Some("spacetime.sync.v1".to_string()),
                refresh_after_in_seconds: None,
                refresh_after: None,
            }
        );
    }

    #[test]
    fn bootstrap_sync_session_performs_token_mint_and_target_resolution() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/json\r\n",
            "Content-Length: 93\r\n",
            "Connection: close\r\n\r\n",
            "{\"token\":\"bootstrap-token\",\"transport\":\"spacetime_ws\",\"protocol_version\":\"spacetime.sync.v1\"}"
        );
        let addr: SocketAddr = run_single_response_server(response);
        let client = Client::builder().build().expect("client should build");

        let result = bootstrap_sync_session(
            &client,
            format!("http://{addr}").as_str(),
            "https://sync.example.com",
            "autopilot",
            None,
        )
        .expect("bootstrap should succeed");

        assert_eq!(result.token_lease.token, "bootstrap-token");
        assert_eq!(result.control_token_endpoint, format!("http://{addr}/api/sync/token"));
        assert_eq!(
            result.target.subscribe_url,
            "https://sync.example.com/v1/database/autopilot/subscribe"
        );
    }
}
