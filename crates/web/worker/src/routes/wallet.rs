//! Wallet routes backed by Spark.

use crate::db::users;
use crate::middleware::auth::AuthenticatedUser;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spark::{Balance, Network, Payment, PaymentDetails, PaymentMethod, PaymentStatus, PaymentType, SparkWallet, SparkSigner, WalletConfig};
use std::time::{Duration, UNIX_EPOCH};
use worker::{Env, Error, Response, Result};

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

pub async fn get_summary(user: AuthenticatedUser, env: Env) -> Result<Response> {
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

pub async fn receive(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
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

pub async fn send(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
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

pub async fn list_payments(user: AuthenticatedUser, env: Env) -> Result<Response> {
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

async fn build_wallet(
    user: &AuthenticatedUser,
    env: &Env,
    config: WalletConfig,
) -> Result<SparkWallet> {
    let db = env.d1("DB")?;
    let session_secret = env.secret("SESSION_SECRET")?.to_string();
    let identity = users::get_identity_material(&db, &user.user_id, &session_secret).await?;

    let entropy = hex::decode(identity.bitcoin_xpriv.as_bytes())
        .map_err(|e| Error::RustError(format!("Invalid wallet seed: {e}")))?;

    if !(16..=64).contains(&entropy.len()) {
        return Err(Error::RustError(format!(
            "Invalid wallet seed length: {}",
            entropy.len()
        )));
    }

    let signer = SparkSigner::from_entropy(&entropy)
        .map_err(|e| Error::RustError(e.to_string()))?;

    SparkWallet::new(signer, config)
        .await
        .map_err(|e| Error::RustError(e.to_string()))
}

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
