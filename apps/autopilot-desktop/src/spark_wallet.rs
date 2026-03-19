use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use lightning_invoice::Bolt11Invoice;
use nostr::identity_mnemonic_path;
use openagents_spark::{
    Balance, Network, NetworkStatus, NetworkStatusReport, PaymentSummary, SparkSigner, SparkWallet,
    WalletConfig,
};
use tokio::runtime::Runtime;

use crate::bitcoin_display::format_sats_amount;

pub const ENV_SPARK_NETWORK: &str = "OPENAGENTS_SPARK_NETWORK";
pub const ENV_SPARK_API_KEY: &str = "OPENAGENTS_SPARK_API_KEY";
const SPARK_ACTION_TIMEOUT: Duration = Duration::from_secs(15);
const SPARK_TRANSIENT_RETRY_ATTEMPTS: u8 = 3;
const SPARK_TRANSIENT_RETRY_DELAY: Duration = Duration::from_millis(350);
const STARTUP_CONVERGENCE_REFRESH_INTERVAL_SECONDS: u64 = 2;
const STARTUP_CONVERGENCE_REFRESH_ATTEMPTS: u8 = 3;
const REFRESH_THROTTLE_INTERVAL: Duration = Duration::from_secs(3);
// MVP release fallback so Spark boots on first run without requiring shell env injection.
const DEFAULT_OPENAGENTS_SPARK_API_KEY: &str = "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0=";

pub(crate) fn normalize_lightning_invoice_ref(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let normalized = trimmed
        .strip_prefix("lightning://")
        .or_else(|| trimmed.strip_prefix("LIGHTNING://"))
        .or_else(|| trimmed.strip_prefix("lightning:"))
        .or_else(|| trimmed.strip_prefix("LIGHTNING:"))
        .unwrap_or(trimmed)
        .trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_ascii_lowercase())
    }
}

pub(crate) fn decode_lightning_invoice_payment_hash(bolt11: &str) -> Option<String> {
    let normalized = normalize_lightning_invoice_ref(bolt11)?;
    let invoice = Bolt11Invoice::from_str(normalized.as_str()).ok()?;
    Some(invoice.payment_hash().to_string())
}

#[derive(Clone, Debug)]
pub enum SparkWalletCommand {
    Refresh,
    Reload,
    ConfigureEnv {
        vars: Vec<(String, String)>,
    },
    GenerateSparkAddress,
    GenerateBitcoinAddress,
    CreateInvoice {
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u64>,
    },
    CreateBolt11Invoice {
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
    },
    SendPayment {
        payment_request: String,
        amount_sats: Option<u64>,
    },
    CancelPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SparkInvoiceState {
    Empty,
    Ready,
    Expired,
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
            let mut backlog = VecDeque::<SparkWalletCommand>::new();
            if let Err(error) = ensure_rustls_crypto_provider() {
                state.last_error = Some(error);
                let _ = result_tx.send(state.clone());
                return;
            }
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

            loop {
                let Some(command) = next_wallet_worker_command(&command_rx, &mut backlog) else {
                    break;
                };
                if matches!(command, SparkWalletCommand::CancelPending) {
                    backlog.clear();
                    while let Ok(_pending) = command_rx.try_recv() {}
                    state.last_error = None;
                    state.last_action = Some("Cancelled pending Spark actions".to_string());
                    let _ = result_tx.send(state.clone());
                    continue;
                }

                let command =
                    coalesce_refresh_like_command_burst(command, &command_rx, &mut backlog);
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
        while let Ok(next) = self.result_rx.try_recv() {
            *target = next;
            changed = true;
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
    pub pending_balance_confirmation_payment_id: Option<String>,
    pub spark_address: Option<String>,
    pub bitcoin_address: Option<String>,
    pub recent_payments: Vec<PaymentSummary>,
    pub last_invoice: Option<String>,
    pub last_invoice_created_at_epoch_seconds: Option<u64>,
    pub last_invoice_expiry_seconds: Option<u64>,
    pub last_payment_id: Option<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    startup_convergence_active: bool,
    startup_convergence_refreshes_remaining: u8,
    startup_convergence_next_refresh_epoch_seconds: Option<u64>,
    env_overrides: HashMap<String, String>,
    last_refresh_started_at: Option<std::time::Instant>,
}

impl Clone for SparkPaneState {
    fn clone(&self) -> Self {
        Self {
            wallet: None,
            network: self.network,
            identity_path: self.identity_path.clone(),
            network_status: self.network_status.clone(),
            balance: self.balance.clone(),
            pending_balance_confirmation_payment_id: self
                .pending_balance_confirmation_payment_id
                .clone(),
            spark_address: self.spark_address.clone(),
            bitcoin_address: self.bitcoin_address.clone(),
            recent_payments: self.recent_payments.clone(),
            last_invoice: self.last_invoice.clone(),
            last_invoice_created_at_epoch_seconds: self.last_invoice_created_at_epoch_seconds,
            last_invoice_expiry_seconds: self.last_invoice_expiry_seconds,
            last_payment_id: self.last_payment_id.clone(),
            last_action: self.last_action.clone(),
            last_error: self.last_error.clone(),
            startup_convergence_active: self.startup_convergence_active,
            startup_convergence_refreshes_remaining: self.startup_convergence_refreshes_remaining,
            startup_convergence_next_refresh_epoch_seconds: self
                .startup_convergence_next_refresh_epoch_seconds,
            env_overrides: self.env_overrides.clone(),
            last_refresh_started_at: self.last_refresh_started_at,
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
            pending_balance_confirmation_payment_id: None,
            spark_address: None,
            bitcoin_address: None,
            recent_payments: Vec::new(),
            last_invoice: None,
            last_invoice_created_at_epoch_seconds: None,
            last_invoice_expiry_seconds: None,
            last_payment_id: None,
            last_action: None,
            last_error: None,
            startup_convergence_active: false,
            startup_convergence_refreshes_remaining: 0,
            startup_convergence_next_refresh_epoch_seconds: None,
            env_overrides: HashMap::new(),
            last_refresh_started_at: None,
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

    pub fn total_balance_sats(&self) -> Option<u64> {
        self.balance.as_ref().map(Balance::total_sats)
    }

    pub fn balance_known(&self) -> bool {
        self.balance.is_some()
    }

    pub fn balance_reconciling(&self) -> bool {
        self.startup_convergence_active || self.pending_balance_confirmation_payment_id.is_some()
    }

    pub fn network_status_label(&self) -> &'static str {
        if self.startup_convergence_active {
            return "reconciling";
        }
        match self.network_status.as_ref().map(|status| status.status) {
            Some(NetworkStatus::Connected) => "connected",
            Some(NetworkStatus::Disconnected) => "disconnected",
            None => "unknown",
        }
    }

    pub fn begin_startup_convergence(&mut self, now_epoch_seconds: u64) {
        if self.startup_convergence_active {
            return;
        }
        self.startup_convergence_active = true;
        self.startup_convergence_refreshes_remaining = STARTUP_CONVERGENCE_REFRESH_ATTEMPTS;
        self.startup_convergence_next_refresh_epoch_seconds =
            Some(now_epoch_seconds.saturating_add(STARTUP_CONVERGENCE_REFRESH_INTERVAL_SECONDS));
        self.last_action = Some("Wallet reconciling after startup sync".to_string());
    }

    pub fn startup_convergence_refresh_due(&self, now_epoch_seconds: u64) -> bool {
        self.startup_convergence_active
            && self
                .startup_convergence_next_refresh_epoch_seconds
                .is_some_and(|due| now_epoch_seconds >= due)
    }

    pub fn note_startup_convergence_refresh_queued(&mut self, now_epoch_seconds: u64) {
        if !self.startup_convergence_active {
            return;
        }
        if self.startup_convergence_refreshes_remaining > 0 {
            self.startup_convergence_refreshes_remaining -= 1;
        }
        self.startup_convergence_next_refresh_epoch_seconds =
            if self.startup_convergence_refreshes_remaining == 0 {
                None
            } else {
                Some(now_epoch_seconds.saturating_add(STARTUP_CONVERGENCE_REFRESH_INTERVAL_SECONDS))
            };
        self.last_action = Some("Wallet reconciling after startup sync".to_string());
    }

    pub fn cancel_startup_convergence(&mut self) {
        self.startup_convergence_active = false;
        self.startup_convergence_refreshes_remaining = 0;
        self.startup_convergence_next_refresh_epoch_seconds = None;
    }

    pub fn last_invoice_state(&self, now_epoch_seconds: u64) -> SparkInvoiceState {
        let has_invoice = self
            .last_invoice
            .as_deref()
            .map(str::trim)
            .is_some_and(|invoice| !invoice.is_empty());
        if !has_invoice {
            return SparkInvoiceState::Empty;
        }

        match (
            self.last_invoice_created_at_epoch_seconds,
            self.last_invoice_expiry_seconds,
        ) {
            (Some(created_at), Some(expiry_seconds))
                if expiry_seconds > 0
                    && now_epoch_seconds >= created_at.saturating_add(expiry_seconds) =>
            {
                SparkInvoiceState::Expired
            }
            _ => SparkInvoiceState::Ready,
        }
    }

    fn apply_command(&mut self, runtime: &Runtime, command: SparkWalletCommand) {
        match command {
            SparkWalletCommand::Refresh => self.refresh(runtime),
            SparkWalletCommand::Reload => self.reload(runtime),
            SparkWalletCommand::ConfigureEnv { vars } => self.configure_env(vars),
            SparkWalletCommand::GenerateSparkAddress => self.request_spark_address(runtime),
            SparkWalletCommand::GenerateBitcoinAddress => self.request_bitcoin_address(runtime),
            SparkWalletCommand::CreateInvoice {
                amount_sats,
                description,
                expiry_seconds,
            } => {
                let _ = self.create_invoice(runtime, amount_sats, description, expiry_seconds);
            }
            SparkWalletCommand::CreateBolt11Invoice {
                amount_sats,
                description,
                expiry_seconds,
            } => {
                let _ =
                    self.create_bolt11_invoice(runtime, amount_sats, description, expiry_seconds);
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

    fn configure_env(&mut self, vars: Vec<(String, String)>) {
        let mut next = HashMap::<String, String>::new();
        for (name, value) in vars {
            let normalized_name = name.trim().to_ascii_uppercase();
            if normalized_name.is_empty() {
                continue;
            }
            let normalized_value = value.trim().to_string();
            if normalized_value.is_empty() {
                continue;
            }
            next.insert(normalized_name, normalized_value);
        }

        if self.env_overrides == next {
            return;
        }

        self.env_overrides = next;
        self.wallet = None;
        self.last_error = None;
        self.last_action = Some("Updated Spark credential env overrides".to_string());
    }

    fn refresh(&mut self, runtime: &Runtime) {
        if self.should_throttle_refresh() {
            return;
        }
        self.last_refresh_started_at = Some(std::time::Instant::now());
        self.last_error = None;

        if let Err(error) = self.ensure_wallet(runtime) {
            self.last_error = Some(error);
            self.complete_startup_convergence_after_refresh();
            return;
        }

        let Some(wallet) = self.wallet.as_ref() else {
            self.last_error = Some("Spark wallet missing after initialization".to_string());
            self.complete_startup_convergence_after_refresh();
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
                self.complete_startup_convergence_after_refresh();
                return;
            }
        };

        if let Some(detail) = status.detail.as_ref() {
            self.last_error = Some(format!("Spark connectivity: {detail}"));
        }
        self.network_status = Some(status);

        self.refresh_balance_and_payments(runtime);
        self.complete_startup_convergence_after_refresh();
        self.last_action = Some("Wallet refreshed".to_string());
    }

    fn reload(&mut self, runtime: &Runtime) {
        self.last_refresh_started_at = None;
        self.wallet = None;
        self.network_status = None;
        self.refresh(runtime);
    }

    fn complete_startup_convergence_after_refresh(&mut self) {
        if self.startup_convergence_active
            && (self.startup_convergence_satisfied()
                || self
                    .startup_convergence_next_refresh_epoch_seconds
                    .is_none())
        {
            self.cancel_startup_convergence();
        }
    }

    fn startup_convergence_satisfied(&self) -> bool {
        self.last_error.is_none() && self.balance.is_some()
    }

    fn should_throttle_refresh(&self) -> bool {
        let Some(last_refresh_started_at) = self.last_refresh_started_at else {
            return false;
        };
        if self.pending_balance_confirmation_payment_id.is_some() || self.last_error.is_some() {
            return false;
        }
        if self.startup_convergence_active && !self.startup_convergence_satisfied() {
            return false;
        }
        last_refresh_started_at.elapsed() < REFRESH_THROTTLE_INTERVAL
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

    fn create_invoice(
        &mut self,
        runtime: &Runtime,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u64>,
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

        let invoice = match run_with_timeout(
            runtime,
            "Create Spark invoice",
            SPARK_ACTION_TIMEOUT,
            wallet.create_invoice(amount_sats, description, expiry_seconds),
        ) {
            Ok(value) => value,
            Err(error) => {
                self.last_error = Some(format!("Failed to create invoice: {error}"));
                return None;
            }
        };

        self.last_invoice = Some(invoice.clone());
        self.last_invoice_created_at_epoch_seconds = Some(current_epoch_seconds());
        self.last_invoice_expiry_seconds = expiry_seconds;
        self.last_action = Some(format!(
            "Created invoice for {}",
            format_sats_amount(amount_sats)
        ));
        self.refresh_balance_and_payments(runtime);
        Some(invoice)
    }

    fn create_bolt11_invoice(
        &mut self,
        runtime: &Runtime,
        amount_sats: u64,
        description: Option<String>,
        expiry_seconds: Option<u32>,
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

        let (invoice, attempts) = match run_with_transient_retry(
            runtime,
            "Create Lightning bolt11 invoice",
            SPARK_ACTION_TIMEOUT,
            || wallet.create_bolt11_invoice(amount_sats, description.clone(), expiry_seconds),
        ) {
            Ok(value) => value,
            Err(error) => {
                self.last_error = Some(format!("Failed to create Lightning invoice: {error}"));
                return None;
            }
        };

        self.last_invoice = Some(invoice.clone());
        self.last_invoice_created_at_epoch_seconds = Some(current_epoch_seconds());
        self.last_invoice_expiry_seconds = expiry_seconds.map(u64::from);
        self.last_action = Some(if attempts > 1 {
            format!(
                "Created Lightning invoice for {} after {} attempts",
                format_sats_amount(amount_sats),
                attempts
            )
        } else {
            format!(
                "Created Lightning invoice for {}",
                format_sats_amount(amount_sats)
            )
        });
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
                self.last_action = Some("Payment send failed".to_string());
                self.last_error = Some(format!("Failed to send payment: {error}"));
                return None;
            }
        };

        self.last_payment_id = Some(payment_id.clone());
        self.pending_balance_confirmation_payment_id = Some(payment_id.clone());
        self.last_action = Some(format!(
            "Payment sent ({payment_id}); awaiting Spark confirmation for balance refresh"
        ));
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

        let config = WalletConfig {
            network: self.network,
            api_key: Some(configured_api_key(&self.env_overrides)),
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

        let fetched_balance = match run_with_timeout(
            runtime,
            "Fetch Spark balance",
            SPARK_ACTION_TIMEOUT,
            wallet.get_balance(),
        ) {
            Ok(balance) => Some(balance),
            Err(error) => {
                self.last_error = Some(format!("Failed to fetch balance: {error}"));
                None
            }
        };

        match run_with_timeout(
            runtime,
            "List Spark payments",
            SPARK_ACTION_TIMEOUT,
            wallet.list_all_payments(),
        ) {
            Ok(payments) => {
                self.apply_balance_refresh_with_payment_confirmation(
                    fetched_balance,
                    payments.as_slice(),
                );
                self.recent_payments = payments;
            }
            Err(error) => {
                self.last_error = Some(format!("Failed to list payments: {error}"));
                if let Some(balance) = fetched_balance {
                    self.balance = Some(balance);
                }
            }
        }
    }

    fn apply_balance_refresh_with_payment_confirmation(
        &mut self,
        fetched_balance: Option<Balance>,
        payments: &[PaymentSummary],
    ) {
        let Some(balance) = fetched_balance else {
            return;
        };
        let previous_balance = self.balance.clone();

        let Some(payment_id) = self.pending_balance_confirmation_payment_id.clone() else {
            self.balance = Some(balance);
            return;
        };

        let Some(payment) = payments.iter().find(|payment| payment.id == payment_id) else {
            if previous_balance.is_none() {
                self.balance = Some(balance);
            }
            self.last_action = Some(format!(
                "Payment sent ({payment_id}); awaiting Spark confirmation for balance refresh"
            ));
            return;
        };

        if !is_terminal_wallet_payment_status(payment.status.as_str()) {
            if previous_balance.is_none() {
                self.balance = Some(balance);
            }
            let status_detail = payment
                .status_detail
                .as_deref()
                .unwrap_or(payment.status.as_str());
            self.last_action = Some(format!("Payment pending ({payment_id}); {status_detail}"));
            return;
        }

        self.balance = Some(balance);
        self.pending_balance_confirmation_payment_id = None;
        if is_settled_wallet_payment_status(payment.status.as_str()) {
            self.last_action = Some(format!(
                "Payment settled ({payment_id}); wallet confirmed; {}",
                wallet_payment_amount_summary(payment)
            ));
        } else {
            let status_detail = payment
                .status_detail
                .as_deref()
                .unwrap_or(payment.status.as_str());
            self.last_action = Some(format!(
                "Payment failed ({payment_id}); {status_detail}; {}",
                wallet_payment_amount_summary(payment)
            ));
        }
    }
}

pub(crate) fn is_settled_wallet_payment_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "success" | "settled" | "completed" | "confirmed"
    )
}

pub(crate) fn is_terminal_wallet_payment_status(status: &str) -> bool {
    is_settled_wallet_payment_status(status)
        || matches!(
            status.to_ascii_lowercase().as_str(),
            "failed" | "error" | "expired" | "cancelled" | "canceled" | "rejected"
        )
}

pub(crate) fn wallet_payment_total_debit_sats(payment: &PaymentSummary) -> u64 {
    if payment.direction.eq_ignore_ascii_case("send") {
        payment.amount_sats.saturating_add(payment.fees_sats)
    } else {
        payment.amount_sats
    }
}

pub(crate) fn wallet_payment_net_delta_sats(payment: &PaymentSummary) -> i64 {
    if payment.direction.eq_ignore_ascii_case("send") {
        i64::try_from(wallet_payment_total_debit_sats(payment))
            .map(|value| -value)
            .unwrap_or(i64::MIN)
    } else {
        i64::try_from(payment.amount_sats).unwrap_or(i64::MAX)
    }
}

pub(crate) fn include_wallet_pending_in_balance(
    payment: &PaymentSummary,
    now_epoch_seconds: u64,
) -> bool {
    if is_terminal_wallet_payment_status(payment.status.as_str()) {
        return false;
    }
    let recent_enough = payment.timestamp >= now_epoch_seconds.saturating_sub(86_400);
    let not_expired = payment
        .htlc_expiry_epoch_seconds
        .is_none_or(|expiry| expiry >= now_epoch_seconds.saturating_sub(60));
    recent_enough && not_expired
}

pub(crate) fn pending_wallet_delta_sats(
    payments: &[PaymentSummary],
    now_epoch_seconds: u64,
) -> i64 {
    payments
        .iter()
        .filter(|payment| include_wallet_pending_in_balance(payment, now_epoch_seconds))
        .fold(0_i64, |acc, payment| {
            acc.saturating_add(wallet_payment_net_delta_sats(payment))
        })
}

pub(crate) fn format_wallet_delta_sats(delta_sats: i64) -> String {
    if delta_sats > 0 {
        format!("+{delta_sats} sats")
    } else {
        format!("{delta_sats} sats")
    }
}

pub(crate) fn wallet_payment_amount_summary(payment: &PaymentSummary) -> String {
    if payment.direction.eq_ignore_ascii_case("send") {
        format!(
            "{} sats + {} sats fee ({} sats total debit)",
            payment.amount_sats,
            payment.fees_sats,
            wallet_payment_total_debit_sats(payment)
        )
    } else if payment.fees_sats > 0 {
        format!(
            "{} sats ({} sats fee)",
            payment.amount_sats, payment.fees_sats
        )
    } else {
        format!("{} sats", payment.amount_sats)
    }
}

fn timeout_message(action: &str, timeout: Duration) -> String {
    format!("{action} timed out after {timeout:?}")
}

fn is_transient_wallet_network_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    [
        "unexpected eof",
        "sendrequest",
        "error sending request",
        "tls_retry_write_records",
        "connection reset",
        "connection closed",
        "timed out",
        "temporary failure",
        "temporarily unavailable",
        "transport error",
        "network error",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
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

fn run_with_transient_retry<R, E, F, Factory>(
    runtime: &Runtime,
    action: &str,
    timeout: Duration,
    mut future_factory: Factory,
) -> Result<(R, u8), String>
where
    E: std::fmt::Display,
    F: Future<Output = Result<R, E>>,
    Factory: FnMut() -> F,
{
    let mut attempt = 1;
    loop {
        match run_with_timeout(runtime, action, timeout, future_factory()) {
            Ok(value) => return Ok((value, attempt)),
            Err(error)
                if attempt < SPARK_TRANSIENT_RETRY_ATTEMPTS
                    && is_transient_wallet_network_error(error.as_str()) =>
            {
                std::thread::sleep(SPARK_TRANSIENT_RETRY_DELAY);
                attempt += 1;
            }
            Err(error) => {
                if attempt > 1 {
                    return Err(format!("{error} after {attempt} Spark network attempts"));
                }
                return Err(error);
            }
        }
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

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn next_wallet_worker_command(
    command_rx: &Receiver<SparkWalletCommand>,
    backlog: &mut VecDeque<SparkWalletCommand>,
) -> Option<SparkWalletCommand> {
    if let Some(command) = backlog.pop_front() {
        return Some(command);
    }
    command_rx.recv().ok()
}

fn is_refresh_like_wallet_command(command: &SparkWalletCommand) -> bool {
    matches!(
        command,
        SparkWalletCommand::Refresh | SparkWalletCommand::Reload
    )
}

fn coalesce_refresh_like_command_burst(
    mut command: SparkWalletCommand,
    command_rx: &Receiver<SparkWalletCommand>,
    backlog: &mut VecDeque<SparkWalletCommand>,
) -> SparkWalletCommand {
    if !is_refresh_like_wallet_command(&command) {
        return command;
    }

    loop {
        let next = if let Some(next) = backlog.pop_front() {
            next
        } else {
            match command_rx.try_recv() {
                Ok(next) => next,
                Err(_) => break,
            }
        };

        if is_refresh_like_wallet_command(&next) {
            if matches!(next, SparkWalletCommand::Reload) {
                command = SparkWalletCommand::Reload;
            }
            continue;
        }

        backlog.push_front(next);
        break;
    }

    command
}

fn ensure_rustls_crypto_provider() -> Result<(), String> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| format!("failed to install rustls crypto provider: {error:?}"))
}

pub fn configured_network() -> Network {
    let configured = std::env::var(ENV_SPARK_NETWORK)
        .unwrap_or_else(|_| "mainnet".to_string())
        .to_ascii_lowercase();

    match configured.as_str() {
        "mainnet" => Network::Mainnet,
        "testnet" => Network::Testnet,
        "signet" => Network::Signet,
        "regtest" => Network::Regtest,
        _ => Network::Mainnet,
    }
}

fn configured_api_key(env_overrides: &HashMap<String, String>) -> String {
    env_overrides
        .get(ENV_SPARK_API_KEY)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var(ENV_SPARK_API_KEY)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_OPENAGENTS_SPARK_API_KEY.to_string())
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
        DEFAULT_OPENAGENTS_SPARK_API_KEY, ENV_SPARK_API_KEY, ENV_SPARK_NETWORK, Network,
        NetworkStatus, NetworkStatusReport, SPARK_ACTION_TIMEOUT, SparkInvoiceState,
        SparkPaneState, SparkWalletCommand, SparkWalletWorker, coalesce_refresh_like_command_burst,
        configured_api_key, configured_network, is_settled_wallet_payment_status,
        is_terminal_wallet_payment_status, run_with_timeout, run_with_transient_retry,
        timeout_message,
    };

    use nostr::ENV_IDENTITY_MNEMONIC_PATH;
    use openagents_spark::{Balance, PaymentSummary};
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Mutex, mpsc};
    use std::time::Duration;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn timeout_message_contains_action_and_duration() {
        let message = timeout_message("refresh", Duration::from_millis(250));
        assert!(message.contains("refresh timed out"));
        assert!(message.contains("250ms"));
    }

    #[test]
    fn configured_network_defaults_to_mainnet_when_unset() {
        let _guard = ENV_LOCK.lock().expect("env mutex poisoned");
        let previous = std::env::var(ENV_SPARK_NETWORK).ok();
        unsafe {
            std::env::remove_var(ENV_SPARK_NETWORK);
        }

        assert_eq!(configured_network(), Network::Mainnet);

        match previous {
            Some(value) => unsafe { std::env::set_var(ENV_SPARK_NETWORK, value) },
            None => unsafe { std::env::remove_var(ENV_SPARK_NETWORK) },
        }
    }

    #[test]
    fn configured_network_accepts_explicit_regtest_override() {
        let _guard = ENV_LOCK.lock().expect("env mutex poisoned");
        let previous = std::env::var(ENV_SPARK_NETWORK).ok();
        unsafe {
            std::env::set_var(ENV_SPARK_NETWORK, "regtest");
        }

        assert_eq!(configured_network(), Network::Regtest);

        match previous {
            Some(value) => unsafe { std::env::set_var(ENV_SPARK_NETWORK, value) },
            None => unsafe { std::env::remove_var(ENV_SPARK_NETWORK) },
        }
    }

    #[test]
    fn configured_network_invalid_value_falls_back_to_mainnet() {
        let _guard = ENV_LOCK.lock().expect("env mutex poisoned");
        let previous = std::env::var(ENV_SPARK_NETWORK).ok();
        unsafe {
            std::env::set_var(ENV_SPARK_NETWORK, "bitcoin");
        }

        assert_eq!(configured_network(), Network::Mainnet);

        match previous {
            Some(value) => unsafe { std::env::set_var(ENV_SPARK_NETWORK, value) },
            None => unsafe { std::env::remove_var(ENV_SPARK_NETWORK) },
        }
    }

    #[test]
    fn configured_api_key_defaults_to_embedded_release_key() {
        let _guard = ENV_LOCK.lock().expect("env mutex poisoned");
        let previous = std::env::var(ENV_SPARK_API_KEY).ok();
        unsafe {
            std::env::remove_var(ENV_SPARK_API_KEY);
        }

        assert_eq!(
            configured_api_key(&HashMap::new()),
            DEFAULT_OPENAGENTS_SPARK_API_KEY
        );

        match previous {
            Some(value) => unsafe { std::env::set_var(ENV_SPARK_API_KEY, value) },
            None => unsafe { std::env::remove_var(ENV_SPARK_API_KEY) },
        }
    }

    #[test]
    fn configured_api_key_prefers_explicit_override() {
        let _guard = ENV_LOCK.lock().expect("env mutex poisoned");
        let previous = std::env::var(ENV_SPARK_API_KEY).ok();
        unsafe {
            std::env::set_var(ENV_SPARK_API_KEY, "env-key");
        }
        let mut overrides = HashMap::new();
        overrides.insert(ENV_SPARK_API_KEY.to_string(), "override-key".to_string());

        assert_eq!(configured_api_key(&overrides), "override-key");

        match previous {
            Some(value) => unsafe { std::env::set_var(ENV_SPARK_API_KEY, value) },
            None => unsafe { std::env::remove_var(ENV_SPARK_API_KEY) },
        }
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
    fn run_with_transient_retry_retries_networkish_errors_before_succeeding() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let attempts = Arc::new(AtomicUsize::new(0));

        let (value, used_attempts) = run_with_transient_retry(
            &runtime,
            "Create Lightning bolt11 invoice",
            Duration::from_secs(1),
            {
                let attempts = Arc::clone(&attempts);
                move || {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    async move {
                        if attempt < 2 {
                            Err::<u8, String>(
                                "network error: error sending request: unexpected EOF".to_string(),
                            )
                        } else {
                            Ok::<u8, String>(7)
                        }
                    }
                }
            },
        )
        .expect("transient retry should eventually succeed");

        assert_eq!(value, 7);
        assert_eq!(used_attempts, 3);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn run_with_transient_retry_does_not_retry_non_transient_errors() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let attempts = Arc::new(AtomicUsize::new(0));

        let error = run_with_transient_retry(
            &runtime,
            "Create Lightning bolt11 invoice",
            Duration::from_secs(1),
            {
                let attempts = Arc::clone(&attempts);
                move || {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    async { Err::<u8, String>("missing Breez API key".to_string()) }
                }
            },
        )
        .expect_err("non-transient errors should fail immediately");

        assert!(error.contains("missing Breez API key"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
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
                SparkWalletCommand::CreateInvoice {
                    amount_sats: 1000,
                    description: None,
                    expiry_seconds: Some(3600),
                },
            );

            let error = state.last_error.expect("invoice should report error");
            assert!(error.contains("No identity mnemonic found"));
            assert!(state.last_invoice.is_none());
        });
    }

    #[test]
    fn spark_invoice_state_tracks_empty_ready_and_expired_targets() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        assert_eq!(state.last_invoice_state(1_000), SparkInvoiceState::Empty);

        state.last_invoice = Some("lnbc1missioncontrol".to_string());
        state.last_invoice_created_at_epoch_seconds = Some(1_000);
        state.last_invoice_expiry_seconds = Some(300);
        assert_eq!(state.last_invoice_state(1_299), SparkInvoiceState::Ready);
        assert_eq!(state.last_invoice_state(1_300), SparkInvoiceState::Expired);

        state.last_invoice_expiry_seconds = None;
        assert_eq!(state.last_invoice_state(9_999), SparkInvoiceState::Ready);
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

    #[test]
    fn balance_refresh_holds_confirmed_balance_while_payment_status_is_pending() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        state.balance = Some(Balance {
            spark_sats: 100,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        state.pending_balance_confirmation_payment_id = Some("pay-123".to_string());

        state.apply_balance_refresh_with_payment_confirmation(
            Some(Balance {
                spark_sats: 80,
                lightning_sats: 0,
                onchain_sats: 0,
            }),
            &[PaymentSummary {
                id: "pay-123".to_string(),
                direction: "send".to_string(),
                status: "pending".to_string(),
                amount_sats: 20,
                timestamp: 1,
                ..Default::default()
            }],
        );

        assert_eq!(state.balance.as_ref().map(Balance::total_sats), Some(100));
        assert_eq!(
            state.pending_balance_confirmation_payment_id.as_deref(),
            Some("pay-123")
        );
    }

    #[test]
    fn balance_refresh_uses_fetched_balance_when_pending_without_prior_snapshot() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        state.pending_balance_confirmation_payment_id = Some("pay-123".to_string());

        state.apply_balance_refresh_with_payment_confirmation(
            Some(Balance {
                spark_sats: 80,
                lightning_sats: 0,
                onchain_sats: 0,
            }),
            &[PaymentSummary {
                id: "pay-123".to_string(),
                direction: "send".to_string(),
                status: "pending".to_string(),
                amount_sats: 20,
                timestamp: 1,
                ..Default::default()
            }],
        );

        assert_eq!(state.balance.as_ref().map(Balance::total_sats), Some(80));
        assert_eq!(
            state.pending_balance_confirmation_payment_id.as_deref(),
            Some("pay-123")
        );
    }

    #[test]
    fn balance_refresh_applies_after_terminal_payment_status() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        state.balance = Some(Balance {
            spark_sats: 100,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        state.pending_balance_confirmation_payment_id = Some("pay-123".to_string());

        state.apply_balance_refresh_with_payment_confirmation(
            Some(Balance {
                spark_sats: 80,
                lightning_sats: 0,
                onchain_sats: 0,
            }),
            &[PaymentSummary {
                id: "pay-123".to_string(),
                direction: "send".to_string(),
                status: "completed".to_string(),
                amount_sats: 20,
                timestamp: 1,
                ..Default::default()
            }],
        );

        assert_eq!(state.balance.as_ref().map(Balance::total_sats), Some(80));
        assert_eq!(state.pending_balance_confirmation_payment_id, None);
    }

    #[test]
    fn balance_refresh_holds_confirmed_balance_when_payment_history_is_missing_entry() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        state.balance = Some(Balance {
            spark_sats: 100,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        state.pending_balance_confirmation_payment_id = Some("pay-123".to_string());

        state.apply_balance_refresh_with_payment_confirmation(
            Some(Balance {
                spark_sats: 80,
                lightning_sats: 0,
                onchain_sats: 0,
            }),
            &[],
        );

        assert_eq!(state.balance.as_ref().map(Balance::total_sats), Some(100));
        assert_eq!(
            state.pending_balance_confirmation_payment_id.as_deref(),
            Some("pay-123")
        );
        assert!(
            state
                .last_action
                .as_deref()
                .is_some_and(|value| value.contains("awaiting Spark confirmation"))
        );
    }

    #[test]
    fn refresh_command_burst_coalesces_to_single_reload() {
        let (_tx, rx) = mpsc::channel::<SparkWalletCommand>();
        let mut backlog = VecDeque::from([
            SparkWalletCommand::Refresh,
            SparkWalletCommand::Reload,
            SparkWalletCommand::Refresh,
            SparkWalletCommand::GenerateSparkAddress,
        ]);

        let command =
            coalesce_refresh_like_command_burst(SparkWalletCommand::Refresh, &rx, &mut backlog);

        assert!(matches!(command, SparkWalletCommand::Reload));
        assert!(matches!(
            backlog.pop_front(),
            Some(SparkWalletCommand::GenerateSparkAddress)
        ));
    }

    #[test]
    fn startup_convergence_refresh_due_after_interval() {
        let mut state = SparkPaneState::with_network(Network::Regtest);

        state.begin_startup_convergence(100);

        assert_eq!(state.network_status_label(), "reconciling");
        assert!(!state.startup_convergence_refresh_due(101));
        assert!(state.startup_convergence_refresh_due(102));

        state.note_startup_convergence_refresh_queued(102);
        assert!(!state.startup_convergence_refresh_due(103));
        assert!(state.startup_convergence_refresh_due(104));
    }

    #[test]
    fn startup_convergence_status_reports_reconciling_until_followups_finish() {
        with_missing_identity_env(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let mut state = SparkPaneState::with_network(Network::Regtest);

            state.begin_startup_convergence(100);
            assert_eq!(state.network_status_label(), "reconciling");

            state.note_startup_convergence_refresh_queued(102);
            state.note_startup_convergence_refresh_queued(104);
            state.note_startup_convergence_refresh_queued(106);

            state.apply_command(&runtime, SparkWalletCommand::Refresh);

            assert!(
                !state.startup_convergence_active,
                "final startup refresh should clear reconciling state even on error"
            );
            assert_eq!(state.network_status_label(), "unknown");
            assert!(state.last_error.is_some());
        });
    }

    #[test]
    fn startup_convergence_clears_early_once_wallet_balance_is_ready() {
        let mut state = SparkPaneState::with_network(Network::Regtest);

        state.begin_startup_convergence(100);
        state.network_status = Some(NetworkStatusReport {
            status: NetworkStatus::Disconnected,
            detail: None,
        });
        state.balance = Some(Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });

        state.complete_startup_convergence_after_refresh();

        assert!(!state.startup_convergence_active);
        assert_eq!(state.network_status_label(), "disconnected");
    }

    #[test]
    fn refresh_throttle_blocks_recent_healthy_duplicate_refreshes() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        state.network_status = Some(NetworkStatusReport {
            status: NetworkStatus::Connected,
            detail: None,
        });
        state.balance = Some(Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        state.last_refresh_started_at = Some(std::time::Instant::now());

        assert!(state.should_throttle_refresh());
    }

    #[test]
    fn refresh_throttle_does_not_block_startup_convergence_retries() {
        let mut state = SparkPaneState::with_network(Network::Regtest);
        state.begin_startup_convergence(100);
        state.last_refresh_started_at = Some(std::time::Instant::now());

        assert!(!state.should_throttle_refresh());
    }

    #[test]
    fn payment_status_helpers_match_terminal_and_settled_states() {
        assert!(is_settled_wallet_payment_status("completed"));
        assert!(is_terminal_wallet_payment_status("completed"));
        assert!(is_terminal_wallet_payment_status("failed"));
        assert!(!is_terminal_wallet_payment_status("pending"));
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
