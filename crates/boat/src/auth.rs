//! Where the Boat API key comes from, and the type that keeps it out of output.

use std::fmt;

use crate::{Error, Result};

/// The environment variable that holds a `boat_…` API key.
pub const API_KEY_ENV: &str = "BOAT_API_KEY";
/// The environment variable that overrides [`crate::BASE_URL`].
pub const API_BASE_ENV: &str = "BOAT_API_BASE";
/// `BOAT_HOSTED=1` opts in to Boat's hosted API at boat.dev: the default
/// base becomes [`crate::HOSTED_BASE_URL`] and the key comes from
/// [`HOSTED_SECRET_NAME`]. Without it a client refuses a boat.dev base.
pub const HOSTED_ENV: &str = "BOAT_HOSTED";
/// The Secret Manager secret that holds our own service's token.
pub const SECRET_NAME: &str = "oa-boat-api-key";
/// The Secret Manager secret that holds the hosted boat.dev key.
pub const HOSTED_SECRET_NAME: &str = "boat-api-key";
/// The project the automation service account reads the secrets from.
pub const SECRET_PROJECT: &str = "openagentsgemini";

/// A Boat API key. `Debug` and `Display` never print it, and it has no
/// `Serialize`, so it cannot reach logs, errors, traces or fixtures by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// Wrap a key. Surrounding whitespace (a trailing newline from a secret
    /// file) is trimmed; an empty key is refused.
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let key = key.into();
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return Err(Error::Configuration("The Boat API key is empty."));
        }
        if trimmed.bytes().any(|b| b.is_ascii_control() || b == b' ') {
            return Err(Error::Configuration("The Boat API key is malformed."));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Read `BOAT_API_KEY` from the environment.
    pub fn from_env() -> Result<Self> {
        match std::env::var(API_KEY_ENV) {
            Ok(value) => Self::new(value),
            Err(_) => Err(Error::Configuration("BOAT_API_KEY is not set.")),
        }
    }

    /// Read the latest version of a Secret Manager secret through `gcloud`.
    ///
    /// The command's output is never logged and its error text is discarded,
    /// so a failure reports only that the secret could not be read.
    pub async fn from_secret_manager(project: &str, secret: &str) -> Result<Self> {
        let output = tokio::process::Command::new("gcloud")
            .args([
                "secrets",
                "versions",
                "access",
                "latest",
                "--secret",
                secret,
                "--project",
                project,
            ])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await
            .map_err(|_| Error::Configuration("Cannot run gcloud to read the Boat API key."))?;
        if !output.status.success() {
            return Err(Error::Configuration(
                "Cannot read the Boat API key from Secret Manager.",
            ));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| Error::Configuration("The Boat API key is malformed."))?;
        Self::new(text)
    }

    /// `BOAT_API_KEY` when set, otherwise Secret Manager `oa-boat-api-key`
    /// (or `boat-api-key` with `BOAT_HOSTED=1`) in [`SECRET_PROJECT`].
    pub async fn resolve() -> Result<Self> {
        match Self::from_env() {
            Ok(key) => Ok(key),
            Err(_) if std::env::var_os(API_KEY_ENV).is_none() => {
                let secret = if hosted() {
                    HOSTED_SECRET_NAME
                } else {
                    SECRET_NAME
                };
                Self::from_secret_manager(SECRET_PROJECT, secret).await
            }
            Err(error) => Err(error),
        }
    }

    /// Whether the key has Boat's `boat_` prefix. Legacy Box keys may not.
    pub fn is_boat_key(&self) -> bool {
        self.0.starts_with("boat_")
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

/// Whether hosted Boat (boat.dev) is opted in: `BOAT_HOSTED=1`.
pub fn hosted() -> bool {
    std::env::var(HOSTED_ENV).is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Whether `host` is Boat's hosted service (boat.dev or the old ascii.dev).
pub fn is_hosted_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    ["boat.dev", "ascii.dev"]
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}
