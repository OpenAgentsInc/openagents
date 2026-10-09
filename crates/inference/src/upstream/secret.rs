//! Keys: where an adapter finds its credential, and a type that never
//! prints it.
//!
//! A key is looked up, in order, from an environment variable, a file
//! named by `<VAR>_FILE` (how Cloud Run mounts a Secret Manager secret as
//! a file), and Secret Manager itself (`projects/<project>/secrets/<name>`,
//! read with the gateway's Google credential; see [`super::google`]).
//! Nothing here logs, and [`Secret`]'s `Debug` and `Display` are redacted.

use std::fmt;
use std::path::PathBuf;

/// A credential. Its text leaves this type only through [`Secret::expose`],
/// for a request header.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// A secret; surrounding whitespace (a file's trailing newline) is
    /// dropped. Empty text is no secret.
    #[must_use]
    pub fn new(text: &str) -> Option<Self> {
        let text = text.trim();
        (!text.is_empty()).then(|| Self(text.to_owned()))
    }

    /// The text, for a request header only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Where one adapter's key lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRef {
    /// Environment variables tried in order, each also as `<VAR>_FILE`.
    pub env: Vec<&'static str>,
    /// The Secret Manager secret, `(project, name)`, read last.
    pub secret_manager: Option<(String, String)>,
}

impl KeyRef {
    /// A key in `vars`, then in Secret Manager's `name` in `project`.
    #[must_use]
    pub fn new(vars: &[&'static str], secret_manager: Option<(&str, &str)>) -> Self {
        Self {
            env: vars.to_vec(),
            secret_manager: secret_manager
                .map(|(project, name)| (project.to_owned(), name.to_owned())),
        }
    }

    /// The key from the environment or a mounted file, without any network
    /// call.
    #[must_use]
    pub fn local(&self) -> Option<Secret> {
        self.local_with(&|name| std::env::var(name).ok())
    }

    /// [`KeyRef::local`] with the environment read through `env`, for
    /// tests.
    #[must_use]
    pub fn local_with(&self, env: &dyn Fn(&str) -> Option<String>) -> Option<Secret> {
        for var in &self.env {
            if let Some(secret) = env(var).as_deref().and_then(Secret::new) {
                return Some(secret);
            }
            if let Some(path) = env(&format!("{var}_FILE")).filter(|path| !path.is_empty())
                && let Ok(text) = std::fs::read_to_string(PathBuf::from(path))
                && let Some(secret) = Secret::new(&text)
            {
                return Some(secret);
            }
        }
        None
    }

    /// The key from the environment, a mounted file, or Secret Manager.
    ///
    /// # Errors
    ///
    /// A sentence when Secret Manager was tried and could not be read; it
    /// names the secret, never its value. `Ok(None)` when no source holds
    /// a key.
    pub async fn resolve(
        &self,
        google: &super::google::TokenSource,
    ) -> Result<Option<Secret>, String> {
        if let Some(secret) = self.local() {
            return Ok(Some(secret));
        }
        let Some((project, name)) = &self.secret_manager else {
            return Ok(None);
        };
        super::google::access_secret(google, project, name).await
    }
}
