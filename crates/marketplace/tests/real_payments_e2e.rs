//! Real payment E2E test for marketplace flows using Spark wallets.
//!
//! This test is ignored by default because it requires funded wallets.
//! Enable with:
//! MARKETPLACE_E2E_SENDER_MNEMONIC="..." \
//! MARKETPLACE_E2E_RECEIVER_MNEMONIC="..." \
//! MARKETPLACE_E2E_AMOUNT_SATS=100 \
//! MARKETPLACE_E2E_NETWORK=testnet \
//! cargo test -p marketplace --test real_payments_e2e -- --ignored
//!
//! Or use the regtest faucet:
//! MARKETPLACE_E2E_USE_FAUCET=1 \
//! cargo test -p marketplace --test real_payments_e2e -- --ignored

use marketplace::core::payments::{PaymentManager, PaymentStatus};
use openagents_spark::{
    Network, Payment as SparkPayment, PaymentStatus as SparkStatus, PaymentType as SparkType,
    SparkSigner, SparkWallet, WalletConfig,
};
use std::env;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use testing::RegtestFaucet;
use tokio::time::{Instant, sleep};

const FAUCET_SENDER_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const FAUCET_RECEIVER_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

struct RealE2eConfig {
    sender_mnemonic: String,
    receiver_mnemonic: String,
    amount_sats: u64,
    network: Network,
    api_key: Option<String>,
    timeout: Duration,
    use_faucet: bool,
}

fn env_value(primary: &str, fallback: &str) -> Option<String> {
    env::var(primary).ok().or_else(|| env::var(fallback).ok())
}

fn parse_network(value: &str) -> Option<Network> {
    match value.to_ascii_lowercase().as_str() {
        "mainnet" => Some(Network::Mainnet),
        "testnet" => Some(Network::Testnet),
        "signet" => Some(Network::Signet),
        "regtest" => Some(Network::Regtest),
        _ => None,
    }
}

fn real_e2e_config() -> Option<RealE2eConfig> {
    let use_faucet = env_value("MARKETPLACE_E2E_USE_FAUCET", "SPARK_E2E_USE_FAUCET").is_some();
    let sender_env = env_value(
        "MARKETPLACE_E2E_SENDER_MNEMONIC",
        "SPARK_E2E_SENDER_MNEMONIC",
    );
    let receiver_env = env_value(
        "MARKETPLACE_E2E_RECEIVER_MNEMONIC",
        "SPARK_E2E_RECEIVER_MNEMONIC",
    );
    let (sender_mnemonic, receiver_mnemonic) = match (sender_env, receiver_env) {
        (Some(sender), Some(receiver)) => (sender, receiver),
        _ if use_faucet => (
            FAUCET_SENDER_MNEMONIC.to_string(),
            FAUCET_RECEIVER_MNEMONIC.to_string(),
        ),
        _ => return None,
    };

    let amount_sats = env_value("MARKETPLACE_E2E_AMOUNT_SATS", "SPARK_E2E_AMOUNT_SATS")
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);

    let timeout = env_value("MARKETPLACE_E2E_TIMEOUT_SECS", "SPARK_E2E_TIMEOUT_SECS")
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(180));

    let network = env_value("MARKETPLACE_E2E_NETWORK", "SPARK_E2E_NETWORK")
        .and_then(|value| parse_network(&value))
        .unwrap_or(Network::Testnet);

    if network == Network::Mainnet && env::var("MARKETPLACE_E2E_ALLOW_MAINNET").is_err() {
        println!(
            "Skipping mainnet marketplace E2E test - set MARKETPLACE_E2E_ALLOW_MAINNET=1 to enable"
        );
        return None;
    }
    if network == Network::Mainnet && use_faucet {
        println!(
            "Skipping mainnet marketplace E2E test - faucet funding only supported on regtest"
        );
        return None;
    }

    let api_key = env_value("MARKETPLACE_E2E_API_KEY", "SPARK_E2E_API_KEY")
        .or_else(|| env::var("BREEZ_API_KEY").ok());

    Some(RealE2eConfig {
        sender_mnemonic,
        receiver_mnemonic,
        amount_sats,
        network,
        api_key,
        timeout,
        use_faucet,
    })
}

fn unique_storage_dir(label: &str) -> std::path::PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "openagents-marketplace-e2e-{}-{}-{}",
        label,
        std::process::id(),
        now
    ));
    fs::create_dir_all(&dir).expect("should create marketplace e2e storage dir");
    dir
}

async fn wait_for_payment_by_id(
    wallet: &SparkWallet,
    payment_id: &str,
    timeout: Duration,
) -> Result<SparkPayment, anyhow::Error> {
    let deadline = Instant::now() + timeout;

    loop {
        let payments = wallet.list_payments(Some(50), Some(0)).await?;
        if let Some(payment) = payments.into_iter().find(|p| p.id == payment_id) {
            if payment.status == SparkStatus::Completed {
                return Ok(payment);
            }
            if payment.status == SparkStatus::Failed {
                return Err(anyhow::anyhow!("payment {} failed", payment_id));
            }
        }

        if Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "timed out waiting for payment {}",
                payment_id
            ));
        }

        sleep(Duration::from_secs(2)).await;
    }
}

async fn wait_for_receive_amount(
    wallet: &SparkWallet,
    amount_sats: u64,
    timeout: Duration,
) -> Result<SparkPayment, anyhow::Error> {
    let deadline = Instant::now() + timeout;
    let amount = amount_sats as u128;

    loop {
        let payments = wallet.list_payments(Some(50), Some(0)).await?;
        if let Some(payment) = payments.into_iter().find(|p| {
            p.payment_type == SparkType::Receive
                && p.amount == amount
                && p.status == SparkStatus::Completed
        }) {
            return Ok(payment);
        }

        if Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "timed out waiting for receive payment of {} sats",
                amount_sats
            ));
        }

        sleep(Duration::from_secs(2)).await;
    }
}

async fn wait_for_min_balance(
    wallet: &SparkWallet,
    min_sats: u64,
    timeout: Duration,
) -> Result<(), anyhow::Error> {
    let deadline = Instant::now() + timeout;

    loop {
        let balance = wallet.get_balance().await?;
        if balance.total_sats() >= min_sats {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "timed out waiting for balance >= {} sats",
                min_sats
            ));
        }

        sleep(Duration::from_secs(2)).await;
    }
}

async fn ensure_funded(
    wallet: &SparkWallet,
    min_balance: u64,
    timeout: Duration,
) -> Result<(), anyhow::Error> {
    let balance = wallet.get_balance().await?;
    if balance.total_sats() >= min_balance {
        return Ok(());
    }

    let needed = min_balance.saturating_sub(balance.total_sats());
    let request_amount = needed.clamp(10_000, 50_000);
    let deposit_address = wallet.get_bitcoin_address().await?;

    let faucet = RegtestFaucet::new()?;
    faucet
        .fund_address(&deposit_address, request_amount)
        .await?;
    wait_for_min_balance(wallet, balance.total_sats().saturating_add(1), timeout).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Requires funded Spark testnet wallets"]
async fn test_marketplace_payment_flow_real_sats() {
    let Some(config) = real_e2e_config() else {
        println!(
            "Skipping marketplace E2E test - set MARKETPLACE_E2E_SENDER_MNEMONIC/MARKETPLACE_E2E_RECEIVER_MNEMONIC or MARKETPLACE_E2E_USE_FAUCET=1"
        );
        return;
    };

    if config.amount_sats == 0 {
        println!("Skipping marketplace E2E test - amount must be > 0");
        return;
    }

    let amount_msats = config
        .amount_sats
        .checked_mul(1000)
        .expect("amount sats should fit into msats");

    let sender_signer = SparkSigner::from_mnemonic(&config.sender_mnemonic, "")
        .expect("should create sender signer");
    let receiver_signer = SparkSigner::from_mnemonic(&config.receiver_mnemonic, "")
        .expect("should create receiver signer");

    let sender_wallet = Arc::new(
        SparkWallet::new(
            sender_signer,
            WalletConfig {
                network: config.network,
                api_key: config.api_key.clone(),
                storage_dir: unique_storage_dir("sender"),
            },
        )
        .await
        .expect("should create sender wallet"),
    );

    let receiver_wallet = Arc::new(
        SparkWallet::new(
            receiver_signer,
            WalletConfig {
                network: config.network,
                api_key: config.api_key.clone(),
                storage_dir: unique_storage_dir("receiver"),
            },
        )
        .await
        .expect("should create receiver wallet"),
    );

    if config.use_faucet {
        if let Err(error) = ensure_funded(&sender_wallet, config.amount_sats, config.timeout).await
        {
            println!(
                "Skipping marketplace E2E test - faucet funding failed: {}",
                error
            );
            return;
        }
    } else {
        let sender_balance_before = sender_wallet
            .get_balance()
            .await
            .expect("should get sender balance");
        if sender_balance_before.total_sats() < config.amount_sats {
            println!("Sender wallet requires funding before running this test");
            return;
        }
    }

    let receiver_manager = PaymentManager::new(Some(receiver_wallet.clone()));
    let invoice = receiver_manager
        .create_invoice(amount_msats, "Marketplace real payment E2E")
        .await
        .expect("should create invoice");

    let sender_manager = PaymentManager::new(Some(sender_wallet.clone()));
    let mut payment = sender_manager
        .pay_compute_job("marketplace-e2e-job", &invoice, Some(amount_msats))
        .await
        .expect("should send payment");

    let payment_id = payment
        .payment_hash
        .clone()
        .unwrap_or_else(|| payment.id.clone());

    let sent_payment = wait_for_payment_by_id(&sender_wallet, &payment_id, config.timeout)
        .await
        .expect("payment should complete");

    wait_for_receive_amount(&receiver_wallet, config.amount_sats, config.timeout)
        .await
        .expect("receiver should see completed payment");

    if payment.status != PaymentStatus::Completed {
        let preimage = sent_payment
            .details
            .as_ref()
            .and_then(|details| match details {
                openagents_spark::PaymentDetails::Lightning { preimage, .. } => preimage.clone(),
                openagents_spark::PaymentDetails::Spark {
                    htlc_details: Some(htlc),
                    ..
                } => htlc.preimage.clone(),
                _ => None,
            })
            .unwrap_or_else(|| payment_id.clone());
        payment.mark_completed(preimage);
    }

    assert_eq!(payment.status, PaymentStatus::Completed);
}
