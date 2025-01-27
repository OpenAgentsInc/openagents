use axum::http::StatusCode;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::server::models::user::User;

#[derive(Debug, Clone)]
pub struct OIDCConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
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
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidConfig => write!(f, "Invalid OIDC configuration"),
            AuthError::AuthenticationFailed => write!(f, "Authentication failed"),
            AuthError::TokenExchangeFailed(msg) => write!(f, "Token exchange failed: {}", msg),
            AuthError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
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
        }
    }
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

    pub fn authorization_url(&self) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid",
            self.auth_url,
            self.client_id,
            urlencoding::encode(&self.redirect_uri)
        )
    }

    pub async fn exchange_code(&self, code: String) -> Result<TokenResponse, AuthError> {
        let client = reqwest::Client::new();
        
        let response = client
            .post(&self.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &self.redirect_uri),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
            ])
            .send()
            .await
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::TokenExchangeFailed(error_text));
        }

        response
            .json::<TokenResponse>()
            .await
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))
    }

    pub async fn authenticate(&self, code: String, pool: &PgPool) -> Result<User, AuthError> {
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
            serde_json::json!({})
        )
        .fetch_one(pool)
        .await
        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(user)
    }
}

// Helper function to extract pseudonym from ID token
fn extract_pseudonym(id_token: &str) -> Result<String, AuthError> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return Err(AuthError::AuthenticationFailed);
    }

    let claims = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| AuthError::AuthenticationFailed)?;
        
    let claims: serde_json::Value = serde_json::from_slice(&claims)
        .map_err(|_| AuthError::AuthenticationFailed)?;

    claims["sub"].as_str()
        .ok_or(AuthError::AuthenticationFailed)
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::method;
    use serde_json::json;

    #[test]
    fn test_oidc_config_validation() {
        // Test invalid config (empty client_id)
        let result = OIDCConfig::new(
            "".to_string(),
            "secret".to_string(),
            "http://localhost:3000/callback".to_string(),
            "https://auth.scramble.com/authorize".to_string(),
            "https://auth.scramble.com/token".to_string(),
        );
        assert!(matches!(result, Err(AuthError::InvalidConfig)));

        // Test valid config
        let result = OIDCConfig::new(
            "client123".to_string(),
            "secret".to_string(),
            "http://localhost:3000/callback".to_string(),
            "https://auth.scramble.com/authorize".to_string(),
            "https://auth.scramble.com/token".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_authorization_url_generation() {
        let config = OIDCConfig::new(
            "client123".to_string(),
            "secret".to_string(),
            "http://localhost:3000/callback".to_string(),
            "https://auth.scramble.com/authorize".to_string(),
            "https://auth.scramble.com/token".to_string(),
        )
        .unwrap();

        let auth_url = config.authorization_url();
        let encoded_callback = urlencoding::encode("http://localhost:3000/callback");
        
        assert!(auth_url.starts_with("https://auth.scramble.com/authorize"));
        assert!(auth_url.contains("client_id=client123"));
        assert!(auth_url.contains("response_type=code"));
        assert!(auth_url.contains("scope=openid"));
        assert!(auth_url.contains(&*encoded_callback));
    }

    #[tokio::test]
    async fn test_token_exchange() {
        // Start mock server
        let mock_server = MockServer::start().await;

        // Create test config with mock server URL
        let config = OIDCConfig::new(
            "client123".to_string(),
            "secret456".to_string(),
            "http://localhost:3000/callback".to_string(),
            "https://auth.scramble.com/authorize".to_string(),
            mock_server.uri(),
        )
        .unwrap();

        // Setup successful token response
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "test_access_token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "id_token": "header.eyJzdWIiOiJ0ZXN0X3BzZXVkb255bSJ9.signature"
            })))
            .mount(&mock_server)
            .await;

        // Test successful token exchange
        let response = config.exchange_code("test_code".to_string()).await.unwrap();
        assert_eq!(response.access_token, "test_access_token");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, Some(3600));
    }
}