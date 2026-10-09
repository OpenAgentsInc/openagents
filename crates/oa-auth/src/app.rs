//! Repository access through a GitHub App (#11056; docs/auth/github.md,
//! "GitHub App").
//!
//! The OAuth App stays for sign-in. A GitHub App is installed on the
//! repositories a person picks, and OpenAgents works in them with
//! installation tokens: minted from a JWT the App signs with its private
//! key (RS256, `iat` a minute back, `exp` ten minutes on), good for one
//! hour, scoped to one repository and to contents, metadata and pull
//! requests.
//!
//! - [`AppCredentials`]: the App's private file and key (account service
//!   only); [`AppInstall`] is the public half the web server needs.
//! - [`TokenCache`]: installation tokens in memory. A token is reused only
//!   while it is under 50 minutes old with at least 5 minutes left;
//!   concurrent callers for the same token wait on one mint.
//! - [`AppClient`]: minting, and one forced refresh when GitHub answers
//!   401 to a cached token.
//!
//! Which installations a person may use is learned with the App's user
//! token (`/user/installations`), never from an `installation_id` a
//! browser sends ([`crate::repos`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::signature::{self, RsaKeyPair};
use serde::Deserialize;
use serde_json::json;

use crate::config::{Endpoints, GithubApp, GithubCredentials, read_private_bytes};
use crate::github::{Api, Github, Secret};
use crate::repos::RepoError;

/// How long a cached installation token is reused at most.
pub const REUSE_SECONDS: u64 = 50 * 60;
/// A cached token with less than this left is minted again.
pub const MARGIN_SECONDS: u64 = 5 * 60;
/// What an installation token may do: read and write code, read
/// metadata, read and write pull requests. Nothing else.
pub const PERMISSIONS: [(&str, &str); 3] = [
    ("contents", "write"),
    ("metadata", "read"),
    ("pull_requests", "write"),
];

/// The public half of a GitHub App: what the web server needs to send a
/// person to GitHub to authorize the App and to install it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppInstall {
    /// The App's own OAuth client (its client id) and the callback.
    pub oauth: GithubApp,
    /// The App's URL name (`github.com/apps/<slug>`).
    pub slug: String,
}

impl AppInstall {
    /// Read the client id and slug from the App's private file.
    pub fn load(path: &Path, redirect_url: &str, endpoints: Endpoints) -> Result<Self, String> {
        let file = File::read(path)?;
        Ok(Self {
            oauth: GithubApp::new(&file.client_id, redirect_url, endpoints)?,
            slug: slug(&file.slug)?,
        })
    }

    /// Build from parts (tests and the fake).
    pub fn new(oauth: GithubApp, slug_value: &str) -> Result<Self, String> {
        Ok(Self {
            oauth,
            slug: slug(slug_value)?,
        })
    }

    /// GitHub's page for installing the App on an account and picking its
    /// repositories (or changing them, for an account that has it).
    #[must_use]
    pub fn install_url(&self) -> String {
        format!(
            "{}/apps/{}/installations/new",
            self.oauth.endpoints.web_url.trim_end_matches('/'),
            self.slug
        )
    }
}

fn slug(value: &str) -> Result<String, String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("The GitHub App slug is malformed.".into());
    }
    Ok(value.into())
}

/// The App's private file: `{"app_id", "slug", "client_id",
/// "client_secret", "token_encryption_key", "private_key"}`, where
/// `private_key` names the `.pem` GitHub generated (relative to the file).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    app_id: u64,
    slug: String,
    client_id: String,
    client_secret: String,
    token_encryption_key: String,
    private_key: PathBuf,
}

impl File {
    fn read(path: &Path) -> Result<Self, String> {
        let bytes = read_private_bytes(path)?;
        serde_json::from_slice(&bytes).map_err(|_| "The GitHub App file is malformed.".into())
    }
}

/// The whole GitHub App, private key included, for the account service.
#[derive(Clone)]
pub struct AppCredentials {
    pub app_id: u64,
    pub slug: String,
    /// The App's OAuth client: user tokens, and the key stored tokens are
    /// encrypted under.
    pub oauth: GithubCredentials,
    key: Arc<RsaKeyPair>,
}

impl std::fmt::Debug for AppCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppCredentials")
            .field("app_id", &self.app_id)
            .field("slug", &self.slug)
            .field("oauth", &self.oauth)
            .field("key", &"[redacted]")
            .finish()
    }
}

impl AppCredentials {
    /// Build from parts: `pem` is the App's private key as GitHub hands it
    /// out (`RSA PRIVATE KEY`) or as PKCS #8 (`PRIVATE KEY`).
    pub fn new(
        app_id: u64,
        slug_value: &str,
        oauth: GithubCredentials,
        pem: &[u8],
    ) -> Result<Self, String> {
        if app_id == 0 {
            return Err("The GitHub App id is malformed.".into());
        }
        Ok(Self {
            app_id,
            slug: slug(slug_value)?,
            oauth,
            key: Arc::new(private_key(pem)?),
        })
    }

    /// Read the App's private file and its key file (both mode 0600).
    pub fn load(path: &Path, redirect_url: &str, endpoints: Endpoints) -> Result<Self, String> {
        let file = File::read(path)?;
        let key_path = if file.private_key.is_absolute() {
            file.private_key.clone()
        } else {
            path.parent()
                .unwrap_or(Path::new("."))
                .join(&file.private_key)
        };
        let mut pem = read_private_bytes(&key_path)
            .map_err(|_| "The GitHub App private key file is unavailable or not mode 0600.")?;
        let token_key = STANDARD
            .decode(file.token_encryption_key.trim())
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
            .ok_or("The GitHub App token encryption key must be 32 bytes of base64.")?;
        let app = GithubApp::new(&file.client_id, redirect_url, endpoints)?;
        let oauth = GithubCredentials::new(app, &file.client_secret, token_key)?;
        let built = Self::new(file.app_id, &file.slug, oauth, &pem);
        pem.fill(0);
        built
    }

    /// The public half.
    #[must_use]
    pub fn install(&self) -> AppInstall {
        AppInstall {
            oauth: self.oauth.app.clone(),
            slug: self.slug.clone(),
        }
    }

    /// The App's JWT at `now`: RS256, `iat` 60 seconds back for clock
    /// drift, `exp` 10 minutes on, `iss` the App id.
    pub(crate) fn jwt(&self, now: u64) -> Result<Secret, RepoError> {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims = URL_SAFE_NO_PAD.encode(
            json!({
                "iat": now.saturating_sub(60),
                "exp": now + 600,
                "iss": self.app_id.to_string(),
            })
            .to_string(),
        );
        let message = format!("{header}.{claims}");
        let mut signed = vec![0u8; self.key.public().modulus_len()];
        self.key
            .sign(
                &signature::RSA_PKCS1_SHA256,
                &ring::rand::SystemRandom::new(),
                message.as_bytes(),
                &mut signed,
            )
            .map_err(|_| RepoError::NotConfigured)?;
        Ok(Secret::new(format!(
            "{message}.{}",
            URL_SAFE_NO_PAD.encode(signed)
        )))
    }

    /// The public key (DER `RSAPublicKey`), for the fake to check JWTs.
    #[must_use]
    pub fn public_key(&self) -> Vec<u8> {
        self.key.public().as_ref().to_vec()
    }
}

/// An RSA private key from PEM: PKCS #1 (what GitHub generates) or
/// PKCS #8.
fn private_key(pem: &[u8]) -> Result<RsaKeyPair, String> {
    let text = std::str::from_utf8(pem).map_err(|_| "The GitHub App private key isn't PEM.")?;
    let pkcs1 = text.contains("-----BEGIN RSA PRIVATE KEY-----");
    let pkcs8 = text.contains("-----BEGIN PRIVATE KEY-----");
    if !pkcs1 && !pkcs8 {
        return Err("The GitHub App private key isn't an RSA private key in PEM.".into());
    }
    let body: String = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("-----"))
        .collect();
    let mut der = STANDARD
        .decode(body)
        .map_err(|_| "The GitHub App private key isn't PEM.")?;
    let key = if pkcs1 {
        RsaKeyPair::from_der(&der)
    } else {
        RsaKeyPair::from_pkcs8(&der)
    }
    .map_err(|_| "The GitHub App private key couldn't be read.".to_string());
    der.fill(0);
    key
}

/// One installation token, as the cache keeps it.
#[derive(Clone)]
pub(crate) struct Minted {
    pub token: Arc<Secret>,
    pub minted_unix: u64,
    pub expires_unix: u64,
}

/// Whether a cached token may be used at `now`: under 50 minutes old and
/// at least 5 minutes left.
#[must_use]
pub fn fresh(minted_unix: u64, expires_unix: u64, now: u64) -> bool {
    now < minted_unix.saturating_add(REUSE_SECONDS)
        && expires_unix >= now.saturating_add(MARGIN_SECONDS)
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Key {
    api: String,
    app: u64,
    installation: u64,
    repository: Option<u64>,
}

type Slot = Arc<tokio::sync::Mutex<Option<Minted>>>;

/// Installation tokens in memory, one per App, installation and
/// repository. Never written anywhere.
pub struct TokenCache {
    slots: std::sync::Mutex<HashMap<Key, Slot>>,
    mints: AtomicUsize,
    /// A fixed clock for tests (seconds); 0 means the system clock.
    clock: AtomicU64,
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: std::sync::Mutex::new(HashMap::new()),
            mints: AtomicUsize::new(0),
            clock: AtomicU64::new(0),
        }
    }

    /// The process's cache, shared by every [`AppClient::new`].
    #[must_use]
    pub fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<TokenCache>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(Self::new())).clone()
    }

    /// How many tokens this cache asked GitHub for.
    #[must_use]
    pub fn mints(&self) -> usize {
        self.mints.load(Ordering::SeqCst)
    }

    /// Pin the clock (tests): `seconds` since the epoch; 0 restores the
    /// system clock.
    pub fn set_clock(&self, seconds: u64) {
        self.clock.store(seconds, Ordering::SeqCst);
    }

    pub(crate) fn now(&self) -> u64 {
        match self.clock.load(Ordering::SeqCst) {
            0 => now(),
            pinned => pinned,
        }
    }

    fn slot(&self, key: &Key) -> Slot {
        let mut slots = self.slots.lock().expect("token cache");
        if slots.len() > 4096 {
            let now = self.now();
            slots.retain(|_, slot| {
                slot.try_lock()
                    .map(|held| {
                        held.as_ref()
                            .is_some_and(|m| fresh(m.minted_unix, m.expires_unix, now))
                    })
                    .unwrap_or(true)
            });
        }
        slots.entry(key.clone()).or_default().clone()
    }

    /// The cached token for `key`, or a new one from `mint`. `stale` is a
    /// token GitHub just refused: it is never handed out again. Callers
    /// for the same key wait for one mint rather than each minting.
    async fn get<F, Fut>(
        &self,
        key: &Key,
        stale: Option<&str>,
        mint: F,
    ) -> Result<Minted, RepoError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(Secret, u64), RepoError>>,
    {
        let slot = self.slot(key);
        let mut held = slot.lock().await;
        let now = self.now();
        if let Some(cached) = held.as_ref() {
            let refused = stale.is_some_and(|s| cached.token.as_str() == s);
            if !refused && fresh(cached.minted_unix, cached.expires_unix, now) {
                return Ok(cached.clone());
            }
        }
        *held = None;
        self.mints.fetch_add(1, Ordering::SeqCst);
        let (token, expires_unix) = mint().await?;
        let minted = Minted {
            token: Arc::new(token),
            minted_unix: now,
            expires_unix,
        };
        *held = Some(minted.clone());
        Ok(minted)
    }
}

/// A GitHub App client: its credentials, an HTTP client for its OAuth
/// half and the API, and the token cache.
#[derive(Clone)]
pub struct AppClient {
    credentials: Arc<AppCredentials>,
    github: Github,
    cache: Arc<TokenCache>,
}

impl std::fmt::Debug for AppClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppClient")
            .field("app_id", &self.credentials.app_id)
            .finish_non_exhaustive()
    }
}

impl AppClient {
    /// A client over the process's shared [`TokenCache`].
    pub fn new(credentials: AppCredentials) -> Result<Self, String> {
        Self::with_cache(credentials, TokenCache::shared())
    }

    /// A client over its own cache (tests).
    pub fn with_cache(credentials: AppCredentials, cache: Arc<TokenCache>) -> Result<Self, String> {
        let github = Github::new(credentials.oauth.clone())?;
        Ok(Self {
            credentials: Arc::new(credentials),
            github,
            cache,
        })
    }

    #[must_use]
    pub fn credentials(&self) -> &AppCredentials {
        &self.credentials
    }

    /// The App's OAuth client (user tokens, sealing).
    #[must_use]
    pub fn github(&self) -> &Github {
        &self.github
    }

    #[must_use]
    pub fn cache(&self) -> &TokenCache {
        &self.cache
    }

    /// An installation token for `installation`, scoped to the one
    /// repository `repository_id` when given, from the cache when one is
    /// fresh. `stale` is a token GitHub just answered 401 to.
    pub(crate) async fn installation_token(
        &self,
        installation: u64,
        repository_id: Option<u64>,
        stale: Option<&str>,
    ) -> Result<Minted, RepoError> {
        let key = Key {
            api: self.credentials.oauth.app.endpoints.api_url.clone(),
            app: self.credentials.app_id,
            installation,
            repository: repository_id,
        };
        self.cache
            .get(&key, stale, || self.mint(installation, repository_id))
            .await
    }

    async fn mint(
        &self,
        installation: u64,
        repository_id: Option<u64>,
    ) -> Result<(Secret, u64), RepoError> {
        let jwt = self.credentials.jwt(now())?;
        let permissions: serde_json::Map<String, serde_json::Value> = PERMISSIONS
            .iter()
            .map(|(k, v)| ((*k).to_string(), json!(v)))
            .collect();
        let mut body = json!({"permissions": permissions});
        if let Some(id) = repository_id {
            body["repository_ids"] = json!([id]);
        }
        let answer = self
            .github
            .api_post(
                jwt.as_str(),
                &format!("/app/installations/{installation}/access_tokens"),
                &body,
            )
            .await?;
        let answer = minted(answer)?;
        let token = answer.body["token"]
            .as_str()
            .filter(|t| !t.is_empty() && t.len() <= 1024 && t.bytes().all(|b| b.is_ascii_graphic()))
            .ok_or(RepoError::BadAnswer)?;
        let expires = answer.body["expires_at"]
            .as_str()
            .and_then(github_time)
            .ok_or(RepoError::BadAnswer)?;
        Ok((Secret::new(token.to_string()), expires))
    }

    /// Read `path` as the App with an installation token (scoped to
    /// `repository_id` when given): the JSON body, or what GitHub's answer
    /// means. When GitHub answers 401 (a token revoked early, or a clock
    /// that drifted), the token is minted again once and the read tried
    /// once more.
    pub async fn installation_get(
        &self,
        installation: u64,
        repository_id: Option<u64>,
        path: &str,
    ) -> Result<serde_json::Value, RepoError> {
        let answer = self
            .installation_api(installation, repository_id, path)
            .await?;
        Ok(crate::repos::answered(answer)?.body)
    }

    async fn installation_api(
        &self,
        installation: u64,
        repository_id: Option<u64>,
        path: &str,
    ) -> Result<Api, RepoError> {
        let first = self
            .installation_token(installation, repository_id, None)
            .await?;
        let answer = self.github.api(first.token.as_str(), path).await?;
        if answer.status != 401 {
            return Ok(answer);
        }
        let again = self
            .installation_token(installation, repository_id, Some(first.token.as_str()))
            .await?;
        Ok(self.github.api(again.token.as_str(), path).await?)
    }
}

/// What GitHub's answer to a token request means.
fn minted(answer: Api) -> Result<Api, RepoError> {
    match answer.status {
        // GitHub refused the JWT: the App id or key is wrong here.
        401 => Err(RepoError::NotConfigured),
        403 if !answer.rate_limited
            && answer.body["message"]
                .as_str()
                .is_some_and(|m| m.to_ascii_lowercase().contains("suspended")) =>
        {
            Err(RepoError::Suspended)
        }
        // The installation is gone, or the repository isn't in it.
        404 | 422 => Err(RepoError::NotInstalled),
        _ => crate::repos::answered(answer),
    }
}

/// GitHub's `2026-10-09T12:34:56Z` as seconds since the epoch.
#[must_use]
pub fn github_time(value: &str) -> Option<u64> {
    let b = value.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| value.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

/// Seconds since the epoch as GitHub writes them.
#[must_use]
pub fn github_time_text(unix: u64) -> String {
    let days = i64::try_from(unix / 86_400).unwrap_or_default();
    let rest = unix % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_rule_is_fifty_minutes_old_and_five_minutes_left() {
        let minted = 1_000_000;
        let expires = minted + 3600;
        assert!(fresh(minted, expires, minted));
        assert!(fresh(minted, expires, minted + 49 * 60));
        assert!(!fresh(minted, expires, minted + 50 * 60));
        // A token GitHub gave less time than usual: 5 minutes left is the
        // floor, whatever its age.
        assert!(fresh(minted, minted + 10 * 60, minted + 5 * 60));
        assert!(!fresh(minted, minted + 10 * 60, minted + 5 * 60 + 1));
        assert!(!fresh(minted, minted + 4 * 60, minted));
    }

    #[test]
    fn github_times_round_trip() {
        assert_eq!(github_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(github_time("2016-07-11T22:14:10Z"), Some(1_468_275_250));
        assert_eq!(github_time_text(1_468_275_250), "2016-07-11T22:14:10Z");
        for t in [0, 951_782_400, 1_790_000_000, 4_102_444_799] {
            assert_eq!(github_time(&github_time_text(t)), Some(t));
        }
        assert_eq!(github_time("2016-07-11 22:14:10Z"), None);
        assert_eq!(github_time("2016-13-11T22:14:10Z"), None);
        assert_eq!(github_time("2016-07-11T22:14:10+00:00"), None);
    }
}
