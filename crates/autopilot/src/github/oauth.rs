//! GitHub OAuth authentication flow
//!
//! Implements the OAuth 2.0 flow for authenticating with GitHub.
//! Supports both initial authorization and token refresh.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::models::TokenInfo;

/// GitHub OAuth configuration
#[derive(Debug, Clone)]
pub struct GitHubOAuth {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

/// OAuth authorization state (CSRF protection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub state: String,
    pub created_at: DateTime<Utc>,
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            state: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now(),
        }
    }

    /// Check if state is still valid (within 10 minutes)
    pub fn is_valid(&self) -> bool {
        let age = Utc::now() - self.created_at;
        age < Duration::minutes(10)
    }
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

/// GitHub OAuth token response
#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

impl GitHubOAuth {
    /// Create a new OAuth client
    ///
    /// # Arguments
    /// * `client_id` - GitHub OAuth App client ID
    /// * `client_secret` - GitHub OAuth App client secret
    /// * `redirect_uri` - OAuth callback URL
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
        }
    }

    /// Create from environment variables
    ///
    /// Expects:
    /// - GITHUB_CLIENT_ID
    /// - GITHUB_CLIENT_SECRET
    /// - GITHUB_REDIRECT_URI (optional, defaults to http://localhost:8080/callback)
    pub fn from_env() -> Result<Self> {
        let client_id =
            std::env::var("GITHUB_CLIENT_ID").context("GITHUB_CLIENT_ID not set")?;
        let client_secret =
            std::env::var("GITHUB_CLIENT_SECRET").context("GITHUB_CLIENT_SECRET not set")?;
        let redirect_uri = std::env::var("GITHUB_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:8080/callback".to_string());

        Ok(Self::new(client_id, client_secret, redirect_uri))
    }

    /// Generate the OAuth authorization URL
    ///
    /// Returns the URL to redirect the user to and the state for CSRF protection.
    ///
    /// Required scopes:
    /// - `repo` - Full control of private repositories
    /// - `read:org` - Read org membership
    /// - `workflow` - Update GitHub Action workflows
    pub fn start_auth_flow(&self) -> (String, AuthState) {
        let state = AuthState::new();

        // Required scopes for Autopilot:
        // - repo: Read code, create branches, push commits, create PRs
        // - read:org: Read organization membership for private repos
        // - workflow: Trigger and read GitHub Actions status
        let scopes = "repo read:org workflow";

        let url = format!(
            "https://github.com/login/oauth/authorize?\
            client_id={}&\
            redirect_uri={}&\
            scope={}&\
            state={}",
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(scopes),
            urlencoding::encode(&state.state)
        );

        (url, state)
    }

    /// Exchange authorization code for access token
    ///
    /// # Arguments
    /// * `code` - The authorization code from GitHub callback
    /// * `state` - The state parameter for CSRF validation
    /// * `expected_state` - The expected state from `start_auth_flow`
    pub async fn exchange_code(
        &self,
        code: &str,
        state: &str,
        expected_state: &AuthState,
    ) -> Result<TokenInfo> {
        // Validate state (CSRF protection)
        if state != expected_state.state {
            anyhow::bail!("Invalid OAuth state - possible CSRF attack");
        }
        if !expected_state.is_valid() {
            anyhow::bail!("OAuth state expired");
        }

        let client = reqwest::Client::new();
        let response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("code", code),
                ("redirect_uri", self.redirect_uri.as_str()),
            ])
            .send()
            .await
            .context("Failed to exchange OAuth code")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GitHub OAuth error: {} - {}", status, body);
        }

        let token_response: GitHubTokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        // Calculate expiration time if provided
        let expires_at = token_response
            .expires_in
            .map(|secs| Utc::now() + Duration::seconds(secs));

        Ok(TokenInfo {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            scope: token_response.scope,
            refresh_token: token_response.refresh_token,
            expires_at,
        })
    }

    /// Refresh an expired access token
    ///
    /// Note: GitHub OAuth apps don't support refresh tokens by default.
    /// This only works if the app is configured with token expiration.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenInfo> {
        let client = reqwest::Client::new();
        let response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .context("Failed to refresh token")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GitHub token refresh error: {} - {}", status, body);
        }

        let token_response: GitHubTokenResponse = response
            .json()
            .await
            .context("Failed to parse refresh response")?;

        let expires_at = token_response
            .expires_in
            .map(|secs| Utc::now() + Duration::seconds(secs));

        Ok(TokenInfo {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            scope: token_response.scope,
            refresh_token: token_response.refresh_token,
            expires_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_creation() {
        let state = AuthState::new();
        assert!(!state.state.is_empty());
        assert!(state.is_valid());
    }

    #[test]
    fn test_auth_url_generation() {
        let oauth = GitHubOAuth::new(
            "test_client_id".to_string(),
            "test_secret".to_string(),
            "http://localhost:8080/callback".to_string(),
        );

        let (url, state) = oauth.start_auth_flow();

        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains(&state.state));
        assert!(url.contains("scope="));
    }
}
