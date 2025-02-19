use super::verifier_store::VerifierStore;
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
    id_token: Option<String>,
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
        let mut extra_fields = HashMap::new();

        // Log the complete token response
        let raw_response = serde_json::to_value(&token).unwrap_or_default();
        info!("Complete token response: {:?}", raw_response);

        // Extract id_token from the raw response
        let id_token = raw_response
            .get("id_token")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        info!("Extracted id_token present: {}", id_token.is_some());
        if let Some(ref token_str) = id_token {
            info!("ID token length: {}", token_str.len());
            extra_fields.insert("id_token".to_string(), token_str.clone());
        }

        Self {
            access_token: token.access_token().clone(),
            token_type: token.token_type().clone(),
            expires_in: token.expires_in(),
            refresh_token: token.refresh_token().cloned(),
            id_token,
            extra_fields,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrambleOAuth {
    service: OAuthService,
    pool: PgPool,
    verifier_store: VerifierStore,
}

impl ScrambleOAuth {
    pub fn new(pool: PgPool, config: OAuthConfig) -> Result<Self, OAuthError> {
        Ok(Self {
            service: OAuthService::new(config)?,
            pool,
            verifier_store: VerifierStore::new(),
        })
    }

    pub fn authorization_url_for_login(
        &self,
        email: &str,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        info!("Generating login authorization URL for email: {}", email);

        let (url, csrf_token, pkce_verifier) = self.service.authorization_url();
        info!("Base authorization URL: {}", url);

        self.verifier_store.store_verifier(
            csrf_token.secret(),
            oauth2::PkceCodeVerifier::new(pkce_verifier.secret().to_string()),
        );
        info!("Stored PKCE verifier for state: {}", csrf_token.secret());

        let final_url = format!("{}&flow=login&email={}&scope=openid+email", url, email);
        info!("Final authorization URL: {}", final_url);

        (final_url, csrf_token, pkce_verifier)
    }

    pub fn authorization_url_for_signup(
        &self,
        email: &str,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        info!("Generating signup authorization URL for email: {}", email);

        let (url, csrf_token, pkce_verifier) = self.service.authorization_url();
        info!("Base authorization URL: {}", url);

        // Store verifier with original state
        self.verifier_store.store_verifier(
            csrf_token.secret(),
            oauth2::PkceCodeVerifier::new(pkce_verifier.secret().to_string()),
        );
        info!("Stored PKCE verifier for state: {}", csrf_token.secret());

        // Add signup parameters without duplicating scopes
        let final_url = format!(
            "{}&flow=signup&is_signup=true&prompt=create&email={}",
            url, email
        );
        info!("Constructed final authorization URL: {}", final_url);

        (final_url, csrf_token, pkce_verifier)
    }

    pub async fn authorization_url(
        &self,
        platform: Option<String>,
    ) -> (String, oauth2::CsrfToken, oauth2::PkceCodeVerifier) {
        let (url, csrf_token, pkce_verifier) = self.service.authorization_url();
        self.verifier_store.store_verifier(
            csrf_token.secret(),
            oauth2::PkceCodeVerifier::new(pkce_verifier.secret().to_string()),
        );
        if let Some(platform) = platform {
            (
                format!("{}&platform={}", url, platform),
                csrf_token,
                pkce_verifier,
            )
        } else {
            (url, csrf_token, pkce_verifier)
        }
    }

    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: oauth2::PkceCodeVerifier,
    ) -> Result<BasicTokenResponse, OAuthError> {
        info!("Starting code exchange with code length: {}", code.len());
        info!(
            "PKCE verifier secret length: {}",
            pkce_verifier.secret().len()
        );

        match self.service.exchange_code(code, pkce_verifier).await {
            Ok(response) => {
                info!("Code exchange successful");
                Ok(response)
            }
            Err(e) => {
                error!("Code exchange failed: {}", e);
                Err(e)
            }
        }
    }

    pub async fn authenticate(
        &self,
        code: String,
        state: String,
        is_signup: bool,
    ) -> Result<User, OAuthError> {
        info!(
            "Starting {} authentication process with code length: {} and state: {}",
            if is_signup { "signup" } else { "login" },
            code.len(),
            state
        );

        // Remove _signup suffix if present to get original state
        let original_state = if state.ends_with("_signup") {
            state[..state.len() - 7].to_string()
        } else {
            state
        };
        info!(
            "Using original state for verifier lookup: {}",
            original_state
        );

        // Get stored verifier using original state
        let pkce_verifier = self
            .verifier_store
            .get_verifier(&original_state)
            .ok_or_else(|| {
                error!("No PKCE verifier found for state: {}", original_state);
                OAuthError::TokenExchangeFailed("No PKCE verifier found".to_string())
            })?;
        info!("Retrieved PKCE verifier for state: {}", original_state);

        // Exchange code for tokens
        info!("Exchanging code for tokens...");
        let token_response = match self
            .service
            .exchange_code(code.clone(), pkce_verifier)
            .await
        {
            Ok(response) => {
                info!("Successfully exchanged code for tokens");
                response
            }
            Err(e) => {
                error!("Failed to exchange code: {}", e);
                return Err(e);
            }
        };

        // Convert to our token response type
        let token: ScrambleTokenResponse = token_response.into();
        info!("Converted token response");

        // Extract claims from ID token
        let id_token = token.extra_fields.get("id_token").ok_or_else(|| {
            error!("No id_token found in token response");
            OAuthError::TokenExchangeFailed("No id_token in response".to_string())
        })?;
        info!("Extracted id_token from response");

        let pseudonym = self.extract_pseudonym(id_token)?;
        info!("Extracted pseudonym: {}", pseudonym);

        let email = self.extract_email(id_token)?;
        info!("Extracted email: {}", email);

        // Handle user creation/update based on signup vs login
        if is_signup {
            info!(
                "Handling signup for pseudonym: {} with email: {}",
                pseudonym, email
            );
            self.handle_signup(pseudonym, email).await
        } else {
            info!(
                "Handling login for pseudonym: {} with email: {}",
                pseudonym, email
            );
            self.handle_login(pseudonym, email).await
        }
    }

    async fn handle_signup(&self, pseudonym: String, email: String) -> Result<User, OAuthError> {
        info!(
            "Checking if user exists with pseudonym: {} or email: {}",
            pseudonym, email
        );
        let row = sqlx::query!(
            r#"
            SELECT id FROM users
            WHERE scramble_id = $1 OR email = $2
            "#,
            pseudonym,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        if row.is_some() {
            info!(
                "User already exists with pseudonym: {} or email: {}",
                pseudonym, email
            );
            // Update last_login_at and return as AuthenticationFailed error
            sqlx::query!(
                r#"
                UPDATE users
                SET last_login_at = NOW()
                WHERE scramble_id = $1 OR email = $2
                "#,
                pseudonym,
                email
            )
            .execute(&self.pool)
            .await
            .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

            return Err(OAuthError::AuthenticationFailed(
                "User already exists".to_string(),
            ));
        }

        // Create new user
        info!(
            "Creating new user with pseudonym: {} and email: {}",
            pseudonym, email
        );
        let user = sqlx::query!(
            r#"
            INSERT INTO users (scramble_id, metadata, github_id, github_token, pseudonym, email)
            VALUES ($1, $2, NULL, NULL, $1, $3)
            RETURNING id, scramble_id, github_id, github_token, metadata,
                      created_at, last_login_at, pseudonym, email
            "#,
            pseudonym,
            serde_json::json!({}) as JsonValue,
            email
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        Ok(User::builder(user.id)
            .scramble_id(user.scramble_id)
            .github_id(user.github_id)
            .github_token(user.github_token)
            .metadata(user.metadata.expect("metadata should never be null"))
            .created_at(DateTimeWrapper(
                user.created_at.expect("created_at should never be null"),
            ))
            .last_login_at(user.last_login_at.map(DateTimeWrapper))
            .pseudonym(user.pseudonym)
            .email(Some(user.email.expect("email should never be null")))
            .build())
    }

    async fn handle_login(&self, pseudonym: String, email: String) -> Result<User, OAuthError> {
        // Get and update existing user
        let row = sqlx::query!(
            r#"
            UPDATE users
            SET last_login_at = NOW(),
                email = COALESCE(email, $2)  -- Only update email if it's null
            WHERE scramble_id = $1
            RETURNING id, scramble_id, github_id, github_token, metadata,
                      created_at, last_login_at, pseudonym, email
            "#,
            pseudonym,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        match row {
            Some(row) => Ok(User::builder(row.id)
                .scramble_id(row.scramble_id)
                .github_id(row.github_id)
                .github_token(row.github_token)
                .metadata(row.metadata.expect("metadata should never be null"))
                .created_at(DateTimeWrapper(
                    row.created_at.expect("created_at should never be null"),
                ))
                .last_login_at(row.last_login_at.map(DateTimeWrapper))
                .pseudonym(row.pseudonym)
                .email(Some(row.email.expect("email should never be null")))
                .build()),
            None => Err(OAuthError::AuthenticationFailed(
                "User not found".to_string(),
            )),
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

    fn extract_email(&self, id_token: &str) -> Result<String, OAuthError> {
        info!("Extracting email from token");
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

        claims["email"]
            .as_str()
            .ok_or_else(|| {
                error!("No 'email' claim found in token");
                OAuthError::TokenExchangeFailed("No 'email' claim found in token".to_string())
            })
            .map(String::from)
    }

    #[allow(dead_code)]
    async fn get_user_by_scramble_id(&self, scramble_id: String) -> Result<User, OAuthError> {
        let row = sqlx::query!(
            r#"
            SELECT id, scramble_id, github_id, github_token, metadata,
                   created_at, last_login_at, pseudonym
            FROM users
            WHERE scramble_id = $1
            "#,
            scramble_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        match row {
            Some(row) => Ok(User::builder(row.id)
                .scramble_id(row.scramble_id)
                .github_id(row.github_id)
                .github_token(row.github_token)
                .metadata(row.metadata.expect("metadata should never be null"))
                .created_at(DateTimeWrapper(
                    row.created_at.expect("created_at should never be null"),
                ))
                .last_login_at(row.last_login_at.map(DateTimeWrapper))
                .pseudonym(row.pseudonym)
                .build()),
            None => Err(OAuthError::AuthenticationFailed(
                "User not found".to_string(),
            )),
        }
    }

    #[allow(dead_code)]
    async fn update_user_token(&self, id: i32, token: String) -> Result<User, OAuthError> {
        let row = sqlx::query!(
            r#"
            UPDATE users
            SET github_token = $1
            WHERE id = $2
            RETURNING id, scramble_id, github_id, github_token, metadata,
                      created_at, last_login_at, pseudonym
            "#,
            token,
            id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OAuthError::DatabaseError(e.to_string()))?;

        Ok(User::builder(row.id)
            .scramble_id(row.scramble_id)
            .github_id(row.github_id)
            .github_token(row.github_token)
            .metadata(row.metadata.expect("metadata should never be null"))
            .created_at(DateTimeWrapper(
                row.created_at.expect("created_at should never be null"),
            ))
            .last_login_at(row.last_login_at.map(DateTimeWrapper))
            .pseudonym(row.pseudonym)
            .build())
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
        return Ok(User::builder(row.id)
            .scramble_id(row.scramble_id)
            .github_id(row.github_id)
            .github_token(row.github_token)
            .metadata(row.metadata.expect("metadata should never be null"))
            .created_at(DateTimeWrapper(
                row.created_at.expect("created_at should never be null"),
            ))
            .last_login_at(row.last_login_at.map(DateTimeWrapper))
            .pseudonym(row.pseudonym)
            .build());
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

    Ok(User::builder(row.id)
        .scramble_id(row.scramble_id)
        .github_id(row.github_id)
        .github_token(row.github_token)
        .metadata(row.metadata.expect("metadata should never be null"))
        .created_at(DateTimeWrapper(
            row.created_at.expect("created_at should never be null"),
        ))
        .last_login_at(row.last_login_at.map(DateTimeWrapper))
        .pseudonym(row.pseudonym)
        .build())
}
