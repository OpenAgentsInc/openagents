//! Stripe API client

use serde::Deserialize;
use wasm_bindgen::JsValue;
use worker::*;

#[derive(Debug, Deserialize)]
pub struct StripeCustomer {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct SetupIntent {
    pub id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone)]
pub struct PaymentMethodDetails {
    pub pm_type: String,
    pub brand: Option<String>,
    pub last4: Option<String>,
    pub exp_month: Option<i32>,
    pub exp_year: Option<i32>,
}

/// Create a Stripe customer
pub async fn create_customer(secret_key: &str, name: &str) -> Result<StripeCustomer> {
    let body = format!("name={}", urlencoding::encode(name));

    let response = stripe_request(
        secret_key,
        "POST",
        "https://api.stripe.com/v1/customers",
        Some(&body),
    )
    .await?;

    serde_json::from_str(&response)
        .map_err(|e| Error::RustError(format!("Failed to parse customer: {} - {}", e, response)))
}

/// Create a SetupIntent for collecting payment method
pub async fn create_setup_intent(secret_key: &str, customer_id: &str) -> Result<SetupIntent> {
    let body = format!(
        "customer={}&payment_method_types[]=card",
        urlencoding::encode(customer_id)
    );

    let response = stripe_request(
        secret_key,
        "POST",
        "https://api.stripe.com/v1/setup_intents",
        Some(&body),
    )
    .await?;

    serde_json::from_str(&response)
        .map_err(|e| Error::RustError(format!("Failed to parse setup intent: {} - {}", e, response)))
}

/// Get payment method details
pub async fn get_payment_method(secret_key: &str, pm_id: &str) -> Result<PaymentMethodDetails> {
    let url = format!("https://api.stripe.com/v1/payment_methods/{}", pm_id);
    let response = stripe_request(secret_key, "GET", &url, None).await?;

    let value: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| Error::RustError(format!("Failed to parse payment method: {} - {}", e, response)))?;

    let pm_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let card = value.get("card");

    Ok(PaymentMethodDetails {
        pm_type,
        brand: card.and_then(|c| c.get("brand")).and_then(|v| v.as_str()).map(String::from),
        last4: card.and_then(|c| c.get("last4")).and_then(|v| v.as_str()).map(String::from),
        exp_month: card.and_then(|c| c.get("exp_month")).and_then(|v| v.as_i64()).map(|v| v as i32),
        exp_year: card.and_then(|c| c.get("exp_year")).and_then(|v| v.as_i64()).map(|v| v as i32),
    })
}

/// Create a PaymentIntent for charging
pub async fn create_payment_intent(
    secret_key: &str,
    customer_id: &str,
    amount_cents: i64,
    metadata: &[(&str, &str)],
) -> Result<String> {
    let mut body = format!(
        "customer={}&amount={}&currency=usd&payment_method_types[]=card",
        urlencoding::encode(customer_id),
        amount_cents
    );

    for (key, value) in metadata {
        body.push_str(&format!(
            "&metadata[{}]={}",
            urlencoding::encode(key),
            urlencoding::encode(value)
        ));
    }

    let response = stripe_request(
        secret_key,
        "POST",
        "https://api.stripe.com/v1/payment_intents",
        Some(&body),
    )
    .await?;

    let value: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| Error::RustError(format!("Failed to parse payment intent: {} - {}", e, response)))?;

    value
        .get("client_secret")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| Error::RustError("Missing client_secret".to_string()))
}

/// Verify Stripe webhook signature
pub fn verify_webhook_signature(payload: &[u8], signature: &str, secret: &str) -> bool {
    // Parse signature header
    let mut timestamp: Option<&str> = None;
    let mut v1_sig: Option<&str> = None;

    for part in signature.split(',') {
        let mut kv = part.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("t"), Some(t)) => timestamp = Some(t),
            (Some("v1"), Some(s)) => v1_sig = Some(s),
            _ => {}
        }
    }

    let (timestamp, v1_sig) = match (timestamp, v1_sig) {
        (Some(t), Some(s)) => (t, s),
        _ => return false,
    };

    // Compute expected signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(signed_payload.as_bytes());
    let result = mac.finalize();
    let expected = hex::encode(result.into_bytes());

    // Constant-time comparison
    expected == v1_sig
}

/// Make a request to the Stripe API
async fn stripe_request(
    secret_key: &str,
    method: &str,
    url: &str,
    body: Option<&str>,
) -> Result<String> {
    let headers = Headers::new();
    headers.set(
        "Authorization",
        &format!("Bearer {}", secret_key),
    )?;
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;

    let mut init = RequestInit::new();
    init.with_method(match method {
        "POST" => Method::Post,
        "DELETE" => Method::Delete,
        _ => Method::Get,
    });
    init.with_headers(headers);

    if let Some(body) = body {
        init.with_body(Some(JsValue::from_str(body)));
    }

    let request = Request::new_with_init(url, &init)?;
    let mut response = Fetch::Request(request).send().await?;

    response.text().await
}

/// Stripe LLM proxy chat completion request
#[derive(Debug, serde::Serialize)]
pub struct LlmChatRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Stripe LLM proxy chat completion response
#[derive(Debug, Deserialize)]
pub struct LlmChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<LlmChoice>,
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, Deserialize)]
pub struct LlmChoice {
    pub index: u32,
    pub message: LlmMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Call Stripe's LLM proxy for chat completions
pub async fn llm_chat_completion(
    secret_key: &str,
    customer_id: &str,
    request: &LlmChatRequest,
) -> Result<LlmChatResponse> {
    let body = serde_json::to_string(request)
        .map_err(|e| Error::RustError(format!("Failed to serialize request: {}", e)))?;

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", secret_key))?;
    headers.set("Content-Type", "application/json")?;
    headers.set("X-Stripe-Customer-ID", customer_id)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body)));

    let request = Request::new_with_init("https://llm.stripe.com/chat/completions", &init)?;
    let mut response = Fetch::Request(request).send().await?;

    let status = response.status_code();
    let text = response.text().await?;

    if status != 200 {
        return Err(Error::RustError(format!(
            "LLM API error ({}): {}",
            status, text
        )));
    }

    serde_json::from_str(&text)
        .map_err(|e| Error::RustError(format!("Failed to parse LLM response: {} - {}", e, text)))
}

// We need hex encoding for the signature verification
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }
}
