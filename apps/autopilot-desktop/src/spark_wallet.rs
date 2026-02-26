use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use nostr::identity_mnemonic_path;
use openagents_spark::{
    Balance, Network, NetworkStatus, NetworkStatusReport, PaymentSummary, SparkSigner, SparkWallet,
    WalletConfig,
};
use tokio::runtime::Runtime;

pub const ENV_SPARK_NETWORK: &str = "OPENAGENTS_SPARK_NETWORK";
pub const ENV_SPARK_API_KEY: &str = "OPENAGENTS_SPARK_API_KEY";
const SPARK_ACTION_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug)]
pub enum SparkWalletCommand {
    Refresh,
    GenerateSparkAddress,
    GenerateBitcoinAddress,
    CreateInvoice {
        amount_sats: u64,
    },
    SendPayment {
        payment_request: String,
        amount_sats: Option<u64>,
    },
    CancelPending,
}

pub struct SparkWalletWorker {
    command_tx: Sender<SparkWalletCommand>,
    result_rx: Receiver<SparkPaneState>,
}

impl SparkWalletWorker {
    pub fn spawn(initial_network: Network) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<SparkWalletCommand>();
        let (result_tx, result_rx) = mpsc::channel::<SparkPaneState>();

        std::thread::spawn(move || {
            let mut state = SparkPaneState::with_network(initial_network);
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    state.last_error = Some(format!(
                        "Failed to initialize Spark worker runtime: {error}"
                    ));
                    let _ = result_tx.send(state.clone());
                    return;
                }
            };

            while let Ok(command) = command_rx.recv() {
                if matches!(command, SparkWalletCommand::CancelPending) {
                    while let Ok(_pending) = command_rx.try_recv() {}
                    state.last_error = None;
                    state.last_action = Some("Cancelled pending Spark actions".to_string());
                    let _ = result_tx.send(state.clone());
                    continue;
                }

                state.apply_command(&runtime, command);
                let _ = result_tx.send(state.clone());
            }
        });

        Self {
            command_tx,
            result_rx,
        }
    }

    pub fn enqueue(&self, command: SparkWalletCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|error| format!("Spark worker offline: {error}"))
    }

    pub fn cancel_pending(&self) -> Result<(), String> {
        self.enqueue(SparkWalletCommand::CancelPending)
    }

    pub fn drain_updates(&mut self, target: &mut SparkPaneState) -> bool {
        let mut changed = false;
        loop {
            match self.result_rx.try_recv() {
                Ok(next) => {
                    *target = next;
                    changed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        changed
    }
}

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

impl Clone for SparkPaneState {
    fn clone(&self) -> Self {
        Self {
            wallet: None,
            network: self.network,
            identity_path: self.identity_path.clone(),
            network_status: self.network_status.clone(),
            balance: self.balance.clone(),
            spark_address: self.spark_address.clone(),
            bitcoin_address: self.bitcoin_address.clone(),
            recent_payments: self.recent_payments.clone(),
            last_invoice: self.last_invoice.clone(),
            last_payment_id: self.last_payment_id.clone(),
            last_action: self.last_action.clone(),
            last_error: self.last_error.clone(),
        }
    }
}

impl Default for SparkPaneState {
    fn default() -> Self {
        Self::with_network(configured_network())
    }
}

impl SparkPaneState {
    pub fn with_network(network: Network) -> Self {
        Self {
            wallet: None,
            network,
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

    fn apply_command(&mut self, runtime: &Runtime, command: SparkWalletCommand) {
        match command {
            SparkWalletCommand::Refresh => self.refresh(runtime),
            SparkWalletCommand::GenerateSparkAddress => self.request_spark_address(runtime),
            SparkWalletCommand::GenerateBitcoinAddress => self.request_bitcoin_address(runtime),
            SparkWalletCommand::CreateInvoice { amount_sats } => {
                let _ = self.create_invoice(runtime, amount_sats);
            }
            SparkWalletCommand::SendPayment {
                payment_request,
                amount_sats,
            } => {
                let _ = self.send_payment(runtime, &payment_request, amount_sats);
            }
            SparkWalletCommand::CancelPending => {
                self.last_error = None;
                self.last_action = Some("Cancelled pending Spark actions".to_string());
            }
        }
    }

    fn refresh(&mut self, runtime: &Runtime) {
        self.last_error = None;

        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return;
        };

        let status = match run_with_timeout_value(
            runtime,
            "Spark network status",
            SPARK_ACTION_TIMEOUT,
            wallet.network_status(),
        ) {
            Ok(status) => status,
            Err(error) => {
                self.last_error = Some(error);
                return;
            }
        };

        if let Some(detail) = status.detail.as_ref() {
            self.last_error = Some(format!("Spark connectivity: {detail}"));
        }
        self.network_status = Some(status);

        self.refresh_balance_and_payments(runtime);
        self.last_action = Some("Wallet refreshed".to_string());
    }

    fn request_spark_address(&mut self, runtime: &Runtime) {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return;
        };

        match run_with_timeout(
            runtime,
            "Generate Spark address",
            SPARK_ACTION_TIMEOUT,
            wallet.get_spark_address(),
        ) {
            Ok(address) => {
                self.spark_address = Some(address);
                self.last_action = Some("Generated Spark receive address".to_string());
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to get Spark address: {error}"));
            }
        }
    }

    fn request_bitcoin_address(&mut self, runtime: &Runtime) {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return;
        };

        match run_with_timeout(
            runtime,
            "Generate Bitcoin address",
            SPARK_ACTION_TIMEOUT,
            wallet.get_bitcoin_address(),
        ) {
            Ok(address) => {
                self.bitcoin_address = Some(address);
                self.last_action = Some("Generated Bitcoin receive address".to_string());
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to get Bitcoin address: {error}"));
            }
        }
    }

    fn create_invoice(&mut self, runtime: &Runtime, amount_sats: u64) -> Option<String> {
        self.last_error = None;
        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            return None;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            return None;
        };

        let invoice = match run_with_timeout(
            runtime,
            "Create Spark invoice",
            SPARK_ACTION_TIMEOUT,
            wallet.create_invoice(
                amount_sats,
                Some("OpenAgents Spark receive".to_string()),
                Some(3600),
            ),
        ) {
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

    fn send_payment(
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

        let payment_id = match run_with_timeout(
            runtime,
            "Send Spark payment",
            SPARK_ACTION_TIMEOUT,
            wallet.send_payment_simple(request, amount_sats),
        ) {
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
        let wallet = run_with_timeout(
            runtime,
            "Spark wallet initialization",
            SPARK_ACTION_TIMEOUT,
            SparkWallet::new(signer, config),
        )
        .map_err(|error| format!("Failed to initialize Spark wallet: {error}"))?;

        self.identity_path = Some(identity_path);
        self.wallet = Some(wallet);

        Ok(())
    }

    fn refresh_balance_and_payments(&mut self, runtime: &Runtime) {
        let Some(wallet) = self.wallet.as_ref() else {
            return;
        };

        match run_with_timeout(
            runtime,
            "Fetch Spark balance",
            SPARK_ACTION_TIMEOUT,
            wallet.get_balance(),
        ) {
            Ok(balance) => {
                self.balance = Some(balance);
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to fetch balance: {error}"));
            }
        }

        match run_with_timeout(
            runtime,
            "List Spark payments",
            SPARK_ACTION_TIMEOUT,
            wallet.list_payments(Some(25), None),
        ) {
            Ok(payments) => {
                self.recent_payments = payments.into_iter().take(10).collect();
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to list payments: {error}"));
            }
        }
    }
}

fn timeout_message(action: &str, timeout: Duration) -> String {
    format!("{action} timed out after {timeout:?}")
}

fn run_with_timeout<R, E, F>(
    runtime: &Runtime,
    action: &str,
    timeout: Duration,
    future: F,
) -> Result<R, String>
where
    E: std::fmt::Display,
    F: Future<Output = Result<R, E>>,
{
    match runtime.block_on(async { tokio::time::timeout(timeout, future).await }) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_elapsed) => Err(timeout_message(action, timeout)),
    }
}

fn run_with_timeout_value<R, F>(
    runtime: &Runtime,
    action: &str,
    timeout: Duration,
    future: F,
) -> Result<R, String>
where
    F: Future<Output = R>,
{
    match runtime.block_on(async { tokio::time::timeout(timeout, future).await }) {
        Ok(value) => Ok(value),
        Err(_elapsed) => Err(timeout_message(action, timeout)),
    }
}

pub fn configured_network() -> Network {
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

#[cfg(test)]
mod tests {
    use super::{
        Network, SPARK_ACTION_TIMEOUT, SparkPaneState, SparkWalletCommand, SparkWalletWorker,
        run_with_timeout, timeout_message,
    };

    use nostr::ENV_IDENTITY_MNEMONIC_PATH;
    use std::sync::Mutex;
    use std::time::Duration;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn timeout_message_contains_action_and_duration() {
        let message = timeout_message("refresh", Duration::from_millis(250));
        assert!(message.contains("refresh timed out"));
        assert!(message.contains("250ms"));
    }

    #[test]
    fn run_with_timeout_returns_timeout_error() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let result = run_with_timeout(&runtime, "slow action", Duration::from_millis(5), async {
            tokio::time::sleep(SPARK_ACTION_TIMEOUT).await;
            Ok::<u8, &'static str>(1)
        });

        let error = result.expect_err("should timeout");
        assert!(error.contains("slow action timed out"));
    }

    #[test]
    fn worker_cancel_pending_emits_update() {
        let mut worker = SparkWalletWorker::spawn(Network::Regtest);
        worker.cancel_pending().expect("cancel command");

        let mut snapshot = SparkPaneState::with_network(Network::Regtest);
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let mut received_update = false;

        while std::time::Instant::now() < deadline {
            if worker.drain_updates(&mut snapshot) {
                received_update = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(received_update, "expected cancellation update from worker");
        assert_eq!(
            snapshot.last_action.as_deref(),
            Some("Cancelled pending Spark actions")
        );
    }

    #[test]
    fn refresh_without_identity_sets_error_state() {
        with_missing_identity_env(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let mut state = SparkPaneState::with_network(Network::Regtest);
            state.last_error = Some("stale error".to_string());

            state.apply_command(&runtime, SparkWalletCommand::Refresh);

            let error = state.last_error.expect("refresh should report error");
            assert!(error.contains("No identity mnemonic found"));
            assert_eq!(state.last_action.as_deref(), None);
        });
    }

    #[test]
    fn create_invoice_without_identity_sets_error_state() {
        with_missing_identity_env(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let mut state = SparkPaneState::with_network(Network::Regtest);

            state.apply_command(
                &runtime,
                SparkWalletCommand::CreateInvoice { amount_sats: 1000 },
            );

            let error = state.last_error.expect("invoice should report error");
            assert!(error.contains("No identity mnemonic found"));
            assert!(state.last_invoice.is_none());
        });
    }

    #[test]
    fn send_payment_without_identity_sets_error_state() {
        with_missing_identity_env(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let mut state = SparkPaneState::with_network(Network::Regtest);

            state.apply_command(
                &runtime,
                SparkWalletCommand::SendPayment {
                    payment_request: "lnbc1example".to_string(),
                    amount_sats: Some(150),
                },
            );

            let error = state.last_error.expect("send should report error");
            assert!(error.contains("No identity mnemonic found"));
            assert!(state.last_payment_id.is_none());
        });
    }

    fn with_missing_identity_env(test: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().expect("env mutex poisoned");
        let key = ENV_IDENTITY_MNEMONIC_PATH;
        let previous = std::env::var(key).ok();

        let marker = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time available")
            .as_nanos();
        let missing_path = std::env::temp_dir().join(format!(
            "openagents-missing-identity-{}-{marker}.mnemonic",
            std::process::id()
        ));

        let _ = std::fs::remove_file(&missing_path);
        unsafe {
            std::env::set_var(key, &missing_path);
        }

        test();

        match previous {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
    }
}
