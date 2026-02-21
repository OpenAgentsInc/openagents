#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use reqwest::Client;

use lightning_wallet_executor::compat::WalletCompatService;
use lightning_wallet_executor::config::{ExecutorMode, WalletExecutorConfig};
use lightning_wallet_executor::gateway::{LivePaymentGateway, MockPaymentGateway};
use lightning_wallet_executor::http::make_wallet_executor_http_server;
use lightning_wallet_executor::secrets::provider_from_config;
use lightning_wallet_executor::service::WalletExecutorService;

#[derive(Parser)]
#[command(name = "lightning-wallet-executor")]
#[command(about = "Rust Lightning wallet executor")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve,
    Smoke,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve => run_serve().await,
        Command::Smoke => run_smoke().await,
    }
}

async fn run_serve() -> Result<()> {
    let config = WalletExecutorConfig::from_process_env()
        .map_err(|error| anyhow!("{}", error))
        .context("failed to load wallet executor config")?;

    let http = Client::new();
    let gateway: Arc<dyn lightning_wallet_executor::gateway::PaymentGateway> = match config.mode {
        ExecutorMode::Mock => Arc::new(MockPaymentGateway::new(None)),
        ExecutorMode::Spark => Arc::new(LivePaymentGateway::new(
            config.clone(),
            provider_from_config(&config, http.clone()),
        )),
    };

    let service = Arc::new(WalletExecutorService::new(config.clone(), gateway));
    let compat = Arc::new(WalletCompatService::new(config));

    let server = make_wallet_executor_http_server(service, compat)
        .await
        .map_err(anyhow::Error::msg)
        .context("failed to start wallet executor server")?;

    println!("[lightning-wallet-executor] listening {}", server.address);
    tokio::signal::ctrl_c()
        .await
        .context("failed while waiting for ctrl-c")?;
    server.close().await.map_err(anyhow::Error::msg)?;
    Ok(())
}

async fn run_smoke() -> Result<()> {
    let mut config = WalletExecutorConfig::default_mock();
    config.host = "127.0.0.1".to_string();
    config.port = 8798;
    config.wallet_id = "smoke-wallet".to_string();

    let service = Arc::new(WalletExecutorService::new(
        config.clone(),
        Arc::new(MockPaymentGateway::new(None)),
    ));
    let compat = Arc::new(WalletCompatService::new(config));

    let server = make_wallet_executor_http_server(service, compat)
        .await
        .map_err(anyhow::Error::msg)
        .context("failed to start smoke server")?;

    println!(
        "[lightning-wallet-executor:smoke] listening {}",
        server.address
    );

    let client = Client::new();
    let status = client
        .get(format!("{}/status", server.address))
        .send()
        .await
        .context("status request failed")?;

    if !status.status().is_success() {
        let body = status.text().await.unwrap_or_default();
        let _ = server.close().await;
        return Err(anyhow!("status endpoint returned non-200: {body}"));
    }

    let pay = client
        .post(format!("{}/pay-bolt11", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "requestId": "smoke-pay-1",
                "payment": {
                    "invoice": "lnbc1smoketestinvoice",
                    "maxAmountMsats": 100_000,
                    "host": "sats4ai.com"
                }
            })
            .to_string(),
        )
        .send()
        .await
        .context("pay request failed")?;

    if !pay.status().is_success() {
        let body = pay.text().await.unwrap_or_default();
        let _ = server.close().await;
        return Err(anyhow!("pay endpoint returned non-200: {body}"));
    }

    println!("[lightning-wallet-executor:smoke] status->pay succeeded");
    server.close().await.map_err(anyhow::Error::msg)?;
    Ok(())
}
