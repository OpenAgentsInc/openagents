//! Process configuration. Secrets are read from the environment (and
//! optionally a dotenv file) and never logged.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub db_path: PathBuf,
    pub public_origin: String,
    pub ai_gateway_api_key: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub credit_allowance_microusd: i64,
    /// When true, `/api/*` requires a Bearer token. `/health` stays open.
    /// Cloud Run sets this so an unsigned public host cannot mint grants.
    pub require_bearer: bool,
    /// Shared secret Phoenix presents when reverse-proxying an already-admitted
    /// inference call. Empty means no internal hop is accepted.
    pub internal_token: Option<String>,
    /// Phoenix origin used to introspect `chat:account` bearers and to read
    /// account credit. Empty keeps the local stub principal and $20 envelope.
    pub identity_origin: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let on_cloud_run = std::env::var_os("K_SERVICE").is_some();
        if !on_cloud_run {
            load_dotenv_quietly();
        }
        let bind = std::env::var("OPENAGENTS_CODER_API_BIND")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var("PORT")
                    .ok()
                    .filter(|value| !value.is_empty())
                    .map(|port| format!("0.0.0.0:{port}"))
            })
            .unwrap_or_else(|| "127.0.0.1:4000".to_string());
        let public_origin = std::env::var("OPENAGENTS_CODER_API_ORIGIN")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if on_cloud_run {
                    "https://coder.openagents.com".to_string()
                } else {
                    format!("http://{bind}")
                }
            });
        let db_path = std::env::var("OPENAGENTS_CODER_API_DB")
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| default_db_path(on_cloud_run));
        let require_bearer = on_cloud_run
            || std::env::var("OPENAGENTS_CODER_API_REQUIRE_BEARER")
                .ok()
                .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
        Self {
            bind,
            db_path,
            public_origin,
            ai_gateway_api_key: first_env(&["AI_GATEWAY_API_KEY", "VERCEL_GATEWAY_API_KEY"]),
            openrouter_api_key: first_env(&["OPENROUTER_API_KEY"]),
            credit_allowance_microusd: std::env::var("OPENAGENTS_CODER_API_ALLOWANCE_MICROUSD")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(20_000_000),
            require_bearer,
            internal_token: first_env(&["OPENAGENTS_CODER_API_INTERNAL_TOKEN"]),
            identity_origin: first_env(&["OPENAGENTS_IDENTITY_ORIGIN"])
                .or_else(|| on_cloud_run.then(|| "https://openagents.com".to_string())),
        }
    }

    pub fn vercel_configured(&self) -> bool {
        self.ai_gateway_api_key
            .as_ref()
            .is_some_and(|key| !key.is_empty())
    }

    pub fn openrouter_configured(&self) -> bool {
        self.openrouter_api_key
            .as_ref()
            .is_some_and(|key| !key.is_empty())
    }

    pub fn internal_token_matches(&self, presented: &str) -> bool {
        let Some(expected) = self.internal_token.as_deref() else {
            return false;
        };
        if expected.is_empty() || expected.len() != presented.len() {
            return false;
        }
        expected
            .bytes()
            .zip(presented.bytes())
            .fold(0u8, |acc, (left, right)| acc | (left ^ right))
            == 0
    }
}

fn first_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn default_db_path(on_cloud_run: bool) -> PathBuf {
    if on_cloud_run {
        return PathBuf::from("/tmp/openagents-coder-api/state.sqlite");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".openagents")
        .join("coder-api")
        .join("state.sqlite")
}

/// Load KEY=VALUE lines from well-known env files without printing values.
fn load_dotenv_quietly() {
    if let Ok(path) = std::env::var("OPENAGENTS_CODER_API_ENV") {
        load_env_file(Path::new(&path));
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        load_env_file(&home.join(".openagents").join("coder-api.env"));
        // Same names Phoenix reads (`config/runtime.exs`).
        load_env_file(&home.join("work").join("openagents.com").join(".env"));
    }
    if let Ok(web) = std::env::var("OPENAGENTS_WEB_REPO") {
        load_env_file(&PathBuf::from(web).join(".env"));
    }
    if let Ok(dir) = std::env::current_dir() {
        load_env_file(&dir.join(".env"));
        load_env_file(&dir.join("../openagents.com/.env"));
    }
}

fn load_env_file(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key
            .trim()
            .strip_prefix("export ")
            .map(str::trim)
            .unwrap_or_else(|| key.trim());
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if key.is_empty() || std::env::var_os(key).is_some() {
            continue;
        }
        // SAFETY: process start, before the serve loop shares the env.
        unsafe { std::env::set_var(key, value) };
    }
}
