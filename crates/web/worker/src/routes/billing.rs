//! Billing and credits routes

use crate::db::users;
use crate::middleware::auth::AuthenticatedUser;
use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Serialize)]
struct BalanceResponse {
    total_credits: i64,
    signup_credits: i64,
    purchased_credits: i64,
    formatted: String,
}

#[derive(Serialize)]
struct Plan {
    id: String,
    name: String,
    description: String,
    credits_per_month: i64,
    price_cents: i64,
}

#[derive(Serialize)]
struct CreditPackage {
    id: String,
    name: String,
    credits: i64,
    price_cents: i64,
    price_per_1k: f64,
}

#[derive(Deserialize)]
struct PurchaseRequest {
    package_id: String,
}

/// Get current credit balance
pub async fn get_balance(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let db = env.d1("DB")?;
    let user_record = users::get_by_id(&db, &user.user_id).await?;

    let balance = BalanceResponse {
        total_credits: user_record.credits_balance,
        signup_credits: user_record.signup_credits,
        purchased_credits: user_record.purchased_credits,
        formatted: format_credits(user_record.credits_balance),
    };

    Response::from_json(&balance)
}

/// List available plans
pub async fn list_plans() -> Result<Response> {
    let plans = vec![
        Plan {
            id: "free".to_string(),
            name: "Free".to_string(),
            description: "100K credits on signup".to_string(),
            credits_per_month: 0,
            price_cents: 0,
        },
        Plan {
            id: "pro".to_string(),
            name: "Pro".to_string(),
            description: "500K credits/month + priority support".to_string(),
            credits_per_month: 500_000,
            price_cents: 2900,
        },
        Plan {
            id: "team".to_string(),
            name: "Team".to_string(),
            description: "2M credits/month + team features".to_string(),
            credits_per_month: 2_000_000,
            price_cents: 9900,
        },
    ];

    Response::from_json(&plans)
}

/// List available credit packages
pub async fn list_credit_packages() -> Result<Response> {
    let packages = vec![
        CreditPackage {
            id: "credits_100k".to_string(),
            name: "100K Credits".to_string(),
            credits: 100_000,
            price_cents: 800,
            price_per_1k: 0.08,
        },
        CreditPackage {
            id: "credits_500k".to_string(),
            name: "500K Credits".to_string(),
            credits: 500_000,
            price_cents: 3500,
            price_per_1k: 0.07,
        },
        CreditPackage {
            id: "credits_1m".to_string(),
            name: "1M Credits".to_string(),
            credits: 1_000_000,
            price_cents: 6000,
            price_per_1k: 0.06,
        },
        CreditPackage {
            id: "credits_5m".to_string(),
            name: "5M Credits".to_string(),
            credits: 5_000_000,
            price_cents: 25000,
            price_per_1k: 0.05,
        },
    ];

    Response::from_json(&packages)
}

/// Purchase credits
pub async fn purchase_credits(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let user_record = users::get_by_id(&db, &user.user_id).await?;

    // Check payment method
    if user_record.payment_method_status != "valid" {
        return Response::error("No valid payment method", 400);
    }

    // Parse request
    let request: PurchaseRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    // Find package
    let (credits, price_cents) = match request.package_id.as_str() {
        "credits_100k" => (100_000i64, 800i64),
        "credits_500k" => (500_000, 3500),
        "credits_1m" => (1_000_000, 6000),
        "credits_5m" => (5_000_000, 25000),
        _ => return Response::error("Invalid package", 400),
    };

    // In production: charge via Stripe here
    // For now, just add the credits

    users::update_credits(&db, &user.user_id, credits, credits).await?;

    Response::from_json(&serde_json::json!({
        "success": true,
        "credits_added": credits,
        "amount_charged_cents": price_cents
    }))
}

fn format_credits(credits: i64) -> String {
    if credits >= 1_000_000 {
        format!("{:.1}M", credits as f64 / 1_000_000.0)
    } else if credits >= 1_000 {
        format!("{:.1}K", credits as f64 / 1_000.0)
    } else {
        credits.to_string()
    }
}
