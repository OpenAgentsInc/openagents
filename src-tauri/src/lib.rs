// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::Serialize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use base64::Engine as _;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiAuthStatus {
    /// "ChatGPT" | "ApiKey" or None if not authenticated
    pub method: Option<String>,
    /// Email parsed from ChatGPT id_token (if present)
    pub email: Option<String>,
    /// Plan type parsed from ChatGPT id_token (e.g., Free, Plus, Pro, ...)
    pub plan: Option<String>,
}

#[derive(Deserialize)]
struct AuthJson {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    #[serde(default)]
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct IdClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<AuthClaims>,
}

#[derive(Deserialize)]
struct AuthClaims {
    #[serde(default)]
    chatgpt_plan_type: Option<serde_json::Value>,
}

fn default_codex_home() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return PathBuf::from(val).canonicalize().ok();
        }
    }
    dirs::home_dir().map(|mut h| {
        h.push(".codex");
        h
    })
}

fn parse_plan_type(val: Option<serde_json::Value>) -> Option<String> {
    match val {
        Some(serde_json::Value::String(s)) => Some(s),
        Some(serde_json::Value::Object(map)) => map.get("Known").and_then(|v| v.as_str().map(|s| s.to_string())),
        _ => None,
    }
}

fn parse_id_token_info(id_token: &str) -> (Option<String>, Option<String>) {
    let mut parts = id_token.split('.');
    let (_h, p, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => return (None, None),
    };

    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(p)
        .ok();
    let Some(bytes) = decoded else { return (None, None) };
    let claims: IdClaims = match serde_json::from_slice(&bytes) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let email = claims.email;
    let plan = parse_plan_type(claims.auth.and_then(|a| a.chatgpt_plan_type));
    (email, plan)
}

#[tauri::command]
async fn get_auth_status() -> UiAuthStatus {
    let codex_home = match default_codex_home() {
        Some(p) => p,
        None => return UiAuthStatus::default(),
    };
    let auth_file = codex_home.join("auth.json");
    let contents = match fs::read_to_string(&auth_file) {
        Ok(c) => c,
        Err(_) => return UiAuthStatus::default(),
    };

    let parsed: AuthJson = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return UiAuthStatus::default(),
    };

    if let Some(api_key) = parsed.openai_api_key {
        if !api_key.is_empty() {
            return UiAuthStatus {
                method: Some("ApiKey".to_string()),
                email: None,
                plan: None,
            };
        }
    }

    if let Some(tokens) = parsed.tokens {
        let (email, plan) = parse_id_token_info(&tokens.id_token);
        return UiAuthStatus {
            method: Some("ChatGPT".to_string()),
            email,
            plan,
        };
    }

    UiAuthStatus::default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_auth_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
