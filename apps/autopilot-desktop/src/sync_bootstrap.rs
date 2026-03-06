use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

const ENV_ENABLE_SPACETIME_SYNC: &str = "OPENAGENTS_ENABLE_SPACETIME_SYNC";
const ENV_CONTROL_BASE_URL_KEYS: [&str; 3] = [
    "OA_CONTROL_HTTP_BASE_URL",
    "OA_CONTROL_BASE_URL",
    "OA_OPENAGENTS_CONTROL_BASE_URL",
];
const ENV_SPACETIME_BASE_URL_KEYS: [&str; 2] = [
    "OA_SPACETIME_HTTP_BASE_URL",
    "OA_SPACETIME_DEV_HTTP_BASE_URL",
];
const ENV_SPACETIME_DATABASE_KEYS: [&str; 2] =
    ["OA_SPACETIME_DATABASE", "OA_SPACETIME_DEV_DATABASE"];
const ENV_REQUIRED_STREAM_GRANTS: &str = "OA_SYNC_REQUIRED_STREAM_GRANTS";
const ENV_CONTROL_BEARER_TOKEN_KEYS: [&str; 3] = [
    "OA_CONTROL_BEARER_TOKEN",
    "OA_CONTROL_ACCESS_TOKEN",
    "OA_OPENAGENTS_CONTROL_BEARER_TOKEN",
];
const ENV_ENABLE_CONTROL_SESSION_BOOTSTRAP: &str = "OA_CONTROL_BOOTSTRAP_DESKTOP_SESSION";
const ENV_CONTROL_DESKTOP_CLIENT_ID_KEYS: [&str; 2] =
    ["OA_CONTROL_DESKTOP_CLIENT_ID", "OA_DESKTOP_CLIENT_ID"];
const ENV_CONTROL_DEVICE_NAME_KEYS: [&str; 2] =
    ["OA_CONTROL_DEVICE_NAME", "OA_DESKTOP_DEVICE_NAME"];
const ENV_CONTROL_BOUND_NOSTR_PUBKEY_KEYS: [&str; 2] =
    ["OA_CONTROL_BOUND_NOSTR_PUBKEY", "OA_NOSTR_PUBKEY"];
const ENV_CONTROL_CLIENT_VERSION_KEYS: [&str; 2] =
    ["OA_CONTROL_CLIENT_VERSION", "OA_DESKTOP_CLIENT_VERSION"];

const LEGACY_SYNC_TOKEN_PATHS: [&str; 3] = [
    "/api/spacetime/token",
    "/api/v1/spacetime/token",
    "/api/v1/sync/token",
];
const REQUIRED_SYNC_SCOPE: &str = "sync.subscribe";
const DEFAULT_REQUIRED_STREAM_GRANTS: [&str; 2] = [
    "stream.activity_projection.v1",
    "stream.earn_job_lifecycle_projection.v1",
];
const CONTROL_DESKTOP_SESSION_PATH: &str = "/api/session/desktop";

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
    pub scopes: Vec<String>,
    pub stream_grants: Vec<String>,
    pub issued_at_unix_ms: Option<u64>,
    pub not_before_unix_ms: Option<u64>,
    pub expires_at_unix_ms: Option<u64>,
    pub revoked: bool,
    pub rotation_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncBootstrapResult {
    pub control_token_endpoint: String,
    pub target: SpacetimeSubscribeTarget,
    pub token_lease: SyncTokenLease,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSessionLease {
    pub session_id: String,
    pub account_id: String,
    pub access_token: String,
    pub token_type: String,
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
    pub issued_at_unix_ms: Option<u64>,
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopSessionBootstrapRequest {
    pub desktop_client_id: String,
    pub device_name: Option<String>,
    pub bound_nostr_pubkey: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ControlSessionLeasePayload {
    session_id: String,
    account_id: String,
    access_token: String,
    #[serde(default)]
    token_type: Option<String>,
    desktop_client_id: String,
    #[serde(default)]
    device_name: Option<String>,
    #[serde(default)]
    bound_nostr_pubkey: Option<String>,
    #[serde(default)]
    client_version: Option<String>,
    #[serde(default)]
    issued_at_unix_ms: Option<u64>,
    #[serde(default)]
    expires_at_unix_ms: Option<u64>,
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

pub fn canonical_desktop_session_endpoint(control_base_url: &str) -> Result<Url, String> {
    let normalized_base = normalize_http_base_url(control_base_url)?;
    let mut url = Url::parse(normalized_base.as_str())
        .map_err(|error| format!("invalid control base url: {error}"))?;
    url.set_path(CONTROL_DESKTOP_SESSION_PATH);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

pub fn mint_control_session_blocking(
    client: &Client,
    control_base_url: &str,
    request_payload: &DesktopSessionBootstrapRequest,
) -> Result<ControlSessionLease, String> {
    let endpoint = canonical_desktop_session_endpoint(control_base_url)?;
    let response = client
        .post(endpoint)
        .json(request_payload)
        .send()
        .map_err(|error| format!("control session bootstrap request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|_| "<unreadable-body>".to_string());
    if !status.is_success() {
        return Err(format!(
            "control session bootstrap failed status={} body={}",
            status.as_u16(),
            truncate_body(body.as_str())
        ));
    }
    let payload: ControlSessionLeasePayload = serde_json::from_str(body.as_str())
        .map_err(|error| format!("invalid control session payload: {error}"))?;
    let session_id = payload.session_id.trim();
    let account_id = payload.account_id.trim();
    let access_token = payload.access_token.trim();
    let desktop_client_id = payload.desktop_client_id.trim();
    if session_id.is_empty() || account_id.is_empty() || access_token.is_empty() {
        return Err("control session response missing required identifiers".to_string());
    }
    if desktop_client_id.is_empty() {
        return Err("control session response missing desktop_client_id".to_string());
    }

    Ok(ControlSessionLease {
        session_id: session_id.to_string(),
        account_id: account_id.to_string(),
        access_token: access_token.to_string(),
        token_type: payload.token_type.unwrap_or_else(|| "Bearer".to_string()),
        desktop_client_id: desktop_client_id.to_string(),
        device_name: payload.device_name,
        bound_nostr_pubkey: payload.bound_nostr_pubkey,
        client_version: payload.client_version,
        issued_at_unix_ms: payload.issued_at_unix_ms,
        expires_at_unix_ms: payload.expires_at_unix_ms,
    })
}

pub fn resolve_control_bearer_auth(
    client: &Client,
    control_base_url: &str,
    static_bearer_auth: Option<&str>,
    session_bootstrap_request: Option<&DesktopSessionBootstrapRequest>,
) -> Result<Option<String>, String> {
    if let Some(token) = static_bearer_auth
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(token.to_string()));
    }

    if let Some(request_payload) = session_bootstrap_request {
        let session = mint_control_session_blocking(client, control_base_url, request_payload)?;
        return Ok(Some(session.access_token));
    }

    Ok(None)
}

pub fn resolve_control_bearer_auth_from_env(
    client: &Client,
    control_base_url: &str,
    bound_nostr_pubkey_override: Option<&str>,
) -> Result<Option<String>, String> {
    let static_bearer_auth = ENV_CONTROL_BEARER_TOKEN_KEYS
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let session_bootstrap_request = session_bootstrap_request_from_env(bound_nostr_pubkey_override);
    resolve_control_bearer_auth(
        client,
        control_base_url,
        static_bearer_auth.as_deref(),
        session_bootstrap_request.as_ref(),
    )
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
        return Err(classify_sync_token_http_error(status, body.as_str()));
    }

    let required_stream_grants = required_stream_grants_from_env();
    parse_sync_token_lease(
        body.as_str(),
        now_unix_ms(),
        required_stream_grants.as_slice(),
    )
}

pub fn bootstrap_sync_session_from_env(
    client: &Client,
    bound_nostr_pubkey_override: Option<&str>,
) -> Result<Option<SyncBootstrapResult>, String> {
    if !spacetime_sync_enabled_from_env() {
        return Ok(None);
    }

    let control_base_url = resolve_control_base_url_from_env()?;
    let bearer_auth = resolve_control_bearer_auth_from_env(
        client,
        control_base_url.as_str(),
        bound_nostr_pubkey_override,
    )?;
    let target = resolve_subscribe_target_from_env()?;
    let result = bootstrap_sync_session(
        client,
        control_base_url.as_str(),
        target.base_url.as_str(),
        target.database.as_str(),
        bearer_auth.as_deref(),
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

fn parse_sync_token_lease(
    raw: &str,
    now_unix_ms: u64,
    required_stream_grants: &[String],
) -> Result<SyncTokenLease, String> {
    let payload: Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid token payload: {error}"))?;
    let claims = payload.get("claims");

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
        return Err(format!(
            "unexpected sync transport '{}'; expected spacetime_ws",
            value
        ));
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
    if matches!(refresh_after_in_seconds, Some(0)) {
        return Err(auth_error_unauthorized("refresh_after_in_invalid"));
    }

    let scopes =
        collect_claim_values(&payload, claims, &["scope", "scopes", "sync_scopes"]).into_iter();
    let scopes: Vec<String> = scopes.collect();
    if !scopes.iter().any(|scope| scope == REQUIRED_SYNC_SCOPE) {
        return Err(auth_error_forbidden(format!(
            "missing_scope:{REQUIRED_SYNC_SCOPE}"
        )));
    }

    let stream_grants = collect_claim_values(
        &payload,
        claims,
        &["stream_grants", "allowed_streams", "streams"],
    )
    .into_iter();
    let stream_grants: Vec<String> = stream_grants.collect();
    if stream_grants.is_empty() {
        return Err(auth_error_forbidden("stream_grants_missing"));
    }
    for stream_id in required_stream_grants {
        if !stream_grants
            .iter()
            .any(|grant| stream_grant_allows(grant.as_str(), stream_id.as_str()))
        {
            return Err(auth_error_forbidden(format!(
                "stream_not_granted:{stream_id}"
            )));
        }
    }

    let issued_at_unix_ms =
        claim_timestamp_ms(&payload, claims, &["issued_at_unix_ms", "issued_at", "iat"]);
    let not_before_unix_ms = claim_timestamp_ms(
        &payload,
        claims,
        &["not_before_unix_ms", "not_before", "nbf"],
    );
    let expires_at_unix_ms = claim_timestamp_ms(
        &payload,
        claims,
        &["expires_at_unix_ms", "expires_at", "exp"],
    );

    let revoked = claim_bool(&payload, claims, &["revoked", "token_revoked"]).unwrap_or(false);
    if revoked {
        return Err(auth_error_forbidden("token_revoked"));
    }
    if let Some(status) = claim_string(&payload, claims, &["token_status", "status"]) {
        let normalized = status.to_ascii_lowercase();
        if normalized.contains("revoked") {
            return Err(auth_error_forbidden("token_revoked"));
        }
        if normalized.contains("expired") {
            return Err(auth_error_expired("token_expired"));
        }
        if normalized.contains("not_yet_valid") || normalized.contains("not yet valid") {
            return Err(auth_error_not_yet_valid("token_not_yet_valid"));
        }
    }
    if let Some(not_before) = not_before_unix_ms
        && now_unix_ms < not_before
    {
        return Err(auth_error_not_yet_valid("token_not_yet_valid"));
    }
    if let Some(expires_at) = expires_at_unix_ms
        && now_unix_ms >= expires_at
    {
        return Err(auth_error_expired("token_expired"));
    }
    if let (Some(refresh_after), Some(expires_at)) = (refresh_after_in_seconds, expires_at_unix_ms)
    {
        let refresh_deadline = now_unix_ms.saturating_add(refresh_after.saturating_mul(1_000));
        if refresh_deadline >= expires_at {
            return Err(auth_error_expired("refresh_boundary_exceeds_expiry"));
        }
    }

    let rotation_id = claim_string(&payload, claims, &["rotation_id", "token_id"]);
    let rotation_required = claim_bool(&payload, claims, &["rotation_required"]).unwrap_or(false);
    if rotation_required && rotation_id.is_none() {
        return Err(auth_error_forbidden("rotation_id_missing"));
    }

    Ok(SyncTokenLease {
        token,
        transport,
        protocol_version,
        refresh_after_in_seconds,
        refresh_after,
        scopes,
        stream_grants,
        issued_at_unix_ms,
        not_before_unix_ms,
        expires_at_unix_ms,
        revoked,
        rotation_id,
    })
}

fn classify_sync_token_http_error(status: StatusCode, body: &str) -> String {
    let normalized = body.to_ascii_lowercase();
    let truncated = truncate_body(body);
    match status {
        StatusCode::UNAUTHORIZED => {
            if normalized.contains("token_expired") || normalized.contains("expired") {
                return format!(
                    "{} status={} body={}",
                    auth_error_expired("token_expired"),
                    status.as_u16(),
                    truncated
                );
            }
            if normalized.contains("token_not_yet_valid") || normalized.contains("not_yet_valid") {
                return format!(
                    "{} status={} body={}",
                    auth_error_not_yet_valid("token_not_yet_valid"),
                    status.as_u16(),
                    truncated
                );
            }
            format!(
                "{} status={} body={}",
                auth_error_unauthorized("status_401"),
                status.as_u16(),
                truncated
            )
        }
        StatusCode::FORBIDDEN => {
            if normalized.contains("token_revoked") || normalized.contains("revoked") {
                return format!(
                    "{} status={} body={}",
                    auth_error_forbidden("token_revoked"),
                    status.as_u16(),
                    truncated
                );
            }
            if normalized.contains("missing_scope") {
                return format!(
                    "{} status={} body={}",
                    auth_error_forbidden("missing_scope"),
                    status.as_u16(),
                    truncated
                );
            }
            format!(
                "{} status={} body={}",
                auth_error_forbidden("status_403"),
                status.as_u16(),
                truncated
            )
        }
        _ => format!(
            "sync token mint failed status={} body={}",
            status.as_u16(),
            truncated
        ),
    }
}

fn auth_error_unauthorized(reason: impl AsRef<str>) -> String {
    format!("sync_auth:unauthorized:{}", reason.as_ref())
}

fn auth_error_forbidden(reason: impl AsRef<str>) -> String {
    format!("sync_auth:forbidden:{}", reason.as_ref())
}

fn auth_error_expired(reason: impl AsRef<str>) -> String {
    format!("sync_auth:expired:{}", reason.as_ref())
}

fn auth_error_not_yet_valid(reason: impl AsRef<str>) -> String {
    format!("sync_auth:not_yet_valid:{}", reason.as_ref())
}

fn required_stream_grants_from_env() -> Vec<String> {
    if let Ok(value) = std::env::var(ENV_REQUIRED_STREAM_GRANTS) {
        let parsed = value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !parsed.is_empty() {
            return parsed;
        }
    }
    DEFAULT_REQUIRED_STREAM_GRANTS
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn claim_string(payload: &Value, claims: Option<&Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| extract_claim_field(payload, claims, key))
        .and_then(Value::as_str)
        .map(str::trim)
        .map(ToOwned::to_owned)
        .filter(|value| !value.is_empty())
}

fn claim_bool(payload: &Value, claims: Option<&Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| extract_claim_field(payload, claims, key))
        .and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            },
            _ => None,
        })
}

fn claim_timestamp_ms(payload: &Value, claims: Option<&Value>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| extract_claim_field(payload, claims, key))
        .and_then(timestamp_value_as_ms)
}

fn collect_claim_values(
    payload: &Value,
    claims: Option<&Value>,
    keys: &[&str],
) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    for key in keys {
        if let Some(field) = extract_claim_field(payload, claims, key) {
            collect_string_claims(field, &mut values);
        }
    }
    values
}

fn extract_claim_field<'a>(
    payload: &'a Value,
    claims: Option<&'a Value>,
    key: &str,
) -> Option<&'a Value> {
    claims
        .and_then(|value| value.get(key))
        .or_else(|| payload.get(key))
}

fn collect_string_claims(value: &Value, target: &mut BTreeSet<String>) {
    match value {
        Value::String(raw) => {
            for part in raw.split([',', ' ']) {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    target.insert(trimmed.to_string());
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Some(raw) = item.as_str() {
                    let trimmed = raw.trim();
                    if !trimmed.is_empty() {
                        target.insert(trimmed.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

fn timestamp_value_as_ms(value: &Value) -> Option<u64> {
    let raw = match value {
        Value::Number(number) => number.as_u64()?,
        Value::String(raw) => raw.trim().parse::<u64>().ok()?,
        _ => return None,
    };
    if raw < 1_000_000_000_000 {
        Some(raw.saturating_mul(1_000))
    } else {
        Some(raw)
    }
}

fn stream_grant_allows(grant: &str, stream_id: &str) -> bool {
    let grant = grant.trim();
    if grant.is_empty() {
        return false;
    }
    if grant == "*" || grant.eq_ignore_ascii_case("all") {
        return true;
    }
    if let Some(prefix) = grant.strip_suffix('*') {
        return stream_id.starts_with(prefix);
    }
    grant == stream_id
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

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn truncate_body(value: &str) -> String {
    const MAX: usize = 240;
    if value.len() <= MAX {
        return value.to_string();
    }
    let prefix = &value[..MAX];
    format!("{prefix}...")
}

pub fn session_bootstrap_request_from_env(
    bound_nostr_pubkey_override: Option<&str>,
) -> Option<DesktopSessionBootstrapRequest> {
    if !env_flag_enabled(ENV_ENABLE_CONTROL_SESSION_BOOTSTRAP) {
        return None;
    }

    Some(DesktopSessionBootstrapRequest {
        desktop_client_id: resolve_optional_env_any(&ENV_CONTROL_DESKTOP_CLIENT_ID_KEYS)
            .unwrap_or_else(default_desktop_client_id),
        device_name: resolve_optional_env_any(&ENV_CONTROL_DEVICE_NAME_KEYS)
            .or_else(default_device_name),
        bound_nostr_pubkey: resolve_optional_env_any(&ENV_CONTROL_BOUND_NOSTR_PUBKEY_KEYS)
            .or_else(|| bound_nostr_pubkey_override.map(str::to_string)),
        client_version: resolve_optional_env_any(&ENV_CONTROL_CLIENT_VERSION_KEYS),
    })
}

fn resolve_optional_env_any(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_flag_enabled(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn default_desktop_client_id() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown-host".to_string());
    format!("autopilot-desktop-{host}")
}

fn default_device_name() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use reqwest::blocking::Client;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};

    use super::{
        ControlSessionLease, DesktopSessionBootstrapRequest, SyncTokenLease,
        bootstrap_sync_session, canonical_desktop_session_endpoint, canonical_sync_token_endpoint,
        mint_control_session_blocking, mint_sync_token_blocking, parse_sync_token_lease,
        resolve_control_bearer_auth, resolve_subscribe_target,
    };

    fn required_streams() -> Vec<String> {
        vec![
            "stream.activity_projection.v1".to_string(),
            "stream.earn_job_lifecycle_projection.v1".to_string(),
        ]
    }

    fn valid_token_payload() -> String {
        r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "refresh_after_in":120,
  "rotation_id":"rotation-1",
  "claims":{
    "scope":"sync.subscribe sync.append",
    "stream_grants":["stream.activity_projection.v1","stream.earn_job_lifecycle_projection.v1"],
    "issued_at_unix_ms":1700000000000,
    "not_before_unix_ms":1700000000000,
    "expires_at_unix_ms":4700000000000
  }
}"#
        .to_string()
    }

    fn run_json_response_server(status: &str, body: &str) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should exist");
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );

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

    fn run_session_then_sync_token_server(
        session_body: &str,
        sync_token_body: &str,
    ) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should exist");
        let session_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{session_body}",
            session_body.len()
        );
        let sync_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sync_token_body}",
            sync_token_body.len()
        );

        std::thread::spawn(move || {
            let (mut first_stream, _) = listener.accept().expect("first accept should succeed");
            let mut first_request = [0u8; 4096];
            let first_read = first_stream
                .read(&mut first_request)
                .expect("first read should succeed");
            let first_text = String::from_utf8_lossy(&first_request[..first_read]);
            assert!(
                first_text.starts_with("POST /api/session/desktop HTTP/1.1"),
                "unexpected first request line: {first_text}"
            );
            first_stream
                .write_all(session_response.as_bytes())
                .expect("first response write should succeed");

            let (mut second_stream, _) = listener.accept().expect("second accept should succeed");
            let mut second_request = [0u8; 4096];
            let second_read = second_stream
                .read(&mut second_request)
                .expect("second read should succeed");
            let second_text = String::from_utf8_lossy(&second_request[..second_read]);
            assert!(
                second_text.starts_with("POST /api/sync/token HTTP/1.1"),
                "unexpected second request line: {second_text}"
            );
            assert!(
                second_text.contains("authorization: Bearer desktop-access-token")
                    || second_text.contains("Authorization: Bearer desktop-access-token"),
                "expected bearer auth header in sync token request: {second_text}"
            );
            second_stream
                .write_all(sync_response.as_bytes())
                .expect("second response write should succeed");
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
    fn canonical_desktop_session_endpoint_uses_canonical_path() {
        let url = canonical_desktop_session_endpoint("https://control.example.com/base")
            .expect("desktop session endpoint should resolve");
        assert_eq!(
            url.as_str(),
            "https://control.example.com/api/session/desktop"
        );
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
        let body = valid_token_payload();
        let addr: SocketAddr = run_json_response_server("200 OK", body.as_str());
        let client = Client::builder().build().expect("client should build");

        let lease = mint_sync_token_blocking(&client, format!("http://{addr}").as_str(), None)
            .expect("token mint should succeed");
        assert_eq!(
            lease,
            SyncTokenLease {
                token: "sync-token".to_string(),
                transport: Some("spacetime_ws".to_string()),
                protocol_version: Some("spacetime.sync.v1".to_string()),
                refresh_after_in_seconds: Some(120),
                refresh_after: None,
                scopes: vec!["sync.append".to_string(), "sync.subscribe".to_string()],
                stream_grants: vec![
                    "stream.activity_projection.v1".to_string(),
                    "stream.earn_job_lifecycle_projection.v1".to_string()
                ],
                issued_at_unix_ms: Some(1_700_000_000_000),
                not_before_unix_ms: Some(1_700_000_000_000),
                expires_at_unix_ms: Some(4_700_000_000_000),
                revoked: false,
                rotation_id: Some("rotation-1".to_string()),
            }
        );
    }

    #[test]
    fn mint_control_session_hits_canonical_endpoint_and_parses_payload() {
        let body = r#"{
  "session_id":"sess-control-1",
  "account_id":"desktop:alpha",
  "access_token":"desktop-access-token",
  "token_type":"Bearer",
  "desktop_client_id":"desktop-alpha",
  "device_name":"Chris MacBook",
  "bound_nostr_pubkey":"npub1alpha",
  "client_version":"mvp",
  "issued_at_unix_ms":1700000000000,
  "expires_at_unix_ms":1700003600000
}"#;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr should exist");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept should succeed");
            let mut request = [0u8; 4096];
            let read = stream.read(&mut request).expect("read should succeed");
            let request_text = String::from_utf8_lossy(&request[..read]);
            assert!(
                request_text.starts_with("POST /api/session/desktop HTTP/1.1"),
                "unexpected request line: {request_text}"
            );
            stream
                .write_all(response.as_bytes())
                .expect("response write should succeed");
        });
        let client = Client::builder().build().expect("client should build");

        let lease = mint_control_session_blocking(
            &client,
            format!("http://{addr}").as_str(),
            &DesktopSessionBootstrapRequest {
                desktop_client_id: "desktop-alpha".to_string(),
                device_name: Some("Chris MacBook".to_string()),
                bound_nostr_pubkey: Some("npub1alpha".to_string()),
                client_version: Some("mvp".to_string()),
            },
        )
        .expect("control session mint should succeed");
        assert_eq!(
            lease,
            ControlSessionLease {
                session_id: "sess-control-1".to_string(),
                account_id: "desktop:alpha".to_string(),
                access_token: "desktop-access-token".to_string(),
                token_type: "Bearer".to_string(),
                desktop_client_id: "desktop-alpha".to_string(),
                device_name: Some("Chris MacBook".to_string()),
                bound_nostr_pubkey: Some("npub1alpha".to_string()),
                client_version: Some("mvp".to_string()),
                issued_at_unix_ms: Some(1_700_000_000_000),
                expires_at_unix_ms: Some(1_700_003_600_000),
            }
        );
    }

    #[test]
    fn resolve_control_bearer_auth_bootstraps_session_before_sync_token_mint() {
        let session_body = r#"{
  "session_id":"sess-control-1",
  "account_id":"desktop:alpha",
  "access_token":"desktop-access-token",
  "token_type":"Bearer",
  "desktop_client_id":"desktop-alpha"
}"#;
        let sync_token_body = valid_token_payload();
        let addr = run_session_then_sync_token_server(session_body, sync_token_body.as_str());
        let client = Client::builder().build().expect("client should build");
        let control_base_url = format!("http://{addr}");
        let bearer_auth = resolve_control_bearer_auth(
            &client,
            control_base_url.as_str(),
            None,
            Some(&DesktopSessionBootstrapRequest {
                desktop_client_id: "desktop-alpha".to_string(),
                device_name: None,
                bound_nostr_pubkey: None,
                client_version: None,
            }),
        )
        .expect("control bearer resolution should succeed");
        assert_eq!(bearer_auth.as_deref(), Some("desktop-access-token"));

        let lease =
            mint_sync_token_blocking(&client, control_base_url.as_str(), bearer_auth.as_deref())
                .expect("sync token mint should succeed");
        assert_eq!(lease.token, "sync-token");
    }

    #[test]
    fn bootstrap_sync_session_performs_token_mint_and_target_resolution() {
        let payload = valid_token_payload().replace("sync-token", "bootstrap-token");
        let addr: SocketAddr = run_json_response_server("200 OK", payload.as_str());
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
        assert_eq!(
            result.control_token_endpoint,
            format!("http://{addr}/api/sync/token")
        );
        assert_eq!(
            result.target.subscribe_url,
            "https://sync.example.com/v1/database/autopilot/subscribe"
        );
    }

    #[test]
    fn parse_sync_token_rejects_expired_claims() {
        let raw = r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "claims":{
    "scope":"sync.subscribe",
    "stream_grants":["stream.activity_projection.v1","stream.earn_job_lifecycle_projection.v1"],
    "expires_at_unix_ms":1700000000000
  }
}"#;
        let error = parse_sync_token_lease(raw, 1_700_000_000_001, required_streams().as_slice())
            .expect_err("expired token should fail");
        assert!(error.starts_with("sync_auth:expired:token_expired"));
    }

    #[test]
    fn parse_sync_token_rejects_not_yet_valid_claims() {
        let raw = r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "claims":{
    "scope":"sync.subscribe",
    "stream_grants":["stream.activity_projection.v1","stream.earn_job_lifecycle_projection.v1"],
    "not_before_unix_ms":2000,
    "expires_at_unix_ms":5000
  }
}"#;
        let error = parse_sync_token_lease(raw, 1_999, required_streams().as_slice())
            .expect_err("not yet valid token should fail");
        assert!(error.starts_with("sync_auth:not_yet_valid:token_not_yet_valid"));
    }

    #[test]
    fn parse_sync_token_rejects_revoked_claims() {
        let raw = r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "claims":{
    "scope":"sync.subscribe",
    "stream_grants":["stream.activity_projection.v1","stream.earn_job_lifecycle_projection.v1"],
    "revoked":true,
    "expires_at_unix_ms":5000
  }
}"#;
        let error = parse_sync_token_lease(raw, 2_000, required_streams().as_slice())
            .expect_err("revoked token should fail");
        assert!(error.starts_with("sync_auth:forbidden:token_revoked"));
    }

    #[test]
    fn parse_sync_token_rejects_scope_mismatch() {
        let raw = r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "claims":{
    "scope":"sync.append",
    "stream_grants":["stream.activity_projection.v1","stream.earn_job_lifecycle_projection.v1"],
    "expires_at_unix_ms":5000
  }
}"#;
        let error = parse_sync_token_lease(raw, 2_000, required_streams().as_slice())
            .expect_err("scope mismatch should fail");
        assert!(error.starts_with("sync_auth:forbidden:missing_scope:sync.subscribe"));
    }

    #[test]
    fn parse_sync_token_rejects_stream_grant_mismatch() {
        let raw = r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "claims":{
    "scope":"sync.subscribe",
    "stream_grants":["stream.activity_projection.v1"],
    "expires_at_unix_ms":5000
  }
}"#;
        let error = parse_sync_token_lease(raw, 2_000, required_streams().as_slice())
            .expect_err("stream grant mismatch should fail");
        assert!(error.starts_with(
            "sync_auth:forbidden:stream_not_granted:stream.earn_job_lifecycle_projection.v1"
        ));
    }

    #[test]
    fn parse_sync_token_rejects_invalid_refresh_window() {
        let raw = r#"{
  "token":"sync-token",
  "transport":"spacetime_ws",
  "protocol_version":"spacetime.sync.v1",
  "refresh_after_in":120,
  "claims":{
    "scope":"sync.subscribe",
    "stream_grants":["stream.activity_projection.v1","stream.earn_job_lifecycle_projection.v1"],
    "expires_at_unix_ms":1000000120000
  }
}"#;
        let error = parse_sync_token_lease(raw, 1_000_000_000_000, required_streams().as_slice())
            .expect_err("refresh boundary violation should fail");
        assert!(error.starts_with("sync_auth:expired:refresh_boundary_exceeds_expiry"));
    }

    #[test]
    fn mint_sync_token_classifies_auth_http_failures() {
        let unauthorized_addr =
            run_json_response_server("401 Unauthorized", "{\"error\":\"token_expired\"}");
        let forbidden_addr =
            run_json_response_server("403 Forbidden", "{\"error\":\"missing_scope\"}");
        let client = Client::builder().build().expect("client should build");

        let unauthorized_error = mint_sync_token_blocking(
            &client,
            format!("http://{unauthorized_addr}").as_str(),
            None,
        )
        .expect_err("401 should fail");
        assert!(unauthorized_error.starts_with("sync_auth:expired:token_expired"));

        let forbidden_error =
            mint_sync_token_blocking(&client, format!("http://{forbidden_addr}").as_str(), None)
                .expect_err("403 should fail");
        assert!(forbidden_error.starts_with("sync_auth:forbidden:missing_scope"));
    }
}
