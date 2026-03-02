use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

const DEFAULT_WORKER_TIMEOUT: Duration = Duration::from_secs(8);
const SWAP_WORKER_TIMEOUT: Duration = Duration::from_secs(12);
const WORKER_QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
pub struct StableSatsBlinkRefreshRequest {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub balance_script_path: PathBuf,
    pub price_script_path: PathBuf,
    pub wallets: Vec<StableSatsBlinkWalletRefreshRequest>,
    pub preflight_failures: Vec<StableSatsBlinkWalletFailure>,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkWalletRefreshRequest {
    pub owner_id: String,
    pub wallet_name: String,
    pub env_overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkWalletSnapshot {
    pub owner_id: String,
    pub btc_balance_sats: u64,
    pub usd_balance_cents: u64,
    pub source_ref: String,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkWalletFailure {
    pub owner_id: String,
    pub wallet_name: String,
    pub error: String,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkLiveSnapshot {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub wallet_snapshots: Vec<StableSatsBlinkWalletSnapshot>,
    pub wallet_failures: Vec<StableSatsBlinkWalletFailure>,
    pub price_usd_cents_per_btc: u64,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkSwapQuoteRequest {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub goal_id: String,
    pub adapter_request_id: String,
    pub script_path: PathBuf,
    pub script_args: Vec<String>,
    pub env_overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkSwapQuoteResult {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub goal_id: String,
    pub adapter_request_id: String,
    pub script_path: String,
    pub script_args: Vec<String>,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkSwapExecuteRequest {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub goal_id: String,
    pub quote_id: String,
    pub script_path: PathBuf,
    pub script_args: Vec<String>,
    pub env_overrides: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkSwapExecuteResult {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub goal_id: String,
    pub quote_id: String,
    pub script_path: String,
    pub script_args: Vec<String>,
    pub payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsBlinkTransferAsset {
    BtcSats,
    UsdCents,
}

impl StableSatsBlinkTransferAsset {
    pub const fn label(self) -> &'static str {
        match self {
            Self::BtcSats => "btc_sats",
            Self::UsdCents => "usd_cents",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkTransferRequest {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub from_owner_id: String,
    pub from_wallet_name: String,
    pub to_owner_id: String,
    pub to_wallet_name: String,
    pub asset: StableSatsBlinkTransferAsset,
    pub amount: u64,
    pub memo: Option<String>,
    pub source_env_overrides: Vec<(String, String)>,
    pub destination_env_overrides: Vec<(String, String)>,
    pub balance_script_path: PathBuf,
    pub create_invoice_script_path: PathBuf,
    pub create_invoice_usd_script_path: PathBuf,
    pub fee_probe_script_path: PathBuf,
    pub pay_invoice_script_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkTransferResult {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub from_owner_id: String,
    pub from_wallet_name: String,
    pub to_owner_id: String,
    pub to_wallet_name: String,
    pub asset: StableSatsBlinkTransferAsset,
    pub amount: u64,
    pub payment_status: String,
    pub payment_reference: Option<String>,
    pub estimated_fee_sats: u64,
    pub effective_fee: u64,
    pub source_pre_btc_sats: u64,
    pub source_pre_usd_cents: u64,
    pub source_post_btc_sats: u64,
    pub source_post_usd_cents: u64,
    pub destination_pre_btc_sats: u64,
    pub destination_pre_usd_cents: u64,
    pub destination_post_btc_sats: u64,
    pub destination_post_usd_cents: u64,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkConvertRequest {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub owner_id: String,
    pub wallet_name: String,
    pub direction: String,
    pub amount: u64,
    pub unit: String,
    pub memo: Option<String>,
    pub env_overrides: Vec<(String, String)>,
    pub swap_execute_script_path: PathBuf,
    pub swap_quote_script_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StableSatsBlinkConvertResult {
    pub request_id: u64,
    pub now_epoch_seconds: u64,
    pub owner_id: String,
    pub wallet_name: String,
    pub direction: String,
    pub amount: u64,
    pub unit: String,
    pub status: String,
    pub quote_id: Option<String>,
    pub transaction_id: Option<String>,
    pub fee_sats: u64,
    pub effective_spread_bps: u32,
    pub pre_btc_sats: u64,
    pub pre_usd_cents: u64,
    pub post_btc_sats: u64,
    pub post_usd_cents: u64,
    pub payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableSatsBlinkCommandKind {
    Refresh,
    SwapQuote,
    SwapExecute,
    Transfer,
    Convert,
}

impl StableSatsBlinkCommandKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::SwapQuote => "swap_quote",
            Self::SwapExecute => "swap_execute",
            Self::Transfer => "transfer",
            Self::Convert => "convert",
        }
    }
}

#[derive(Clone, Debug)]
pub enum StableSatsBlinkUpdate {
    CommandStarted {
        request_id: u64,
        kind: StableSatsBlinkCommandKind,
    },
    CommandCancelled {
        request_id: u64,
        kind: StableSatsBlinkCommandKind,
        detail: String,
    },
    Completed(StableSatsBlinkLiveSnapshot),
    Failed {
        request_id: u64,
        error: String,
    },
    SwapQuoteCompleted(StableSatsBlinkSwapQuoteResult),
    SwapQuoteFailed {
        request_id: u64,
        goal_id: String,
        adapter_request_id: String,
        error: String,
    },
    SwapExecuteCompleted(StableSatsBlinkSwapExecuteResult),
    SwapExecuteFailed {
        request_id: u64,
        goal_id: String,
        quote_id: String,
        error: String,
    },
    TransferCompleted(StableSatsBlinkTransferResult),
    TransferFailed {
        request_id: u64,
        from_owner_id: String,
        to_owner_id: String,
        error: String,
    },
    ConvertCompleted(StableSatsBlinkConvertResult),
    ConvertFailed {
        request_id: u64,
        owner_id: String,
        error: String,
    },
}

enum StableSatsBlinkCommand {
    Refresh(StableSatsBlinkRefreshRequest),
    SwapQuote(StableSatsBlinkSwapQuoteRequest),
    SwapExecute(StableSatsBlinkSwapExecuteRequest),
    Transfer(StableSatsBlinkTransferRequest),
    Convert(StableSatsBlinkConvertRequest),
    CancelPending,
}

pub struct StableSatsBlinkWorker {
    command_tx: SyncSender<StableSatsBlinkCommand>,
    update_rx: Receiver<StableSatsBlinkUpdate>,
}

impl StableSatsBlinkWorker {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) =
            mpsc::sync_channel::<StableSatsBlinkCommand>(WORKER_QUEUE_CAPACITY);
        let (update_tx, update_rx) = mpsc::channel::<StableSatsBlinkUpdate>();
        std::thread::Builder::new()
            .name("stablesats-blink-worker".to_string())
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    match command {
                        StableSatsBlinkCommand::Refresh(request) => {
                            if update_tx
                                .send(StableSatsBlinkUpdate::CommandStarted {
                                    request_id: request.request_id,
                                    kind: StableSatsBlinkCommandKind::Refresh,
                                })
                                .is_err()
                            {
                                break;
                            }
                            let update = match run_refresh(&request) {
                                Ok(snapshot) => StableSatsBlinkUpdate::Completed(snapshot),
                                Err(error) => StableSatsBlinkUpdate::Failed {
                                    request_id: request.request_id,
                                    error,
                                },
                            };
                            if update_tx.send(update).is_err() {
                                break;
                            }
                        }
                        StableSatsBlinkCommand::SwapQuote(request) => {
                            if update_tx
                                .send(StableSatsBlinkUpdate::CommandStarted {
                                    request_id: request.request_id,
                                    kind: StableSatsBlinkCommandKind::SwapQuote,
                                })
                                .is_err()
                            {
                                break;
                            }
                            let update = match run_swap_quote(&request) {
                                Ok(result) => StableSatsBlinkUpdate::SwapQuoteCompleted(result),
                                Err(error) => StableSatsBlinkUpdate::SwapQuoteFailed {
                                    request_id: request.request_id,
                                    goal_id: request.goal_id.clone(),
                                    adapter_request_id: request.adapter_request_id.clone(),
                                    error,
                                },
                            };
                            if update_tx.send(update).is_err() {
                                break;
                            }
                        }
                        StableSatsBlinkCommand::SwapExecute(request) => {
                            if update_tx
                                .send(StableSatsBlinkUpdate::CommandStarted {
                                    request_id: request.request_id,
                                    kind: StableSatsBlinkCommandKind::SwapExecute,
                                })
                                .is_err()
                            {
                                break;
                            }
                            let update = match run_swap_execute(&request) {
                                Ok(result) => StableSatsBlinkUpdate::SwapExecuteCompleted(result),
                                Err(error) => StableSatsBlinkUpdate::SwapExecuteFailed {
                                    request_id: request.request_id,
                                    goal_id: request.goal_id.clone(),
                                    quote_id: request.quote_id.clone(),
                                    error,
                                },
                            };
                            if update_tx.send(update).is_err() {
                                break;
                            }
                        }
                        StableSatsBlinkCommand::Transfer(request) => {
                            if update_tx
                                .send(StableSatsBlinkUpdate::CommandStarted {
                                    request_id: request.request_id,
                                    kind: StableSatsBlinkCommandKind::Transfer,
                                })
                                .is_err()
                            {
                                break;
                            }
                            let update = match run_transfer(&request) {
                                Ok(result) => StableSatsBlinkUpdate::TransferCompleted(result),
                                Err(error) => StableSatsBlinkUpdate::TransferFailed {
                                    request_id: request.request_id,
                                    from_owner_id: request.from_owner_id.clone(),
                                    to_owner_id: request.to_owner_id.clone(),
                                    error,
                                },
                            };
                            if update_tx.send(update).is_err() {
                                break;
                            }
                        }
                        StableSatsBlinkCommand::Convert(request) => {
                            if update_tx
                                .send(StableSatsBlinkUpdate::CommandStarted {
                                    request_id: request.request_id,
                                    kind: StableSatsBlinkCommandKind::Convert,
                                })
                                .is_err()
                            {
                                break;
                            }
                            let update = match run_convert(&request) {
                                Ok(result) => StableSatsBlinkUpdate::ConvertCompleted(result),
                                Err(error) => StableSatsBlinkUpdate::ConvertFailed {
                                    request_id: request.request_id,
                                    owner_id: request.owner_id.clone(),
                                    error,
                                },
                            };
                            if update_tx.send(update).is_err() {
                                break;
                            }
                        }
                        StableSatsBlinkCommand::CancelPending => {
                            while let Ok(pending) = command_rx.try_recv() {
                                if let Some((request_id, kind)) = command_identity(&pending) {
                                    let _ =
                                        update_tx.send(StableSatsBlinkUpdate::CommandCancelled {
                                            request_id,
                                            kind,
                                            detail: "command cancelled before execution"
                                                .to_string(),
                                        });
                                }
                            }
                        }
                    }
                }
            })
            .expect("stablesats blink worker thread should spawn");

        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue_refresh(&self, request: StableSatsBlinkRefreshRequest) -> Result<(), String> {
        self.command_tx
            .try_send(StableSatsBlinkCommand::Refresh(request))
            .map_err(|error| format!("stablesats blink worker offline: {error}"))
    }

    pub fn enqueue_swap_quote(
        &self,
        request: StableSatsBlinkSwapQuoteRequest,
    ) -> Result<(), String> {
        self.command_tx
            .try_send(StableSatsBlinkCommand::SwapQuote(request))
            .map_err(|error| format!("stablesats blink worker unavailable: {error}"))
    }

    pub fn enqueue_swap_execute(
        &self,
        request: StableSatsBlinkSwapExecuteRequest,
    ) -> Result<(), String> {
        self.command_tx
            .try_send(StableSatsBlinkCommand::SwapExecute(request))
            .map_err(|error| format!("stablesats blink worker unavailable: {error}"))
    }

    pub fn enqueue_transfer(&self, request: StableSatsBlinkTransferRequest) -> Result<(), String> {
        self.command_tx
            .try_send(StableSatsBlinkCommand::Transfer(request))
            .map_err(|error| format!("stablesats blink worker unavailable: {error}"))
    }

    pub fn enqueue_convert(&self, request: StableSatsBlinkConvertRequest) -> Result<(), String> {
        self.command_tx
            .try_send(StableSatsBlinkCommand::Convert(request))
            .map_err(|error| format!("stablesats blink worker unavailable: {error}"))
    }

    pub fn cancel_pending(&self) -> Result<(), String> {
        self.command_tx
            .try_send(StableSatsBlinkCommand::CancelPending)
            .map_err(|error| format!("stablesats blink worker cancel failed: {error}"))
    }

    pub fn drain_updates(&self, max_items: usize) -> Vec<StableSatsBlinkUpdate> {
        let mut updates = Vec::new();
        for _ in 0..max_items {
            match self.update_rx.try_recv() {
                Ok(update) => updates.push(update),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        updates
    }
}

fn run_refresh(
    request: &StableSatsBlinkRefreshRequest,
) -> Result<StableSatsBlinkLiveSnapshot, String> {
    if request.wallets.is_empty() {
        return Err("Live refresh request did not include any wallet bindings".to_string());
    }

    let mut wallet_snapshots = Vec::<StableSatsBlinkWalletSnapshot>::new();
    let mut wallet_failures = request.preflight_failures.clone();

    for wallet in &request.wallets {
        let balance_json = match run_blink_script_json(
            request.balance_script_path.as_path(),
            &[],
            wallet.env_overrides.as_slice(),
            DEFAULT_WORKER_TIMEOUT,
        ) {
            Ok(value) => value,
            Err(error) => {
                wallet_failures.push(StableSatsBlinkWalletFailure {
                    owner_id: wallet.owner_id.clone(),
                    wallet_name: wallet.wallet_name.clone(),
                    error,
                });
                continue;
            }
        };
        let btc_balance_sats = match parse_json_u64(&balance_json, "btcBalanceSats")
            .or_else(|| parse_json_u64(&balance_json, "btcBalance"))
        {
            Some(value) => value,
            None => {
                wallet_failures.push(StableSatsBlinkWalletFailure {
                    owner_id: wallet.owner_id.clone(),
                    wallet_name: wallet.wallet_name.clone(),
                    error: "Blink balance payload missing btcBalanceSats".to_string(),
                });
                continue;
            }
        };
        let usd_balance_cents = match parse_json_u64(&balance_json, "usdBalanceCents")
            .or_else(|| parse_json_u64(&balance_json, "usdBalance"))
        {
            Some(value) => value,
            None => {
                wallet_failures.push(StableSatsBlinkWalletFailure {
                    owner_id: wallet.owner_id.clone(),
                    wallet_name: wallet.wallet_name.clone(),
                    error: "Blink balance payload missing usdBalanceCents".to_string(),
                });
                continue;
            }
        };

        let btc_wallet_id = balance_json
            .get("btcWalletId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown-btc");
        let usd_wallet_id = balance_json
            .get("usdWalletId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown-usd");

        wallet_snapshots.push(StableSatsBlinkWalletSnapshot {
            owner_id: wallet.owner_id.clone(),
            btc_balance_sats,
            usd_balance_cents,
            source_ref: format!("btc:{} usd:{}", btc_wallet_id, usd_wallet_id),
        });
    }

    if wallet_snapshots.is_empty() {
        let message = wallet_failures
            .iter()
            .map(|entry| {
                format!(
                    "{} ({}) refresh failed: {}",
                    entry.wallet_name, entry.owner_id, entry.error
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(if message.is_empty() {
            "All wallet refresh requests failed".to_string()
        } else {
            message
        });
    }

    let price_env = wallet_snapshots
        .first()
        .and_then(|snapshot| {
            request
                .wallets
                .iter()
                .find(|wallet| wallet.owner_id == snapshot.owner_id)
        })
        .map_or(&[][..], |wallet| wallet.env_overrides.as_slice());
    let price_json = run_blink_script_json(
        request.price_script_path.as_path(),
        &[],
        price_env,
        DEFAULT_WORKER_TIMEOUT,
    )
    .ok();
    let price_usd_cents_per_btc = price_json
        .as_ref()
        .and_then(|json| parse_json_f64(json, "btcPriceUsd"))
        .map(|price| (price * 100.0).round())
        .and_then(|price| {
            if price.is_finite() && price >= 1.0 {
                Some(price as u64)
            } else {
                None
            }
        })
        .unwrap_or(1);

    Ok(StableSatsBlinkLiveSnapshot {
        request_id: request.request_id,
        now_epoch_seconds: request.now_epoch_seconds,
        wallet_snapshots,
        wallet_failures,
        price_usd_cents_per_btc,
    })
}

fn command_identity(command: &StableSatsBlinkCommand) -> Option<(u64, StableSatsBlinkCommandKind)> {
    match command {
        StableSatsBlinkCommand::Refresh(request) => {
            Some((request.request_id, StableSatsBlinkCommandKind::Refresh))
        }
        StableSatsBlinkCommand::SwapQuote(request) => {
            Some((request.request_id, StableSatsBlinkCommandKind::SwapQuote))
        }
        StableSatsBlinkCommand::SwapExecute(request) => {
            Some((request.request_id, StableSatsBlinkCommandKind::SwapExecute))
        }
        StableSatsBlinkCommand::Transfer(request) => {
            Some((request.request_id, StableSatsBlinkCommandKind::Transfer))
        }
        StableSatsBlinkCommand::Convert(request) => {
            Some((request.request_id, StableSatsBlinkCommandKind::Convert))
        }
        StableSatsBlinkCommand::CancelPending => None,
    }
}

fn run_swap_quote(
    request: &StableSatsBlinkSwapQuoteRequest,
) -> Result<StableSatsBlinkSwapQuoteResult, String> {
    let payload = run_blink_script_json(
        request.script_path.as_path(),
        request.script_args.as_slice(),
        request.env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    Ok(StableSatsBlinkSwapQuoteResult {
        request_id: request.request_id,
        now_epoch_seconds: request.now_epoch_seconds,
        goal_id: request.goal_id.clone(),
        adapter_request_id: request.adapter_request_id.clone(),
        script_path: request.script_path.display().to_string(),
        script_args: request.script_args.clone(),
        payload,
    })
}

fn run_swap_execute(
    request: &StableSatsBlinkSwapExecuteRequest,
) -> Result<StableSatsBlinkSwapExecuteResult, String> {
    let payload = run_blink_script_json(
        request.script_path.as_path(),
        request.script_args.as_slice(),
        request.env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    Ok(StableSatsBlinkSwapExecuteResult {
        request_id: request.request_id,
        now_epoch_seconds: request.now_epoch_seconds,
        goal_id: request.goal_id.clone(),
        quote_id: request.quote_id.clone(),
        script_path: request.script_path.display().to_string(),
        script_args: request.script_args.clone(),
        payload,
    })
}

fn run_transfer(
    request: &StableSatsBlinkTransferRequest,
) -> Result<StableSatsBlinkTransferResult, String> {
    let source_pre = run_blink_script_json(
        request.balance_script_path.as_path(),
        &[],
        request.source_env_overrides.as_slice(),
        DEFAULT_WORKER_TIMEOUT,
    )?;
    let destination_pre = run_blink_script_json(
        request.balance_script_path.as_path(),
        &[],
        request.destination_env_overrides.as_slice(),
        DEFAULT_WORKER_TIMEOUT,
    )?;
    let (source_pre_btc_sats, source_pre_usd_cents) = parse_balance_snapshot(&source_pre)?;
    let (destination_pre_btc_sats, destination_pre_usd_cents) =
        parse_balance_snapshot(&destination_pre)?;

    let mut invoice_args = vec![request.amount.to_string(), "--no-subscribe".to_string()];
    if let Some(memo) = request
        .memo
        .as_deref()
        .map(str::trim)
        .filter(|memo| !memo.is_empty())
    {
        invoice_args.push(memo.to_string());
    }
    let invoice_script = match request.asset {
        StableSatsBlinkTransferAsset::BtcSats => request.create_invoice_script_path.as_path(),
        StableSatsBlinkTransferAsset::UsdCents => request.create_invoice_usd_script_path.as_path(),
    };
    let invoice_payload = run_blink_script_json(
        invoice_script,
        invoice_args.as_slice(),
        request.destination_env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    let payment_request = parse_json_string(&invoice_payload, "paymentRequest")
        .ok_or_else(|| "Blink invoice payload missing paymentRequest".to_string())?;
    let payment_reference = parse_json_string(&invoice_payload, "paymentHash");

    let wallet_flag = match request.asset {
        StableSatsBlinkTransferAsset::BtcSats => "BTC",
        StableSatsBlinkTransferAsset::UsdCents => "USD",
    };
    let fee_probe_payload = run_blink_script_json(
        request.fee_probe_script_path.as_path(),
        &[
            payment_request.clone(),
            "--wallet".to_string(),
            wallet_flag.to_string(),
        ],
        request.source_env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    let estimated_fee_sats = parse_json_u64(&fee_probe_payload, "estimatedFeeSats").unwrap_or(0);

    let pay_payload = run_blink_script_json(
        request.pay_invoice_script_path.as_path(),
        &[
            payment_request.clone(),
            "--wallet".to_string(),
            wallet_flag.to_string(),
        ],
        request.source_env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    let payment_status =
        parse_json_string(&pay_payload, "status").unwrap_or_else(|| "UNKNOWN".to_string());

    let source_post = run_blink_script_json(
        request.balance_script_path.as_path(),
        &[],
        request.source_env_overrides.as_slice(),
        DEFAULT_WORKER_TIMEOUT,
    )?;
    let destination_post = run_blink_script_json(
        request.balance_script_path.as_path(),
        &[],
        request.destination_env_overrides.as_slice(),
        DEFAULT_WORKER_TIMEOUT,
    )?;
    let (source_post_btc_sats, source_post_usd_cents) = parse_balance_snapshot(&source_post)?;
    let (destination_post_btc_sats, destination_post_usd_cents) =
        parse_balance_snapshot(&destination_post)?;

    let effective_fee = match request.asset {
        StableSatsBlinkTransferAsset::BtcSats => {
            let source_sent = source_pre_btc_sats.saturating_sub(source_post_btc_sats);
            let destination_received =
                destination_post_btc_sats.saturating_sub(destination_pre_btc_sats);
            source_sent.saturating_sub(destination_received)
        }
        StableSatsBlinkTransferAsset::UsdCents => {
            let source_sent = source_pre_usd_cents.saturating_sub(source_post_usd_cents);
            let destination_received =
                destination_post_usd_cents.saturating_sub(destination_pre_usd_cents);
            source_sent.saturating_sub(destination_received)
        }
    };

    Ok(StableSatsBlinkTransferResult {
        request_id: request.request_id,
        now_epoch_seconds: request.now_epoch_seconds,
        from_owner_id: request.from_owner_id.clone(),
        from_wallet_name: request.from_wallet_name.clone(),
        to_owner_id: request.to_owner_id.clone(),
        to_wallet_name: request.to_wallet_name.clone(),
        asset: request.asset,
        amount: request.amount,
        payment_status,
        payment_reference,
        estimated_fee_sats,
        effective_fee,
        source_pre_btc_sats,
        source_pre_usd_cents,
        source_post_btc_sats,
        source_post_usd_cents,
        destination_pre_btc_sats,
        destination_pre_usd_cents,
        destination_post_btc_sats,
        destination_post_usd_cents,
        payload: serde_json::json!({
            "invoice": invoice_payload,
            "fee_probe": fee_probe_payload,
            "payment": pay_payload,
            "payment_request": payment_request,
        }),
    })
}

fn run_convert(
    request: &StableSatsBlinkConvertRequest,
) -> Result<StableSatsBlinkConvertResult, String> {
    let quote_args = vec![
        request.direction.clone(),
        request.amount.to_string(),
        "--unit".to_string(),
        request.unit.clone(),
    ];
    let mut execute_args = quote_args.clone();
    if let Some(memo) = request
        .memo
        .as_deref()
        .map(str::trim)
        .filter(|memo| !memo.is_empty())
    {
        execute_args.push("--memo".to_string());
        execute_args.push(memo.to_string());
    }

    let quote_payload = run_blink_script_json(
        request.swap_quote_script_path.as_path(),
        quote_args.as_slice(),
        request.env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    let execute_payload = run_blink_script_json(
        request.swap_execute_script_path.as_path(),
        execute_args.as_slice(),
        request.env_overrides.as_slice(),
        SWAP_WORKER_TIMEOUT,
    )?;
    let status =
        parse_json_string(&execute_payload, "status").unwrap_or_else(|| "UNKNOWN".to_string());
    let quote_id = quote_payload
        .pointer("/quote/quoteId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let fee_sats = quote_payload
        .pointer("/quote/feeSats")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
        })
        .unwrap_or(0);
    let effective_spread_bps = quote_payload
        .pointer("/quote/slippageBps")
        .and_then(|value| value.as_u64())
        .map(|value| value as u32)
        .unwrap_or(0);
    let transaction_id = execute_payload
        .pointer("/execution/transactionId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let pre_btc_sats = execute_payload
        .pointer("/preBalance/btcBalanceSats")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let pre_usd_cents = execute_payload
        .pointer("/preBalance/usdBalanceCents")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let post_btc_sats = execute_payload
        .pointer("/postBalance/btcBalanceSats")
        .and_then(|value| value.as_u64())
        .unwrap_or(pre_btc_sats);
    let post_usd_cents = execute_payload
        .pointer("/postBalance/usdBalanceCents")
        .and_then(|value| value.as_u64())
        .unwrap_or(pre_usd_cents);

    Ok(StableSatsBlinkConvertResult {
        request_id: request.request_id,
        now_epoch_seconds: request.now_epoch_seconds,
        owner_id: request.owner_id.clone(),
        wallet_name: request.wallet_name.clone(),
        direction: request.direction.clone(),
        amount: request.amount,
        unit: request.unit.clone(),
        status,
        quote_id,
        transaction_id,
        fee_sats,
        effective_spread_bps,
        pre_btc_sats,
        pre_usd_cents,
        post_btc_sats,
        post_usd_cents,
        payload: serde_json::json!({
            "quote": quote_payload,
            "execute": execute_payload,
            "args": {
                "quote": quote_args,
                "execute": execute_args,
            }
        }),
    })
}

fn parse_balance_snapshot(payload: &serde_json::Value) -> Result<(u64, u64), String> {
    let btc_balance_sats = parse_json_u64(payload, "btcBalanceSats")
        .or_else(|| parse_json_u64(payload, "btcBalance"))
        .ok_or_else(|| "Blink balance payload missing btcBalanceSats".to_string())?;
    let usd_balance_cents = parse_json_u64(payload, "usdBalanceCents")
        .or_else(|| parse_json_u64(payload, "usdBalance"))
        .ok_or_else(|| "Blink balance payload missing usdBalanceCents".to_string())?;
    Ok((btc_balance_sats, usd_balance_cents))
}

fn run_blink_script_json(
    script_path: &Path,
    args: &[String],
    env_overrides: &[(String, String)],
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let mut command = std::process::Command::new("node");
    command.arg(script_path);
    command.args(args);
    for (name, value) in env_overrides {
        command.env(name, value);
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        format!(
            "Failed launching Blink script {}: {error}",
            script_path.display()
        )
    })?;

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Blink script {} timed out after {} ms",
                        script_path.display(),
                        timeout.as_millis()
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                return Err(format!(
                    "Blink script {} wait failed: {error}",
                    script_path.display()
                ));
            }
        }
    }

    let output = child.wait_with_output().map_err(|error| {
        format!(
            "Failed collecting output from Blink script {}: {error}",
            script_path.display()
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let status = output.status.code().map_or_else(
            || "signal".to_string(),
            |value| format!("exit_code={value}"),
        );
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output".to_string()
        };
        return Err(format!(
            "Blink script {} failed ({status}): {details}",
            script_path.display()
        ));
    }
    if stdout.is_empty() {
        return Err(format!(
            "Blink script {} returned empty stdout",
            script_path.display()
        ));
    }

    serde_json::from_str::<serde_json::Value>(&stdout).map_err(|error| {
        format!(
            "Blink script {} returned non-JSON stdout: {error}",
            script_path.display()
        )
    })
}

fn parse_json_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    let raw = value.get(key)?;
    raw.as_u64()
        .or_else(|| raw.as_i64().and_then(|number| u64::try_from(number).ok()))
        .or_else(|| {
            raw.as_str()
                .and_then(|text| text.trim().parse::<u64>().ok())
        })
}

fn parse_json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    let raw = value.get(key)?;
    raw.as_f64()
        .or_else(|| raw.as_i64().map(|number| number as f64))
        .or_else(|| raw.as_u64().map(|number| number as f64))
        .or_else(|| {
            raw.as_str()
                .and_then(|text| text.trim().parse::<f64>().ok())
        })
}

fn parse_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}
