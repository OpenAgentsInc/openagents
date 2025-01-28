use axum::http::StatusCode;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use tracing::{debug, error, info};

use crate::server::models::user::User;

#[derive(Debug, Clone)]
pub struct OIDCService {
    pub config: OIDCConfig,
    pub pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct OIDCConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
}

impl OIDCConfig {
    pub fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        auth_url: String,
        token_url: String,
    ) -> Result<Self, AuthError> {
        if client_id.is_empty() || client_secret.is_empty() || redirect_uri.is_empty() {
            return Err(AuthError::InvalidConfig);
        }

        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
            auth_url,
            token_url,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<i32>,
    id_token: String,
}

#[derive(Debug, Clone)]
pub enum AuthError {
    InvalidConfig,
    AuthenticationFailed,
    TokenExchangeFailed(String),
    DatabaseError(String),
    UserAlreadyExists,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidConfig => write!(f, "Invalid OIDC configuration"),
            AuthError::AuthenticationFailed => write!(f, "Authentication failed"),
            AuthError::TokenExchangeFailed(msg) => write!(f, "Token exchange failed: {}", msg),
            AuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AuthError::UserAlreadyExists => write!(f, "User already exists"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<AuthError> for StatusCode {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidConfig => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::AuthenticationFailed => StatusCode::UNAUTHORIZED,
            AuthError::TokenExchangeFailed(_) => StatusCode::BAD_GATEWAY,
            AuthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::UserAlreadyExists => StatusCode::CONFLICT,
        }
    }
}

impl OIDCService {
    pub fn new(pool: PgPool, config: OIDCConfig) -> Self {
        Self { config, pool }
    }

    pub fn authorization_url_for_login(&self) -> Result<String, AuthError> {
        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid",
            self.config.auth_url,
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri)
        );
        Ok(url)
    }

    pub fn authorization_url_for_signup(&self) -> Result<String, AuthError> {
        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid",
            self.config.auth_url,
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri)
        );
        url.push_str("&prompt=create");
        Ok(url)
    }

    pub async fn login(&self, code: String) -> Result<User, AuthError> {
        // Exchange code for tokens
        let token_response = self.exchange_code(code).await?;

        // Extract pseudonym from ID token claims
        let pseudonym = extract_pseudonym(&token_response.id_token)
            .map_err(|_| AuthError::AuthenticationFailed)?;

        // Get or create user
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (scramble_id, metadata)
            VALUES ($1, $2)
            ON CONFLICT (scramble_id) DO UPDATE
            SET last_login_at = NOW()
            RETURNING id, scramble_id, metadata, last_login_at, created_at, updated_at
            "#,
            pseudonym,
            serde_json::json!({}) as _
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    pub async fn signup(&self, code: String) -> Result<User, AuthError> {
        debug!("Starting signup process with code length: {}", code.len());

        // Exchange code for tokens
        let token_response = self.exchange_code(code).await?;
        debug!("Received token response");

        // Extract pseudonym from ID token claims
        let pseudonym = extract_pseudonym(&token_response.id_token)
            .map_err(|e| {
                error!("Failed to extract pseudonym: {:?}", e);
                AuthError::AuthenticationFailed
            })?;
        info!("Extracted pseudonym: {}", pseudonym);

        // Check if user already exists
        let existing_user = sqlx::query!(
            "SELECT id FROM users WHERE scramble_id = $1",
            pseudonym
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        if existing_user.is_some() {
            debug!("User already exists with pseudonym: {}", pseudonym);
            return Err(AuthError::UserAlreadyExists);
        }

        // Create new user
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (scramble_id, metadata)
            VALUES ($1, $2)
            RETURNING id, scramble_id, metadata, last_login_at, created_at, updated_at
            "#,
            pseudonym,
            serde_json::json!({}) as _
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        info!("Created new user with pseudonym: {}", pseudonym);
        Ok(user)
    }

    async fn exchange_code(&self, code: String) -> Result<TokenResponse, AuthError> {
        debug!("Exchanging code for tokens");
        let client = reqwest::Client::new();

        let response = client
            .post(&self.config.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &self.config.redirect_uri),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
            ])
            .send()
            .await
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("Token exchange failed: {}", error_text);
            return Err(AuthError::TokenExchangeFailed(error_text));
        }

        let token_response = response
            .json::<TokenResponse>()
            .await
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))?;
        debug!("Successfully exchanged code for tokens");
        Ok(token_response)
    }
}

// Helper function to extract pseudonym from ID token
fn extract_pseudonym(id_token: &str) -> Result<String, AuthError> {
    debug!("Extracting pseudonym from token: {}", id_token);
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        error!("Invalid token format - expected 3 parts, got {}", parts.len());
        return Err(AuthError::AuthenticationFailed);
    }

    let claims = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| {
            error!("Failed to decode claims: {}", e);
            AuthError::AuthenticationFailed
        })?;

    let claims: serde_json::Value =
        serde_json::from_slice(&claims).map_err(|e| {
            error!("Failed to parse claims: {}", e);
            AuthError::AuthenticationFailed
        })?;

    debug!("Parsed claims: {:?}", claims);

    claims["sub"]
        .as_str()
        .ok_or_else(|| {
            error!("No 'sub' claim found in token");
            AuthError::AuthenticationFailed
        })
        .map(String::from)
}