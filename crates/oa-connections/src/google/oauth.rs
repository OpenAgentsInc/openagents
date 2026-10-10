//! Google OAuth 2.0 for a web server: the authorization URL (PKCE, offline
//! access, incremental consent), the code exchange, refreshing an access
//! token, and revoking a connection.
//!
//! Incremental consent: every authorization asks with
//! `include_granted_scopes=true`, so asking later for one more scope (Sheets
//! after Drive) keeps what was granted, and the token's `scope` says
//! everything the person has granted. Google lets the person untick a
//! scope; [`Grant::scopes`] records what they actually granted.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::Endpoints;

/// The OAuth client: its id, and its secret, which is never printed.
#[derive(Clone)]
pub struct Client {
    pub id: String,
    secret: String,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client({}, secret redacted)", self.id)
    }
}

impl Client {
    #[must_use]
    pub fn new(id: String, secret: String) -> Self {
        Self { id, secret }
    }

    /// The client from the JSON Google's console downloads
    /// (`{"web": {"client_id", "client_secret", ...}}`), or the same two
    /// fields at the top level.
    ///
    /// # Errors
    ///
    /// A sentence that never contains the secret.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Value =
            serde_json::from_str(text).map_err(|_| "The Google OAuth client is not JSON.")?;
        let inner = if value["web"].is_object() {
            &value["web"]
        } else if value["installed"].is_object() {
            &value["installed"]
        } else {
            &value
        };
        let field = |name: &str| {
            inner[name]
                .as_str()
                .map(str::trim)
                .filter(|v| !v.is_empty() && v.len() <= 512)
                .map(str::to_owned)
        };
        match (field("client_id"), field("client_secret")) {
            (Some(id), Some(secret)) => Ok(Self { id, secret }),
            _ => Err("The Google OAuth client needs client_id and client_secret.".into()),
        }
    }
}

/// A PKCE verifier and its S256 challenge.
#[derive(Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    /// A fresh random verifier.
    ///
    /// # Panics
    ///
    /// When the system has no randomness.
    #[must_use]
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("system randomness");
        Self::from_verifier(URL_SAFE_NO_PAD.encode(bytes))
    }

    #[must_use]
    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

/// Where to send the person to grant `scopes` (plus who they are).
#[must_use]
pub fn authorize_url(
    endpoints: &Endpoints,
    client: &Client,
    redirect_uri: &str,
    scopes: &[&str],
    state: &str,
    challenge: &str,
) -> String {
    let mut all: Vec<&str> = super::IDENTITY.to_vec();
    for scope in scopes {
        if !all.contains(scope) {
            all.push(scope);
        }
    }
    let mut url = url::Url::parse(&endpoints.authorize).expect("the authorize endpoint is a URL");
    url.query_pairs_mut()
        .append_pair("client_id", &client.id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &all.join(" "))
        .append_pair("access_type", "offline")
        .append_pair("include_granted_scopes", "true")
        // Always ask, so Google sends a refresh token every time.
        .append_pair("prompt", "consent")
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    url.into()
}

/// Why a call to Google's OAuth endpoints didn't work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthError {
    /// Google refused the code or the refresh token (`invalid_grant`): it
    /// expired, or the person removed access. Connect again.
    Refused,
    /// Google couldn't be reached or answered with an error.
    Unreachable,
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Refused => "Google no longer accepts this connection. Connect Google again.",
            Self::Unreachable => "Google couldn't be reached. Try again in a minute.",
        })
    }
}

/// What a finished authorization gives.
pub struct Grant {
    /// Absent only when Google didn't send one; keep the earlier one then.
    pub refresh_token: Option<String>,
    pub access_token: String,
    pub expires_in: u64,
    /// Every scope the person has granted this client.
    pub scopes: Vec<String>,
    /// The Google account's verified email, from the ID token.
    pub email: Option<String>,
}

impl fmt::Debug for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Grant")
            .field("scopes", &self.scopes)
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

/// A short-lived access token.
#[derive(Clone)]
pub struct Access {
    pub token: String,
    pub expires_in: u64,
    pub scopes: Vec<String>,
}

impl fmt::Debug for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Access(expires in {} s, token redacted)",
            self.expires_in
        )
    }
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    id_token: Option<String>,
    error: Option<String>,
}

async fn token_call(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    form: &[(&str, &str)],
) -> Result<TokenAnswer, OAuthError> {
    let response = http
        .post(&endpoints.token)
        .form(form)
        .send()
        .await
        .map_err(|_| OAuthError::Unreachable)?;
    let status = response.status();
    let answer: TokenAnswer = response.json().await.map_err(|_| OAuthError::Unreachable)?;
    if answer.error.as_deref() == Some("invalid_grant") {
        return Err(OAuthError::Refused);
    }
    if !status.is_success() || answer.access_token.as_deref().is_none_or(str::is_empty) {
        return Err(OAuthError::Unreachable);
    }
    Ok(answer)
}

fn scopes_of(scope: Option<&str>) -> Vec<String> {
    let mut scopes: Vec<String> = scope
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    scopes.sort();
    scopes.dedup();
    scopes
}

/// Trade the authorization `code` for tokens.
///
/// # Errors
///
/// [`OAuthError`]; nothing secret is in it.
pub async fn exchange(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    client: &Client,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<Grant, OAuthError> {
    let answer = token_call(
        http,
        endpoints,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &client.id),
            ("client_secret", &client.secret),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ],
    )
    .await?;
    Ok(Grant {
        email: answer.id_token.as_deref().and_then(verified_email),
        scopes: scopes_of(answer.scope.as_deref()),
        refresh_token: answer.refresh_token.filter(|t| !t.is_empty()),
        access_token: answer.access_token.unwrap_or_default(),
        expires_in: answer.expires_in.unwrap_or(3_600),
    })
}

/// A fresh access token from a refresh token.
///
/// # Errors
///
/// [`OAuthError::Refused`] when the person removed access.
pub async fn refresh(
    http: &reqwest::Client,
    endpoints: &Endpoints,
    client: &Client,
    refresh_token: &str,
) -> Result<Access, OAuthError> {
    let answer = token_call(
        http,
        endpoints,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &client.id),
            ("client_secret", &client.secret),
        ],
    )
    .await?;
    Ok(Access {
        scopes: scopes_of(answer.scope.as_deref()),
        token: answer.access_token.unwrap_or_default(),
        expires_in: answer.expires_in.unwrap_or(3_600),
    })
}

/// Ask Google to forget the connection (best effort: removing it here
/// happens either way).
pub async fn revoke(http: &reqwest::Client, endpoints: &Endpoints, token: &str) -> bool {
    http.post(&endpoints.revoke)
        .form(&[("token", token)])
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

/// The verified email in an ID token received straight from Google's token
/// endpoint over TLS (OpenID Connect Core 3.1.3.7 lets a client that got it
/// that way skip checking the signature).
#[must_use]
pub fn verified_email(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    if claims["email_verified"] != Value::Bool(true) {
        return None;
    }
    claims["email"]
        .as_str()
        .filter(|e| e.len() <= 320 && e.contains('@'))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_client_reads_the_console_download_and_never_prints_its_secret() {
        let client = Client::parse(
            r#"{"web":{"client_id":"id.apps","client_secret":"s3cret","redirect_uris":[]}}"#,
        )
        .unwrap();
        assert_eq!(client.id, "id.apps");
        assert!(!format!("{client:?}").contains("s3cret"));
        assert!(Client::parse(r#"{"client_id":"a","client_secret":"b"}"#).is_ok());
        let error = Client::parse(r#"{"web":{"client_id":"a"}}"#).unwrap_err();
        assert!(error.contains("client_secret"));
    }

    #[test]
    fn the_authorization_asks_offline_with_incremental_consent_and_pkce() {
        let client = Client::new("cid".into(), "x".into());
        let pkce = Pkce::from_verifier("v".repeat(43));
        let url = authorize_url(
            &Endpoints::default(),
            &client,
            "https://openagents.com/auth/google/callback",
            &[super::super::DRIVE_READONLY, super::super::SHEETS_READONLY],
            "st",
            &pkce.challenge,
        );
        let parsed = url::Url::parse(&url).unwrap();
        let get = |k: &str| {
            parsed
                .query_pairs()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.into_owned())
        };
        assert_eq!(get("include_granted_scopes").as_deref(), Some("true"));
        assert_eq!(get("access_type").as_deref(), Some("offline"));
        assert_eq!(get("code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(get("code_challenge"), Some(pkce.challenge.clone()));
        assert_eq!(
            get("scope").as_deref(),
            Some(
                "openid email https://www.googleapis.com/auth/drive.readonly https://www.googleapis.com/auth/spreadsheets.readonly"
            )
        );
        assert!(!url.contains("client_secret"));
    }

    #[test]
    fn only_a_verified_email_is_read_from_the_id_token() {
        let token = |claims: &str| format!("h.{}.s", URL_SAFE_NO_PAD.encode(claims));
        assert_eq!(
            verified_email(&token(r#"{"email":"a@b.c","email_verified":true}"#)).as_deref(),
            Some("a@b.c")
        );
        assert_eq!(
            verified_email(&token(r#"{"email":"a@b.c","email_verified":false}"#)),
            None
        );
    }
}
