use super::{OAuthConfig, OAuthError, OAuthService};
use crate::server::models::user::User;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use oauth2::TokenResponse;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrambleTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<i32>,
    id_token: String,
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
    ) -> Result<impl TokenResponse, OAuthError> {
        let token = self.service.exchange_code(code, pkce_verifier).await?;
        let id_token = token
            .extra_fields()
            .id_token
            .ok_or_else(|| OAuthError::TokenExchange("Missing ID token".to_string()))?;

        Ok(token)
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
        let id_token = token
            .extra_fields()
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
