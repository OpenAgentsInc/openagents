use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use thiserror::Error;
use tracing::{error, info};

pub mod github;
pub mod scramble;
pub mod verifier_store;

#[derive(Debug, Default, Clone)]
pub struct EmptyExtraTokenFields {
    id_token: Option<String>,
}

impl EmptyExtraTokenFields {
    pub fn new() -> Self {
        Self { id_token: None }
    }

    pub fn id_token(&self) -> Option<&str> {
        self.id_token.as_deref()
    }

    pub fn set_id_token(&mut self, token: String) {
        self.id_token = Some(token);
    }
}

#[derive(Debug)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub auth_url: String,
    pub token_url: String,
}

#[derive(Debug, Error)]
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
            AuthUrl::new(config.auth_url)
                .map_err(|e| OAuthError::ClientCreationFailed(e.to_string()))?,
            Some(
                TokenUrl::new(config.token_url)
                    .map_err(|e| OAuthError::ClientCreationFailed(e.to_string()))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_url)
                .map_err(|e| OAuthError::ClientCreationFailed(e.to_string()))?,
        );

        Ok(Self { client })
    }

    pub fn authorization_url(&self) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        info!("Generated PKCE challenge and verifier");

        let (auth_url, csrf_token) = self
            .client
            .authorize_url(oauth2::CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge)
            .url();

        info!(
            "Generated base authorization URL: {} with state: {}",
            auth_url,
            csrf_token.secret()
        );

        (auth_url.to_string(), csrf_token, pkce_verifier)
    }

    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: oauth2::PkceCodeVerifier,
    ) -> Result<oauth2::basic::BasicTokenResponse, OAuthError> {
        info!("OAuth service exchanging code for tokens");
        info!(
            "Code length: {}, PKCE verifier length: {}",
            code.len(),
            pkce_verifier.secret().len()
        );

        let result = self
            .client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await;

        match &result {
            Ok(_) => info!("Token exchange successful"),
            Err(e) => error!("Token exchange failed: {}", e),
        }

        result.map_err(|e| OAuthError::TokenExchangeFailed(e.to_string()))
    }
}
