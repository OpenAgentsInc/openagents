// Stripe integration for credit purchases

use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

#[derive(Debug, Deserialize)]
pub struct CreateCheckoutRequest {
    pub credits: u64, // Number of credits to purchase
}

#[derive(Debug, Serialize)]
pub struct CheckoutSession {
    pub session_id: String,
    pub url: String,
}

pub async fn create_checkout_session(
    req: web::Json<CreateCheckoutRequest>,
) -> Result<HttpResponse> {
    info!("Creating Stripe checkout session for {} credits", req.credits);

    // Pricing: $20 for 500,000 credits
    // Calculate price based on credits requested
    let price_per_credit = 20.0 / 500_000.0; // $0.00004 per credit
    let total_price = (req.credits as f64 * price_per_credit * 100.0) as i64; // Convert to cents

    let _stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set");

    // In production, use stripe-rust crate to create actual session
    // For now, return mock response
    let session_id = format!("cs_test_{}", rand::random::<u64>());
    let checkout_url = format!("https://checkout.stripe.com/c/pay/{}", session_id);

    info!("Created checkout session: {}, total: ${:.2}", session_id, total_price as f64 / 100.0);

    Ok(HttpResponse::Ok().json(CheckoutSession {
        session_id: session_id.clone(),
        url: checkout_url,
    }))
}

pub async fn checkout_success(req: HttpRequest) -> Result<HttpResponse> {
    // Extract session_id from query params
    let query_string = req.query_string();
    info!("Checkout success: {}", query_string);

    Ok(HttpResponse::Ok().content_type("text/html").body(r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Payment Successful - OpenAgents</title>
            <style>
                body {
                    font-family: 'Vera Mono', monospace;
                    background: #0a0a0a;
                    color: #e0e0e0;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    height: 100vh;
                    margin: 0;
                }
                .container {
                    text-align: center;
                    max-width: 500px;
                }
                h1 {
                    color: #00ff41;
                    font-size: 2rem;
                }
                p {
                    font-size: 1.1rem;
                    margin: 1rem 0;
                }
                a {
                    color: #00ff41;
                    text-decoration: none;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <h1>✓ Payment Successful</h1>
                <p>Your credits have been added to your account.</p>
                <p><a href="/dashboard">Go to Dashboard →</a></p>
            </div>
        </body>
        </html>
    "#))
}

pub async fn checkout_cancel() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().content_type("text/html").body(r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Payment Cancelled - OpenAgents</title>
            <style>
                body {
                    font-family: 'Vera Mono', monospace;
                    background: #0a0a0a;
                    color: #e0e0e0;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    height: 100vh;
                    margin: 0;
                }
                .container {
                    text-align: center;
                    max-width: 500px;
                }
                h1 {
                    color: #ff4444;
                    font-size: 2rem;
                }
                p {
                    font-size: 1.1rem;
                    margin: 1rem 0;
                }
                a {
                    color: #00ff41;
                    text-decoration: none;
                }
            </style>
        </head>
        <body>
            <div class="container">
                <h1>Payment Cancelled</h1>
                <p>No charges were made to your account.</p>
                <p><a href="/">Return Home →</a></p>
            </div>
        </body>
        </html>
    "#))
}

#[derive(Debug, Deserialize)]
pub struct StripeWebhookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
}

pub async fn stripe_webhook(
    payload: web::Bytes,
) -> Result<HttpResponse> {
    // In production:
    // 1. Verify webhook signature using Stripe-Signature header
    // 2. Parse event
    // 3. Handle different event types (checkout.session.completed, etc.)
    // 4. Update user credits in database
    // 5. Send confirmation email

    info!("Received Stripe webhook: {} bytes", payload.len());

    // Parse webhook payload
    let event: Result<StripeWebhookEvent, _> = serde_json::from_slice(&payload);

    match event {
        Ok(event) => {
            info!("Webhook event type: {}", event.event_type);

            match event.event_type.as_str() {
                "checkout.session.completed" => {
                    // Extract session details and credit user's account
                    info!("Checkout session completed");
                    // TODO: Update database with new credits
                }
                "payment_intent.succeeded" => {
                    info!("Payment succeeded");
                }
                "payment_intent.payment_failed" => {
                    error!("Payment failed");
                }
                _ => {
                    info!("Unhandled event type: {}", event.event_type);
                }
            }

            Ok(HttpResponse::Ok().json(serde_json::json!({"received": true})))
        }
        Err(e) => {
            error!("Failed to parse webhook payload: {}", e);
            Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid webhook payload"
            })))
        }
    }
}
