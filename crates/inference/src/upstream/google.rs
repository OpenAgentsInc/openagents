//! Google credentials: OAuth access tokens for Vertex AI and Secret
//! Manager.
//!
//! A [`TokenSource`] is one of:
//!
//! - a service-account key file (`GOOGLE_APPLICATION_CREDENTIALS`, or the
//!   path given): an RS256 JWT assertion exchanged at the key's
//!   `token_uri`, the same flow `push-gateway` uses for FCM, reimplemented
//!   here;
//! - the metadata server, on Cloud Run and GCE;
//! - a fixed token from `VERTEX_ACCESS_TOKEN` or the file
//!   `VERTEX_TOKEN_FILE` (re-read on every call), for an operator's shell.
//!
//! Tokens are cached until a minute before they expire. Nothing here logs,
//! and no error carries a token or key.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::secret::Secret;

/// The scope Vertex AI and Secret Manager both accept.
pub const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

/// The metadata server's token URL.
pub const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

/// Secret Manager's API root.
pub const SECRET_MANAGER_URL: &str = "https://secretmanager.googleapis.com";

#[derive(Deserialize)]
struct ServiceAccountFile {
    client_email: String,
    private_key: String,
    #[serde(default)]
    private_key_id: Option<String>,
    token_uri: String,
}

struct ServiceAccount {
    email: String,
    key_id: Option<String>,
    key: RsaKeyPair,
    token_uri: String,
}

enum Kind {
    ServiceAccount(ServiceAccount),
    Metadata(String),
    Fixed(Secret),
    File(PathBuf),
    None,
}

/// Where Google access tokens come from.
#[derive(Clone)]
pub struct TokenSource {
    kind: Arc<Kind>,
    cached: Arc<Mutex<Option<(Secret, u64)>>>,
    http: reqwest::Client,
    secret_manager_url: String,
}

impl std::fmt::Debug for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match &*self.kind {
            Kind::ServiceAccount(_) => "service_account",
            Kind::Metadata(_) => "metadata",
            Kind::Fixed(_) => "fixed",
            Kind::File(_) => "file",
            Kind::None => "none",
        };
        f.debug_struct("TokenSource").field("kind", &kind).finish()
    }
}

impl TokenSource {
    fn with(kind: Kind) -> Self {
        Self {
            kind: Arc::new(kind),
            cached: Arc::new(Mutex::new(None)),
            http: super::http::client(Duration::from_secs(10)),
            secret_manager_url: SECRET_MANAGER_URL.to_owned(),
        }
    }

    /// No credential: every token request fails, and an adapter built on
    /// it reports itself unconfigured.
    #[must_use]
    pub fn none() -> Self {
        Self::with(Kind::None)
    }

    /// A fixed access token.
    #[must_use]
    pub fn fixed(token: Secret) -> Self {
        Self::with(Kind::Fixed(token))
    }

    /// A token re-read from `path` on every call.
    #[must_use]
    pub fn file(path: PathBuf) -> Self {
        Self::with(Kind::File(path))
    }

    /// The metadata server at `url` (normally [`METADATA_TOKEN_URL`]).
    #[must_use]
    pub fn metadata(url: &str) -> Self {
        Self::with(Kind::Metadata(url.to_owned()))
    }

    /// A service-account key file.
    ///
    /// # Errors
    ///
    /// A sentence when the file is unreadable or not an RSA service-account
    /// key; never the key's contents.
    pub fn service_account(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|_| format!("cannot read the service account key {}", path.display()))?;
        Self::service_account_json(&text)
    }

    /// A service account from its key file's JSON text.
    ///
    /// # Errors
    ///
    /// As [`TokenSource::service_account`].
    pub fn service_account_json(text: &str) -> Result<Self, String> {
        let file: ServiceAccountFile = serde_json::from_str(text)
            .map_err(|_| "the service account file is not a service account key".to_owned())?;
        let der = pem_der(&file.private_key)?;
        let key = RsaKeyPair::from_pkcs8(&der)
            .map_err(|_| "the service account key is not an RSA PKCS#8 key".to_owned())?;
        Ok(Self::with(Kind::ServiceAccount(ServiceAccount {
            email: file.client_email,
            key_id: file.private_key_id,
            key,
            token_uri: file.token_uri,
        })))
    }

    /// The source the environment names: `VERTEX_ACCESS_TOKEN`, then
    /// `VERTEX_TOKEN_FILE`, then `GOOGLE_APPLICATION_CREDENTIALS`, then the
    /// metadata server when `K_SERVICE` (Cloud Run) is set, else none.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        if let Some(token) = var("VERTEX_ACCESS_TOKEN").as_deref().and_then(Secret::new) {
            return Self::fixed(token);
        }
        if let Some(path) = var("VERTEX_TOKEN_FILE") {
            return Self::file(PathBuf::from(path));
        }
        if let Some(path) = var("GOOGLE_APPLICATION_CREDENTIALS")
            && let Ok(source) = Self::service_account(Path::new(&path))
        {
            return source;
        }
        if var("K_SERVICE").is_some() || var("GCE_METADATA_HOST").is_some() {
            return Self::metadata(METADATA_TOKEN_URL);
        }
        Self::none()
    }

    /// The same source with Secret Manager at another root, for tests.
    #[must_use]
    pub fn secret_manager_url(mut self, url: &str) -> Self {
        self.secret_manager_url = url.trim_end_matches('/').to_owned();
        self
    }

    /// Whether this source can produce a token at all.
    #[must_use]
    pub fn present(&self) -> bool {
        !matches!(*self.kind, Kind::None)
    }

    /// Drops the cached token, after an upstream rejected it.
    pub async fn forget(&self) {
        *self.cached.lock().await = None;
    }

    /// An access token.
    ///
    /// # Errors
    ///
    /// A sentence when no token can be had; never a token or key.
    pub async fn token(&self) -> Result<Secret, String> {
        let now = now();
        let mut cached = self.cached.lock().await;
        if let Some((token, until)) = cached.as_ref()
            && now < *until
        {
            return Ok(token.clone());
        }
        let (token, lifetime) = match &*self.kind {
            Kind::None => return Err("no Google credential is configured".to_owned()),
            Kind::Fixed(token) => return Ok(token.clone()),
            Kind::File(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|_| format!("cannot read the token file {}", path.display()))?;
                return Secret::new(&text)
                    .ok_or_else(|| format!("the token file {} is empty", path.display()));
            }
            Kind::Metadata(url) => {
                let response = self
                    .http
                    .get(url)
                    .header("Metadata-Flavor", "Google")
                    .send()
                    .await
                    .map_err(|_| "the metadata server did not answer".to_owned())?;
                read_token(response).await?
            }
            Kind::ServiceAccount(account) => {
                let assertion = account.assertion(now)?;
                let response = self
                    .http
                    .post(&account.token_uri)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(format!(
                        "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}"
                    ))
                    .send()
                    .await
                    .map_err(|_| "the Google token endpoint did not answer".to_owned())?;
                read_token(response).await?
            }
        };
        *cached = Some((token.clone(), now + lifetime.clamp(120, 3_600) - 60));
        Ok(token)
    }
}

impl ServiceAccount {
    fn assertion(&self, now: u64) -> Result<String, String> {
        let mut header = json!({"alg": "RS256", "typ": "JWT"});
        if let Some(key_id) = &self.key_id {
            header["kid"] = json!(key_id);
        }
        let claims = json!({
            "iss": self.email,
            "scope": SCOPE,
            "aud": self.token_uri,
            "iat": now,
            "exp": now + 3_600,
        });
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let mut signature = vec![0; self.key.public().modulus_len()];
        self.key
            .sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                input.as_bytes(),
                &mut signature,
            )
            .map_err(|_| "signing the service account assertion failed".to_owned())?;
        Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature)))
    }
}

async fn read_token(response: reqwest::Response) -> Result<(Secret, u64), String> {
    #[derive(Deserialize)]
    struct Token {
        access_token: String,
        #[serde(default)]
        expires_in: Option<u64>,
    }
    let status = response.status().as_u16();
    if status != 200 {
        return Err(format!("the Google token endpoint answered HTTP {status}"));
    }
    let token: Token = response
        .json()
        .await
        .map_err(|_| "the Google token endpoint sent no token".to_owned())?;
    let secret = Secret::new(&token.access_token)
        .ok_or_else(|| "the Google token endpoint sent an empty token".to_owned())?;
    Ok((secret, token.expires_in.unwrap_or(3_600)))
}

/// Reads the latest version of the secret `name` in `project`.
///
/// # Errors
///
/// A sentence naming the secret when it cannot be read; never its value.
pub async fn access_secret(
    google: &TokenSource,
    project: &str,
    name: &str,
) -> Result<Option<Secret>, String> {
    if !google.present() {
        return Ok(None);
    }
    let token = google.token().await?;
    let url = format!(
        "{}/v1/projects/{project}/secrets/{name}/versions/latest:access",
        google.secret_manager_url
    );
    let response = google
        .http
        .get(&url)
        .bearer_auth(token.expose())
        .send()
        .await
        .map_err(|_| format!("Secret Manager did not answer for {name}"))?;
    let status = response.status().as_u16();
    if status == 404 {
        return Ok(None);
    }
    if status != 200 {
        return Err(format!("Secret Manager answered HTTP {status} for {name}"));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| format!("Secret Manager sent no payload for {name}"))?;
    let data = body["payload"]["data"]
        .as_str()
        .ok_or_else(|| format!("Secret Manager sent no payload for {name}"))?;
    let bytes = STANDARD
        .decode(data)
        .map_err(|_| format!("the payload of {name} is not base64"))?;
    let text =
        String::from_utf8(bytes).map_err(|_| format!("the payload of {name} is not text"))?;
    Ok(Secret::new(&text))
}

fn pem_der(pem: &str) -> Result<Vec<u8>, String> {
    const BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
    const END: &str = "-----END PRIVATE KEY-----";
    let start = pem
        .find(BEGIN)
        .ok_or("the service account key has no PEM block")?
        + BEGIN.len();
    let stop = pem[start..]
        .find(END)
        .ok_or("the service account key's PEM block is unterminated")?
        + start;
    let body: String = pem[start..stop]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    STANDARD
        .decode(body)
        .map_err(|_| "the service account key's PEM block is not base64".to_owned())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}
