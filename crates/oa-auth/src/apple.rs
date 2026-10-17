//! Sign in with Apple for the phone (#11980): `POST /v1/sessions/apple`
//! with `{identity_token, nonce, device?, name?}` from the native Apple
//! sheet.
//!
//! [`Apple::verify`] checks the identity token, an RS256 JWT, against
//! Apple's published keys (fetched from [`KEYS_URL`] with a bounded
//! timeout and cached), its issuer ([`ISSUER`]), its audience (the
//! deployment's bundle ids), its expiry, and its nonce: the app hands
//! Apple the SHA-256 (hex) of a random value it keeps, and sends us the
//! raw value, so the token's `nonce` claim must be its digest. Then
//! [`sign_in`] finds or creates the account linked to Apple's `sub` and
//! issues the same app session a device sign-in issues, so the phone's
//! credential store works unchanged. No Apple token is kept.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tenancy::accounts::apple::{AppleProfile, apple_principal};
use tenancy::sessions::{self, AppLabel, Issued, Sessions};
use tenancy::workspaces::UserId;
use tenancy::{Account, Accounts};

use crate::AuthError;

/// The issuer every Apple identity token names.
pub const ISSUER: &str = "https://appleid.apple.com";
/// Where Apple publishes its token-signing keys.
pub const KEYS_URL: &str = "https://appleid.apple.com/auth/keys";

/// How long fetched keys are trusted before they are fetched again.
const KEYS_TTL: Duration = Duration::from_secs(3600);
/// A token signed by a key we don't hold refetches at most this often.
const REFETCH_GAP: Duration = Duration::from_secs(60);
/// The fetch's bound, start to finish.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
/// The largest key document accepted.
const KEYS_MAX: usize = 64 * 1024;
/// The largest identity token accepted.
const TOKEN_MAX: usize = 8 * 1024;
/// Clock skew allowed on `exp` and `iat`, in seconds.
const SKEW: u64 = 60;

/// One of Apple's published keys: an RSA modulus and exponent.
#[derive(Clone, Debug, Deserialize)]
pub struct Jwk {
    pub kid: String,
    #[serde(default)]
    pub kty: String,
    #[serde(default)]
    pub alg: Option<String>,
    pub n: String,
    pub e: String,
}

/// Apple's key document.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Keys {
    pub keys: Vec<Jwk>,
}

impl Keys {
    fn get(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|key| key.kid == kid)
    }
}

/// Why a token was refused. Never shown to people; the route answers one
/// plain refusal for all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejected {
    Malformed,
    UnknownKey,
    Signature,
    Issuer,
    Audience,
    Expired,
    Nonce,
}

/// The claims of an Apple identity token this service reads.
#[derive(Deserialize)]
struct Claims {
    iss: String,
    aud: String,
    exp: u64,
    #[serde(default)]
    iat: Option<u64>,
    sub: String,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<Value>,
    #[serde(default)]
    is_private_email: Option<Value>,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

/// Apple writes booleans as `true` or `"true"`.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    }
}

/// SHA-256 of the raw nonce, lowercase hex: what the app gave Apple.
#[must_use]
pub fn nonce_digest(raw: &str) -> String {
    Sha256::digest(raw.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The token's key id, so a caller can tell whether it holds the key.
fn key_id(token: &str) -> Result<String, Rejected> {
    let header = token.split('.').next().ok_or(Rejected::Malformed)?;
    let header: Header = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(header)
            .map_err(|_| Rejected::Malformed)?,
    )
    .map_err(|_| Rejected::Malformed)?;
    Ok(header.kid)
}

/// Verify `token` against `keys` for one of `audiences` at `now`, with
/// the app's raw nonce. Pure: the tests drive it with a fixture key set.
pub fn verify_with(
    keys: &Keys,
    audiences: &[String],
    token: &str,
    raw_nonce: &str,
    now: u64,
) -> Result<AppleProfile, Rejected> {
    if token.len() > TOKEN_MAX || raw_nonce.is_empty() || raw_nonce.len() > 256 {
        return Err(Rejected::Malformed);
    }
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Rejected::Malformed);
    };
    let decode = |part: &str| {
        URL_SAFE_NO_PAD
            .decode(part)
            .map_err(|_| Rejected::Malformed)
    };
    let head: Header = serde_json::from_slice(&decode(header)?).map_err(|_| Rejected::Malformed)?;
    if head.alg != "RS256" {
        return Err(Rejected::Malformed);
    }
    let key = keys.get(&head.kid).ok_or(Rejected::UnknownKey)?;
    if key.kty != "RSA" || key.alg.as_deref().is_some_and(|alg| alg != "RS256") {
        return Err(Rejected::UnknownKey);
    }
    let public = ring::signature::RsaPublicKeyComponents {
        n: decode(&key.n)?,
        e: decode(&key.e)?,
    };
    let signed = &token[..header.len() + 1 + payload.len()];
    public
        .verify(
            &ring::signature::RSA_PKCS1_2048_8192_SHA256,
            signed.as_bytes(),
            &decode(signature)?,
        )
        .map_err(|_| Rejected::Signature)?;
    let claims: Claims =
        serde_json::from_slice(&decode(payload)?).map_err(|_| Rejected::Malformed)?;
    if claims.iss != ISSUER {
        return Err(Rejected::Issuer);
    }
    if !audiences.iter().any(|aud| *aud == claims.aud) {
        return Err(Rejected::Audience);
    }
    if claims.exp + SKEW <= now || claims.iat.is_some_and(|iat| iat > now + SKEW) {
        return Err(Rejected::Expired);
    }
    let expected = nonce_digest(raw_nonce);
    let nonce_ok = claims.nonce.as_deref().is_some_and(|nonce| {
        subtle::ConstantTimeEq::ct_eq(nonce.as_bytes(), expected.as_bytes()).into()
    });
    if !nonce_ok {
        return Err(Rejected::Nonce);
    }
    let profile = AppleProfile {
        sub: claims.sub,
        email: claims.email.filter(|e| !e.is_empty()),
        email_verified: truthy(claims.email_verified.as_ref()),
        private_email: truthy(claims.is_private_email.as_ref()),
    };
    profile.validate().map_err(|_| Rejected::Malformed)?;
    Ok(profile)
}

/// Fetched key documents by address, shared by every [`Apple`] in the
/// process (the gateway builds one per request).
struct Cached {
    keys: Keys,
    fetched: Instant,
}

fn cache() -> &'static Mutex<HashMap<String, Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The deployment's Sign in with Apple verifier.
#[derive(Clone, Debug)]
pub struct Apple {
    audiences: Vec<String>,
    keys_url: String,
    http: reqwest::Client,
}

impl Apple {
    /// A verifier for tokens issued to `audiences` (the app's bundle ids),
    /// with keys from `keys_url` (Apple's, [`KEYS_URL`], unless a test
    /// names its own).
    pub fn new(audiences: Vec<String>, keys_url: Option<String>) -> Result<Self, AuthError> {
        if audiences.is_empty() || audiences.iter().any(String::is_empty) {
            return Err(AuthError::Unavailable);
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(FETCH_TIMEOUT)
            .build()
            .map_err(|_| AuthError::Unavailable)?;
        Ok(Self {
            audiences,
            keys_url: keys_url.unwrap_or_else(|| KEYS_URL.to_string()),
            http,
        })
    }

    async fn fetch(&self) -> Result<Keys, AuthError> {
        let response = self
            .http
            .get(&self.keys_url)
            .send()
            .await
            .map_err(|_| AuthError::Unavailable)?;
        if !response.status().is_success() {
            return Err(AuthError::Unavailable);
        }
        let bytes = response.bytes().await.map_err(|_| AuthError::Unavailable)?;
        if bytes.len() > KEYS_MAX {
            return Err(AuthError::Unavailable);
        }
        serde_json::from_slice(&bytes).map_err(|_| AuthError::Unavailable)
    }

    /// The keys to check a token signed by `kid` with: the cached set
    /// while it is fresh and holds `kid`, else a new fetch (at most once
    /// a minute for an unknown `kid`).
    async fn keys_for(&self, kid: &str) -> Result<Keys, AuthError> {
        let held = {
            let cache = cache().lock().map_err(|_| AuthError::Unavailable)?;
            cache.get(&self.keys_url).map(|cached| {
                let fresh = cached.fetched.elapsed() < KEYS_TTL;
                let knows = cached.keys.get(kid).is_some();
                let recent = cached.fetched.elapsed() < REFETCH_GAP;
                (cached.keys.clone(), (fresh && knows) || recent)
            })
        };
        if let Some((keys, true)) = held {
            return Ok(keys);
        }
        match self.fetch().await {
            Ok(keys) => {
                if let Ok(mut cache) = cache().lock() {
                    cache.insert(
                        self.keys_url.clone(),
                        Cached {
                            keys: keys.clone(),
                            fetched: Instant::now(),
                        },
                    );
                }
                Ok(keys)
            }
            // Apple unreachable: keep using what we hold.
            Err(error) => held.map(|(keys, _)| keys).ok_or(error),
        }
    }

    /// Verify an identity token from the app with its raw nonce.
    pub async fn verify(&self, token: &str, raw_nonce: &str) -> Result<AppleProfile, AuthError> {
        let kid = key_id(token).map_err(|_| AuthError::AppleDenied)?;
        let keys = self.keys_for(&kid).await?;
        verify_with(&keys, &self.audiences, token, raw_nonce, now())
            .map_err(|_| AuthError::AppleDenied)
    }
}

/// The app that signed in, as the app names itself.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub computer: String,
}

/// `POST /v1/sessions/apple`'s body.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleRequest {
    pub identity_token: String,
    pub nonce: String,
    #[serde(default)]
    pub device: Option<Device>,
    /// The name Apple's sheet gave the app, sent only the first time.
    #[serde(default)]
    pub name: Option<String>,
}

impl std::fmt::Debug for AppleRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AppleRequest([redacted])")
    }
}

/// A completed Apple sign-in: the account, whether it is new, and the app
/// session (its token in `session.once`, returned exactly once).
pub struct SignedIn {
    pub account: Account,
    pub created: bool,
    pub session: Issued,
}

/// Sign in (or sign up) with a verified Apple identity: find or create the
/// account behind `profile.sub` (a new one gets a personal workspace on
/// `tenant`), then issue an app session labeled with `device`.
pub fn sign_in(
    dir: &Path,
    tenant: &str,
    profile: &AppleProfile,
    name: Option<&str>,
    device: Option<&Device>,
) -> Result<SignedIn, AuthError> {
    let label = AppLabel::new(
        device
            .map(|d| d.app.as_str())
            .filter(|a| !a.trim().is_empty())
            .unwrap_or("OpenAgents"),
        device
            .map(|d| d.computer.as_str())
            .filter(|c| !c.trim().is_empty())
            .unwrap_or("iPhone"),
    )
    .map_err(|_| AuthError::AppleDenied)?;
    let accounts = Accounts::open(dir).map_err(|_| AuthError::Unavailable)?;
    let resolved = accounts
        .sign_in_apple(profile, name, tenant)
        .map_err(AuthError::from_refusal)?;
    let sessions = Sessions::open(dir).map_err(|_| AuthError::Unavailable)?;
    let account = resolved.account.id.clone();
    let workspace = resolved.workspace.as_ref().map(|w| w.id.clone());
    let created = resolved.created;
    let session = sessions
        .mutate(|book, access, now| {
            let issued = book.issue_app(UserId::from(account.as_str()), label.clone(), now)?;
            sessions::push_access(
                access,
                sessions::Access {
                    at: now,
                    actor: account.clone(),
                    action: if created { "sign-up" } else { "sign-in" }.to_string(),
                    workspace: workspace.clone(),
                    session: Some(issued.session.id.as_str().to_string()),
                    detail: Some(apple_principal(&profile.sub)),
                },
            );
            Ok(issued)
        })
        .map_err(|_| AuthError::Unavailable)?;
    Ok(SignedIn {
        account: resolved.account,
        created,
        session,
    })
}

/// The answer, in the device poll's shape (`{session, token, account}`)
/// plus `created`, so the web server hands the app what a device sign-in
/// gives it.
#[must_use]
pub fn signed_in_body(signed: &SignedIn) -> Value {
    let session = &signed.session.session;
    let mut body = json!({
        "v": "openagents.accounts.v1",
        "session": {
            "id": session.id.as_str(),
            "kind": "user",
            "account": session.user.as_str(),
            "created_at": session.created_at,
            "expires_at": session.expires_at,
        },
        "token": signed.session.once,
        "account": {"id": signed.account.id, "label": signed.account.label},
        "created": signed.created,
    });
    if let Some(label) = &session.app {
        body["session"]["app"] = json!(label.app);
        body["session"]["computer"] = json!(label.computer);
    }
    body
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests;
