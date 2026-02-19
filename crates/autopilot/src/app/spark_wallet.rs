use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use openagents_spark::{
    Balance, Network, NetworkStatusReport, Payment, PaymentStatus, PaymentType, SparkSigner,
    SparkWallet, WalletConfig,
};
use tokio::sync::mpsc;

const DEFAULT_STORAGE_DIR: &str = "wallet";
const FALLBACK_STORAGE_DIR: &str = "spark";

#[derive(Clone, Debug)]
pub(crate) enum SparkWalletStatus {
    Idle,
    Refreshing,
    NotConfigured,
    Error(String),
}

impl SparkWalletStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            SparkWalletStatus::Idle => "Idle",
            SparkWalletStatus::Refreshing => "Refreshing",
            SparkWalletStatus::NotConfigured => "Not configured",
            SparkWalletStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            SparkWalletStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SparkPaymentDirection {
    Send,
    Receive,
}

impl SparkPaymentDirection {
    pub(crate) fn label(&self) -> &str {
        match self {
            SparkPaymentDirection::Send => "Send",
            SparkPaymentDirection::Receive => "Receive",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SparkPaymentState {
    Completed,
    Pending,
    Failed,
}

impl SparkPaymentState {
    pub(crate) fn label(&self) -> &str {
        match self {
            SparkPaymentState::Completed => "Completed",
            SparkPaymentState::Pending => "Pending",
            SparkPaymentState::Failed => "Failed",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SparkPaymentSummary {
    pub(crate) id: String,
    pub(crate) amount_sats: u64,
    pub(crate) direction: SparkPaymentDirection,
    pub(crate) status: SparkPaymentState,
    pub(crate) timestamp: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SparkWalletSnapshot {
    pub(crate) network: Network,
    pub(crate) api_key_present: bool,
    pub(crate) openagents_api_key_present: bool,
    pub(crate) storage_dir: PathBuf,
    pub(crate) balance: Balance,
    pub(crate) spark_address: String,
    pub(crate) bitcoin_address: String,
    pub(crate) network_status: NetworkStatusReport,
    pub(crate) payments: Vec<SparkPaymentSummary>,
}

#[derive(Debug, Clone)]
pub(crate) enum SparkWalletEvent {
    Snapshot(SparkWalletSnapshot),
    NotConfigured(String),
    Error(String),
    /// GET /agents/me/wallet result: true = linked, false = not linked
    OpenAgentsLinked(bool),
    /// POST /agents/me/wallet result
    AttachResult { ok: bool, error: Option<String> },
}

#[derive(Debug)]
pub(crate) enum SparkWalletCommand {
    Refresh,
    /// Attach local spark_address to OpenAgents account (requires openagents_api_key in config)
    AttachToOpenAgents {
        spark_address: String,
        lud16: Option<String>,
    },
}

pub(crate) struct SparkWalletRuntime {
    cmd_tx: mpsc::Sender<SparkWalletCommand>,
    pub(crate) event_rx: mpsc::Receiver<SparkWalletEvent>,
}

impl SparkWalletRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<SparkWalletCommand>(8);
        let (event_tx, event_rx) = mpsc::channel::<SparkWalletEvent>(16);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_spark_wallet_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn refresh(&self) {
        let _ = self.cmd_tx.try_send(SparkWalletCommand::Refresh);
    }

    pub(crate) fn attach_to_openagents(&self, spark_address: String, lud16: Option<String>) {
        let _ = self.cmd_tx.try_send(SparkWalletCommand::AttachToOpenAgents {
            spark_address,
            lud16,
        });
    }
}

impl Default for SparkWalletRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct SparkWalletState {
    pub(crate) runtime: SparkWalletRuntime,
    pub(crate) status: SparkWalletStatus,
    pub(crate) snapshot: Option<SparkWalletSnapshot>,
    pub(crate) status_message: Option<String>,
    pub(crate) last_refresh: Option<u64>,
    /// Whether wallet is linked to OpenAgents account (from GET /agents/me/wallet)
    pub(crate) openagents_linked: Option<bool>,
    /// Last attach-to-OpenAgents error message if any
    pub(crate) openagents_attach_error: Option<String>,
}

impl SparkWalletState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: SparkWalletRuntime::new(),
            status: SparkWalletStatus::Idle,
            snapshot: None,
            status_message: None,
            last_refresh: None,
            openagents_linked: None,
            openagents_attach_error: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = SparkWalletStatus::Refreshing;
        self.status_message = Some("Refreshing Spark wallet status...".to_string());
        self.runtime.refresh();
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: SparkWalletSnapshot) {
        self.snapshot = Some(snapshot);
        self.last_refresh = Some(now());
        self.status = SparkWalletStatus::Idle;
        self.status_message = None;
    }

    pub(crate) fn set_not_configured(&mut self, message: String) {
        self.snapshot = None;
        self.last_refresh = Some(now());
        self.status = SparkWalletStatus::NotConfigured;
        self.status_message = Some(message);
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.last_refresh = Some(now());
        self.status = SparkWalletStatus::Error(message.clone());
        self.status_message = Some(message);
    }

    pub(crate) fn set_openagents_linked(&mut self, linked: bool) {
        self.openagents_linked = Some(linked);
        self.openagents_attach_error = None;
    }

    pub(crate) fn set_attach_result(&mut self, ok: bool, error: Option<String>) {
        if ok {
            self.openagents_linked = Some(true);
            self.openagents_attach_error = None;
        } else {
            self.openagents_attach_error = error;
        }
    }

    /// Send attach command with current snapshot's spark_address (call when snapshot and openagents_api_key present).
    pub(crate) fn attach_to_openagents(&self) {
        if let Some(snap) = &self.snapshot {
            if snap.openagents_api_key_present && !snap.spark_address.is_empty() {
                self.runtime.attach_to_openagents(snap.spark_address.clone(), None);
            }
        }
    }
}

impl Default for SparkWalletState {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn run_spark_wallet_loop(
    mut cmd_rx: mpsc::Receiver<SparkWalletCommand>,
    event_tx: mpsc::Sender<SparkWalletEvent>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SparkWalletCommand::Refresh => match build_wallet_config() {
                Ok(config) => match load_mnemonic() {
                    Ok(mnemonic) => {
                        let signer = match SparkSigner::from_mnemonic(&mnemonic, "") {
                            Ok(signer) => signer,
                            Err(err) => {
                                let _ = event_tx
                                    .send(SparkWalletEvent::Error(format!(
                                        "Failed to load signer: {}",
                                        err
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        let wallet = match SparkWallet::new(signer, config.wallet_config).await {
                            Ok(wallet) => wallet,
                            Err(err) => {
                                let _ = event_tx
                                    .send(SparkWalletEvent::Error(format!(
                                        "Wallet init failed: {}",
                                        err
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        let balance = match wallet.get_balance().await {
                            Ok(balance) => balance,
                            Err(err) => {
                                let _ = event_tx
                                    .send(SparkWalletEvent::Error(format!(
                                        "Balance failed: {}",
                                        err
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        let spark_address = match wallet.get_spark_address().await {
                            Ok(address) => address,
                            Err(err) => {
                                let _ = event_tx
                                    .send(SparkWalletEvent::Error(format!(
                                        "Spark address failed: {}",
                                        err
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        let bitcoin_address = match wallet.get_bitcoin_address().await {
                            Ok(address) => address,
                            Err(err) => {
                                let _ = event_tx
                                    .send(SparkWalletEvent::Error(format!(
                                        "Bitcoin address failed: {}",
                                        err
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        let network_status = wallet.network_status(Duration::from_secs(5)).await;

                        let payments = match wallet.list_payments(Some(20), None).await {
                            Ok(payments) => payments,
                            Err(err) => {
                                let _ = event_tx
                                    .send(SparkWalletEvent::Error(format!(
                                        "Payment history failed: {}",
                                        err
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        let snapshot = SparkWalletSnapshot {
                            network: config.network,
                            api_key_present: config.api_key_present,
                            openagents_api_key_present: config.openagents_api_key.is_some(),
                            storage_dir: config.storage_dir,
                            balance,
                            spark_address: spark_address.clone(),
                            bitcoin_address,
                            network_status,
                            payments: summarize_payments(payments),
                        };
                        let _ = event_tx.send(SparkWalletEvent::Snapshot(snapshot)).await;
                        // If OpenAgents API key is set, check linked status
                        if let Some(ref key) = config.openagents_api_key {
                            if let Ok(linked) = fetch_openagents_wallet_linked(key).await {
                                let _ = event_tx.send(SparkWalletEvent::OpenAgentsLinked(linked)).await;
                            }
                        }
                    }
                    Err(message) => {
                        let _ = event_tx
                            .send(SparkWalletEvent::NotConfigured(message))
                            .await;
                    }
                },
                Err(message) => {
                    let _ = event_tx.send(SparkWalletEvent::Error(message)).await;
                }
            },
            SparkWalletCommand::AttachToOpenAgents {
                spark_address,
                lud16,
            } => {
                let pylon_dir = match pylon_dir() {
                    Some(d) => d,
                    None => {
                        let _ = event_tx
                            .send(SparkWalletEvent::AttachResult {
                                ok: false,
                                error: Some("No home directory".to_string()),
                            })
                            .await;
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        continue;
                    }
                };
                let (_, _, openagents_api_key, _) = load_pylon_config(&pylon_dir);
                let api_key = match openagents_api_key {
                    Some(k) if !k.is_empty() => k,
                    _ => {
                        let _ = event_tx
                            .send(SparkWalletEvent::AttachResult {
                                ok: false,
                                error: Some("Set openagents_api_key in pylon config (e.g. ~/.openagents/pylon/config.toml)".to_string()),
                            })
                            .await;
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        continue;
                    }
                };
                match post_openagents_wallet(&api_key, &spark_address, lud16.as_deref()).await {
                    Ok(()) => {
                        let _ = event_tx
                            .send(SparkWalletEvent::AttachResult {
                                ok: true,
                                error: None,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(SparkWalletEvent::AttachResult {
                                ok: false,
                                error: Some(e.to_string()),
                            })
                            .await;
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

const OPENAGENTS_API_BASE: &str = "https://openagents.com/api";

async fn fetch_openagents_wallet_linked(api_key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/agents/me/wallet", OPENAGENTS_API_BASE))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;
    Ok(resp.status().as_u16() == 200)
}

async fn post_openagents_wallet(
    api_key: &str,
    spark_address: &str,
    lud16: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let mut body = serde_json::json!({ "spark_address": spark_address });
    if let Some(l) = lud16 {
        body["lud16"] = serde_json::Value::String(l.to_string());
    }
    let resp = client
        .post(format!("{}/agents/me/wallet", OPENAGENTS_API_BASE))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(Box::<dyn std::error::Error + Send + Sync>::from(format!(
            "{}: {}",
            status, text
        )));
    }
    Ok(())
}

struct SparkWalletRuntimeConfig {
    network: Network,
    api_key_present: bool,
    openagents_api_key: Option<String>,
    storage_dir: PathBuf,
    wallet_config: WalletConfig,
}

fn build_wallet_config() -> Result<SparkWalletRuntimeConfig, String> {
    let pylon_dir = pylon_dir().ok_or_else(|| "No home directory found".to_string())?;
    let (network, api_key, openagents_api_key, data_dir) = load_pylon_config(&pylon_dir);
    let storage_dir = select_storage_dir(&data_dir.unwrap_or(pylon_dir))?;

    let wallet_config = WalletConfig {
        network,
        api_key: api_key.clone(),
        storage_dir: storage_dir.clone(),
    };

    Ok(SparkWalletRuntimeConfig {
        network,
        api_key_present: api_key.is_some(),
        openagents_api_key,
        storage_dir,
        wallet_config,
    })
}

fn load_pylon_config(pylon_dir: &PathBuf) -> (Network, Option<String>, Option<String>, Option<PathBuf>) {
    let config_path = pylon_dir.join("config.toml");
    let mut network = Network::Regtest;
    let mut api_key: Option<String> = None;
    let mut openagents_api_key: Option<String> = None;
    let mut data_dir: Option<PathBuf> = None;

    if let Ok(contents) = std::fs::read_to_string(&config_path) {
        if let Ok(value) = toml::from_str::<toml::Value>(&contents) {
            if let Some(net) = value.get("network").and_then(|v| v.as_str()) {
                network = parse_network(net);
            }
            if let Some(token) = value.get("spark_token").and_then(|v| v.as_str()) {
                let token = token.trim();
                if !token.is_empty() {
                    api_key = Some(token.to_string());
                }
            }
            if let Some(key) = value
                .get("openagents_api_key")
                .or_else(|| value.get("openagents-api-key"))
                .and_then(|v| v.as_str())
            {
                let key = key.trim();
                if !key.is_empty() {
                    openagents_api_key = Some(key.to_string());
                }
            }
            if let Some(dir) = value.get("data_dir").and_then(|v| v.as_str()) {
                if !dir.trim().is_empty() {
                    data_dir = Some(PathBuf::from(dir));
                }
            }
        }
    }

    (network, api_key, openagents_api_key, data_dir)
}

fn parse_network(network: &str) -> Network {
    match network.trim().to_ascii_lowercase().as_str() {
        "mainnet" => Network::Mainnet,
        "testnet" => Network::Testnet,
        "signet" => Network::Signet,
        _ => Network::Regtest,
    }
}

fn select_storage_dir(base_dir: &PathBuf) -> Result<PathBuf, String> {
    let primary = base_dir.join(DEFAULT_STORAGE_DIR);
    let fallback = base_dir.join(FALLBACK_STORAGE_DIR);
    let selected = if primary.exists() { primary } else { fallback };
    if let Some(parent) = selected.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("Failed to create data dir: {}", err))?;
        }
    }
    if !selected.exists() {
        std::fs::create_dir_all(&selected)
            .map_err(|err| format!("Failed to create wallet dir: {}", err))?;
    }
    Ok(selected)
}

fn load_mnemonic() -> Result<String, String> {
    let identity_file = pylon_dir()
        .map(|dir| dir.join("identity.mnemonic"))
        .ok_or_else(|| "No home directory found".to_string())?;

    if !identity_file.exists() {
        return Err("No identity found. Run `pylon init` first.".to_string());
    }

    std::fs::read_to_string(&identity_file)
        .map(|contents| contents.trim().to_string())
        .map_err(|err| format!("Failed to read identity: {}", err))
}

fn pylon_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".openagents").join("pylon"))
}

fn summarize_payments(payments: Vec<Payment>) -> Vec<SparkPaymentSummary> {
    payments
        .into_iter()
        .map(|payment| SparkPaymentSummary {
            id: payment.id,
            amount_sats: payment.amount as u64,
            direction: match payment.payment_type {
                PaymentType::Send => SparkPaymentDirection::Send,
                PaymentType::Receive => SparkPaymentDirection::Receive,
            },
            status: match payment.status {
                PaymentStatus::Completed => SparkPaymentState::Completed,
                PaymentStatus::Pending => SparkPaymentState::Pending,
                PaymentStatus::Failed => SparkPaymentState::Failed,
            },
            timestamp: payment.timestamp as u64,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spark_wallet_status_labels() {
        assert_eq!(SparkWalletStatus::Idle.label(), "Idle");
        assert_eq!(SparkWalletStatus::Refreshing.label(), "Refreshing");
        assert_eq!(SparkWalletStatus::NotConfigured.label(), "Not configured");
    }

    #[test]
    fn parse_network_defaults_to_regtest() {
        assert_eq!(parse_network("unknown"), Network::Regtest);
    }
}
