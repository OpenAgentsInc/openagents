use oauth2::{
    basic::BasicClient,
    AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
    EmptyExtraTokenFields, StandardTokenResponse, TokenResponse,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

pub mod github;
pub mod scramble;

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub auth_url: String,
    pub token_url: String,
}

#[derive(Debug, Clone)]
pub struct OAuthService {
    pub config: OAuthConfig,
    pub pool: PgPool,
    client: BasicClient,
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
        )
        .set_auth_uri(
            AuthUrl::new(config.auth_url.clone())
                .map_err(|e| OAuthError::InvalidConfig(e.to_string()))?,
        )
        .set_token_uri(
            TokenUrl::new(config.token_url.clone())
                .map_err(|e| OAuthError::InvalidConfig(e.to_string()))?,
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_url.clone())
                .map_err(|e| OAuthError::InvalidConfig(e.to_string()))?,
        );

        Ok(Self {
            client,
            config,
            pool,
        })
    }

    pub fn authorization_url(&self) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        let csrf_token = oauth2::CsrfToken::new_random();

        let auth_url = self
            .client
            .authorize_url(|| csrf_token.clone())
            .set_pkce_challenge(pkce_challenge)
            .url();

        (auth_url.to_string(), csrf_token, pkce_verifier)
    }

    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: oauth2::PkceCodeVerifier,
    ) -> Result<StandardTokenResponse<EmptyExtraTokenFields, oauth2::basic::BasicTokenType>, OAuthError> {
        self.client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(reqwest::Client::new())
            .await
            .map_err(|e| OAuthError::TokenExchangeFailed(e.to_string()))
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
