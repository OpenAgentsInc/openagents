use std::time::Duration;

use autopilot_app::{WalletPaymentSummary, WalletStatus};
use openagents_spark::{
    Network as SparkNetwork, PaymentStatus as SparkPaymentStatus, PaymentType as SparkPaymentType,
    SparkSigner, SparkWallet, WalletConfig,
};
use pylon::PylonConfig;

fn spark_network_for_pylon(network: &str) -> SparkNetwork {
    match network.to_lowercase().as_str() {
        "mainnet" => SparkNetwork::Mainnet,
        "testnet" => SparkNetwork::Testnet,
        "signet" => SparkNetwork::Signet,
        _ => SparkNetwork::Regtest,
    }
}

struct LocalSparkWalletContext {
    wallet: SparkWallet,
    network: String,
}

async fn connect_local_spark_wallet() -> Result<LocalSparkWalletContext, String> {
    let config =
        PylonConfig::load().map_err(|err| format!("Failed to load Pylon config: {err}"))?;
    let data_dir = config
        .data_path()
        .map_err(|err| format!("Failed to resolve Pylon data dir: {err}"))?;
    let identity_path = data_dir.join("identity.mnemonic");
    if !identity_path.exists() {
        return Err(format!(
            "No identity found. Run 'pylon init' first. Expected: {}",
            identity_path.display()
        ));
    }

    let mnemonic = std::fs::read_to_string(&identity_path)
        .map_err(|err| format!("Failed to read identity: {err}"))?
        .trim()
        .to_string();
    let signer = SparkSigner::from_mnemonic(&mnemonic, "")
        .map_err(|err| format!("Failed to derive Spark signer: {err}"))?;
    let wallet_config = WalletConfig {
        network: spark_network_for_pylon(&config.network),
        api_key: None,
        storage_dir: data_dir.join("spark"),
    };
    let wallet = SparkWallet::new(signer, wallet_config)
        .await
        .map_err(|err| format!("Failed to init Spark wallet: {err}"))?;

    Ok(LocalSparkWalletContext {
        wallet,
        network: config.network,
    })
}

fn map_wallet_payment_status(status: SparkPaymentStatus) -> String {
    match status {
        SparkPaymentStatus::Completed => "completed".to_string(),
        SparkPaymentStatus::Pending => "pending".to_string(),
        SparkPaymentStatus::Failed => "failed".to_string(),
    }
}

fn map_wallet_payment_direction(direction: SparkPaymentType) -> String {
    match direction {
        SparkPaymentType::Send => "send".to_string(),
        SparkPaymentType::Receive => "receive".to_string(),
    }
}

pub(crate) async fn fetch_wallet_status(
    last_invoice: Option<String>,
    last_payment_id: Option<String>,
) -> WalletStatus {
    let mut status = WalletStatus {
        network: None,
        network_status: None,
        spark_sats: 0,
        lightning_sats: 0,
        onchain_sats: 0,
        total_sats: 0,
        spark_address: None,
        bitcoin_address: None,
        last_invoice,
        last_payment_id,
        recent_payments: Vec::new(),
        identity_exists: false,
        last_error: None,
    };

    let wallet_ctx = match connect_local_spark_wallet().await {
        Ok(wallet_ctx) => wallet_ctx,
        Err(err) => {
            status.last_error = Some(err);
            return status;
        }
    };
    status.network = Some(wallet_ctx.network.clone());
    status.identity_exists = true;

    let network_report = wallet_ctx
        .wallet
        .network_status(Duration::from_secs(5))
        .await;
    status.network_status = Some(network_report.status.as_str().to_ascii_lowercase());
    if let Some(detail) = network_report.detail {
        status.last_error = Some(format!("Wallet connectivity: {detail}"));
    }

    match wallet_ctx.wallet.get_balance().await {
        Ok(balance) => {
            status.spark_sats = balance.spark_sats;
            status.lightning_sats = balance.lightning_sats;
            status.onchain_sats = balance.onchain_sats;
            status.total_sats = balance.total_sats();
        }
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch balance: {err}"));
            return status;
        }
    }

    match wallet_ctx.wallet.get_spark_address().await {
        Ok(address) => status.spark_address = Some(address),
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch Spark address: {err}"));
        }
    }

    match wallet_ctx.wallet.get_bitcoin_address().await {
        Ok(address) => status.bitcoin_address = Some(address),
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch Bitcoin address: {err}"));
        }
    }

    match wallet_ctx.wallet.list_payments(Some(25), None).await {
        Ok(payments) => {
            status.recent_payments = payments
                .into_iter()
                .take(10)
                .map(|payment| WalletPaymentSummary {
                    id: payment.id,
                    direction: map_wallet_payment_direction(payment.payment_type),
                    status: map_wallet_payment_status(payment.status),
                    amount_sats: payment.amount as u64,
                    timestamp: payment.timestamp,
                })
                .collect();
        }
        Err(err) => {
            status.last_error = Some(format!("Failed to fetch wallet history: {err}"));
        }
    }

    status
}

pub(crate) async fn create_wallet_invoice(amount_sats: u64) -> Result<String, String> {
    if amount_sats == 0 {
        return Err("Amount must be > 0 sats.".to_string());
    }

    let wallet_ctx = connect_local_spark_wallet().await?;
    let response = wallet_ctx
        .wallet
        .create_invoice(
            amount_sats,
            Some("OpenAgents wallet receive".to_string()),
            Some(3600),
        )
        .await
        .map_err(|err| format!("Failed to create invoice: {err}"))?;

    Ok(response.payment_request)
}

pub(crate) async fn pay_wallet_request(
    payment_request: &str,
    amount_sats: Option<u64>,
) -> Result<String, String> {
    let request = payment_request.trim();
    if request.is_empty() {
        return Err("Payment request is required.".to_string());
    }

    let wallet_ctx = connect_local_spark_wallet().await?;
    let response = wallet_ctx
        .wallet
        .send_payment_simple(request, amount_sats)
        .await
        .map_err(|err| format!("Failed to send payment: {err}"))?;

    Ok(response.payment.id)
}
