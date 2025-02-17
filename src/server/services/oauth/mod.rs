use oauth2::{
    basic::{BasicClient, BasicTokenType},
    AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, error};
use thiserror::Error;

pub mod github;
pub mod scramble;

#[derive(Debug)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub auth_url: String,
    pub token_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("Failed to create OAuth client: {0}")]
    ClientCreationFailed(String),
    #[error("Failed to exchange token: {0}")]
    TokenExchangeFailed(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("User creation error: {0}")]
    UserCreationError(String),
}

#[derive(Debug, Clone)]
pub struct OAuthService {
    client: BasicClient,
}

impl OAuthService {
    pub fn new(config: OAuthConfig) -> Result<Self, OAuthError> {
        let client = BasicClient::new(
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
            AuthUrl::new(config.auth_url).map_err(|e| OAuthError::ClientCreationFailed(e.to_string()))?,
            Some(TokenUrl::new(config.token_url).map_err(|e| OAuthError::ClientCreationFailed(e.to_string()))?),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_url)
                .map_err(|e| OAuthError::ClientCreationFailed(e.to_string()))?,
        );

        Ok(Self { client })
    }

    pub fn authorization_url(&self) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        let (auth_url, csrf_token) = self
            .client
            .authorize_url(oauth2::CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge)
            .url();

        (auth_url.to_string(), csrf_token, pkce_verifier)
    }

    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: oauth2::PkceCodeVerifier,
    ) -> Result<oauth2::basic::BasicTokenResponse, OAuthError> {
        self.client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| OAuthError::TokenExchangeFailed(e.to_string()))
    }
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthError::ClientCreationFailed(msg) => write!(f, "Failed to create OAuth client: {}", msg),
            OAuthError::TokenExchangeFailed(msg) => write!(f, "Failed to exchange token: {}", msg),
            OAuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            OAuthError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            OAuthError::UserCreationError(msg) => write!(f, "User creation error: {}", msg),
        }
    }
}

impl std::error::Error for OAuthError {}
