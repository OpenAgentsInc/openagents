use super::{OAuthConfig, OAuthError, OAuthService};
use crate::server::models::{timestamp::DateTimeWrapper, user::User};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use oauth2::{
    basic::{BasicTokenResponse, BasicTokenType},
    AccessToken, EmptyExtraTokenFields, RefreshToken, Scope, StandardTokenResponse, TokenResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::{types::JsonValue, PgPool};
use std::collections::HashMap;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrambleTokenResponse {
    access_token: AccessToken,
    token_type: BasicTokenType,
    expires_in: Option<std::time::Duration>,
    refresh_token: Option<RefreshToken>,
    extra_fields: HashMap<String, String>,
}

impl TokenResponse<BasicTokenType> for ScrambleTokenResponse {
    fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    fn token_type(&self) -> &BasicTokenType {
        &self.token_type
    }

    fn expires_in(&self) -> Option<std::time::Duration> {
        self.expires_in
    }

    fn refresh_token(&self) -> Option<&RefreshToken> {
        self.refresh_token.as_ref()
    }

    fn scopes(&self) -> Option<&Vec<Scope>> {
        None
    }
}

impl From<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>> for ScrambleTokenResponse {
    fn from(token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>) -> Self {
        Self {
            access_token: token.access_token().clone(),
            token_type: token.token_type().clone(),
            expires_in: token.expires_in(),
            refresh_token: token.refresh_token().cloned(),
            extra_fields: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrambleOAuth {
    service: OAuthService,
    pool: PgPool,
}

impl ScrambleOAuth {
    pub fn new(pool: PgPool, config: OAuthConfig) -> Result<Self, OAuthError> {
        Ok(Self {
            service: OAuthService::new(config)?,
            pool,
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

    pub async fn authorization_url(
        &self,
        platform: Option<String>,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let (url, token, verifier) = self.service.authorization_url();
        if let Some(platform) = platform {
            (format!("{}&platform={}", url, platform), token, verifier)
        } else {
            (url, token, verifier)
        }
    }

    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: oauth2::PkceCodeVerifier,
    ) -> Result<BasicTokenResponse, OAuthError> {
        self.service.exchange_code(code, pkce_verifier).await
    }

    pub async fn authenticate(&self, code: String, is_signup: bool) -> Result<User, OAuthError> {
        info!(
            "Processing {} with code length: {}",
            if is_signup { "signup" } else { "login" },
            code.len()
        );

        // Exchange code for tokens
        let (_, _, pkce_verifier) = self.service.authorization_url();
        let token_response = self.service.exchange_code(code, pkce_verifier).await?;

        // Convert to our token response type
        let token: ScrambleTokenResponse = token_response.into();

        // Extract claims from ID token
        let id_token = token.extra_fields.get("id_token").ok_or_else(|| {
            OAuthError::TokenExchangeFailed("No id_token in response".to_string())
        })?;

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
        let row = sqlx::query!(
            r#"
            SELECT id, scramble_id, github_id, github_token, metadata,
                   created_at, last_login_at, pseudonym
            FROM users
            WHERE scramble_id = $1
            "#,
            pseudonym
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        if let Some(row) = row {
            return Ok(User::new(
                row.id,
                row.scramble_id,
                row.github_id,
                row.github_token,
                row.metadata.expect("metadata should never be null"),
                DateTimeWrapper(row.created_at.expect("created_at should never be null")),
                row.last_login_at.map(DateTimeWrapper),
                row.pseudonym,
            ));
        }

        // Create new user
        let row = sqlx::query!(
            r#"
            INSERT INTO users (scramble_id, metadata, github_id, github_token)
            VALUES ($1, $2, NULL, NULL)
            RETURNING id, scramble_id, github_id, github_token, metadata, created_at, last_login_at, pseudonym
            "#,
            pseudonym,
            serde_json::json!({}) as JsonValue
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        Ok(User::new(
            row.id,
            row.scramble_id,
            row.github_id,
            row.github_token,
            row.metadata.expect("metadata should never be null"),
            DateTimeWrapper(row.created_at.expect("created_at should never be null")),
            row.last_login_at.map(DateTimeWrapper),
            row.pseudonym,
        ))
    }

    async fn handle_login(&self, pseudonym: String) -> Result<User, OAuthError> {
        info!("Handling login for pseudonym: {}", pseudonym);

        // Get and update existing user
        let row = sqlx::query!(
            r#"
            UPDATE users
            SET last_login_at = NOW()
            WHERE scramble_id = $1
            RETURNING id, scramble_id, github_id, github_token, metadata, created_at, last_login_at, pseudonym
            "#,
            pseudonym
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        if let Some(row) = row {
            Ok(User::new(
                row.id,
                row.scramble_id,
                row.github_id,
                row.github_token,
                row.metadata.expect("metadata should never be null"),
                DateTimeWrapper(row.created_at.expect("created_at should never be null")),
                row.last_login_at.map(DateTimeWrapper),
                row.pseudonym,
            ))
        } else {
            Err(OAuthError::AuthenticationFailed(
                "User not found".to_string(),
            ))
        }
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

pub async fn get_or_create_user(pool: &PgPool, pseudonym: &str) -> Result<User, OAuthError> {
    let row = sqlx::query!(
        r#"
        SELECT id, scramble_id, github_id, github_token, metadata,
        created_at, last_login_at, pseudonym
        FROM users
        WHERE pseudonym = $1
        "#,
        pseudonym
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

    if let Some(row) = row {
        return Ok(User::new(
            row.id,
            row.scramble_id,
            row.github_id,
            row.github_token,
            row.metadata.expect("metadata should never be null"),
            DateTimeWrapper(row.created_at.expect("created_at should never be null")),
            row.last_login_at.map(DateTimeWrapper),
            row.pseudonym,
        ));
    }

    let metadata = serde_json::json!({
        "pseudonym": pseudonym,
    });

    let row = sqlx::query!(
        r#"
        INSERT INTO users (pseudonym, metadata)
        VALUES ($1, $2)
        RETURNING id, scramble_id, github_id, github_token, metadata,
        created_at, last_login_at, pseudonym
        "#,
        pseudonym,
        metadata
    )
    .fetch_one(pool)
    .await
    .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

    Ok(User::new(
        row.id,
        row.scramble_id,
        row.github_id,
        row.github_token,
        row.metadata.expect("metadata should never be null"),
        DateTimeWrapper(row.created_at.expect("created_at should never be null")),
        row.last_login_at.map(DateTimeWrapper),
        row.pseudonym,
    ))
}
