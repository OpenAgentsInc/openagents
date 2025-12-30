//! Account management routes

use crate::db::users;
use crate::middleware::auth::AuthenticatedUser;
use serde::Serialize;
use worker::*;

#[derive(Serialize)]
struct AccountSettings {
    user_id: String,
    email: Option<String>,
    username: Option<String>,
    github_username: String,
    credits_balance: i64,
    signup_credits: i64,
    purchased_credits: i64,
    payment_method_status: String,
    payment_method_brand: Option<String>,
    payment_method_last4: Option<String>,
}

/// Get account settings
pub async fn get_settings(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    let user_record = users::get_by_id(&db, &user.user_id).await?;

    let settings = AccountSettings {
        user_id: user_record.user_id,
        email: user_record.email,
        username: user_record.username,
        github_username: user_record.github_username,
        credits_balance: user_record.credits_balance,
        signup_credits: user_record.signup_credits,
        purchased_credits: user_record.purchased_credits,
        payment_method_status: user_record.payment_method_status,
        payment_method_brand: user_record.payment_method_brand,
        payment_method_last4: user_record.payment_method_last4,
    };

    Response::from_json(&settings)
}

/// Generate a new API key
pub async fn generate_api_key(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    let api_key = users::generate_api_key(&db, &user.user_id).await?;

    Response::from_json(&serde_json::json!({
        "api_key": api_key,
        "message": "Store this key securely - it won't be shown again"
    }))
}

/// Delete account (soft delete)
pub async fn delete_account(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    users::soft_delete(&db, &user.user_id).await?;

    // Clear session
    let kv = env.kv("SESSIONS")?;
    crate::db::sessions::Session::delete(&kv, &user.session_token).await?;

    let mut headers = Headers::new();
    headers.set(
        "Set-Cookie",
        &crate::db::sessions::clear_session_cookie(),
    )?;

    Ok(Response::from_json(&serde_json::json!({
        "message": "Account deleted"
    }))?
    .with_headers(headers))
}
