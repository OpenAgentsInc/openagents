use crate::server::services::auth::AuthError;
use axum::http::StatusCode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};

use crate::server::models::user::User;

#[derive(Debug, Clone)]
pub struct GitHubAuthService {
    pub config: GitHubConfig,
    pub pool: PgPool,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct GitHubConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Clone)]
pub enum GitHubAuthError {
    InvalidConfig,
    AuthenticationFailed,
    TokenExchangeFailed(String),
    DatabaseError(String),
    UserAlreadyExists(User),
}

impl std::fmt::Display for GitHubAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitHubAuthError::InvalidConfig => write!(f, "Invalid GitHub configuration"),
            GitHubAuthError::AuthenticationFailed => write!(f, "Authentication failed"),
            GitHubAuthError::TokenExchangeFailed(msg) => {
                write!(f, "Token exchange failed: {}", msg)
            }
            GitHubAuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            GitHubAuthError::UserAlreadyExists(_) => write!(f, "User already exists"),
        }
    }
}

impl std::error::Error for GitHubAuthError {}

impl From<GitHubAuthError> for StatusCode {
    fn from(error: GitHubAuthError) -> Self {
        match error {
            GitHubAuthError::InvalidConfig => StatusCode::INTERNAL_SERVER_ERROR,
            GitHubAuthError::AuthenticationFailed => StatusCode::BAD_GATEWAY,
            GitHubAuthError::TokenExchangeFailed(_) => StatusCode::BAD_GATEWAY,
            GitHubAuthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GitHubAuthError::UserAlreadyExists(_) => StatusCode::TEMPORARY_REDIRECT,
        }
    }
}

impl GitHubAuthService {
    pub fn new(pool: PgPool, config: GitHubConfig) -> Self {
        Self {
            config,
            pool,
            client: Client::new(),
        }
    }

    pub fn authorization_url(&self, platform: Option<String>) -> Result<String, GitHubAuthError> {
        info!(
            "Generating GitHub authorization URL with platform: {:?}",
            platform
        );

        // Add platform to state if provided, otherwise use empty string
        let state = platform.unwrap_or_default();
        info!("Using state value: {}", state);

        let url = format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user%20repo&state={}",
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri),
            urlencoding::encode(&state)
        );
        info!("Generated GitHub URL: {}", url);
        Ok(url)
    }

    pub async fn authenticate(&self, code: String) -> Result<User, GitHubAuthError> {
        info!(
            "Processing GitHub authentication with code length: {}",
            code.len()
        );

        // Exchange code for token
        let token_response = self.exchange_code(code).await?;
        info!("Successfully exchanged code for GitHub token");

        // Get GitHub user info
        let github_user = self.get_github_user(&token_response.access_token).await?;
        info!("Retrieved GitHub user info: {:?}", github_user);

        // Get or create user
        let user = self.get_or_create_user(github_user, token_response).await?;
        info!("Successfully processed GitHub auth for user: {:?}", user);

        Ok(user)
    }

    async fn exchange_code(&self, code: String) -> Result<GitHubTokenResponse, GitHubAuthError> {
        info!("Exchanging code for GitHub token");
        let client = reqwest::Client::new();

        let response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("code", &code),
                ("redirect_uri", &self.config.redirect_uri),
            ])
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send GitHub token exchange request: {}", e);
                GitHubAuthError::TokenExchangeFailed(e.to_string())
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("GitHub token exchange failed: {}", error_text);
            return Err(GitHubAuthError::TokenExchangeFailed(error_text));
        }

        response.json::<GitHubTokenResponse>().await.map_err(|e| {
            error!("Failed to parse GitHub token response: {}", e);
            GitHubAuthError::TokenExchangeFailed(e.to_string())
        })
    }

    async fn get_github_user(&self, token: &str) -> Result<GitHubUser, GitHubAuthError> {
        info!("Fetching GitHub user info");
        let client = reqwest::Client::new();

        let response = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "OpenAgents")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch GitHub user: {}", e);
                GitHubAuthError::AuthenticationFailed
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("GitHub user fetch failed: {}", error_text);
            return Err(GitHubAuthError::AuthenticationFailed);
        }

        response.json::<GitHubUser>().await.map_err(|e| {
            error!("Failed to parse GitHub user: {}", e);
            GitHubAuthError::AuthenticationFailed
        })
    }

    async fn get_or_create_user(
        &self,
        github_user: GitHubUser,
        tokens: GitHubTokenResponse,
    ) -> Result<User, GitHubAuthError> {
        info!("Getting or creating user for GitHub ID: {}", github_user.id);

        // Store tokens and user info in metadata
        let metadata = serde_json::json!({
            "github": {
                "login": github_user.login,
                "name": github_user.name,
                "email": github_user.email,
                "scope": tokens.scope
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
            tokens.access_token,
            metadata as _
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error during user creation: {}", e);
            GitHubAuthError::DatabaseError(e.to_string())
        })?;

        Ok(user)
    }

    pub async fn exchange_code_for_token(
        &self,
        code: &str,
    ) -> Result<GitHubTokenResponse, AuthError> {
        let token_url = "https://github.com/login/oauth/access_token";

        let response = self
            .client
            .post(token_url)
            .json(&serde_json::json!({
                "client_id": self.config.client_id,
                "client_secret": self.config.client_secret,
                "code": code
            }))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to exchange code for token: {}", e);
                AuthError::TokenExchangeFailed(e.to_string())
            })?;

        response.json::<GitHubTokenResponse>().await.map_err(|e| {
            error!("Failed to parse token response: {}", e);
            AuthError::TokenExchangeFailed(format!("error decoding response body: {}", e))
        })
    }

    pub async fn process_auth_code(&self, code: &str) -> Result<User, AuthError> {
        info!(
            "Processing GitHub authentication with code length: {}",
            code.len()
        );

        // Exchange code for token
        let tokens = self.exchange_code_for_token(code).await?;

        // Get user info with token
        let github_user = self
            .get_github_user(&tokens.access_token)
            .await
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))?;

        // Create or update user
        self.get_or_create_user(github_user, tokens)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))
    }
}
