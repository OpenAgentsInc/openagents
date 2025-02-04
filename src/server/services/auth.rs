use axum::http::StatusCode;
use base64::engine::general_purpose::{URL_SAFE_NO_PAD, STANDARD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};

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
    UserAlreadyExists(User),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidConfig => write!(f, "Invalid OIDC configuration"),
            AuthError::AuthenticationFailed => write!(f, "Authentication failed"),
            AuthError::TokenExchangeFailed(msg) => write!(f, "Token exchange failed: {}", msg),
            AuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AuthError::UserAlreadyExists(_) => write!(f, "User already exists"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<AuthError> for StatusCode {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::InvalidConfig => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::AuthenticationFailed => StatusCode::BAD_GATEWAY,
            AuthError::TokenExchangeFailed(_) => StatusCode::BAD_GATEWAY,
            AuthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthError::UserAlreadyExists(_) => StatusCode::TEMPORARY_REDIRECT,
        }
    }
}

impl OIDCService {
    pub fn new(pool: PgPool, config: OIDCConfig) -> Self {
        Self { config, pool }
    }

    pub fn authorization_url_for_login(&self) -> Result<String, AuthError> {
        info!("Generating login authorization URL");
        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid&flow=login",
            self.config.auth_url,
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri)
        );
        info!("Generated login URL: {}", url);
        Ok(url)
    }

    pub fn authorization_url_for_signup(&self, email: &str) -> Result<String, AuthError> {
        info!("Generating signup authorization URL for email: {}", email);
        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid&flow=signup",
            self.config.auth_url,
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri)
        );
        url.push_str("&prompt=create");
        url.push_str(&format!("&email={}", urlencoding::encode(email)));
        info!("Generated signup URL: {}", url);
        Ok(url)
    }

    pub async fn login(&self, code: String) -> Result<User, AuthError> {
        info!("Processing login with code length: {}", code.len());

        // Exchange code for tokens
        let token_response = self.exchange_code(code).await?;
        info!("Successfully exchanged code for tokens");

        // Extract pseudonym from ID token claims
        let pseudonym = extract_pseudonym(&token_response.id_token)?;
        info!("Extracted pseudonym: {}", pseudonym);

        // Get or create user
        info!(
            "Attempting to get or create user with pseudonym: {}",
            pseudonym
        );
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
        .map_err(|e| {
            error!("Database error during login: {}", e);
            AuthError::DatabaseError(e.to_string())
        })?;

        info!("Successfully processed login for user: {:?}", user);
        Ok(user)
    }

    pub async fn signup(&self, code: String) -> Result<User, AuthError> {
        info!("Processing signup with code length: {}", code.len());

        // Exchange code for tokens
        let token_response = self.exchange_code(code).await?;
        info!("Successfully exchanged code for tokens");

        // Extract pseudonym from ID token claims
        let pseudonym = extract_pseudonym(&token_response.id_token)?;
        info!("Extracted pseudonym: {}", pseudonym);

        // Check if user already exists
        info!("Checking if user exists with pseudonym: {}", pseudonym);
        let existing_user = sqlx::query_as!(
            User,
            r#"
            SELECT id, scramble_id, metadata, last_login_at, created_at, updated_at
            FROM users 
            WHERE scramble_id = $1
            "#,
            pseudonym
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error checking existing user: {}", e);
            AuthError::DatabaseError(e.to_string())
        })?;

        if let Some(_) = existing_user {
            info!("User already exists with pseudonym: {}", pseudonym);
            // Update last_login_at and return as UserAlreadyExists
            let updated_user = sqlx::query_as!(
                User,
                r#"
                UPDATE users 
                SET last_login_at = NOW()
                WHERE scramble_id = $1
                RETURNING id, scramble_id, metadata, last_login_at, created_at, updated_at
                "#,
                pseudonym
            )
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error updating existing user: {}", e);
                AuthError::DatabaseError(e.to_string())
            })?;

            info!("Successfully updated existing user: {:?}", updated_user);
            return Err(AuthError::UserAlreadyExists(updated_user));
        }

        // Create new user
        info!("Creating new user with pseudonym: {}", pseudonym);
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
        .map_err(|e| {
            error!("Database error creating new user: {}", e);
            AuthError::DatabaseError(e.to_string())
        })?;

        info!("Successfully created new user: {:?}", user);
        Ok(user)
    }

    async fn exchange_code(&self, code: String) -> Result<TokenResponse, AuthError> {
        info!("Exchanging code for tokens, code length: {}", code.len());
        let client = reqwest::Client::new();

        info!(
            "Sending token exchange request to: {}",
            self.config.token_url
        );
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
            .map_err(|e| {
                error!("Failed to send token exchange request: {}", e);
                AuthError::TokenExchangeFailed(e.to_string())
            })?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        if !status.is_success() {
            error!(
                "Token exchange failed with status {}: {}",
                status, response_text
            );
            return Err(AuthError::TokenExchangeFailed(response_text));
        }

        info!("Received successful response from token endpoint");

        // Parse response as JSON
        let json_value: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            error!("Failed to parse token response as JSON: {}", e);
            AuthError::TokenExchangeFailed(e.to_string())
        })?;

        // Get id_token field
        let id_token = match json_value.get("id_token") {
            Some(token) => token.as_str().ok_or_else(|| {
                error!("id_token is not a string");
                AuthError::TokenExchangeFailed("id_token is not a string".to_string())
            })?,
            None => {
                error!("Response missing required id_token field");
                return Err(AuthError::TokenExchangeFailed(
                    "missing field `id_token`".to_string(),
                ));
            }
        };

        info!("Received id_token: {}", id_token);

        // Validate JWT format after successful parsing
        if !is_valid_jwt_format(id_token) {
            error!("Invalid JWT format in id_token");
            return Err(AuthError::TokenExchangeFailed(
                "Invalid JWT format".to_string(),
            ));
        }

        // Now try to parse into TokenResponse
        let token_response: TokenResponse = serde_json::from_value(json_value).map_err(|e| {
            error!("Failed to parse token response: {}", e);
            AuthError::TokenExchangeFailed(format!("error decoding response body: {}", e))
        })?;

        info!("Successfully parsed token response");
        Ok(token_response)
    }
}

fn is_valid_jwt_format(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        error!(
            "Invalid JWT format: wrong number of parts ({})",
            parts.len()
        );
        return false;
    }

        // Try to decode each part as base64
    for (i, part) in parts[..2].iter().enumerate() {
        info!("Attempting to decode part {}: {}", i, part);
        let standard_result = STANDARD.decode(part);
        let url_safe_result = URL_SAFE_NO_PAD.decode(part);
        
        if standard_result.is_err() && url_safe_result.is_err() {
            error!("Invalid JWT format: part {} is not valid base64", i);
            error!("Standard decode error: {:?}", standard_result.err());
            error!("URL-safe decode error: {:?}", url_safe_result.err());
            return false;
        }
    }

    true
}

fn extract_pseudonym(id_token: &str) -> Result<String, AuthError> {
    info!("Extracting pseudonym from token: {}", id_token);
    let parts: Vec<&str> = id_token.split('.').collect();
    info!("Token parts: {:?}", parts);
    
    if parts.len() != 3 {
        error!(
            "Invalid token format - expected 3 parts, got {}",
            parts.len()
        );
        return Err(AuthError::TokenExchangeFailed(
            "Invalid token format".to_string(),
        ));
    }

    let claims = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|e| {
        error!("Failed to decode claims with URL_SAFE_NO_PAD: {}", e);
        AuthError::TokenExchangeFailed("Failed to decode claims".to_string())
    })?;

    let claims: serde_json::Value = serde_json::from_slice(&claims).map_err(|e| {
        error!("Failed to parse claims as JSON: {}", e);
        AuthError::TokenExchangeFailed("Failed to parse claims".to_string())
    })?;

    info!("Parsed claims: {:?}", claims);

    claims["sub"]
        .as_str()
        .ok_or_else(|| {
            error!("No 'sub' claim found in token");
            AuthError::TokenExchangeFailed("No 'sub' claim found in token".to_string())
        })
        .map(String::from)
}
