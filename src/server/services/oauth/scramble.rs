use super::{OAuthConfig, OAuthError, OAuthService};
use crate::server::models::user::User;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use oauth2::{
    AccessToken, BasicTokenResponse, EmptyExtraTokenFields, RefreshToken, Scope, StandardTokenResponse,
    TokenResponse, TokenType,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrambleTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    id_token: Option<String>,
}

impl TokenResponse<BasicTokenResponse> for ScrambleTokenResponse {
    fn access_token(&self) -> &AccessToken {
        // SAFETY: This is safe because we store the access token as a string
        unsafe { std::mem::transmute(&self.access_token) }
    }

    fn token_type(&self) -> &BasicTokenType {
        // SAFETY: This is safe because we store the token type as a string
        unsafe { std::mem::transmute(&self.token_type) }
    }

    fn expires_in(&self) -> Option<u64> {
        self.expires_in
    }

    fn refresh_token(&self) -> Option<&RefreshToken> {
        // SAFETY: This is safe because we store the refresh token as an Option<String>
        self.refresh_token.as_ref().map(|t| unsafe { std::mem::transmute(t) })
    }

    fn scopes(&self) -> Option<&Vec<Scope>> {
        None
    }
}

impl From<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>> for ScrambleTokenResponse {
    fn from(token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>) -> Self {
        Self {
            access_token: token.access_token().secret().to_string(),
            token_type: token.token_type().as_str().to_string(),
            expires_in: token.expires_in(),
            refresh_token: token.refresh_token().map(|t| t.secret().to_string()),
            id_token: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrambleOAuth {
    service: OAuthService,
}

impl ScrambleOAuth {
    pub fn new(pool: PgPool, config: OAuthConfig) -> Result<Self, OAuthError> {
        Ok(Self {
            service: OAuthService::new(pool, config)?,
        })
    }

    pub fn authorization_url_for_login(
        &self,
        email: &str,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let mut url = self.service.authorization_url();
        url.0 = format!("{}&flow=login&email={}&scope=openid", url.0, email);
        url
    }

    pub fn authorization_url_for_signup(
        &self,
        email: &str,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let mut url = self.service.authorization_url();
        url.0 = format!(
            "{}&flow=signup&prompt=create&email={}&scope=openid",
            url.0, email
        );
        url
    }

    pub fn authorization_url(
        &self,
        platform: Option<String>,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        self.service.authorization_url(platform)
    }

    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: oauth2::PkceCodeVerifier,
    ) -> Result<impl TokenResponse<BasicTokenResponse>, OAuthError> {
        let token = self.service
            .exchange_code(code)
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| OAuthError::TokenExchangeFailed(e.to_string()))?;

        let id_token = token.extra_fields()
            .get("id_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OAuthError::TokenExchangeFailed("Missing ID token".to_string()))?;

        Ok(ScrambleTokenResponse {
            access_token: token.access_token().secret().to_string(),
            token_type: token.token_type().as_ref().to_string(),
            expires_in: token.expires_in().map(|d| d.as_secs()),
            refresh_token: token.refresh_token().map(|t| t.secret().to_string()),
            id_token: Some(id_token.to_string()),
        })
    }

    pub async fn authenticate(&self, code: String, is_signup: bool) -> Result<User, OAuthError> {
        info!(
            "Processing {} with code length: {}",
            if is_signup { "signup" } else { "login" },
            code.len()
        );

        // Exchange code for tokens
        let (_, _, pkce_verifier) = self.service.authorization_url();
        let token = self.service.exchange_code(code, pkce_verifier).await?;

        // Extract claims from ID token
        let extra_fields = token.extra_fields();
        let id_token = extra_fields
            .get("id_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OAuthError::TokenExchangeFailed("No id_token in response".to_string()))?;

        let pseudonym = self.extract_pseudonym(id_token)?;

        // Handle user creation/update based on signup vs login
        if is_signup {
            self.handle_signup(pseudonym).await
        } else {
            self.handle_login(pseudonym).await
        }
    }

    async fn handle_signup(&self, pseudonym: String) -> Result<User, OAuthError> {
        info!("Handling signup for pseudonym: {}", pseudonym);

        // Check if user exists
        let existing_user = sqlx::query_as!(
            User,
            r#"
            SELECT id, scramble_id, github_id, github_token, metadata,
                   last_login_at, created_at, updated_at
            FROM users
            WHERE scramble_id = $1
            "#,
            pseudonym
        )
        .fetch_optional(&self.service.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        if existing_user.is_some() {
            return Err(OAuthError::UserCreationError(
                "User already exists".to_string(),
            ));
        }

        // Create new user
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (scramble_id, metadata, github_id, github_token)
            VALUES ($1, $2, NULL, NULL)
            RETURNING *
            "#,
            pseudonym,
            serde_json::json!({}) as _
        )
        .fetch_one(&self.service.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    async fn handle_login(&self, pseudonym: String) -> Result<User, OAuthError> {
        info!("Handling login for pseudonym: {}", pseudonym);

        // Get and update existing user
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET last_login_at = NOW()
            WHERE scramble_id = $1
            RETURNING *
            "#,
            pseudonym
        )
        .fetch_optional(&self.service.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        user.ok_or_else(|| OAuthError::AuthenticationFailed("User not found".to_string()))
    }

    fn extract_pseudonym(&self, id_token: &str) -> Result<String, OAuthError> {
        info!("Extracting pseudonym from token");
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            error!(
                "Invalid token format - expected 3 parts, got {}",
                parts.len()
            );
            return Err(OAuthError::TokenExchangeFailed(
                "Invalid token format".to_string(),
            ));
        }

        let claims = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|e| {
            error!("Failed to decode claims: {}", e);
            OAuthError::TokenExchangeFailed("Failed to decode claims".to_string())
        })?;

        let claims: serde_json::Value = serde_json::from_slice(&claims).map_err(|e| {
            error!("Failed to parse claims: {}", e);
            OAuthError::TokenExchangeFailed("Failed to parse claims".to_string())
        })?;

        info!("Parsed claims: {:?}", claims);

        claims["sub"]
            .as_str()
            .ok_or_else(|| {
                error!("No 'sub' claim found in token");
                OAuthError::TokenExchangeFailed("No 'sub' claim found in token".to_string())
            })
            .map(String::from)
    }
}
