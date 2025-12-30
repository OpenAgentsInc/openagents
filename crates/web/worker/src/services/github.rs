//! GitHub API client

use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::*;

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

/// Exchange OAuth code for access token
pub async fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<TokenResponse> {
    let body = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret,
        "code": code
    });

    let mut headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Content-Type", "application/json")?;
    headers.set("User-Agent", "OpenAgents-Worker/1.0")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body.to_string())));

    let request = Request::new_with_init(
        "https://github.com/login/oauth/access_token",
        &init,
    )?;

    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse token response: {} - {}", e, text)))
}

/// Get authenticated user info
pub async fn get_user(access_token: &str) -> Result<GitHubUser> {
    let mut headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", access_token))?;
    headers.set("User-Agent", "OpenAgents-Worker/1.0")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let request = Request::new_with_init("https://api.github.com/user", &init)?;

    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse user response: {} - {}", e, text)))
}

/// Get user's emails
pub async fn get_emails(access_token: &str) -> Result<Vec<GitHubEmail>> {
    let mut headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", access_token))?;
    headers.set("User-Agent", "OpenAgents-Worker/1.0")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let request = Request::new_with_init("https://api.github.com/user/emails", &init)?;

    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse emails response: {} - {}", e, text)))
}

/// Get user's repositories
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub description: Option<String>,
    pub default_branch: String,
}

pub async fn get_repos(access_token: &str) -> Result<Vec<GitHubRepo>> {
    let mut headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", access_token))?;
    headers.set("User-Agent", "OpenAgents-Worker/1.0")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let request = Request::new_with_init(
        "https://api.github.com/user/repos?sort=pushed&per_page=10",
        &init,
    )?;

    let mut response = Fetch::Request(request).send().await?;
    let text = response.text().await?;

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse repos response: {} - {}", e, text)))
}
