//! User database operations for D1

use crate::identity;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

/// User record from D1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub github_id: String,
    pub github_username: String,
    pub nostr_npub: Option<String>,
    pub signup_credits: i64,
    pub purchased_credits: i64,
    pub credits_balance: i64,
    pub payment_method_status: String,
    pub payment_method_brand: Option<String>,
    pub payment_method_last4: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
struct IdentityRow {
    nostr_private_key_encrypted: Option<String>,
    bitcoin_xpriv_encrypted: Option<String>,
    nostr_public_key: Option<String>,
    nostr_npub: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CredentialRow {
    github_access_token_encrypted: Option<String>,
    nostr_private_key_encrypted: Option<String>,
    bitcoin_xpriv_encrypted: Option<String>,
}

/// Create or update a user from GitHub OAuth
pub async fn upsert_from_github(
    db: &D1Database,
    github_id: &str,
    github_username: &str,
    email: Option<&str>,
    access_token: &str,
    session_secret: &str,
) -> Result<User> {
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Try to find existing user
    let existing = db
        .prepare("SELECT user_id FROM users WHERE github_id = ?")
        .bind(&[github_id.into()])?
        .first::<String>(Some("user_id"))
        .await?;

    if let Some(existing_id) = existing {
        let identity_material = ensure_identity(db, &existing_id, session_secret).await?;
        let credentials_key = identity::derive_credentials_key(&identity_material);
        let encrypted_token =
            identity::encrypt_with_key(&credentials_key, access_token.as_bytes())?;

        // Update existing user
        db.prepare(
            "UPDATE users SET
                github_username = ?,
                github_access_token_encrypted = ?,
                email = COALESCE(?, email),
                updated_at = ?
             WHERE user_id = ?",
        )
        .bind(&[
            github_username.into(),
            encrypted_token.into(),
            email.map(|e| e.into()).unwrap_or(JsValue::NULL),
            now.clone().into(),
            existing_id.clone().into(),
        ])?
        .run()
        .await?;

        get_by_id(db, &existing_id).await
    } else {
        let (stored_identity, identity_material) =
            identity::generate_identity_bundle(session_secret)?;
        let credentials_key = identity::derive_credentials_key(&identity_material);
        let encrypted_token =
            identity::encrypt_with_key(&credentials_key, access_token.as_bytes())?;

        // Create new user
        db.prepare(
            "INSERT INTO users (
                user_id, github_id, github_username, email,
                github_access_token_encrypted,
                nostr_public_key, nostr_npub,
                nostr_private_key_encrypted, bitcoin_xpriv_encrypted,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&[
            user_id.clone().into(),
            github_id.into(),
            github_username.into(),
            email.map(|e| e.into()).unwrap_or(JsValue::NULL),
            encrypted_token.into(),
            stored_identity.nostr_public_key.into(),
            stored_identity.nostr_npub.into(),
            stored_identity.nostr_private_key_encrypted.into(),
            stored_identity.bitcoin_xpriv_encrypted.into(),
            now.clone().into(),
            now.into(),
        ])?
        .run()
        .await?;

        get_by_id(db, &user_id).await
    }
}

/// Get user by ID
pub async fn get_by_id(db: &D1Database, user_id: &str) -> Result<User> {
    let result = db
        .prepare(
            "SELECT user_id, email, username, github_id, github_username,
                    nostr_npub,
                    signup_credits, purchased_credits, credits_balance,
                    payment_method_status, payment_method_brand, payment_method_last4,
                    created_at
             FROM users WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(&[user_id.into()])?
        .first::<User>(None)
        .await?;

    result.ok_or_else(|| Error::RustError("User not found".to_string()))
}

/// Get decrypted GitHub access token for API calls
pub async fn get_github_access_token(
    db: &D1Database,
    user_id: &str,
    session_secret: &str,
) -> Result<String> {
    let row = db
        .prepare(
            "SELECT github_access_token_encrypted,
                    nostr_private_key_encrypted, bitcoin_xpriv_encrypted
             FROM users WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(&[user_id.into()])?
        .first::<CredentialRow>(None)
        .await?
        .ok_or_else(|| Error::RustError("User not found".to_string()))?;

    let token = row
        .github_access_token_encrypted
        .ok_or_else(|| Error::RustError("No GitHub token".to_string()))?;

    if !token.starts_with("v1:") {
        if let (Some(nostr_priv), Some(bitcoin_xpriv)) = (
            row.nostr_private_key_encrypted.as_ref(),
            row.bitcoin_xpriv_encrypted.as_ref(),
        ) {
            let identity_material = identity::decrypt_identity(
                session_secret,
                nostr_priv,
                bitcoin_xpriv,
            )?;
            let credentials_key = identity::derive_credentials_key(&identity_material);
            let encrypted_token =
                identity::encrypt_with_key(&credentials_key, token.as_bytes())?;
            let now = chrono::Utc::now().to_rfc3339();

            db.prepare(
                "UPDATE users SET
                    github_access_token_encrypted = ?,
                    updated_at = ?
                 WHERE user_id = ?",
            )
            .bind(&[encrypted_token.into(), now.into(), user_id.into()])?
            .run()
            .await?;
        }

        return Ok(token);
    }

    let nostr_priv = row
        .nostr_private_key_encrypted
        .ok_or_else(|| Error::RustError("Missing identity keys".to_string()))?;
    let bitcoin_xpriv = row
        .bitcoin_xpriv_encrypted
        .ok_or_else(|| Error::RustError("Missing identity keys".to_string()))?;
    let identity_material = identity::decrypt_identity(session_secret, &nostr_priv, &bitcoin_xpriv)?;
    let credentials_key = identity::derive_credentials_key(&identity_material);
    let token_bytes = identity::decrypt_with_key(&credentials_key, &token)?;
    let token_str = String::from_utf8(token_bytes)
        .map_err(|e| Error::RustError(format!("Invalid token encoding: {}", e)))?;

    Ok(token_str)
}

/// Get user by GitHub ID
pub async fn get_by_github_id(db: &D1Database, github_id: &str) -> Result<Option<User>> {
    db.prepare(
        "SELECT user_id, email, username, github_id, github_username,
                nostr_npub,
                signup_credits, purchased_credits, credits_balance,
                payment_method_status, payment_method_brand, payment_method_last4,
                created_at
         FROM users WHERE github_id = ? AND deleted_at IS NULL",
    )
    .bind(&[github_id.into()])?
    .first::<User>(None)
    .await
}

/// Get user by username (for HUD lookups)
pub async fn get_by_github_username(db: &D1Database, username: &str) -> Result<Option<User>> {
    db.prepare(
        "SELECT user_id, email, username, github_id, github_username,
                nostr_npub,
                signup_credits, purchased_credits, credits_balance,
                payment_method_status, payment_method_brand, payment_method_last4,
                created_at
         FROM users WHERE github_username = ? AND deleted_at IS NULL",
    )
    .bind(&[username.into()])?
    .first::<User>(None)
    .await
}

/// Soft delete a user
pub async fn soft_delete(db: &D1Database, user_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "UPDATE users SET
            deleted_at = ?,
            github_access_token_encrypted = NULL,
            api_key_encrypted = NULL,
            handoff_token_encrypted = NULL,
            nostr_public_key = NULL,
            nostr_npub = NULL,
            nostr_private_key_encrypted = NULL,
            bitcoin_xpriv_encrypted = NULL
         WHERE user_id = ?",
    )
    .bind(&[now.into(), user_id.into()])?
    .run()
    .await?;

    Ok(())
}

/// Generate and store a new API key
pub async fn generate_api_key(
    db: &D1Database,
    user_id: &str,
    session_secret: &str,
) -> Result<String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    let api_key = format!(
        "oa_{}",
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
    );

    let identity_material = ensure_identity(db, user_id, session_secret).await?;
    let credentials_key = identity::derive_credentials_key(&identity_material);
    let encrypted_key = identity::encrypt_with_key(&credentials_key, api_key.as_bytes())?;
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare("UPDATE users SET api_key_encrypted = ?, updated_at = ? WHERE user_id = ?")
        .bind(&[encrypted_key.into(), now.into(), user_id.into()])?
        .run()
        .await?;

    Ok(api_key)
}

/// Get decrypted identity material for a user (ensures identity exists).
pub async fn get_identity_material(
    db: &D1Database,
    user_id: &str,
    session_secret: &str,
) -> Result<identity::IdentityMaterial> {
    ensure_identity(db, user_id, session_secret).await
}

async fn ensure_identity(
    db: &D1Database,
    user_id: &str,
    session_secret: &str,
) -> Result<identity::IdentityMaterial> {
    let row = db
        .prepare(
            "SELECT nostr_private_key_encrypted, bitcoin_xpriv_encrypted,
                    nostr_public_key, nostr_npub
             FROM users WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(&[user_id.into()])?
        .first::<IdentityRow>(None)
        .await?
        .ok_or_else(|| Error::RustError("User not found".to_string()))?;

    if let (Some(nostr_priv), Some(bitcoin_xpriv)) = (
        row.nostr_private_key_encrypted.as_ref(),
        row.bitcoin_xpriv_encrypted.as_ref(),
    ) {
        let identity_material =
            identity::decrypt_identity(session_secret, nostr_priv, bitcoin_xpriv)?;

        if row.nostr_public_key.is_none() || row.nostr_npub.is_none() {
            let public_key = identity::nostr_public_key_from_private(
                &identity_material.nostr_private_key,
            )?;
            let public_key_hex = hex::encode(public_key);
            let npub = identity::nostr_npub_from_private(
                &identity_material.nostr_private_key,
            )?;
            let now = chrono::Utc::now().to_rfc3339();

            db.prepare(
                "UPDATE users SET
                    nostr_public_key = ?,
                    nostr_npub = ?,
                    updated_at = ?
                 WHERE user_id = ?",
            )
            .bind(&[
                public_key_hex.into(),
                npub.into(),
                now.into(),
                user_id.into(),
            ])?
            .run()
            .await?;
        }

        return Ok(identity_material);
    }

    let (stored_identity, identity_material) =
        identity::generate_identity_bundle(session_secret)?;
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "UPDATE users SET
            nostr_public_key = ?,
            nostr_npub = ?,
            nostr_private_key_encrypted = ?,
            bitcoin_xpriv_encrypted = ?,
            updated_at = ?
         WHERE user_id = ?",
    )
    .bind(&[
        stored_identity.nostr_public_key.into(),
        stored_identity.nostr_npub.into(),
        stored_identity.nostr_private_key_encrypted.into(),
        stored_identity.bitcoin_xpriv_encrypted.into(),
        now.into(),
        user_id.into(),
    ])?
    .run()
    .await?;

    Ok(identity_material)
}

/// Update credits balance
pub async fn update_credits(
    db: &D1Database,
    user_id: &str,
    purchased_delta: i64,
    balance_delta: i64,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "UPDATE users SET
            purchased_credits = purchased_credits + ?,
            credits_balance = credits_balance + ?,
            updated_at = ?
         WHERE user_id = ?",
    )
    .bind(&[
        purchased_delta.into(),
        balance_delta.into(),
        now.into(),
        user_id.into(),
    ])?
    .run()
    .await?;

    Ok(())
}

/// Update payment method status
pub async fn update_payment_method(
    db: &D1Database,
    user_id: &str,
    status: &str,
    brand: Option<&str>,
    last4: Option<&str>,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "UPDATE users SET
            payment_method_status = ?,
            payment_method_brand = ?,
            payment_method_last4 = ?,
            payment_method_added_at = ?,
            updated_at = ?
         WHERE user_id = ?",
    )
    .bind(&[
        status.into(),
        brand.map(|b| b.into()).unwrap_or(JsValue::NULL),
        last4.map(|l| l.into()).unwrap_or(JsValue::NULL),
        now.clone().into(),
        now.into(),
        user_id.into(),
    ])?
    .run()
    .await?;

    Ok(())
}
