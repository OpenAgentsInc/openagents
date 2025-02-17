use super::{OAuthConfig, OAuthError, OAuthService, TokenInfo};
use crate::server::models::user::User;
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, TokenResponse,
};
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

pub struct GitHubOAuth {
    service: OAuthService,
    http_client: Client,
}

impl GitHubOAuth {
    pub fn new(pool: PgPool, config: OAuthConfig) -> Result<Self, OAuthError> {
        Ok(Self {
            service: OAuthService::new(pool, config)?,
            http_client: Client::new(),
        })
    }

    pub fn authorization_url(&self, platform: Option<String>) -> (String, CsrfToken, PkceCodeVerifier) {
        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Get the auth URL with PKCE challenge
        let mut auth_request = self
            .service
            .client()
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        // Add scopes
        auth_request = auth_request.add_scope("user".into());
        auth_request = auth_request.add_scope("repo".into());

        // Add platform if provided
        if let Some(platform) = platform {
            auth_request = auth_request.add_extra_param("platform", platform);
        }

        let (auth_url, csrf_token) = auth_request.url();

        (auth_url.to_string(), csrf_token, pkce_verifier)
    }

    pub async fn authenticate(
        &self,
        code: String,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<User, OAuthError> {
        // Exchange the code for a token using PKCE
        let token = self
            .service
            .client()
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| OAuthError::TokenExchangeFailed(e.to_string()))?;

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

    async fn get_or_create_user(&self, github_user: GitHubUser, access_token: &str) -> Result<User, OAuthError> {
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
        .fetch_one(&self.service.pool)
        .await
        .map_err(|e| {
            error!("Database error during user creation: {}", e);
            OAuthError::DatabaseError(e.to_string())
        })?;

        Ok(user)
    }
}