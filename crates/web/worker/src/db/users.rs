//! User database operations for D1

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
    pub github_access_token: Option<String>,
    pub signup_credits: i64,
    pub purchased_credits: i64,
    pub credits_balance: i64,
    pub payment_method_status: String,
    pub payment_method_brand: Option<String>,
    pub payment_method_last4: Option<String>,
    pub created_at: String,
}

/// Create or update a user from GitHub OAuth
pub async fn upsert_from_github(
    db: &D1Database,
    github_id: &str,
    github_username: &str,
    email: Option<&str>,
    access_token_encrypted: &str,
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
            access_token_encrypted.into(),
            email.map(|e| e.into()).unwrap_or(JsValue::NULL),
            now.clone().into(),
            existing_id.clone().into(),
        ])?
        .run()
        .await?;

        get_by_id(db, &existing_id).await
    } else {
        // Create new user
        db.prepare(
            "INSERT INTO users (
                user_id, github_id, github_username, email,
                github_access_token_encrypted, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&[
            user_id.clone().into(),
            github_id.into(),
            github_username.into(),
            email.map(|e| e.into()).unwrap_or(JsValue::NULL),
            access_token_encrypted.into(),
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
                    github_access_token_encrypted as github_access_token,
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

/// Get user by GitHub ID
pub async fn get_by_github_id(db: &D1Database, github_id: &str) -> Result<Option<User>> {
    db.prepare(
        "SELECT user_id, email, username, github_id, github_username,
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
            handoff_token_encrypted = NULL
         WHERE user_id = ?",
    )
    .bind(&[now.into(), user_id.into()])?
    .run()
    .await?;

    Ok(())
}

/// Generate and store a new API key
pub async fn generate_api_key(db: &D1Database, user_id: &str) -> Result<String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    let api_key = format!(
        "oa_{}",
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
    );

    // In production, this should be encrypted before storage
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare("UPDATE users SET api_key_encrypted = ?, updated_at = ? WHERE user_id = ?")
        .bind(&[api_key.clone().into(), now.into(), user_id.into()])?
        .run()
        .await?;

    Ok(api_key)
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
