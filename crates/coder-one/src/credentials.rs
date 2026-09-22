//! Where Coder One's two credentials come from.
//!
//! - The TypeSafe key for Jev: `TYPESAFE_API_KEY`, or `api_key` in
//!   `~/.openagents/jev.json`.
//! - The OpenAgents bearer for generation: `OPENAGENTS_API_KEY`, or the
//!   session token in `~/.openagents/bearer`.
//!
//! The environment wins over the file. An empty or whitespace-only value
//! reads as unset. No value is ever printed: [`Secret`] displays as `***`,
//! and every message names a variable or a path instead.

use std::fmt;
use std::path::{Path, PathBuf};

/// The TypeSafe door Jev answers on.
pub const JEV_BASE_URL: &str = "https://api.typesafe.ai";

/// The Jev version runs pin, so two runs compare the same model.
pub const JEV_MODEL: &str = "jev-1.13.0";

/// The OpenAgents service that serves generation on `/v1/responses`.
pub const GENERATION_BASE_URL: &str = "https://openagents.com";

const JEV_KEY_VAR: &str = "TYPESAFE_API_KEY";
const JEV_FILE: &str = "jev.json";
const BEARER_VAR: &str = "OPENAGENTS_API_KEY";
const BEARER_FILE: &str = "bearer";

/// A credential value that never displays.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// The value, for the one place that sends it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Where a credential was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Env(&'static str),
    File(PathBuf),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Env(name) => write!(f, "${name}"),
            Source::File(path) => write!(f, "{}", path.display()),
        }
    }
}

/// A resolved credential and where it came from.
#[derive(Debug, Clone)]
pub struct Found {
    pub secret: Secret,
    pub source: Source,
}

/// The TypeSafe key, from the environment or `jev.json` under `dir`.
pub fn jev_key(env: impl Fn(&str) -> Option<String>, dir: &Path) -> Result<Found, String> {
    if let Some(found) = from_env(&env, JEV_KEY_VAR) {
        return Ok(found);
    }
    let path = dir.join(JEV_FILE);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!(
            "no TypeSafe key: set {JEV_KEY_VAR} or put `api_key` in {}",
            path.display()
        )
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| format!("{} is not valid JSON", path.display()))?;
    let key = value
        .get("api_key")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| format!("{} has no `api_key`", path.display()))?;
    Ok(Found {
        secret: Secret(key.to_string()),
        source: Source::File(path),
    })
}

/// The OpenAgents bearer, from the environment or `bearer` under `dir`.
pub fn bearer(env: impl Fn(&str) -> Option<String>, dir: &Path) -> Result<Found, String> {
    if let Some(found) = from_env(&env, BEARER_VAR) {
        return Ok(found);
    }
    let path = dir.join(BEARER_FILE);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!(
            "no OpenAgents bearer: set {BEARER_VAR} or sign in so {} exists",
            path.display()
        )
    })?;
    let token = text.trim();
    if token.is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    Ok(Found {
        secret: Secret(token.to_string()),
        source: Source::File(path),
    })
}

/// A Jev client on the pinned model, authenticated with `key`.
pub fn jev_client(key: &Secret) -> Result<jev::Client, String> {
    jev::Client::new(
        jev::Config::new()
            .api_key(key.expose())
            .base_url(JEV_BASE_URL)
            .default_model(JEV_MODEL),
    )
    .map_err(|error| format!("cannot build the Jev client: {error}"))
}

/// `~/.openagents`, the directory both credential files live in.
pub fn openagents_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents"))
}

fn from_env(env: &impl Fn(&str) -> Option<String>, name: &'static str) -> Option<Found> {
    let value = env(name)?;
    let value = value.trim();
    (!value.is_empty()).then(|| Found {
        secret: Secret(value.to_string()),
        source: Source::Env(name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("coder-one-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let dir = scratch("env-wins");
        std::fs::write(dir.join("bearer"), "from-file\n").expect("write");
        let env = |name: &str| (name == BEARER_VAR).then(|| "from-env".to_string());

        let found = bearer(env, &dir).expect("bearer");
        assert_eq!(found.secret.expose(), "from-env");
        assert_eq!(found.source, Source::Env(BEARER_VAR));
    }

    #[test]
    fn files_are_read_and_blank_values_are_unset() {
        let dir = scratch("files");
        std::fs::write(dir.join("bearer"), "token\n").expect("write");
        std::fs::write(dir.join("jev.json"), r#"{"api_key":" key "}"#).expect("write");
        let blank = |_: &str| Some("   ".to_string());

        assert_eq!(
            bearer(blank, &dir).expect("bearer").secret.expose(),
            "token"
        );
        let key = jev_key(no_env, &dir).expect("key");
        assert_eq!(key.secret.expose(), "key");
        assert_eq!(key.source, Source::File(dir.join("jev.json")));
    }

    #[test]
    fn missing_credentials_name_a_variable_and_a_path_but_no_value() {
        let dir = scratch("missing");
        std::fs::write(dir.join("jev.json"), r#"{"model":"jev-latest"}"#).expect("write");

        let error = jev_key(no_env, &dir).expect_err("no key");
        assert!(error.contains("api_key"), "{error}");
        let error = bearer(no_env, &dir).expect_err("no bearer");
        assert!(error.contains(BEARER_VAR), "{error}");
    }

    #[test]
    fn a_secret_never_formats() {
        let secret = Secret("sk-live-value".to_string());
        assert_eq!(format!("{secret:?}"), "***");
        let found = Found {
            secret,
            source: Source::Env(JEV_KEY_VAR),
        };
        assert!(!format!("{found:?}").contains("sk-live-value"));
    }
}
