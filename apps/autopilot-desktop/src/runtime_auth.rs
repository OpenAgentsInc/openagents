use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use chrono::Utc;
use reqwest::{Client as HttpClient, Method};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const DEFAULT_AUTH_BASE_URL: &str = "https://openagents.com";
const AUTH_STATE_FILE_NAME: &str = "autopilot-desktop-runtime-auth.json";
const AUTH_CLIENT_HEADER: &str = "autopilot-desktop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSyncAuthState {
    pub base_url: String,
    pub token: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub issued_at: String,
}

pub async fn login_with_email_code(
    base_url: &str,
    email: &str,
    code_override: Option<String>,
) -> Result<RuntimeSyncAuthState, String> {
    let normalized_base_url = normalize_base_url(base_url)?;
    let normalized_email = normalize_email(email)?;
    let client = HttpClient::builder()
        .cookie_store(true)
        .build()
        .map_err(|err| format!("auth client init failed: {err}"))?;

    post_json(
        &client,
        &normalized_base_url,
        "/api/auth/email",
        json!({ "email": normalized_email }),
        Some((("x-client"), AUTH_CLIENT_HEADER)),
    )
    .await?;

    let code = match code_override {
        Some(code) => normalize_code(&code)?,
        None => read_code_from_stdin()?,
    };

    let verify = post_json(
        &client,
        &normalized_base_url,
        "/api/auth/verify",
        json!({ "code": code }),
        Some((("x-client"), AUTH_CLIENT_HEADER)),
    )
    .await?;

    let token = verify
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "auth verify succeeded but token was missing".to_string())?
        .to_string();

    let user_id = verify
        .get("userId")
        .and_then(|value| {
            value
                .as_str()
                .map(|raw| raw.trim().to_string())
                .or_else(|| value.as_u64().map(|raw| raw.to_string()))
        })
        .filter(|value| !value.is_empty());

    let response_email = verify
        .get("user")
        .and_then(Value::as_object)
        .and_then(|user| user.get("email"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    Ok(RuntimeSyncAuthState {
        base_url: normalized_base_url,
        token,
        user_id,
        email: response_email.or_else(|| Some(normalized_email.to_string())),
        issued_at: Utc::now().to_rfc3339(),
    })
}

pub fn load_runtime_auth_state() -> Option<RuntimeSyncAuthState> {
    let path = runtime_auth_state_path()?;
    let raw = fs::read_to_string(path).ok()?;
    let state = serde_json::from_str::<RuntimeSyncAuthState>(&raw).ok()?;
    if state.base_url.trim().is_empty() || state.token.trim().is_empty() {
        return None;
    }

    Some(RuntimeSyncAuthState {
        base_url: state.base_url.trim().trim_end_matches('/').to_string(),
        token: state.token.trim().to_string(),
        user_id: state
            .user_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        email: state
            .email
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        issued_at: state.issued_at.trim().to_string(),
    })
}

pub fn persist_runtime_auth_state(state: &RuntimeSyncAuthState) -> Result<PathBuf, String> {
    let path = runtime_auth_state_path().ok_or_else(|| "home directory unavailable".to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| "invalid auth state path".to_string())?;

    fs::create_dir_all(parent).map_err(|err| format!("failed to create auth state dir: {err}"))?;
    let raw = serde_json::to_string_pretty(state)
        .map_err(|err| format!("failed to serialize auth state: {err}"))?;
    fs::write(&path, raw).map_err(|err| format!("failed to write auth state: {err}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, permissions)
            .map_err(|err| format!("failed to set auth state permissions: {err}"))?;
    }

    Ok(path)
}

pub fn clear_runtime_auth_state() -> Result<(), String> {
    let Some(path) = runtime_auth_state_path() else {
        return Ok(());
    };

    if path.exists() {
        fs::remove_file(path).map_err(|err| format!("failed to remove auth state: {err}"))?;
    }

    Ok(())
}

pub fn runtime_auth_state_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(
        home.join(".openagents")
            .join(AUTH_STATE_FILE_NAME)
            .to_path_buf(),
    )
}

async fn post_json(
    client: &HttpClient,
    base_url: &str,
    path: &str,
    body: Value,
    header: Option<(&str, &str)>,
) -> Result<Value, String> {
    let request_id = format!("desktopauth-{}", Uuid::new_v4().to_string().to_lowercase());
    let mut request = client
        .request(Method::POST, format!("{base_url}{path}"))
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .header("x-request-id", request_id)
        .json(&body);

    if let Some((key, value)) = header {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| format!("auth request failed: {err}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("auth response read failed: {err}"))?;

    if !status.is_success() {
        return Err(auth_error_message(status.as_u16(), &text));
    }

    Ok(serde_json::from_str::<Value>(&text).unwrap_or(Value::Null))
}

fn normalize_base_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("base url must not be empty".to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_email(raw: &str) -> Result<String, String> {
    let normalized = raw.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("email must not be empty".to_string());
    }
    Ok(normalized)
}

fn normalize_code(raw: &str) -> Result<String, String> {
    let code = raw.split_whitespace().collect::<String>();
    if code.is_empty() {
        return Err("verification code must not be empty".to_string());
    }
    Ok(code)
}

fn read_code_from_stdin() -> Result<String, String> {
    print!("Enter email verification code: ");
    io::stdout()
        .flush()
        .map_err(|err| format!("stdout flush failed: {err}"))?;
    let mut code = String::new();
    io::stdin()
        .read_line(&mut code)
        .map_err(|err| format!("stdin read failed: {err}"))?;
    normalize_code(&code)
}

fn auth_error_message(status: u16, body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    parsed
        .get("error")
        .and_then(|value| value.get("message"))
        .and_then(Value::as_str)
        .or_else(|| parsed.get("message").and_then(Value::as_str))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("auth request failed ({status})"))
}
