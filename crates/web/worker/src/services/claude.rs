//! Claude/Anthropic OAuth service
//!
//! Implements OAuth 2.0 with PKCE for Claude Pro/Max authentication.
//! Based on the opencode-anthropic-auth implementation.

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

/// Claude OAuth endpoints
pub const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
pub const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
pub const CREATE_API_KEY_URL: &str = "https://api.anthropic.com/api/oauth/claude_cli/create_api_key";

/// OAuth token response from Anthropic
#[derive(Debug, Deserialize, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// PKCE challenge pair (verifier + challenge)
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

/// Generate a PKCE challenge using S256 method
pub fn generate_pkce() -> PkceChallenge {
    // Generate random verifier (43-128 chars)
    let verifier: String = (0..64)
        .map(|_| {
            let idx = (js_sys::Math::random() * 66.0) as usize;
            // A-Z, a-z, 0-9, -, ., _, ~
            const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
            CHARSET[idx % CHARSET.len()] as char
        })
        .collect();

    // Create S256 challenge: base64url(sha256(verifier))
    let challenge = sha256_base64url(&verifier);

    PkceChallenge { verifier, challenge }
}

/// SHA256 + base64url encode (for PKCE S256)
fn sha256_base64url(input: &str) -> String {
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();

    // Base64url encode (no padding)
    base64_url_encode(&result)
}

/// Base64url encode without padding
fn base64_url_encode(data: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(data)
}

/// Exchange authorization code for tokens (with PKCE)
pub async fn exchange_code(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
    client_id: &str,
) -> Result<TokenResponse> {
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        urlencoding::encode(code),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(client_id),
        urlencoding::encode(code_verifier)
    );

    let mut headers = Headers::new();
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;
    headers.set("Accept", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body)));

    let request = Request::new_with_init(TOKEN_URL, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    // Check for error response
    if response.status_code() >= 400 {
        return Err(Error::RustError(format!(
            "Token exchange failed ({}): {}",
            response.status_code(),
            text
        )));
    }

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse token response: {} - {}", e, text)))
}

/// Refresh an access token
pub async fn refresh_token(
    refresh_token: &str,
    client_id: &str,
) -> Result<TokenResponse> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        urlencoding::encode(refresh_token),
        urlencoding::encode(client_id)
    );

    let mut headers = Headers::new();
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;
    headers.set("Accept", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body)));

    let request = Request::new_with_init(TOKEN_URL, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    if response.status_code() >= 400 {
        return Err(Error::RustError(format!(
            "Token refresh failed ({}): {}",
            response.status_code(),
            text
        )));
    }

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse refresh response: {} - {}", e, text)))
}

/// Create an API key from OAuth tokens
pub async fn create_api_key(access_token: &str, name: &str) -> Result<String> {
    let body = serde_json::json!({
        "name": name
    });

    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", access_token))?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body.to_string())));

    let request = Request::new_with_init(CREATE_API_KEY_URL, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    if response.status_code() >= 400 {
        return Err(Error::RustError(format!(
            "API key creation failed ({}): {}",
            response.status_code(),
            text
        )));
    }

    #[derive(Deserialize)]
    struct ApiKeyResponse {
        key: String,
    }

    let resp: ApiKeyResponse = serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse API key response: {} - {}", e, text)))?;

    Ok(resp.key)
}
