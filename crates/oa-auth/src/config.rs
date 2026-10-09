//! GitHub OAuth App settings and the private credentials file.
//!
//! The credentials file is the shape the owner keeps under
//! `~/work/.secrets/github-oauth-<env>.json`:
//! `{"client_id", "client_secret", "token_encryption_key"}`. It must be
//! readable by its owner only. The web server needs only the client id
//! ([`GithubApp`]); the account service also needs the secret
//! ([`GithubCredentials`]). Neither is ever printed: `Debug` redacts.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;

/// The callback path every deployment registers with its OAuth App.
pub const CALLBACK_PATH: &str = "/auth/github/callback";

/// The scopes P1 requests: the profile and every email address.
pub const SCOPES: [&str; 2] = ["read:user", "user:email"];

/// The scopes connecting repositories asks for when the person wants their
/// private repositories too (and those of their organizations). Asked only
/// at that moment, never at sign-in.
pub const PRIVATE_REPO_SCOPES: [&str; 3] = ["read:user", "repo", "read:org"];

/// The scopes connecting public repositories only asks for: nothing more
/// than sign-in already has, so GitHub shows no new permission screen.
pub const PUBLIC_REPO_SCOPES: [&str; 1] = ["read:user"];

/// Where GitHub (or the fake) answers. Defaults are GitHub's.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Endpoints {
    #[serde(default = "default_authorize")]
    pub authorize_url: String,
    #[serde(default = "default_token")]
    pub token_url: String,
    #[serde(default = "default_api")]
    pub api_url: String,
    /// github.com itself, where a GitHub App's install page lives.
    #[serde(default = "default_web")]
    pub web_url: String,
}

fn default_authorize() -> String {
    "https://github.com/login/oauth/authorize".into()
}
fn default_token() -> String {
    "https://github.com/login/oauth/access_token".into()
}
fn default_api() -> String {
    "https://api.github.com".into()
}
fn default_web() -> String {
    "https://github.com".into()
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            authorize_url: default_authorize(),
            token_url: default_token(),
            api_url: default_api(),
            web_url: default_web(),
        }
    }
}

impl Endpoints {
    /// The endpoints of a fake GitHub served at `origin` (`http://127.0.0.1:port`).
    #[must_use]
    pub fn at(origin: &str) -> Self {
        let origin = origin.trim_end_matches('/');
        Self {
            authorize_url: format!("{origin}/login/oauth/authorize"),
            token_url: format!("{origin}/login/oauth/access_token"),
            api_url: origin.to_string(),
            web_url: origin.to_string(),
        }
    }

    fn check(&self) -> Result<(), String> {
        for value in [
            &self.authorize_url,
            &self.token_url,
            &self.api_url,
            &self.web_url,
        ] {
            web_url(value)?;
        }
        Ok(())
    }
}

/// The public half of an OAuth App: what the web server needs to send a
/// browser to GitHub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GithubApp {
    pub client_id: String,
    /// The exact callback URL registered with the OAuth App.
    pub redirect_url: String,
    pub endpoints: Endpoints,
}

impl GithubApp {
    pub fn new(client_id: &str, redirect_url: &str, endpoints: Endpoints) -> Result<Self, String> {
        if client_id.is_empty()
            || client_id.len() > 128
            || !client_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        {
            return Err("The GitHub client id is malformed.".into());
        }
        let redirect = web_url(redirect_url)?;
        if redirect.path() != CALLBACK_PATH || redirect.query().is_some() {
            return Err(format!(
                "The GitHub redirect URL must end in {CALLBACK_PATH} with no query."
            ));
        }
        endpoints.check()?;
        Ok(Self {
            client_id: client_id.into(),
            redirect_url: redirect_url.into(),
            endpoints,
        })
    }

    /// Read the client id from the private credentials file.
    pub fn load(path: &Path, redirect_url: &str, endpoints: Endpoints) -> Result<Self, String> {
        let file = read_private(path)?;
        Self::new(&file.client_id, redirect_url, endpoints)
    }
}

/// The whole OAuth App, secret included, for the account service.
#[derive(Clone)]
pub struct GithubCredentials {
    pub app: GithubApp,
    client_secret: String,
    token_key: [u8; 32],
}

impl std::fmt::Debug for GithubCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubCredentials")
            .field("app", &self.app)
            .field("client_secret", &"[redacted]")
            .field("token_key", &"[redacted]")
            .finish()
    }
}

impl Drop for GithubCredentials {
    fn drop(&mut self) {
        self.token_key.fill(0);
    }
}

impl GithubCredentials {
    /// Build from parts (tests and the fake).
    pub fn new(app: GithubApp, client_secret: &str, token_key: [u8; 32]) -> Result<Self, String> {
        if client_secret.is_empty() || client_secret.len() > 256 {
            return Err("The GitHub client secret is malformed.".into());
        }
        Ok(Self {
            app,
            client_secret: client_secret.into(),
            token_key,
        })
    }

    /// Read the private credentials file.
    pub fn load(path: &Path, redirect_url: &str, endpoints: Endpoints) -> Result<Self, String> {
        let file = read_private(path)?;
        let key = STANDARD
            .decode(file.token_encryption_key.trim())
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
            .ok_or("The GitHub token encryption key must be 32 bytes of base64.")?;
        let app = GithubApp::new(&file.client_id, redirect_url, endpoints)?;
        Self::new(app, &file.client_secret, key)
    }

    pub(crate) fn secret(&self) -> &str {
        &self.client_secret
    }

    /// Whether the key stored GitHub tokens are encrypted under is set.
    #[must_use]
    pub fn has_token_key(&self) -> bool {
        self.token_key != [0; 32]
    }

    /// The key a stored repository-access token is encrypted under
    /// (AES-256-GCM, [`crate::repos`]). Never leaves this crate.
    pub(crate) fn token_key(&self) -> &[u8; 32] {
        &self.token_key
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    client_id: String,
    client_secret: String,
    token_encryption_key: String,
}

fn read_private(path: &Path) -> Result<File, String> {
    let bytes = read_private_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|_| "The GitHub credentials file is malformed.".into())
}

/// A small regular file readable by its owner only (the OAuth App's and
/// the GitHub App's private files, and the App's private key).
pub(crate) fn read_private_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| "The GitHub credentials file is unavailable.")?;
    if !meta.is_file() || meta.len() > 16 * 1024 {
        return Err("The GitHub credentials file must be a small regular file.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(
                "The GitHub credentials file must be readable by its owner only (chmod 600)."
                    .into(),
            );
        }
    }
    std::fs::read(path).map_err(|_| "The GitHub credentials file is unavailable.".into())
}

/// An `https` URL, or `http` on a literal loopback address.
pub(crate) fn web_url(value: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(value).map_err(|_| format!("`{value}` is not a URL."))?;
    let loopback = match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) => name == "localhost",
        None => false,
    };
    if !(parsed.scheme() == "https" || parsed.scheme() == "http" && loopback)
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(format!(
            "`{value}` must be https, or http on a loopback address."
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    fn write(dir: &Path, mode: u32, body: &str) -> std::path::PathBuf {
        let path = dir.join(format!("creds-{mode:o}.json"));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&path)
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        path
    }

    #[test]
    fn reads_the_owner_file_shape_and_refuses_loose_modes() {
        let dir = tempfile::tempdir().unwrap();
        let body = format!(
            r#"{{"client_id":"Ov23liAbc","client_secret":"s3cr3t","token_encryption_key":"{}"}}"#,
            STANDARD.encode([7u8; 32])
        );
        let good = write(dir.path(), 0o600, &body);
        let redirect = "http://127.0.0.1:4301/auth/github/callback";
        let creds = GithubCredentials::load(&good, redirect, Endpoints::default()).unwrap();
        assert_eq!(creds.app.client_id, "Ov23liAbc");
        assert!(creds.has_token_key());
        assert!(!format!("{creds:?}").contains("s3cr3t"));
        let app = GithubApp::load(&good, redirect, Endpoints::default()).unwrap();
        assert_eq!(app.redirect_url, redirect);

        let loose = write(dir.path(), 0o644, &body);
        assert!(GithubApp::load(&loose, redirect, Endpoints::default()).is_err());
        assert!(
            GithubApp::load(
                &good,
                "http://evil.example/auth/github/callback",
                Endpoints::default()
            )
            .is_err()
        );
        assert!(
            GithubApp::load(
                &good,
                "https://openagents.com/elsewhere",
                Endpoints::default()
            )
            .is_err()
        );
        assert!(
            GithubApp::load(
                &good,
                "https://openagents.com/auth/github/callback",
                Endpoints::default()
            )
            .is_ok()
        );
    }
}
