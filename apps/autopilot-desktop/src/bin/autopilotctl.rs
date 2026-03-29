#![allow(
    clippy::print_stdout,
    reason = "CLI intentionally prints structured operator and agent-facing output."
)]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use autopilot_desktop::desktop_control::{
    DesktopControlActionRequest, DesktopControlActionResponse, DesktopControlActiveJobStatus,
    DesktopControlAppleAdapterOperatorRunStatus, DesktopControlAttnResStatus,
    DesktopControlAttnResView, DesktopControlBuyModeRequestStatus, DesktopControlBuyModeStatus,
    DesktopControlDataMarketBuyerRequestArgs, DesktopControlDataMarketDraftAssetArgs,
    DesktopControlDataMarketDraftGrantArgs, DesktopControlDataMarketImportBuyerResponseArgs,
    DesktopControlDataMarketImportSellerRequestArgs, DesktopControlDataMarketIssueDeliveryArgs,
    DesktopControlDataMarketPrepareDeliveryArgs, DesktopControlDataMarketPublishArgs,
    DesktopControlDataMarketRequestPaymentArgs, DesktopControlDataMarketResolveDeliveryArgs,
    DesktopControlDataMarketRevokeGrantArgs, DesktopControlEventBatch,
    DesktopControlLocalRuntimeStatus, DesktopControlManifest,
    DesktopControlNip90SentPaymentsReport, DesktopControlSnapshot,
    DesktopControlTassadarReplayFamily, DesktopControlTassadarSourceMode,
    DesktopControlTassadarStatus, DesktopControlTassadarView, control_manifest_path,
};
use autopilot_desktop::{
    compile_path_temperature_label, local_runtime_cache_invalidation_reason_label,
    local_runtime_device_inventory_label, local_runtime_execution_posture_label,
    local_runtime_scheduler_posture_label,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bitcoin::hashes::{Hash, sha256};
use chrono::{DateTime, Datelike, Local, LocalResult, NaiveDate, TimeZone};
use clap::{Parser, Subcommand, ValueEnum};
use nostr_client::{RelayConnection, RelayMessage};
use psionic_sandbox::ProviderSandboxEntrypointType;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const DEFAULT_EVENTS_LIMIT: usize = 64;
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 20_000;
const DEFAULT_TRAINING_WATCH_POLL_MS: u64 = 1_000;
const DEFAULT_TRAINING_WATCH_TIMEOUT_MS: u64 = 30 * 60 * 1_000;
const DEFAULT_APPLE_FM_BASE_URL: &str = "http://127.0.0.1:11435";

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
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
enum Command {
    Status,
    Perf,
    #[command(name = "attnres")]
    AttnRes {
        #[command(subcommand)]
        command: AttnResCommand,
    },
    #[command(name = "tassadar")]
    Tassadar {
        #[command(subcommand)]
        command: TassadarCommand,
    },
    Cluster {
        #[command(subcommand)]
        command: ClusterCommand,
    },
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommand,
    },
    Proof {
        #[command(subcommand)]
        command: ProofCommand,
    },
    Challenge {
        #[command(subcommand)]
        command: ChallengeCommand,
    },
    Training {
        #[command(subcommand)]
        command: TrainingCommand,
    },
    #[command(name = "remote-training")]
    RemoteTraining {
        #[command(subcommand)]
        command: RemoteTrainingCommand,
    },
    Research {
        #[command(subcommand)]
        command: ResearchCommand,
    },
    Pane {
        #[command(subcommand)]
        command: PaneCommand,
    },
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    LocalRuntime {
        #[command(subcommand)]
        command: LocalRuntimeCommand,
    },
    GptOss {
        #[command(subcommand)]
        command: GptOssCommand,
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
    Nip90Payments {
        #[command(subcommand)]
        command: Nip90PaymentsCommand,
    },
    Tunnels {
        #[command(subcommand)]
        command: TunnelCommand,
    },
    Tailnet {
        #[command(subcommand)]
        command: TailnetCommand,
    },
    Chat {
        #[command(subcommand)]
        command: ChatCommand,
    },
    DataMarket {
        #[command(subcommand)]
        command: DataMarketCommand,
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
enum PaneCommand {
    List,
    Open {
        pane: String,
    },
    Focus {
        pane: String,
    },
    Close {
        pane: String,
    },
    Status {
        pane: String,
    },
    Capture {
        pane: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        #[arg(long)]
        allow_fallback_adapter: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AttnResCommand {
    Status,
    Start,
    Pause,
    Reset,
    Refresh,
    View {
        #[arg(value_enum)]
        view: AttnResViewArg,
    },
    Sublayer {
        #[command(subcommand)]
        command: AttnResSublayerCommand,
    },
    Speed {
        #[command(subcommand)]
        command: AttnResSpeedCommand,
    },
}

#[derive(Subcommand, Debug)]
enum TassadarCommand {
    Status,
    Play,
    Pause,
    Reset,
    Refresh,
    View {
        #[arg(value_enum)]
        view: TassadarViewArg,
    },
    Source {
        #[arg(value_enum)]
        source: TassadarSourceArg,
    },
    Family {
        #[command(subcommand)]
        command: TassadarFamilyCommand,
    },
    Case {
        #[command(subcommand)]
        command: TassadarNavigationCommand,
    },
    Update {
        #[command(subcommand)]
        command: TassadarNavigationCommand,
    },
    ReadableLog {
        #[command(subcommand)]
        command: TassadarNavigationCommand,
    },
    Token {
        #[command(subcommand)]
        command: TassadarNavigationCommand,
    },
    Fact {
        #[command(subcommand)]
        command: TassadarNavigationCommand,
    },
    Speed {
        #[command(subcommand)]
        command: TassadarSpeedCommand,
    },
    Window {
        #[command(subcommand)]
        command: TassadarWindowCommand,
    },
}

#[derive(Subcommand, Debug)]
enum TassadarNavigationCommand {
    Next,
    Prev,
}

#[derive(Subcommand, Debug)]
enum TassadarFamilyCommand {
    Set {
        #[arg(value_enum)]
        family: TassadarReplayFamilyArg,
    },
    Next,
    Prev,
}

#[derive(Subcommand, Debug)]
enum TassadarSpeedCommand {
    Set { speed_multiplier: usize },
    Increase,
    Decrease,
}

#[derive(Subcommand, Debug)]
enum TassadarWindowCommand {
    Increase,
    Decrease,
}

#[derive(Subcommand, Debug)]
enum AttnResSublayerCommand {
    Set { index: usize },
    Next,
    Prev,
}

#[derive(Subcommand, Debug)]
enum AttnResSpeedCommand {
    Set { speed_multiplier: usize },
    Increase,
    Decrease,
}

#[derive(Subcommand, Debug)]
enum ClusterCommand {
    Status,
    Topology,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SandboxEntrypointTypeArg {
    WorkspaceFile,
    InlinePayload,
    Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
enum SandboxCommand {
    Status,
    Job {
        job_id: String,
    },
    Create {
        profile_id: String,
        job_id: String,
        workspace_root: PathBuf,
        #[arg(long, value_enum, default_value_t = SandboxEntrypointTypeArg::WorkspaceFile)]
        entrypoint_type: SandboxEntrypointTypeArg,
        #[arg(long)]
        entrypoint: String,
        #[arg(long)]
        payload: Option<String>,
        #[arg(long = "arg")]
        arguments: Vec<String>,
        #[arg(long = "expected-output")]
        expected_outputs: Vec<String>,
        #[arg(long, default_value_t = 60)]
        timeout_s: u64,
        #[arg(long, default_value = "host_inherit")]
        network: String,
        #[arg(long, default_value = "host_inherit")]
        filesystem: String,
        #[arg(long)]
        payout_reference: Option<String>,
        #[arg(long)]
        verification_posture: Option<String>,
    },
    Upload {
        job_id: String,
        relative_path: String,
        file: PathBuf,
    },
    Start {
        job_id: String,
    },
    Wait {
        job_id: String,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    DownloadArtifact {
        job_id: String,
        relative_path: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    DownloadWorkspace {
        job_id: String,
        relative_path: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum ProofCommand {
    Status,
}

#[derive(Subcommand, Debug)]
enum ChallengeCommand {
    Status,
}

#[derive(Subcommand, Debug)]
enum TrainingCommand {
    Status,
    Watch {
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_TRAINING_WATCH_POLL_MS)]
        poll_ms: u64,
        #[arg(long, default_value_t = DEFAULT_TRAINING_WATCH_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Launch {
        train_dataset_path: PathBuf,
        held_out_dataset_path: PathBuf,
        package_name: String,
        #[arg(long, default_value = "")]
        author: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value = "")]
        license: String,
        #[arg(long, default_value = DEFAULT_APPLE_FM_BASE_URL)]
        apple_fm_base_url: String,
        #[arg(long)]
        experiment_manifest_path: Option<PathBuf>,
        #[arg(long)]
        training_policy_override_path: Option<PathBuf>,
    },
    Export {
        run_id: String,
        export_path: PathBuf,
    },
    Accept {
        run_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum RemoteTrainingCommand {
    Status,
    List,
    Run { run_id: Option<String> },
    Stale,
    Refresh,
}

#[derive(Subcommand, Debug)]
enum ResearchCommand {
    Status,
    Reset,
}

#[derive(Subcommand, Debug)]
enum LocalRuntimeCommand {
    Status,
    Refresh {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
}

#[derive(Subcommand, Debug)]
enum GptOssCommand {
    Status,
    Refresh {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Warm {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Unload {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
}

#[derive(Subcommand, Debug)]
enum AppleFmCommand {
    Status,
    List,
    Refresh {
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Load {
        package_path: PathBuf,
        #[arg(long)]
        adapter_id: Option<String>,
    },
    Unload {
        adapter_id: String,
    },
    Attach {
        session_id: String,
        adapter_id: String,
    },
    Detach {
        session_id: String,
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
    Target,
    Roster {
        #[arg(long, default_value_t = 12)]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
enum Nip90PaymentsCommand {
    Daily {
        #[arg(long)]
        date: String,
    },
    Window {
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: String,
    },
}

#[derive(Subcommand, Debug)]
enum TunnelCommand {
    Status,
}

#[derive(Subcommand, Debug)]
enum TailnetCommand {
    Status,
}

#[derive(Subcommand, Debug)]
enum ChatCommand {
    Status,
    Main,
    Groups,
    Channels,
    Tail {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    SelectGroup {
        group_id: String,
    },
    SelectChannel {
        channel_id: String,
    },
    Send {
        content: String,
        #[arg(long)]
        reply_to_event_id: Option<String>,
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    Retry {
        event_id: String,
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_MS)]
        timeout_ms: u64,
    },
    CreateChannel {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "OpenAgents team test channel")]
        about: String,
    },
}

#[derive(Subcommand, Debug)]
enum DataMarketCommand {
    SellerStatus,
    SellerPrompt {
        prompt: String,
    },
    BuyerStatus,
    BuyerRefresh,
    Snapshot,
    DraftAsset {
        #[arg(long)]
        file: PathBuf,
    },
    PreviewAsset,
    PublishAsset {
        #[arg(long)]
        confirm: bool,
    },
    DraftGrant {
        #[arg(long)]
        file: PathBuf,
    },
    PreviewGrant,
    PublishGrant {
        #[arg(long)]
        confirm: bool,
    },
    RequestPayment {
        #[arg(long)]
        request_id: String,
    },
    PrepareDelivery {
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        file: PathBuf,
    },
    IssueDelivery {
        #[arg(long)]
        request_id: String,
    },
    RevokeGrant {
        #[arg(long)]
        request_id: String,
        #[arg(long, value_enum)]
        action: DataMarketRevocationActionArg,
        #[arg(long)]
        confirm: bool,
        #[arg(long)]
        reason_code: Option<String>,
    },
    BuyerPublishRequest {
        #[arg(long)]
        asset_id: Option<String>,
        #[arg(long, default_value_t = true)]
        refresh_market: bool,
    },
    SellerImportRequest {
        #[arg(long)]
        event_id: String,
        #[arg(long = "relay-url")]
        relay_urls: Vec<String>,
        #[arg(long, default_value_t = 15_000)]
        timeout_ms: u64,
    },
    BuyerImportResponse {
        #[arg(long)]
        event_id: String,
        #[arg(long = "relay-url")]
        relay_urls: Vec<String>,
        #[arg(long, default_value_t = 15_000)]
        timeout_ms: u64,
    },
    ConsumeDelivery {
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        delivery_bundle_id: Option<String>,
        #[arg(long)]
        request_id: Option<String>,
        #[arg(long)]
        grant_id: Option<String>,
        #[arg(long)]
        asset_id: Option<String>,
        #[arg(long, default_value_t = true)]
        refresh_market: bool,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum DataMarketRevocationActionArg {
    Revoke,
    Expire,
}

#[derive(serde::Deserialize)]
struct DataMarketPrepareDeliveryFileArgs {
    preview_text: Option<String>,
    delivery_ref: Option<String>,
    delivery_digest: Option<String>,
    manifest_refs: Option<Vec<String>>,
    bundle_size_bytes: Option<u64>,
    expires_in_hours: Option<u64>,
}

impl ProviderCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Online { .. } => DesktopControlActionRequest::SetProviderMode { online: true },
            Self::Offline { .. } => DesktopControlActionRequest::SetProviderMode { online: false },
        }
    }
}

impl PaneCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::List => DesktopControlActionRequest::ListPanes,
            Self::Open { pane } => DesktopControlActionRequest::OpenPane { pane: pane.clone() },
            Self::Focus { pane } => DesktopControlActionRequest::FocusPane { pane: pane.clone() },
            Self::Close { pane } => DesktopControlActionRequest::ClosePane { pane: pane.clone() },
            Self::Status { pane } => {
                DesktopControlActionRequest::GetPaneSnapshot { pane: pane.clone() }
            }
            Self::Capture {
                pane,
                output,
                width,
                height,
                scale,
                allow_fallback_adapter,
            } => DesktopControlActionRequest::CapturePane {
                pane: pane.clone(),
                output_path: output.display().to_string(),
                width: *width,
                height: *height,
                scale_factor_milli: scale_factor_milli(*scale),
                allow_fallback_adapter: *allow_fallback_adapter,
            },
        }
    }
}

fn scale_factor_milli(scale_factor: f32) -> u32 {
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return 0;
    }
    (scale_factor * 1000.0).round().clamp(1.0, u32::MAX as f32) as u32
}

impl AttnResViewArg {
    const fn into_request_view(self) -> DesktopControlAttnResView {
        match self {
            Self::Overview => DesktopControlAttnResView::Overview,
            Self::Pipeline => DesktopControlAttnResView::Pipeline,
            Self::Inference => DesktopControlAttnResView::Inference,
            Self::Loss => DesktopControlAttnResView::Loss,
        }
    }
}

impl AttnResCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status => DesktopControlActionRequest::GetAttnResStatus,
            Self::Start => DesktopControlActionRequest::StartAttnRes,
            Self::Pause => DesktopControlActionRequest::PauseAttnRes,
            Self::Reset => DesktopControlActionRequest::ResetAttnRes,
            Self::Refresh => DesktopControlActionRequest::RefreshAttnRes,
            Self::View { view } => DesktopControlActionRequest::SetAttnResView {
                view: view.into_request_view(),
            },
            Self::Sublayer { command } => match command {
                AttnResSublayerCommand::Set { index } => {
                    DesktopControlActionRequest::SetAttnResSublayer { index: *index }
                }
                AttnResSublayerCommand::Next => DesktopControlActionRequest::NextAttnResSublayer,
                AttnResSublayerCommand::Prev => {
                    DesktopControlActionRequest::PreviousAttnResSublayer
                }
            },
            Self::Speed { command } => match command {
                AttnResSpeedCommand::Set { speed_multiplier } => {
                    DesktopControlActionRequest::SetAttnResSpeed {
                        speed_multiplier: *speed_multiplier,
                    }
                }
                AttnResSpeedCommand::Increase => DesktopControlActionRequest::IncreaseAttnResSpeed,
                AttnResSpeedCommand::Decrease => DesktopControlActionRequest::DecreaseAttnResSpeed,
            },
        }
    }
}

impl TassadarViewArg {
    const fn into_request_view(self) -> DesktopControlTassadarView {
        match self {
            Self::Overview => DesktopControlTassadarView::Overview,
            Self::Trace => DesktopControlTassadarView::Trace,
            Self::Program => DesktopControlTassadarView::Program,
            Self::Evidence => DesktopControlTassadarView::Evidence,
        }
    }
}

impl TassadarSourceArg {
    const fn into_request_source(self) -> DesktopControlTassadarSourceMode {
        match self {
            Self::Replay => DesktopControlTassadarSourceMode::Replay,
            Self::ArticleSession => DesktopControlTassadarSourceMode::ArticleSession,
            Self::HybridWorkflow => DesktopControlTassadarSourceMode::HybridWorkflow,
        }
    }
}

impl TassadarReplayFamilyArg {
    const fn into_request_family(self) -> DesktopControlTassadarReplayFamily {
        match self {
            Self::ArticleSessions => DesktopControlTassadarReplayFamily::ArticleSessions,
            Self::HybridWorkflows => DesktopControlTassadarReplayFamily::HybridWorkflows,
            Self::CompiledClosure => DesktopControlTassadarReplayFamily::CompiledClosure,
            Self::Acceptance => DesktopControlTassadarReplayFamily::Acceptance,
            Self::LearnedPromotion => DesktopControlTassadarReplayFamily::LearnedPromotion,
            Self::Learned9x9Fit => DesktopControlTassadarReplayFamily::Learned9x9Fit,
            Self::LearnedHorizon => DesktopControlTassadarReplayFamily::LearnedHorizon,
            Self::ArchitectureComparison => {
                DesktopControlTassadarReplayFamily::ArchitectureComparison
            }
        }
    }
}

impl TassadarCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status => DesktopControlActionRequest::GetTassadarStatus,
            Self::Play => DesktopControlActionRequest::ToggleTassadarPlayback,
            Self::Pause => DesktopControlActionRequest::PauseTassadarPlayback,
            Self::Reset => DesktopControlActionRequest::ResetTassadarPlayback,
            Self::Refresh => DesktopControlActionRequest::RefreshTassadar,
            Self::View { view } => DesktopControlActionRequest::SetTassadarView {
                view: view.into_request_view(),
            },
            Self::Source { source } => DesktopControlActionRequest::SetTassadarSourceMode {
                source_mode: source.into_request_source(),
            },
            Self::Family { command } => match command {
                TassadarFamilyCommand::Set { family } => {
                    DesktopControlActionRequest::SetTassadarReplayFamily {
                        family: family.into_request_family(),
                    }
                }
                TassadarFamilyCommand::Next => {
                    DesktopControlActionRequest::NextTassadarReplayFamily
                }
                TassadarFamilyCommand::Prev => {
                    DesktopControlActionRequest::PreviousTassadarReplayFamily
                }
            },
            Self::Case { command } => match command {
                TassadarNavigationCommand::Next => DesktopControlActionRequest::NextTassadarCase,
                TassadarNavigationCommand::Prev => {
                    DesktopControlActionRequest::PreviousTassadarCase
                }
            },
            Self::Update { command } => match command {
                TassadarNavigationCommand::Next => DesktopControlActionRequest::NextTassadarUpdate,
                TassadarNavigationCommand::Prev => {
                    DesktopControlActionRequest::PreviousTassadarUpdate
                }
            },
            Self::ReadableLog { command } => match command {
                TassadarNavigationCommand::Next => {
                    DesktopControlActionRequest::NextTassadarReadableLogLine
                }
                TassadarNavigationCommand::Prev => {
                    DesktopControlActionRequest::PreviousTassadarReadableLogLine
                }
            },
            Self::Token { command } => match command {
                TassadarNavigationCommand::Next => {
                    DesktopControlActionRequest::NextTassadarTokenChunk
                }
                TassadarNavigationCommand::Prev => {
                    DesktopControlActionRequest::PreviousTassadarTokenChunk
                }
            },
            Self::Fact { command } => match command {
                TassadarNavigationCommand::Next => {
                    DesktopControlActionRequest::NextTassadarFactLine
                }
                TassadarNavigationCommand::Prev => {
                    DesktopControlActionRequest::PreviousTassadarFactLine
                }
            },
            Self::Speed { command } => match command {
                TassadarSpeedCommand::Set { speed_multiplier } => {
                    DesktopControlActionRequest::SetTassadarSpeed {
                        speed_multiplier: *speed_multiplier,
                    }
                }
                TassadarSpeedCommand::Increase => {
                    DesktopControlActionRequest::IncreaseTassadarSpeed
                }
                TassadarSpeedCommand::Decrease => {
                    DesktopControlActionRequest::DecreaseTassadarSpeed
                }
            },
            Self::Window { command } => match command {
                TassadarWindowCommand::Increase => {
                    DesktopControlActionRequest::IncreaseTassadarTraceWindow
                }
                TassadarWindowCommand::Decrease => {
                    DesktopControlActionRequest::DecreaseTassadarTraceWindow
                }
            },
        }
    }
}

impl SandboxEntrypointTypeArg {
    const fn into_request_type(self) -> ProviderSandboxEntrypointType {
        match self {
            Self::WorkspaceFile => ProviderSandboxEntrypointType::WorkspaceFile,
            Self::InlinePayload => ProviderSandboxEntrypointType::InlinePayload,
            Self::Command => ProviderSandboxEntrypointType::Command,
        }
    }
}

impl ClusterCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status => DesktopControlActionRequest::GetClusterStatus,
            Self::Topology => DesktopControlActionRequest::GetClusterTopology,
        }
    }
}

impl SandboxCommand {
    fn action_request(&self) -> Result<Option<DesktopControlActionRequest>> {
        match self {
            Self::Status => Ok(None),
            Self::Job { job_id } => Ok(Some(DesktopControlActionRequest::GetSandboxJob {
                job_id: job_id.clone(),
            })),
            Self::Create {
                profile_id,
                job_id,
                workspace_root,
                entrypoint_type,
                entrypoint,
                payload,
                arguments,
                expected_outputs,
                timeout_s,
                network,
                filesystem,
                payout_reference,
                verification_posture,
            } => Ok(Some(DesktopControlActionRequest::CreateSandboxJob {
                profile_id: profile_id.clone(),
                job_id: job_id.clone(),
                workspace_root: workspace_root.display().to_string(),
                entrypoint_type: entrypoint_type.into_request_type(),
                entrypoint: entrypoint.clone(),
                payload: payload.clone(),
                arguments: arguments.clone(),
                expected_outputs: expected_outputs.clone(),
                timeout_request_s: *timeout_s,
                network_request: network.clone(),
                filesystem_request: filesystem.clone(),
                payout_reference: payout_reference.clone(),
                verification_posture: verification_posture.clone(),
            })),
            Self::Upload {
                job_id,
                relative_path,
                file,
            } => {
                let bytes = fs::read(file)
                    .with_context(|| format!("read sandbox upload {}", file.display()))?;
                Ok(Some(DesktopControlActionRequest::UploadSandboxFile {
                    job_id: job_id.clone(),
                    relative_path: relative_path.clone(),
                    content_base64: URL_SAFE_NO_PAD.encode(bytes.as_slice()),
                }))
            }
            Self::Start { job_id } => Ok(Some(DesktopControlActionRequest::StartSandboxJob {
                job_id: job_id.clone(),
            })),
            Self::Wait { job_id, timeout_ms } => {
                Ok(Some(DesktopControlActionRequest::WaitSandboxJob {
                    job_id: job_id.clone(),
                    timeout_ms: *timeout_ms,
                }))
            }
            Self::DownloadArtifact {
                job_id,
                relative_path,
                ..
            } => Ok(Some(DesktopControlActionRequest::DownloadSandboxArtifact {
                job_id: job_id.clone(),
                relative_path: relative_path.clone(),
            })),
            Self::DownloadWorkspace {
                job_id,
                relative_path,
                ..
            } => Ok(Some(
                DesktopControlActionRequest::DownloadSandboxWorkspaceFile {
                    job_id: job_id.clone(),
                    relative_path: relative_path.clone(),
                },
            )),
        }
    }
}

impl ProofCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status => DesktopControlActionRequest::GetProofStatus,
        }
    }
}

impl ChallengeCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status => DesktopControlActionRequest::GetChallengeStatus,
        }
    }
}

impl TrainingCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status | Self::Watch { .. } => DesktopControlActionRequest::GetTrainingStatus,
            Self::Launch {
                train_dataset_path,
                held_out_dataset_path,
                package_name,
                author,
                description,
                license,
                apple_fm_base_url,
                experiment_manifest_path,
                training_policy_override_path,
            } => DesktopControlActionRequest::LaunchAppleAdapterTraining {
                train_dataset_path: train_dataset_path.display().to_string(),
                held_out_dataset_path: held_out_dataset_path.display().to_string(),
                package_name: package_name.clone(),
                author: author.clone(),
                description: description.clone(),
                license: license.clone(),
                apple_fm_base_url: apple_fm_base_url.clone(),
                experiment_manifest_path: experiment_manifest_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
                training_policy_override_path: training_policy_override_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
            },
            Self::Export {
                run_id,
                export_path,
            } => DesktopControlActionRequest::ExportAppleAdapterTraining {
                run_id: run_id.clone(),
                export_path: export_path.display().to_string(),
            },
            Self::Accept { run_id } => DesktopControlActionRequest::AcceptAppleAdapterTraining {
                run_id: run_id.clone(),
            },
        }
    }
}

impl RemoteTrainingCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status | Self::List | Self::Stale => {
                DesktopControlActionRequest::GetRemoteTrainingStatus
            }
            Self::Run { run_id } => DesktopControlActionRequest::GetRemoteTrainingRun {
                run_id: run_id.clone(),
            },
            Self::Refresh => DesktopControlActionRequest::RefreshRemoteTrainingStatus,
        }
    }
}

impl ResearchCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Status => DesktopControlActionRequest::GetResearchStatus,
            Self::Reset => DesktopControlActionRequest::ResetResearchState,
        }
    }
}

impl LocalRuntimeCommand {
    fn action_request(&self) -> Option<DesktopControlActionRequest> {
        match self {
            Self::Status => None,
            Self::Refresh { .. } => Some(DesktopControlActionRequest::RefreshLocalRuntime),
        }
    }
}

impl GptOssCommand {
    fn action_request(&self) -> Option<DesktopControlActionRequest> {
        match self {
            Self::Status => None,
            Self::Refresh { .. } => Some(DesktopControlActionRequest::RefreshGptOss),
            Self::Warm { .. } => Some(DesktopControlActionRequest::WarmGptOss),
            Self::Unload { .. } => Some(DesktopControlActionRequest::UnloadGptOss),
        }
    }
}

impl AppleFmCommand {
    fn action_request(&self) -> Option<DesktopControlActionRequest> {
        match self {
            Self::Status | Self::List => None,
            Self::Refresh { .. } => Some(DesktopControlActionRequest::RefreshAppleFm),
            Self::Load {
                package_path,
                adapter_id,
            } => Some(DesktopControlActionRequest::LoadAppleFmAdapter {
                package_path: package_path.display().to_string(),
                requested_adapter_id: adapter_id.clone(),
            }),
            Self::Unload { adapter_id } => {
                Some(DesktopControlActionRequest::UnloadAppleFmAdapter {
                    adapter_id: adapter_id.clone(),
                })
            }
            Self::Attach {
                session_id,
                adapter_id,
            } => Some(DesktopControlActionRequest::AttachAppleFmSessionAdapter {
                session_id: session_id.clone(),
                adapter_id: adapter_id.clone(),
            }),
            Self::Detach { session_id } => {
                Some(DesktopControlActionRequest::DetachAppleFmSessionAdapter {
                    session_id: session_id.clone(),
                })
            }
            Self::SmokeTest => Some(DesktopControlActionRequest::RunAppleFmSmokeTest),
        }
    }
}

impl WalletCommand {
    fn action_request(&self) -> DesktopControlActionRequest {
        match self {
            Self::Refresh => DesktopControlActionRequest::RefreshWallet,
        }
    }
}

impl BuyModeCommand {
    fn action_request(&self) -> Option<DesktopControlActionRequest> {
        match self {
            Self::Start { .. } => Some(DesktopControlActionRequest::StartBuyMode),
            Self::Stop { .. } => Some(DesktopControlActionRequest::StopBuyMode),
            Self::Status | Self::Target | Self::Roster { .. } => None,
        }
    }
}

impl Nip90PaymentsCommand {
    fn action_request(&self) -> Result<DesktopControlActionRequest> {
        match self {
            Self::Daily { date } => {
                let (start_epoch_seconds, end_epoch_seconds) =
                    parse_local_daily_window(date.as_str())?;
                Ok(DesktopControlActionRequest::GetNip90SentPaymentsReport {
                    start_epoch_seconds,
                    end_epoch_seconds,
                    report_date: Some(date.clone()),
                })
            }
            Self::Window { start, end } => {
                let start_epoch_seconds = parse_report_boundary(start.as_str())?;
                let end_epoch_seconds = parse_report_boundary(end.as_str())?;
                if end_epoch_seconds <= start_epoch_seconds {
                    bail!("window end must be greater than start");
                }
                Ok(DesktopControlActionRequest::GetNip90SentPaymentsReport {
                    start_epoch_seconds,
                    end_epoch_seconds,
                    report_date: None,
                })
            }
        }
    }
}

impl ChatCommand {
    fn action_request(&self) -> Option<DesktopControlActionRequest> {
        match self {
            Self::Status | Self::Groups | Self::Channels | Self::Tail { .. } => None,
            Self::Main => Some(DesktopControlActionRequest::SelectNip28MainChannel),
            Self::SelectGroup { group_id } => Some(DesktopControlActionRequest::SelectNip28Group {
                group_id: group_id.clone(),
            }),
            Self::SelectChannel { channel_id } => {
                Some(DesktopControlActionRequest::SelectNip28Channel {
                    channel_id: channel_id.clone(),
                })
            }
            Self::Send {
                content,
                reply_to_event_id,
                ..
            } => Some(DesktopControlActionRequest::SendNip28Message {
                content: content.clone(),
                reply_to_event_id: reply_to_event_id.clone(),
            }),
            Self::Retry { event_id, .. } => Some(DesktopControlActionRequest::RetryNip28Message {
                event_id: event_id.clone(),
            }),
            Self::CreateChannel { name, about } => {
                Some(DesktopControlActionRequest::CreateNip28Channel {
                    name: name.clone(),
                    about: about.clone(),
                })
            }
        }
    }
}

impl DataMarketRevocationActionArg {
    const fn as_request_action(self) -> &'static str {
        match self {
            Self::Revoke => "revoke",
            Self::Expire => "expire",
        }
    }
}

impl DataMarketCommand {
    fn action_request(&self) -> Result<DesktopControlActionRequest> {
        match self {
            Self::SellerStatus => Ok(DesktopControlActionRequest::GetDataMarketSellerStatus),
            Self::SellerPrompt { prompt } => {
                Ok(DesktopControlActionRequest::SendDataMarketSellerPrompt {
                    prompt: prompt.clone(),
                })
            }
            Self::BuyerStatus => Ok(DesktopControlActionRequest::GetDataMarketBuyerStatus),
            Self::BuyerRefresh => Ok(DesktopControlActionRequest::RefreshDataMarketBuyerMarket),
            Self::Snapshot => Ok(DesktopControlActionRequest::GetDataMarketSnapshot),
            Self::DraftAsset { file } => Ok(DesktopControlActionRequest::DraftDataMarketAsset {
                args: load_json_file::<DesktopControlDataMarketDraftAssetArgs>(
                    file,
                    "data market asset draft",
                )?,
            }),
            Self::PreviewAsset => Ok(DesktopControlActionRequest::PreviewDataMarketAsset),
            Self::PublishAsset { confirm } => {
                Ok(DesktopControlActionRequest::PublishDataMarketAsset {
                    args: DesktopControlDataMarketPublishArgs { confirm: *confirm },
                })
            }
            Self::DraftGrant { file } => Ok(DesktopControlActionRequest::DraftDataMarketGrant {
                args: load_json_file::<DesktopControlDataMarketDraftGrantArgs>(
                    file,
                    "data market grant draft",
                )?,
            }),
            Self::PreviewGrant => Ok(DesktopControlActionRequest::PreviewDataMarketGrant),
            Self::PublishGrant { confirm } => {
                Ok(DesktopControlActionRequest::PublishDataMarketGrant {
                    args: DesktopControlDataMarketPublishArgs { confirm: *confirm },
                })
            }
            Self::RequestPayment { request_id } => {
                Ok(DesktopControlActionRequest::RequestDataMarketPayment {
                    args: DesktopControlDataMarketRequestPaymentArgs {
                        request_id: request_id.clone(),
                    },
                })
            }
            Self::PrepareDelivery { request_id, file } => {
                let loaded = load_json_file::<DataMarketPrepareDeliveryFileArgs>(
                    file,
                    "data market delivery draft",
                )?;
                Ok(DesktopControlActionRequest::PrepareDataMarketDelivery {
                    args: DesktopControlDataMarketPrepareDeliveryArgs {
                        request_id: request_id.clone(),
                        preview_text: loaded.preview_text,
                        delivery_ref: loaded.delivery_ref,
                        delivery_digest: loaded.delivery_digest,
                        manifest_refs: loaded.manifest_refs,
                        bundle_size_bytes: loaded.bundle_size_bytes,
                        expires_in_hours: loaded.expires_in_hours,
                    },
                })
            }
            Self::IssueDelivery { request_id } => {
                Ok(DesktopControlActionRequest::IssueDataMarketDelivery {
                    args: DesktopControlDataMarketIssueDeliveryArgs {
                        request_id: request_id.clone(),
                    },
                })
            }
            Self::RevokeGrant {
                request_id,
                action,
                confirm,
                reason_code,
            } => Ok(DesktopControlActionRequest::RevokeDataMarketGrant {
                args: DesktopControlDataMarketRevokeGrantArgs {
                    request_id: request_id.clone(),
                    action: action.as_request_action().to_string(),
                    confirm: *confirm,
                    reason_code: reason_code.clone(),
                },
            }),
            Self::BuyerPublishRequest {
                asset_id,
                refresh_market,
            } => Ok(DesktopControlActionRequest::PublishDataMarketBuyerRequest {
                args: DesktopControlDataMarketBuyerRequestArgs {
                    asset_id: asset_id.clone(),
                    refresh_market: *refresh_market,
                },
            }),
            Self::SellerImportRequest { .. } | Self::BuyerImportResponse { .. } => Err(anyhow!(
                "relay import commands build their action after fetching from relay"
            )),
            Self::ConsumeDelivery {
                delivery_bundle_id,
                request_id,
                grant_id,
                asset_id,
                refresh_market,
                ..
            } => Ok(DesktopControlActionRequest::ResolveDataMarketDelivery {
                args: DesktopControlDataMarketResolveDeliveryArgs {
                    delivery_bundle_id: delivery_bundle_id.clone(),
                    request_id: request_id.clone(),
                    grant_id: grant_id.clone(),
                    asset_id: asset_id.clone(),
                    refresh_market: *refresh_market,
                },
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum LogSourceArg {
    MissionControl,
    Session,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AttnResViewArg {
    Overview,
    Pipeline,
    Inference,
    Loss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TassadarViewArg {
    Overview,
    Trace,
    Program,
    Evidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TassadarSourceArg {
    Replay,
    #[value(name = "article-session")]
    ArticleSession,
    #[value(name = "hybrid-workflow")]
    HybridWorkflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TassadarReplayFamilyArg {
    #[value(name = "article-sessions")]
    ArticleSessions,
    #[value(name = "hybrid-workflows")]
    HybridWorkflows,
    #[value(name = "compiled-closure")]
    CompiledClosure,
    Acceptance,
    #[value(name = "learned-promotion")]
    LearnedPromotion,
    #[value(name = "learned-9x9-fit")]
    Learned9x9Fit,
    #[value(name = "learned-horizon")]
    LearnedHorizon,
    #[value(name = "architecture-comparison")]
    ArchitectureComparison,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum WaitConditionArg {
    ProviderOnline,
    ProviderOffline,
    LocalRuntimeReady,
    AppleFmReady,
    GptOssReady,
    GptOssUnloaded,
    Nip28Ready,
    Nip28MessagePresent,
    Nip28OutboundIdle,
    BuyModeRunning,
    BuyModeStopped,
    BuyModeTargetReady,
    BuyModeInFlight,
    BuyModePaymentRequired,
    BuyModePaid,
    BuyModeFailed,
    #[value(name = "attnres-running")]
    AttnResRunning,
    #[value(name = "attnres-paused")]
    AttnResPaused,
    #[value(name = "attnres-completed")]
    AttnResCompleted,
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
    identity_path: Option<PathBuf>,
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
    LocalRuntimeReady,
    AppleFmReady,
    GptOssReady,
    GptOssUnloaded,
    Nip28Ready,
    Nip28MessagePresent,
    Nip28OutboundIdle,
    BuyModeRunning,
    BuyModeStopped,
    BuyModeTargetReady,
    BuyModeInFlight,
    BuyModePaymentRequired,
    BuyModePaid,
    BuyModeFailed,
    AttnResRunning,
    AttnResPaused,
    AttnResCompleted,
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
struct DataMarketConsumeEnvelope<'a> {
    message: &'a str,
    payload: &'a Value,
    consumed: &'a DataMarketConsumedDeliverySummary,
}

#[derive(Clone, Debug, Serialize)]
struct RelayFetchedEventEnvelope {
    relay_url: String,
    event_id: String,
    kind: u16,
    pubkey: String,
    event_json: Value,
}

#[derive(Clone, Debug)]
struct DataMarketResolvedDeliveryLocator {
    delivery_ref: String,
    delivery_digest: Option<String>,
    manifest_refs: Vec<String>,
}

#[derive(Clone, Debug)]
enum DataMarketResolvedPayloadSource {
    LocalPath(PathBuf),
    RemoteFile {
        source_ref: String,
        suggested_name: String,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, Default, Deserialize)]
struct DataMarketDeliveryPointerEnvelope {
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    delivery_ref: Option<String>,
    #[serde(default, alias = "ref")]
    ref_value: Option<String>,
    #[serde(default, alias = "url")]
    url: Option<String>,
    #[serde(default)]
    delivery_digest: Option<String>,
    #[serde(default, alias = "x")]
    digest: Option<String>,
    #[serde(default)]
    manifest_refs: Vec<String>,
    #[serde(default)]
    sender_pubkey: Option<String>,
    #[serde(default)]
    ciphertext: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DataMarketConsumedDeliverySummary {
    schema_version: &'static str,
    selection_reason: Option<String>,
    resolved_request_id: Option<String>,
    delivery_bundle_id: String,
    grant_id: String,
    asset_id: String,
    provider_id: String,
    consumer_id: String,
    delivery_ref: String,
    delivery_digest: Option<String>,
    output_dir: String,
    payload_source_kind: String,
    payload_output_path: String,
    copied_manifest_paths: Vec<String>,
    unresolved_manifest_refs: Vec<String>,
    manifest_refs: Vec<String>,
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
        Command::Perf => {
            let response = client.action(&DesktopControlActionRequest::GetPaneSnapshot {
                pane: "frame_debugger".to_string(),
            })?;
            ensure_action_success(&response)?;
            let payload = response.payload.as_ref().unwrap_or(&Value::Null);
            let perf_payload = payload.get("frame_debugger").unwrap_or(payload);
            if json_output {
                print_json(perf_payload)?;
            } else {
                print_perf_text(perf_payload);
            }
        }
        Command::AttnRes { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let status = parse_attnres_status(response.payload.as_ref())?;
            if json_output {
                print_json(&status)?;
            } else {
                if !matches!(command, AttnResCommand::Status) {
                    println!("{}", response.message);
                }
                print_attnres_text(&status);
            }
        }
        Command::Tassadar { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let status = parse_tassadar_status(response.payload.as_ref())?;
            if json_output {
                print_json(&status)?;
            } else {
                if !matches!(command, TassadarCommand::Status) {
                    println!("{}", response.message);
                }
                print_tassadar_text(&status);
            }
        }
        Command::Cluster { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let payload = response.payload.as_ref().unwrap_or(&Value::Null);
            if json_output {
                print_json(payload)?;
            } else {
                print_cluster_text(payload);
            }
        }
        Command::Sandbox { command } => match command {
            SandboxCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.sandbox)?;
                } else {
                    print_sandbox_status_text(&snapshot);
                }
            }
            SandboxCommand::Job { .. } => {
                let action = command.action_request()?.ok_or_else(|| {
                    anyhow!("sandbox job inspection did not produce a control action")
                })?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    print_sandbox_job_text(payload);
                }
            }
            SandboxCommand::Create { .. }
            | SandboxCommand::Upload { .. }
            | SandboxCommand::Start { .. } => {
                let action = command
                    .action_request()?
                    .ok_or_else(|| anyhow!("sandbox command did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    println!("{}", response.message);
                    print_sandbox_payload_summary(payload);
                }
            }
            SandboxCommand::Wait { .. } => {
                let action = command
                    .action_request()?
                    .ok_or_else(|| anyhow!("sandbox wait did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    println!("{}", response.message);
                    print_sandbox_job_text(payload);
                }
            }
            SandboxCommand::DownloadArtifact { ref output, .. }
            | SandboxCommand::DownloadWorkspace { ref output, .. } => {
                let action = command
                    .action_request()?
                    .ok_or_else(|| anyhow!("sandbox download did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    print_sandbox_download_text(payload, output.as_ref())?;
                }
            }
        },
        Command::Proof { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let payload = response.payload.as_ref().unwrap_or(&Value::Null);
            if json_output {
                print_json(payload)?;
            } else {
                print_proof_text(payload);
            }
        }
        Command::Challenge { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let payload = response.payload.as_ref().unwrap_or(&Value::Null);
            if json_output {
                print_json(payload)?;
            } else {
                print_challenge_text(payload);
            }
        }
        Command::Training { command } => match command {
            TrainingCommand::Watch {
                run_id,
                poll_ms,
                timeout_ms,
            } => watch_training_run(&client, run_id.as_deref(), poll_ms, timeout_ms, json_output)?,
            _ => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    if !matches!(command, TrainingCommand::Status) {
                        println!("{}", response.message);
                    }
                    print_training_text(payload);
                }
            }
        },
        Command::RemoteTraining { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let payload = response.payload.as_ref().unwrap_or(&Value::Null);
            if json_output {
                print_json(payload)?;
            } else {
                if matches!(command, RemoteTrainingCommand::Refresh) {
                    println!("{}", response.message);
                }
                match command {
                    RemoteTrainingCommand::Status | RemoteTrainingCommand::Refresh => {
                        print_remote_training_text(payload);
                    }
                    RemoteTrainingCommand::List => print_remote_training_runs_text(payload),
                    RemoteTrainingCommand::Run { .. } => print_remote_training_run_text(payload),
                    RemoteTrainingCommand::Stale => print_remote_training_stale_text(payload),
                }
            }
        }
        Command::Research { command } => {
            let response = client.action(&command.action_request())?;
            ensure_action_success(&response)?;
            let payload = response.payload.as_ref().unwrap_or(&Value::Null);
            if json_output {
                print_json(payload)?;
            } else {
                if matches!(command, ResearchCommand::Reset) {
                    println!("{}", response.message);
                }
                print_research_text(payload);
            }
        }
        Command::Pane { command } => match command {
            PaneCommand::List => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    print_pane_list_text(payload);
                }
            }
            PaneCommand::Status { .. } => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    print_pane_snapshot_text(payload);
                }
            }
            PaneCommand::Capture { .. } => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(payload)?;
                } else {
                    print_pane_capture_text(payload);
                }
            }
            PaneCommand::Open { .. } | PaneCommand::Focus { .. } | PaneCommand::Close { .. } => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
        },
        Command::Provider { command } => match command {
            ProviderCommand::Online { wait, timeout_ms } => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::ProviderOnline, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            ProviderCommand::Offline { wait, timeout_ms } => {
                let response = client.action(&command.action_request())?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::ProviderOffline, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
        },
        Command::LocalRuntime { command } => match command {
            LocalRuntimeCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.local_runtime)?;
                } else {
                    print_local_runtime_text(&snapshot);
                }
            }
            LocalRuntimeCommand::Refresh { wait, timeout_ms } => {
                let action = command.action_request().ok_or_else(|| {
                    anyhow!("local runtime refresh did not produce a control action")
                })?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::LocalRuntimeReady, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
        },
        Command::GptOss { command } => match command {
            GptOssCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.gpt_oss)?;
                } else {
                    print_gpt_oss_text(&snapshot);
                }
            }
            GptOssCommand::Refresh { wait, timeout_ms } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("gpt-oss refresh did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::GptOssReady, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            GptOssCommand::Warm { wait, timeout_ms } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("gpt-oss warm did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::GptOssReady, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            GptOssCommand::Unload { wait, timeout_ms } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("gpt-oss unload did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::GptOssUnloaded, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
        },
        Command::AppleFm { command } => match command {
            AppleFmCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.apple_fm)?;
                } else {
                    print_apple_fm_text(&snapshot);
                }
            }
            AppleFmCommand::List => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.apple_fm.loaded_adapters)?;
                } else {
                    print_apple_fm_adapter_list_text(&snapshot);
                }
            }
            AppleFmCommand::Refresh { wait, timeout_ms } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("apple fm refresh did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::AppleFmReady, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            AppleFmCommand::Load { .. }
            | AppleFmCommand::Unload { .. }
            | AppleFmCommand::Attach { .. }
            | AppleFmCommand::Detach { .. } => {
                let action = command.action_request().ok_or_else(|| {
                    anyhow!("apple fm adapter command did not produce a control action")
                })?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
            AppleFmCommand::SmokeTest => {
                let action = command.action_request().ok_or_else(|| {
                    anyhow!("apple fm smoke test did not produce a control action")
                })?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
        },
        Command::Wallet { command } => match command {
            WalletCommand::Refresh => {
                let response = client.action(&command.action_request())?;
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
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("buy mode start did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::BuyModeRunning, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            BuyModeCommand::Stop { wait, timeout_ms } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("buy mode stop did not produce a control action"))?;
                let response = client.action(&action)?;
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
                        "cadenceMillis": snapshot.buy_mode.cadence_millis,
                        "enabled": snapshot.buy_mode.enabled,
                        "nextDispatchCountdownSeconds": snapshot.buy_mode.next_dispatch_countdown_seconds,
                        "nextDispatchCountdownMillis": snapshot.buy_mode.next_dispatch_countdown_millis,
                        "inFlightRequestId": snapshot.buy_mode.in_flight_request_id,
                        "inFlightPhase": snapshot.buy_mode.in_flight_phase,
                        "inFlightStatus": snapshot.buy_mode.in_flight_status,
                        "selectedProviderPubkey": snapshot.buy_mode.selected_provider_pubkey,
                        "resultProviderPubkey": snapshot.buy_mode.result_provider_pubkey,
                        "invoiceProviderPubkey": snapshot.buy_mode.invoice_provider_pubkey,
                        "payableProviderPubkey": snapshot.buy_mode.payable_provider_pubkey,
                        "paymentBlockerCodes": snapshot.buy_mode.payment_blocker_codes,
                        "paymentBlockerSummary": snapshot.buy_mode.payment_blocker_summary,
                        "targetSelection": snapshot.buy_mode.target_selection,
                        "peerRoster": snapshot.buy_mode.peer_roster,
                        "recentRequests": snapshot.buy_mode.recent_requests,
                    }))?;
                } else {
                    print_buy_mode_text(&snapshot);
                }
            }
            BuyModeCommand::Target => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.buy_mode.target_selection)?;
                } else {
                    print_buy_mode_target_text(&snapshot);
                }
            }
            BuyModeCommand::Roster { limit } => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(
                        &snapshot
                            .buy_mode
                            .peer_roster
                            .iter()
                            .take(limit)
                            .collect::<Vec<_>>(),
                    )?;
                } else {
                    print_buy_mode_roster_text(&snapshot, limit);
                }
            }
        },
        Command::Nip90Payments { command } => {
            let action = command.action_request()?;
            let response = client.action(&action)?;
            ensure_action_success(&response)?;
            let report = parse_nip90_sent_payments_report(response.payload.as_ref())?;
            if json_output {
                print_json(&report)?;
            } else {
                print_nip90_sent_payments_report_text(&report);
            }
        }
        Command::Tunnels { command } => match command {
            TunnelCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.tunnels)?;
                } else {
                    print_tunnels_text(&snapshot);
                }
            }
        },
        Command::Tailnet { command } => match command {
            TailnetCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.tailnet)?;
                } else {
                    print_tailnet_text(&snapshot);
                }
            }
        },
        Command::Chat { command } => match command {
            ChatCommand::Status => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.nip28)?;
                } else {
                    print_nip28_status_text(&snapshot);
                }
            }
            ChatCommand::Main => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("chat main did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
            ChatCommand::Groups => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.nip28.groups)?;
                } else {
                    print_nip28_groups_text(&snapshot);
                }
            }
            ChatCommand::Channels => {
                let snapshot = client.snapshot()?;
                if json_output {
                    print_json(&snapshot.nip28.channels)?;
                } else {
                    print_nip28_channels_text(&snapshot);
                }
            }
            ChatCommand::Tail { limit } => {
                let snapshot = client.snapshot()?;
                let mut messages = snapshot.nip28.recent_messages.clone();
                if messages.len() > limit {
                    messages.drain(0..messages.len().saturating_sub(limit));
                }
                if json_output {
                    print_json(&messages)?;
                } else {
                    print_nip28_messages_text(&messages);
                }
            }
            ChatCommand::SelectGroup { .. } | ChatCommand::SelectChannel { .. } => {
                let action = command.action_request().ok_or_else(|| {
                    anyhow!("chat selection command did not produce a control action")
                })?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
            ChatCommand::Send {
                wait, timeout_ms, ..
            } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("chat send did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::Nip28OutboundIdle, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            ChatCommand::Retry {
                wait, timeout_ms, ..
            } => {
                let action = command
                    .action_request()
                    .ok_or_else(|| anyhow!("chat retry did not produce a control action"))?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                let waited = if wait {
                    Some(client.wait_for_condition(WaitCondition::Nip28OutboundIdle, timeout_ms)?)
                } else {
                    None
                };
                print_action(json_output, &response, waited.as_ref())?;
            }
            ChatCommand::CreateChannel { .. } => {
                let action = command.action_request().ok_or_else(|| {
                    anyhow!("chat create-channel did not produce a control action")
                })?;
                let response = client.action(&action)?;
                ensure_action_success(&response)?;
                print_action(json_output, &response, None)?;
            }
        },
        Command::DataMarket { command } => match &command {
            DataMarketCommand::SellerImportRequest {
                event_id,
                relay_urls,
                timeout_ms,
            } => {
                let relay_urls = resolve_data_market_relay_urls(&client, relay_urls.as_slice())?;
                let fetched =
                    fetch_relay_event_by_id(event_id.as_str(), relay_urls.as_slice(), *timeout_ms)?;
                let response = client.action(
                    &DesktopControlActionRequest::ImportDataMarketSellerRequest {
                        args: DesktopControlDataMarketImportSellerRequestArgs {
                            event_json: fetched.event_json.clone(),
                            source_relay_url: Some(fetched.relay_url.clone()),
                        },
                    },
                )?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(&json!({
                        "message": response.message,
                        "payload": payload,
                        "relay_fetch": fetched,
                    }))?;
                } else {
                    println!("{}", response.message);
                    println!(
                        "imported from relay {} event={} kind={}",
                        fetched.relay_url, fetched.event_id, fetched.kind
                    );
                    print_data_market_snapshot_text(payload);
                }
            }
            DataMarketCommand::BuyerImportResponse {
                event_id,
                relay_urls,
                timeout_ms,
            } => {
                let relay_urls = resolve_data_market_relay_urls(&client, relay_urls.as_slice())?;
                let fetched =
                    fetch_relay_event_by_id(event_id.as_str(), relay_urls.as_slice(), *timeout_ms)?;
                let response = client.action(
                    &DesktopControlActionRequest::ImportDataMarketBuyerResponse {
                        args: DesktopControlDataMarketImportBuyerResponseArgs {
                            event_json: fetched.event_json.clone(),
                            source_relay_url: Some(fetched.relay_url.clone()),
                        },
                    },
                )?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if json_output {
                    print_json(&json!({
                        "message": response.message,
                        "payload": payload,
                        "relay_fetch": fetched,
                    }))?;
                } else {
                    println!("{}", response.message);
                    println!(
                        "imported from relay {} event={} kind={}",
                        fetched.relay_url, fetched.event_id, fetched.kind
                    );
                    print_data_market_snapshot_text(payload);
                }
            }
            _ => {
                let response = client.action(&command.action_request()?)?;
                ensure_action_success(&response)?;
                let payload = response.payload.as_ref().unwrap_or(&Value::Null);
                if let DataMarketCommand::ConsumeDelivery {
                    output_dir,
                    overwrite,
                    ..
                } = &command
                {
                    let consumed = materialize_data_market_delivery(
                        &client,
                        payload,
                        output_dir.as_path(),
                        *overwrite,
                    )?;
                    if json_output {
                        print_json(&DataMarketConsumeEnvelope {
                            message: response.message.as_str(),
                            payload,
                            consumed: &consumed,
                        })?;
                    } else {
                        println!("{}", response.message);
                        print_data_market_consumed_delivery_text(&consumed);
                    }
                } else if json_output {
                    print_json(&json!({
                        "message": response.message,
                        "payload": payload,
                    }))?;
                } else {
                    if !matches!(
                        command,
                        DataMarketCommand::SellerStatus
                            | DataMarketCommand::BuyerStatus
                            | DataMarketCommand::Snapshot
                    ) {
                        println!("{}", response.message);
                    }
                    print_data_market_snapshot_text(payload);
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
                    .ok_or_else(|| {
                        anyhow!("Desktop control manifest does not include latest session log path")
                    })?;
                let lines = tail_file_lines(path, tail)?;
                if json_output {
                    let parsed = lines
                        .iter()
                        .map(|line| {
                            serde_json::from_str::<Value>(line)
                                .unwrap_or(Value::String(line.clone()))
                        })
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
        let body = response
            .text()
            .context("read desktop control action body")?;
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
        format!("{}{}", self.target.base_url.trim_end_matches('/'), path)
    }
}

fn resolve_data_market_relay_urls(
    client: &DesktopControlClient,
    requested: &[String],
) -> Result<Vec<String>> {
    if !requested.is_empty() {
        let relays = requested
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if relays.is_empty() {
            bail!("No usable relay URLs were supplied");
        }
        return Ok(relays);
    }

    let snapshot = client.snapshot()?;
    if snapshot.provider.relay_urls.is_empty() {
        bail!("No relay URLs were supplied and the runtime snapshot has no provider relay URLs");
    }
    Ok(snapshot.provider.relay_urls)
}

fn fetch_relay_event_by_id(
    event_id: &str,
    relay_urls: &[String],
    timeout_ms: u64,
) -> Result<RelayFetchedEventEnvelope> {
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let runtime = tokio::runtime::Runtime::new().context("create relay fetch runtime")?;
    runtime.block_on(async move {
        let mut last_error = None::<String>;
        for relay_url in relay_urls {
            let connection = RelayConnection::new(relay_url.as_str())
                .with_context(|| format!("create relay connection for {relay_url}"))?;
            if let Err(error) = connection.connect().await {
                last_error = Some(format!("{relay_url}: {error}"));
                continue;
            }

            let subscription_id = format!(
                "autopilotctl-import-{}",
                event_id.chars().take(12).collect::<String>()
            );
            if let Err(error) = connection
                .subscribe_filters(
                    subscription_id.as_str(),
                    vec![json!({
                        "ids": [event_id],
                        "limit": 1,
                    })],
                )
                .await
            {
                let _ = connection.disconnect().await;
                last_error = Some(format!("{relay_url}: {error}"));
                continue;
            }

            let started = Instant::now();
            let mut found = None;
            while started.elapsed() < timeout {
                let remaining = timeout
                    .checked_sub(started.elapsed())
                    .unwrap_or_else(|| Duration::from_millis(1));
                match tokio::time::timeout(remaining.min(Duration::from_secs(1)), connection.recv())
                    .await
                {
                    Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                        if event.id == event_id {
                            found = Some(event);
                            break;
                        }
                    }
                    Ok(Ok(Some(RelayMessage::Eose(_)))) => break,
                    Ok(Ok(Some(_))) => {}
                    Ok(Ok(None)) => break,
                    Ok(Err(error)) => {
                        last_error = Some(format!("{relay_url}: {error}"));
                        break;
                    }
                    Err(_) => break,
                }
            }

            let _ = connection.unsubscribe(subscription_id.as_str()).await;
            let _ = connection.disconnect().await;

            if let Some(event) = found {
                return Ok(RelayFetchedEventEnvelope {
                    relay_url: relay_url.clone(),
                    event_id: event.id.clone(),
                    kind: event.kind,
                    pubkey: event.pubkey.clone(),
                    event_json: serde_json::to_value(&event)
                        .context("encode fetched relay event")?,
                });
            }
        }

        Err(anyhow!(
            "Failed to fetch event {} from relays: {}",
            event_id,
            last_error.unwrap_or_else(|| "not found before relay EOSE".to_string())
        ))
    })
}

impl WaitConditionArg {
    fn into_condition(self) -> WaitCondition {
        match self {
            Self::ProviderOnline => WaitCondition::ProviderOnline,
            Self::ProviderOffline => WaitCondition::ProviderOffline,
            Self::LocalRuntimeReady => WaitCondition::LocalRuntimeReady,
            Self::AppleFmReady => WaitCondition::AppleFmReady,
            Self::GptOssReady => WaitCondition::GptOssReady,
            Self::GptOssUnloaded => WaitCondition::GptOssUnloaded,
            Self::Nip28Ready => WaitCondition::Nip28Ready,
            Self::Nip28MessagePresent => WaitCondition::Nip28MessagePresent,
            Self::Nip28OutboundIdle => WaitCondition::Nip28OutboundIdle,
            Self::BuyModeRunning => WaitCondition::BuyModeRunning,
            Self::BuyModeStopped => WaitCondition::BuyModeStopped,
            Self::BuyModeTargetReady => WaitCondition::BuyModeTargetReady,
            Self::BuyModeInFlight => WaitCondition::BuyModeInFlight,
            Self::BuyModePaymentRequired => WaitCondition::BuyModePaymentRequired,
            Self::BuyModePaid => WaitCondition::BuyModePaid,
            Self::BuyModeFailed => WaitCondition::BuyModeFailed,
            Self::AttnResRunning => WaitCondition::AttnResRunning,
            Self::AttnResPaused => WaitCondition::AttnResPaused,
            Self::AttnResCompleted => WaitCondition::AttnResCompleted,
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
            Self::LocalRuntimeReady => "local-runtime-ready",
            Self::AppleFmReady => "apple-fm-ready",
            Self::GptOssReady => "gpt-oss-ready",
            Self::GptOssUnloaded => "gpt-oss-unloaded",
            Self::Nip28Ready => "nip28-ready",
            Self::Nip28MessagePresent => "nip28-message-present",
            Self::Nip28OutboundIdle => "nip28-outbound-idle",
            Self::BuyModeRunning => "buy-mode-running",
            Self::BuyModeStopped => "buy-mode-stopped",
            Self::BuyModeTargetReady => "buy-mode-target-ready",
            Self::BuyModeInFlight => "buy-mode-in-flight",
            Self::BuyModePaymentRequired => "buy-mode-payment-required",
            Self::BuyModePaid => "buy-mode-paid",
            Self::BuyModeFailed => "buy-mode-failed",
            Self::AttnResRunning => "attnres-running",
            Self::AttnResPaused => "attnres-paused",
            Self::AttnResCompleted => "attnres-completed",
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
            Self::LocalRuntimeReady => "local runtime ready",
            Self::AppleFmReady => "Apple FM ready",
            Self::GptOssReady => "GPT-OSS ready",
            Self::GptOssUnloaded => "GPT-OSS unloaded",
            Self::Nip28Ready => "NIP-28 ready",
            Self::Nip28MessagePresent => "NIP-28 message present",
            Self::Nip28OutboundIdle => "NIP-28 outbound idle",
            Self::BuyModeRunning => "buy mode running",
            Self::BuyModeStopped => "buy mode stopped",
            Self::BuyModeTargetReady => "buy mode target ready",
            Self::BuyModeInFlight => "buy mode in flight",
            Self::BuyModePaymentRequired => "buy mode payment-required",
            Self::BuyModePaid => "buy mode paid",
            Self::BuyModeFailed => "buy mode failed",
            Self::AttnResRunning => "AttnRes running",
            Self::AttnResPaused => "AttnRes paused",
            Self::AttnResCompleted => "AttnRes completed",
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
            Self::LocalRuntimeReady => snapshot.local_runtime.runtime_ready,
            Self::AppleFmReady => snapshot.apple_fm.ready,
            Self::GptOssReady => snapshot.gpt_oss.ready,
            Self::GptOssUnloaded => {
                snapshot.gpt_oss.detected
                    && !snapshot.gpt_oss.ready
                    && !snapshot.gpt_oss.busy
                    && !snapshot.gpt_oss.loaded
            }
            Self::Nip28Ready => {
                snapshot.nip28.available && snapshot.nip28.selected_channel_id.is_some()
            }
            Self::Nip28MessagePresent => !snapshot.nip28.recent_messages.is_empty(),
            Self::Nip28OutboundIdle => snapshot.nip28.publishing_outbound_count == 0,
            Self::BuyModeRunning => snapshot.buy_mode.enabled,
            Self::BuyModeStopped => !snapshot.buy_mode.enabled,
            Self::BuyModeTargetReady => snapshot
                .buy_mode
                .target_selection
                .selected_peer_pubkey
                .is_some(),
            Self::BuyModeInFlight => snapshot.buy_mode.in_flight_request_id.is_some(),
            Self::BuyModePaymentRequired => snapshot
                .buy_mode
                .recent_requests
                .iter()
                .any(request_has_payment_required),
            Self::BuyModePaid => buy_mode_has_paid_request(&snapshot.buy_mode),
            Self::BuyModeFailed => buy_mode_has_failed_request(&snapshot.buy_mode),
            Self::AttnResRunning => snapshot.attnres_lab.running,
            Self::AttnResPaused => {
                !snapshot.attnres_lab.running
                    && matches!(
                        snapshot.attnres_lab.playback_state.as_str(),
                        "paused" | "training paused"
                    )
            }
            Self::AttnResCompleted => matches!(
                snapshot.attnres_lab.playback_state.as_str(),
                "completed" | "run complete"
            ),
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

fn request_has_paid(request: &DesktopControlBuyModeRequestStatus) -> bool {
    request.status.eq_ignore_ascii_case("paid")
        || request.phase.eq_ignore_ascii_case("paid")
        || request.wallet_status.eq_ignore_ascii_case("sent")
}

fn request_has_failed(request: &DesktopControlBuyModeRequestStatus) -> bool {
    request.status.eq_ignore_ascii_case("failed")
        || request.phase.eq_ignore_ascii_case("failed")
        || request.payment_error.is_some()
}

fn buy_mode_has_paid_request(status: &DesktopControlBuyModeStatus) -> bool {
    status
        .in_flight_status
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("paid"))
        || status.recent_requests.iter().any(request_has_paid)
}

fn buy_mode_has_failed_request(status: &DesktopControlBuyModeStatus) -> bool {
    status
        .in_flight_status
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("failed"))
        || status.recent_requests.iter().any(request_has_failed)
}

fn resolve_target(cli: &Cli) -> Result<ResolvedTarget> {
    match (cli.base_url.as_ref(), cli.auth_token.as_ref()) {
        (Some(base_url), Some(auth_token)) => Ok(ResolvedTarget {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            auth_token: auth_token.trim().to_string(),
            manifest_path: cli.manifest.clone(),
            identity_path: cli
                .manifest
                .as_ref()
                .and_then(|path| load_manifest_from_path(path).ok())
                .and_then(|manifest| manifest.identity_path.map(PathBuf::from)),
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
                identity_path: manifest.identity_path.map(PathBuf::from),
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

fn parse_nip90_sent_payments_report(
    payload: Option<&Value>,
) -> Result<DesktopControlNip90SentPaymentsReport> {
    let payload = payload.ok_or_else(|| anyhow!("missing NIP-90 sent-payments report payload"))?;
    serde_json::from_value::<DesktopControlNip90SentPaymentsReport>(payload.clone())
        .context("decode NIP-90 sent-payments report payload")
}

fn parse_attnres_status(payload: Option<&Value>) -> Result<DesktopControlAttnResStatus> {
    let payload = payload.ok_or_else(|| anyhow!("missing AttnRes status payload"))?;
    serde_json::from_value::<DesktopControlAttnResStatus>(payload.clone())
        .context("decode AttnRes status payload")
}

fn parse_tassadar_status(payload: Option<&Value>) -> Result<DesktopControlTassadarStatus> {
    let payload = payload.ok_or_else(|| anyhow!("missing Tassadar status payload"))?;
    serde_json::from_value::<DesktopControlTassadarStatus>(payload.clone())
        .context("decode Tassadar status payload")
}

fn parse_local_daily_window(date: &str) -> Result<(u64, u64)> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
        .with_context(|| format!("parse daily report date '{}'", date.trim()))?;
    let next_day = date
        .succ_opt()
        .ok_or_else(|| anyhow!("cannot compute next day for {}", date.format("%Y-%m-%d")))?;
    Ok((
        local_midnight_epoch_seconds(date)?,
        local_midnight_epoch_seconds(next_day)?,
    ))
}

fn local_midnight_epoch_seconds(date: NaiveDate) -> Result<u64> {
    match Local.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0) {
        LocalResult::Single(timestamp) => u64::try_from(timestamp.timestamp())
            .context("local midnight timestamp should be non-negative"),
        LocalResult::Ambiguous(earliest, _) => u64::try_from(earliest.timestamp())
            .context("local midnight timestamp should be non-negative"),
        LocalResult::None => bail!(
            "local midnight does not exist for {}",
            date.format("%Y-%m-%d")
        ),
    }
}

fn parse_report_boundary(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("report boundary cannot be empty");
    }
    if trimmed.chars().all(|value| value.is_ascii_digit()) {
        return trimmed
            .parse::<u64>()
            .with_context(|| format!("parse epoch-seconds boundary '{trimmed}'"));
    }
    let timestamp = DateTime::parse_from_rfc3339(trimmed)
        .with_context(|| format!("parse RFC3339 boundary '{trimmed}'"))?;
    u64::try_from(timestamp.timestamp()).context("report boundary timestamp should be non-negative")
}

fn tail_file_lines(path: &Path, tail: usize) -> Result<Vec<String>> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("read session log {}", path.display()))?;
    let mut lines = raw.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.len() > tail {
        lines.drain(0..lines.len().saturating_sub(tail));
    }
    Ok(lines)
}

fn load_json_file<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("read {label} {}", path.display()))?;
    serde_json::from_str::<T>(raw.as_str())
        .with_context(|| format!("decode {label} {}", path.display()))
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("serialize autopilotctl JSON output")?
    );
    Ok(())
}

fn print_nip90_sent_payments_report_text(report: &DesktopControlNip90SentPaymentsReport) {
    for line in nip90_sent_payments_report_lines(report) {
        println!("{line}");
    }
}

fn nip90_sent_payments_report_lines(report: &DesktopControlNip90SentPaymentsReport) -> Vec<String> {
    let title = report
        .report_date
        .as_deref()
        .map(|date| format!("NIP-90 sent payments report: {date}"))
        .unwrap_or_else(|| "NIP-90 sent payments report".to_string());
    vec![
        title,
        format!(
            "window: {} -> {}",
            report.window_start_rfc3339, report.window_end_rfc3339
        ),
        format!(
            "totals: payment_count={} total_sats_sent={} total_fee_sats={} total_wallet_debit_sats={}",
            report.payment_count,
            report.total_sats_sent,
            report.total_fee_sats,
            report.total_wallet_debit_sats
        ),
        format!(
            "dedupe: connected_relay_count={} deduped_request_count={} degraded_binding_count={}",
            report.connected_relay_count,
            report.deduped_request_count,
            report.degraded_binding_count
        ),
        format!(
            "relay_urls_considered: {}",
            if report.relay_urls_considered.is_empty() {
                "none".to_string()
            } else {
                report.relay_urls_considered.join(", ")
            }
        ),
        format!("generated_at: {}", report.generated_at_rfc3339),
    ]
}

fn print_pane_list_text(payload: &Value) {
    let active_pane_id = payload.get("active_pane_id").and_then(Value::as_u64);
    println!("active pane id: {}", active_pane_id.unwrap_or(0));
    println!("registered panes:");
    for pane in payload
        .get("registered")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let kind = pane.get("kind").and_then(Value::as_str).unwrap_or("-");
        let title = pane.get("title").and_then(Value::as_str).unwrap_or("-");
        let command_id = pane
            .get("command_id")
            .and_then(Value::as_str)
            .unwrap_or("-");
        println!("  {kind:<24} {title} [{command_id}]");
    }
    println!("open panes:");
    for pane in payload
        .get("open")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let pane_id = pane.get("pane_id").and_then(Value::as_u64).unwrap_or(0);
        let kind = pane.get("kind").and_then(Value::as_str).unwrap_or("-");
        let title = pane.get("title").and_then(Value::as_str).unwrap_or("-");
        let z_index = pane.get("z_index").and_then(Value::as_u64).unwrap_or(0);
        println!("  #{pane_id:<6} {kind:<24} z={z_index:<4} {title}");
    }
}

fn print_pane_snapshot_text(payload: &Value) {
    let kind = payload.get("kind").and_then(Value::as_str).unwrap_or("-");
    let title = payload.get("title").and_then(Value::as_str).unwrap_or("-");
    let open_instances = payload
        .get("open_instances")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let top_pane_id = payload
        .get("top_pane_id")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let active_pane_id = payload
        .get("active_pane_id")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    println!("pane: {kind} ({title})");
    println!(
        "open_instances={} top_pane_id={} active_pane_id={}",
        open_instances, top_pane_id, active_pane_id
    );
    if let Some(object) = payload.as_object() {
        for (key, value) in object {
            if matches!(
                key.as_str(),
                "kind" | "title" | "open_instances" | "top_pane_id" | "active_pane_id"
            ) {
                continue;
            }
            println!("{key}:");
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            );
        }
    }
}

fn print_pane_capture_text(payload: &Value) {
    let kind = payload.get("kind").and_then(Value::as_str).unwrap_or("-");
    let title = payload.get("title").and_then(Value::as_str).unwrap_or("-");
    let pane_id = payload.get("pane_id").and_then(Value::as_u64);
    let presentation = payload
        .get("presentation")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let png_path = payload
        .get("png_path")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let manifest_path = payload
        .get("manifest_path")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let width = payload.get("width").and_then(Value::as_u64).unwrap_or(0);
    let height = payload.get("height").and_then(Value::as_u64).unwrap_or(0);
    let scale_factor = payload
        .get("scale_factor")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let capture_target = payload
        .get("capture_target")
        .and_then(Value::as_str)
        .unwrap_or("-");

    println!("pane capture: {kind} ({title})");
    println!(
        "pane_id={} presentation={} capture_target={}",
        pane_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        presentation,
        capture_target
    );
    println!("size={}x{} scale={scale_factor:.2}", width, height);
    println!("png: {png_path}");
    println!("manifest: {manifest_path}");
}

fn print_data_market_snapshot_text(payload: &Value) {
    let schema_version = payload
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let seller = payload.get("seller").unwrap_or(&Value::Null);
    let draft = seller.get("draft").unwrap_or(&Value::Null);
    println!(
        "data market: schema={} seller_load_state={} preview_enabled={} confirm_enabled={} publish_enabled={} incoming_requests={}",
        schema_version,
        json_str(seller.get("load_state")).unwrap_or("-"),
        json_bool(seller.get("preview_enabled")),
        json_bool(seller.get("confirm_enabled")),
        json_bool(seller.get("publish_enabled")),
        json_array_len(seller.get("incoming_requests"))
    );
    println!(
        "seller: status={} codex_phase={} inventory_warnings={} required_skills={}",
        json_str(seller.get("status_line")).unwrap_or("-"),
        json_str(seller.get("codex_session_phase")).unwrap_or("-"),
        json_array_len(seller.get("inventory_warnings")),
        seller
            .get("required_skill_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    );
    println!(
        "seller inventory: assets={} grants={}",
        seller
            .get("published_asset_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        seller
            .get("published_grant_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    );
    if let Some(thread_id) = json_str(seller.get("codex_thread_id")) {
        println!("seller thread: {thread_id}");
    }
    if let Some(last_action) = json_str(seller.get("last_action")) {
        println!("seller last action: {last_action}");
    }
    if let Some(last_error) = json_str(seller.get("last_error")) {
        println!("seller last error: {last_error}");
    }
    println!(
        "draft: kind={} title={} price_hint_sats={} policy={} visibility={} sensitivity={} preview_posture={}",
        json_str(draft.get("asset_kind")).unwrap_or("-"),
        json_str(draft.get("title")).unwrap_or("-"),
        draft
            .get("price_hint_sats")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        json_str(draft.get("default_policy")).unwrap_or("-"),
        json_str(draft.get("visibility_posture")).unwrap_or("-"),
        json_str(draft.get("sensitivity_posture")).unwrap_or("-"),
        json_str(draft.get("preview_posture")).unwrap_or("-"),
    );
    println!(
        "draft refs: content_digest={} provenance_ref={} delivery_modes={}",
        json_str(draft.get("content_digest")).unwrap_or("-"),
        json_str(draft.get("provenance_ref")).unwrap_or("-"),
        json_joined_array(draft.get("delivery_modes")).unwrap_or_else(|| "-".to_string()),
    );
    println!(
        "draft publish: asset_id={} grant_id={} asset_confirmed={} grant_confirmed={}",
        json_str(draft.get("last_published_asset_id")).unwrap_or("-"),
        json_str(draft.get("last_published_grant_id")).unwrap_or("-"),
        json_bool(seller.get("asset_preview_confirmed")),
        json_bool(seller.get("grant_preview_confirmed")),
    );
    let last_asset = draft.get("last_published_asset").unwrap_or(&Value::Null);
    if !last_asset.is_null() {
        println!(
            "asset nostr: listing={} listing_event={} listing_relay={}",
            json_str(
                last_asset
                    .get("nostr_publications")
                    .and_then(|value| value.get("ds_listing"))
                    .and_then(|value| value.get("coordinate"))
            )
            .unwrap_or("-"),
            json_str(
                last_asset
                    .get("nostr_publications")
                    .and_then(|value| value.get("ds_listing"))
                    .and_then(|value| value.get("event_id"))
            )
            .unwrap_or("-"),
            json_str(
                last_asset
                    .get("nostr_publications")
                    .and_then(|value| value.get("ds_listing"))
                    .and_then(|value| value.get("relay_url"))
            )
            .unwrap_or("-"),
        );
    }
    let last_grant = draft.get("last_published_grant").unwrap_or(&Value::Null);
    if !last_grant.is_null() {
        println!(
            "grant nostr: offer={} offer_event={} offer_relay={}",
            json_str(
                last_grant
                    .get("nostr_publications")
                    .and_then(|value| value.get("ds_offer"))
                    .and_then(|value| value.get("coordinate"))
            )
            .unwrap_or("-"),
            json_str(
                last_grant
                    .get("nostr_publications")
                    .and_then(|value| value.get("ds_offer"))
                    .and_then(|value| value.get("event_id"))
            )
            .unwrap_or("-"),
            json_str(
                last_grant
                    .get("nostr_publications")
                    .and_then(|value| value.get("ds_offer"))
                    .and_then(|value| value.get("relay_url"))
            )
            .unwrap_or("-"),
        );
    }
    for asset in seller
        .get("published_assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(3)
    {
        println!(
            "inventory asset: id={} kind={} status={} listing={}",
            json_str(asset.get("asset_id")).unwrap_or("-"),
            json_str(asset.get("asset_kind")).unwrap_or("-"),
            json_str(asset.get("status")).unwrap_or("-"),
            json_str(asset.get("ds_listing_coordinate")).unwrap_or("-"),
        );
    }
    for grant in seller
        .get("published_grants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(3)
    {
        println!(
            "inventory grant: id={} asset={} status={} offer={}",
            json_str(grant.get("grant_id")).unwrap_or("-"),
            json_str(grant.get("asset_id")).unwrap_or("-"),
            json_str(grant.get("status")).unwrap_or("-"),
            json_str(grant.get("ds_offer_coordinate")).unwrap_or("-"),
        );
    }
    let blockers = draft
        .get("readiness_blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if blockers.is_empty() {
        println!("draft blockers: none");
    } else {
        println!("draft blockers:");
        for blocker in blockers {
            let code = json_str(blocker.get("code")).unwrap_or("-");
            let message = json_str(blocker.get("message")).unwrap_or("-");
            println!("  {code}: {message}");
        }
    }
    if let Some(latest_request) = seller.get("latest_incoming_request") {
        if !latest_request.is_null() {
            print_data_market_request_summary("latest request", latest_request);
        }
    }
    if let Some(buyer) = payload.get("buyer") {
        println!(
            "buyer: load_state={} buyer_id={} selected_asset={} selected_listing={} selected_offer={} status={}",
            json_str(buyer.get("load_state")).unwrap_or("-"),
            json_str(buyer.get("local_buyer_id")).unwrap_or("-"),
            json_str(buyer.get("selected_asset_id")).unwrap_or("-"),
            json_str(buyer.get("selected_listing_coordinate")).unwrap_or("-"),
            json_str(buyer.get("selected_offer_coordinate")).unwrap_or("-"),
            json_str(buyer.get("status_line")).unwrap_or("-"),
        );
        if let Some(last_action) = json_str(buyer.get("last_action")) {
            println!("buyer last action: {last_action}");
        }
        if let Some(last_error) = json_str(buyer.get("last_error")) {
            println!("buyer last error: {last_error}");
        }
        if let Some(draft) = buyer.get("derived_request_draft") {
            if !draft.is_null() {
                println!(
                    "buyer draft: asset_ref={} asset_id={} listing={} offer={} provider={} grant={} bid_sats={} delivery_mode={} preview_posture={}",
                    json_str(draft.get("asset_ref")).unwrap_or("-"),
                    json_str(draft.get("asset_id")).unwrap_or("-"),
                    json_str(draft.get("listing_coordinate")).unwrap_or("-"),
                    json_str(draft.get("offer_coordinate")).unwrap_or("-"),
                    json_str(draft.get("provider_id")).unwrap_or("-"),
                    json_str(draft.get("offer_grant_id")).unwrap_or("-"),
                    draft
                        .get("bid_sats")
                        .and_then(Value::as_u64)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    json_str(draft.get("delivery_mode")).unwrap_or("-"),
                    json_str(draft.get("preview_posture")).unwrap_or("-"),
                );
            }
        }
        if let Some(latest_request) = buyer.get("latest_request") {
            if !latest_request.is_null() {
                println!(
                    "buyer request: id={} status={} provider={} feedback={} result={} payment_pointer={}",
                    json_str(latest_request.get("request_id")).unwrap_or("-"),
                    json_str(latest_request.get("status")).unwrap_or("-"),
                    json_str(
                        latest_request
                            .get("winning_provider_pubkey")
                            .or_else(|| latest_request.get("last_provider_pubkey"))
                    )
                    .unwrap_or("-"),
                    json_str(latest_request.get("last_feedback_status")).unwrap_or("-"),
                    json_str(latest_request.get("last_result_event_id")).unwrap_or("-"),
                    json_str(latest_request.get("last_payment_pointer")).unwrap_or("-"),
                );
            }
        }
    }
    if let Some(market) = payload.get("market") {
        println!(
            "market: load_state={} assets={} grants={} relay_listings={} relay_offers={} deliveries={} revocations={} refreshed_at_ms={}",
            json_str(market.get("load_state")).unwrap_or("-"),
            market
                .get("asset_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            market
                .get("grant_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            market
                .get("relay_listing_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            market
                .get("relay_offer_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            market
                .get("delivery_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            market
                .get("revocation_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            market
                .get("last_refreshed_at_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        );
        if let Some(last_action) = json_str(market.get("last_action")) {
            println!("market last action: {last_action}");
        }
        if let Some(last_error) = json_str(market.get("last_error")) {
            println!("market last error: {last_error}");
        }
        for listing in market
            .get("relay_listings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(3)
        {
            println!(
                "relay listing: coordinate={} title={} draft={} linked_asset={}",
                json_str(listing.get("coordinate")).unwrap_or("-"),
                json_str(listing.get("title")).unwrap_or("-"),
                json_bool(listing.get("draft")),
                json_str(listing.get("linked_asset_id")).unwrap_or("-"),
            );
        }
        for offer in market
            .get("relay_offers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(3)
        {
            println!(
                "relay offer: coordinate={} listing={} status={} linked_grant={}",
                json_str(offer.get("coordinate")).unwrap_or("-"),
                json_str(offer.get("listing_coordinate")).unwrap_or("-"),
                json_str(offer.get("status")).unwrap_or("-"),
                json_str(offer.get("linked_grant_id")).unwrap_or("-"),
            );
        }
    }
}

fn print_data_market_request_summary(label: &str, request: &Value) {
    println!(
        "{label}: id={} requester={} eval={} matched_asset={} matched_grant={} listing={} offer={} price_sats={}",
        json_str(request.get("request_id")).unwrap_or("-"),
        json_str(request.get("requester")).unwrap_or("-"),
        json_str(request.get("evaluation_disposition")).unwrap_or("-"),
        json_str(request.get("matched_asset_id")).unwrap_or("-"),
        json_str(request.get("matched_grant_id")).unwrap_or("-"),
        json_str(request.get("matched_listing_coordinate")).unwrap_or("-"),
        json_str(request.get("matched_offer_coordinate")).unwrap_or("-"),
        request
            .get("required_price_sats")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    println!(
        "{label} states: payment={} delivery={} revocation={}",
        json_str(request.get("payment").and_then(|value| value.get("state"))).unwrap_or("-"),
        json_str(request.get("delivery").and_then(|value| value.get("state"))).unwrap_or("-"),
        json_str(
            request
                .get("revocation")
                .and_then(|value| value.get("state"))
        )
        .unwrap_or("-"),
    );
}

fn resolve_local_data_market_ref(value: &str) -> Result<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("delivery reference cannot be empty");
    }
    if let Some(path) = trimmed.strip_prefix("file://localhost/") {
        return Ok(PathBuf::from(format!("/{path}")));
    }
    if let Some(path) = trimmed.strip_prefix("file:///") {
        return Ok(PathBuf::from(format!("/{path}")));
    }
    if trimmed.contains("://") {
        bail!(
            "unsupported delivery reference scheme in {trimmed}; current headless consume only supports file:// or local paths"
        );
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .context("resolve current working directory for relative delivery reference")?
            .join(path))
    }
}

fn is_local_data_market_ref(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("file://localhost/")
        || trimmed.starts_with("file:///")
        || !trimmed.contains("://")
}

fn normalize_delivery_digest(value: Option<&str>) -> Result<Option<String>> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = raw.strip_prefix("sha256:").unwrap_or(raw);
    if normalized.len() != 64
        || !normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        bail!("delivery digest must be a sha256 hex string, got {raw}");
    }
    Ok(Some(normalized.to_ascii_lowercase()))
}

fn data_market_sha256_hex(bytes: &[u8]) -> String {
    sha256::Hash::hash(bytes).to_string()
}

fn verify_delivery_bytes_digest(
    bytes: &[u8],
    expected_digest: Option<&str>,
    source_ref: &str,
) -> Result<Option<String>> {
    let expected = normalize_delivery_digest(expected_digest)?;
    let Some(expected) = expected else {
        return Ok(None);
    };
    let calculated = data_market_sha256_hex(bytes);
    if calculated != expected {
        bail!(
            "delivery digest mismatch for {source_ref}: expected sha256:{expected}, got sha256:{calculated}"
        );
    }
    Ok(Some(format!("sha256:{calculated}")))
}

fn private_key_bytes(identity: &nostr::NostrIdentity) -> Result<[u8; 32]> {
    let decoded = hex::decode(identity.private_key_hex.as_str())
        .with_context(|| "decode buyer private key hex for delivery decryption")?;
    decoded
        .try_into()
        .map_err(|_| anyhow!("buyer private key has invalid length"))
}

fn compressed_pubkey_from_xonly_hex(pubkey: &str) -> Result<Vec<u8>> {
    let decoded =
        hex::decode(pubkey.trim()).with_context(|| format!("decode sender pubkey hex {pubkey}"))?;
    if decoded.len() != 32 {
        bail!("sender pubkey must be 32-byte hex, got {}", decoded.len());
    }
    let mut compressed = Vec::with_capacity(33);
    compressed.push(0x02);
    compressed.extend_from_slice(decoded.as_slice());
    Ok(compressed)
}

fn infer_remote_file_name(reference: &str, content_type: Option<&str>) -> String {
    let from_url = Url::parse(reference).ok().and_then(|url| {
        url.path_segments().and_then(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .next_back()
                .map(str::to_string)
        })
    });
    from_url
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| match content_type.unwrap_or_default() {
            value if value.contains("json") => "payload.json".to_string(),
            value if value.contains("text/plain") => "payload.txt".to_string(),
            _ => "payload.bin".to_string(),
        })
}

fn load_target_identity(target: &ResolvedTarget) -> Result<nostr::NostrIdentity> {
    let identity_path = target
        .identity_path
        .clone()
        .or_else(|| nostr::identity_mnemonic_path().ok())
        .ok_or_else(|| {
            anyhow!("could not resolve an identity mnemonic path for remote delivery decryption")
        })?;
    nostr::load_identity_from_path(identity_path.as_path()).with_context(|| {
        format!(
            "load buyer identity mnemonic for remote delivery decryption from {}",
            identity_path.display()
        )
    })
}

fn is_probable_delivery_ref(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && !trimmed.contains('\n')
        && (trimmed.contains("://")
            || trimmed.starts_with('/')
            || trimmed.starts_with("./")
            || trimmed.starts_with("../"))
}

fn parse_delivery_pointer_text(
    raw: &str,
    target: &ResolvedTarget,
    inherited_digest: Option<&str>,
    inherited_manifest_refs: &[String],
) -> Result<Option<DataMarketResolvedDeliveryLocator>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if let Ok(envelope) = serde_json::from_str::<DataMarketDeliveryPointerEnvelope>(trimmed) {
        let schema = envelope.schema_version.as_deref().unwrap_or_default();
        if envelope.ciphertext.is_some()
            || schema.eq_ignore_ascii_case("oa.data_market.encrypted_pointer.v1")
        {
            let sender_pubkey = envelope
                .sender_pubkey
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("encrypted delivery pointer is missing sender_pubkey"))?;
            let ciphertext = envelope
                .ciphertext
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("encrypted delivery pointer is missing ciphertext"))?;
            let identity = load_target_identity(target)?;
            let private_key = private_key_bytes(&identity)?;
            let sender_pubkey = compressed_pubkey_from_xonly_hex(sender_pubkey)?;
            let plaintext =
                nostr::nip44::decrypt(&private_key, sender_pubkey.as_slice(), ciphertext)
                    .map_err(|error| anyhow!("decrypt encrypted delivery pointer: {error}"))?;
            let nested_digest = envelope
                .delivery_digest
                .as_deref()
                .or(envelope.digest.as_deref())
                .or(inherited_digest);
            let nested_manifest_refs = if envelope.manifest_refs.is_empty() {
                inherited_manifest_refs.to_vec()
            } else {
                envelope.manifest_refs
            };
            return parse_delivery_pointer_text(
                plaintext.as_str(),
                target,
                nested_digest,
                nested_manifest_refs.as_slice(),
            );
        }

        let delivery_ref = envelope
            .delivery_ref
            .as_deref()
            .or(envelope.ref_value.as_deref())
            .or(envelope.url.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(delivery_ref) = delivery_ref {
            let delivery_digest = envelope
                .delivery_digest
                .as_deref()
                .or(envelope.digest.as_deref())
                .or(inherited_digest)
                .map(str::to_string);
            let manifest_refs = if envelope.manifest_refs.is_empty() {
                inherited_manifest_refs.to_vec()
            } else {
                envelope.manifest_refs
            };
            return Ok(Some(DataMarketResolvedDeliveryLocator {
                delivery_ref: delivery_ref.to_string(),
                delivery_digest,
                manifest_refs,
            }));
        }
    }

    if is_probable_delivery_ref(trimmed) {
        return Ok(Some(DataMarketResolvedDeliveryLocator {
            delivery_ref: trimmed.to_string(),
            delivery_digest: inherited_digest.map(str::to_string),
            manifest_refs: inherited_manifest_refs.to_vec(),
        }));
    }

    Ok(None)
}

fn parse_giftwrap_delivery_ref(value: &str) -> Result<(String, Vec<String>)> {
    let trimmed = value.trim();
    let parsed = Url::parse(trimmed)
        .with_context(|| format!("parse giftwrap delivery reference {trimmed}"))?;
    match parsed.scheme() {
        "giftwrap" | "nostr+giftwrap" => {}
        scheme => bail!("unsupported giftwrap delivery scheme {scheme}"),
    }
    let event_id = parsed
        .host_str()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let path = parsed.path().trim_matches('/');
            (!path.is_empty()).then_some(path)
        })
        .ok_or_else(|| anyhow!("giftwrap delivery ref is missing an event id"))?
        .to_string();
    if event_id.len() != 64
        || !event_id
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        bail!("giftwrap delivery ref must carry a 64-character event id");
    }
    let relay_urls = parsed
        .query_pairs()
        .filter(|(key, _)| key == "relay")
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    Ok((event_id, relay_urls))
}

fn parse_giftwrapped_delivery_pointer(
    event: &nostr::Event,
    target: &ResolvedTarget,
    inherited_digest: Option<&str>,
    inherited_manifest_refs: &[String],
) -> Result<DataMarketResolvedDeliveryLocator> {
    let identity = load_target_identity(target)?;
    let private_key = private_key_bytes(&identity)?;
    let rumor = nostr::nip59::unwrap_gift_wrap_full(event, &private_key)
        .map_err(|error| anyhow!("decrypt giftwrapped delivery pointer {}: {error}", event.id))?;
    parse_delivery_pointer_text(
        rumor.content.as_str(),
        target,
        inherited_digest,
        inherited_manifest_refs,
    )?
    .ok_or_else(|| {
        anyhow!(
            "giftwrapped delivery pointer {} did not contain a supported delivery ref or pointer envelope",
            event.id
        )
    })
}

fn fetch_remote_delivery_bytes(http: &Client, url: &str) -> Result<(Vec<u8>, Option<String>)> {
    let mut response = http
        .get(url)
        .send()
        .with_context(|| format!("download remote delivery ref {url}"))?;
    if !response.status().is_success() {
        bail!(
            "remote delivery download failed for {url}: status {}",
            response.status()
        );
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .with_context(|| format!("read remote delivery body from {url}"))?;
    Ok((bytes, content_type))
}

fn resolve_delivery_source(
    client: &DesktopControlClient,
    locator: DataMarketResolvedDeliveryLocator,
    require_remote_digest: bool,
    depth_remaining: usize,
) -> Result<(
    DataMarketResolvedPayloadSource,
    DataMarketResolvedDeliveryLocator,
)> {
    if depth_remaining == 0 {
        bail!("delivery pointer resolution exceeded the maximum recursion depth");
    }
    let delivery_ref = locator.delivery_ref.trim().to_string();
    if delivery_ref.is_empty() {
        bail!("delivery reference cannot be empty");
    }
    if is_local_data_market_ref(delivery_ref.as_str()) {
        let path = resolve_local_data_market_ref(delivery_ref.as_str())?;
        return Ok((DataMarketResolvedPayloadSource::LocalPath(path), locator));
    }
    if delivery_ref.starts_with("http://") || delivery_ref.starts_with("https://") {
        let (bytes, content_type) =
            fetch_remote_delivery_bytes(&client.http, delivery_ref.as_str())?;
        if let Ok(text) = std::str::from_utf8(bytes.as_slice())
            && let Some(nested) = parse_delivery_pointer_text(
                text,
                &client.target,
                locator.delivery_digest.as_deref(),
                locator.manifest_refs.as_slice(),
            )?
        {
            return resolve_delivery_source(
                client,
                nested,
                require_remote_digest,
                depth_remaining.saturating_sub(1),
            );
        }
        if require_remote_digest
            && normalize_delivery_digest(locator.delivery_digest.as_deref())?.is_none()
        {
            bail!(
                "remote delivery ref {} requires delivery_digest so the payload can be verified before materialization",
                delivery_ref
            );
        }
        let verified_digest = verify_delivery_bytes_digest(
            bytes.as_slice(),
            locator.delivery_digest.as_deref(),
            delivery_ref.as_str(),
        )?;
        return Ok((
            DataMarketResolvedPayloadSource::RemoteFile {
                source_ref: delivery_ref.clone(),
                suggested_name: infer_remote_file_name(
                    delivery_ref.as_str(),
                    content_type.as_deref(),
                ),
                bytes,
            },
            DataMarketResolvedDeliveryLocator {
                delivery_ref,
                delivery_digest: verified_digest.or(locator.delivery_digest),
                manifest_refs: locator.manifest_refs,
            },
        ));
    }
    if delivery_ref.starts_with("giftwrap:") || delivery_ref.starts_with("nostr+giftwrap:") {
        let (event_id, relay_urls) = parse_giftwrap_delivery_ref(delivery_ref.as_str())?;
        let relay_urls = resolve_data_market_relay_urls(client, relay_urls.as_slice())?;
        let fetched = fetch_relay_event_by_id(event_id.as_str(), relay_urls.as_slice(), 15_000)?;
        let event: nostr::Event = serde_json::from_value(fetched.event_json.clone())
            .with_context(|| format!("decode giftwrap event {}", fetched.event_id))?;
        let nested = parse_giftwrapped_delivery_pointer(
            &event,
            &client.target,
            locator.delivery_digest.as_deref(),
            locator.manifest_refs.as_slice(),
        )?;
        return resolve_delivery_source(
            client,
            nested,
            require_remote_digest,
            depth_remaining.saturating_sub(1),
        );
    }
    bail!(
        "unsupported delivery reference scheme in {}; supported remote refs are http(s) payloads, encrypted pointer documents, and giftwrap:// event ids",
        delivery_ref
    )
}

fn copy_manifest_ref_to_output(
    client: &DesktopControlClient,
    manifest_ref: &str,
    target_path: &Path,
) -> Result<()> {
    let locator = DataMarketResolvedDeliveryLocator {
        delivery_ref: manifest_ref.to_string(),
        delivery_digest: None,
        manifest_refs: Vec::new(),
    };
    let (source, _) = resolve_delivery_source(client, locator, false, 4)?;
    match source {
        DataMarketResolvedPayloadSource::LocalPath(path) => {
            if path.is_dir() {
                copy_dir_recursive(path.as_path(), target_path)?;
            } else if path.is_file() {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("create parent {}", parent.display()))?;
                }
                fs::copy(path.as_path(), target_path).with_context(|| {
                    format!(
                        "copy manifest {} -> {}",
                        path.display(),
                        target_path.display()
                    )
                })?;
            } else {
                bail!(
                    "manifest source {} is neither a regular file nor a directory",
                    path.display()
                );
            }
        }
        DataMarketResolvedPayloadSource::RemoteFile { bytes, .. } => {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create parent {}", parent.display()))?;
            }
            let mut handle = fs::File::create(target_path)
                .with_context(|| format!("create manifest {}", target_path.display()))?;
            handle
                .write_all(bytes.as_slice())
                .with_context(|| format!("write manifest {}", target_path.display()))?;
        }
    }
    Ok(())
}

fn ensure_clean_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    match fs::metadata(path) {
        Ok(metadata) => {
            if !overwrite {
                bail!(
                    "output directory {} already exists; pass --overwrite to replace it",
                    path.display()
                );
            }
            if metadata.is_dir() {
                fs::remove_dir_all(path)
                    .with_context(|| format!("remove existing output dir {}", path.display()))?;
            } else {
                fs::remove_file(path)
                    .with_context(|| format!("remove existing output file {}", path.display()))?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("inspect output path {}", path.display()));
        }
    }
    fs::create_dir_all(path).with_context(|| format!("create output dir {}", path.display()))
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("create directory {}", destination.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("read directory {}", source.display()))?
    {
        let entry = entry.with_context(|| format!("read entry in {}", source.display()))?;
        let entry_type = entry
            .file_type()
            .with_context(|| format!("inspect {}", entry.path().display()))?;
        let target_path = destination.join(entry.file_name());
        if entry_type.is_dir() {
            copy_dir_recursive(entry.path().as_path(), target_path.as_path())?;
        } else if entry_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create parent {}", parent.display()))?;
            }
            fs::copy(entry.path(), target_path.as_path()).with_context(|| {
                format!(
                    "copy {} -> {}",
                    entry.path().display(),
                    target_path.display()
                )
            })?;
        } else {
            bail!(
                "unsupported non-file, non-directory entry in delivery payload: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn materialize_data_market_delivery(
    client: &DesktopControlClient,
    payload: &Value,
    output_dir: &Path,
    overwrite: bool,
) -> Result<DataMarketConsumedDeliverySummary> {
    let delivery = payload
        .get("delivery")
        .ok_or_else(|| anyhow!("missing delivery payload"))?;
    let delivery_bundle_id = json_str(delivery.get("delivery_bundle_id"))
        .ok_or_else(|| anyhow!("delivery payload missing delivery_bundle_id"))?
        .to_string();
    let grant_id = json_str(delivery.get("grant_id"))
        .ok_or_else(|| anyhow!("delivery payload missing grant_id"))?
        .to_string();
    let asset_id = json_str(delivery.get("asset_id"))
        .ok_or_else(|| anyhow!("delivery payload missing asset_id"))?
        .to_string();
    let provider_id = json_str(delivery.get("provider_id"))
        .ok_or_else(|| anyhow!("delivery payload missing provider_id"))?
        .to_string();
    let consumer_id = json_str(delivery.get("consumer_id"))
        .ok_or_else(|| anyhow!("delivery payload missing consumer_id"))?
        .to_string();
    let delivery_ref = json_str(delivery.get("delivery_ref"))
        .ok_or_else(|| anyhow!("delivery payload missing delivery_ref"))?
        .to_string();
    let delivery_digest = json_str(delivery.get("delivery_digest")).map(str::to_string);
    let manifest_refs = delivery
        .get("manifest_refs")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ensure_clean_output_dir(output_dir, overwrite)?;

    let payload_root = output_dir.join("payload");
    let locator = DataMarketResolvedDeliveryLocator {
        delivery_ref: delivery_ref.clone(),
        delivery_digest: delivery_digest.clone(),
        manifest_refs: manifest_refs.clone(),
    };
    let (payload_source, resolved_locator) = resolve_delivery_source(client, locator, true, 8)?;
    let (payload_source_kind, payload_output_path) = match payload_source {
        DataMarketResolvedPayloadSource::LocalPath(source_path) => {
            if !source_path.exists() {
                bail!(
                    "delivery source {} does not exist locally",
                    source_path.display()
                );
            }
            if source_path.is_dir() {
                copy_dir_recursive(source_path.as_path(), payload_root.as_path())?;
                ("directory".to_string(), payload_root.display().to_string())
            } else if source_path.is_file() {
                if let Some(expected_digest) = resolved_locator.delivery_digest.as_deref() {
                    let bytes = fs::read(source_path.as_path()).with_context(|| {
                        format!("read local delivery file {}", source_path.display())
                    })?;
                    let _ = verify_delivery_bytes_digest(
                        bytes.as_slice(),
                        Some(expected_digest),
                        source_path.display().to_string().as_str(),
                    )?;
                }
                fs::create_dir_all(payload_root.as_path())
                    .with_context(|| format!("create payload dir {}", payload_root.display()))?;
                let file_name = source_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow!("delivery source file has no usable basename"))?;
                let target_path = payload_root.join(file_name);
                fs::copy(source_path.as_path(), target_path.as_path()).with_context(|| {
                    format!(
                        "copy {} -> {}",
                        source_path.display(),
                        target_path.display()
                    )
                })?;
                ("file".to_string(), target_path.display().to_string())
            } else {
                bail!(
                    "delivery source {} is neither a regular file nor a directory",
                    source_path.display()
                );
            }
        }
        DataMarketResolvedPayloadSource::RemoteFile {
            source_ref,
            suggested_name,
            bytes,
        } => {
            fs::create_dir_all(payload_root.as_path())
                .with_context(|| format!("create payload dir {}", payload_root.display()))?;
            let target_path = payload_root.join(suggested_name);
            let mut handle = fs::File::create(target_path.as_path())
                .with_context(|| format!("create payload file {}", target_path.display()))?;
            handle
                .write_all(bytes.as_slice())
                .with_context(|| format!("write payload file {}", target_path.display()))?;
            (
                format!("remote_file:{source_ref}"),
                target_path.display().to_string(),
            )
        }
    };

    let manifests_root = output_dir.join("manifests");
    let mut copied_manifest_paths = Vec::new();
    let mut unresolved_manifest_refs = Vec::new();
    for (index, manifest_ref) in resolved_locator.manifest_refs.iter().enumerate() {
        fs::create_dir_all(manifests_root.as_path())
            .with_context(|| format!("create manifests dir {}", manifests_root.display()))?;
        let base_name = if is_local_data_market_ref(manifest_ref.as_str()) {
            resolve_local_data_market_ref(manifest_ref.as_str())
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "manifest".to_string())
        } else {
            infer_remote_file_name(manifest_ref.as_str(), None)
        };
        let target_path = manifests_root.join(format!("{index:02}-{base_name}"));
        if let Err(error) =
            copy_manifest_ref_to_output(client, manifest_ref.as_str(), target_path.as_path())
        {
            unresolved_manifest_refs.push(manifest_ref.clone());
            let _ = error;
            continue;
        }
        copied_manifest_paths.push(target_path.display().to_string());
    }

    let summary = DataMarketConsumedDeliverySummary {
        schema_version: "oa.data_market.consume.v1",
        selection_reason: json_str(payload.get("selection_reason")).map(str::to_string),
        resolved_request_id: json_str(payload.get("resolved_request_id")).map(str::to_string),
        delivery_bundle_id,
        grant_id,
        asset_id,
        provider_id,
        consumer_id,
        delivery_ref,
        delivery_digest: resolved_locator.delivery_digest.clone(),
        output_dir: output_dir.display().to_string(),
        payload_source_kind,
        payload_output_path,
        copied_manifest_paths,
        unresolved_manifest_refs,
        manifest_refs: resolved_locator.manifest_refs,
    };
    fs::write(
        output_dir.join("consumed-delivery.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&summary).context("encode consumed delivery summary")?
        ),
    )
    .with_context(|| {
        format!(
            "write consumed delivery summary {}",
            output_dir.join("consumed-delivery.json").display()
        )
    })?;
    Ok(summary)
}

fn print_data_market_consumed_delivery_text(summary: &DataMarketConsumedDeliverySummary) {
    println!(
        "consumed delivery: bundle_id={} grant_id={} asset_id={} request_id={} source_kind={}",
        summary.delivery_bundle_id,
        summary.grant_id,
        summary.asset_id,
        summary.resolved_request_id.as_deref().unwrap_or("-"),
        summary.payload_source_kind,
    );
    println!(
        "output: dir={} payload={} copied_manifests={} unresolved_manifests={}",
        summary.output_dir,
        summary.payload_output_path,
        summary.copied_manifest_paths.len(),
        summary.unresolved_manifest_refs.len(),
    );
    println!(
        "delivery refs: delivery_ref={} delivery_digest={}",
        summary.delivery_ref,
        summary.delivery_digest.as_deref().unwrap_or("-"),
    );
    if !summary.copied_manifest_paths.is_empty() {
        println!("copied manifests:");
        for path in &summary.copied_manifest_paths {
            println!("  {path}");
        }
    }
    if !summary.unresolved_manifest_refs.is_empty() {
        println!("unresolved manifest refs:");
        for reference in &summary.unresolved_manifest_refs {
            println!("  {reference}");
        }
    }
}

fn json_str(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn json_nested_str<'a>(value: Option<&'a Value>, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value?, |current, segment| current.get(*segment))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn json_bool(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

fn json_array_len(value: Option<&Value>) -> usize {
    value.and_then(Value::as_array).map(Vec::len).unwrap_or(0)
}

fn json_nested_u64(value: Option<&Value>, path: &[&str]) -> Option<u64> {
    path.iter()
        .try_fold(value?, |current, segment| current.get(*segment))
        .and_then(Value::as_u64)
}

fn json_joined_array(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_array).map(|entries| {
        entries
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    })
}

fn remote_training_track_summary(run: &Value) -> String {
    let family = json_nested_str(Some(run), &["track", "track_family"]).unwrap_or("-");
    let track_id = json_nested_str(Some(run), &["track", "track_id"]).unwrap_or("-");
    format!("{family}:{track_id}")
}

fn remote_training_primary_score_summary(run: &Value) -> String {
    json_nested_str(Some(run), &["primary_score", "score_summary"])
        .unwrap_or("-")
        .to_string()
}

fn remote_training_score_surface_summary(run: &Value) -> String {
    json_nested_str(Some(run), &["score_surface", "semantic_summary"])
        .unwrap_or("-")
        .to_string()
}

fn print_attnres_text(status: &DesktopControlAttnResStatus) {
    println!(
        "attnres: playback={} running={} view={} selected={} step={}/{} speed={}x open_instances={} active={}",
        status.summary.playback_state,
        status.summary.running,
        status.summary.selected_view,
        status
            .summary
            .selected_sublayer_label
            .as_deref()
            .unwrap_or("-"),
        status.summary.step,
        status.summary.max_steps,
        status.summary.speed_multiplier,
        status.summary.open_instances,
        status.summary.active
    );
    println!(
        "run: label={} status={} source={} model={} architecture={}",
        status.run_label,
        status.summary.run_status,
        status.source_badge,
        status.model_label,
        status.architecture_label
    );
    println!(
        "metrics: loss={:.4} ema={:.4} avg_selectivity={:.1}% active_block={} current_block_fill={} completed_blocks={}",
        status.training_loss,
        status.ema_loss,
        status.avg_selectivity * 100.0,
        status.active_block,
        status.current_block_fill,
        status.completed_blocks
    );
    println!(
        "timing: train_ms={:.1} diag_ms={:.1} avg_loop_ms={:.1} steps_per_second={:.1} eta_seconds={:.1}",
        status.last_train_ms,
        status.last_diag_ms,
        status.avg_loop_ms,
        status.steps_per_second,
        status.eta_seconds
    );
    if let Some(selected) = status.selected_sublayer.as_ref() {
        println!(
            "selected: index={} label={} kind={} target_block={} dominant={} ({:.1}%) cache={:.1}% partial={:.1}% query_norm={:.3} selectivity={:.1}%",
            selected.sublayer_index,
            selected.label,
            selected.kind_label,
            selected.target_block,
            selected.dominant_source_label,
            selected.dominant_weight * 100.0,
            selected.cache_mass * 100.0,
            selected.partial_mass * 100.0,
            selected.query_norm,
            selected.selectivity * 100.0
        );
        println!("selected route: {}", selected.route_note);
    }
    println!(
        "parity: hidden={} diff={:.6} logit={} diff={:.6} merge_partial={:.1}% merge_cache={:.1}% cache_fill={:.1}%",
        status.inference.hidden_parity_label,
        status.inference.hidden_max_abs_diff,
        status.inference.logit_parity_label,
        status.inference.logit_max_abs_diff,
        status.inference.partial_merge_share * 100.0,
        status.inference.cache_merge_share * 100.0,
        status.inference.block_cache_fill_share * 100.0
    );
    if let Some(last_action) = status.summary.last_action.as_deref() {
        println!("last action: {last_action}");
    }
    if let Some(last_error) = status.summary.last_error.as_deref() {
        println!("last error: {last_error}");
    }
    for block in status.block_summaries.iter().take(4) {
        println!(
            "block: index={} avg_selectivity={:.1}% avg_query_norm={:.3} sublayers={}",
            block.block_index,
            block.avg_selectivity * 100.0,
            block.avg_query_norm,
            block.sublayers
        );
    }
    for event in status.recent_events.iter().rev().take(6) {
        println!("event: {event}");
    }
}

fn print_tassadar_text(status: &DesktopControlTassadarStatus) {
    println!(
        "tassadar: playback={} running={} source={} case={} view={} update={} speed={} window={} open_instances={} active={}",
        status.summary.playback_state,
        status.summary.running,
        status.summary.selected_source_mode,
        status.summary.selected_source_label,
        status.summary.selected_view,
        status.summary.selected_update,
        status.summary.speed_multiplier,
        status.summary.trace_chunk_size,
        status.summary.open_instances,
        status.summary.active
    );
    if let Some(replay_family) = status.summary.selected_replay_family.as_deref() {
        println!(
            "explorer: family={} family_index={}/{} case_index={}/{}",
            replay_family,
            status.summary.replay_family_position,
            status.summary.replay_family_count.max(1),
            status.summary.replay_family_case_position,
            status.summary.replay_family_case_count.max(1)
        );
    }
    println!(
        "surface: badge={} kind={} family={} subject={} status={}",
        status.source_badge,
        status.source_kind,
        status.family_label,
        status.subject_label,
        status.status_label
    );
    println!(
        "route: state={} requested_decode={} effective_decode={} detail={}",
        status.route_state_label.as_deref().unwrap_or("-"),
        status.requested_decode_mode.as_deref().unwrap_or("-"),
        status.effective_decode_mode.as_deref().unwrap_or("-"),
        status
            .route_detail
            .as_deref()
            .unwrap_or(status.detail_label.as_str())
    );
    println!(
        "program: program_id={} wasm_profile_id={} artifact_ref={}",
        status.program_id.as_deref().unwrap_or("-"),
        status.wasm_profile_id.as_deref().unwrap_or("-"),
        status.artifact_ref.as_deref().unwrap_or("-")
    );
    println!("outputs: {:?}", status.final_outputs);
    if !status.metric_chips.is_empty() {
        println!("metric chips:");
        for chip in &status.metric_chips {
            println!("  {}={} ({})", chip.label, chip.value, chip.tone);
        }
    }
    if !status.fact_lines.is_empty() {
        println!("fact lines:");
        for fact in &status.fact_lines {
            println!("  {}: {}", fact.label, fact.value);
        }
    }
    if !status.local_events.is_empty() {
        println!("local events:");
        for line in status.local_events.iter().rev().take(6) {
            println!("  {line}");
        }
    }
    if !status.recent_events.is_empty() {
        println!("snapshot events:");
        for line in status.recent_events.iter().rev().take(6) {
            println!("  {line}");
        }
    }
}

fn print_perf_text(payload: &Value) {
    let rolling_fps = payload
        .get("rolling_fps")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let rolling_interval_ms = payload
        .get("rolling_frame_interval_ms")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let last_frame_ms = payload
        .get("last_report")
        .and_then(|report| report.get("total_cpu_ms"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    println!(
        "perf: {:.1} fps rolling // {:.2}ms cadence // {:.2}ms last frame cpu",
        rolling_fps, rolling_interval_ms, last_frame_ms
    );
    print_perf_summary_block("top pane paints", payload.get("top_pane_paints"));
    print_perf_summary_block("top runtime pumps", payload.get("top_runtime_pumps"));
    print_perf_summary_block("top snapshot timings", payload.get("top_snapshot_timings"));
}

fn print_perf_summary_block(label: &str, value: Option<&Value>) {
    println!("{label}:");
    let Some(entries) = value.and_then(Value::as_array) else {
        println!("  []");
        return;
    };
    if entries.is_empty() {
        println!("  []");
        return;
    }
    for entry in entries {
        println!(
            "  {}",
            serde_json::to_string(entry).unwrap_or_else(|_| entry.to_string())
        );
    }
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
                    identity_path: None,
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
        "local runtime: lane={} policy={} posture={} ready={} go_online_ready={} workbench={} text={} streaming={} structured={} model_management={} sessions={} action={} enabled={} model={} backend={} load={}",
        snapshot.local_runtime.lane.as_deref().unwrap_or("-"),
        snapshot.local_runtime.policy,
        local_runtime_execution_posture_label(snapshot.local_runtime.diagnostics.posture),
        snapshot.local_runtime.runtime_ready,
        snapshot.local_runtime.go_online_ready,
        snapshot.local_runtime.workbench_label,
        snapshot.local_runtime.supports_run_text,
        snapshot.local_runtime.supports_streaming,
        snapshot.local_runtime.supports_structured,
        snapshot.local_runtime.supports_model_management,
        snapshot.local_runtime.supports_sessions,
        snapshot.local_runtime.action,
        snapshot.local_runtime.action_enabled,
        snapshot.local_runtime.model_label,
        snapshot.local_runtime.backend_label,
        snapshot.local_runtime.load_label
    );
    println!(
        "attnres: playback={} running={} view={} selected={} step={}/{} speed={}x open_instances={} active={}",
        snapshot.attnres_lab.playback_state,
        snapshot.attnres_lab.running,
        snapshot.attnres_lab.selected_view,
        snapshot
            .attnres_lab
            .selected_sublayer_label
            .as_deref()
            .unwrap_or("-"),
        snapshot.attnres_lab.step,
        snapshot.attnres_lab.max_steps,
        snapshot.attnres_lab.speed_multiplier,
        snapshot.attnres_lab.open_instances,
        snapshot.attnres_lab.active
    );
    println!(
        "apple fm: ready={} reachable={} model={} adapters={} attached={}",
        snapshot.apple_fm.ready,
        snapshot.apple_fm.reachable,
        snapshot.apple_fm.ready_model.as_deref().unwrap_or("-"),
        snapshot.apple_fm.loaded_adapters.len(),
        snapshot
            .apple_fm
            .active_session_adapter
            .as_ref()
            .map(|adapter| adapter.adapter_id.as_str())
            .unwrap_or("-")
    );
    println!(
        "gpt-oss: detected={} backend={} ready={} busy={} loaded={} artifact_present={} model={}",
        snapshot.gpt_oss.detected,
        snapshot.gpt_oss.backend.as_deref().unwrap_or("-"),
        snapshot.gpt_oss.ready,
        snapshot.gpt_oss.busy,
        snapshot.gpt_oss.loaded,
        snapshot.gpt_oss.artifact_present,
        snapshot.gpt_oss.ready_model.as_deref().unwrap_or("-")
    );
    println!(
        "wallet: balance={} network={} status={} reconciling={} withdraw_ready={}",
        if snapshot.wallet.balance_known {
            format!("{} sats", snapshot.wallet.balance_sats)
        } else {
            "unknown".to_string()
        },
        snapshot.wallet.network,
        snapshot.wallet.network_status,
        snapshot.wallet.balance_reconciling,
        snapshot.wallet.can_withdraw
    );
    println!(
        "tailnet: available={} tailnet={} online_devices={}/{} health={} last_error={}",
        snapshot.tailnet.available,
        snapshot.tailnet.current_tailnet.as_deref().unwrap_or("-"),
        snapshot.tailnet.online_device_count,
        snapshot.tailnet.device_count,
        if snapshot.tailnet.health.is_empty() {
            "-".to_string()
        } else {
            snapshot.tailnet.health.join(" | ")
        },
        snapshot.tailnet.last_error.as_deref().unwrap_or("-")
    );
    println!(
        "tunnels: available={} approved_services={} active_services={} open_tunnels={}",
        snapshot.tunnels.available,
        snapshot.tunnels.approved_service_count,
        snapshot.tunnels.active_service_count,
        snapshot.tunnels.open_tunnel_count
    );
    for line in inventory_status_lines(snapshot) {
        println!("{line}");
    }
    for line in buyer_procurement_status_lines(snapshot) {
        println!("{line}");
    }
    println!(
        "cluster: available={} topology={} members={}",
        snapshot.cluster.available, snapshot.cluster.topology_label, snapshot.cluster.member_count
    );
    println!(
        "sandbox: available={} profiles={}/{} jobs={} active_jobs={}",
        snapshot.sandbox.available,
        snapshot.sandbox.ready_profile_count,
        snapshot.sandbox.declared_profile_count,
        snapshot.sandbox.job_count,
        snapshot.sandbox.active_job_count
    );
    for line in training_status_lines(snapshot) {
        println!("{line}");
    }
    for line in remote_training_status_lines(snapshot) {
        println!("{line}");
    }
    println!(
        "proofs: available={} source={} pending={} accepted={} rejected={} challenged={} settlements_open={} settlements_terminal={}",
        snapshot.proofs.available,
        snapshot.proofs.source,
        snapshot.proofs.pending_count,
        snapshot.proofs.accepted_count,
        snapshot.proofs.rejected_count,
        snapshot.proofs.challenged_count,
        snapshot.proofs.settlement_open_count,
        snapshot.proofs.settlement_terminal_count
    );
    println!(
        "challenges: available={} source={} open={} verified={} rejected={} timed_out={}",
        snapshot.challenges.available,
        snapshot.challenges.source,
        snapshot.challenges.open_count,
        snapshot.challenges.verified_count,
        snapshot.challenges.rejected_count,
        snapshot.challenges.timed_out_count
    );
    println!(
        "buy mode: enabled={} approved_budget={} sats cadence={} next_dispatch={}",
        snapshot.buy_mode.enabled,
        snapshot.buy_mode.approved_budget_sats,
        format_buy_mode_cadence(snapshot),
        format_buy_mode_next_dispatch(snapshot)
    );
    println!(
        "buy mode target: selected={} relay={} model={} roster={}/{} blocked_code={} blocked_reason={}",
        snapshot
            .buy_mode
            .target_selection
            .selected_peer_pubkey
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buy_mode
            .target_selection
            .selected_relay_url
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buy_mode
            .target_selection
            .selected_ready_model
            .as_deref()
            .unwrap_or("-"),
        snapshot.buy_mode.target_selection.eligible_peer_count,
        snapshot.buy_mode.target_selection.observed_peer_count,
        snapshot
            .buy_mode
            .target_selection
            .blocked_reason_code
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buy_mode
            .target_selection
            .blocked_reason
            .as_deref()
            .unwrap_or("-")
    );
    if let Some(request_id) = snapshot.buy_mode.in_flight_request_id.as_deref() {
        println!(
            "buy mode in-flight: request={} phase={} status={} selected_provider_nostr={} result_provider_nostr={} invoice_provider_nostr={} payable_provider_nostr={} blockers={} blocker_summary={}",
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
                .result_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            snapshot
                .buy_mode
                .invoice_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            snapshot
                .buy_mode
                .payable_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            blocker_codes_label(snapshot.buy_mode.payment_blocker_codes.as_slice()),
            snapshot
                .buy_mode
                .payment_blocker_summary
                .as_deref()
                .unwrap_or("-")
        );
    }
    println!(
        "nip28: available={} selected_group={} selected_channel={} messages={} publishing_outbound={} configured_main={}",
        snapshot.nip28.available,
        snapshot.nip28.selected_group_name.as_deref().unwrap_or("-"),
        snapshot
            .nip28
            .selected_channel_name
            .as_deref()
            .unwrap_or("-"),
        snapshot.nip28.recent_messages.len(),
        snapshot.nip28.publishing_outbound_count,
        snapshot.nip28.configured_channel_id
    );
    print_active_job_text(snapshot.active_job.as_ref());
}

fn inventory_status_lines(snapshot: &DesktopControlSnapshot) -> Vec<String> {
    let mut lines = vec![format!(
        "inventory: authority={} projection={} snapshot={} products={} lots_open={} inventory_open={} reserved={} delivering={} proofs_24h={} challenges_open={}",
        snapshot.inventory.authority,
        snapshot.inventory.projection.source,
        snapshot
            .inventory
            .projection
            .latest_snapshot_id
            .as_deref()
            .unwrap_or("-"),
        snapshot.inventory.projection.compute_products_active,
        snapshot.inventory.projection.compute_capacity_lots_open,
        snapshot
            .inventory
            .projection
            .compute_inventory_quantity_open,
        snapshot
            .inventory
            .projection
            .compute_inventory_quantity_reserved,
        snapshot
            .inventory
            .projection
            .compute_inventory_quantity_delivering,
        snapshot.inventory.projection.compute_delivery_proofs_24h,
        snapshot
            .inventory
            .projection
            .compute_validator_challenges_open
    )];
    for section in &snapshot.inventory.sections {
        lines.push(format!(
            "inventory section: id={} available={} products={} ready={} eligible={} open_quantity={} blocker={} summary={}",
            section.section_id,
            section.available,
            section.product_count,
            section.ready_product_count,
            section.eligible_product_count,
            section.open_quantity,
            section.blocker_reason.as_deref().unwrap_or("-"),
            section.summary
        ));
    }
    lines
}

fn buyer_procurement_status_lines(snapshot: &DesktopControlSnapshot) -> Vec<String> {
    let mut lines = vec![format!(
        "buyer procurement: load={} mode={} spot_quotes={} forward_quotes={} accepted_spot_orders={} accepted_forward_orders={} selected_spot={} selected_forward={}",
        snapshot.buyer_procurement.load_state,
        snapshot.buyer_procurement.quote_mode,
        snapshot.buyer_procurement.spot_quotes.len(),
        snapshot.buyer_procurement.forward_quotes.len(),
        snapshot.buyer_procurement.accepted_spot_orders.len(),
        snapshot.buyer_procurement.accepted_forward_orders.len(),
        snapshot
            .buyer_procurement
            .selected_spot_quote_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buyer_procurement
            .selected_forward_quote_id
            .as_deref()
            .unwrap_or("-"),
    )];
    if let Some(summary) = snapshot.buyer_procurement.last_spot_rfq_summary.as_deref() {
        lines.push(format!("buyer spot rfq: {summary}"));
    }
    if let Some(summary) = snapshot
        .buyer_procurement
        .last_forward_rfq_summary
        .as_deref()
    {
        lines.push(format!("buyer forward rfq: {summary}"));
    }
    for quote in snapshot.buyer_procurement.spot_quotes.iter().take(3) {
        lines.push(format!(
            "buyer spot quote: id={} selected={} product={} backend={} topology={} proof={} env={} profile={} qty={}/{} price={} provider={}",
            quote.quote_id,
            quote.selected,
            quote.product_id,
            quote.backend,
            quote.topology,
            quote.proof_posture,
            quote.environment_ref.as_deref().unwrap_or("-"),
            quote.sandbox_profile_ref.as_deref().unwrap_or("-"),
            quote.requested_quantity,
            quote.available_quantity,
            quote.price_sats,
            quote.provider_id
        ));
    }
    for quote in snapshot.buyer_procurement.forward_quotes.iter().take(3) {
        lines.push(format!(
            "buyer forward quote: id={} selected={} product={} backend={} topology={} proof={} env={} profile={} qty={}/{} price={} window={} provider={}",
            quote.quote_id,
            quote.selected,
            quote.product_id,
            quote.backend,
            quote.topology,
            quote.proof_posture,
            quote.environment_ref.as_deref().unwrap_or("-"),
            quote.sandbox_profile_ref.as_deref().unwrap_or("-"),
            quote.requested_quantity,
            quote.available_quantity,
            quote.price_sats,
            quote.delivery_window_label,
            quote.provider_id
        ));
    }
    for order in snapshot
        .buyer_procurement
        .accepted_spot_orders
        .iter()
        .take(3)
    {
        lines.push(format!(
            "buyer spot order: id={} instrument={} product={} backend={} topology={} proof={} env={} profile={} qty={} price={} status={}",
            order.order_id,
            order.instrument_id,
            order.product_id,
            order.backend,
            order.topology,
            order.proof_posture,
            order.environment_ref.as_deref().unwrap_or("-"),
            order.sandbox_profile_ref.as_deref().unwrap_or("-"),
            order.quantity,
            order.price_sats,
            order.authority_status
        ));
    }
    for order in snapshot
        .buyer_procurement
        .accepted_forward_orders
        .iter()
        .take(3)
    {
        lines.push(format!(
            "buyer forward order: id={} instrument={} product={} backend={} topology={} proof={} env={} profile={} qty={} price={} status={} remedy={}",
            order.order_id,
            order.instrument_id,
            order.product_id,
            order.backend,
            order.topology,
            order.proof_posture,
            order.environment_ref.as_deref().unwrap_or("-"),
            order.sandbox_profile_ref.as_deref().unwrap_or("-"),
            order.quantity,
            order.price_sats,
            order.authority_status,
            order.remedy_summary.as_deref().unwrap_or("-")
        ));
    }
    lines
}

fn training_status_lines(snapshot: &DesktopControlSnapshot) -> Vec<String> {
    let mut lines = vec![format!(
        "training: available={} source={} control={} artifact={} runs={} active_runs={} accepted_runs={} windows={}/{} promotion_ready_windows={} contributions={} accepted_outcomes={} env_versions={} checkpoints={} participants={}/{} stale_rollout_discards={} duplicate_quarantine={} duplicate_deweights={} validator={}/{}/{} sandbox_ready_profiles={} sandbox_active_jobs={} contributor_revision={} reselection={} contributor_state={} operator={} operator_runs={}/{}",
        snapshot.training.available,
        snapshot.training.source,
        snapshot.training.control_plane_state,
        snapshot.training.artifact_plane_state,
        snapshot.training.run_count,
        snapshot.training.active_run_count,
        snapshot.training.accepted_run_count,
        snapshot.training.active_adapter_window_count,
        snapshot.training.adapter_window_count,
        snapshot.training.promotion_ready_window_count,
        snapshot.training.contribution_count,
        snapshot.training.accepted_outcome_count,
        if snapshot.training.environment_versions.is_empty() {
            "-".to_string()
        } else {
            snapshot.training.environment_versions.join(",")
        },
        snapshot.training.checkpoint_refs.len(),
        snapshot.training.contributing_participant_count,
        snapshot.training.admitted_participant_count,
        snapshot.training.stale_rollout_discard_count,
        snapshot.training.duplicate_rollout_quarantine_count,
        snapshot.training.duplicate_rollout_deweight_count,
        snapshot.training.validator_verified_count,
        snapshot.training.validator_rejected_count,
        snapshot.training.validator_timed_out_count,
        snapshot.training.sandbox_ready_profile_count,
        snapshot.training.sandbox_active_job_count,
        snapshot
            .training
            .contributor_set_revision
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .training
            .contributor_reselection_timing
            .as_deref()
            .unwrap_or("-"),
        snapshot.training.contributor.assignment_state,
        snapshot.training.operator.workflow_state,
        snapshot.training.operator.active_run_count,
        snapshot.training.operator.run_count
    )];
    lines.push(format!(
        "training contributor: available={} node={} enabled={} backend_ready={} contributor_supported={} coordinator_match={} authority_receipts={} match_eligible={} local_assignments={}/{} uploaded={} dispositions={}/{}/{}/{} settlement_ready={} trigger={} backends={} families={} formats={} validator_policies={} latest_window={} latest_assignment={} latest_payout={} detail={}",
        snapshot.training.contributor.available,
        snapshot
            .training
            .contributor
            .local_node_id
            .as_deref()
            .unwrap_or("-"),
        snapshot.training.contributor.product_enabled,
        snapshot.training.contributor.backend_ready,
        snapshot.training.contributor.contributor_supported,
        snapshot.training.contributor.coordinator_match_supported,
        snapshot.training.contributor.authority_receipt_supported,
        snapshot.training.contributor.match_eligible,
        snapshot.training.contributor.local_active_assignment_count,
        snapshot.training.contributor.local_assignment_count,
        snapshot.training.contributor.local_uploaded_count,
        snapshot.training.contributor.local_accepted_count,
        snapshot.training.contributor.local_quarantined_count,
        snapshot.training.contributor.local_rejected_count,
        snapshot.training.contributor.local_replay_required_count,
        snapshot.training.contributor.local_settlement_ready_count,
        snapshot
            .training
            .contributor
            .settlement_trigger
            .as_deref()
            .unwrap_or("-"),
        if snapshot.training.contributor.execution_backends.is_empty() {
            "-".to_string()
        } else {
            snapshot.training.contributor.execution_backends.join(",")
        },
        if snapshot.training.contributor.adapter_families.is_empty() {
            "-".to_string()
        } else {
            snapshot.training.contributor.adapter_families.join(",")
        },
        if snapshot.training.contributor.adapter_formats.is_empty() {
            "-".to_string()
        } else {
            snapshot.training.contributor.adapter_formats.join(",")
        },
        if snapshot.training.contributor.validator_policy_refs.is_empty() {
            "-".to_string()
        } else {
            snapshot.training.contributor.validator_policy_refs.join(",")
        },
        snapshot
            .training
            .contributor
            .latest_window_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .training
            .contributor
            .latest_assignment_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .training
            .contributor
            .latest_payout_state
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .training
            .contributor
            .readiness_detail
            .as_deref()
            .unwrap_or("-")
    ));
    lines.push(format!(
        "training operator: available={} state={} storage={} accepted_runs={} exported_runs={} last_action={} last_error={}",
        snapshot.training.operator.available,
        snapshot.training.operator.workflow_state,
        snapshot
            .training
            .operator
            .storage_path
            .as_deref()
            .unwrap_or("-"),
        snapshot.training.operator.accepted_run_count,
        snapshot.training.operator.exported_run_count,
        snapshot
            .training
            .operator
            .last_action
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .training
            .operator
            .last_error
            .as_deref()
            .unwrap_or("-")
    ));
    for run in snapshot.training.runs.iter().take(3) {
        lines.push(format!(
            "training run: id={} status={} control={} artifact={} policy={} env={}@{} checkpoint_family={} steps={}/{} eval_runs={} benchmarks={} best_eval_bps={} final_checkpoint={} promotion_checkpoint={} accepted_outcome={}",
            run.training_run_id,
            run.status,
            run.control_plane_state,
            run.artifact_plane_state,
            run.training_policy_ref,
            run.environment_ref,
            run.environment_version.as_deref().unwrap_or("-"),
            run.checkpoint_family,
            run.completed_step_count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            run.expected_step_count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            run.rollout_verification_eval_run_count,
            run.benchmark_package_count,
            run.best_eval_score_bps.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            run.final_checkpoint_ref.as_deref().unwrap_or("-"),
            run.promotion_checkpoint_ref.as_deref().unwrap_or("-"),
            run.accepted_outcome_id.as_deref().unwrap_or("-")
        ));
    }
    for participant in snapshot.training.participants.iter().take(4) {
        lines.push(format!(
            "training participant: id={} visible_reason={} admitted={} contributing={} priority={} deweight={} excluded={}",
            participant.participant_id,
            participant.visible_reason,
            participant.admitted,
            participant.contributing,
            participant.priority_label,
            participant.deweight_reason.as_deref().unwrap_or("-"),
            participant.exclusion_reason.as_deref().unwrap_or("-")
        ));
    }
    for window in snapshot.training.windows.iter().take(4) {
        lines.push(format!(
            "training window: id={} run={} stage={} status={} revision={} counts={}/{}/{}/{}/{}/{} uploaded={} held_out_bps={} benchmark_bps={} runtime_smoke={} promotion_ready={} promotion={} accepted_outcome={} gates={} holds={}",
            window.window_id,
            window.training_run_id,
            window.stage_id,
            window.status,
            window.contributor_set_revision_id,
            window.total_contributions,
            window.admitted_contributions,
            window.accepted_contributions,
            window.quarantined_contributions,
            window.rejected_contributions,
            window.replay_required_contributions,
            window.uploaded_contributions,
            window
                .held_out_average_score_bps
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            window
                .benchmark_pass_rate_bps
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            window
                .runtime_smoke_passed
                .map(|value| if value { "passed" } else { "failed" })
                .unwrap_or("-"),
            window.promotion_ready,
            window.promotion_disposition.as_deref().unwrap_or("-"),
            window.accepted_outcome_id.as_deref().unwrap_or("-"),
            if window.gate_reason_codes.is_empty() {
                "-".to_string()
            } else {
                window.gate_reason_codes.join(",")
            },
            if window.hold_reason_codes.is_empty() {
                "-".to_string()
            } else {
                window.hold_reason_codes.join(",")
            }
        ));
    }
    for contribution in snapshot.training.contributions.iter().take(6) {
        lines.push(format!(
            "training contribution: id={} run={} window={} assignment={} node={} worker={} disposition={} reasons={} aggregation={} accepted_for_aggregation={} weight_bps={} upload={} payout={} trigger={}",
            contribution.contribution_id,
            contribution.training_run_id,
            contribution.window_id,
            contribution.assignment_id,
            contribution.contributor_node_id,
            contribution.worker_id,
            contribution.validator_disposition,
            if contribution.validation_reason_codes.is_empty() {
                "-".to_string()
            } else {
                contribution.validation_reason_codes.join(",")
            },
            contribution.aggregation_eligibility,
            contribution.accepted_for_aggregation,
            contribution
                .aggregation_weight_bps
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            contribution.upload_state,
            contribution.payout_state,
            contribution.settlement_trigger.as_deref().unwrap_or("-")
        ));
    }
    for run in snapshot.training.operator.runs.iter().take(4) {
        lines.push(format!(
            "training operator run: id={} package={} launch={} eval={} export={} authority_accept={} steps={}/{} avg_loss={} held_out_bps={} runtime_smoke={} exported_path={} authority_outcome={} training_run={}",
            run.run_id,
            run.package_name,
            run.launch_state,
            run.evaluation_state,
            run.export_state,
            run.acceptance_state,
            run.completed_step_count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            run.expected_step_count.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
            run.average_loss_label.as_deref().unwrap_or("-"),
            run.held_out_average_score_bps
                .or(run.held_out_pass_rate_bps)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.runtime_smoke_passed
                .map(|value| if value { "passed" } else { "failed" })
                .unwrap_or("-"),
            run.exported_package_path.as_deref().unwrap_or("-"),
            run.authority.accepted_outcome_id.as_deref().unwrap_or("-"),
            run.authority.training_run_id.as_deref().unwrap_or("-")
        ));
        lines.push(format!(
            "training operator live: id={} phase={} heartbeat_ms={} elapsed_ms={} phase_elapsed_ms={} eta_ms={} epoch={}/{} steps={}/{} eval_samples={}/{} loss={} checkpoint={} telemetry={} artifact={} {} failure={} {} resource={}",
            run.run_id,
            run.progress.current_phase.as_deref().unwrap_or("-"),
            run.progress
                .last_heartbeat_at_epoch_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .run_elapsed_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .phase_elapsed_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .eta_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .current_epoch
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .expected_epochs
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .completed_steps
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .expected_steps
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .completed_eval_samples
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress
                .expected_eval_samples
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.progress.latest_loss_label.as_deref().unwrap_or("-"),
            run.progress.last_checkpoint_path.as_deref().unwrap_or("-"),
            run.progress.telemetry_log_path.as_deref().unwrap_or("-"),
            run.progress.latest_artifact_kind.as_deref().unwrap_or("-"),
            run.progress.latest_artifact_path.as_deref().unwrap_or("-"),
            run.progress.last_failure_phase.as_deref().unwrap_or("-"),
            run.progress.last_failure_detail.as_deref().unwrap_or("-"),
            run.progress.latest_resource_summary.as_deref().unwrap_or("-"),
        ));
        for event in run.recent_events.iter().rev().take(4) {
            lines.push(format!(
                "training operator event: id={} seq={} phase={} kind={} detail={} epoch={}/{} step={}/{} eval_sample={}#{}/{} loss={} eta_ms={} checkpoint={} artifact={} {} failure={} resource={}",
                run.run_id,
                event.sequence,
                event.phase,
                event.kind,
                event.detail,
                event.epoch_index
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.expected_epochs
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.step_index
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.expected_steps
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.eval_sample_id.as_deref().unwrap_or("-"),
                event.eval_sample_index
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.expected_eval_samples
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.loss_label.as_deref().unwrap_or("-"),
                event.eta_ms
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event.checkpoint_path.as_deref().unwrap_or("-"),
                event.artifact_kind.as_deref().unwrap_or("-"),
                event.artifact_path.as_deref().unwrap_or("-"),
                event.failure_detail.as_deref().unwrap_or("-"),
                event.resource_summary.as_deref().unwrap_or("-"),
            ));
        }
    }
    if !snapshot.training.operator.runs.is_empty() {
        lines.push(
            "training operator note: export, runtime smoke, and authority acceptance do not by themselves prove benchmark-useful adapter quality; use the architecture-explainer acceptance harness for that claim".to_string(),
        );
    }
    if let Some(error) = snapshot.training.last_error.as_deref() {
        lines.push(format!("training last_error: {error}"));
    }
    lines
}

fn remote_training_status_lines(snapshot: &DesktopControlSnapshot) -> Vec<String> {
    let mut lines = vec![format!(
        "remote training: available={} source={} state={} last_sync={} last_success={} refresh_ms={} runs={} active_runs={} full_series={} summary_only={} stale_runs={} selected={} last_error={}",
        snapshot.remote_training.available,
        snapshot.remote_training.source,
        snapshot.remote_training.sync_state,
        snapshot
            .remote_training
            .last_synced_at_epoch_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        snapshot
            .remote_training
            .last_successful_sync_at_epoch_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        snapshot.remote_training.refresh_interval_ms,
        snapshot.remote_training.run_count,
        snapshot.remote_training.active_run_count,
        snapshot.remote_training.full_series_run_count,
        snapshot.remote_training.summary_only_run_count,
        snapshot.remote_training.stale_run_count,
        snapshot
            .remote_training
            .selected_run_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .remote_training
            .last_error
            .as_deref()
            .unwrap_or("-"),
    )];
    if let Some(selected_run) = snapshot.remote_training.selected_run.as_ref() {
        lines.push(format!(
            "remote training selected: run={} provider={} lane={} track={} proof={} comparability={} score={} topology={} result={} series={} heartbeat_age_ms={} stale={} cache={} digest={} contract={} unavailable_reason={} stale_reason={}",
            selected_run.run.run_id,
            selected_run.run.provider,
            selected_run.run.lane_id,
            format!(
                "{}:{}",
                selected_run.run.track.track_family, selected_run.run.track.track_id
            ),
            selected_run.run.track.proof_posture,
            selected_run.run.track.comparability_class,
            selected_run
                .run
                .primary_score
                .as_ref()
                .map(|score| score.score_summary.as_str())
                .unwrap_or("-"),
            selected_run.run.topology_summary.as_deref().unwrap_or("-"),
            selected_run.run.result_classification,
            selected_run.run.series_status,
            selected_run
                .run
                .heartbeat_age_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            selected_run.run.stale,
            selected_run.run.bundle_cached,
            selected_run.run.bundle_digest.as_deref().unwrap_or("-"),
            selected_run.run.contract_state,
            selected_run
                .run
                .series_unavailable_reason
                .as_deref()
                .unwrap_or("-"),
            selected_run.run.stale_reason.as_deref().unwrap_or("-"),
        ));
    }
    if let Some(compare) = snapshot.remote_training.compare.as_ref() {
        lines.push(format!(
            "remote training compare: state={} selected={} baseline={} selected_track={} baseline_track={} metric={} direction={} anchor_mode={} anchor={} overlay={} delta={} caveat={}",
            compare.state,
            compare.selected_run_id.as_deref().unwrap_or("-"),
            compare.baseline_run_id,
            compare.selected_track_id.as_deref().unwrap_or("-"),
            compare.baseline_track_id.as_deref().unwrap_or("-"),
            compare.score_metric_id.as_deref().unwrap_or("-"),
            compare.score_direction.as_deref().unwrap_or("-"),
            compare.anchor_mode,
            compare.anchor_label.as_deref().unwrap_or("-"),
            compare.overlay_capable,
            compare.delta_summary.as_deref().unwrap_or("-"),
            compare.caveat,
        ));
    }
    if let Some(focus) = snapshot.remote_training.topology_focus.as_ref() {
        lines.push(format!(
            "remote training topology focus: target={} label={} matching_events={} detail={}",
            focus.target, focus.label, focus.matching_event_count, focus.detail,
        ));
    }
    if let Some(focus) = snapshot.remote_training.provenance_focus.as_ref() {
        lines.push(format!(
            "remote training provenance focus: role={} found={} source_kind={} authoritative={} uri={} digest={} receipts={} detail={}",
            focus.artifact_role,
            focus.found,
            focus.source_kind.as_deref().unwrap_or("-"),
            focus
                .authoritative
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            focus.artifact_uri.as_deref().unwrap_or("-"),
            focus.artifact_digest.as_deref().unwrap_or("-"),
            focus.receipt_count,
            focus.detail,
        ));
    }
    lines.push(format!(
        "remote training explorer: available={} state={} view={} snapshot={} participant={} network={} epoch={} window={} participants={} windows={} checkpoints={} events={} authoritative_artifacts={} run_surface_links={} summary={} last_error={}",
        snapshot.remote_training.explorer.available,
        snapshot.remote_training.explorer.load_state,
        snapshot.remote_training.explorer.selected_view,
        snapshot
            .remote_training
            .explorer
            .selected_snapshot_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .remote_training
            .explorer
            .selected_participant_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .remote_training
            .explorer
            .network_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .remote_training
            .explorer
            .current_epoch_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .remote_training
            .explorer
            .active_window_id
            .as_deref()
            .unwrap_or("-"),
        snapshot.remote_training.explorer.participant_count,
        snapshot.remote_training.explorer.window_count,
        snapshot.remote_training.explorer.checkpoint_count,
        snapshot.remote_training.explorer.event_count,
        snapshot.remote_training.explorer.authoritative_artifact_count,
        snapshot.remote_training.explorer.run_surface_link_count,
        snapshot.remote_training.explorer.summary,
        snapshot
            .remote_training
            .explorer
            .last_error
            .as_deref()
            .unwrap_or("-"),
    ));
    lines
}

fn print_tunnels_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "tunnels: available={} approved_services={} active_services={} open_tunnels={}",
        snapshot.tunnels.available,
        snapshot.tunnels.approved_service_count,
        snapshot.tunnels.active_service_count,
        snapshot.tunnels.open_tunnel_count
    );
    for service in &snapshot.tunnels.services {
        println!(
            "service={} kind={} protocol={} active={} allowed_peers={} requests={} responses={} bytes_in={} bytes_out={} last_error={}",
            service.service_id,
            service.kind,
            service.protocol,
            service.active,
            service.allowed_peer_count,
            service.request_count,
            service.response_count,
            service.bytes_in,
            service.bytes_out,
            service.last_error.as_deref().unwrap_or("-")
        );
    }
    for tunnel in &snapshot.tunnels.tunnels {
        println!(
            "tunnel={} service={} direction={} peer={} state={} transport={} session_path={} requests={} responses={} bytes_sent={} bytes_received={} close_reason={} last_error={}",
            tunnel.tunnel_id,
            tunnel.service_id,
            tunnel.direction,
            tunnel.peer_node_id,
            tunnel.state,
            tunnel.transport_class,
            tunnel.session_path_kind,
            tunnel.request_count,
            tunnel.response_count,
            tunnel.bytes_sent,
            tunnel.bytes_received,
            tunnel.close_reason.as_deref().unwrap_or("-"),
            tunnel.last_error.as_deref().unwrap_or("-")
        );
    }
}

fn print_tailnet_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "tailnet: available={} backend={} tailnet={} magic_dns={} version={} client_version={} online_devices={}/{} last_error={}",
        snapshot.tailnet.available,
        snapshot.tailnet.backend_state.as_deref().unwrap_or("-"),
        snapshot.tailnet.current_tailnet.as_deref().unwrap_or("-"),
        snapshot.tailnet.magic_dns_suffix.as_deref().unwrap_or("-"),
        snapshot.tailnet.version.as_deref().unwrap_or("-"),
        snapshot.tailnet.client_version.as_deref().unwrap_or("-"),
        snapshot.tailnet.online_device_count,
        snapshot.tailnet.device_count,
        snapshot.tailnet.last_error.as_deref().unwrap_or("-")
    );
    if !snapshot.tailnet.health.is_empty() {
        println!("tailnet health: {}", snapshot.tailnet.health.join(" | "));
    }
    if let Some(device) = snapshot.tailnet.self_device.as_ref() {
        println!(
            "self: name={} os={} online={} active={} relay={} addr={} ips={} rx={} tx={} last_seen={} last_handshake={}",
            device.display_name,
            device.os,
            device.online,
            device.active,
            device.relay.as_deref().unwrap_or("-"),
            device.current_address.as_deref().unwrap_or("-"),
            if device.tailscale_ips.is_empty() {
                "-".to_string()
            } else {
                device.tailscale_ips.join(",")
            },
            device.rx_bytes,
            device.tx_bytes,
            device.last_seen.as_deref().unwrap_or("-"),
            device.last_handshake.as_deref().unwrap_or("-")
        );
    }
    for device in &snapshot.tailnet.peers {
        println!(
            "peer={} os={} online={} active={} relay={} addr={} ips={} rx={} tx={} last_seen={} last_handshake={}",
            device.display_name,
            device.os,
            device.online,
            device.active,
            device.relay.as_deref().unwrap_or("-"),
            device.current_address.as_deref().unwrap_or("-"),
            if device.tailscale_ips.is_empty() {
                "-".to_string()
            } else {
                device.tailscale_ips.join(",")
            },
            device.rx_bytes,
            device.tx_bytes,
            device.last_seen.as_deref().unwrap_or("-"),
            device.last_handshake.as_deref().unwrap_or("-")
        );
    }
}

fn print_cluster_text(payload: &Value) {
    println!(
        "cluster: available={} topology={} members={} last_error={}",
        payload
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("topology_label")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("member_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("last_error")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    for member in payload
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "member={} state={} transport={} session_path={} last_error={}",
            member
                .get("peer_node_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            member.get("state").and_then(Value::as_str).unwrap_or("-"),
            member
                .get("transport_class")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            member
                .get("session_path_kind")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            member
                .get("last_error")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
}

fn print_sandbox_status_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "sandbox: available={} profiles={}/{} jobs={} active_jobs={} last_error={}",
        snapshot.sandbox.available,
        snapshot.sandbox.ready_profile_count,
        snapshot.sandbox.declared_profile_count,
        snapshot.sandbox.job_count,
        snapshot.sandbox.active_job_count,
        snapshot.sandbox.last_error.as_deref().unwrap_or("-")
    );
    for profile in &snapshot.sandbox.profiles {
        println!(
            "profile={} execution={} runtime={} ready={}",
            profile.profile_id,
            profile.execution_class,
            profile.runtime_kind,
            profile.runtime_ready
        );
    }
    for job in &snapshot.sandbox.jobs {
        println!(
            "job={} state={} profile={} outputs={}/{} receipt_type={} detail={}",
            job.job_id,
            job.state,
            job.profile_id,
            job.upload_count,
            job.download_count,
            job.terminal_receipt_type.as_deref().unwrap_or("-"),
            job.last_detail.as_deref().unwrap_or("-")
        );
    }
}

fn print_sandbox_job_text(payload: &Value) {
    println!(
        "sandbox job: job={} state={} profile={} compute_product={} uploads={} downloads={} receipt_type={}",
        payload.get("job_id").and_then(Value::as_str).unwrap_or("-"),
        payload.get("state").and_then(Value::as_str).unwrap_or("-"),
        payload
            .get("profile_id")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("compute_product_id")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("uploads")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        payload
            .get("downloads")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        payload
            .get("terminal_receipt")
            .and_then(|receipt| receipt.get("receipt_type"))
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    if let Some(events) = payload.get("lifecycle_events").and_then(Value::as_array) {
        for event in events.iter().rev().take(4).rev() {
            println!(
                "event: state={} at={} detail={}",
                event.get("state").and_then(Value::as_str).unwrap_or("-"),
                event
                    .get("observed_at_ms")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                event.get("detail").and_then(Value::as_str).unwrap_or("-")
            );
        }
    }
}

fn print_sandbox_payload_summary(payload: &Value) {
    if payload.get("job_id").is_some() {
        print_sandbox_job_text(payload);
        return;
    }
    if payload.get("transfer_id").is_some() {
        println!(
            "sandbox transfer: id={} kind={} path={} size={} digest={}",
            payload
                .get("transfer_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            payload
                .get("transfer_kind")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            payload
                .get("relative_path")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            payload
                .get("size_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            payload
                .get("sha256_digest")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        return;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string())
    );
}

fn print_sandbox_download_text(payload: &Value, output: Option<&PathBuf>) -> Result<()> {
    let content_base64 = payload
        .get("content_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("sandbox download payload was missing content_base64"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(content_base64.as_bytes())
        .context("decode sandbox download payload")?;
    if let Some(output) = output {
        fs::write(output, bytes.as_slice())
            .with_context(|| format!("write sandbox download {}", output.display()))?;
        println!("wrote sandbox file: {}", output.display());
    } else if let Some(preview) = payload.get("utf8_preview").and_then(Value::as_str) {
        println!("{preview}");
    }
    let receipt = payload.get("receipt").unwrap_or(&Value::Null);
    println!(
        "sandbox download: kind={} path={} size={} digest={}",
        receipt
            .get("transfer_kind")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        receipt
            .get("relative_path")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        receipt
            .get("size_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        receipt
            .get("sha256_digest")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    Ok(())
}

fn print_proof_text(payload: &Value) {
    println!(
        "proofs: available={} source={} last_sync={} pending={} accepted={} rejected={} challenged={} settlements_open={} settlements_terminal={} last_error={}",
        payload
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload.get("source").and_then(Value::as_str).unwrap_or("-"),
        payload
            .get("last_synced_at_epoch_ms")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("pending_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("accepted_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("rejected_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("challenged_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("settlement_open_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("settlement_terminal_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("last_error")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    for proof in payload
        .get("history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "proof={} status={} posture={} topology={} provisioning={} env={} qty={}/{} settlement={} challenge={}",
            proof
                .get("delivery_proof_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("proof_status")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("proof_posture")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("topology_kind")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("provisioning_kind")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("environment_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("accepted_quantity")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            proof
                .get("metered_quantity")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            proof
                .get("settlement_summary")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("challenge_summary")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        println!(
            "  refs: bundle={} activation={} validator_pool={} validator_run={} runtime_manifest={} runtime_manifest_digest={} session_claims={} session_posture={} transport_posture={} config_identity={} mutable_runtime_variables={}",
            proof
                .get("proof_bundle_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("activation_fingerprint_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("validator_pool_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("validator_run_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("runtime_manifest_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("runtime_manifest_digest")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("session_claims_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("session_identity_posture")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("transport_identity_posture")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("runtime_config_identity_mode")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("mutable_runtime_variables_present")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
        println!(
            "  review: acceptance={} command_digest={} environment_digest={}",
            proof
                .get("acceptance_summary")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("command_digest")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            proof
                .get("environment_digest")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
    for settlement in payload
        .get("settlements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "settlement={} kind={} status={} product={} proofs={} challenges={} qty={} price_sats={} mode={} reason={} detail={}",
            settlement
                .get("settlement_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            settlement
                .get("settlement_kind")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            settlement
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            settlement
                .get("product_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            settlement
                .get("delivery_proof_ids")
                .and_then(Value::as_array)
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string()),
            settlement
                .get("challenge_ids")
                .and_then(Value::as_array)
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string()),
            settlement
                .get("quantity")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            settlement
                .get("fixed_price_sats")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            settlement
                .get("settlement_mode")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            settlement
                .get("reason_code")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            settlement
                .get("reason_detail")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        println!(
            "  outcome: {}",
            settlement
                .get("outcome_summary")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
}

fn print_challenge_text(payload: &Value) {
    println!(
        "challenges: available={} source={} last_sync={} open={} queued={} leased={} retrying={} verified={} rejected={} timed_out={} last_error={}",
        payload
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload.get("source").and_then(Value::as_str).unwrap_or("-"),
        payload
            .get("last_synced_at_epoch_ms")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("open_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("queued_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("leased_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("retrying_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("verified_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("rejected_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("timed_out_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("last_error")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    for challenge in payload
        .get("history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "challenge={} status={} verdict={} reason={} backend={} model={} proofs={} attempts={} validator={} pool={} protocol={}",
            challenge
                .get("challenge_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("verdict")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("reason_code")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("runtime_backend")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("model_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("delivery_proof_ids")
                .and_then(Value::as_array)
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string()),
            challenge
                .get("attempts_used")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            challenge
                .get("validator_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("validator_pool_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("protocol_id")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        println!(
            "  detail: proof_bundle_digest={} challenge_result_ref={} settlement_impact={} {}",
            challenge
                .get("proof_bundle_digest")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("challenge_result_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("settlement_impact_summary")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            challenge
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
}

fn print_training_text(payload: &Value) {
    println!(
        "training: available={} source={} last_sync={} control={} artifact={} runs={} active_runs={} accepted_runs={} windows={}/{} promotion_ready_windows={} contributions={} accepted_outcomes={} env_versions={} checkpoints={} participants={}/{} stale_rollout_discards={} duplicate_quarantine={} duplicate_deweights={} validator={}/{}/{} sandbox_ready_profiles={} sandbox_active_jobs={} contributor_revision={} reselection={} contributor_state={} operator={} operator_runs={}/{} last_error={}",
        payload
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload.get("source").and_then(Value::as_str).unwrap_or("-"),
        payload
            .get("last_synced_at_epoch_ms")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("control_plane_state")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("artifact_plane_state")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("active_run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("accepted_run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("active_adapter_window_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("adapter_window_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("promotion_ready_window_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contribution_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("accepted_outcome_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("environment_versions")
            .and_then(Value::as_array)
            .map(|items| items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("checkpoint_refs")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        payload
            .get("contributing_participant_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("admitted_participant_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("stale_rollout_discard_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("duplicate_rollout_quarantine_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("duplicate_rollout_deweight_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("validator_verified_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("validator_rejected_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("validator_timed_out_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("sandbox_ready_profile_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("sandbox_active_job_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor_set_revision")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor_reselection_timing")
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor")
            .and_then(|value| value.get("assignment_state"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("operator")
            .and_then(|value| value.get("workflow_state"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("operator")
            .and_then(|value| value.get("active_run_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("operator")
            .and_then(|value| value.get("run_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("last_error")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    println!(
        "training contributor: available={} node={} enabled={} backend_ready={} contributor_supported={} coordinator_match={} authority_receipts={} match_eligible={} local_assignments={}/{} uploaded={} dispositions={}/{}/{}/{} settlement_ready={} trigger={} backends={} families={} formats={} validator_policies={} latest_window={} latest_assignment={} latest_payout={} detail={}",
        payload
            .get("contributor")
            .and_then(|value| value.get("available"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_node_id"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor")
            .and_then(|value| value.get("product_enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("backend_ready"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("contributor_supported"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("coordinator_match_supported"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("authority_receipt_supported"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("match_eligible"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_active_assignment_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_assignment_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_uploaded_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_accepted_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_quarantined_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_rejected_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_replay_required_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("local_settlement_ready_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("contributor")
            .and_then(|value| value.get("settlement_trigger"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor")
            .and_then(|value| value.get("execution_backends"))
            .and_then(Value::as_array)
            .map(|items| items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("contributor")
            .and_then(|value| value.get("adapter_families"))
            .and_then(Value::as_array)
            .map(|items| items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("contributor")
            .and_then(|value| value.get("adapter_formats"))
            .and_then(Value::as_array)
            .map(|items| items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("contributor")
            .and_then(|value| value.get("validator_policy_refs"))
            .and_then(Value::as_array)
            .map(|items| items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("contributor")
            .and_then(|value| value.get("latest_window_id"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor")
            .and_then(|value| value.get("latest_assignment_id"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor")
            .and_then(|value| value.get("latest_payout_state"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("contributor")
            .and_then(|value| value.get("readiness_detail"))
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    println!(
        "training operator: available={} state={} storage={} accepted_runs={} exported_runs={} last_action={} last_error={}",
        payload
            .get("operator")
            .and_then(|value| value.get("available"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        payload
            .get("operator")
            .and_then(|value| value.get("workflow_state"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("operator")
            .and_then(|value| value.get("storage_path"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("operator")
            .and_then(|value| value.get("accepted_run_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("operator")
            .and_then(|value| value.get("exported_run_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("operator")
            .and_then(|value| value.get("last_action"))
            .and_then(Value::as_str)
            .unwrap_or("-"),
        payload
            .get("operator")
            .and_then(|value| value.get("last_error"))
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    for run in payload
        .get("runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "training run: id={} status={} control={} artifact={} policy={} env={}@{} checkpoint_family={} steps={}/{} eval_runs={} benchmarks={} best_eval_bps={} final_checkpoint={} promotion_checkpoint={} accepted_outcome={}",
            run.get("training_run_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("status").and_then(Value::as_str).unwrap_or("-"),
            run.get("control_plane_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("artifact_plane_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("training_policy_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("environment_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("environment_version")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("checkpoint_family")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("completed_step_count")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("expected_step_count")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("rollout_verification_eval_run_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            run.get("benchmark_package_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            run.get("best_eval_score_bps")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("final_checkpoint_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("promotion_checkpoint_ref")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("accepted_outcome_id")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
    for participant in payload
        .get("participants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "training participant: id={} visible_reason={} admitted={} contributing={} priority={} deweight={} excluded={}",
            participant
                .get("participant_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            participant
                .get("visible_reason")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            participant
                .get("admitted")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            participant
                .get("contributing")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            participant
                .get("priority_label")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            participant
                .get("deweight_reason")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            participant
                .get("exclusion_reason")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
    for window in payload
        .get("windows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "training window: id={} run={} stage={} status={} revision={} counts={}/{}/{}/{}/{}/{} uploaded={} held_out_bps={} benchmark_bps={} runtime_smoke={} promotion_ready={} promotion={} accepted_outcome={} gates={} holds={}",
            window
                .get("window_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            window
                .get("training_run_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            window
                .get("stage_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            window.get("status").and_then(Value::as_str).unwrap_or("-"),
            window
                .get("contributor_set_revision_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            window
                .get("total_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("admitted_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("accepted_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("quarantined_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("rejected_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("replay_required_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("uploaded_contributions")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window
                .get("held_out_average_score_bps")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            window
                .get("benchmark_pass_rate_bps")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            window
                .get("runtime_smoke_passed")
                .and_then(Value::as_bool)
                .map(|value| if value { "passed" } else { "failed" })
                .unwrap_or("-"),
            window
                .get("promotion_ready")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            window
                .get("promotion_disposition")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            window
                .get("accepted_outcome_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            window
                .get("gate_reason_codes")
                .and_then(Value::as_array)
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string()),
            window
                .get("hold_reason_codes")
                .and_then(Value::as_array)
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string())
        );
    }
    for contribution in payload
        .get("contributions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "training contribution: id={} run={} window={} assignment={} node={} worker={} disposition={} reasons={} aggregation={} accepted_for_aggregation={} weight_bps={} upload={} payout={} trigger={}",
            contribution
                .get("contribution_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("training_run_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("window_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("assignment_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("contributor_node_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("worker_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("validator_disposition")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("validation_reason_codes")
                .and_then(Value::as_array)
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(","))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string()),
            contribution
                .get("aggregation_eligibility")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("accepted_for_aggregation")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            contribution
                .get("aggregation_weight_bps")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            contribution
                .get("upload_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("payout_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            contribution
                .get("settlement_trigger")
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
    }
    for run in payload
        .get("operator")
        .and_then(|value| value.get("runs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        println!(
            "training operator run: id={} package={} launch={} eval={} export={} authority_accept={} steps={}/{} avg_loss={} held_out_bps={} runtime_smoke={} exported_path={} authority_outcome={} training_run={}",
            run.get("run_id").and_then(Value::as_str).unwrap_or("-"),
            run.get("package_name")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("launch_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("evaluation_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("export_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("acceptance_state")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("completed_step_count")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("expected_step_count")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("average_loss_label")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("held_out_average_score_bps")
                .or_else(|| run.get("held_out_pass_rate_bps"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("runtime_smoke_passed")
                .and_then(Value::as_bool)
                .map(|value| if value { "passed" } else { "failed" })
                .unwrap_or("-"),
            run.get("exported_package_path")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("authority")
                .and_then(|value| value.get("accepted_outcome_id"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("authority")
                .and_then(|value| value.get("training_run_id"))
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        println!(
            "training operator live: id={} phase={} heartbeat_ms={} elapsed_ms={} phase_elapsed_ms={} eta_ms={} epoch={}/{} steps={}/{} eval_samples={}/{} loss={} checkpoint={} telemetry={} artifact={} {} failure={} {} resource={}",
            run.get("run_id").and_then(Value::as_str).unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("current_phase"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("last_heartbeat_at_epoch_ms"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("run_elapsed_ms"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("phase_elapsed_ms"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("eta_ms"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("current_epoch"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("expected_epochs"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("completed_steps"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("expected_steps"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("completed_eval_samples"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("expected_eval_samples"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.get("progress")
                .and_then(|value| value.get("latest_loss_label"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("last_checkpoint_path"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("telemetry_log_path"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("latest_artifact_kind"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("latest_artifact_path"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("last_failure_phase"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("last_failure_detail"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            run.get("progress")
                .and_then(|value| value.get("latest_resource_summary"))
                .and_then(Value::as_str)
                .unwrap_or("-")
        );
        for event in run
            .get("recent_events")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .rev()
            .take(4)
        {
            println!(
                "training operator event: id={} seq={} phase={} kind={} detail={} epoch={}/{} step={}/{} eval_sample={}#{}/{} loss={} eta_ms={} checkpoint={} artifact={} {} failure={} resource={}",
                run.get("run_id").and_then(Value::as_str).unwrap_or("-"),
                event
                    .get("sequence")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
                event.get("phase").and_then(Value::as_str).unwrap_or("-"),
                event.get("kind").and_then(Value::as_str).unwrap_or("-"),
                event.get("detail").and_then(Value::as_str).unwrap_or("-"),
                event
                    .get("epoch_index")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("expected_epochs")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("step_index")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("expected_steps")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("eval_sample_id")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                event
                    .get("eval_sample_index")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("expected_eval_samples")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("loss_label")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                event
                    .get("eta_ms")
                    .and_then(Value::as_u64)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                event
                    .get("checkpoint_path")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                event
                    .get("artifact_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                event
                    .get("artifact_path")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                event
                    .get("failure_detail")
                    .and_then(Value::as_str)
                    .unwrap_or("-"),
                event
                    .get("resource_summary")
                    .and_then(Value::as_str)
                    .unwrap_or("-")
            );
        }
    }
    if payload
        .get("operator")
        .and_then(|value| value.get("runs"))
        .and_then(Value::as_array)
        .is_some_and(|runs| !runs.is_empty())
    {
        println!(
            "training operator note: export, runtime smoke, and authority acceptance do not by themselves prove benchmark-useful adapter quality; use the architecture-explainer acceptance harness for that claim"
        );
    }
}

fn watch_training_run(
    client: &DesktopControlClient,
    requested_run_id: Option<&str>,
    poll_ms: u64,
    timeout_ms: u64,
    json_output: bool,
) -> Result<()> {
    let started = Instant::now();
    let poll_ms = poll_ms.max(100);
    let mut last_sequence = 0_u64;
    let mut announced_run_id = None::<String>;
    loop {
        let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        if elapsed_ms >= timeout_ms {
            bail!(
                "Timed out watching Apple adapter training telemetry after {} ms",
                timeout_ms
            );
        }
        let snapshot = client.snapshot()?;
        let run = selected_training_operator_run(&snapshot, requested_run_id).ok_or_else(|| {
            if let Some(run_id) = requested_run_id {
                anyhow!("Apple adapter operator run `{run_id}` was not found")
            } else {
                anyhow!("No Apple adapter operator runs are available to watch")
            }
        })?;
        if announced_run_id.as_deref() != Some(run.run_id.as_str()) {
            if json_output {
                print_json(&json!({
                    "kind": "training_watch_started",
                    "run_id": run.run_id,
                    "package_name": run.package_name,
                    "telemetry_log_path": run.progress.telemetry_log_path,
                }))?;
            } else {
                println!(
                    "training watch: run={} package={} telemetry={}",
                    run.run_id,
                    run.package_name,
                    run.progress.telemetry_log_path.as_deref().unwrap_or("-")
                );
                println!(
                    "training watch live: phase={} steps={}/{} eval_samples={}/{} eta_ms={} artifact={} {} failure={} {}",
                    run.progress.current_phase.as_deref().unwrap_or("-"),
                    run.progress
                        .completed_steps
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    run.progress
                        .expected_steps
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    run.progress
                        .completed_eval_samples
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    run.progress
                        .expected_eval_samples
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    run.progress
                        .eta_ms
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    run.progress.latest_artifact_kind.as_deref().unwrap_or("-"),
                    run.progress.latest_artifact_path.as_deref().unwrap_or("-"),
                    run.progress.last_failure_phase.as_deref().unwrap_or("-"),
                    run.progress.last_failure_detail.as_deref().unwrap_or("-"),
                );
            }
            announced_run_id = Some(run.run_id.clone());
        }
        let new_events = run
            .recent_events
            .iter()
            .filter(|event| event.sequence > last_sequence)
            .cloned()
            .collect::<Vec<_>>();
        for event in &new_events {
            if json_output {
                print_json(&json!({
                    "kind": "training_watch_event",
                    "run_id": run.run_id,
                    "event": event,
                }))?;
            } else {
                println!(
                    "training watch event: id={} seq={} phase={} kind={} detail={} step={}/{} eval={}/{} loss={} eta_ms={} artifact={} {} failure={} resource={}",
                    run.run_id,
                    event.sequence,
                    event.phase,
                    event.kind,
                    event.detail,
                    event
                        .step_index
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    event
                        .expected_steps
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    event
                        .eval_sample_index
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    event
                        .expected_eval_samples
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    event.loss_label.as_deref().unwrap_or("-"),
                    event
                        .eta_ms
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    event.artifact_kind.as_deref().unwrap_or("-"),
                    event.artifact_path.as_deref().unwrap_or("-"),
                    event.failure_detail.as_deref().unwrap_or("-"),
                    event.resource_summary.as_deref().unwrap_or("-"),
                );
            }
            last_sequence = last_sequence.max(event.sequence);
        }
        if training_operator_run_is_terminal(run) {
            if json_output {
                print_json(&json!({
                    "kind": "training_watch_terminal",
                    "run_id": run.run_id,
                    "launch_state": run.launch_state,
                    "evaluation_state": run.evaluation_state,
                    "export_state": run.export_state,
                    "acceptance_state": run.acceptance_state,
                    "last_error": run.last_error,
                }))?;
            } else {
                println!(
                    "training watch terminal: run={} launch={} eval={} export={} authority_accept={} last_error={}",
                    run.run_id,
                    run.launch_state,
                    run.evaluation_state,
                    run.export_state,
                    run.acceptance_state,
                    run.last_error.as_deref().unwrap_or("-"),
                );
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(poll_ms));
    }
}

fn selected_training_operator_run<'a>(
    snapshot: &'a DesktopControlSnapshot,
    requested_run_id: Option<&str>,
) -> Option<&'a DesktopControlAppleAdapterOperatorRunStatus> {
    let runs = &snapshot.training.operator.runs;
    requested_run_id.map_or_else(
        || runs.first(),
        |run_id| runs.iter().find(|run| run.run_id == run_id),
    )
}

fn training_operator_run_is_terminal(run: &DesktopControlAppleAdapterOperatorRunStatus) -> bool {
    !matches!(run.launch_state.as_str(), "running")
        && !matches!(run.evaluation_state.as_str(), "running")
        && !matches!(run.export_state.as_str(), "running")
        && !matches!(run.acceptance_state.as_str(), "running")
}

fn print_remote_training_text(payload: &Value) {
    println!(
        "remote training: available={} source={} state={} last_sync={} last_success={} refresh_ms={} runs={} active_runs={} full_series={} summary_only={} stale_runs={} selected={} last_error={}",
        json_bool(payload.get("available")),
        json_str(payload.get("source")).unwrap_or("-"),
        json_str(payload.get("sync_state")).unwrap_or("-"),
        payload
            .get("last_synced_at_epoch_ms")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("last_successful_sync_at_epoch_ms")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        payload
            .get("refresh_interval_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("active_run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("full_series_run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("summary_only_run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        payload
            .get("stale_run_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        json_str(payload.get("selected_run_id")).unwrap_or("-"),
        json_str(payload.get("last_error")).unwrap_or("-"),
    );
    if let Some(selected_run) = payload.get("selected_run") {
        print_remote_training_run_text(selected_run);
    }
    if let Some(compare) = payload.get("compare") {
        println!(
            "remote training compare: state={} selected={} baseline={} selected_track={} baseline_track={} metric={} direction={} anchor_mode={} anchor={} overlay={} delta={} caveat={}",
            json_str(compare.get("state")).unwrap_or("-"),
            json_str(compare.get("selected_run_id")).unwrap_or("-"),
            json_str(compare.get("baseline_run_id")).unwrap_or("-"),
            json_str(compare.get("selected_track_id")).unwrap_or("-"),
            json_str(compare.get("baseline_track_id")).unwrap_or("-"),
            json_str(compare.get("score_metric_id")).unwrap_or("-"),
            json_str(compare.get("score_direction")).unwrap_or("-"),
            json_str(compare.get("anchor_mode")).unwrap_or("-"),
            json_str(compare.get("anchor_label")).unwrap_or("-"),
            json_bool(compare.get("overlay_capable")),
            json_str(compare.get("delta_summary")).unwrap_or("-"),
            json_str(compare.get("caveat")).unwrap_or("-"),
        );
    }
    if let Some(focus) = payload.get("topology_focus") {
        println!(
            "remote training topology focus: target={} label={} matching_events={} detail={}",
            json_str(focus.get("target")).unwrap_or("-"),
            json_str(focus.get("label")).unwrap_or("-"),
            focus
                .get("matching_event_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            json_str(focus.get("detail")).unwrap_or("-"),
        );
    }
    if let Some(focus) = payload.get("provenance_focus") {
        println!(
            "remote training provenance focus: role={} found={} source_kind={} authoritative={} uri={} digest={} receipts={} detail={}",
            json_str(focus.get("artifact_role")).unwrap_or("-"),
            json_bool(focus.get("found")),
            json_str(focus.get("source_kind")).unwrap_or("-"),
            focus
                .get("authoritative")
                .map(|value| {
                    if value.is_null() {
                        "-".to_string()
                    } else {
                        json_bool(Some(value)).to_string()
                    }
                })
                .unwrap_or_else(|| "-".to_string()),
            json_str(focus.get("artifact_uri")).unwrap_or("-"),
            json_str(focus.get("artifact_digest")).unwrap_or("-"),
            focus
                .get("receipt_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            json_str(focus.get("detail")).unwrap_or("-"),
        );
    }
    if let Some(explorer) = payload.get("explorer") {
        println!(
            "remote training explorer: available={} state={} view={} snapshot={} participant={} network={} epoch={} window={} participants={} windows={} checkpoints={} events={} authoritative_artifacts={} run_surface_links={} summary={} last_error={}",
            json_bool(explorer.get("available")),
            json_str(explorer.get("load_state")).unwrap_or("-"),
            json_str(explorer.get("selected_view")).unwrap_or("-"),
            json_str(explorer.get("selected_snapshot_id")).unwrap_or("-"),
            json_str(explorer.get("selected_participant_id")).unwrap_or("-"),
            json_str(explorer.get("network_id")).unwrap_or("-"),
            json_str(explorer.get("current_epoch_id")).unwrap_or("-"),
            json_str(explorer.get("active_window_id")).unwrap_or("-"),
            explorer
                .get("participant_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            explorer
                .get("window_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            explorer
                .get("checkpoint_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            explorer
                .get("event_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            explorer
                .get("authoritative_artifact_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            explorer
                .get("run_surface_link_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            json_str(explorer.get("summary")).unwrap_or("-"),
            json_str(explorer.get("last_error")).unwrap_or("-"),
        );
    }
    print_remote_training_runs_text(payload);
}

fn print_remote_training_runs_text(payload: &Value) {
    let runs = payload
        .get("runs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if runs.is_empty() {
        println!("remote training runs: none");
        return;
    }
    for run in runs {
        println!(
            "remote training run: run={} provider={} lane={} track={} proof={} comparability={} result={} series={} score={} topology={} heartbeat_age_ms={} stale={} cached={} digest={} contract={} unavailable_reason={} stale_reason={}",
            json_str(run.get("run_id")).unwrap_or("-"),
            json_str(run.get("provider")).unwrap_or("-"),
            json_str(run.get("lane_id")).unwrap_or("-"),
            remote_training_track_summary(run),
            json_nested_str(Some(run), &["track", "proof_posture"]).unwrap_or("-"),
            json_nested_str(Some(run), &["track", "comparability_class"]).unwrap_or("-"),
            json_str(run.get("result_classification")).unwrap_or("-"),
            json_str(run.get("series_status")).unwrap_or("-"),
            remote_training_primary_score_summary(run),
            json_str(run.get("topology_summary")).unwrap_or("-"),
            run.get("heartbeat_age_ms")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            json_bool(run.get("stale")),
            json_bool(run.get("bundle_cached")),
            json_str(run.get("bundle_digest")).unwrap_or("-"),
            json_str(run.get("contract_state")).unwrap_or("-"),
            json_str(run.get("series_unavailable_reason")).unwrap_or("-"),
            json_str(run.get("stale_reason")).unwrap_or("-"),
        );
    }
}

fn print_remote_training_run_text(payload: &Value) {
    let run = payload.get("run").unwrap_or(payload);
    println!(
        "remote training detail: run={} provider={} profile={} lane={} repo={} track={} execution={} proof={} comparability={} score_law={} score={} score_surface={} topology={} result={} series={} heartbeat_age_ms={} stale={} cached={} cache_path={} digest={} contract={} summary={}",
        json_str(run.get("run_id")).unwrap_or("-"),
        json_str(run.get("provider")).unwrap_or("-"),
        json_str(run.get("profile_id")).unwrap_or("-"),
        json_str(run.get("lane_id")).unwrap_or("-"),
        json_str(run.get("repo_revision")).unwrap_or("-"),
        remote_training_track_summary(run),
        json_nested_str(Some(run), &["track", "execution_class"]).unwrap_or("-"),
        json_nested_str(Some(run), &["track", "proof_posture"]).unwrap_or("-"),
        json_nested_str(Some(run), &["track", "comparability_class"]).unwrap_or("-"),
        json_nested_str(Some(run), &["track", "score_law_ref"]).unwrap_or("-"),
        remote_training_primary_score_summary(run),
        remote_training_score_surface_summary(run),
        json_str(run.get("topology_summary")).unwrap_or("-"),
        json_str(run.get("result_classification")).unwrap_or("-"),
        json_str(run.get("series_status")).unwrap_or("-"),
        run.get("heartbeat_age_ms")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        json_bool(run.get("stale")),
        json_bool(run.get("bundle_cached")),
        json_str(run.get("bundle_cache_path")).unwrap_or("-"),
        json_str(run.get("bundle_digest")).unwrap_or("-"),
        json_str(run.get("contract_state")).unwrap_or("-"),
        json_str(run.get("semantic_summary")).unwrap_or("-"),
    );
    println!(
        "remote training detail freshness: unavailable_reason={} stale_reason={} contract_error={} artifact_cap_bytes={} wallclock_cap_seconds={} source_root={} source_index={}",
        json_str(run.get("series_unavailable_reason")).unwrap_or("-"),
        json_str(run.get("stale_reason")).unwrap_or("-"),
        json_str(run.get("contract_error")).unwrap_or("-"),
        json_nested_u64(Some(run), &["track", "artifact_cap_bytes"])
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        json_nested_u64(Some(run), &["track", "wallclock_cap_seconds"])
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        json_str(payload.get("source_root")).unwrap_or("-"),
        json_str(payload.get("source_index_path")).unwrap_or("-"),
    );
    if let Some(bundle) = payload.get("bundle") {
        println!(
            "remote training bundle: profile={} lane={} run={} provider={} result={} emission={} refresh_ms={} heartbeat_seq={} last_heartbeat={} series={} summary_steps={} latest_step={} losses={} math={} runtime={} gpu={} distributed={} events={} source_artifacts={}",
            json_str(bundle.get("profile_id")).unwrap_or("-"),
            json_str(bundle.get("lane_id")).unwrap_or("-"),
            json_str(bundle.get("run_id")).unwrap_or("-"),
            json_str(bundle.get("provider")).unwrap_or("-"),
            json_str(bundle.get("result_classification")).unwrap_or("-"),
            bundle
                .get("refresh_contract")
                .and_then(|value| value.get("emission_mode"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            bundle
                .get("refresh_contract")
                .and_then(|value| value.get("target_ui_update_interval_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            bundle
                .get("refresh_contract")
                .and_then(|value| value.get("heartbeat_seq"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            bundle
                .get("refresh_contract")
                .and_then(|value| value.get("last_heartbeat_at_ms"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            json_str(bundle.get("series_status")).unwrap_or("-"),
            bundle
                .get("summary")
                .and_then(|value| value.get("total_steps_completed"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_global_step"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            json_array_len(bundle.get("loss_series")),
            json_array_len(bundle.get("math_series")),
            json_array_len(bundle.get("runtime_series")),
            json_array_len(bundle.get("gpu_series")),
            json_array_len(bundle.get("distributed_series")),
            json_array_len(bundle.get("event_series")),
            json_array_len(bundle.get("source_artifacts")),
        );
        println!(
            "remote training bundle summary: train_loss={} ema_loss={} validation_loss={} tokens_per_second={} samples_per_second_milli={} checkpoint={} summary_detail={}",
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_train_loss"))
                .and_then(Value::as_f64)
                .map(|value| format!("{value:.4}"))
                .unwrap_or_else(|| "-".to_string()),
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_ema_loss"))
                .and_then(Value::as_f64)
                .map(|value| format!("{value:.4}"))
                .unwrap_or_else(|| "-".to_string()),
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_validation_loss"))
                .and_then(Value::as_f64)
                .map(|value| format!("{value:.4}"))
                .unwrap_or_else(|| "-".to_string()),
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_tokens_per_second"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_samples_per_second_milli"))
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            bundle
                .get("summary")
                .and_then(|value| value.get("latest_checkpoint_ref"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
            bundle
                .get("summary")
                .and_then(|value| value.get("detail"))
                .and_then(Value::as_str)
                .unwrap_or("-"),
        );
        if let Some(source_artifacts) = bundle.get("source_artifacts").and_then(Value::as_array) {
            for artifact in source_artifacts {
                println!(
                    "remote training provenance: role={} source_kind={} authoritative={} uri={} digest={} receipts={} detail={}",
                    json_str(artifact.get("artifact_role")).unwrap_or("-"),
                    json_str(artifact.get("source_kind")).unwrap_or("-"),
                    json_bool(artifact.get("authoritative")),
                    json_str(artifact.get("artifact_uri")).unwrap_or("-"),
                    json_str(artifact.get("artifact_digest")).unwrap_or("-"),
                    json_joined_array(artifact.get("source_receipt_ids"))
                        .unwrap_or_else(|| "-".to_string()),
                    json_str(artifact.get("detail")).unwrap_or("-"),
                );
            }
        }
    }
}

fn print_remote_training_stale_text(payload: &Value) {
    println!(
        "remote training stale: state={} source={} last_error={}",
        json_str(payload.get("sync_state")).unwrap_or("-"),
        json_str(payload.get("source")).unwrap_or("-"),
        json_str(payload.get("last_error")).unwrap_or("-"),
    );
    let stale_runs = payload
        .get("runs")
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter(|run| json_bool(run.get("stale")))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if stale_runs.is_empty() {
        println!("remote training stale runs: none");
        return;
    }
    for run in stale_runs {
        println!(
            "remote training stale run: run={} provider={} result={} heartbeat_age_ms={} reason={} unavailable_reason={}",
            json_str(run.get("run_id")).unwrap_or("-"),
            json_str(run.get("provider")).unwrap_or("-"),
            json_str(run.get("result_classification")).unwrap_or("-"),
            run.get("heartbeat_age_ms")
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            json_str(run.get("stale_reason")).unwrap_or("-"),
            json_str(run.get("series_unavailable_reason")).unwrap_or("-"),
        );
    }
}

fn print_research_text(payload: &Value) {
    println!(
        "research: programs={} updated_at={} storage={}",
        payload
            .get("programs")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        payload
            .get("updated_at_epoch_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        payload
            .get("storage_path")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    if let Some(last_action) = payload.get("last_action").and_then(Value::as_str) {
        println!("last action: {last_action}");
    }
    if let Some(last_error) = payload.get("last_error").and_then(Value::as_str) {
        println!("last error: {last_error}");
    }
    let Some(programs) = payload.get("programs").and_then(Value::as_array) else {
        return;
    };
    for program in programs {
        println!(
            "program: id={} family={} objective={} leader={} promoted={} frontier={} candidates={}",
            program
                .get("program_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            program.get("family").and_then(Value::as_str).unwrap_or("-"),
            program
                .get("objective")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            program
                .get("leader_candidate_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            program
                .get("promoted_candidate_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            program
                .get("frontier_candidate_ids")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            program
                .get("candidate_count")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        );
        if let Some(candidates) = program.get("candidates").and_then(Value::as_array) {
            for candidate in candidates {
                println!(
                    "research candidate: id={} run={} status={} decision={} promotable={} gate_failed={} weighted_score={} summary={}",
                    candidate
                        .get("candidate_id")
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                    candidate
                        .get("run_id")
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                    candidate
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                    candidate
                        .get("decision")
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                    candidate
                        .get("promotable")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    candidate
                        .get("hard_gate_failed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    candidate
                        .get("weighted_score")
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                    candidate
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                );
            }
        }
    }
}

fn print_local_runtime_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "local runtime: lane={} policy={} posture={} ready={} go_online_ready={} sell_compute_supported={} workbench={} text={} streaming={} structured={} model_management={} sessions={} action={} enabled={} label={}",
        snapshot.local_runtime.lane.as_deref().unwrap_or("-"),
        snapshot.local_runtime.policy,
        local_runtime_execution_posture_label(snapshot.local_runtime.diagnostics.posture),
        snapshot.local_runtime.runtime_ready,
        snapshot.local_runtime.go_online_ready,
        snapshot.local_runtime.supports_sell_compute,
        snapshot.local_runtime.workbench_label,
        snapshot.local_runtime.supports_run_text,
        snapshot.local_runtime.supports_streaming,
        snapshot.local_runtime.supports_structured,
        snapshot.local_runtime.supports_model_management,
        snapshot.local_runtime.supports_sessions,
        snapshot.local_runtime.action,
        snapshot.local_runtime.action_enabled,
        snapshot.local_runtime.action_label
    );
    println!(
        "model={} backend={} load={} status_stream={}",
        snapshot.local_runtime.model_label,
        snapshot.local_runtime.backend_label,
        snapshot.local_runtime.load_label,
        snapshot.local_runtime.status_stream
    );
    for line in local_runtime_diagnostic_lines(&snapshot.local_runtime) {
        println!("{line}");
    }
    println!("status: {}", snapshot.local_runtime.status_line);
    if let Some(hint) = snapshot.local_runtime.go_online_hint.as_deref() {
        println!("go_online_hint: {hint}");
    }
    for line in &snapshot.local_runtime.detail_lines {
        println!("detail: {line}");
    }
}

fn local_runtime_diagnostic_lines(status: &DesktopControlLocalRuntimeStatus) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(scheduler_posture) = local_runtime_scheduler_posture_label(&status.diagnostics) {
        lines.push(format!("scheduler: {scheduler_posture}"));
    }
    if let Some(resources) = status.diagnostics.runtime_resources.as_ref() {
        lines.push(format!(
            "plan_cache: entries={} bytes={} limit={}",
            resources.execution_plan_cache.state.cached_entries,
            resources.execution_plan_cache.state.cached_bytes,
            resources
                .execution_plan_cache
                .policy
                .max_cached_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unbounded".to_string())
        ));
        lines.push(format!(
            "kernel_cache: entries={} bytes={} limit={}",
            resources.kernel_cache.state.cached_entries,
            resources.kernel_cache.state.cached_bytes,
            resources
                .kernel_cache
                .policy
                .max_cached_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unbounded".to_string())
        ));
    }
    lines.extend(
        status
            .diagnostics
            .selected_devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                format!(
                    "device[{index}]: {}",
                    local_runtime_device_inventory_label(device)
                )
            }),
    );
    if let Some(compile_path) = status.diagnostics.last_compile_path.as_ref() {
        lines.push(format!(
            "compile_path: temperature={} plan={} kernel={}",
            compile_path_temperature_label(compile_path.temperature),
            compile_path.execution_plan_cache.detail,
            compile_path.kernel_cache.detail
        ));
    }
    if let Some(duration_ns) = status.diagnostics.last_cold_compile_duration_ns {
        lines.push(format!("last_cold_compile_ns: {duration_ns}"));
    }
    if let Some(duration_ns) = status.diagnostics.last_warm_refresh_duration_ns {
        lines.push(format!("last_warm_refresh_ns: {duration_ns}"));
    }
    if let Some(invalidation) = status.diagnostics.last_cache_invalidation.as_ref() {
        lines.push(format!(
            "cache_invalidation: reason={} at={} detail={}",
            local_runtime_cache_invalidation_reason_label(invalidation.reason),
            invalidation.observed_at_epoch_ms,
            invalidation.summary
        ));
    }
    if let Some(failure) = status.diagnostics.last_compile_failure.as_ref() {
        lines.push(format!(
            "compile_failure: at={} detail={}",
            failure.observed_at_epoch_ms, failure.summary
        ));
    }
    lines
}

fn print_gpt_oss_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "gpt-oss: detected={} backend={} ready={} busy={} loaded={} sell_compute_supported={} artifact_present={}",
        snapshot.gpt_oss.detected,
        snapshot.gpt_oss.backend.as_deref().unwrap_or("-"),
        snapshot.gpt_oss.ready,
        snapshot.gpt_oss.busy,
        snapshot.gpt_oss.loaded,
        snapshot.gpt_oss.supports_sell_compute,
        snapshot.gpt_oss.artifact_present
    );
    println!(
        "configured_model={} ready_model={} configured_model_path={}",
        snapshot.gpt_oss.configured_model.as_deref().unwrap_or("-"),
        snapshot.gpt_oss.ready_model.as_deref().unwrap_or("-"),
        snapshot
            .gpt_oss
            .configured_model_path
            .as_deref()
            .unwrap_or("-")
    );
    println!(
        "loaded_models={}",
        if snapshot.gpt_oss.loaded_models.is_empty() {
            "-".to_string()
        } else {
            snapshot.gpt_oss.loaded_models.join(",")
        }
    );
    if let Some(action) = snapshot.gpt_oss.last_action.as_deref() {
        println!("last_action: {action}");
    }
    if let Some(error) = snapshot.gpt_oss.last_error.as_deref() {
        println!("last_error: {error}");
    }
}

fn print_apple_fm_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "apple fm: ready={} reachable={} model_available={} model={} bridge_status={} inventory_supported={} attach_supported={} loaded_adapters={}",
        snapshot.apple_fm.ready,
        snapshot.apple_fm.reachable,
        snapshot.apple_fm.model_available,
        snapshot.apple_fm.ready_model.as_deref().unwrap_or("-"),
        snapshot.apple_fm.bridge_status.as_deref().unwrap_or("-"),
        snapshot.apple_fm.adapter_inventory_supported,
        snapshot.apple_fm.adapter_attach_supported,
        snapshot.apple_fm.loaded_adapters.len()
    );
    println!(
        "active_session={} active_adapter={}",
        snapshot
            .apple_fm
            .active_session_id
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .apple_fm
            .active_session_adapter
            .as_ref()
            .map(|adapter| adapter.adapter_id.as_str())
            .unwrap_or("-")
    );
    if let Some(action) = snapshot.apple_fm.last_action.as_deref() {
        println!("last_action: {action}");
    }
    if let Some(error) = snapshot.apple_fm.last_error.as_deref() {
        println!("last_error: {error}");
    }
    print_apple_fm_adapter_list_text(snapshot);
}

fn print_apple_fm_adapter_list_text(snapshot: &DesktopControlSnapshot) {
    if snapshot.apple_fm.loaded_adapters.is_empty() {
        println!("loaded adapters: none");
        return;
    }
    println!("loaded adapters:");
    for adapter in &snapshot.apple_fm.loaded_adapters {
        println!(
            "- id={} digest={} compatible={} reason={} attached_sessions={}",
            adapter.adapter.adapter_id,
            adapter.adapter.package_digest.as_deref().unwrap_or("-"),
            adapter.compatibility.compatible,
            adapter
                .compatibility
                .message
                .as_deref()
                .or(adapter.compatibility.reason_code.as_deref())
                .unwrap_or("-"),
            if adapter.attached_session_ids.is_empty() {
                "-".to_string()
            } else {
                adapter.attached_session_ids.join(",")
            }
        );
    }
}

fn print_buy_mode_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "buy mode: enabled={} approved_budget={} sats cadence={} next_dispatch={}",
        snapshot.buy_mode.enabled,
        snapshot.buy_mode.approved_budget_sats,
        format_buy_mode_cadence(snapshot),
        format_buy_mode_next_dispatch(snapshot)
    );
    print_buy_mode_target_text(snapshot);
    if let Some(request_id) = snapshot.buy_mode.in_flight_request_id.as_deref() {
        println!(
            "in-flight: request={} phase={} status={} selected_provider_nostr={} result_provider_nostr={} invoice_provider_nostr={} payable_provider_nostr={} blockers={} blocker_summary={}",
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
                .result_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            snapshot
                .buy_mode
                .invoice_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            snapshot
                .buy_mode
                .payable_provider_pubkey
                .as_deref()
                .unwrap_or("-"),
            blocker_codes_label(snapshot.buy_mode.payment_blocker_codes.as_slice()),
            snapshot
                .buy_mode
                .payment_blocker_summary
                .as_deref()
                .unwrap_or("-")
        );
    }
    for request in snapshot.buy_mode.recent_requests.iter().take(6) {
        println!(
            "request={} status={} phase={} next={} result_provider_nostr={} invoice_provider_nostr={} payable_provider_nostr={} blockers={} blocker_summary={} payment_pointer={} payment_error={}",
            request.request_id,
            request.status,
            request.phase,
            request.next_expected_event,
            request.result_provider_pubkey.as_deref().unwrap_or("-"),
            request.invoice_provider_pubkey.as_deref().unwrap_or("-"),
            request.payable_provider_pubkey.as_deref().unwrap_or("-"),
            blocker_codes_label(request.payment_blocker_codes.as_slice()),
            request.payment_blocker_summary.as_deref().unwrap_or("-"),
            request.payment_pointer.as_deref().unwrap_or("-"),
            request.payment_error.as_deref().unwrap_or("-")
        );
    }
    print_buy_mode_roster_text(snapshot, 8);
}

fn print_buy_mode_target_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "target: selected={} relay={} model={} roster={}/{} blocked_code={} blocked_reason={}",
        snapshot
            .buy_mode
            .target_selection
            .selected_peer_pubkey
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buy_mode
            .target_selection
            .selected_relay_url
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buy_mode
            .target_selection
            .selected_ready_model
            .as_deref()
            .unwrap_or("-"),
        snapshot.buy_mode.target_selection.eligible_peer_count,
        snapshot.buy_mode.target_selection.observed_peer_count,
        snapshot
            .buy_mode
            .target_selection
            .blocked_reason_code
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .buy_mode
            .target_selection
            .blocked_reason
            .as_deref()
            .unwrap_or("-")
    );
}

fn print_buy_mode_roster_text(snapshot: &DesktopControlSnapshot, limit: usize) {
    for peer in snapshot.buy_mode.peer_roster.iter().take(limit) {
        println!(
            "peer={} eligible={} online={} reason={} relay={} model={} last_presence={} expires={} last_chat={}",
            peer.pubkey,
            peer.eligible_for_buy_mode,
            peer.online_for_compute,
            peer.eligibility_reason,
            peer.relay_url,
            peer.ready_model.as_deref().unwrap_or("-"),
            peer.last_presence_at.unwrap_or(0),
            peer.presence_expires_at.unwrap_or(0),
            peer.last_chat_message_at.unwrap_or(0)
        );
    }
    if snapshot.buy_mode.peer_roster.is_empty() {
        println!("peer-roster: empty");
    }
}

fn print_nip28_status_text(snapshot: &DesktopControlSnapshot) {
    println!(
        "nip28: available={} browse_mode={} configured_relay={} configured_channel={} loaded={}",
        snapshot.nip28.available,
        snapshot.nip28.browse_mode,
        snapshot.nip28.configured_relay_url,
        snapshot.nip28.configured_channel_id,
        snapshot.nip28.configured_channel_loaded
    );
    println!(
        "selected: group={} channel={} relay={} local_pubkey={} publishing_outbound={} retryable_event={}",
        snapshot.nip28.selected_group_name.as_deref().unwrap_or("-"),
        snapshot
            .nip28
            .selected_channel_name
            .as_deref()
            .unwrap_or("-"),
        snapshot
            .nip28
            .selected_channel_relay_url
            .as_deref()
            .unwrap_or("-"),
        snapshot.nip28.local_pubkey.as_deref().unwrap_or("-"),
        snapshot.nip28.publishing_outbound_count,
        snapshot.nip28.retryable_event_id.as_deref().unwrap_or("-")
    );
    if let Some(error) = snapshot.nip28.last_error.as_deref() {
        println!("last_error: {error}");
    }
}

fn print_nip28_groups_text(snapshot: &DesktopControlSnapshot) {
    if snapshot.nip28.groups.is_empty() {
        println!("nip28 groups: none");
        return;
    }
    for group in &snapshot.nip28.groups {
        println!(
            "{} group={} name={} unread={} mentions={} channels={}",
            if group.selected { "*" } else { "-" },
            group.group_id,
            group.name,
            group.unread_count,
            group.mention_count,
            group.channel_count
        );
    }
}

fn print_nip28_channels_text(snapshot: &DesktopControlSnapshot) {
    if snapshot.nip28.channels.is_empty() {
        println!("nip28 channels: none");
        return;
    }
    for channel in &snapshot.nip28.channels {
        println!(
            "{} channel={} name={} relay={} unread={} mentions={} messages={}",
            if channel.selected { "*" } else { "-" },
            channel.channel_id,
            channel.name,
            channel.relay_url.as_deref().unwrap_or("-"),
            channel.unread_count,
            channel.mention_count,
            channel.message_count
        );
    }
}

fn print_nip28_messages_text(
    messages: &[autopilot_desktop::desktop_control::DesktopControlNip28MessageStatus],
) {
    if messages.is_empty() {
        println!("nip28 messages: none");
        return;
    }
    for message in messages {
        println!(
            "{} {} {} {}",
            message.created_at, message.delivery_state, message.author_pubkey, message.content
        );
    }
}

fn blocker_codes_label(codes: &[String]) -> String {
    if codes.is_empty() {
        "-".to_string()
    } else {
        codes.join(",")
    }
}

fn format_buy_mode_cadence(snapshot: &DesktopControlSnapshot) -> String {
    if snapshot.buy_mode.cadence_millis > 0 && snapshot.buy_mode.cadence_millis < 1_000 {
        format!("{}ms", snapshot.buy_mode.cadence_millis)
    } else {
        format!("{}s", snapshot.buy_mode.cadence_seconds)
    }
}

fn format_buy_mode_next_dispatch(snapshot: &DesktopControlSnapshot) -> String {
    if let Some(millis) = snapshot.buy_mode.next_dispatch_countdown_millis
        && millis < 1_000
    {
        return format!("{millis}ms");
    }
    format!(
        "{}s",
        snapshot
            .buy_mode
            .next_dispatch_countdown_seconds
            .unwrap_or(0)
    )
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
        println!("#{} {} {}", event.event_id, event.event_type, event.summary);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppleFmCommand, AttnResCommand, AttnResSpeedCommand, AttnResSublayerCommand,
        AttnResViewArg, BuyModeCommand, ChallengeCommand, ChatCommand, ClusterCommand,
        DataMarketCommand, DataMarketRevocationActionArg, DesktopControlClient, GptOssCommand,
        LocalRuntimeCommand, ProofCommand, ProviderCommand, RemoteTrainingCommand, ResearchCommand,
        ResolvedTarget, SandboxCommand, SandboxEntrypointTypeArg, TassadarCommand,
        TassadarFamilyCommand, TassadarNavigationCommand, TassadarReplayFamilyArg,
        TassadarSourceArg, TassadarSpeedCommand, TassadarViewArg, TassadarWindowCommand,
        TrainingCommand, WaitCondition, WaitConditionArg, WalletCommand,
        buy_mode_has_failed_request, buy_mode_has_paid_request, buyer_procurement_status_lines,
        compressed_pubkey_from_xonly_hex, data_market_sha256_hex, ensure_buy_mode_budget_ack,
        inventory_status_lines, materialize_data_market_delivery, nip90_sent_payments_report_lines,
        parse_giftwrapped_delivery_pointer, parse_local_daily_window,
        parse_nip90_sent_payments_report, parse_report_boundary, private_key_bytes,
        remote_training_status_lines, request_has_failed, request_has_paid,
        request_has_payment_required, training_status_lines,
    };
    use autopilot_desktop::desktop_control::{
        DesktopControlActionRequest, DesktopControlAttnResView, DesktopControlBuyModeRequestStatus,
        DesktopControlBuyModeStatus, DesktopControlDataMarketBuyerRequestArgs,
        DesktopControlDataMarketDraftAssetArgs, DesktopControlDataMarketDraftGrantArgs,
        DesktopControlDataMarketIssueDeliveryArgs, DesktopControlDataMarketPrepareDeliveryArgs,
        DesktopControlDataMarketPublishArgs, DesktopControlDataMarketRequestPaymentArgs,
        DesktopControlDataMarketResolveDeliveryArgs, DesktopControlDataMarketRevokeGrantArgs,
        DesktopControlNip28MessageStatus, DesktopControlNip90SentPaymentsReport,
        DesktopControlSnapshot, DesktopControlTassadarReplayFamily,
        DesktopControlTassadarSourceMode, DesktopControlTassadarView,
    };
    use autopilot_desktop::{
        LocalRuntimeCacheInvalidation, LocalRuntimeCacheInvalidationReason,
        LocalRuntimeCompileFailure, LocalRuntimeDiagnostics, LocalRuntimeExecutionPosture,
    };
    use clap::Parser;
    use psionic_runtime::{
        AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState, BackendRuntimeResources,
        CacheAction, CacheKind, CacheObservation, CompilePathEvidence, CompilePathTemperature,
        DeviceInventoryQualifiers, DeviceMemoryClass, DevicePerformanceClass,
        ExecutionPlanCachePolicy, ExecutionPlanCacheReport, ExecutionPlanCacheState,
        KernelCachePolicy, KernelCacheReport, KernelCacheState, LocalRuntimeObservability,
    };
    use psionic_sandbox::ProviderSandboxEntrypointType;
    use reqwest::blocking::Client;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;
    use tempfile::{NamedTempFile, TempDir};

    fn write_temp_json(raw: &str) -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp json file");
        std::fs::write(file.path(), raw).expect("write temp json");
        file
    }

    fn write_identity(temp_dir: &TempDir, name: &str, mnemonic: &str) -> PathBuf {
        let path = temp_dir.path().join(format!("{name}.mnemonic"));
        std::fs::write(&path, format!("{mnemonic}\n")).expect("write identity mnemonic");
        path
    }

    fn make_client(identity_path: PathBuf) -> DesktopControlClient {
        DesktopControlClient {
            http: Client::new(),
            target: ResolvedTarget {
                base_url: "http://127.0.0.1:1".to_string(),
                auth_token: "test-token".to_string(),
                manifest_path: None,
                identity_path: Some(identity_path),
                latest_session_log_path: None,
            },
        }
    }

    fn spawn_http_server(
        routes: BTreeMap<String, (Vec<u8>, &'static str)>,
        expected_connections: usize,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http listener");
        let base_url = format!("http://{}", listener.local_addr().expect("listener addr"));
        let handle = thread::spawn(move || {
            for _ in 0..expected_connections {
                let (mut stream, _) = listener.accept().expect("accept http connection");
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).expect("read request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .expect("request path")
                    .to_string();
                let (body, content_type) = routes
                    .get(path.as_str())
                    .cloned()
                    .unwrap_or_else(|| (b"not found".to_vec(), "text/plain"));
                let status = if routes.contains_key(path.as_str()) {
                    "HTTP/1.1 200 OK"
                } else {
                    "HTTP/1.1 404 Not Found"
                };
                let response = format!(
                    "{status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response head");
                stream
                    .write_all(body.as_slice())
                    .expect("write response body");
            }
        });
        (base_url, handle)
    }

    fn sample_delivery_payload(
        delivery_ref: &str,
        delivery_digest: Option<&str>,
        provider_id: &str,
        consumer_id: &str,
    ) -> Value {
        json!({
            "selection_reason": "explicit_selector",
            "resolved_request_id": "request.alpha",
            "delivery": {
                "delivery_bundle_id": "delivery.alpha",
                "grant_id": "grant.alpha",
                "asset_id": "asset.alpha",
                "provider_id": provider_id,
                "consumer_id": consumer_id,
                "delivery_ref": delivery_ref,
                "delivery_digest": delivery_digest,
                "manifest_refs": [],
            }
        })
    }

    fn sample_snapshot() -> DesktopControlSnapshot {
        let mut snapshot = DesktopControlSnapshot::default();
        snapshot.buy_mode = DesktopControlBuyModeStatus {
            approved_budget_sats: 2,
            ..DesktopControlBuyModeStatus::default()
        };
        snapshot.attnres_lab.available = true;
        snapshot.attnres_lab.playback_state = "armed".to_string();
        snapshot.attnres_lab.selected_view = "overview".to_string();
        snapshot.attnres_lab.selected_sublayer = 4;
        snapshot.attnres_lab.selected_sublayer_label = Some("L2 Attention".to_string());
        snapshot.attnres_lab.step = 18;
        snapshot.attnres_lab.max_steps = 24;
        snapshot.attnres_lab.speed_multiplier = 3;
        snapshot.attnres_lab.run_status = "replay loaded".to_string();
        snapshot
    }

    fn sample_nip90_sent_payments_report() -> DesktopControlNip90SentPaymentsReport {
        DesktopControlNip90SentPaymentsReport {
            report_date: Some("2026-03-14".to_string()),
            window_start_epoch_seconds: 1_773_464_400,
            window_end_epoch_seconds: 1_773_550_800,
            window_start_rfc3339: "2026-03-14T05:00:00+00:00".to_string(),
            window_end_rfc3339: "2026-03-15T05:00:00+00:00".to_string(),
            payment_count: 2,
            total_sats_sent: 42,
            total_fee_sats: 3,
            total_wallet_debit_sats: 45,
            connected_relay_count: 2,
            relay_urls_considered: vec![
                "wss://relay.one".to_string(),
                "wss://relay.two".to_string(),
            ],
            deduped_request_count: 1,
            degraded_binding_count: 0,
            generated_at_epoch_seconds: 1_773_550_801,
            generated_at_rfc3339: "2026-03-15T05:00:01+00:00".to_string(),
        }
    }

    #[test]
    fn local_runtime_diagnostic_lines_surface_scheduler_cache_and_failure_truth() {
        let mut snapshot = sample_snapshot();
        snapshot.local_runtime.diagnostics = LocalRuntimeDiagnostics {
            posture: LocalRuntimeExecutionPosture::CompileFailed,
            observability: Some(LocalRuntimeObservability::default()),
            runtime_resources: Some(BackendRuntimeResources {
                execution_plan_cache: ExecutionPlanCacheReport {
                    policy: ExecutionPlanCachePolicy::bounded(8, Some(4096)),
                    state: ExecutionPlanCacheState {
                        cached_entries: 2,
                        cached_bytes: 512,
                    },
                },
                allocator_pool: AllocatorPoolReport {
                    policy: AllocatorPoolPolicy::disabled(),
                    state: AllocatorPoolState::default(),
                },
                kernel_cache: KernelCacheReport {
                    policy: KernelCachePolicy::bounded(4, Some(2048)),
                    state: KernelCacheState {
                        cached_entries: 1,
                        cached_bytes: 256,
                    },
                },
                device_memory_budget: None,
            }),
            selected_devices: vec![DeviceInventoryQualifiers {
                stable_device_id: "00000000:01:00.0".to_string(),
                topology_key: Some("00000000:01:00.0".to_string()),
                performance_class: DevicePerformanceClass::DiscreteAccelerator,
                memory_class: DeviceMemoryClass::DedicatedDevice,
                total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
                free_memory_bytes: Some(12 * 1024 * 1024 * 1024),
            }],
            last_compile_path: Some(CompilePathEvidence {
                temperature: CompilePathTemperature::WarmReuse,
                execution_plan_cache: CacheObservation::new(
                    CacheKind::ExecutionPlan,
                    CacheAction::Reuse,
                    "reused cached plan",
                ),
                kernel_cache: CacheObservation::new(
                    CacheKind::KernelCache,
                    CacheAction::Reuse,
                    "reused cached kernels",
                ),
            }),
            last_cold_compile_duration_ns: Some(42),
            last_warm_refresh_duration_ns: Some(7),
            last_cache_invalidation: Some(LocalRuntimeCacheInvalidation {
                reason: LocalRuntimeCacheInvalidationReason::BackendChange,
                summary: "backend changed".to_string(),
                observed_at_epoch_ms: 99,
            }),
            last_compile_failure: Some(LocalRuntimeCompileFailure {
                summary: "compile failed".to_string(),
                observed_at_epoch_ms: 100,
            }),
            ..LocalRuntimeDiagnostics::default()
        };
        let lines = super::local_runtime_diagnostic_lines(&snapshot.local_runtime);
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("scheduler: single_request_only/")),
            "missing scheduler line: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("plan_cache: entries=2"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("kernel_cache: entries=1"))
        );
        assert!(
            lines.iter().any(|line| {
                line.starts_with("device[0]: id=00000000:01:00.0")
                    && line.contains("topology=00000000:01:00.0")
            }),
            "missing device line: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| {
                line.contains("compile_path: temperature=warm_reuse")
                    && line.contains("reused cached plan")
            }),
            "missing compile path line: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| line == "last_cold_compile_ns: 42"),
            "missing cold compile line: {lines:?}"
        );
        assert!(
            lines.iter().any(|line| {
                line.contains("cache_invalidation: reason=backend_change")
                    && line.contains("backend changed")
            }),
            "missing invalidation line: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("compile_failure: at=100 detail=compile failed")),
            "missing compile failure line: {lines:?}"
        );
    }

    #[test]
    fn inventory_status_lines_surface_section_counts() {
        let mut snapshot = sample_snapshot();
        snapshot.inventory.authority = "kernel_projected".to_string();
        snapshot.inventory.projection.source = "kernel_projection".to_string();
        snapshot.inventory.projection.latest_snapshot_id = Some("snapshot.compute.1".to_string());
        snapshot.inventory.projection.compute_products_active = 3;
        snapshot.inventory.projection.compute_capacity_lots_open = 2;
        snapshot
            .inventory
            .projection
            .compute_inventory_quantity_open = 1088;
        snapshot.inventory.projection.compute_delivery_proofs_24h = 8;
        snapshot
            .inventory
            .projection
            .compute_validator_challenges_open = 1;
        snapshot.inventory.sections.push(
            autopilot_desktop::desktop_control::DesktopControlInventorySectionStatus {
                section_id: "sandbox".to_string(),
                label: "Sandbox".to_string(),
                available: true,
                blocker_reason: None,
                summary:
                    "profiles=1 ready_profiles=1 products=1 ready=1 eligible=1 open_quantity=63"
                        .to_string(),
                product_count: 1,
                ready_product_count: 1,
                eligible_product_count: 1,
                open_quantity: 63,
                products: Vec::new(),
            },
        );

        let lines = inventory_status_lines(&snapshot);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("inventory: authority=kernel_projected"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("inventory section: id=sandbox"))
        );
    }

    #[test]
    fn buyer_procurement_status_lines_surface_sandbox_quote_details() {
        let mut snapshot = sample_snapshot();
        snapshot.buyer_procurement.load_state = "ready".to_string();
        snapshot.buyer_procurement.quote_mode = "forward_physical".to_string();
        snapshot.buyer_procurement.last_forward_rfq_summary =
            Some("rfq=rfq-forward-1 family=sandbox_execution".to_string());
        snapshot.buyer_procurement.forward_quotes.push(
            autopilot_desktop::desktop_control::DesktopControlBuyerProcurementQuoteStatus {
                quote_id: "forward-quote-1".to_string(),
                rfq_id: "rfq-forward-1".to_string(),
                product_id: "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
                    .to_string(),
                provider_id: "npub1sandbox".to_string(),
                compute_family: "sandbox_execution".to_string(),
                backend: "sandbox".to_string(),
                execution: "sandbox_execution".to_string(),
                topology: "sandbox_isolated".to_string(),
                provisioning: "remote_sandbox".to_string(),
                proof_posture: "topology_and_delivery".to_string(),
                requested_quantity: 1,
                available_quantity: 2,
                price_sats: 55,
                delivery_window_label: "start+180m / 1..2".to_string(),
                environment_ref: Some("env://sandbox/python".to_string()),
                sandbox_profile_ref: Some("python-batch".to_string()),
                source_badge: "desktop.forward_inventory".to_string(),
                terms_label: "forward physical / declared sandbox profile window".to_string(),
                capability_summary: "backend=sandbox execution=sandbox.python.exec".to_string(),
                collateral_summary: Some("bond=performance_bond".to_string()),
                remedy_summary: Some("forward_physical.sandbox.v1".to_string()),
                selected: true,
            },
        );

        let lines = buyer_procurement_status_lines(&snapshot);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("buyer procurement: load=ready mode=forward_physical"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("buyer forward quote: id=forward-quote-1"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("env=env://sandbox/python"))
        );
    }

    #[test]
    fn buy_mode_budget_ack_rejects_mismatch() {
        let snapshot = sample_snapshot();
        let error = ensure_buy_mode_budget_ack(&snapshot, 5).err();
        assert!(
            matches!(error, Some(error) if error.to_string().contains("Approved budget mismatch"))
        );
    }

    #[test]
    fn wait_condition_matches_buy_mode_payment_required() {
        let mut snapshot = sample_snapshot();
        snapshot
            .buy_mode
            .recent_requests
            .push(DesktopControlBuyModeRequestStatus {
                request_id: "req-1".to_string(),
                status: "streaming".to_string(),
                phase: "awaiting-payment".to_string(),
                next_expected_event: "invoice".to_string(),
                last_feedback_status: Some("payment-required".to_string()),
                ..DesktopControlBuyModeRequestStatus::default()
            });
        assert!(request_has_payment_required(
            &snapshot.buy_mode.recent_requests[0]
        ));
        assert!(WaitCondition::BuyModePaymentRequired.matches(&snapshot));
    }

    #[test]
    fn wait_conditions_cover_buy_mode_target_paid_and_failed_states() {
        let mut paid = sample_snapshot();
        paid.buy_mode.target_selection.selected_peer_pubkey = Some("a".repeat(64));
        paid.buy_mode
            .recent_requests
            .push(DesktopControlBuyModeRequestStatus {
                request_id: "req-paid".to_string(),
                status: "paid".to_string(),
                phase: "paid".to_string(),
                next_expected_event: "none".to_string(),
                wallet_status: "sent".to_string(),
                ..DesktopControlBuyModeRequestStatus::default()
            });
        assert!(WaitCondition::BuyModeTargetReady.matches(&paid));
        assert!(request_has_paid(&paid.buy_mode.recent_requests[0]));
        assert!(buy_mode_has_paid_request(&paid.buy_mode));
        assert!(WaitCondition::BuyModePaid.matches(&paid));
        assert!(!WaitCondition::BuyModeFailed.matches(&paid));

        let mut failed = sample_snapshot();
        failed
            .buy_mode
            .recent_requests
            .push(DesktopControlBuyModeRequestStatus {
                request_id: "req-failed".to_string(),
                status: "failed".to_string(),
                phase: "failed".to_string(),
                next_expected_event: "none".to_string(),
                payment_error: Some("payment timed out".to_string()),
                ..DesktopControlBuyModeRequestStatus::default()
            });
        assert!(request_has_failed(&failed.buy_mode.recent_requests[0]));
        assert!(buy_mode_has_failed_request(&failed.buy_mode));
        assert!(WaitCondition::BuyModeFailed.matches(&failed));
        assert!(!WaitCondition::BuyModePaid.matches(&failed));
    }

    #[test]
    fn lifecycle_commands_map_to_control_requests() {
        assert_eq!(
            ProviderCommand::Online {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            DesktopControlActionRequest::SetProviderMode { online: true }
        );
        assert_eq!(
            ProviderCommand::Offline {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            DesktopControlActionRequest::SetProviderMode { online: false }
        );
        assert_eq!(LocalRuntimeCommand::Status.action_request(), None);
        assert_eq!(
            LocalRuntimeCommand::Refresh {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::RefreshLocalRuntime)
        );
        assert_eq!(GptOssCommand::Status.action_request(), None);
        assert_eq!(
            GptOssCommand::Refresh {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::RefreshGptOss)
        );
        assert_eq!(
            GptOssCommand::Warm {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::WarmGptOss)
        );
        assert_eq!(
            GptOssCommand::Unload {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::UnloadGptOss)
        );
        assert_eq!(
            AppleFmCommand::Refresh {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::RefreshAppleFm)
        );
        assert_eq!(AppleFmCommand::Status.action_request(), None);
        assert_eq!(AppleFmCommand::List.action_request(), None);
        assert_eq!(
            AppleFmCommand::Load {
                package_path: PathBuf::from("/tmp/mock.fmadapter"),
                adapter_id: Some("fixture-chat-adapter".to_string()),
            }
            .action_request(),
            Some(DesktopControlActionRequest::LoadAppleFmAdapter {
                package_path: "/tmp/mock.fmadapter".to_string(),
                requested_adapter_id: Some("fixture-chat-adapter".to_string()),
            })
        );
        assert_eq!(
            AppleFmCommand::Unload {
                adapter_id: "fixture-chat-adapter".to_string(),
            }
            .action_request(),
            Some(DesktopControlActionRequest::UnloadAppleFmAdapter {
                adapter_id: "fixture-chat-adapter".to_string(),
            })
        );
        assert_eq!(
            AppleFmCommand::Attach {
                session_id: "sess-1".to_string(),
                adapter_id: "fixture-chat-adapter".to_string(),
            }
            .action_request(),
            Some(DesktopControlActionRequest::AttachAppleFmSessionAdapter {
                session_id: "sess-1".to_string(),
                adapter_id: "fixture-chat-adapter".to_string(),
            })
        );
        assert_eq!(
            AppleFmCommand::Detach {
                session_id: "sess-1".to_string(),
            }
            .action_request(),
            Some(DesktopControlActionRequest::DetachAppleFmSessionAdapter {
                session_id: "sess-1".to_string(),
            })
        );
        assert_eq!(
            AppleFmCommand::SmokeTest.action_request(),
            Some(DesktopControlActionRequest::RunAppleFmSmokeTest)
        );
        assert_eq!(
            WalletCommand::Refresh.action_request(),
            DesktopControlActionRequest::RefreshWallet
        );
        assert_eq!(
            AttnResCommand::Status.action_request(),
            DesktopControlActionRequest::GetAttnResStatus
        );
        assert_eq!(
            AttnResCommand::Start.action_request(),
            DesktopControlActionRequest::StartAttnRes
        );
        assert_eq!(
            AttnResCommand::Pause.action_request(),
            DesktopControlActionRequest::PauseAttnRes
        );
        assert_eq!(
            AttnResCommand::Reset.action_request(),
            DesktopControlActionRequest::ResetAttnRes
        );
        assert_eq!(
            AttnResCommand::Refresh.action_request(),
            DesktopControlActionRequest::RefreshAttnRes
        );
        assert_eq!(
            AttnResCommand::View {
                view: AttnResViewArg::Inference,
            }
            .action_request(),
            DesktopControlActionRequest::SetAttnResView {
                view: DesktopControlAttnResView::Inference,
            }
        );
        assert_eq!(
            AttnResCommand::Sublayer {
                command: AttnResSublayerCommand::Set { index: 4 },
            }
            .action_request(),
            DesktopControlActionRequest::SetAttnResSublayer { index: 4 }
        );
        assert_eq!(
            AttnResCommand::Sublayer {
                command: AttnResSublayerCommand::Next,
            }
            .action_request(),
            DesktopControlActionRequest::NextAttnResSublayer
        );
        assert_eq!(
            AttnResCommand::Sublayer {
                command: AttnResSublayerCommand::Prev,
            }
            .action_request(),
            DesktopControlActionRequest::PreviousAttnResSublayer
        );
        assert_eq!(
            AttnResCommand::Speed {
                command: AttnResSpeedCommand::Set {
                    speed_multiplier: 5,
                },
            }
            .action_request(),
            DesktopControlActionRequest::SetAttnResSpeed {
                speed_multiplier: 5,
            }
        );
        assert_eq!(
            AttnResCommand::Speed {
                command: AttnResSpeedCommand::Increase,
            }
            .action_request(),
            DesktopControlActionRequest::IncreaseAttnResSpeed
        );
        assert_eq!(
            AttnResCommand::Speed {
                command: AttnResSpeedCommand::Decrease,
            }
            .action_request(),
            DesktopControlActionRequest::DecreaseAttnResSpeed
        );
        assert_eq!(
            BuyModeCommand::Start {
                approved_budget_sats: 2,
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::StartBuyMode)
        );
        assert_eq!(
            BuyModeCommand::Stop {
                wait: false,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::StopBuyMode)
        );
        assert_eq!(BuyModeCommand::Status.action_request(), None);
        assert_eq!(BuyModeCommand::Target.action_request(), None);
        assert_eq!(BuyModeCommand::Roster { limit: 5 }.action_request(), None);
        assert_eq!(
            ClusterCommand::Status.action_request(),
            DesktopControlActionRequest::GetClusterStatus
        );
        assert_eq!(
            ClusterCommand::Topology.action_request(),
            DesktopControlActionRequest::GetClusterTopology
        );
        assert_eq!(
            ProofCommand::Status.action_request(),
            DesktopControlActionRequest::GetProofStatus
        );
        assert_eq!(
            ChallengeCommand::Status.action_request(),
            DesktopControlActionRequest::GetChallengeStatus
        );
        assert_eq!(
            TrainingCommand::Status.action_request(),
            DesktopControlActionRequest::GetTrainingStatus
        );
        assert_eq!(
            TrainingCommand::Watch {
                run_id: Some("weather-helper-1".to_string()),
                poll_ms: 500,
                timeout_ms: 5_000,
            }
            .action_request(),
            DesktopControlActionRequest::GetTrainingStatus
        );
        assert_eq!(
            TrainingCommand::Launch {
                train_dataset_path: PathBuf::from("/tmp/train.jsonl"),
                held_out_dataset_path: PathBuf::from("/tmp/held-out.jsonl"),
                package_name: "weather-helper".to_string(),
                author: "OpenAgents".to_string(),
                description: "Operator launch".to_string(),
                license: "Apache-2.0".to_string(),
                apple_fm_base_url: "http://127.0.0.1:11435".to_string(),
                experiment_manifest_path: Some(PathBuf::from("/tmp/experiment.json")),
                training_policy_override_path: Some(PathBuf::from("/tmp/policy.json")),
            }
            .action_request(),
            DesktopControlActionRequest::LaunchAppleAdapterTraining {
                train_dataset_path: "/tmp/train.jsonl".to_string(),
                held_out_dataset_path: "/tmp/held-out.jsonl".to_string(),
                package_name: "weather-helper".to_string(),
                author: "OpenAgents".to_string(),
                description: "Operator launch".to_string(),
                license: "Apache-2.0".to_string(),
                apple_fm_base_url: "http://127.0.0.1:11435".to_string(),
                experiment_manifest_path: Some("/tmp/experiment.json".to_string()),
                training_policy_override_path: Some("/tmp/policy.json".to_string()),
            }
        );
        assert_eq!(
            TrainingCommand::Export {
                run_id: "weather-helper-1".to_string(),
                export_path: PathBuf::from("/tmp/weather-helper.fmadapter"),
            }
            .action_request(),
            DesktopControlActionRequest::ExportAppleAdapterTraining {
                run_id: "weather-helper-1".to_string(),
                export_path: "/tmp/weather-helper.fmadapter".to_string(),
            }
        );
        assert_eq!(
            TrainingCommand::Accept {
                run_id: "weather-helper-1".to_string(),
            }
            .action_request(),
            DesktopControlActionRequest::AcceptAppleAdapterTraining {
                run_id: "weather-helper-1".to_string(),
            }
        );
        assert_eq!(
            RemoteTrainingCommand::Status.action_request(),
            DesktopControlActionRequest::GetRemoteTrainingStatus
        );
        assert_eq!(
            RemoteTrainingCommand::List.action_request(),
            DesktopControlActionRequest::GetRemoteTrainingStatus
        );
        assert_eq!(
            RemoteTrainingCommand::Stale.action_request(),
            DesktopControlActionRequest::GetRemoteTrainingStatus
        );
        assert_eq!(
            RemoteTrainingCommand::Run {
                run_id: Some("parameter-golf-runpod-single-h100-live-sample".to_string()),
            }
            .action_request(),
            DesktopControlActionRequest::GetRemoteTrainingRun {
                run_id: Some("parameter-golf-runpod-single-h100-live-sample".to_string()),
            }
        );
        assert_eq!(
            RemoteTrainingCommand::Refresh.action_request(),
            DesktopControlActionRequest::RefreshRemoteTrainingStatus
        );
        assert_eq!(
            ResearchCommand::Status.action_request(),
            DesktopControlActionRequest::GetResearchStatus
        );
        assert_eq!(
            ResearchCommand::Reset.action_request(),
            DesktopControlActionRequest::ResetResearchState
        );
        assert!(matches!(SandboxCommand::Status.action_request(), Ok(None)));
        assert!(matches!(
            SandboxCommand::Job {
                job_id: "job-1".to_string(),
            }
            .action_request(),
            Ok(Some(DesktopControlActionRequest::GetSandboxJob { ref job_id }))
                if job_id == "job-1"
        ));
        assert!(matches!(
            SandboxCommand::Create {
                profile_id: "pythonexec-profile".to_string(),
                job_id: "job-2".to_string(),
                workspace_root: PathBuf::from("/tmp/openagents-sandbox"),
                entrypoint_type: SandboxEntrypointTypeArg::WorkspaceFile,
                entrypoint: "scripts/job.py".to_string(),
                payload: None,
                arguments: vec!["--flag".to_string()],
                expected_outputs: vec!["result.txt".to_string()],
                timeout_s: 30,
                network: "host_inherit".to_string(),
                filesystem: "host_inherit".to_string(),
                payout_reference: Some("payment-1".to_string()),
                verification_posture: Some("hash_only".to_string()),
            }
            .action_request(),
            Ok(Some(DesktopControlActionRequest::CreateSandboxJob {
                ref profile_id,
                ref job_id,
                ref workspace_root,
                entrypoint_type: ProviderSandboxEntrypointType::WorkspaceFile,
                ref entrypoint,
                payload: None,
                ref arguments,
                ref expected_outputs,
                timeout_request_s: 30,
                ref network_request,
                ref filesystem_request,
                payout_reference: Some(ref payout_reference),
                verification_posture: Some(ref verification_posture),
            })) if profile_id == "pythonexec-profile"
                && job_id == "job-2"
                && workspace_root == "/tmp/openagents-sandbox"
                && entrypoint == "scripts/job.py"
                && arguments == &vec!["--flag".to_string()]
                && expected_outputs == &vec!["result.txt".to_string()]
                && network_request == "host_inherit"
                && filesystem_request == "host_inherit"
                && payout_reference == "payment-1"
                && verification_posture == "hash_only"
        ));
        assert!(matches!(
            SandboxCommand::Start {
                job_id: "job-3".to_string(),
            }
            .action_request(),
            Ok(Some(DesktopControlActionRequest::StartSandboxJob { ref job_id }))
                if job_id == "job-3"
        ));
        assert!(matches!(
            SandboxCommand::Wait {
                job_id: "job-4".to_string(),
                timeout_ms: 5_000,
            }
            .action_request(),
            Ok(Some(DesktopControlActionRequest::WaitSandboxJob {
                ref job_id,
                timeout_ms: 5_000,
            })) if job_id == "job-4"
        ));
        assert_eq!(
            ChatCommand::Main.action_request(),
            Some(DesktopControlActionRequest::SelectNip28MainChannel)
        );
        assert_eq!(
            ChatCommand::SelectGroup {
                group_id: "oa-main".to_string(),
            }
            .action_request(),
            Some(DesktopControlActionRequest::SelectNip28Group {
                group_id: "oa-main".to_string(),
            })
        );
        assert_eq!(
            ChatCommand::SelectChannel {
                channel_id: "chan-1".to_string(),
            }
            .action_request(),
            Some(DesktopControlActionRequest::SelectNip28Channel {
                channel_id: "chan-1".to_string(),
            })
        );
        assert_eq!(
            ChatCommand::Send {
                content: "hello".to_string(),
                reply_to_event_id: Some("event-1".to_string()),
                wait: true,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::SendNip28Message {
                content: "hello".to_string(),
                reply_to_event_id: Some("event-1".to_string()),
            })
        );
        assert_eq!(
            ChatCommand::Retry {
                event_id: "event-2".to_string(),
                wait: true,
                timeout_ms: 1_000,
            }
            .action_request(),
            Some(DesktopControlActionRequest::RetryNip28Message {
                event_id: "event-2".to_string(),
            })
        );
        assert_eq!(
            ChatCommand::CreateChannel {
                name: "oa-team-chat".to_string(),
                about: "OpenAgents team test channel".to_string(),
            }
            .action_request(),
            Some(DesktopControlActionRequest::CreateNip28Channel {
                name: "oa-team-chat".to_string(),
                about: "OpenAgents team test channel".to_string(),
            })
        );
        assert_eq!(ChatCommand::Status.action_request(), None);
        assert_eq!(ChatCommand::Groups.action_request(), None);
        assert_eq!(ChatCommand::Channels.action_request(), None);
        assert_eq!(ChatCommand::Tail { limit: 5 }.action_request(), None);
    }

    #[test]
    fn data_market_commands_map_to_control_requests() {
        let asset_file = write_temp_json(
            r#"{
                "asset_kind": "conversation_bundle",
                "title": "Support transcripts",
                "price_hint_sats": 250,
                "delivery_modes": ["bundle_ref"]
            }"#,
        );
        let asset_action = DataMarketCommand::DraftAsset {
            file: asset_file.path().to_path_buf(),
        }
        .action_request()
        .expect("asset draft action");
        assert_eq!(
            asset_action,
            DesktopControlActionRequest::DraftDataMarketAsset {
                args: DesktopControlDataMarketDraftAssetArgs {
                    asset_kind: Some("conversation_bundle".to_string()),
                    title: Some("Support transcripts".to_string()),
                    description: None,
                    content_digest: None,
                    provenance_ref: None,
                    default_policy: None,
                    price_hint_sats: Some(250),
                    delivery_modes: Some(vec!["bundle_ref".to_string()]),
                    visibility_posture: None,
                    sensitivity_posture: None,
                    metadata: None,
                },
            }
        );

        assert_eq!(
            DataMarketCommand::SellerStatus
                .action_request()
                .expect("seller status action"),
            DesktopControlActionRequest::GetDataMarketSellerStatus
        );
        assert_eq!(
            DataMarketCommand::SellerPrompt {
                prompt: "Turn ./fixtures/data-market/example into a saleable listing.".to_string(),
            }
            .action_request()
            .expect("seller prompt action"),
            DesktopControlActionRequest::SendDataMarketSellerPrompt {
                prompt: "Turn ./fixtures/data-market/example into a saleable listing.".to_string(),
            }
        );
        assert_eq!(
            DataMarketCommand::BuyerStatus
                .action_request()
                .expect("buyer status action"),
            DesktopControlActionRequest::GetDataMarketBuyerStatus
        );
        assert_eq!(
            DataMarketCommand::BuyerRefresh
                .action_request()
                .expect("buyer refresh action"),
            DesktopControlActionRequest::RefreshDataMarketBuyerMarket
        );
        assert_eq!(
            DataMarketCommand::PreviewAsset
                .action_request()
                .expect("preview asset action"),
            DesktopControlActionRequest::PreviewDataMarketAsset
        );
        assert_eq!(
            DataMarketCommand::PublishAsset { confirm: true }
                .action_request()
                .expect("publish asset action"),
            DesktopControlActionRequest::PublishDataMarketAsset {
                args: DesktopControlDataMarketPublishArgs { confirm: true },
            }
        );

        let grant_file = write_temp_json(
            r#"{
                "consumer_id": "buyer-pubkey",
                "price_hint_sats": 300,
                "expires_in_hours": 24
            }"#,
        );
        let grant_action = DataMarketCommand::DraftGrant {
            file: grant_file.path().to_path_buf(),
        }
        .action_request()
        .expect("grant draft action");
        assert_eq!(
            grant_action,
            DesktopControlActionRequest::DraftDataMarketGrant {
                args: DesktopControlDataMarketDraftGrantArgs {
                    default_policy: None,
                    policy_template: None,
                    consumer_id: Some("buyer-pubkey".to_string()),
                    price_hint_sats: Some(300),
                    delivery_modes: None,
                    visibility_posture: None,
                    expires_in_hours: Some(24),
                    warranty_window_hours: None,
                    metadata: None,
                },
            }
        );

        assert_eq!(
            DataMarketCommand::PreviewGrant
                .action_request()
                .expect("preview grant action"),
            DesktopControlActionRequest::PreviewDataMarketGrant
        );
        assert_eq!(
            DataMarketCommand::PublishGrant { confirm: true }
                .action_request()
                .expect("publish grant action"),
            DesktopControlActionRequest::PublishDataMarketGrant {
                args: DesktopControlDataMarketPublishArgs { confirm: true },
            }
        );
        assert_eq!(
            DataMarketCommand::RequestPayment {
                request_id: "req-1".to_string(),
            }
            .action_request()
            .expect("payment request action"),
            DesktopControlActionRequest::RequestDataMarketPayment {
                args: DesktopControlDataMarketRequestPaymentArgs {
                    request_id: "req-1".to_string(),
                },
            }
        );

        let delivery_file = write_temp_json(
            r#"{
                "preview_text": "bundle preview",
                "delivery_ref": "bundle://fixture/1",
                "delivery_digest": "sha256:fixture",
                "manifest_refs": ["manifest://fixture/1"],
                "bundle_size_bytes": 4096,
                "expires_in_hours": 48
            }"#,
        );
        let delivery_action = DataMarketCommand::PrepareDelivery {
            request_id: "req-2".to_string(),
            file: delivery_file.path().to_path_buf(),
        }
        .action_request()
        .expect("prepare delivery action");
        assert_eq!(
            delivery_action,
            DesktopControlActionRequest::PrepareDataMarketDelivery {
                args: DesktopControlDataMarketPrepareDeliveryArgs {
                    request_id: "req-2".to_string(),
                    preview_text: Some("bundle preview".to_string()),
                    delivery_ref: Some("bundle://fixture/1".to_string()),
                    delivery_digest: Some("sha256:fixture".to_string()),
                    manifest_refs: Some(vec!["manifest://fixture/1".to_string()]),
                    bundle_size_bytes: Some(4096),
                    expires_in_hours: Some(48),
                },
            }
        );
        assert_eq!(
            DataMarketCommand::IssueDelivery {
                request_id: "req-2".to_string(),
            }
            .action_request()
            .expect("issue delivery action"),
            DesktopControlActionRequest::IssueDataMarketDelivery {
                args: DesktopControlDataMarketIssueDeliveryArgs {
                    request_id: "req-2".to_string(),
                },
            }
        );
        assert_eq!(
            DataMarketCommand::RevokeGrant {
                request_id: "req-3".to_string(),
                action: DataMarketRevocationActionArg::Expire,
                confirm: true,
                reason_code: Some("ttl_elapsed".to_string()),
            }
            .action_request()
            .expect("revoke action"),
            DesktopControlActionRequest::RevokeDataMarketGrant {
                args: DesktopControlDataMarketRevokeGrantArgs {
                    request_id: "req-3".to_string(),
                    action: "expire".to_string(),
                    confirm: true,
                    reason_code: Some("ttl_elapsed".to_string()),
                },
            }
        );
        assert_eq!(
            DataMarketCommand::Snapshot
                .action_request()
                .expect("snapshot action"),
            DesktopControlActionRequest::GetDataMarketSnapshot
        );
        assert_eq!(
            DataMarketCommand::BuyerPublishRequest {
                asset_id: Some("data_asset.alpha".to_string()),
                refresh_market: true,
            }
            .action_request()
            .expect("buyer publish request action"),
            DesktopControlActionRequest::PublishDataMarketBuyerRequest {
                args: DesktopControlDataMarketBuyerRequestArgs {
                    asset_id: Some("data_asset.alpha".to_string()),
                    refresh_market: true,
                },
            }
        );
        assert!(
            DataMarketCommand::SellerImportRequest {
                event_id: "event.alpha".to_string(),
                relay_urls: vec!["wss://relay.example".to_string()],
                timeout_ms: 15_000,
            }
            .action_request()
            .is_err()
        );
        assert!(
            DataMarketCommand::BuyerImportResponse {
                event_id: "event.beta".to_string(),
                relay_urls: vec!["wss://relay.example".to_string()],
                timeout_ms: 15_000,
            }
            .action_request()
            .is_err()
        );
        assert_eq!(
            DataMarketCommand::ConsumeDelivery {
                output_dir: PathBuf::from("/tmp/data-market-consume"),
                delivery_bundle_id: Some("delivery.alpha".to_string()),
                request_id: Some("data_request.alpha".to_string()),
                grant_id: Some("grant.alpha".to_string()),
                asset_id: Some("asset.alpha".to_string()),
                refresh_market: true,
                overwrite: true,
            }
            .action_request()
            .expect("consume delivery action"),
            DesktopControlActionRequest::ResolveDataMarketDelivery {
                args: DesktopControlDataMarketResolveDeliveryArgs {
                    delivery_bundle_id: Some("delivery.alpha".to_string()),
                    request_id: Some("data_request.alpha".to_string()),
                    grant_id: Some("grant.alpha".to_string()),
                    asset_id: Some("asset.alpha".to_string()),
                    refresh_market: true,
                },
            }
        );
    }

    #[test]
    fn tassadar_commands_map_to_control_requests() {
        assert_eq!(
            TassadarCommand::Status.action_request(),
            DesktopControlActionRequest::GetTassadarStatus
        );
        assert_eq!(
            TassadarCommand::Play.action_request(),
            DesktopControlActionRequest::ToggleTassadarPlayback
        );
        assert_eq!(
            TassadarCommand::Pause.action_request(),
            DesktopControlActionRequest::PauseTassadarPlayback
        );
        assert_eq!(
            TassadarCommand::Reset.action_request(),
            DesktopControlActionRequest::ResetTassadarPlayback
        );
        assert_eq!(
            TassadarCommand::Refresh.action_request(),
            DesktopControlActionRequest::RefreshTassadar
        );
        assert_eq!(
            TassadarCommand::View {
                view: TassadarViewArg::Program,
            }
            .action_request(),
            DesktopControlActionRequest::SetTassadarView {
                view: DesktopControlTassadarView::Program,
            }
        );
        assert_eq!(
            TassadarCommand::Source {
                source: TassadarSourceArg::HybridWorkflow,
            }
            .action_request(),
            DesktopControlActionRequest::SetTassadarSourceMode {
                source_mode: DesktopControlTassadarSourceMode::HybridWorkflow,
            }
        );
        assert_eq!(
            TassadarCommand::Family {
                command: TassadarFamilyCommand::Set {
                    family: TassadarReplayFamilyArg::Learned9x9Fit,
                },
            }
            .action_request(),
            DesktopControlActionRequest::SetTassadarReplayFamily {
                family: DesktopControlTassadarReplayFamily::Learned9x9Fit,
            }
        );
        assert_eq!(
            TassadarCommand::Family {
                command: TassadarFamilyCommand::Next,
            }
            .action_request(),
            DesktopControlActionRequest::NextTassadarReplayFamily
        );
        assert_eq!(
            TassadarCommand::Case {
                command: TassadarNavigationCommand::Next,
            }
            .action_request(),
            DesktopControlActionRequest::NextTassadarCase
        );
        assert_eq!(
            TassadarCommand::Update {
                command: TassadarNavigationCommand::Prev,
            }
            .action_request(),
            DesktopControlActionRequest::PreviousTassadarUpdate
        );
        assert_eq!(
            TassadarCommand::ReadableLog {
                command: TassadarNavigationCommand::Next,
            }
            .action_request(),
            DesktopControlActionRequest::NextTassadarReadableLogLine
        );
        assert_eq!(
            TassadarCommand::Token {
                command: TassadarNavigationCommand::Prev,
            }
            .action_request(),
            DesktopControlActionRequest::PreviousTassadarTokenChunk
        );
        assert_eq!(
            TassadarCommand::Fact {
                command: TassadarNavigationCommand::Next,
            }
            .action_request(),
            DesktopControlActionRequest::NextTassadarFactLine
        );
        assert_eq!(
            TassadarCommand::Speed {
                command: TassadarSpeedCommand::Set {
                    speed_multiplier: 4,
                },
            }
            .action_request(),
            DesktopControlActionRequest::SetTassadarSpeed {
                speed_multiplier: 4,
            }
        );
        assert_eq!(
            TassadarCommand::Speed {
                command: TassadarSpeedCommand::Increase,
            }
            .action_request(),
            DesktopControlActionRequest::IncreaseTassadarSpeed
        );
        assert_eq!(
            TassadarCommand::Window {
                command: TassadarWindowCommand::Decrease,
            }
            .action_request(),
            DesktopControlActionRequest::DecreaseTassadarTraceWindow
        );
    }

    #[test]
    fn nip90_payment_commands_map_to_control_requests() {
        let daily = super::Nip90PaymentsCommand::Daily {
            date: "2026-03-14".to_string(),
        }
        .action_request()
        .expect("daily report action should build");
        assert!(matches!(
            daily,
            DesktopControlActionRequest::GetNip90SentPaymentsReport {
                report_date: Some(ref report_date),
                ..
            } if report_date == "2026-03-14"
        ));

        let window = super::Nip90PaymentsCommand::Window {
            start: "1773464400".to_string(),
            end: "1773550800".to_string(),
        }
        .action_request()
        .expect("window report action should build");
        assert_eq!(
            window,
            DesktopControlActionRequest::GetNip90SentPaymentsReport {
                start_epoch_seconds: 1_773_464_400,
                end_epoch_seconds: 1_773_550_800,
                report_date: None,
            }
        );
    }

    #[test]
    fn nip90_payment_report_parsers_accept_daily_and_rfc3339_windows() {
        let (start_epoch_seconds, end_epoch_seconds) =
            parse_local_daily_window("2026-03-14").expect("daily window should parse");
        assert!(end_epoch_seconds > start_epoch_seconds);
        assert_eq!(
            end_epoch_seconds.saturating_sub(start_epoch_seconds),
            24 * 60 * 60
        );

        assert_eq!(
            parse_report_boundary("1773464400").expect("epoch boundary"),
            1_773_464_400
        );
        assert_eq!(
            parse_report_boundary("2026-03-14T05:00:00+00:00").expect("rfc3339 boundary"),
            1_773_464_400
        );
    }

    #[test]
    fn nip90_payment_report_payload_decodes_and_serializes_for_json() {
        let report = sample_nip90_sent_payments_report();
        let payload = serde_json::to_value(&report).expect("serialize report");
        let decoded =
            parse_nip90_sent_payments_report(Some(&payload)).expect("report payload should decode");
        assert_eq!(decoded, report);
        assert_eq!(
            payload
                .get("payment_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            payload
                .get("total_sats_sent")
                .and_then(|value| value.as_u64()),
            Some(42)
        );
    }

    #[test]
    fn nip90_payment_report_lines_surface_required_totals() {
        let lines = nip90_sent_payments_report_lines(&sample_nip90_sent_payments_report());
        assert!(lines.iter().any(|line| line.contains("payment_count=2")));
        assert!(lines.iter().any(|line| line.contains("total_sats_sent=42")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("total_wallet_debit_sats=45"))
        );
        assert!(lines.iter().any(|line| line.contains("wss://relay.one")));
    }

    #[test]
    fn cli_accepts_global_json_after_nip90_payment_subcommand() {
        let cli = super::Cli::try_parse_from([
            "autopilotctl",
            "nip90-payments",
            "daily",
            "--date",
            "2026-03-14",
            "--json",
        ])
        .expect("cli should accept trailing global json flag");
        assert!(cli.json);
    }

    #[test]
    fn cli_accepts_global_json_after_data_market_subcommand() {
        let cli =
            super::Cli::try_parse_from(["autopilotctl", "data-market", "seller-status", "--json"])
                .expect("cli should accept trailing global json flag");
        assert!(cli.json);
    }

    #[test]
    fn training_status_lines_surface_control_and_artifact_planes() {
        let mut snapshot = sample_snapshot();
        snapshot.training.available = true;
        snapshot.training.source = "kernel_authority".to_string();
        snapshot.training.control_plane_state = "authority_projected".to_string();
        snapshot.training.artifact_plane_state = "staging_active".to_string();
        snapshot.training.run_count = 1;
        snapshot.training.active_run_count = 1;
        snapshot.training.accepted_outcome_count = 1;
        snapshot.training.environment_versions = vec!["2026.03.13".to_string()];
        snapshot.training.checkpoint_refs = vec!["checkpoint://decoder/base".to_string()];
        snapshot.training.contributing_participant_count = 1;
        snapshot.training.admitted_participant_count = 2;
        snapshot.training.validator_verified_count = 3;
        snapshot.training.sandbox_ready_profile_count = 1;
        snapshot.training.sandbox_active_job_count = 1;
        snapshot.training.contributor_set_revision = Some("contributors-7".to_string());
        snapshot.training.adapter_window_count = 1;
        snapshot.training.active_adapter_window_count = 1;
        snapshot.training.promotion_ready_window_count = 0;
        snapshot.training.contribution_count = 1;
        snapshot.training.contributor.available = true;
        snapshot.training.contributor.local_node_id = Some("device:test".to_string());
        snapshot.training.contributor.product_enabled = true;
        snapshot.training.contributor.backend_ready = true;
        snapshot.training.contributor.contributor_supported = true;
        snapshot.training.contributor.coordinator_match_supported = true;
        snapshot.training.contributor.authority_receipt_supported = true;
        snapshot.training.contributor.match_eligible = true;
        snapshot.training.contributor.assignment_state = "awaiting_assignment".to_string();
        snapshot.training.contributor.local_assignment_count = 1;
        snapshot.training.contributor.local_active_assignment_count = 1;
        snapshot.training.contributor.local_uploaded_count = 1;
        snapshot.training.contributor.settlement_trigger =
            Some("accepted_contribution".to_string());
        snapshot.training.contributor.execution_backends =
            vec!["apple_foundation_models".to_string()];
        snapshot.training.contributor.adapter_families = vec!["apple_adapter".to_string()];
        snapshot.training.contributor.adapter_formats =
            vec!["openagents.apple-fmadapter.v1".to_string()];
        snapshot.training.contributor.validator_policy_refs =
            vec!["policy://validator/apple_adapter/helpdesk".to_string()];
        snapshot.training.contributor.latest_window_id = Some("window-1".to_string());
        snapshot.training.contributor.latest_assignment_id = Some("assignment-1".to_string());
        snapshot.training.contributor.latest_payout_state =
            Some("accepted_pending_settlement".to_string());
        snapshot.training.contributor.readiness_detail = Some(
            "Contributor prerequisites satisfy the latest decentralized adapter window".to_string(),
        );
        snapshot.training.operator.available = true;
        snapshot.training.operator.workflow_state = "running".to_string();
        snapshot.training.operator.run_count = 1;
        snapshot.training.operator.active_run_count = 1;
        snapshot.training.operator.exported_run_count = 0;
        snapshot.training.runs.push(
            autopilot_desktop::desktop_control::DesktopControlTrainingRunStatus {
                training_run_id: "train-1".to_string(),
                status: "running".to_string(),
                training_policy_ref: "policy://train/weather".to_string(),
                environment_ref: "env.openagents.weather.agent".to_string(),
                environment_version: Some("2026.03.13".to_string()),
                checkpoint_family: "decoder".to_string(),
                validator_policy_ref: "policy://validator/training".to_string(),
                benchmark_package_count: 2,
                rollout_verification_eval_run_count: 1,
                expected_step_count: Some(64),
                completed_step_count: Some(21),
                final_checkpoint_ref: Some("checkpoint://decoder/base".to_string()),
                promotion_checkpoint_ref: None,
                accepted_outcome_id: None,
                best_eval_score_bps: Some(9_200),
                control_plane_state: "running".to_string(),
                artifact_plane_state: "artifacts_active".to_string(),
            },
        );
        snapshot.training.participants.push(
            autopilot_desktop::desktop_control::DesktopControlTrainingParticipantStatus {
                participant_id: "node-a".to_string(),
                visible_reason: "cluster_member".to_string(),
                admitted: true,
                contributing: true,
                priority_label: "selected".to_string(),
                deweight_reason: None,
                exclusion_reason: None,
            },
        );
        snapshot.training.windows.push(
            autopilot_desktop::desktop_control::DesktopControlAdapterTrainingWindowStatus {
                window_id: "window-1".to_string(),
                training_run_id: "train-1".to_string(),
                stage_id: "stage-a".to_string(),
                status: "active".to_string(),
                contributor_set_revision_id: "contributors-7".to_string(),
                total_contributions: 3,
                admitted_contributions: 2,
                accepted_contributions: 1,
                quarantined_contributions: 1,
                rejected_contributions: 0,
                replay_required_contributions: 1,
                uploaded_contributions: 2,
                held_out_average_score_bps: Some(9_100),
                benchmark_pass_rate_bps: Some(8_900),
                runtime_smoke_passed: Some(true),
                promotion_ready: false,
                gate_reason_codes: vec!["runtime_smoke_required".to_string()],
                ..autopilot_desktop::desktop_control::DesktopControlAdapterTrainingWindowStatus::default()
            },
        );
        snapshot.training.contributions.push(
            autopilot_desktop::desktop_control::DesktopControlAdapterContributionStatus {
                contribution_id: "contrib-1".to_string(),
                training_run_id: "train-1".to_string(),
                stage_id: "stage-a".to_string(),
                window_id: "window-1".to_string(),
                assignment_id: "assignment-1".to_string(),
                contributor_node_id: "device:test".to_string(),
                worker_id: "device:test".to_string(),
                validator_disposition: "accepted".to_string(),
                aggregation_eligibility: "eligible".to_string(),
                accepted_for_aggregation: true,
                upload_state: "validated".to_string(),
                payout_state: "accepted_pending_settlement".to_string(),
                settlement_trigger: Some("accepted_contribution".to_string()),
                ..autopilot_desktop::desktop_control::DesktopControlAdapterContributionStatus::default()
            },
        );
        snapshot.training.operator.runs.push(
            autopilot_desktop::desktop_control::DesktopControlAppleAdapterOperatorRunStatus {
                run_id: "apple-run-1".to_string(),
                package_name: "weather-helper".to_string(),
                author: "OpenAgents".to_string(),
                description: "Operator run".to_string(),
                license: "Apache-2.0".to_string(),
                train_dataset_path: "/tmp/train.jsonl".to_string(),
                held_out_dataset_path: "/tmp/held-out.jsonl".to_string(),
                created_at_epoch_ms: 1,
                updated_at_epoch_ms: 2,
                launched_at_epoch_ms: Some(2),
                evaluated_at_epoch_ms: Some(3),
                exported_at_epoch_ms: None,
                accepted_at_epoch_ms: None,
                launch_state: "completed".to_string(),
                export_state: "pending".to_string(),
                evaluation_state: "completed".to_string(),
                acceptance_state: "pending".to_string(),
                run_directory: "/tmp/apple-run-1".to_string(),
                staged_package_path: Some(
                    "/tmp/apple-run-1/staged/weather-helper.fmadapter".to_string()
                ),
                exported_package_path: None,
                completed_step_count: Some(21),
                expected_step_count: Some(64),
                average_loss_label: Some("0.125000".to_string()),
                held_out_pass_rate_bps: Some(9_000),
                held_out_average_score_bps: Some(9_200),
                runtime_smoke_passed: Some(true),
                runtime_smoke_digest: Some("sha256:smoke".to_string()),
                package_digest: Some("sha256:package".to_string()),
                adapter_identifier: Some("weather-helper".to_string()),
                authority:
                    autopilot_desktop::desktop_control::DesktopControlAppleAdapterOperatorAuthorityStatus::default(),
                progress: autopilot_desktop::desktop_control::DesktopControlAppleAdapterOperatorProgressStatus {
                    current_phase: Some("training".to_string()),
                    run_started_at_epoch_ms: Some(2),
                    phase_started_at_epoch_ms: Some(2),
                    last_heartbeat_at_epoch_ms: Some(4),
                    run_elapsed_ms: Some(2_000),
                    phase_elapsed_ms: Some(1_500),
                    eta_ms: Some(750),
                    current_epoch: Some(1),
                    expected_epochs: Some(1),
                    completed_steps: Some(21),
                    expected_steps: Some(64),
                    latest_loss_label: Some("0.125000".to_string()),
                    completed_eval_samples: Some(1),
                    expected_eval_samples: Some(3),
                    last_checkpoint_path: Some("/tmp/apple-run-1/checkpoints/final".to_string()),
                    telemetry_log_path: Some("/tmp/apple-run-1/telemetry.jsonl".to_string()),
                    latest_artifact_path: Some("/tmp/apple-run-1/checkpoints/final".to_string()),
                    latest_artifact_kind: Some("training_checkpoint".to_string()),
                    latest_resource_summary: Some(
                        "training_wall_clock_ms=2000 checkpoint_size_bytes=4096".to_string(),
                    ),
                    last_failure_phase: None,
                    last_failure_detail: None,
                },
                recent_events: vec![
                    autopilot_desktop::desktop_control::DesktopControlAppleAdapterOperatorEventStatus {
                        sequence: 7,
                        event_id: "apple-run-1-telemetry-000007".to_string(),
                        occurred_at_epoch_ms: 4,
                        phase: "training".to_string(),
                        kind: "heartbeat".to_string(),
                        detail: "Apple adapter training heartbeat step=21/64 eta_ms=750"
                            .to_string(),
                        epoch_index: Some(1),
                        expected_epochs: Some(1),
                        step_index: Some(21),
                        expected_steps: Some(64),
                        eval_sample_id: None,
                        eval_sample_index: None,
                        expected_eval_samples: None,
                        loss_label: Some("0.125000".to_string()),
                        eta_ms: Some(750),
                        checkpoint_path: Some(
                            "/tmp/apple-run-1/checkpoints/final".to_string(),
                        ),
                        artifact_path: Some(
                            "/tmp/apple-run-1/checkpoints/final".to_string(),
                        ),
                        artifact_kind: Some("training_checkpoint".to_string()),
                        failure_detail: None,
                        resource_summary: Some(
                            "training_wall_clock_ms=2000 checkpoint_size_bytes=4096".to_string(),
                        ),
                    },
                ],
                last_action: Some("Completed repo-native Apple adapter launch".to_string()),
                last_error: None,
                log_lines: vec!["launch complete".to_string()],
            },
        );

        let lines = training_status_lines(&snapshot);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("training: available=true source=kernel_authority"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("control=authority_projected artifact=staging_active"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("operator=running operator_runs=1/1"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("contributor_state=awaiting_assignment"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training run: id=train-1"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training participant: id=node-a"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training contributor: available=true node=device:test"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training window: id=window-1 run=train-1"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training contribution: id=contrib-1 run=train-1"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training operator run: id=apple-run-1"))
        );
        assert!(lines.iter().any(|line| line.contains("authority_accept=")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training operator live: id=apple-run-1 phase=training"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("telemetry=/tmp/apple-run-1/telemetry.jsonl"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("training operator event: id=apple-run-1 seq=7"))
        );
        assert!(lines.iter().any(|line| line.contains(
            "training operator note: export, runtime smoke, and authority acceptance do not by themselves prove benchmark-useful adapter quality"
        )));
    }

    #[test]
    fn remote_training_status_lines_surface_stale_and_series_posture() {
        let mut snapshot = sample_snapshot();
        snapshot.remote_training.available = true;
        snapshot.remote_training.source = "live_psionic_mirror".to_string();
        snapshot.remote_training.sync_state = "stale".to_string();
        snapshot.remote_training.last_synced_at_epoch_ms = Some(1_742_846_403_000);
        snapshot.remote_training.last_successful_sync_at_epoch_ms = Some(1_742_846_402_000);
        snapshot.remote_training.refresh_interval_ms = 1_000;
        snapshot.remote_training.run_count = 2;
        snapshot.remote_training.active_run_count = 1;
        snapshot.remote_training.full_series_run_count = 1;
        snapshot.remote_training.summary_only_run_count = 1;
        snapshot.remote_training.stale_run_count = 1;
        snapshot.remote_training.selected_run_id =
            Some("parameter-golf-runpod-single-h100-live-sample".to_string());
        snapshot.remote_training.compare_baseline_run_id = None;
        snapshot.remote_training.chart_anchor_ratio_milli = None;
        snapshot.remote_training.focused_topology_target = None;
        snapshot.remote_training.selected_provenance_artifact_role = None;
        snapshot.remote_training.last_error =
            Some("live mirror refresh failed and the app is using the cache".to_string());
        snapshot.remote_training.runs = vec![
            autopilot_desktop::desktop_control::DesktopControlRemoteTrainingRunStatus {
                provider: "google_cloud".to_string(),
                profile_id: "google_a2_ultragpu_1g".to_string(),
                lane_id: "psion.google_single_node.accelerated".to_string(),
                run_id: "psion-google-summary-only-sample".to_string(),
                repo_revision: "main@f7d62771".to_string(),
                track: autopilot_desktop::desktop_control::DesktopControlRemoteTrainingTrackStatus {
                    track_family: "psion".to_string(),
                    track_id: "psion.reference_pilot.single_node.v1".to_string(),
                    execution_class: "single_node".to_string(),
                    comparability_class: "not_comparable".to_string(),
                    proof_posture: "summary_only".to_string(),
                    public_equivalence_class: "not_applicable".to_string(),
                    score_law_ref: None,
                    artifact_cap_bytes: None,
                    wallclock_cap_seconds: None,
                    semantic_summary: "Google single-node Psion keeps provider-neutral run identity and summary truth without inventing a score law or loss curve.".to_string(),
                },
                primary_score: None,
                score_surface: None,
                result_classification: "completed_success".to_string(),
                series_status: "unavailable".to_string(),
                series_unavailable_reason: Some(
                    "no canonical optimizer-step loss series was retained".to_string(),
                ),
                semantic_summary: "Google single-node Psion keeps provider-neutral run identity and summary truth without inventing a score law or loss curve.".to_string(),
                topology_summary: None,
                last_heartbeat_at_ms: Some(1_742_760_920_000),
                heartbeat_age_ms: Some(15_000),
                stale: false,
                stale_reason: None,
                contract_state: "authoritative".to_string(),
                contract_error: None,
                bundle_cached: true,
                bundle_cache_path: Some("/tmp/cache/google.json".to_string()),
                bundle_digest: Some("digest-google".to_string()),
            },
            autopilot_desktop::desktop_control::DesktopControlRemoteTrainingRunStatus {
                provider: "run_pod".to_string(),
                profile_id: "runpod_h100_single_gpu".to_string(),
                lane_id: "parameter_golf.runpod_single_h100".to_string(),
                run_id: "parameter-golf-runpod-single-h100-live-sample".to_string(),
                repo_revision: "main@f7d62771".to_string(),
                track: autopilot_desktop::desktop_control::DesktopControlRemoteTrainingTrackStatus {
                    track_family: "parameter_golf".to_string(),
                    track_id: "non-record-unlimited-compute-16mb".to_string(),
                    execution_class: "single_node".to_string(),
                    comparability_class: "same_track_comparable".to_string(),
                    proof_posture: "runtime_measured".to_string(),
                    public_equivalence_class: "not_applicable".to_string(),
                    score_law_ref: Some("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md".to_string()),
                    artifact_cap_bytes: Some(16_000_000),
                    wallclock_cap_seconds: None,
                    semantic_summary: "Single-node PGOLF retains live optimizer telemetry under the non-record unlimited-compute 16MB submission track.".to_string(),
                },
                primary_score: None,
                score_surface: None,
                result_classification: "active".to_string(),
                series_status: "available".to_string(),
                series_unavailable_reason: None,
                semantic_summary: "Single-node PGOLF retains live optimizer telemetry under the non-record unlimited-compute 16MB submission track.".to_string(),
                topology_summary: Some("devices=1 latest=H100 util=91%".to_string()),
                last_heartbeat_at_ms: Some(1_742_846_402_000),
                heartbeat_age_ms: Some(4_200),
                stale: true,
                stale_reason: Some(
                    "latest heartbeat is 4200ms old and exceeded the 2500ms stale window"
                        .to_string(),
                ),
                contract_state: "authoritative".to_string(),
                contract_error: None,
                bundle_cached: true,
                bundle_cache_path: Some("/tmp/cache/runpod.json".to_string()),
                bundle_digest: Some("digest-runpod".to_string()),
            },
        ];
        snapshot.remote_training.selected_run = Some(
            autopilot_desktop::desktop_control::DesktopControlRemoteTrainingSelectedRunStatus {
                run: snapshot.remote_training.runs[1].clone(),
                source_root: Some("/tmp/psionic".to_string()),
                source_index_path: Some(
                    "/tmp/psionic/fixtures/training_visualization/remote_training_run_index_v2.json"
                        .to_string(),
                ),
                bundle: None,
            },
        );
        snapshot.remote_training.compare_baseline = None;

        let lines = remote_training_status_lines(&snapshot);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("state=stale") && line.contains("summary_only=1"))
        );
        assert!(lines.iter().any(|line| {
            line.contains("run=parameter-golf-runpod-single-h100-live-sample")
                && line.contains("stale=true")
        }));
    }

    #[test]
    fn remote_training_status_lines_cover_compare_focus_and_explorer() {
        let mut snapshot = sample_snapshot();
        snapshot.remote_training.compare = Some(
            autopilot_desktop::desktop_control::DesktopControlRemoteTrainingCompareStatus {
                state: "direct_compare".to_string(),
                selected_run_id: Some("homegolf-run-a".to_string()),
                baseline_run_id: "homegolf-run-b".to_string(),
                selected_track_id: Some("homegolf.public.weekly".to_string()),
                baseline_track_id: Some("homegolf.public.weekly".to_string()),
                score_metric_id: Some("win_rate".to_string()),
                score_direction: Some("higher_is_better".to_string()),
                anchor_mode: "global_step".to_string(),
                anchor_label: Some("anchor step 1280".to_string()),
                overlay_capable: true,
                delta_value: Some(0.0425),
                delta_summary: Some("+0.0425 ratio vs baseline // better".to_string()),
                caveat: "same-track direct delta".to_string(),
            },
        );
        snapshot.remote_training.topology_focus = Some(
            autopilot_desktop::desktop_control::DesktopControlRemoteTrainingTopologyFocusStatus {
                target: "device:gpu0".to_string(),
                label: "H100".to_string(),
                detail: "util 91.0% // mem 42.0GB // temp 67C".to_string(),
                matching_event_count: 2,
            },
        );
        snapshot.remote_training.provenance_focus = Some(
            autopilot_desktop::desktop_control::DesktopControlRemoteTrainingProvenanceFocusStatus {
                artifact_role: "training_log".to_string(),
                found: true,
                source_kind: Some("runtime_owned".to_string()),
                authoritative: Some(true),
                artifact_uri: Some("s3://bucket/run/log.jsonl".to_string()),
                artifact_digest: Some("sha256:abc123".to_string()),
                receipt_count: 2,
                detail: "primary optimizer log".to_string(),
            },
        );
        snapshot.remote_training.explorer = autopilot_desktop::desktop_control::DesktopControlRemoteTrainingExplorerStatus {
            available: true,
            load_state: "ready".to_string(),
            source_root: Some("/tmp/psionic".to_string()),
            index_path: Some("/tmp/psionic/fixtures/training/xtrain_explorer_index_v1.json".to_string()),
            snapshot_path: Some("/tmp/psionic/fixtures/training/xtrain_explorer_snapshot_v1.json".to_string()),
            last_refreshed_at_epoch_ms: Some(1_742_846_403_500),
            selected_view: "Evidence".to_string(),
            selected_snapshot_id: Some("snapshot.xtrain_explorer.window1231.v1".to_string()),
            selected_participant_id: Some("participant://miner-a".to_string()),
            network_id: Some("xtrain-public-testnet".to_string()),
            current_epoch_id: Some("epoch-12".to_string()),
            active_window_id: Some("window1231".to_string()),
            participant_count: 4,
            window_count: 1,
            checkpoint_count: 1,
            event_count: 4,
            authoritative_artifact_count: 7,
            run_surface_link_count: 1,
            summary: "participants=4 events=4 active_window=window1231 selected_participant=participant://miner-a".to_string(),
            last_error: None,
            last_action: Some("Loaded XTRAIN explorer snapshot".to_string()),
        };

        let lines = remote_training_status_lines(&snapshot);

        assert!(lines.iter().any(|line| {
            line.contains("remote training compare:")
                && line.contains("state=direct_compare")
                && line.contains("anchor=anchor step 1280")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("remote training topology focus:")
                && line.contains("target=device:gpu0")
                && line.contains("matching_events=2")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("remote training provenance focus:")
                && line.contains("role=training_log")
                && line.contains("source_kind=runtime_owned")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("remote training explorer:")
                && line.contains("state=ready")
                && line.contains("snapshot=snapshot.xtrain_explorer.window1231.v1")
        }));
    }

    #[test]
    fn wait_conditions_cover_nip28_readiness_messages_and_outbound_idle() {
        let mut snapshot = sample_snapshot();
        snapshot.local_runtime.runtime_ready = true;
        snapshot.gpt_oss.detected = true;
        snapshot.gpt_oss.ready = true;
        snapshot.gpt_oss.loaded = true;
        snapshot.nip28.available = true;
        snapshot.nip28.selected_channel_id = Some("chan-1".to_string());
        snapshot
            .nip28
            .recent_messages
            .push(DesktopControlNip28MessageStatus {
                event_id: "event-1".to_string(),
                author_pubkey: "a".repeat(64),
                content: "hello".to_string(),
                created_at: 1,
                reply_to_event_id: None,
                delivery_state: "confirmed".to_string(),
                delivery_error: None,
                attempt_count: 1,
            });
        snapshot.nip28.publishing_outbound_count = 0;
        snapshot.attnres_lab.playback_state = "running".to_string();
        snapshot.attnres_lab.running = true;

        assert!(WaitCondition::LocalRuntimeReady.matches(&snapshot));
        assert!(WaitCondition::GptOssReady.matches(&snapshot));
        assert!(!WaitCondition::GptOssUnloaded.matches(&snapshot));
        assert!(WaitCondition::Nip28Ready.matches(&snapshot));
        assert!(WaitCondition::Nip28MessagePresent.matches(&snapshot));
        assert!(WaitCondition::Nip28OutboundIdle.matches(&snapshot));
        assert!(WaitCondition::AttnResRunning.matches(&snapshot));
        assert!(!WaitCondition::AttnResPaused.matches(&snapshot));
        assert!(!WaitCondition::AttnResCompleted.matches(&snapshot));
        assert_eq!(
            WaitConditionArg::LocalRuntimeReady.into_condition(),
            WaitCondition::LocalRuntimeReady
        );
        assert_eq!(
            WaitConditionArg::LocalRuntimeReady.as_str(),
            "local-runtime-ready"
        );
        assert_eq!(
            WaitConditionArg::GptOssReady.into_condition(),
            WaitCondition::GptOssReady
        );
        assert_eq!(WaitConditionArg::GptOssReady.as_str(), "gpt-oss-ready");
        assert_eq!(
            WaitConditionArg::GptOssUnloaded.into_condition(),
            WaitCondition::GptOssUnloaded
        );
        assert_eq!(
            WaitConditionArg::GptOssUnloaded.as_str(),
            "gpt-oss-unloaded"
        );
        assert_eq!(
            WaitConditionArg::Nip28Ready.into_condition(),
            WaitCondition::Nip28Ready
        );
        assert_eq!(WaitConditionArg::Nip28Ready.as_str(), "nip28-ready");
        assert_eq!(
            WaitConditionArg::BuyModeTargetReady.into_condition(),
            WaitCondition::BuyModeTargetReady
        );
        assert_eq!(
            WaitConditionArg::BuyModeTargetReady.as_str(),
            "buy-mode-target-ready"
        );
        assert_eq!(
            WaitConditionArg::BuyModePaid.into_condition(),
            WaitCondition::BuyModePaid
        );
        assert_eq!(WaitConditionArg::BuyModePaid.as_str(), "buy-mode-paid");
        assert_eq!(
            WaitConditionArg::BuyModeFailed.into_condition(),
            WaitCondition::BuyModeFailed
        );
        assert_eq!(WaitConditionArg::BuyModeFailed.as_str(), "buy-mode-failed");
        assert_eq!(
            WaitConditionArg::AttnResRunning.into_condition(),
            WaitCondition::AttnResRunning
        );
        assert_eq!(WaitConditionArg::AttnResRunning.as_str(), "attnres-running");
        assert_eq!(
            WaitConditionArg::AttnResPaused.into_condition(),
            WaitCondition::AttnResPaused
        );
        assert_eq!(WaitConditionArg::AttnResPaused.as_str(), "attnres-paused");
        assert_eq!(
            WaitConditionArg::AttnResCompleted.into_condition(),
            WaitCondition::AttnResCompleted
        );
        assert_eq!(
            WaitConditionArg::AttnResCompleted.as_str(),
            "attnres-completed"
        );

        snapshot.attnres_lab.playback_state = "paused".to_string();
        snapshot.attnres_lab.running = false;
        assert!(WaitCondition::AttnResPaused.matches(&snapshot));
        assert!(!WaitCondition::AttnResCompleted.matches(&snapshot));

        snapshot.attnres_lab.playback_state = "training paused".to_string();
        assert!(WaitCondition::AttnResPaused.matches(&snapshot));

        snapshot.attnres_lab.playback_state = "completed".to_string();
        assert!(WaitCondition::AttnResCompleted.matches(&snapshot));

        snapshot.attnres_lab.playback_state = "run complete".to_string();
        assert!(WaitCondition::AttnResCompleted.matches(&snapshot));

        snapshot.gpt_oss.ready = false;
        snapshot.gpt_oss.loaded = false;
        assert!(WaitCondition::GptOssUnloaded.matches(&snapshot));
    }

    #[test]
    fn materialize_data_market_delivery_downloads_remote_payload_and_verifies_digest() {
        let temp_dir = TempDir::new().expect("temp dir");
        let buyer_path = write_identity(
            &temp_dir,
            "buyer",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        );
        let buyer = nostr::load_identity_from_path(buyer_path.as_path()).expect("buyer identity");
        let client = make_client(buyer_path);
        let payload_bytes = b"alpha dataset payload".to_vec();
        let digest = format!(
            "sha256:{}",
            data_market_sha256_hex(payload_bytes.as_slice())
        );
        let (base_url, handle) = spawn_http_server(
            BTreeMap::from([(
                "/payload.bin".to_string(),
                (payload_bytes.clone(), "application/octet-stream"),
            )]),
            1,
        );
        let payload = sample_delivery_payload(
            format!("{base_url}/payload.bin").as_str(),
            Some(digest.as_str()),
            buyer.public_key_hex.as_str(),
            buyer.public_key_hex.as_str(),
        );
        let output_dir = temp_dir.path().join("remote-consume");
        let summary =
            materialize_data_market_delivery(&client, &payload, output_dir.as_path(), true)
                .expect("materialize remote delivery");
        handle.join().expect("http server thread");

        let written =
            std::fs::read(output_dir.join("payload").join("payload.bin")).expect("written payload");
        assert_eq!(written, payload_bytes);
        assert_eq!(summary.delivery_digest.as_deref(), Some(digest.as_str()));
        assert!(
            summary
                .payload_source_kind
                .starts_with("remote_file:http://")
        );
    }

    #[test]
    fn materialize_data_market_delivery_resolves_encrypted_pointer_documents() {
        let temp_dir = TempDir::new().expect("temp dir");
        let buyer_path = write_identity(
            &temp_dir,
            "buyer",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        );
        let seller_path = write_identity(
            &temp_dir,
            "seller",
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
        );
        let buyer = nostr::load_identity_from_path(buyer_path.as_path()).expect("buyer identity");
        let seller =
            nostr::load_identity_from_path(seller_path.as_path()).expect("seller identity");
        let client = make_client(buyer_path);
        let payload_bytes = b"encrypted pointer payload".to_vec();
        let digest = format!(
            "sha256:{}",
            data_market_sha256_hex(payload_bytes.as_slice())
        );
        let seller_private_key = private_key_bytes(&seller).expect("seller private key");
        let buyer_public_key =
            compressed_pubkey_from_xonly_hex(buyer.public_key_hex.as_str()).expect("buyer pubkey");

        let (payload_base_url, payload_handle) = spawn_http_server(
            BTreeMap::from([(
                "/payload.bin".to_string(),
                (payload_bytes.clone(), "application/octet-stream"),
            )]),
            1,
        );
        let pointer_plaintext = json!({
            "schema_version": "oa.data_market.delivery_pointer.v1",
            "delivery_ref": format!("{payload_base_url}/payload.bin"),
        })
        .to_string();
        let pointer_ciphertext = nostr::nip44::encrypt(
            &seller_private_key,
            buyer_public_key.as_slice(),
            pointer_plaintext.as_str(),
        )
        .expect("encrypt pointer");
        let pointer_document = json!({
            "schema_version": "oa.data_market.encrypted_pointer.v1",
            "sender_pubkey": seller.public_key_hex,
            "ciphertext": pointer_ciphertext,
        })
        .to_string();
        let (pointer_base_url, pointer_handle) = spawn_http_server(
            BTreeMap::from([(
                "/pointer.json".to_string(),
                (pointer_document.into_bytes(), "application/json"),
            )]),
            1,
        );

        let payload = sample_delivery_payload(
            format!("{pointer_base_url}/pointer.json").as_str(),
            Some(digest.as_str()),
            seller.public_key_hex.as_str(),
            buyer.public_key_hex.as_str(),
        );
        let output_dir = temp_dir.path().join("encrypted-pointer-consume");
        let summary =
            materialize_data_market_delivery(&client, &payload, output_dir.as_path(), true)
                .expect("materialize encrypted pointer delivery");
        pointer_handle.join().expect("pointer http server thread");
        payload_handle.join().expect("payload http server thread");

        let written =
            std::fs::read(output_dir.join("payload").join("payload.bin")).expect("written payload");
        assert_eq!(written, payload_bytes);
        assert_eq!(summary.delivery_digest.as_deref(), Some(digest.as_str()));
    }

    #[test]
    fn parse_giftwrapped_delivery_pointer_decrypts_nested_pointer_content() {
        let temp_dir = TempDir::new().expect("temp dir");
        let buyer_path = write_identity(
            &temp_dir,
            "buyer",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        );
        let seller_path = write_identity(
            &temp_dir,
            "seller",
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
        );
        let buyer = nostr::load_identity_from_path(buyer_path.as_path()).expect("buyer identity");
        let seller =
            nostr::load_identity_from_path(seller_path.as_path()).expect("seller identity");
        let client = make_client(buyer_path);
        let payload_bytes = b"giftwrap payload".to_vec();
        let digest = format!(
            "sha256:{}",
            data_market_sha256_hex(payload_bytes.as_slice())
        );
        let pointer_json = json!({
            "schema_version": "oa.data_market.delivery_pointer.v1",
            "delivery_ref": "https://download.example/dataset.bin",
        })
        .to_string();
        let seller_private_key = private_key_bytes(&seller).expect("seller private key");
        let wrap = nostr::nip59::gift_wrap(
            nostr::UnsignedEvent {
                pubkey: seller.public_key_hex.clone(),
                created_at: 1,
                kind: 14,
                tags: vec![],
                content: pointer_json,
            },
            &seller_private_key,
            buyer.public_key_hex.as_str(),
        )
        .expect("giftwrap event");

        let parsed =
            parse_giftwrapped_delivery_pointer(&wrap, &client.target, Some(digest.as_str()), &[])
                .expect("parse giftwrapped pointer");

        assert_eq!(parsed.delivery_ref, "https://download.example/dataset.bin");
        assert_eq!(parsed.delivery_digest.as_deref(), Some(digest.as_str()));
    }

    #[test]
    fn materialize_data_market_delivery_rejects_remote_payload_without_digest() {
        let temp_dir = TempDir::new().expect("temp dir");
        let buyer_path = write_identity(
            &temp_dir,
            "buyer",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        );
        let buyer = nostr::load_identity_from_path(buyer_path.as_path()).expect("buyer identity");
        let client = make_client(buyer_path);
        let (base_url, handle) = spawn_http_server(
            BTreeMap::from([(
                "/payload.bin".to_string(),
                (b"digestless payload".to_vec(), "application/octet-stream"),
            )]),
            1,
        );
        let payload = sample_delivery_payload(
            format!("{base_url}/payload.bin").as_str(),
            None,
            buyer.public_key_hex.as_str(),
            buyer.public_key_hex.as_str(),
        );
        let output_dir = temp_dir.path().join("missing-digest");
        let error = materialize_data_market_delivery(&client, &payload, output_dir.as_path(), true)
            .expect_err("missing digest should fail");
        handle.join().expect("http server thread");
        assert!(
            error
                .to_string()
                .contains("requires delivery_digest so the payload can be verified")
        );
    }
}
