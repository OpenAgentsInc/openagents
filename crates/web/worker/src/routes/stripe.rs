//! Stripe payment integration routes

use crate::db::users;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::stripe as stripe_service;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

#[derive(Serialize)]
struct StripeConfigResponse {
    publishable_key_available: bool,
}

#[derive(Serialize)]
struct PaymentMethod {
    id: String,
    pm_type: String,
    brand: Option<String>,
    last4: Option<String>,
    exp_month: Option<i32>,
    exp_year: Option<i32>,
    is_default: bool,
}

#[derive(Serialize)]
struct SetupIntentResponse {
    client_secret: String,
}

#[derive(Deserialize)]
struct StripeWebhookEvent {
    #[serde(rename = "type")]
    event_type: String,
    data: StripeEventData,
}

#[derive(Deserialize)]
struct StripeEventData {
    object: serde_json::Value,
}

/// Get Stripe configuration
pub async fn get_config(env: Env) -> Result<Response> {
    let has_key = env.var("STRIPE_PUBLISHABLE_KEY").is_ok();

    Response::from_json(&StripeConfigResponse {
        publishable_key_available: has_key,
    })
}

/// List payment methods for authenticated user
pub async fn list_payment_methods(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;

    // Get payment methods from D1
    let methods = db
        .prepare(
            "SELECT stripe_payment_method_id, pm_type, brand, last4, exp_month, exp_year, is_default
             FROM stripe_payment_methods WHERE user_id = ? ORDER BY is_default DESC, created_at DESC",
        )
        .bind(&[user.user_id.into()])?
        .all()
        .await?;

    let payment_methods: Vec<PaymentMethod> = methods
        .results::<serde_json::Value>()?
        .into_iter()
        .filter_map(|row| {
            Some(PaymentMethod {
                id: row.get("stripe_payment_method_id")?.as_str()?.to_string(),
                pm_type: row.get("pm_type")?.as_str()?.to_string(),
                brand: row.get("brand").and_then(|v| v.as_str()).map(String::from),
                last4: row.get("last4").and_then(|v| v.as_str()).map(String::from),
                exp_month: row.get("exp_month").and_then(|v| v.as_i64()).map(|v| v as i32),
                exp_year: row.get("exp_year").and_then(|v| v.as_i64()).map(|v| v as i32),
                is_default: row.get("is_default").and_then(|v| v.as_i64()).unwrap_or(0) == 1,
            })
        })
        .collect();

    Response::from_json(&payment_methods)
}

/// Create a Stripe SetupIntent for adding a payment method
pub async fn create_setup_intent(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    let stripe_secret = env.secret("STRIPE_SECRET_KEY")?.to_string();

    // Get or create Stripe customer
    let customer_id = get_or_create_stripe_customer(&db, &stripe_secret, &user).await?;

    // Create SetupIntent
    let setup_intent = stripe_service::create_setup_intent(&stripe_secret, &customer_id).await?;

    Response::from_json(&SetupIntentResponse {
        client_secret: setup_intent.client_secret,
    })
}

/// Handle Stripe webhooks
pub async fn webhook(env: Env, signature: String, body: Vec<u8>) -> Result<Response> {
    let webhook_secret = env.secret("STRIPE_WEBHOOK_SECRET")?.to_string();

    // Verify webhook signature
    if !stripe_service::verify_webhook_signature(&body, &signature, &webhook_secret) {
        return Response::error("Invalid signature", 400);
    }

    // Parse event
    let event: StripeWebhookEvent = serde_json::from_slice(&body)
        .map_err(|e| Error::RustError(format!("Invalid webhook payload: {}", e)))?;

    let db = env.d1("DB")?;

    match event.event_type.as_str() {
        "setup_intent.succeeded" => {
            let payment_method = event.data.object.get("payment_method")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::RustError("Missing payment_method".to_string()))?;

            let customer = event.data.object.get("customer")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::RustError("Missing customer".to_string()))?;

            // Get user by Stripe customer ID
            let user_id = db
                .prepare("SELECT user_id FROM stripe_customers WHERE stripe_customer_id = ?")
                .bind(&[customer.into()])?
                .first::<String>(Some("user_id"))
                .await?
                .ok_or_else(|| Error::RustError("Customer not found".to_string()))?;

            // Fetch payment method details from Stripe and save to D1
            let stripe_secret = env.secret("STRIPE_SECRET_KEY")?.to_string();
            let pm_details = stripe_service::get_payment_method(&stripe_secret, payment_method).await?;

            // Insert payment method
            let now = chrono::Utc::now().to_rfc3339();
            let brand_str = pm_details.brand.clone();
            let last4_str = pm_details.last4.clone();

            db.prepare(
                "INSERT INTO stripe_payment_methods (
                    stripe_payment_method_id, user_id, pm_type, brand, last4,
                    exp_month, exp_year, is_default, created_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?)
                 ON CONFLICT(stripe_payment_method_id) DO UPDATE SET
                    brand = excluded.brand, last4 = excluded.last4,
                    exp_month = excluded.exp_month, exp_year = excluded.exp_year"
            )
            .bind(&[
                payment_method.into(),
                user_id.clone().into(),
                pm_details.pm_type.clone().into(),
                brand_str.clone().map(|s| s.into()).unwrap_or(JsValue::NULL),
                last4_str.clone().map(|s| s.into()).unwrap_or(JsValue::NULL),
                pm_details.exp_month.map(|v| v.into()).unwrap_or(JsValue::NULL),
                pm_details.exp_year.map(|v| v.into()).unwrap_or(JsValue::NULL),
                now.into(),
            ])?
            .run()
            .await?;

            // Update user payment status
            users::update_payment_method(
                &db,
                &user_id,
                "valid",
                brand_str.as_deref(),
                last4_str.as_deref(),
            ).await?;
        }

        "payment_intent.succeeded" => {
            // Handle credit purchase completion
            if let Some(metadata) = event.data.object.get("metadata") {
                if let (Some(user_id), Some(credits)) = (
                    metadata.get("user_id").and_then(|v| v.as_str()),
                    metadata.get("credits").and_then(|v| v.as_str()).and_then(|s| s.parse::<i64>().ok()),
                ) {
                    users::update_credits(&db, user_id, credits, credits).await?;
                }
            }
        }

        _ => {
            // Ignore other events
        }
    }

    Response::from_json(&serde_json::json!({ "received": true }))
}

/// Get or create Stripe customer for user
async fn get_or_create_stripe_customer(
    db: &D1Database,
    stripe_secret: &str,
    user: &AuthenticatedUser,
) -> Result<String> {
    // Check if customer exists
    let existing = db
        .prepare("SELECT stripe_customer_id FROM stripe_customers WHERE user_id = ?")
        .bind(&[user.user_id.clone().into()])?
        .first::<String>(Some("stripe_customer_id"))
        .await?;

    if let Some(customer_id) = existing {
        return Ok(customer_id);
    }

    // Create new customer in Stripe
    let customer = stripe_service::create_customer(stripe_secret, &user.github_username).await?;

    // Store in D1
    let now = chrono::Utc::now().to_rfc3339();
    db.prepare(
        "INSERT INTO stripe_customers (user_id, stripe_customer_id, created_at)
         VALUES (?, ?, ?)",
    )
    .bind(&[
        user.user_id.clone().into(),
        customer.id.clone().into(),
        now.into(),
    ])?
    .run()
    .await?;

    Ok(customer.id)
}
