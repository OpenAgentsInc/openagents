use std::path::{Path, PathBuf};

use nostr::identity_mnemonic_path;
use openagents_spark::{
    Balance, Network, NetworkStatus, NetworkStatusReport, PaymentSummary, SparkSigner, SparkWallet,
    WalletConfig,
};
use tokio::runtime::Runtime;

pub const ENV_SPARK_NETWORK: &str = "OPENAGENTS_SPARK_NETWORK";
pub const ENV_SPARK_API_KEY: &str = "OPENAGENTS_SPARK_API_KEY";

pub struct SparkPaneState {
    wallet: Option<SparkWallet>,
    pub network: Network,
    pub identity_path: Option<PathBuf>,
    pub network_status: Option<NetworkStatusReport>,
    pub balance: Option<Balance>,
    pub spark_address: Option<String>,
    pub bitcoin_address: Option<String>,
    pub recent_payments: Vec<PaymentSummary>,
    pub last_invoice: Option<String>,
    pub last_payment_id: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

impl Default for SparkPaneState {
    fn default() -> Self {
        Self {
            wallet: None,
            network: configured_network(),
            identity_path: None,
            network_status: None,
            balance: None,
            spark_address: None,
            bitcoin_address: None,
            recent_payments: Vec::new(),
            last_invoice: None,
            last_payment_id: None,
            last_action: None,
            last_error: None,
        }
    }
}

impl SparkPaneState {
    pub fn network_name(&self) -> &'static str {
        match self.network {
            Network::Mainnet => "mainnet",
            Network::Testnet => "testnet",
            Network::Signet => "signet",
            Network::Regtest => "regtest",
        }
    }

    pub fn network_status_label(&self) -> &'static str {
        match self.network_status.as_ref().map(|status| status.status) {
            Some(NetworkStatus::Connected) => "connected",
            Some(NetworkStatus::Disconnected) => "disconnected",
            None => "unknown",
        }
    }

    pub fn refresh(&mut self, runtime: &Runtime) {
        self.last_error = None;

        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return;
        };

        let status = runtime.block_on(wallet.network_status());
        if let Some(detail) = status.detail.as_ref() {
            self.last_error = Some(format!("Spark connectivity: {detail}"));
        }
        self.network_status = Some(status);

        self.refresh_balance_and_payments(runtime);
        self.last_action = Some("Wallet refreshed".to_string());
    }

    pub fn request_spark_address(&mut self, runtime: &Runtime) {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return;
        };

        match runtime.block_on(wallet.get_spark_address()) {
            Ok(address) => {
                self.spark_address = Some(address);
                self.last_action = Some("Generated Spark receive address".to_string());
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to get Spark address: {error}"));
            }
        }
    }

    pub fn request_bitcoin_address(&mut self, runtime: &Runtime) {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return;
        };

        match runtime.block_on(wallet.get_bitcoin_address()) {
            Ok(address) => {
                self.bitcoin_address = Some(address);
                self.last_action = Some("Generated Bitcoin receive address".to_string());
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to get Bitcoin address: {error}"));
            }
        }
    }

    pub fn create_invoice(&mut self, runtime: &Runtime, amount_sats: u64) -> Option<String> {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return None;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return None;
        };

        let invoice = match runtime.block_on(wallet.create_invoice(
            amount_sats,
            Some("OpenAgents Spark receive".to_string()),
            Some(3600),
        )) {
            Ok(value) => value,
            Err(error) => {
                self.last_error = Some(format!("Failed to create invoice: {error}"));
                return None;
            }
        };

        self.last_invoice = Some(invoice.clone());
        self.last_action = Some(format!("Created invoice for {amount_sats} sats"));
        self.refresh_balance_and_payments(runtime);
        Some(invoice)
    }

    pub fn send_payment(
        &mut self,
        runtime: &Runtime,
        payment_request: &str,
        amount_sats: Option<u64>,
    ) -> Option<String> {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return None;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return None;
        };

        let request = payment_request.trim();
        if request.is_empty() {
            self.last_error = Some("Payment request cannot be empty".to_string());
            return None;
        }

        let payment_id = match runtime.block_on(wallet.send_payment_simple(request, amount_sats)) {
            Ok(id) => id,
            Err(error) => {
                self.last_error = Some(format!("Failed to send payment: {error}"));
                return None;
            }
        };

        self.last_payment_id = Some(payment_id.clone());
        self.last_action = Some(format!("Payment sent ({payment_id})"));
        self.refresh_balance_and_payments(runtime);
        Some(payment_id)
    }

    fn ensure_wallet(&mut self, runtime: &Runtime) -> Result<(), String> {
        if self.wallet.is_some() {
            return Ok(());
        }

        let identity_path = identity_mnemonic_path()
            .map_err(|error| format!("Failed to resolve identity path: {error}"))?;
        if !identity_path.exists() {
            return Err(format!(
                "No identity mnemonic found at {}. Open Nostr pane and regenerate keys first.",
                identity_path.display()
            ));
        }

        let mnemonic = read_mnemonic(identity_path.as_path())?;
        let signer = SparkSigner::from_mnemonic(&mnemonic, "")
            .map_err(|error| format!("Failed to derive Spark signer: {error}"))?;

        let storage_dir = identity_path
            .parent()
            .map(|parent| parent.join("spark"))
            .unwrap_or_else(|| PathBuf::from(".openagents/spark"));
        std::fs::create_dir_all(storage_dir.as_path()).map_err(|error| {
            format!(
                "Failed to create Spark storage directory {}: {error}",
                storage_dir.display()
            )
        })?;

        let api_key = std::env::var(ENV_SPARK_API_KEY)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let config = WalletConfig {
            network: self.network,
            api_key,
            storage_dir,
        };
        let wallet = runtime
            .block_on(SparkWallet::new(signer, config))
            .map_err(|error| format!("Failed to initialize Spark wallet: {error}"))?;

        self.identity_path = Some(identity_path);
        self.wallet = Some(wallet);

        Ok(())
    }

    fn refresh_balance_and_payments(&mut self, runtime: &Runtime) {
        let Some(wallet) = self.wallet.as_ref() else {
            return;
        };

        match runtime.block_on(wallet.get_balance()) {
            Ok(balance) => {
                self.balance = Some(balance);
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to fetch balance: {error}"));
            }
        }

        match runtime.block_on(wallet.list_payments(Some(25), None)) {
            Ok(payments) => {
                self.recent_payments = payments.into_iter().take(10).collect();
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to list payments: {error}"));
            }
        }
    }
}

fn configured_network() -> Network {
    let configured = std::env::var(ENV_SPARK_NETWORK)
        .unwrap_or_else(|_| "regtest".to_string())
        .to_ascii_lowercase();

    match configured.as_str() {
        "mainnet" => Network::Mainnet,
        "testnet" => Network::Testnet,
        "signet" => Network::Signet,
        _ => Network::Regtest,
    }
}

fn read_mnemonic(path: &Path) -> Result<String, String> {
    let mnemonic = std::fs::read_to_string(path)
        .map_err(|error| {
            format!(
                "Failed to read identity mnemonic {}: {error}",
                path.display()
            )
        })?
        .trim()
        .to_string();

    if mnemonic.is_empty() {
        return Err(format!("Identity mnemonic is empty: {}", path.display()));
    }

    Ok(mnemonic)
}
