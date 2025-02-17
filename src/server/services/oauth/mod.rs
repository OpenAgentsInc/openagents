use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

pub mod github;
pub mod scramble;

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
}

#[derive(Debug, Clone)]
pub struct OAuthService {
    pub config: OAuthConfig,
    pub pool: PgPool,
    client: BasicClient,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
    pub id_token: Option<String>,
}

#[derive(Debug, Clone)]
pub enum OAuthError {
    InvalidConfig(String),
    AuthenticationFailed(String),
    TokenExchangeFailed(String),
    DatabaseError(String),
    UserCreationError(String),
}

impl OAuthService {
    pub fn new(pool: PgPool, config: OAuthConfig) -> Result<Self, OAuthError> {
        let client = BasicClient::new(
            ClientId::new(config.client_id.clone()),
            Some(ClientSecret::new(config.client_secret.clone())),
            AuthUrl::new(config.auth_url.clone()).map_err(|e| OAuthError::InvalidConfig(e.to_string()))?,
            Some(TokenUrl::new(config.token_url.clone()).map_err(|e| OAuthError::InvalidConfig(e.to_string()))?),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_uri.clone())
                .map_err(|e| OAuthError::InvalidConfig(e.to_string()))?,
        );

        Ok(Self {
            config,
            pool,
            client,
        })
    }

    pub fn authorization_url(&self, platform: Option<String>, extra_params: Vec<(&str, &str)>) -> String {
        let mut auth_request = self.client.authorize_url(|| uuid::Uuid::new_v4().to_string());

        // Add platform to state if provided
        if let Some(platform) = platform {
            auth_request = auth_request.add_extra_param("platform", platform);
        }

        // Add any additional parameters
        for (key, value) in extra_params {
            auth_request = auth_request.add_extra_param(key, value);
        }

        auth_request.url().to_string()
    }

    pub fn client(&self) -> &BasicClient {
        &self.client
    }
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthError::InvalidConfig(msg) => write!(f, "Invalid OAuth configuration: {}", msg),
            OAuthError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            OAuthError::TokenExchangeFailed(msg) => write!(f, "Token exchange failed: {}", msg),
            OAuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            OAuthError::UserCreationError(msg) => write!(f, "User creation error: {}", msg),
        }
    }
}

impl std::error::Error for OAuthError {}