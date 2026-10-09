//! The account-service half: trade a GitHub authorization code (and its
//! PKCE verifier) for the person's GitHub profile.
//!
//! The access token lives only inside [`Github::profile`]: it reads
//! `/user` and `/user/emails` and is dropped. No redirects are followed,
//! every call is time-bounded, and response bodies are size-bounded.

use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, PkceCodeVerifier, RedirectUrl,
    TokenResponse, TokenUrl,
};
use serde::Deserialize;
use tenancy::accounts::identities::{GithubEmail, GithubProfile, MAX_EMAILS, MAX_FIELD};

use crate::AuthError;
use crate::config::GithubCredentials;

/// The largest GitHub API body read.
const BODY_MAX: usize = 256 * 1024;

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
        let app = &self.credentials.app;
        let client = BasicClient::new(ClientId::new(app.client_id.clone()))
            .set_client_secret(ClientSecret::new(self.credentials.secret().to_string()))
            .set_auth_uri(
                AuthUrl::new(app.endpoints.authorize_url.clone())
                    .map_err(|_| AuthError::Unavailable)?,
            )
            .set_token_uri(
                TokenUrl::new(app.endpoints.token_url.clone())
                    .map_err(|_| AuthError::Unavailable)?,
            )
            .set_redirect_uri(
                RedirectUrl::new(app.redirect_url.clone()).map_err(|_| AuthError::Unavailable)?,
            )
            .set_auth_type(AuthType::RequestBody);
        let token = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(PkceCodeVerifier::new(verifier.to_string()))
            .request_async(&self.http)
            .await
            .map_err(|error| match error {
                oauth2::RequestTokenError::ServerResponse(_)
                | oauth2::RequestTokenError::Parse(..) => AuthError::Denied,
                _ => AuthError::Unavailable,
            })?;
        let bearer = token.access_token().secret();
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

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        bearer: &str,
        path: &str,
    ) -> Result<Option<T>, AuthError> {
        let url = format!(
            "{}{path}",
            self.credentials.app.endpoints.api_url.trim_end_matches('/')
        );
        let mut response = self
            .http
            .get(url)
            .bearer_auth(bearer)
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28")
            .send()
            .await
            .map_err(|_| AuthError::Unavailable)?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| AuthError::Unavailable)? {
            body.extend_from_slice(&chunk);
            if body.len() > BODY_MAX {
                return Err(AuthError::Unavailable);
            }
        }
        Ok(serde_json::from_slice(&body).ok())
    }
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
