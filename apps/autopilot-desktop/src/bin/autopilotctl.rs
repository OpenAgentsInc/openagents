#![allow(
    clippy::print_stdout,
    reason = "CLI intentionally prints structured operator and agent-facing output."
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use autopilot_desktop::desktop_control::{
    DesktopControlActionRequest, DesktopControlActionResponse, DesktopControlActiveJobStatus,
    DesktopControlBuyModeRequestStatus, DesktopControlEventBatch, DesktopControlManifest,
    DesktopControlSnapshot, control_manifest_path,
};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::{Value, json};

const DEFAULT_EVENTS_LIMIT: usize = 64;
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 20_000;

#[derive(Parser, Debug)]
#[command(name = "autopilotctl")]
#[command(about = "Thin CLI client for the running Autopilot desktop control runtime")]
struct Cli {
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long)]
    manifest: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Status,
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    AppleFm {
        #[command(subcommand)]
        command: AppleFmCommand,
    },
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
    BuyMode {
        #[command(subcommand)]
        command: BuyModeCommand,
    },
    ActiveJob,
    Withdraw {
        bolt11: String,
    },
    Logs {
        #[arg(long, default_value_t = 40)]
        tail: usize,
        #[arg(long, value_enum, default_value_t = LogSourceArg::MissionControl)]
        source: LogSourceArg,
    },
    Events {
        #[arg(long, default_value_t = 0)]
        after_event_id: u64,
        #[arg(long, default_value_t = DEFAULT_EVENTS_LIMIT)]
        limit: usize,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Wait {
        #[arg(value_enum)]
        condition: WaitConditionArg,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
}

#[derive(Subcommand, Debug)]
enum ProviderCommand {
    Online {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Offline {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
}

#[derive(Subcommand, Debug)]
enum AppleFmCommand {
    Refresh {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    SmokeTest,
}

#[derive(Subcommand, Debug)]
enum WalletCommand {
    Refresh,
}

#[derive(Subcommand, Debug)]
enum BuyModeCommand {
    Start {
        #[arg(long)]
        approved_budget_sats: u64,
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Stop {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum LogSourceArg {
    MissionControl,
    Session,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum WaitConditionArg {
    ProviderOnline,
    ProviderOffline,
    AppleFmReady,
    BuyModeRunning,
    BuyModeStopped,
    BuyModeInFlight,
    BuyModePaymentRequired,
    ActiveJobPresent,
    ActiveJobRunning,
    ActiveJobDelivered,
    ActiveJobSettling,
    ActiveJobPaid,
    ActiveJobFailed,
}

#[derive(Clone, Debug)]
struct ResolvedTarget {
    base_url: String,
    auth_token: String,
    manifest_path: Option<PathBuf>,
    latest_session_log_path: Option<PathBuf>,
}

struct DesktopControlClient {
    http: Client,
    target: ResolvedTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitCondition {
    ProviderOnline,
    ProviderOffline,
    AppleFmReady,
    BuyModeRunning,
    BuyModeStopped,
    BuyModeInFlight,
    BuyModePaymentRequired,
    ActiveJobPresent,
    ActiveJobRunning,
    ActiveJobDelivered,
    ActiveJobSettling,
    ActiveJobPaid,
    ActiveJobFailed,
}

#[derive(Serialize)]
struct StatusEnvelope<'a> {
    base_url: &'a str,
    manifest_path: Option<String>,
    latest_session_log_path: Option<String>,
    snapshot: &'a DesktopControlSnapshot,
}

#[derive(Serialize)]
struct ActionEnvelope<'a> {
    response: &'a DesktopControlActionResponse,
    snapshot: Option<&'a DesktopControlSnapshot>,
}

#[derive(Serialize)]
struct LogEnvelope {
    source: String,
    path: Option<String>,
    lines: Vec<Value>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let json_output = cli.json;
    let target = resolve_target(&cli)?;
    let client = DesktopControlClient::new(target)?;
    match cli.command {
        Command::Status => {
            let snapshot = client.snapshot()?;
            if json_output {
                print_json(&StatusEnvelope {
                    base_url: client.target.base_url.as_str(),
                    manifest_path: client
                        .target
                        .manifest_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                    latest_session_log_path: client
                        .target
                        .latest_session_log_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                    snapshot: &snapshot,
                })?;
            } else {
                print_status_text(&client.target, &snapshot);
            }
        }
        Command::Provider { command } => match command {
            ProviderCommand::Online { wait, timeout_ms } => {
                let response =
                    client.action(&DesktopControlActionRequest::SetProviderMode { online: true })?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::ProviderOnline, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            ProviderCommand::Offline { wait, timeout_ms } => {
                let response =
                    client.action(&DesktopControlActionRequest::SetProviderMode { online: false })?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::ProviderOffline, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
        },
        Command::AppleFm { command } => match command {
            AppleFmCommand::Refresh { wait, timeout_ms } => {
                let response = client.action(&DesktopControlActionRequest::RefreshAppleFm)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::AppleFmReady, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            AppleFmCommand::SmokeTest => {
                let response = client.action(&DesktopControlActionRequest::RunAppleFmSmokeTest)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
        },
        Command::Wallet { command } => match command {
            WalletCommand::Refresh => {
                let response = client.action(&DesktopControlActionRequest::RefreshWallet)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
        },
        Command::BuyMode { command } => match command {
            BuyModeCommand::Start {
                approved_budget_sats,
                wait,
                timeout_ms,
            } => {
                let snapshot = client.snapshot()?;
                ensure_buy_mode_budget_ack(&snapshot, approved_budget_sats)?;
                let response = client.action(&DesktopControlActionRequest::StartBuyMode)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::BuyModeRunning, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            BuyModeCommand::Stop { wait, timeout_ms } => {
                let response = client.action(&DesktopControlActionRequest::StopBuyMode)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::BuyModeStopped, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            BuyModeCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&json!({
                        "approvedBudgetSats": snapshot.buy_mode.approved_budget_sats,
                        "cadenceSeconds": snapshot.buy_mode.cadence_seconds,
                        "enabled": snapshot.buy_mode.enabled,
                        "nextDispatchCountdownSeconds": snapshot.buy_mode.next_dispatch_countdown_seconds,
                        "inFlightRequestId": snapshot.buy_mode.in_flight_request_id,
                        "inFlightPhase": snapshot.buy_mode.in_flight_phase,
                        "inFlightStatus": snapshot.buy_mode.in_flight_status,
                        "selectedProviderPubkey": snapshot.buy_mode.selected_provider_pubkey,
                        "payableProviderPubkey": snapshot.buy_mode.payable_provider_pubkey,
                        "recentRequests": snapshot.buy_mode.recent_requests,
                    }))?;
                } else {
                    print_buy_mode_text(&snapshot);
                }
            }
        },
        Command::ActiveJob => {
            let snapshot = client.snapshot()?;
            if json_output {
                print_json(&json!({
                    "activeJob": snapshot.active_job,
                    "latestSessionLogPath": client
                        .target
                        .latest_session_log_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                }))?;
            } else {
                print_active_job_text(snapshot.active_job.as_ref());
            }
        }
        Command::Withdraw { bolt11 } => {
            let response = client.action(&DesktopControlActionRequest::Withdraw { bolt11 })?;
            ensure_action_success(&response)?;
            print_action(json_output, &response, None)?;
        }
        Command::Logs { tail, source } => match source {
            LogSourceArg::MissionControl => {
                let lines = client.mission_control_log_tail(tail)?;
                if json_output {
                    print_json(&LogEnvelope {
                        source: "mission-control".to_string(),
                        path: None,
                        lines: lines.into_iter().map(Value::String).collect(),
                    })?;
                } else {
                    for line in lines {
                        println!("{line}");
                    }
                }
            }
            LogSourceArg::Session => {
                let path = client
                    .target
                    .latest_session_log_path
                    .as_ref()
                    .ok_or_else(|| anyhow!("Desktop control manifest does not include latest session log path"))?;
                let lines = tail_file_lines(path, tail)?;
                if json_output {
                    let parsed = lines
                        .iter()
                        .map(|line| serde_json::from_str::<Value>(line).unwrap_or(Value::String(line.clone())))
                        .collect::<Vec<_>>();
                    print_json(&LogEnvelope {
                        source: "session".to_string(),
                        path: Some(path.display().to_string()),
                        lines: parsed,
                    })?;
                } else {
                    println!("session log: {}", path.display());
                    for line in lines {
                        println!("{line}");
                    }
                }
            }
        },
        Command::Events {
            after_event_id,
            limit,
            timeout_ms,
        } => {
            let batch = client.events(after_event_id, limit, timeout_ms)?;
            if json_output {
                print_json(&batch)?;
            } else {
                print_event_batch_text(&batch);
            }
        }
        Command::Wait {
            condition,
            timeout_ms,
        } => {
            let snapshot = client.wait_for_condition(condition.into_condition(), timeout_ms)?;
            if json_output {
                print_json(&json!({
                    "condition": condition.as_str(),
                    "snapshot": snapshot,
                }))?;
            } else {
                println!("wait satisfied: {}", condition.as_str());
                print_status_text(&client.target, &snapshot);
            }
        }
    }
    Ok(())
}

impl DesktopControlClient {
    fn new(target: ResolvedTarget) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("build autopilotctl HTTP client")?;
        Ok(Self { http, target })
    }

    fn snapshot(&self) -> Result<DesktopControlSnapshot> {
        self.http
            .get(self.url("/v1/snapshot"))
            .bearer_auth(self.target.auth_token.as_str())
            .send()
            .context("request desktop control snapshot")?
            .error_for_status()
            .context("desktop control snapshot request failed")?
            .json::<DesktopControlSnapshot>()
            .context("decode desktop control snapshot")
    }

    fn action(&self, action: &DesktopControlActionRequest) -> Result<DesktopControlActionResponse> {
        let response = self
            .http
            .post(self.url("/v1/action"))
            .bearer_auth(self.target.auth_token.as_str())
            .json(action)
            .send()
            .context("send desktop control action")?;
        let status = response.status();
        let body = response.text().context("read desktop control action body")?;
        let decoded = serde_json::from_str::<DesktopControlActionResponse>(body.as_str())
            .unwrap_or_else(|_| DesktopControlActionResponse {
                success: false,
                message: body.clone(),
                payload: None,
                snapshot_revision: None,
                state_signature: None,
            });
        if !status.is_success() {
            bail!("{}", decoded.message);
        }
        Ok(decoded)
    }

    fn events(
        &self,
        after_event_id: u64,
        limit: usize,
        timeout_ms: u64,
    ) -> Result<DesktopControlEventBatch> {
        self.http
            .get(self.url("/v1/events"))
            .bearer_auth(self.target.auth_token.as_str())
            .query(&[
                ("after_event_id", after_event_id.to_string()),
                ("limit", limit.max(1).to_string()),
                ("timeout_ms", timeout_ms.to_string()),
            ])
            .send()
            .context("request desktop control event batch")?
            .error_for_status()
            .context("desktop control event request failed")?
            .json::<DesktopControlEventBatch>()
            .context("decode desktop control event batch")
    }

    fn mission_control_log_tail(&self, tail: usize) -> Result<Vec<String>> {
        let response = self.action(&DesktopControlActionRequest::GetMissionControlLogTail {
            limit: tail.max(1),
        })?;
        ensure_action_success(&response)?;
        let Some(payload) = response.payload else {
            return Ok(Vec::new());
        };
        Ok(payload
            .get("lines")
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default())
    }

    fn wait_for_condition(
        &self,
        condition: WaitCondition,
        timeout_ms: u64,
    ) -> Result<DesktopControlSnapshot> {
        let started = Instant::now();
        let mut snapshot = self.snapshot()?;
        if condition.matches(&snapshot) {
            return Ok(snapshot);
        }
        let mut after_event_id = 0_u64;
        loop {
            let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            if elapsed_ms >= timeout_ms {
                bail!("Timed out waiting for {}", condition.label());
            }
            let remaining_ms = timeout_ms.saturating_sub(elapsed_ms);
            let batch = self.events(
                after_event_id,
                DEFAULT_EVENTS_LIMIT,
                remaining_ms.min(DEFAULT_WAIT_TIMEOUT_MS),
            )?;
            after_event_id = after_event_id.max(batch.last_event_id);
            snapshot = self.snapshot()?;
            if condition.matches(&snapshot) {
                return Ok(snapshot);
            }
        }
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}{}",
            self.target.base_url.trim_end_matches('/'),
            path
        )
    }
}

impl WaitConditionArg {
    fn into_condition(self) -> WaitCondition {
        match self {
            Self::ProviderOnline => WaitCondition::ProviderOnline,
            Self::ProviderOffline => WaitCondition::ProviderOffline,
            Self::AppleFmReady => WaitCondition::AppleFmReady,
            Self::BuyModeRunning => WaitCondition::BuyModeRunning,
            Self::BuyModeStopped => WaitCondition::BuyModeStopped,
            Self::BuyModeInFlight => WaitCondition::BuyModeInFlight,
            Self::BuyModePaymentRequired => WaitCondition::BuyModePaymentRequired,
            Self::ActiveJobPresent => WaitCondition::ActiveJobPresent,
            Self::ActiveJobRunning => WaitCondition::ActiveJobRunning,
            Self::ActiveJobDelivered => WaitCondition::ActiveJobDelivered,
            Self::ActiveJobSettling => WaitCondition::ActiveJobSettling,
            Self::ActiveJobPaid => WaitCondition::ActiveJobPaid,
            Self::ActiveJobFailed => WaitCondition::ActiveJobFailed,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ProviderOnline => "provider-online",
            Self::ProviderOffline => "provider-offline",
            Self::AppleFmReady => "apple-fm-ready",
            Self::BuyModeRunning => "buy-mode-running",
            Self::BuyModeStopped => "buy-mode-stopped",
            Self::BuyModeInFlight => "buy-mode-in-flight",
            Self::BuyModePaymentRequired => "buy-mode-payment-required",
            Self::ActiveJobPresent => "active-job-present",
            Self::ActiveJobRunning => "active-job-running",
            Self::ActiveJobDelivered => "active-job-delivered",
            Self::ActiveJobSettling => "active-job-settling",
            Self::ActiveJobPaid => "active-job-paid",
            Self::ActiveJobFailed => "active-job-failed",
        }
    }
}

impl WaitCondition {
    fn label(self) -> &'static str {
        match self {
            Self::ProviderOnline => "provider online",
            Self::ProviderOffline => "provider offline",
            Self::AppleFmReady => "Apple FM ready",
            Self::BuyModeRunning => "buy mode running",
            Self::BuyModeStopped => "buy mode stopped",
            Self::BuyModeInFlight => "buy mode in flight",
            Self::BuyModePaymentRequired => "buy mode payment-required",
            Self::ActiveJobPresent => "active job present",
            Self::ActiveJobRunning => "active job running",
            Self::ActiveJobDelivered => "active job delivered",
            Self::ActiveJobSettling => "active job settling",
            Self::ActiveJobPaid => "active job paid",
            Self::ActiveJobFailed => "active job failed",
        }
    }

    fn matches(self, snapshot: &DesktopControlSnapshot) -> bool {
        match self {
            Self::ProviderOnline => snapshot.provider.online,
            Self::ProviderOffline => !snapshot.provider.online,
            Self::AppleFmReady => snapshot.apple_fm.ready,
            Self::BuyModeRunning => snapshot.buy_mode.enabled,
            Self::BuyModeStopped => !snapshot.buy_mode.enabled,
            Self::BuyModeInFlight => snapshot.buy_mode.in_flight_request_id.is_some(),
            Self::BuyModePaymentRequired => snapshot
                .buy_mode
                .recent_requests
                .iter()
                .any(request_has_payment_required),
            Self::ActiveJobPresent => snapshot.active_job.is_some(),
            Self::ActiveJobRunning => snapshot
                .active_job
                .as_ref()
                .is_some_and(|job| job.stage == "running"),
            Self::ActiveJobDelivered => snapshot
                .active_job
                .as_ref()
                .is_some_and(|job| job.stage == "delivered"),
            Self::ActiveJobSettling => snapshot
                .active_job
                .as_ref()
                .is_some_and(|job| job.stage == "settling"),
            Self::ActiveJobPaid => snapshot.active_job.as_ref().is_some_and(|job| {
                job.stage == "paid" || job.settlement_status.as_deref() == Some("paid")
            }),
            Self::ActiveJobFailed => snapshot
                .active_job
                .as_ref()
                .is_some_and(|job| job.stage == "failed"),
        }
    }
}

fn request_has_payment_required(request: &DesktopControlBuyModeRequestStatus) -> bool {
    request.last_feedback_status.as_deref() == Some("payment-required")
        || request.status.eq_ignore_ascii_case("payment-required")
        || request.phase.eq_ignore_ascii_case("requesting-payment")
        || request.phase.eq_ignore_ascii_case("awaiting-payment")
}

fn resolve_target(cli: &Cli) -> Result<ResolvedTarget> {
    match (cli.base_url.as_ref(), cli.auth_token.as_ref()) {
        (Some(base_url), Some(auth_token)) => Ok(ResolvedTarget {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            auth_token: auth_token.trim().to_string(),
            manifest_path: cli.manifest.clone(),
            latest_session_log_path: cli
                .manifest
                .as_ref()
                .and_then(|path| load_manifest_from_path(path).ok())
                .map(|manifest| PathBuf::from(manifest.latest_session_log_path)),
        }),
        (Some(_), None) | (None, Some(_)) => bail!(
            "--base-url and --auth-token must be provided together, or neither to use the manifest"
        ),
        (None, None) => {
            let manifest_path = cli.manifest.clone().unwrap_or_else(control_manifest_path);
            let manifest = load_manifest_from_path(manifest_path.as_path())?;
            Ok(ResolvedTarget {
                base_url: manifest.base_url.trim().trim_end_matches('/').to_string(),
                auth_token: manifest.auth_token,
                manifest_path: Some(manifest_path),
                latest_session_log_path: Some(PathBuf::from(manifest.latest_session_log_path)),
            })
        }
    }
}

fn load_manifest_from_path(path: &Path) -> Result<DesktopControlManifest> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read desktop control manifest {}", path.display()))?;
    serde_json::from_str::<DesktopControlManifest>(raw.as_str())
        .with_context(|| format!("decode desktop control manifest {}", path.display()))
}

fn ensure_action_success(response: &DesktopControlActionResponse) -> Result<()> {
    if response.success {
        Ok(())
    } else {
        bail!("{}", response.message);
    }
}

fn ensure_buy_mode_budget_ack(
    snapshot: &DesktopControlSnapshot,
    approved_budget_sats: u64,
) -> Result<()> {
    let actual = snapshot.buy_mode.approved_budget_sats;
    if actual != approved_budget_sats {
        bail!(
            "Approved budget mismatch: CLI requested {} sats but Mission Control requires {} sats",
            approved_budget_sats,
            actual
        );
    }
    Ok(())
}

fn tail_file_lines(path: &Path, tail: usize) -> Result<Vec<String>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read session log {}", path.display()))?;
    let mut lines = raw.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.len() > tail {
        lines.drain(0..lines.len().saturating_sub(tail));
    }
    Ok(lines)
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("serialize autopilotctl JSON output")?
    );
    Ok(())
}

fn print_action(
    json_output: bool,
    response: &DesktopControlActionResponse,
    snapshot: Option<&DesktopControlSnapshot>,
) -> Result<()> {
    if json_output {
        print_json(&ActionEnvelope { response, snapshot })
    } else {
        println!("{}", response.message);
        if let Some(snapshot) = snapshot {
            print_status_text(
                &ResolvedTarget {
                    base_url: String::new(),
                    auth_token: String::new(),
                    manifest_path: None,
                    latest_session_log_path: None,
                },
                snapshot,
            );
        }
        Ok(())
    }
}

fn print_status_text(target: &ResolvedTarget, snapshot: &DesktopControlSnapshot) {
    if !target.base_url.is_empty() {
        println!("desktop control: {}", target.base_url);
    }
    if let Some(path) = target.manifest_path.as_ref() {
        println!("manifest: {}", path.display());
    }
    if let Some(path) = target.latest_session_log_path.as_ref() {
        println!("latest session log: {}", path.display());
    }
    println!(
        "provider: mode={} runtime={} online={} relays={} blockers={}",
        snapshot.provider.mode,
        snapshot.provider.runtime_mode,
        snapshot.provider.online,
        snapshot.provider.connected_relays,
        snapshot.provider.blocker_codes.len()
    );
    println!(
        "apple fm: ready={} reachable={} model={}",
        snapshot.apple_fm.ready,
        snapshot.apple_fm.reachable,
        snapshot
            .apple_fm
            .ready_model
            .as_deref()
            .unwrap_or("-")
    );
    println!(
        "wallet: balance={} sats network={} status={} withdraw_ready={}",
        snapshot.wallet.balance_sats,
        snapshot.wallet.network,
        snapshot.wallet.network_status,
        snapshot.wallet.can_withdraw
    );
    println!(
        "buy mode: enabled={} approved_budget={} sats cadence={}s next_dispatch={}s",
        snapshot.buy_mode.enabled,
        snapshot.buy_mode.approved_budget_sats,
        snapshot.buy_mode.cadence_seconds,
        snapshot.buy_mode.next_dispatch_countdown_seconds.unwrap_or(0)
    );
    if let Some(request_id) = snapshot.buy_mode.in_flight_request_id.as_deref() {
        println!(
            "buy mode in-flight: request={} phase={} status={} selected_provider={} payable_provider={}",
            request_id,
            snapshot.buy_mode.in_flight_phase.as_deref().unwrap_or("-"),
            snapshot.buy_mode.in_flight_status.as_deref().unwrap_or("-"),
            snapshot
                .buy_mode
                .selected_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            snapshot
                .buy_mode
                .payable_provider_pubkey
                .as_deref()
                .unwrap_or("-")
        );
    }
    print_active_job_text(snapshot.active_job.as_ref());
}

fn print_buy_mode_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "buy mode: enabled={} approved_budget={} sats cadence={}s next_dispatch={}s",
        snapshot.buy_mode.enabled,
        snapshot.buy_mode.approved_budget_sats,
        snapshot.buy_mode.cadence_seconds,
        snapshot.buy_mode.next_dispatch_countdown_seconds.unwrap_or(0)
    );
    if let Some(request_id) = snapshot.buy_mode.in_flight_request_id.as_deref() {
        println!(
            "in-flight: request={} phase={} status={} selected_provider={} payable_provider={}",
            request_id,
            snapshot.buy_mode.in_flight_phase.as_deref().unwrap_or("-"),
            snapshot.buy_mode.in_flight_status.as_deref().unwrap_or("-"),
            snapshot
                .buy_mode
                .selected_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            snapshot
                .buy_mode
                .payable_provider_pubkey
                .as_deref()
                .unwrap_or("-")
        );
    }
    for request in snapshot.buy_mode.recent_requests.iter().take(6) {
        println!(
            "request={} status={} phase={} next={} payment_pointer={} payment_error={}",
            request.request_id,
            request.status,
            request.phase,
            request.next_expected_event,
            request.payment_pointer.as_deref().unwrap_or("-"),
            request.payment_error.as_deref().unwrap_or("-")
        );
    }
}

fn print_active_job_text(active_job: Option<&DesktopControlActiveJobStatus>) {
    match active_job {
        Some(active_job) => {
            println!(
                "active job: request={} capability={} stage={} next={} settlement={} payment_pointer={} fees={}",
                active_job.request_id,
                active_job.capability,
                active_job.stage,
                active_job.next_expected_event,
                active_job.settlement_status.as_deref().unwrap_or("-"),
                active_job.payment_pointer.as_deref().unwrap_or("-"),
                active_job
                    .settlement_fees_sats
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
        }
        None => println!("active job: none"),
    }
}

fn print_event_batch_text(batch: &DesktopControlEventBatch) {
    println!(
        "event stream: last_event_id={} timed_out={}",
        batch.last_event_id, batch.timed_out
    );
    for event in &batch.events {
        println!(
            "#{} {} {}",
            event.event_id, event.event_type, event.summary
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{WaitCondition, ensure_buy_mode_budget_ack, request_has_payment_required};
    use autopilot_desktop::desktop_control::{
        DesktopControlBuyModeRequestStatus, DesktopControlBuyModeStatus, DesktopControlSnapshot,
    };

    fn sample_snapshot() -> DesktopControlSnapshot {
        DesktopControlSnapshot {
            buy_mode: DesktopControlBuyModeStatus {
                approved_budget_sats: 2,
                ..DesktopControlBuyModeStatus::default()
            },
            ..DesktopControlSnapshot::default()
        }
    }

    #[test]
    fn buy_mode_budget_ack_rejects_mismatch() {
        let snapshot = sample_snapshot();
        let error = ensure_buy_mode_budget_ack(&snapshot, 5).expect_err("budget mismatch");
        assert!(error.to_string().contains("Approved budget mismatch"));
    }

    #[test]
    fn wait_condition_matches_buy_mode_payment_required() {
        let mut snapshot = sample_snapshot();
        snapshot.buy_mode.recent_requests.push(DesktopControlBuyModeRequestStatus {
            request_id: "req-1".to_string(),
            status: "streaming".to_string(),
            phase: "awaiting-payment".to_string(),
            next_expected_event: "invoice".to_string(),
            last_feedback_status: Some("payment-required".to_string()),
            ..DesktopControlBuyModeRequestStatus::default()
        });
        assert!(request_has_payment_required(&snapshot.buy_mode.recent_requests[0]));
        assert!(WaitCondition::BuyModePaymentRequired.matches(&snapshot));
    }
}
