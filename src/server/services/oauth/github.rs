use super::{OAuthConfig, OAuthError, OAuthService};
use crate::server::models::user::User;
use oauth2::{basic::BasicTokenType, TokenResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubOAuth {
    service: OAuthService,
    http_client: Client,
    pool: PgPool,
}

impl GitHubOAuth {
    pub fn new(pool: PgPool, config: OAuthConfig) -> Result<Self, OAuthError> {
        Ok(Self {
            service: OAuthService::new(config)?,
            http_client: Client::new(),
            pool,
        })
    }

    pub fn authorization_url_for_login(
        &self,
        email: &str,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let mut url = self.service.authorization_url();
        url.0 = format!("{}&login_hint={}", url.0, email);
        url
    }

    pub fn authorization_url_for_signup(
        &self,
        email: &str,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let mut url = self.service.authorization_url();
        url.0 = format!("{}&login_hint={}", url.0, email);
        url
    }

    pub async fn authenticate(&self, code: String, _is_signup: bool) -> Result<User, OAuthError> {
        info!(
            "Processing GitHub authentication with code length: {}",
            code.len()
        );

        // Exchange code for token using PKCE
        let (_, _, pkce_verifier) = self.service.authorization_url();
        let token = self.service.exchange_code(code, pkce_verifier).await?;

        // Get GitHub user info
        let github_user = self.get_github_user(token.access_token().secret()).await?;

        // Create or update user
        self.get_or_create_user(github_user, token.access_token().secret())
            .await
    }

    async fn get_github_user(&self, token: &str) -> Result<GitHubUser, OAuthError> {
        info!("Fetching GitHub user info");

        let response = self
            .http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "OpenAgents")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch GitHub user: {}", e);
                OAuthError::AuthenticationFailed(e.to_string())
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("GitHub user fetch failed: {}", error_text);
            return Err(OAuthError::AuthenticationFailed(error_text));
        }

        response.json::<GitHubUser>().await.map_err(|e| {
            error!("Failed to parse GitHub user: {}", e);
            OAuthError::AuthenticationFailed(e.to_string())
        })
    }

    async fn get_or_create_user(
        &self,
        github_user: GitHubUser,
        access_token: &str,
    ) -> Result<User, OAuthError> {
        info!("Getting or creating user for GitHub ID: {}", github_user.id);

        // Store tokens and user info in metadata
        let metadata = serde_json::json!({
            "github": {
                "login": github_user.login,
                "name": github_user.name,
                "email": github_user.email
            }
        });

        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (github_id, github_token, metadata)
            VALUES ($1, $2, $3)
            ON CONFLICT (github_id) DO UPDATE
            SET github_token = $2,
                metadata = $3,
                last_login_at = NOW()
            RETURNING *
            "#,
            github_user.id,
            access_token,
            metadata as _
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error during user creation: {}", e);
            OAuthError::DatabaseError(e.to_string())
        })?;

        Ok(user)
    }
}
