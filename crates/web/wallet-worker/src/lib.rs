//! OpenAgents Wallet Worker - Spark Lightning payments on Cloudflare Workers
//!
//! This is a separate worker to keep the main worker under Cloudflare's 3MB WASM limit.
//! The spark/breez-sdk adds ~5-6MB to the binary.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spark::{
    Balance, Network, Payment, PaymentDetails, PaymentMethod, PaymentStatus, PaymentType,
    SparkSigner, SparkWallet, WalletConfig,
};
use std::time::{Duration, UNIX_EPOCH};
use worker::*;

mod identity;

// ============================================================================
// Session & Auth
// ============================================================================

const SESSION_COOKIE_NAME: &str = "oa_session";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    user_id: String,
    github_username: String,
    #[allow(dead_code)]
    github_oauth_state: Option<String>,
    #[allow(dead_code)]
    created_at: String,
    #[allow(dead_code)]
    last_active_at: String,
}

#[derive(Debug, Clone)]
struct AuthenticatedUser {
    user_id: String,
    #[allow(dead_code)]
    github_username: String,
    #[allow(dead_code)]
    session_token: String,
}

fn extract_session_token(cookie_header: &str) -> Option<String> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

async fn authenticate(req: &Request, env: &Env) -> Result<AuthenticatedUser> {
    let cookie_header = req
        .headers()
        .get("cookie")?
        .ok_or_else(|| Error::RustError("No cookies".to_string()))?;

    let token = extract_session_token(&cookie_header)
        .ok_or_else(|| Error::RustError("No session cookie".to_string()))?;

    let kv = env.kv("SESSIONS")?;
    let key = format!("session:{}", token);
    let json = kv
        .get(&key)
        .text()
        .await?
        .ok_or_else(|| Error::RustError("Invalid session".to_string()))?;

    let session: Session = serde_json::from_str(&json)
        .map_err(|e| Error::RustError(format!("JSON parse error: {}", e)))?;

    Ok(AuthenticatedUser {
        user_id: session.user_id,
        github_username: session.github_username,
        session_token: token,
    })
}

async fn with_auth<F, Fut>(req: &Request, env: &Env, handler: F) -> Result<Response>
where
    F: FnOnce(AuthenticatedUser) -> Fut,
    Fut: std::future::Future<Output = Result<Response>>,
{
    match authenticate(req, env).await {
        Ok(user) => handler(user).await,
        Err(e) => Response::error(format!("Unauthorized: {}", e), 401),
    }
}

// ============================================================================
// DB: Get identity material
// ============================================================================

#[derive(Debug, Deserialize)]
struct IdentityRow {
    nostr_private_key_encrypted: Option<String>,
    bitcoin_xpriv_encrypted: Option<String>,
}

async fn get_identity_material(
    db: &D1Database,
    user_id: &str,
    session_secret: &str,
) -> Result<identity::IdentityMaterial> {
    let row = db
        .prepare(
            "SELECT nostr_private_key_encrypted, bitcoin_xpriv_encrypted
             FROM users WHERE user_id = ? AND deleted_at IS NULL",
        )
        .bind(&[user_id.into()])?
        .first::<IdentityRow>(None)
        .await?
        .ok_or_else(|| Error::RustError("User not found".to_string()))?;

    let nostr_priv = row
        .nostr_private_key_encrypted
        .ok_or_else(|| Error::RustError("Missing identity keys".to_string()))?;
    let bitcoin_xpriv = row
        .bitcoin_xpriv_encrypted
        .ok_or_else(|| Error::RustError("Missing identity keys".to_string()))?;

    identity::decrypt_identity(session_secret, &nostr_priv, &bitcoin_xpriv)
}

// ============================================================================
// Wallet Response Types
// ============================================================================

#[derive(Serialize)]
struct WalletBalance {
    spark_sats: u64,
    lightning_sats: u64,
    onchain_sats: u64,
    total_sats: u64,
}

#[derive(Serialize)]
struct WalletAddresses {
    spark: Option<String>,
    onchain: Option<String>,
}

#[derive(Serialize)]
struct WalletPayment {
    id: String,
    amount_sats: u64,
    fee_sats: u64,
    direction: String,
    method: String,
    status: String,
    timestamp: String,
    description: Option<String>,
}

#[derive(Serialize)]
struct WalletSummary {
    status: String,
    network: Option<String>,
    balance: Option<WalletBalance>,
    addresses: Option<WalletAddresses>,
    payments: Vec<WalletPayment>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct WalletReceiveRequest {
    method: String,
    amount_sats: Option<u64>,
    description: Option<String>,
}

#[derive(Serialize)]
struct WalletReceiveResponse {
    method: String,
    payment_request: String,
    amount_sats: Option<u64>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct WalletSendRequest {
    payment_request: String,
    amount_sats: Option<u64>,
}

#[derive(Serialize)]
struct WalletSendResponse {
    payment_id: String,
    status: String,
    method: String,
    amount_sats: u64,
    fee_sats: u64,
}

#[derive(Serialize)]
struct WalletPaymentsResponse {
    payments: Vec<WalletPayment>,
}

// ============================================================================
// Wallet Helpers
// ============================================================================

fn wallet_config(env: &Env) -> Result<WalletConfig> {
    let network = match env.var("SPARK_NETWORK") {
        Ok(value) => parse_network(&value.to_string())?,
        Err(_) => Network::Testnet,
    };

    let api_key = env
        .var("BREEZ_API_KEY")
        .ok()
        .map(|value| value.to_string())
        .or_else(|| env.var("SPARK_API_KEY").ok().map(|value| value.to_string()));

    Ok(WalletConfig {
        network,
        api_key,
        ..Default::default()
    })
}

fn parse_network(value: &str) -> Result<Network> {
    match value.to_lowercase().as_str() {
        "mainnet" => Ok(Network::Mainnet),
        "testnet" => Ok(Network::Testnet),
        "signet" => Ok(Network::Signet),
        "regtest" => Ok(Network::Regtest),
        _ => Err(Error::RustError(format!(
            "Unknown SPARK_NETWORK '{}'",
            value
        ))),
    }
}

fn network_label(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
    }
}

async fn build_wallet(
    user: &AuthenticatedUser,
    env: &Env,
    config: WalletConfig,
) -> Result<SparkWallet> {
    let db = env.d1("DB")?;
    let session_secret = env.secret("SESSION_SECRET")?.to_string();
    let identity = get_identity_material(&db, &user.user_id, &session_secret).await?;

    let entropy = hex::decode(identity.bitcoin_xpriv.as_bytes())
        .map_err(|e| Error::RustError(format!("Invalid wallet seed: {e}")))?;

    if !(16..=64).contains(&entropy.len()) {
        return Err(Error::RustError(format!(
            "Invalid wallet seed length: {}",
            entropy.len()
        )));
    }

    let signer =
        SparkSigner::from_entropy(&entropy).map_err(|e| Error::RustError(e.to_string()))?;

    SparkWallet::new(signer, config)
        .await
        .map_err(|e| Error::RustError(e.to_string()))
}

fn map_balance(balance: Balance) -> WalletBalance {
    let total_sats = balance
        .spark_sats
        .saturating_add(balance.lightning_sats)
        .saturating_add(balance.onchain_sats);
    WalletBalance {
        spark_sats: balance.spark_sats,
        lightning_sats: balance.lightning_sats,
        onchain_sats: balance.onchain_sats,
        total_sats,
    }
}

fn map_payment(payment: Payment) -> WalletPayment {
    let amount_sats = u64::try_from(payment.amount).unwrap_or(u64::MAX);
    let fee_sats = u64::try_from(payment.fees).unwrap_or(0);
    let direction = match payment.payment_type {
        PaymentType::Send => "send",
        PaymentType::Receive => "receive",
    };
    let status = match payment.status {
        PaymentStatus::Completed => "completed",
        PaymentStatus::Pending => "pending",
        PaymentStatus::Failed => "failed",
    };
    let method = match payment.method {
        PaymentMethod::Lightning => "lightning",
        PaymentMethod::Spark => "spark",
        PaymentMethod::Token => "token",
        PaymentMethod::Deposit => "deposit",
        PaymentMethod::Withdraw => "withdraw",
        PaymentMethod::Unknown => "unknown",
    };

    let timestamp =
        DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(payment.timestamp)).to_rfc3339();

    let description = match &payment.details {
        Some(PaymentDetails::Lightning { description, .. }) => description.clone(),
        Some(PaymentDetails::Spark { invoice_details, .. }) => invoice_details
            .as_ref()
            .and_then(|details| details.description.clone()),
        Some(PaymentDetails::Token { metadata, .. }) => Some(metadata.name.clone()),
        _ => None,
    };

    WalletPayment {
        id: payment.id,
        amount_sats,
        fee_sats,
        direction: direction.to_string(),
        method: method.to_string(),
        status: status.to_string(),
        timestamp,
        description,
    }
}

// ============================================================================
// Route Handlers
// ============================================================================

async fn get_summary(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let config = match wallet_config(&env) {
        Ok(config) => config,
        Err(err) => {
            return Response::from_json(&WalletSummary {
                status: "error".to_string(),
                network: None,
                balance: None,
                addresses: None,
                payments: Vec::new(),
                error: Some(err.to_string()),
            });
        }
    };
    let network_label = Some(network_label(config.network).to_string());

    let wallet = match build_wallet(&user, &env, config).await {
        Ok(wallet) => wallet,
        Err(err) => {
            return Response::from_json(&WalletSummary {
                status: "error".to_string(),
                network: network_label,
                balance: None,
                addresses: None,
                payments: Vec::new(),
                error: Some(err.to_string()),
            });
        }
    };

    let mut errors = Vec::new();
    let balance = match wallet.get_balance().await {
        Ok(balance) => Some(balance),
        Err(err) => {
            errors.push(format!("balance: {}", err));
            None
        }
    };
    let spark_address = match wallet.get_spark_address().await {
        Ok(address) => Some(address),
        Err(err) => {
            errors.push(format!("spark address: {}", err));
            None
        }
    };
    let onchain_address = match wallet.get_bitcoin_address().await {
        Ok(address) => Some(address),
        Err(err) => {
            errors.push(format!("onchain address: {}", err));
            None
        }
    };
    let payments = match wallet.list_payments(Some(10), Some(0)).await {
        Ok(payments) => payments,
        Err(err) => {
            errors.push(format!("payments: {}", err));
            Vec::new()
        }
    }
    .into_iter()
    .map(map_payment)
    .collect::<Vec<_>>();

    let status = if balance.is_some() {
        if errors.is_empty() {
            "ready"
        } else {
            "partial"
        }
    } else {
        "error"
    };

    let summary = WalletSummary {
        status: status.to_string(),
        network: network_label,
        balance: balance.map(map_balance),
        addresses: Some(WalletAddresses {
            spark: spark_address,
            onchain: onchain_address,
        }),
        payments,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
    };

    Response::from_json(&summary)
}

async fn receive(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
    let request: WalletReceiveRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {e}")))?;
    let config = wallet_config(&env)?;
    let wallet = build_wallet(&user, &env, config).await?;

    let method = request.method.to_lowercase();
    let (payment_request, amount_sats) = match method.as_str() {
        "spark" => {
            if let Some(amount) = request.amount_sats {
                let response = wallet
                    .create_invoice(amount, request.description.clone(), None)
                    .await
                    .map_err(|e| Error::RustError(e.to_string()))?;
                (response.payment_request, Some(amount))
            } else {
                let address = wallet
                    .get_spark_address()
                    .await
                    .map_err(|e| Error::RustError(e.to_string()))?;
                (address, None)
            }
        }
        "lightning" => {
            let amount = request.amount_sats.ok_or_else(|| {
                Error::RustError("amount_sats is required for lightning invoices".to_string())
            })?;
            let response = wallet
                .create_lightning_invoice(amount, request.description.clone())
                .await
                .map_err(|e| Error::RustError(e.to_string()))?;
            (response.payment_request, Some(amount))
        }
        "onchain" | "bitcoin" => {
            let address = wallet
                .get_bitcoin_address()
                .await
                .map_err(|e| Error::RustError(e.to_string()))?;
            (address, None)
        }
        _ => {
            return Response::error("Unsupported receive method", 400);
        }
    };

    Response::from_json(&WalletReceiveResponse {
        method,
        payment_request,
        amount_sats,
        description: request.description,
    })
}

async fn send(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
    let request: WalletSendRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {e}")))?;
    let config = wallet_config(&env)?;
    let wallet = build_wallet(&user, &env, config).await?;

    let response = wallet
        .send_payment_simple(&request.payment_request, request.amount_sats)
        .await
        .map_err(|e| Error::RustError(e.to_string()))?;

    let payment = map_payment(response.payment);
    Response::from_json(&WalletSendResponse {
        payment_id: payment.id,
        status: payment.status,
        method: payment.method,
        amount_sats: payment.amount_sats,
        fee_sats: payment.fee_sats,
    })
}

async fn list_payments(user: AuthenticatedUser, env: Env) -> Result<Response> {
    let config = wallet_config(&env)?;
    let wallet = build_wallet(&user, &env, config).await?;

    let payments = wallet
        .list_payments(Some(25), Some(0))
        .await
        .unwrap_or_default()
        .into_iter()
        .map(map_payment)
        .collect::<Vec<_>>();

    Response::from_json(&WalletPaymentsResponse { payments })
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    // Add CORS headers for cross-origin requests from main worker
    let cors_headers = |mut resp: Response| -> Result<Response> {
        let headers = resp.headers_mut();
        headers.set("Access-Control-Allow-Origin", "*")?;
        headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
        headers.set("Access-Control-Allow-Headers", "Content-Type, Cookie")?;
        headers.set("Access-Control-Allow-Credentials", "true")?;
        Ok(resp)
    };

    // Handle CORS preflight
    if method == Method::Options {
        return cors_headers(Response::empty()?);
    }

    let result = match (method, path.as_ref()) {
        // Wallet summary (balance, addresses, recent payments)
        (Method::Get, "/api/wallet/summary") => {
            with_auth(&req, &env, |user| get_summary(user, env.clone())).await
        }

        // Receive: generate invoice or address
        (Method::Post, "/api/wallet/receive") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| receive(user, env.clone(), body.clone())).await
        }

        // Send: pay invoice or address
        (Method::Post, "/api/wallet/send") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| send(user, env.clone(), body.clone())).await
        }

        // List payments
        (Method::Get, "/api/wallet/payments") => {
            with_auth(&req, &env, |user| list_payments(user, env.clone())).await
        }

        // Health check
        (Method::Get, "/health") => Response::ok("wallet-worker ok"),

        _ => Response::error("Not Found", 404),
    };

    match result {
        Ok(resp) => cors_headers(resp),
        Err(e) => cors_headers(Response::error(e.to_string(), 500)?),
    }
}
