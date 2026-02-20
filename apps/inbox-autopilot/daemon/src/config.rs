use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub db_path: PathBuf,
    pub data_dir: PathBuf,
    pub session_ttl_seconds: i64,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_scopes: String,
    pub openai_base_url: String,
    pub openai_model: String,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = env::var("INBOX_AUTOPILOT_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut default_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                default_dir.push(".inbox-autopilot");
                default_dir
            });

        let db_path = env::var("INBOX_AUTOPILOT_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = data_dir.clone();
                path.push("daemon.sqlite");
                path
            });

        Self {
            bind_addr: env::var("INBOX_AUTOPILOT_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8787".to_string()),
            db_path,
            data_dir,
            session_ttl_seconds: env::var("INBOX_AUTOPILOT_SESSION_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(300),
            google_client_id: env::var("GOOGLE_OAUTH_CLIENT_ID").ok(),
            google_client_secret: env::var("GOOGLE_OAUTH_CLIENT_SECRET").ok(),
            google_scopes: env::var("GOOGLE_OAUTH_SCOPES").unwrap_or_else(|_| {
                "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose https://www.googleapis.com/auth/gmail.send"
                    .to_string()
            }),
            openai_base_url: env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            openai_model: env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
        }
    }
}
