//! The account-service half: trade a GitHub authorization code (and its
//! PKCE verifier) for the person's GitHub profile.
//!
//! The sign-in token lives only inside [`Github::profile`]: it reads
//! `/user` and `/user/emails` and is dropped. Connecting repositories
//! ([`crate::repos`]) exchanges its own code for a token it keeps
//! encrypted. No redirects are followed, every call is time-bounded, and
//! response bodies are size-bounded.

use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthType, AuthorizationCode, ClientId, ClientSecret, PkceCodeVerifier, RedirectUrl,
    RefreshToken, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use tenancy::accounts::identities::{GithubEmail, GithubProfile, MAX_EMAILS, MAX_FIELD};

use crate::AuthError;
use crate::config::GithubCredentials;

/// The largest GitHub API body read. A page of 100 repositories from
/// `/user/repos` is about 6 KB a repository, so 256 KB refused real accounts
/// ("GitHub isn't answering"); 8 MB leaves room for the largest pages.
const BODY_MAX: usize = 8 * 1024 * 1024;

/// The `X-GitHub-Api-Version` every OpenAgents GitHub read sends (one
/// version everywhere; GitHub refuses a version it doesn't know with 400).
pub const API_VERSION: &str = "2022-11-28";

/// A GitHub OAuth client with its secret.
#[derive(Clone, Debug)]
pub struct Github {
    credentials: GithubCredentials,
    http: reqwest::Client,
}

impl Github {
    pub fn new(credentials: GithubCredentials) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .user_agent("OpenAgents")
            .build()
            .map_err(|_| "The GitHub client is unavailable.")?;
        Ok(Self { credentials, http })
    }

    #[must_use]
    pub fn credentials(&self) -> &GithubCredentials {
        &self.credentials
    }

    /// Exchange `code` with its PKCE `verifier`, read the profile and
    /// emails, and drop the token.
    pub async fn profile(&self, code: &str, verifier: &str) -> Result<GithubProfile, AuthError> {
        let token = self.exchange(code, verifier).await?;
        let bearer = token.as_str();
        let user: User = self
            .get(bearer, "/user")
            .await?
            .ok_or(AuthError::Unavailable)?;
        // Without the email scope, or for an account with no addresses,
        // GitHub answers an error or an empty list: the profile has none.
        let emails: Vec<Email> = self
            .get(bearer, "/user/emails")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        Ok(user.into_profile(emails))
    }

    /// Trade `code` and its PKCE `verifier` for an access token.
    pub(crate) async fn exchange(&self, code: &str, verifier: &str) -> Result<Secret, AuthError> {
        Ok(self.exchange_tokens(code, verifier).await?.access)
    }

    /// Trade `code` and its PKCE `verifier` for tokens: the access token
    /// and, for a GitHub App's expiring user tokens, its lifetime and the
    /// refresh token.
    pub(crate) async fn exchange_tokens(
        &self,
        code: &str,
        verifier: &str,
    ) -> Result<Tokens, AuthError> {
        if code.is_empty()
            || code.len() > 256
            || verifier.len() < 43
            || verifier.len() > 128
            || !code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(AuthError::Denied);
        }
        let token = self
            .client()?
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(PkceCodeVerifier::new(verifier.to_string()))
            .request_async(&self.http)
            .await
            .map_err(refused)?;
        Tokens::from_response(&token)
    }

    /// Trade a GitHub App user token's refresh token for new tokens.
    /// GitHub rotates the refresh token: the old one stops working.
    pub(crate) async fn refresh(&self, refresh: &Secret) -> Result<Tokens, AuthError> {
        let token = self
            .client()?
            .exchange_refresh_token(&RefreshToken::new(refresh.as_str().to_string()))
            .request_async(&self.http)
            .await
            .map_err(refused)?;
        Tokens::from_response(&token)
    }

    fn client(
        &self,
    ) -> Result<
        BasicClient<
            oauth2::EndpointNotSet,
            oauth2::EndpointNotSet,
            oauth2::EndpointNotSet,
            oauth2::EndpointNotSet,
            oauth2::EndpointSet,
        >,
        AuthError,
    > {
        let app = &self.credentials.app;
        Ok(BasicClient::new(ClientId::new(app.client_id.clone()))
            .set_client_secret(ClientSecret::new(self.credentials.secret().to_string()))
            .set_token_uri(
                TokenUrl::new(app.endpoints.token_url.clone())
                    .map_err(|_| AuthError::Unavailable)?,
            )
            .set_redirect_uri(
                RedirectUrl::new(app.redirect_url.clone()).map_err(|_| AuthError::Unavailable)?,
            )
            .set_auth_type(AuthType::RequestBody))
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        bearer: &str,
        path: &str,
    ) -> Result<Option<T>, AuthError> {
        let answer = self.api(bearer, path).await?;
        if !(200..300).contains(&answer.status) {
            return Ok(None);
        }
        Ok(serde_json::from_value(answer.body).ok())
    }

    /// One GitHub API read with `bearer` (see [`Github::api_within`]),
    /// bounded by [`API_TIMEOUT`].
    pub(crate) async fn api(&self, bearer: &str, path: &str) -> Result<Api, ApiFault> {
        self.api_within(bearer, path, API_TIMEOUT).await
    }

    /// One GitHub API read with `bearer` within `limit`: the status, the
    /// scopes GitHub says the token holds (`X-OAuth-Scopes`), what it said
    /// about rate limits and single sign-on, the next page (only when its
    /// `Link` stays on this API origin), and the JSON body (null when it
    /// isn't JSON). A dropped connection or a 502/503/504 is tried once
    /// more after a short pause; GitHub answers those to slow listings.
    pub(crate) async fn api_within(
        &self,
        bearer: &str,
        path: &str,
        limit: Duration,
    ) -> Result<Api, ApiFault> {
        let started = std::time::Instant::now();
        let first = self.api_once(bearer, path, limit).await;
        match &first {
            Ok(answer) if !matches!(answer.status, 502..=504) => return first,
            Err(ApiFault::TooLarge) => return first,
            _ => {}
        }
        let left = limit.saturating_sub(started.elapsed() + RETRY_PAUSE);
        if left < Duration::from_secs(1) {
            return first;
        }
        tokio::time::sleep(RETRY_PAUSE).await;
        self.api_once(bearer, path, left).await
    }

    async fn api_once(&self, bearer: &str, path: &str, limit: Duration) -> Result<Api, ApiFault> {
        self.send_once(reqwest::Method::GET, bearer, path, None, limit)
            .await
    }

    /// One GitHub API `POST` of `body` with `bearer` (a GitHub App's JWT
    /// minting an installation token), bounded by [`API_TIMEOUT`]. A
    /// dropped connection or a 502/503/504 is tried once more, as reads
    /// are: minting twice is harmless.
    pub(crate) async fn api_post(
        &self,
        bearer: &str,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<Api, ApiFault> {
        let first = self
            .send_once(reqwest::Method::POST, bearer, path, Some(body), API_TIMEOUT)
            .await;
        match &first {
            Ok(answer) if !matches!(answer.status, 502..=504) => return first,
            Err(ApiFault::TooLarge) => return first,
            _ => {}
        }
        tokio::time::sleep(RETRY_PAUSE).await;
        self.send_once(reqwest::Method::POST, bearer, path, Some(body), API_TIMEOUT)
            .await
    }

    async fn send_once(
        &self,
        method: reqwest::Method,
        bearer: &str,
        path: &str,
        json: Option<&serde_json::Value>,
        limit: Duration,
    ) -> Result<Api, ApiFault> {
        let base = self.credentials.app.endpoints.api_url.trim_end_matches('/');
        let mut request = self.http.request(method, format!("{base}{path}"));
        if let Some(json) = json {
            request = request.json(json);
        }
        let mut response = request
            .bearer_auth(bearer)
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", API_VERSION)
            .timeout(limit)
            .send()
            .await
            .map_err(|_| ApiFault::Unreachable)?;
        let status = response.status().as_u16();
        let headers = response.headers();
        let text = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
        };
        let number = |name: &str| text(name).and_then(|value| value.parse::<u64>().ok());
        let scopes = text("x-oauth-scopes").map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|scope| !scope.is_empty() && scope.len() <= 64)
                .take(32)
                .map(str::to_string)
                .collect()
        });
        let sso = text("x-github-sso").and_then(|value| {
            if value.starts_with("required") {
                Some(Sso::Required)
            } else if value.starts_with("partial-results") {
                Some(Sso::Partial)
            } else {
                None
            }
        });
        let next = text("link").and_then(|value| next_page(value, base));
        let remaining = number("x-ratelimit-remaining");
        let reset = number("x-ratelimit-reset");
        let retry_after = number("retry-after");
        if response
            .content_length()
            .is_some_and(|length| length > BODY_MAX as u64)
        {
            return Err(ApiFault::TooLarge);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ApiFault::Unreachable)? {
            if body.len().saturating_add(chunk.len()) > BODY_MAX {
                return Err(ApiFault::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let body: serde_json::Value =
            serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        // GitHub's documented rate-limit answers: 429, or 403 with no
        // requests left, a Retry-After, or a rate-limit message (the
        // secondary limits).
        let limited = status == 429
            || (status == 403
                && (remaining == Some(0)
                    || retry_after.is_some()
                    || body["message"]
                        .as_str()
                        .is_some_and(|m| m.to_ascii_lowercase().contains("rate limit"))));
        Ok(Api {
            status,
            scopes,
            body,
            next,
            sso,
            rate_limited: limited,
            remaining,
            reset,
        })
    }
}

/// How long one GitHub API read may take. A page of 100 repositories for
/// an account in large organizations can take GitHub several seconds.
const API_TIMEOUT: Duration = Duration::from_secs(15);
/// The pause before the one retry of a dropped or 502/503/504 read.
const RETRY_PAUSE: Duration = Duration::from_millis(500);

/// The `rel="next"` target of a `Link` header, as a path under `base`.
/// A next page on any other origin is not followed: the token would go
/// with it.
pub(crate) fn next_page(link: &str, base: &str) -> Option<String> {
    link.split(',').find_map(|part| {
        let (target, params) = part.trim().split_once(';')?;
        let target = target.trim().strip_prefix('<')?.strip_suffix('>')?;
        if !params
            .split(';')
            .any(|p| matches!(p.trim(), "rel=\"next\"" | "rel=next"))
        {
            return None;
        }
        let rest = target.strip_prefix(base)?;
        (rest.starts_with('/')
            && rest.len() <= 2048
            && !rest.chars().any(|c| c.is_whitespace() || c.is_control()))
        .then(|| rest.to_string())
    })
}

/// Why a GitHub API read produced no answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApiFault {
    /// No connection, a dropped one, or no answer in time.
    Unreachable,
    /// The answer was larger than [`BODY_MAX`].
    TooLarge,
}

impl From<ApiFault> for AuthError {
    fn from(_: ApiFault) -> Self {
        Self::Unavailable
    }
}

/// What `X-GitHub-SSO` said.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Sso {
    /// The resource belongs to an organization that requires single
    /// sign-on, and the token isn't authorized for it yet.
    Required,
    /// A listing left out such organizations' repositories.
    Partial,
}

/// A GitHub access token in memory. `Debug` never shows it, and the
/// bytes are overwritten when it is dropped.
pub(crate) struct Secret(String);

impl Secret {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Overwrite the bytes in place before the allocation is freed.
        let len = self.0.len();
        self.0.clear();
        self.0.extend(std::iter::repeat_n('\0', len));
        self.0.clear();
    }
}

/// What a token exchange or refresh returned.
pub(crate) struct Tokens {
    pub access: Secret,
    /// A GitHub App's user tokens expire (8 hours) and come with a
    /// refresh token; an OAuth App's tokens have neither.
    pub refresh: Option<Secret>,
    pub expires_in: Option<u64>,
}

impl Tokens {
    fn from_response(token: &oauth2::basic::BasicTokenResponse) -> Result<Self, AuthError> {
        let secret = token.access_token().secret();
        if secret.is_empty() || secret.len() > 1024 {
            return Err(AuthError::Denied);
        }
        Ok(Self {
            access: Secret(secret.clone()),
            refresh: token
                .refresh_token()
                .map(|r| r.secret())
                .filter(|r| !r.is_empty() && r.len() <= 1024)
                .map(|r| Secret(r.clone())),
            expires_in: token.expires_in().map(|d| d.as_secs()),
        })
    }
}

fn refused<E>(error: oauth2::RequestTokenError<E, oauth2::basic::BasicErrorResponse>) -> AuthError
where
    E: std::error::Error + 'static,
{
    match error {
        oauth2::RequestTokenError::ServerResponse(_) | oauth2::RequestTokenError::Parse(..) => {
            AuthError::Denied
        }
        _ => AuthError::Unavailable,
    }
}

/// One GitHub API answer.
pub(crate) struct Api {
    pub status: u16,
    pub scopes: Option<Vec<String>>,
    pub body: serde_json::Value,
    /// The next page's path, from `Link: <...>; rel="next"`.
    pub next: Option<String>,
    pub sso: Option<Sso>,
    /// GitHub said the token is over a rate limit.
    pub rate_limited: bool,
    /// `x-ratelimit-remaining`: reads left in this hour's budget.
    pub remaining: Option<u64>,
    /// `x-ratelimit-reset`: when the budget refills (Unix seconds).
    pub reset: Option<u64>,
}

/// GitHub's `/user`, tolerant of any extra fields.
#[derive(Deserialize)]
struct User {
    id: u64,
    login: String,
    node_id: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
    html_url: Option<String>,
    email: Option<String>,
    company: Option<String>,
    blog: Option<String>,
    location: Option<String>,
    bio: Option<String>,
    twitter_username: Option<String>,
    hireable: Option<bool>,
    #[serde(rename = "type")]
    kind: Option<String>,
    public_repos: Option<u64>,
    public_gists: Option<u64>,
    followers: Option<u64>,
    following: Option<u64>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct Email {
    email: String,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    primary: bool,
    visibility: Option<String>,
}

/// One line of text, control characters removed, bounded; empty is none.
fn line(value: Option<String>) -> Option<String> {
    let value: String = value?
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string();
    (!value.is_empty()).then(|| bounded(value))
}

fn bounded(mut value: String) -> String {
    if value.len() > MAX_FIELD {
        let mut end = MAX_FIELD;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
    }
    value
}

impl User {
    fn into_profile(self, emails: Vec<Email>) -> GithubProfile {
        let bio = self.bio.and_then(|bio| {
            let bio: String = bio
                .chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
                .collect::<String>()
                .trim()
                .to_string();
            (!bio.is_empty()).then(|| bounded(bio))
        });
        GithubProfile {
            id: self.id,
            login: self.login,
            node_id: line(self.node_id),
            name: line(self.name),
            avatar_url: line(self.avatar_url),
            html_url: line(self.html_url),
            email: line(self.email),
            emails: emails
                .into_iter()
                .filter_map(|e| {
                    let email = line(Some(e.email)).filter(|e| e.len() <= 320)?;
                    Some(GithubEmail {
                        email,
                        verified: e.verified,
                        primary: e.primary,
                        visibility: line(e.visibility),
                    })
                })
                .take(MAX_EMAILS)
                .collect(),
            company: line(self.company),
            blog: line(self.blog),
            location: line(self.location),
            bio,
            twitter_username: line(self.twitter_username),
            hireable: self.hireable,
            kind: line(self.kind),
            public_repos: self.public_repos,
            public_gists: self.public_gists,
            followers: self.followers,
            following: self.following,
            created_at: line(self.created_at),
            updated_at: line(self.updated_at),
        }
    }
}
