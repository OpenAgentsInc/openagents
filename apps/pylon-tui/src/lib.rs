mod bottom_pane;
mod slash_commands;
mod transcript;

use std::collections::BTreeMap;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use bottom_pane::{BottomPane, ComposerSubmission};
use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, MouseEvent, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use openagents_provider_substrate::{
    ProviderBackendHealth, ProviderDesiredMode, ProviderPersistedSnapshot,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde_json::Value;
use slash_commands::{ParsedSubmission, SlashCommandId};
use sysinfo::{Components, CpuRefreshKind, Disks, Networks, RefreshKind, System};
use transcript::{ActiveTurn, RetainedTranscript, TranscriptEntry, TranscriptRole};
use unicode_width::UnicodeWidthStr;

const TICK_RATE: Duration = Duration::from_millis(50);
const REFRESH_RATE: Duration = Duration::from_secs(10);
const GPU_REFRESH_RATE: Duration = Duration::from_secs(300);
const LOOKBACK_WINDOW_24H_MS: u64 = 86_400_000;
const LOCAL_CHAT_PLAIN_TEXT_POLICY: &str = "Reply in plain terminal text only. Do not use Markdown, LaTeX, HTML, tables, or code fences. For math, use plain text or simple Unicode, not TeX commands or delimiters.";

fn shell_border() -> Style {
    Style::default().fg(Color::Rgb(0x73, 0xc2, 0xfb))
}

fn shell_accent() -> Style {
    Style::default()
        .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
        .bg(Color::Rgb(0x13, 0x26, 0x3a))
        .add_modifier(Modifier::BOLD)
}

fn success_accent() -> Style {
    Style::default()
        .fg(Color::Rgb(0xf1, 0xff, 0xf3))
        .bg(Color::Rgb(0x12, 0x5f, 0x2d))
        .add_modifier(Modifier::BOLD)
}

fn warning_accent() -> Style {
    Style::default()
        .fg(Color::Rgb(0x20, 0x15, 0x00))
        .bg(Color::Rgb(0xff, 0xcd, 0x6b))
        .add_modifier(Modifier::BOLD)
}

fn danger_accent() -> Style {
    Style::default()
        .fg(Color::Rgb(0xff, 0xf2, 0xee))
        .bg(Color::Rgb(0x8c, 0x22, 0x22))
        .add_modifier(Modifier::BOLD)
}

fn muted_text() -> Style {
    Style::default().fg(Color::Rgb(0x8b, 0xc7, 0xff))
}

fn emphasis_text() -> Style {
    Style::default()
        .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
        .add_modifier(Modifier::BOLD)
}

fn panel_title_slash() -> Style {
    Style::default().fg(Color::Rgb(0x9b, 0xd6, 0xff))
}

fn key_label(label: &str) -> Span<'static> {
    Span::styled(format!("{label}: "), muted_text())
}

fn pulse_highlight_text() -> Style {
    Style::default()
        .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
        .bg(Color::Rgb(0x1c, 0x3b, 0x55))
        .add_modifier(Modifier::BOLD)
}

fn panel_title(title: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(" // ", panel_title_slash()),
        Span::styled(title.into(), shell_border().add_modifier(Modifier::BOLD)),
    ])
}

fn panel(title: &str, body: Text<'static>) -> Paragraph<'static> {
    Paragraph::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1))
                .title(panel_title(title))
                .style(shell_border()),
        )
        .wrap(Wrap { trim: false })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TuiLaunchConfig {
    config_path: PathBuf,
}

impl TuiLaunchConfig {
    fn from_args(args: Vec<String>) -> Result<Self> {
        let mut index = 0usize;
        let mut config_path = pylon::default_config_path();
        while index < args.len() {
            match args[index].as_str() {
                "--config-path" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| anyhow!("missing value for --config-path"))?;
                    config_path = PathBuf::from(value);
                    index += 1;
                }
                "--help" | "-h" => return Err(anyhow!(usage())),
                other => return Err(anyhow!("unexpected argument: {other}")),
            }
        }
        Ok(Self { config_path })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoadedState {
    snapshot: Option<ProviderPersistedSnapshot>,
    wallet_status: Option<pylon::WalletStatusReport>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Gemma4Status {
    loaded: bool,
    models: Vec<String>,
    note: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct OperatorPanelStats {
    desired_mode: ProviderDesiredMode,
    runtime_status: Option<String>,
    runtime_error: Option<String>,
    backend_label: Option<String>,
    provider_presence_online: bool,
    wallet_runtime_status: Option<String>,
    wallet_balance: Option<pylon::WalletBalanceSnapshot>,
    wallet_balance_live: bool,
    jobs_found_24h: u64,
    matching_jobs_24h: u64,
    jobs_processed_24h: u64,
    jobs_settled_24h: u64,
    session_earnings_sats: u64,
    settled_sats_24h: u64,
    settled_sats_lifetime: u64,
    total_earnings_sats: u64,
    awaiting_payment_jobs: u64,
    processing_jobs: u64,
    last_job_result: Option<String>,
    last_provider_event_at_ms: Option<u64>,
    recent_activity: Vec<String>,
    online_uptime_seconds: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WalletSurfaceState {
    runtime_status: Option<String>,
    runtime_detail: Option<String>,
    network: Option<String>,
    balance: Option<pylon::WalletBalanceSnapshot>,
    balance_live: bool,
    spark_address: Option<String>,
    bitcoin_address: Option<String>,
    latest_invoice: Option<pylon::PylonWalletInvoiceRecord>,
    recent_payments: Vec<pylon::PylonWalletPaymentRecord>,
    identity_path: Option<PathBuf>,
    last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StackerRank {
    name: &'static str,
    threshold_sats: u64,
}

const STACKER_RANKS: [StackerRank; 8] = [
    StackerRank {
        name: "Pleb",
        threshold_sats: 0,
    },
    StackerRank {
        name: "Drifter",
        threshold_sats: 1_000,
    },
    StackerRank {
        name: "Runner",
        threshold_sats: 10_000,
    },
    StackerRank {
        name: "Courier",
        threshold_sats: 100_000,
    },
    StackerRank {
        name: "Operator",
        threshold_sats: 1_000_000,
    },
    StackerRank {
        name: "Captain",
        threshold_sats: 10_000_000,
    },
    StackerRank {
        name: "Sovereign",
        threshold_sats: 50_000_000,
    },
    StackerRank {
        name: "King",
        threshold_sats: 100_000_000,
    },
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum SidebarView {
    #[default]
    Operate,
    Wallet,
    Inspect,
}

impl SidebarView {
    fn toggle(&mut self) {
        *self = match self {
            Self::Operate => Self::Wallet,
            Self::Wallet => Self::Inspect,
            Self::Inspect => Self::Operate,
        };
    }

    fn label(self) -> &'static str {
        match self {
            Self::Operate => "operate",
            Self::Wallet => "wallet",
            Self::Inspect => "inspect",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProviderCommandInFlight {
    Scan { started_at: Instant, seconds: u64 },
    Run { started_at: Instant, seconds: u64 },
}

impl ProviderCommandInFlight {
    fn state_label(&self) -> &'static str {
        match self {
            Self::Scan { .. } => "Scanning requests",
            Self::Run { .. } => "Listening for work",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Scan {
                started_at,
                seconds,
            } => format!(
                "scanning configured relays for {seconds}s ({})",
                format_duration(started_at.elapsed())
            ),
            Self::Run {
                started_at,
                seconds,
            } => format!(
                "running retained provider intake for {seconds}s ({})",
                format_duration(started_at.elapsed())
            ),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct LiveSystemStats {
    host_name: Option<String>,
    os_version: Option<String>,
    kernel_version: Option<String>,
    cpu_arch: Option<String>,
    physical_cpus: Option<usize>,
    cpu_brand: Option<String>,
    logical_cpus: usize,
    cpu_frequency_mhz: Option<u64>,
    cpu_usage_percent: Option<f32>,
    load_average: Option<(f64, f64, f64)>,
    used_memory_bytes: Option<u64>,
    available_memory_bytes: Option<u64>,
    total_memory_bytes: Option<u64>,
    used_swap_bytes: Option<u64>,
    free_swap_bytes: Option<u64>,
    total_swap_bytes: Option<u64>,
    uptime_seconds: Option<u64>,
    gpu_summary: Option<String>,
    disk_summary: Option<String>,
    disk_io_summary: Option<String>,
    network_summary: Option<String>,
    thermal_summary: Option<String>,
    power_summary: Option<String>,
    power_draw_summary: Option<String>,
}

#[derive(Debug)]
enum WorkerEvent {
    RefreshCompleted {
        loaded: Option<LoadedState>,
        installed_gemma_models: BTreeMap<String, u64>,
        operator_stats: OperatorPanelStats,
        wallet_surface: WalletSurfaceState,
        last_error: Option<String>,
        last_wallet_error: Option<String>,
        provider_presence_online: bool,
        nexus_treasury_health: Option<pylon::NexusTreasuryHealthSnapshot>,
    },
    StreamStarted(String),
    StreamDelta(String),
    StreamFinished,
    StreamFailed(String),
    ModelDownloadStarted {
        spec: pylon::GemmaDownloadSpec,
        total_bytes: Option<u64>,
    },
    ModelDownloadProgress {
        spec: pylon::GemmaDownloadSpec,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    ModelDownloadFinished {
        spec: pylon::GemmaDownloadSpec,
        file_bytes: u64,
    },
    ModelDownloadFailed {
        spec: pylon::GemmaDownloadSpec,
        error: String,
    },
    ModelSelectionFinished {
        output: String,
    },
    ModelSelectionFailed {
        error: String,
    },
    ModelUninstallFinished {
        output: String,
    },
    ModelUninstallFailed {
        error: String,
    },
    RelayRefreshFinished {
        report: pylon::RelayReport,
    },
    RelayRefreshFailed {
        error: String,
    },
    AnnouncementFinished {
        output: String,
    },
    AnnouncementFailed {
        error: String,
    },
    ProviderScanFinished {
        output: String,
    },
    ProviderScanFailed {
        error: String,
    },
    ProviderRunFinished {
        output: String,
    },
    ProviderRunFailed {
        error: String,
    },
    BuyerJobSubmitted {
        report: pylon::BuyerJobSubmitReport,
    },
    BuyerJobWatchObserved {
        entry: pylon::BuyerJobWatchEntry,
    },
    BuyerJobWatchFinished {
        report: pylon::BuyerJobWatchReport,
    },
    BuyerJobCommandFinished {
        title: String,
        output: String,
    },
    BuyerJobCommandFailed {
        error: String,
    },
    TranscriptReportFinished {
        title: String,
        output: String,
    },
    TranscriptReportFailed {
        title: String,
        error: String,
    },
    PayoutCommandFinished {
        title: String,
        output: String,
    },
    PayoutCommandFailed {
        error: String,
    },
    WalletCommandFinished {
        title: String,
        output: String,
    },
    WalletCommandFailed {
        error: String,
    },
}

#[derive(Clone, Copy, Debug)]
struct ActiveChatMetrics {
    started_at: Instant,
    first_token_at: Option<Instant>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GemmaDownloadProgressState {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

struct AppShell {
    config_path: PathBuf,
    loaded: Option<LoadedState>,
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    system_stats: LiveSystemStats,
    last_refresh_at: Option<Instant>,
    last_error: Option<String>,
    last_wallet_error: Option<String>,
    next_refresh_at: Instant,
    next_gpu_refresh_at: Instant,
    provider_presence_session_id: String,
    provider_presence_online: bool,
    next_provider_presence_heartbeat_at: Instant,
    refresh_in_flight: bool,
    should_quit: bool,
    transcript: RetainedTranscript,
    bottom_pane: BottomPane,
    worker_tx: mpsc::Sender<WorkerEvent>,
    worker_rx: mpsc::Receiver<WorkerEvent>,
    chat_in_flight: bool,
    active_chat_target: Option<String>,
    active_chat_text: String,
    active_chat_metrics: Option<ActiveChatMetrics>,
    chat_history: Vec<pylon::LocalGemmaChatMessage>,
    pending_chat_prompt: Option<String>,
    installed_gemma_models: BTreeMap<String, u64>,
    gemma_downloads: BTreeMap<String, GemmaDownloadProgressState>,
    operator_stats: OperatorPanelStats,
    wallet_surface: WalletSurfaceState,
    provider_command_in_flight: Option<ProviderCommandInFlight>,
    sidebar_view: SidebarView,
    transcript_follow_latest: bool,
    transcript_scroll_y: u16,
    transcript_wrap_width: u16,
    transcript_viewport_height: u16,
    transcript_max_scroll_y: u16,
    animation_started_at: Instant,
    live_activity_pulse_until: Option<Instant>,
    session_started_at_ms: u64,
    latest_paid_moment: Option<(u64, Instant)>,
    latest_rank_up_moment: Option<(String, String, Instant)>,
    nexus_treasury_health: Option<pylon::NexusTreasuryHealthSnapshot>,
    next_nexus_treasury_refresh_at: Instant,
}

impl AppShell {
    fn new(config_path: PathBuf) -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(sysinfo::MemoryRefreshKind::everything()),
        );
        let (worker_tx, worker_rx) = mpsc::channel();
        let mut transcript = RetainedTranscript::new();
        transcript.push_entry(TranscriptEntry::new(
            TranscriptRole::System,
            "Shell Ready",
            vec![String::from("Ask Gemma or type /help.")],
        ));
        Self {
            installed_gemma_models: installed_gemma_models(config_path.as_path()),
            config_path,
            loaded: None,
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            system_stats: LiveSystemStats::default(),
            last_refresh_at: None,
            last_error: None,
            last_wallet_error: None,
            next_refresh_at: Instant::now(),
            next_gpu_refresh_at: Instant::now() + GPU_REFRESH_RATE,
            provider_presence_session_id: pylon::new_provider_presence_session_id(),
            provider_presence_online: false,
            next_provider_presence_heartbeat_at: Instant::now(),
            refresh_in_flight: false,
            should_quit: false,
            transcript,
            bottom_pane: BottomPane::default(),
            worker_tx,
            worker_rx,
            chat_in_flight: false,
            active_chat_target: None,
            active_chat_text: String::new(),
            active_chat_metrics: None,
            chat_history: Vec::new(),
            pending_chat_prompt: None,
            gemma_downloads: BTreeMap::new(),
            operator_stats: OperatorPanelStats::default(),
            wallet_surface: WalletSurfaceState::default(),
            provider_command_in_flight: None,
            sidebar_view: SidebarView::Operate,
            transcript_follow_latest: true,
            transcript_scroll_y: 0,
            transcript_wrap_width: 0,
            transcript_viewport_height: 0,
            transcript_max_scroll_y: 0,
            animation_started_at: Instant::now(),
            live_activity_pulse_until: None,
            session_started_at_ms: current_epoch_ms_u64(),
            latest_paid_moment: None,
            latest_rank_up_moment: None,
            nexus_treasury_health: None,
            next_nexus_treasury_refresh_at: Instant::now(),
        }
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn should_refresh(&self) -> bool {
        !self.refresh_in_flight && Instant::now() >= self.next_refresh_at
    }

    fn schedule_refresh_now(&mut self) {
        self.next_refresh_at = Instant::now();
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if matches!(
            key,
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL)
        ) {
            self.should_quit = true;
            return;
        }

        if let Some(submission) = self.bottom_pane.handle_key(key) {
            self.handle_submission(submission);
            return;
        }

        match key.code {
            KeyCode::Tab => self.sidebar_view.toggle(),
            KeyCode::PageUp => self.scroll_transcript_up(10),
            KeyCode::PageDown => self.scroll_transcript_down(10),
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.scroll_transcript_up(3),
            MouseEventKind::ScrollDown => self.scroll_transcript_down(3),
            _ => {}
        }
    }

    fn handle_submission(&mut self, submission: ComposerSubmission) {
        let parsed = slash_commands::parse_submission(submission.text.as_str());
        let title = match &parsed {
            ParsedSubmission::Prompt(_) => String::from("Prompt"),
            ParsedSubmission::Command { spec, .. } => format!("Command /{}", spec.name),
            ParsedSubmission::UnknownCommand { name, .. } => format!("Command /{name}"),
        };
        let body = submission
            .text
            .lines()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        self.transcript
            .push_entry(TranscriptEntry::new(TranscriptRole::User, title, body));
        self.sync_transcript_scroll_after_update();

        let prompt = match parsed {
            ParsedSubmission::Prompt(prompt) => prompt,
            ParsedSubmission::Command { spec, args, .. } => match spec.id {
                SlashCommandId::Help => {
                    self.transcript.push_entry(TranscriptEntry::new(
                        TranscriptRole::System,
                        "Pylon Commands",
                        slash_commands::help_lines(),
                    ));
                    self.sync_transcript_scroll_after_update();
                    return;
                }
                SlashCommandId::Chat => args,
                SlashCommandId::Download => {
                    let model_id = args;
                    if model_id.is_empty() {
                        self.push_system_message(
                            "Command Error",
                            format!(
                                "Usage: /download <model>. Available: {}",
                                available_download_ids()
                            ),
                        );
                        return;
                    }
                    self.start_model_download(model_id);
                    return;
                }
                SlashCommandId::Model => {
                    let model_id = args;
                    if model_id.is_empty() {
                        self.push_system_message(
                            "Command Error",
                            format!(
                                "Usage: /model [model]. Available: {}",
                                available_download_ids()
                            ),
                        );
                        return;
                    }
                    self.start_model_selection(model_id);
                    return;
                }
                SlashCommandId::Uninstall => {
                    let model_id = args;
                    if model_id.is_empty() {
                        self.push_system_message(
                            "Command Error",
                            format!(
                                "Usage: /uninstall [model]. Available: {}",
                                available_download_ids()
                            ),
                        );
                        return;
                    }
                    self.start_model_uninstall(model_id);
                    return;
                }
                SlashCommandId::Announce => {
                    self.handle_announce_command(args);
                    return;
                }
                SlashCommandId::Provider => {
                    self.handle_provider_command(args);
                    return;
                }
                SlashCommandId::Job => {
                    self.handle_job_command(args);
                    return;
                }
                SlashCommandId::Jobs => {
                    self.handle_jobs_command(args);
                    return;
                }
                SlashCommandId::Earnings => {
                    self.handle_earnings_command(args);
                    return;
                }
                SlashCommandId::Receipts => {
                    self.handle_receipts_command(args);
                    return;
                }
                SlashCommandId::Activity => {
                    self.handle_activity_command(args);
                    return;
                }
                SlashCommandId::Payout => {
                    self.handle_payout_command(args);
                    return;
                }
                SlashCommandId::Relay => {
                    self.handle_relay_command(args);
                    return;
                }
                SlashCommandId::Wallet => {
                    self.handle_wallet_command(args);
                    return;
                }
            },
            ParsedSubmission::UnknownCommand { name, .. } => {
                self.push_system_message(
                    "Command Error",
                    format!("Unknown command /{name}. Type /help."),
                );
                return;
            }
        };
        if self.chat_in_flight {
            self.push_system_message("Chat Busy", "A local Gemma chat is already running.");
            return;
        }
        if prompt.is_empty() {
            self.push_system_message("Prompt Error", "Type a prompt.");
            return;
        }

        self.start_chat(prompt);
    }

    fn start_model_download(&mut self, model_id: String) {
        let Some(spec) = pylon::gemma_download_spec(model_id.as_str()) else {
            self.push_system_message(
                "Download Error",
                format!(
                    "Unknown Gemma model `{model_id}`. Available: {}",
                    available_download_ids()
                ),
            );
            return;
        };
        if self.gemma_downloads.contains_key(spec.id) {
            self.push_system_message(
                "Download Busy",
                format!("{} is already downloading.", spec.id),
            );
            return;
        }
        if self.installed_gemma_models.contains_key(spec.id) {
            self.push_system_message(
                "Already Installed",
                format!("{} is already in the local Pylon cache.", spec.id),
            );
            return;
        }

        self.gemma_downloads
            .insert(spec.id.to_string(), GemmaDownloadProgressState::default());
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::ModelDownloadFailed {
                        spec,
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                pylon::download_gemma_model(config_path.as_path(), spec.id, |event| match event {
                    pylon::GemmaDownloadEvent::Started { spec, total_bytes } => {
                        let _ = tx.send(WorkerEvent::ModelDownloadStarted { spec, total_bytes });
                    }
                    pylon::GemmaDownloadEvent::Progress {
                        spec,
                        downloaded_bytes,
                        total_bytes,
                    } => {
                        let _ = tx.send(WorkerEvent::ModelDownloadProgress {
                            spec,
                            downloaded_bytes,
                            total_bytes,
                        });
                    }
                    pylon::GemmaDownloadEvent::Finished {
                        spec, file_bytes, ..
                    } => {
                        let _ = tx.send(WorkerEvent::ModelDownloadFinished { spec, file_bytes });
                    }
                })
                .await
            });
            if let Err(error) = result {
                let _ = error_tx.send(WorkerEvent::ModelDownloadFailed {
                    spec,
                    error: error.to_string(),
                });
            }
        });
    }

    fn start_model_selection(&mut self, model_id: String) {
        let Some(spec) = pylon::gemma_download_spec_for_selector(model_id.as_str()) else {
            self.push_system_message(
                "Model Error",
                format!(
                    "Unknown Gemma model `{model_id}`. Available: {}",
                    available_download_ids()
                ),
            );
            return;
        };
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::ModelSelectionFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                let report =
                    pylon::select_local_gemma_model(config_path.as_path(), spec.id).await?;
                Ok::<String, anyhow::Error>(pylon::render_local_gemma_model_selection_report(
                    &report,
                ))
            });
            match result {
                Ok(output) => {
                    let _ = tx.send(WorkerEvent::ModelSelectionFinished { output });
                }
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::ModelSelectionFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Model", "Selecting local Gemma model...");
    }

    fn start_model_uninstall(&mut self, model_id: String) {
        let Some(spec) = pylon::gemma_download_spec_for_selector(model_id.as_str()) else {
            self.push_system_message(
                "Uninstall Error",
                format!(
                    "Unknown Gemma model `{model_id}`. Available: {}",
                    available_download_ids()
                ),
            );
            return;
        };
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            match pylon::uninstall_gemma_model(config_path.as_path(), spec.id) {
                Ok(report) => {
                    let _ = tx.send(WorkerEvent::ModelUninstallFinished {
                        output: pylon::render_gemma_uninstall_report(&report),
                    });
                }
                Err(error) => {
                    let _ = tx.send(WorkerEvent::ModelUninstallFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Uninstall", "Removing local Gemma cache...");
    }

    fn start_chat(&mut self, prompt: String) {
        let config = match pylon::load_config_or_default(self.config_path.as_path()) {
            Ok(config) => config,
            Err(error) => {
                self.push_system_message("Chat Error", error.to_string());
                return;
            }
        };
        let Some(snapshot) = self
            .loaded
            .as_ref()
            .and_then(|loaded| loaded.snapshot.as_ref())
        else {
            self.push_system_message(
                "Chat Error",
                "No local Gemma weights are visible right now.",
            );
            return;
        };
        let target = match pylon::resolve_local_gemma_chat_target_from_snapshot(&config, snapshot) {
            Ok(target) => target,
            Err(error) => {
                self.push_system_message("Chat Error", error.to_string());
                return;
            }
        };

        let messages = local_chat_request_messages(self.chat_history.as_slice(), prompt.as_str());
        self.chat_in_flight = true;
        self.active_chat_target = None;
        self.active_chat_text.clear();
        self.active_chat_metrics = Some(ActiveChatMetrics {
            started_at: Instant::now(),
            first_token_at: None,
        });
        self.pending_chat_prompt = Some(prompt);
        self.transcript.set_active_turn(ActiveTurn::new(
            TranscriptRole::Assistant,
            "Local Gemma",
            vec![String::from("Connecting to local Gemma...")],
        ));
        self.sync_transcript_scroll_after_update();

        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        let target_for_task = target.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::StreamFailed(error.to_string()));
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();
            let result = local.block_on(&runtime, async move {
                let stream_tx = tx.clone();
                pylon::stream_local_gemma_chat_messages_target(
                    config_path.as_path(),
                    &target_for_task,
                    messages.as_slice(),
                    |event| match event {
                        pylon::LocalGemmaChatEvent::Started { target } => {
                            let _ = stream_tx.send(WorkerEvent::StreamStarted(target.model));
                        }
                        pylon::LocalGemmaChatEvent::Delta(delta) => {
                            let _ = stream_tx.send(WorkerEvent::StreamDelta(delta));
                        }
                        pylon::LocalGemmaChatEvent::Finished { .. } => {
                            let _ = stream_tx.send(WorkerEvent::StreamFinished);
                        }
                    },
                )
                .await
            });
            if let Err(error) = result {
                let _ = error_tx.send(WorkerEvent::StreamFailed(error.to_string()));
            }
        });
    }

    fn drain_worker_events(&mut self) {
        while let Ok(event) = self.worker_rx.try_recv() {
            self.handle_worker_event(event);
        }
    }

    fn handle_worker_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::RefreshCompleted {
                loaded,
                installed_gemma_models,
                operator_stats,
                wallet_surface,
                last_error,
                last_wallet_error,
                provider_presence_online,
                nexus_treasury_health,
            } => {
                let previous_stats = self.operator_stats.clone();
                let operator_stats =
                    stabilize_operator_panel_stats(previous_stats.clone(), operator_stats);
                let activity_changed =
                    previous_stats.recent_activity != operator_stats.recent_activity;
                let paid_delta_sats = operator_stats
                    .session_earnings_sats
                    .saturating_sub(previous_stats.session_earnings_sats);
                if paid_delta_sats > 0 && self.last_refresh_at.is_some() {
                    let until = Instant::now() + Duration::from_millis(3200);
                    self.latest_paid_moment = Some((paid_delta_sats, until));
                    self.live_activity_pulse_until = Some(until);
                }
                let previous_rank =
                    stacker_rank_progress(previous_stats.total_earnings_sats).current.name;
                let current_rank =
                    stacker_rank_progress(operator_stats.total_earnings_sats).current.name;
                if previous_rank != current_rank && self.last_refresh_at.is_some() {
                    let until = Instant::now() + Duration::from_millis(4200);
                    self.latest_rank_up_moment = Some((
                        previous_rank.to_string(),
                        current_rank.to_string(),
                        until,
                    ));
                    self.live_activity_pulse_until = Some(until);
                }
                self.loaded = loaded;
                self.installed_gemma_models = installed_gemma_models;
                self.operator_stats = operator_stats;
                self.wallet_surface = wallet_surface;
                if activity_changed && !self.operator_stats.recent_activity.is_empty() {
                    self.live_activity_pulse_until =
                        Some(Instant::now() + Duration::from_millis(1800));
                }
                self.last_error = last_error;
                self.last_wallet_error = last_wallet_error;
                self.provider_presence_online = provider_presence_online;
                self.nexus_treasury_health = nexus_treasury_health;
                self.refresh_in_flight = false;
                self.last_refresh_at = Some(Instant::now());
                self.next_refresh_at = Instant::now() + REFRESH_RATE;
                self.next_provider_presence_heartbeat_at = Instant::now()
                    + if provider_presence_online {
                        pylon::provider_presence_heartbeat_interval()
                    } else {
                        REFRESH_RATE
                    };
                if self
                    .live_activity_pulse_until
                    .is_some_and(|until| Instant::now() >= until)
                {
                    self.live_activity_pulse_until = None;
                }
                if self
                    .latest_paid_moment
                    .as_ref()
                    .is_some_and(|(_, until)| Instant::now() >= *until)
                {
                    self.latest_paid_moment = None;
                }
                if self
                    .latest_rank_up_moment
                    .as_ref()
                    .is_some_and(|(_, _, until)| Instant::now() >= *until)
                {
                    self.latest_rank_up_moment = None;
                }
            }
            WorkerEvent::StreamStarted(model) => {
                self.active_chat_target = Some(model.clone());
                self.transcript.set_active_turn(ActiveTurn::new(
                    TranscriptRole::Assistant,
                    active_chat_title(model.as_str(), None),
                    vec![String::from("Waiting for tokens...")],
                ));
            }
            WorkerEvent::StreamDelta(delta) => {
                if let Some(metrics) = self.active_chat_metrics.as_mut() {
                    if metrics.first_token_at.is_none() {
                        metrics.first_token_at = Some(Instant::now());
                    }
                }
                self.active_chat_text.push_str(delta.as_str());
                self.transcript.set_active_turn(ActiveTurn::new(
                    TranscriptRole::Assistant,
                    active_chat_title(self.active_chat_target.as_deref().unwrap_or("chat"), None),
                    text_body_lines(self.active_chat_text.as_str()),
                ));
            }
            WorkerEvent::StreamFinished => {
                self.chat_in_flight = false;
                self.transcript.clear_active_turn();
                let metrics_summary = self
                    .active_chat_metrics
                    .take()
                    .map(|metrics| summarize_chat_metrics(metrics, self.active_chat_text.as_str()));
                if let Some(prompt) = self.pending_chat_prompt.take() {
                    self.chat_history
                        .push(pylon::LocalGemmaChatMessage::user(prompt));
                }
                if !self.active_chat_text.trim().is_empty() {
                    self.chat_history
                        .push(pylon::LocalGemmaChatMessage::assistant(
                            self.active_chat_text.clone(),
                        ));
                }
                self.transcript.push_entry(TranscriptEntry::new(
                    TranscriptRole::Assistant,
                    active_chat_title(
                        self.active_chat_target.as_deref().unwrap_or("chat"),
                        metrics_summary.as_ref(),
                    ),
                    text_body_lines(self.active_chat_text.as_str()),
                ));
                self.active_chat_target = None;
                self.active_chat_text.clear();
            }
            WorkerEvent::StreamFailed(error) => {
                let had_partial = !self.active_chat_text.trim().is_empty();
                self.chat_in_flight = false;
                self.pending_chat_prompt = None;
                self.transcript.clear_active_turn();
                self.active_chat_metrics = None;
                if had_partial {
                    self.transcript.push_entry(TranscriptEntry::new(
                        TranscriptRole::Assistant,
                        active_chat_title(
                            self.active_chat_target.as_deref().unwrap_or("chat"),
                            None,
                        ),
                        text_body_lines(self.active_chat_text.as_str()),
                    ));
                }
                self.push_system_message("Chat Error", error);
                self.active_chat_target = None;
                self.active_chat_text.clear();
            }
            WorkerEvent::ModelDownloadStarted { spec, total_bytes } => {
                self.gemma_downloads.insert(
                    spec.id.to_string(),
                    GemmaDownloadProgressState {
                        downloaded_bytes: 0,
                        total_bytes,
                    },
                );
                self.push_system_message(
                    "Download Started",
                    format!("Downloading {} from Hugging Face.", spec.id),
                );
            }
            WorkerEvent::ModelDownloadProgress {
                spec,
                downloaded_bytes,
                total_bytes,
            } => {
                self.gemma_downloads.insert(
                    spec.id.to_string(),
                    GemmaDownloadProgressState {
                        downloaded_bytes,
                        total_bytes,
                    },
                );
            }
            WorkerEvent::ModelDownloadFinished { spec, file_bytes } => {
                self.gemma_downloads.remove(spec.id);
                self.installed_gemma_models
                    .insert(spec.id.to_string(), file_bytes);
                self.push_system_message(
                    "Download Finished",
                    format!(
                        "{} installed at {}.",
                        spec.id,
                        pylon::gemma_model_path(self.config_path.as_path(), spec).display()
                    ),
                );
            }
            WorkerEvent::ModelDownloadFailed { spec, error } => {
                self.gemma_downloads.remove(spec.id);
                self.push_system_message("Download Error", format!("{}: {}", spec.id, error));
            }
            WorkerEvent::ModelSelectionFinished { output } => {
                self.push_system_lines("Model", text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::ModelSelectionFailed { error } => {
                self.push_system_message("Model Error", error);
            }
            WorkerEvent::ModelUninstallFinished { output } => {
                self.push_system_lines("Uninstall", text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::ModelUninstallFailed { error } => {
                self.push_system_message("Uninstall Error", error);
            }
            WorkerEvent::RelayRefreshFinished { report } => {
                self.push_system_lines("Relays", relay_report_lines(&report));
                self.schedule_refresh_now();
            }
            WorkerEvent::RelayRefreshFailed { error } => {
                self.push_system_message("Relay Error", error);
            }
            WorkerEvent::AnnouncementFinished { output } => {
                self.push_system_lines("Announcement", text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::AnnouncementFailed { error } => {
                self.push_system_message("Announcement Error", error);
            }
            WorkerEvent::ProviderScanFinished { output } => {
                self.provider_command_in_flight = None;
                self.push_system_lines("Provider Intake", text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::ProviderScanFailed { error } => {
                self.provider_command_in_flight = None;
                self.push_system_message("Provider Error", error);
            }
            WorkerEvent::ProviderRunFinished { output } => {
                self.provider_command_in_flight = None;
                self.push_system_lines("Provider Run", text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::ProviderRunFailed { error } => {
                self.provider_command_in_flight = None;
                self.push_system_message("Provider Error", error);
            }
            WorkerEvent::BuyerJobSubmitted { report } => {
                let request_event_id = report.request_event_id.clone();
                self.push_system_lines(
                    "Buyer Job",
                    text_body_lines(pylon::render_buyer_job_submit_report(&report).as_str()),
                );
                self.start_buyer_job_watch(Some(request_event_id), 30);
            }
            WorkerEvent::BuyerJobWatchObserved { entry } => {
                self.push_system_lines(
                    buyer_job_entry_title(&entry),
                    buyer_job_entry_lines(&entry),
                );
            }
            WorkerEvent::BuyerJobWatchFinished { report } => {
                self.push_system_lines(
                    "Buyer Job Watch",
                    text_body_lines(pylon::render_buyer_job_watch_report(&report).as_str()),
                );
            }
            WorkerEvent::BuyerJobCommandFinished { title, output } => {
                self.push_system_lines(title, text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::BuyerJobCommandFailed { error } => {
                self.push_system_message("Buyer Job Error", error);
            }
            WorkerEvent::TranscriptReportFinished { title, output } => {
                self.push_system_lines(title, text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::TranscriptReportFailed { title, error } => {
                self.push_system_message(title, error);
            }
            WorkerEvent::PayoutCommandFinished { title, output } => {
                self.push_system_lines(title, text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::PayoutCommandFailed { error } => {
                self.push_system_message("Payout Error", error);
            }
            WorkerEvent::WalletCommandFinished { title, output } => {
                self.push_system_lines(title, text_body_lines(output.as_str()));
                self.schedule_refresh_now();
            }
            WorkerEvent::WalletCommandFailed { error } => {
                self.push_system_message("Wallet Error", error);
            }
        }
        self.sync_transcript_scroll_after_update();
    }

    fn push_system_message(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.push_system_lines(title, vec![message.into()]);
    }

    fn push_system_lines(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.transcript
            .push_entry(TranscriptEntry::new(TranscriptRole::System, title, lines));
        self.sync_transcript_scroll_after_update();
    }

    fn spawn_transcript_report<F>(
        &mut self,
        title: &'static str,
        progress: impl Into<String>,
        job: F,
    ) where
        F: FnOnce(PathBuf) -> Result<String> + Send + 'static,
    {
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || match job(config_path) {
            Ok(output) => {
                let _ = tx.send(WorkerEvent::TranscriptReportFinished {
                    title: title.to_string(),
                    output,
                });
            }
            Err(error) => {
                let _ = tx.send(WorkerEvent::TranscriptReportFailed {
                    title: format!("{title} Error"),
                    error: error.to_string(),
                });
            }
        });
        self.push_system_message(title, progress);
    }

    fn handle_relay_command(&mut self, args: String) {
        let mut parts = args.split_whitespace();
        match parts.next() {
            None | Some("list") => match pylon::load_relay_report(self.config_path.as_path()) {
                Ok(report) => self.push_system_lines("Relays", relay_report_lines(&report)),
                Err(error) => self.push_system_message("Relay Error", error.to_string()),
            },
            Some("add") => {
                let Some(url) = parts.next() else {
                    self.push_system_message("Relay Error", "Usage: /relay add <ws://...>");
                    return;
                };
                match pylon::add_configured_relay(self.config_path.as_path(), url) {
                    Ok(report) => self.push_system_lines("Relays", relay_report_lines(&report)),
                    Err(error) => self.push_system_message("Relay Error", error.to_string()),
                }
            }
            Some("remove") => {
                let Some(url) = parts.next() else {
                    self.push_system_message("Relay Error", "Usage: /relay remove <ws://...>");
                    return;
                };
                match pylon::remove_configured_relay(self.config_path.as_path(), url) {
                    Ok(report) => self.push_system_lines("Relays", relay_report_lines(&report)),
                    Err(error) => self.push_system_message("Relay Error", error.to_string()),
                }
            }
            Some("refresh") => {
                let config_path = self.config_path.clone();
                let tx = self.worker_tx.clone();
                std::thread::spawn(move || {
                    let error_tx = tx.clone();
                    let runtime = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = error_tx.send(WorkerEvent::RelayRefreshFailed {
                                error: error.to_string(),
                            });
                            return;
                        }
                    };
                    let result = runtime.block_on(async move {
                        pylon::refresh_relay_report(config_path.as_path()).await
                    });
                    match result {
                        Ok(report) => {
                            let _ = tx.send(WorkerEvent::RelayRefreshFinished { report });
                        }
                        Err(error) => {
                            let _ = error_tx.send(WorkerEvent::RelayRefreshFailed {
                                error: error.to_string(),
                            });
                        }
                    }
                });
                self.push_system_message("Relays", "Refreshing configured relay connectivity...");
            }
            Some(other) => {
                self.push_system_message(
                    "Relay Error",
                    format!("Unknown relay command `{other}`. Use list, add, remove, or refresh."),
                );
            }
        }
    }

    fn handle_announce_command(&mut self, args: String) {
        let (action, title) = match args.trim() {
            "" | "show" => (
                pylon::AnnouncementAction::Show,
                "Loading announcement state...",
            ),
            "publish" => (
                pylon::AnnouncementAction::Publish,
                "Publishing provider announcement...",
            ),
            "refresh" => (
                pylon::AnnouncementAction::Refresh,
                "Refreshing provider announcement...",
            ),
            other => {
                self.push_system_message(
                    "Announcement Error",
                    format!("Unknown announce command `{other}`. Use show, publish, or refresh."),
                );
                return;
            }
        };
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::AnnouncementFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                let report = match action {
                    pylon::AnnouncementAction::Show => {
                        pylon::load_announcement_report(config_path.as_path()).await
                    }
                    pylon::AnnouncementAction::Publish => {
                        pylon::publish_announcement_report(config_path.as_path(), false).await
                    }
                    pylon::AnnouncementAction::Refresh => {
                        pylon::publish_announcement_report(config_path.as_path(), true).await
                    }
                }?;
                Ok::<String, anyhow::Error>(pylon::render_announcement_report(&report))
            });
            match result {
                Ok(output) => {
                    let _ = tx.send(WorkerEvent::AnnouncementFinished { output });
                }
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::AnnouncementFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Announcement", title);
    }

    fn handle_provider_command(&mut self, args: String) {
        let mut parts = args.split_whitespace();
        let Some(subcommand) = parts.next() else {
            self.push_system_message(
                "Provider Error",
                "Usage: /provider [scan|run] [--seconds <n>]",
            );
            return;
        };
        let action = subcommand.to_string();
        if !matches!(action.as_str(), "scan" | "run") {
            self.push_system_message(
                "Provider Error",
                format!("Unknown provider command `{action}`. Use scan or run."),
            );
            return;
        }

        let mut seconds = 5u64;
        while let Some(flag) = parts.next() {
            match flag {
                "--seconds" => {
                    let Some(raw) = parts.next() else {
                        self.push_system_message("Provider Error", "Missing value for --seconds.");
                        return;
                    };
                    match raw.parse::<u64>() {
                        Ok(value) if value > 0 => seconds = value,
                        Ok(_) | Err(_) => {
                            self.push_system_message(
                                "Provider Error",
                                format!("Invalid provider window seconds `{raw}`."),
                            );
                            return;
                        }
                    }
                }
                other => {
                    self.push_system_message(
                        "Provider Error",
                        format!("Unexpected provider flag `{other}`."),
                    );
                    return;
                }
            }
        }

        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        let action_for_thread = action.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let is_run = action_for_thread == "run";
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::ProviderScanFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                match if is_run { "run" } else { "scan" } {
                    "scan" => {
                        let report =
                            pylon::scan_provider_requests(config_path.as_path(), seconds).await?;
                        Ok::<(bool, String), anyhow::Error>((
                            false,
                            pylon::render_provider_intake_report(&report),
                        ))
                    }
                    "run" => {
                        let report =
                            pylon::run_provider_requests(config_path.as_path(), seconds).await?;
                        Ok::<(bool, String), anyhow::Error>((
                            true,
                            pylon::render_provider_run_report(&report),
                        ))
                    }
                    other => Err(anyhow::anyhow!(
                        "Unknown provider command `{other}`. Use scan or run."
                    )),
                }
            });
            match result {
                Ok((is_run, output)) => {
                    let _ = tx.send(if is_run {
                        WorkerEvent::ProviderRunFinished { output }
                    } else {
                        WorkerEvent::ProviderScanFinished { output }
                    });
                }
                Err(error) => {
                    let _ = error_tx.send(if is_run {
                        WorkerEvent::ProviderRunFailed {
                            error: error.to_string(),
                        }
                    } else {
                        WorkerEvent::ProviderScanFailed {
                            error: error.to_string(),
                        }
                    });
                }
            }
        });
        self.provider_command_in_flight = Some(if action == "run" {
            ProviderCommandInFlight::Run {
                started_at: Instant::now(),
                seconds,
            }
        } else {
            ProviderCommandInFlight::Scan {
                started_at: Instant::now(),
                seconds,
            }
        });
        self.push_system_message(
            if action == "run" {
                "Provider Run"
            } else {
                "Provider Intake"
            },
            if action == "run" {
                format!("Running retained provider intake for {}s...", seconds)
            } else {
                format!("Scanning configured relays for {}s...", seconds)
            },
        );
    }

    fn handle_jobs_command(&mut self, args: String) {
        let limit = match parse_tui_optional_limit(args.as_str(), "jobs") {
            Ok(limit) => limit.or(Some(10)),
            Err(error) => {
                self.push_system_message("Jobs Error", error.to_string());
                return;
            }
        };
        self.spawn_transcript_report(
            "Jobs",
            "Loading retained provider jobs...",
            move |config_path| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let report = runtime.block_on(async move {
                    pylon::load_jobs_report(config_path.as_path(), limit).await
                })?;
                Ok(pylon::render_jobs_report(&report))
            },
        );
    }

    fn handle_earnings_command(&mut self, args: String) {
        let trimmed = args.trim();
        if !(trimmed.is_empty() || trimmed == "show" || trimmed == "status") {
            self.push_system_message("Earnings Error", "Usage: /earnings");
            return;
        }
        self.spawn_transcript_report(
            "Earnings",
            "Loading retained provider earnings...",
            move |config_path| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let report = runtime.block_on(async move {
                    pylon::load_earnings_report(config_path.as_path()).await
                })?;
                Ok(pylon::render_earnings_report(&report))
            },
        );
    }

    fn handle_receipts_command(&mut self, args: String) {
        let limit = match parse_tui_optional_limit(args.as_str(), "receipts") {
            Ok(limit) => limit.or(Some(10)),
            Err(error) => {
                self.push_system_message("Receipts Error", error.to_string());
                return;
            }
        };
        self.spawn_transcript_report(
            "Receipts",
            "Loading retained provider receipts...",
            move |config_path| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let report = runtime.block_on(async move {
                    pylon::load_receipts_report(config_path.as_path(), limit).await
                })?;
                Ok(pylon::render_receipts_report(&report))
            },
        );
    }

    fn handle_activity_command(&mut self, args: String) {
        let limit = match parse_tui_optional_limit(args.as_str(), "activity") {
            Ok(limit) => limit.or(Some(10)),
            Err(error) => {
                self.push_system_message("Activity Error", error.to_string());
                return;
            }
        };
        self.spawn_transcript_report(
            "Activity",
            "Loading retained relay activity...",
            move |config_path| {
                let report = pylon::load_relay_activity_report(config_path.as_path(), limit)?;
                Ok(pylon::render_relay_activity_report(&report))
            },
        );
    }

    fn handle_job_command(&mut self, args: String) {
        let trimmed = args.trim();
        if let Some(remainder) = trimmed.strip_prefix("submit") {
            let request = match parse_tui_buyer_job_submit_request(remainder) {
                Ok(request) => request,
                Err(error) => {
                    self.push_system_message("Buyer Job Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                let error_tx = tx.clone();
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                        return;
                    }
                };
                let result = runtime.block_on(async move {
                    pylon::submit_buyer_job(config_path.as_path(), request).await
                });
                match result {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::BuyerJobSubmitted { report });
                    }
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Buyer Job", "Publishing retained NIP-90 buyer request...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("watch") {
            match parse_tui_buyer_job_watch_request(remainder) {
                Ok((request_event_id, seconds)) => {
                    self.start_buyer_job_watch(request_event_id, seconds);
                }
                Err(error) => self.push_system_message("Buyer Job Error", error.to_string()),
            }
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("history") {
            let limit = match parse_tui_buyer_job_history_request(remainder) {
                Ok(limit) => limit,
                Err(error) => {
                    self.push_system_message("Buyer Job Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                match pylon::load_buyer_job_history(config_path.as_path(), limit) {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFinished {
                            title: "Buyer Job History".to_string(),
                            output: pylon::render_buyer_job_history_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Buyer Job History", "Loading retained buyer job history...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("replay") {
            let request_event_id = match parse_tui_buyer_job_request_id(remainder, "job replay") {
                Ok(request_event_id) => request_event_id,
                Err(error) => {
                    self.push_system_message("Buyer Job Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                match pylon::load_buyer_job_replay(config_path.as_path(), request_event_id.as_str())
                {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFinished {
                            title: format!("Buyer Job Replay {}", report.entry.request_event_id),
                            output: pylon::render_buyer_job_replay_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Buyer Job Replay", "Replaying retained buyer job state...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("approve") {
            let request_event_id = match parse_tui_buyer_job_request_id(remainder, "job approve") {
                Ok(request_event_id) => request_event_id,
                Err(error) => {
                    self.push_system_message("Buyer Job Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                let error_tx = tx.clone();
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                        return;
                    }
                };
                let result = runtime.block_on(async move {
                    pylon::approve_buyer_job_payment(
                        config_path.as_path(),
                        request_event_id.as_str(),
                    )
                    .await
                });
                match result {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFinished {
                            title: "Buyer Payment".to_string(),
                            output: pylon::render_buyer_job_payment_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Buyer Payment", "Submitting buyer invoice payment...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("deny") {
            let request_event_id = match parse_tui_buyer_job_request_id(remainder, "job deny") {
                Ok(request_event_id) => request_event_id,
                Err(error) => {
                    self.push_system_message("Buyer Job Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                match pylon::deny_buyer_job_payment(
                    config_path.as_path(),
                    request_event_id.as_str(),
                ) {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFinished {
                            title: "Buyer Payment".to_string(),
                            output: pylon::render_buyer_job_payment_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Buyer Payment", "Denying buyer invoice payment...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("policy") {
            let mode = match parse_tui_buyer_job_policy_mode(remainder) {
                Ok(mode) => mode,
                Err(error) => {
                    self.push_system_message("Buyer Job Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                match pylon::apply_buyer_payment_policy(config_path.as_path(), mode) {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFinished {
                            title: "Buyer Payment Policy".to_string(),
                            output: pylon::render_buyer_payment_policy_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(WorkerEvent::BuyerJobCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Buyer Payment Policy", "Updating buyer payment policy...");
            return;
        }
        self.push_system_message(
            "Buyer Job Error",
            "Usage: /job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] [--request-json <json>] <prompt> | /job watch [<request_event_id>] [--seconds <n>] | /job history [--limit <n>] | /job replay <request_event_id> | /job approve <request_event_id> | /job deny <request_event_id> | /job policy [show|auto|manual]",
        );
    }

    fn start_buyer_job_watch(&mut self, request_event_id: Option<String>, seconds: u64) {
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        let request_event_id_for_thread = request_event_id.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let event_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::BuyerJobCommandFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                pylon::watch_buyer_jobs(
                    config_path.as_path(),
                    request_event_id_for_thread.as_deref(),
                    seconds,
                    |entry| {
                        let _ = event_tx.send(WorkerEvent::BuyerJobWatchObserved { entry });
                    },
                )
                .await
            });
            match result {
                Ok(report) => {
                    let _ = tx.send(WorkerEvent::BuyerJobWatchFinished { report });
                }
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::BuyerJobCommandFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message(
            "Buyer Job Watch",
            if let Some(request_event_id) = request_event_id.as_deref() {
                format!("Watching buyer job {request_event_id} for {}s...", seconds)
            } else {
                format!("Watching retained buyer jobs for {}s...", seconds)
            },
        );
    }

    fn handle_wallet_command(&mut self, args: String) {
        let trimmed = args.trim();
        if trimmed.is_empty() || trimmed == "show" {
            self.push_system_lines("Wallet", self.wallet_home_transcript_lines());
            return;
        }
        if trimmed == "receive" {
            self.start_wallet_receive();
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("withdraw") {
            self.start_wallet_withdraw_alias(remainder.trim());
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("recovery") {
            self.handle_wallet_recovery_command(remainder.trim());
            return;
        }

        let mut argv = vec![String::from("wallet")];
        if trimmed.is_empty() {
            argv.push(String::from("status"));
        } else {
            argv.extend(args.split_whitespace().map(ToString::to_string));
        }
        let command = match pylon::parse_wallet_command(argv.as_slice(), 0) {
            Ok(command) => command,
            Err(error) => {
                self.push_system_message("Wallet Error", error.to_string());
                return;
            }
        };
        let title = wallet_command_title(&command);
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                render_wallet_command_output(config_path.as_path(), &command).await
            });
            match result {
                Ok(output) => {
                    let _ = tx.send(WorkerEvent::WalletCommandFinished { title, output });
                }
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Wallet", "Running wallet command...");
    }

    fn wallet_home_transcript_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "Status: {}",
            self.wallet_surface
                .runtime_status
                .clone()
                .unwrap_or_else(|| "warming up".to_string())
        ));
        if let Some(network) = self.wallet_surface.network.as_deref() {
            lines.push(format!("Network: {network}"));
        }
        if let Some(balance) = self.wallet_surface.balance.as_ref() {
            lines.push(format!("Total balance: {}", format_sats(balance.total_sats)));
            lines.push(format!(
                "Balance mix: {} Spark, {} Lightning, {} on-chain",
                format_sats(balance.spark_sats),
                format_sats(balance.lightning_sats),
                format_sats(balance.onchain_sats)
            ));
        } else {
            lines.push("Total balance: unavailable".to_string());
        }
        lines.push(String::new());
        lines.push("Receive: /wallet receive".to_string());
        lines.push("Create invoice: /wallet invoice <sats>".to_string());
        lines.push("Withdraw: /wallet withdraw <lightning_invoice>".to_string());
        lines.push("Recovery: /wallet recovery".to_string());
        lines
    }

    fn start_wallet_receive(&mut self) {
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                let report = pylon::create_wallet_address_report(config_path.as_path()).await?;
                Ok::<String, anyhow::Error>(render_wallet_receive_output(&report))
            });
            match result {
                Ok(output) => {
                    let _ = tx.send(WorkerEvent::WalletCommandFinished {
                        title: "Wallet Receive".to_string(),
                        output,
                    });
                }
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Wallet", "Refreshing receive addresses...");
    }

    fn start_wallet_withdraw_alias(&mut self, remainder: &str) {
        if remainder.is_empty() {
            self.push_system_lines(
                "Wallet Withdraw",
                vec![
                    "Paste a Lightning invoice from your real wallet after the command.".to_string(),
                    "Usage: /wallet withdraw <lightning_invoice> [--amount-sats <n>]".to_string(),
                ],
            );
            return;
        }
        let mut argv = vec![String::from("wallet"), String::from("pay")];
        argv.extend(remainder.split_whitespace().map(ToString::to_string));
        let command = match pylon::parse_wallet_command(argv.as_slice(), 0) {
            Ok(command) => command,
            Err(error) => {
                self.push_system_message("Wallet Error", error.to_string());
                return;
            }
        };
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let error_tx = tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                    return;
                }
            };
            let result = runtime.block_on(async move {
                render_wallet_command_output(config_path.as_path(), &command).await
            });
            match result {
                Ok(output) => {
                    let _ = tx.send(WorkerEvent::WalletCommandFinished {
                        title: "Wallet Withdraw".to_string(),
                        output,
                    });
                }
                Err(error) => {
                    let _ = error_tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Wallet", "Submitting Lightning withdrawal...");
    }

    fn handle_wallet_recovery_command(&mut self, remainder: &str) {
        if remainder.is_empty() || remainder == "show" {
            let mut lines = vec![
                "Recovery phrase is hidden by default.".to_string(),
                "It controls both this Spark wallet and this node identity.".to_string(),
                "Run /wallet recovery reveal only in a private place you trust.".to_string(),
            ];
            if let Some(path) = self.wallet_surface.identity_path.as_ref() {
                lines.push(format!("Stored at: {}", path.display()));
            }
            self.push_system_lines("Wallet Recovery", lines);
            return;
        }
        if remainder != "reveal" {
            self.push_system_message(
                "Wallet Error",
                "Usage: /wallet recovery [show|reveal]",
            );
            return;
        }
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        std::thread::spawn(move || {
            let result = reveal_wallet_recovery_phrase(config_path.as_path());
            match result {
                Ok(output) => {
                    let _ = tx.send(WorkerEvent::WalletCommandFinished {
                        title: "Wallet Recovery".to_string(),
                        output,
                    });
                }
                Err(error) => {
                    let _ = tx.send(WorkerEvent::WalletCommandFailed {
                        error: error.to_string(),
                    });
                }
            }
        });
        self.push_system_message("Wallet Recovery", "Revealing the local recovery phrase...");
    }

    fn handle_payout_command(&mut self, args: String) {
        let trimmed = args.trim();
        if trimmed.is_empty() || trimmed == "show" || trimmed == "status" {
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                let error_tx = tx.clone();
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::PayoutCommandFailed {
                            error: error.to_string(),
                        });
                        return;
                    }
                };
                let result = runtime.block_on(async move {
                    pylon::load_payout_report(config_path.as_path(), Some(10)).await
                });
                match result {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::PayoutCommandFinished {
                            title: "Payout".to_string(),
                            output: pylon::render_payout_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::PayoutCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Payout", "Loading provider payout state...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("history") {
            let limit = match parse_tui_payout_history_request(remainder) {
                Ok(limit) => limit,
                Err(error) => {
                    self.push_system_message("Payout Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                let error_tx = tx.clone();
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::PayoutCommandFailed {
                            error: error.to_string(),
                        });
                        return;
                    }
                };
                let result = runtime.block_on(async move {
                    pylon::load_payout_report(config_path.as_path(), limit).await
                });
                match result {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::PayoutCommandFinished {
                            title: "Payout History".to_string(),
                            output: pylon::render_payout_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::PayoutCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Payout History", "Loading retained payout history...");
            return;
        }
        if let Some(remainder) = trimmed.strip_prefix("withdraw") {
            let (payment_request, amount_sats) = match parse_tui_payout_withdraw_request(remainder)
            {
                Ok(request) => request,
                Err(error) => {
                    self.push_system_message("Payout Error", error.to_string());
                    return;
                }
            };
            let config_path = self.config_path.clone();
            let tx = self.worker_tx.clone();
            std::thread::spawn(move || {
                let error_tx = tx.clone();
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::PayoutCommandFailed {
                            error: error.to_string(),
                        });
                        return;
                    }
                };
                let result = runtime.block_on(async move {
                    pylon::run_payout_withdrawal(
                        config_path.as_path(),
                        payment_request.as_str(),
                        amount_sats,
                    )
                    .await
                });
                match result {
                    Ok(report) => {
                        let _ = tx.send(WorkerEvent::PayoutCommandFinished {
                            title: "Payout Withdrawal".to_string(),
                            output: pylon::render_payout_withdrawal_report(&report),
                        });
                    }
                    Err(error) => {
                        let _ = error_tx.send(WorkerEvent::PayoutCommandFailed {
                            error: error.to_string(),
                        });
                    }
                }
            });
            self.push_system_message("Payout Withdrawal", "Submitting provider withdrawal...");
            return;
        }
        self.push_system_message(
            "Payout Error",
            "Usage: /payout | /payout history [--limit <n>] | /payout withdraw <payment_request> [--amount-sats <n>]",
        );
    }

    async fn refresh(&mut self) {
        self.refresh_system_stats();
        self.start_refresh();
    }

    fn start_refresh(&mut self) {
        if self.refresh_in_flight {
            return;
        }
        self.refresh_in_flight = true;
        let config_path = self.config_path.clone();
        let tx = self.worker_tx.clone();
        let session_id = self.provider_presence_session_id.clone();
        let provider_presence_online = self.provider_presence_online;
        let session_started_at_ms = self.session_started_at_ms;
        let heartbeat_due = Instant::now() >= self.next_provider_presence_heartbeat_at;
        let treasury_refresh_due = Instant::now() >= self.next_nexus_treasury_refresh_at;
        let current_nexus_treasury = self.nexus_treasury_health.clone();
        if treasury_refresh_due {
            self.next_nexus_treasury_refresh_at = Instant::now() + GPU_REFRESH_RATE;
        }
        std::thread::spawn(move || {
            let installed_gemma_models = installed_gemma_models(config_path.as_path());
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = tx.send(WorkerEvent::RefreshCompleted {
                        loaded: None,
                        installed_gemma_models,
                        operator_stats: OperatorPanelStats::default(),
                        wallet_surface: WalletSurfaceState::default(),
                        last_error: Some(error.to_string()),
                        last_wallet_error: None,
                        provider_presence_online,
                        nexus_treasury_health: current_nexus_treasury,
                    });
                    return;
                }
            };
            let event = runtime.block_on(async move {
                match pylon::ensure_local_setup(config_path.as_path()) {
                    Ok(_) => {
                        let config = match pylon::load_config_or_default(config_path.as_path()) {
                            Ok(config) => config,
                            Err(error) => {
                                return WorkerEvent::RefreshCompleted {
                                    loaded: None,
                                    installed_gemma_models,
                                    operator_stats: OperatorPanelStats::default(),
                                    wallet_surface: WalletSurfaceState::default(),
                                    last_error: Some(error.to_string()),
                                    last_wallet_error: None,
                                    provider_presence_online,
                                    nexus_treasury_health: current_nexus_treasury,
                                };
                            }
                        };
                        let nexus_treasury_health = if treasury_refresh_due {
                            pylon::fetch_nexus_treasury_health(&config)
                                .await
                                .ok()
                                .or(current_nexus_treasury)
                        } else {
                            current_nexus_treasury
                        };
                        let (wallet_status, last_wallet_error) =
                            match pylon::load_wallet_balance_status_report(config_path.as_path())
                                .await
                            {
                                Ok(report) => (Some(report), None),
                                Err(error) => (None, Some(error.to_string())),
                            };
                        match pylon::load_config_and_status(config_path.as_path()).await {
                            Ok((_, status)) => {
                                let provider_presence_online = sync_provider_presence_for_refresh(
                                    config_path.as_path(),
                                    session_id.as_str(),
                                    status.desired_mode,
                                    status.snapshot.as_ref(),
                                    provider_presence_online,
                                    heartbeat_due,
                                )
                                .await;
                                let ledger =
                                    pylon::load_ledger(config_path.as_path()).unwrap_or_default();
                                WorkerEvent::RefreshCompleted {
                                    loaded: Some(LoadedState {
                                        snapshot: status.snapshot.clone(),
                                        wallet_status: wallet_status.clone(),
                                    }),
                                    wallet_surface: build_wallet_surface(
                                        &config,
                                        wallet_status.as_ref(),
                                        &ledger,
                                    ),
                                    installed_gemma_models,
                                    operator_stats: compute_operator_panel_stats(
                                        status.desired_mode,
                                        provider_presence_online,
                                        wallet_status.as_ref(),
                                        status.snapshot.as_ref(),
                                        &ledger,
                                        session_started_at_ms,
                                    ),
                                    last_error: None,
                                    last_wallet_error,
                                    provider_presence_online,
                                    nexus_treasury_health,
                                }
                            }
                            Err(error) => {
                                let provider_presence_online = sync_provider_presence_for_refresh(
                                    config_path.as_path(),
                                    session_id.as_str(),
                                    ProviderDesiredMode::Offline,
                                    None,
                                    provider_presence_online,
                                    heartbeat_due,
                                )
                                .await;
                                let ledger =
                                    pylon::load_ledger(config_path.as_path()).unwrap_or_default();
                                WorkerEvent::RefreshCompleted {
                                    loaded: Some(LoadedState {
                                        snapshot: None,
                                        wallet_status: wallet_status.clone(),
                                    }),
                                    wallet_surface: build_wallet_surface(
                                        &config,
                                        wallet_status.as_ref(),
                                        &ledger,
                                    ),
                                    installed_gemma_models,
                                    operator_stats: compute_operator_panel_stats(
                                        ProviderDesiredMode::Offline,
                                        provider_presence_online,
                                        wallet_status.as_ref(),
                                        None,
                                        &ledger,
                                        session_started_at_ms,
                                    ),
                                    last_error: Some(error.to_string()),
                                    last_wallet_error,
                                    provider_presence_online,
                                    nexus_treasury_health,
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let provider_presence_online = sync_provider_presence_for_refresh(
                            config_path.as_path(),
                            session_id.as_str(),
                            ProviderDesiredMode::Offline,
                            None,
                            provider_presence_online,
                            heartbeat_due,
                        )
                        .await;
                        let ledger = pylon::load_ledger(config_path.as_path()).unwrap_or_default();
                        let wallet_surface = match pylon::load_config_or_default(config_path.as_path()) {
                            Ok(config) => build_wallet_surface(&config, None, &ledger),
                            Err(_) => WalletSurfaceState::default(),
                        };
                        WorkerEvent::RefreshCompleted {
                            loaded: None,
                            installed_gemma_models,
                            wallet_surface,
                            operator_stats: compute_operator_panel_stats(
                                ProviderDesiredMode::Offline,
                                provider_presence_online,
                                None,
                                None,
                                &ledger,
                                session_started_at_ms,
                            ),
                            last_error: Some(error.to_string()),
                            last_wallet_error: None,
                            provider_presence_online,
                            nexus_treasury_health: current_nexus_treasury,
                        }
                    }
                }
            });
            let _ = tx.send(event);
        });
    }

    async fn report_provider_presence_offline(&mut self) {
        if !self.provider_presence_online {
            return;
        }
        let _ = pylon::report_provider_presence_offline_for_config(
            self.config_path.as_path(),
            self.provider_presence_session_id.as_str(),
        )
        .await;
        self.provider_presence_online = false;
        self.next_provider_presence_heartbeat_at = Instant::now();
    }

    fn refresh_system_stats(&mut self) {
        self.system.refresh_memory();
        self.system.refresh_cpu_all();
        self.disks.refresh(false);
        self.networks.refresh(false);
        self.components.refresh(false);

        self.system_stats.host_name = System::host_name();
        self.system_stats.os_version = System::long_os_version();
        self.system_stats.kernel_version = Some(System::kernel_long_version());
        self.system_stats.cpu_arch = Some(System::cpu_arch());
        self.system_stats.physical_cpus = System::physical_core_count();
        self.system_stats.logical_cpus = self.system.cpus().len();
        self.system_stats.cpu_brand = self.system.cpus().iter().find_map(|cpu| {
            let brand = cpu.brand().trim();
            (!brand.is_empty()).then(|| brand.to_string())
        });
        self.system_stats.cpu_frequency_mhz = self
            .system
            .cpus()
            .first()
            .map(sysinfo::Cpu::frequency)
            .filter(|frequency| *frequency > 0);
        self.system_stats.cpu_usage_percent =
            (!self.system.cpus().is_empty()).then(|| self.system.global_cpu_usage());

        let load = System::load_average();
        self.system_stats.load_average = Some((load.one, load.five, load.fifteen));
        self.system_stats.used_memory_bytes = Some(self.system.used_memory());
        self.system_stats.available_memory_bytes = Some(self.system.available_memory());
        self.system_stats.total_memory_bytes = Some(self.system.total_memory());
        self.system_stats.used_swap_bytes = Some(self.system.used_swap());
        self.system_stats.free_swap_bytes = Some(self.system.free_swap());
        self.system_stats.total_swap_bytes = Some(self.system.total_swap());
        self.system_stats.uptime_seconds = Some(System::uptime());
        self.system_stats.disk_summary =
            detect_disk_summary(self.disks.list(), self.config_path.as_path());
        self.system_stats.disk_io_summary =
            detect_disk_io_summary(self.disks.list(), self.config_path.as_path());
        self.system_stats.network_summary = detect_network_summary(&self.networks);
        self.system_stats.thermal_summary = detect_thermal_summary(&self.components);

        if Instant::now() >= self.next_gpu_refresh_at || self.system_stats.gpu_summary.is_none() {
            self.system_stats.gpu_summary = detect_gpu_summary().ok();
            let (power_summary, power_draw_summary) = detect_power_status();
            self.system_stats.power_summary = power_summary;
            self.system_stats.power_draw_summary = power_draw_summary;
            self.next_gpu_refresh_at = Instant::now() + GPU_REFRESH_RATE;
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        let shell = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .title(Line::from(vec![
                Span::styled(" Pylon ", shell_accent()),
                Span::styled(" transcript shell ", shell_border()),
                Span::raw("  "),
                Span::styled(
                    format!("{} view", self.sidebar_view.label()),
                    shell_accent(),
                ),
            ]))
            .style(shell_border());
        let area = frame.area();
        let inner = shell.inner(area);
        frame.render_widget(shell, area);

        let vertical = Layout::vertical([
            Constraint::Length(self.header_height()),
            Constraint::Min(10),
            Constraint::Length(self.bottom_pane.height()),
            Constraint::Length(2),
        ])
        .split(inner);
        let middle = Layout::horizontal([Constraint::Percentage(67), Constraint::Percentage(33)])
            .split(vertical[1]);
        self.update_transcript_layout(middle[0]);

        frame.render_widget(self.header_panel(), vertical[0]);
        frame.render_widget(self.transcript_panel(), middle[0]);
        match self.sidebar_view {
            SidebarView::Operate => {
                let operator_height = (self.operator_lines().len() as u16 + 2).clamp(7, 9);
                let wallet_card_height = (self.wallet_card_lines().len() as u16 + 2).clamp(7, 9);
                let rank_height = (self.rank_lines().len() as u16 + 2).clamp(8, 10);
                let node_height = (self.summary_lines().len() as u16 + 2).clamp(6, 8);
                let right_column = Layout::vertical([
                    Constraint::Length(operator_height),
                    Constraint::Length(wallet_card_height),
                    Constraint::Length(rank_height),
                    Constraint::Length(node_height),
                    Constraint::Min(0),
                ])
                .split(middle[1]);
                frame.render_widget(self.operator_panel(), right_column[0]);
                frame.render_widget(self.wallet_card_panel(), right_column[1]);
                frame.render_widget(self.rank_panel(), right_column[2]);
                frame.render_widget(self.summary_panel(), right_column[3]);
            }
            SidebarView::Wallet => {
                let wallet_height = (self.wallet_overview_lines().len() as u16 + 2).clamp(7, 9);
                let receive_height = (self.wallet_receive_lines().len() as u16 + 2).clamp(8, 11);
                let move_height = (self.wallet_withdraw_lines().len() as u16 + 2).clamp(7, 10);
                let recovery_height =
                    (self.wallet_recovery_lines().len() as u16 + 2).clamp(7, 10);
                let right_column = Layout::vertical([
                    Constraint::Length(wallet_height),
                    Constraint::Length(receive_height),
                    Constraint::Length(move_height),
                    Constraint::Min(recovery_height),
                ])
                .split(middle[1]);
                frame.render_widget(self.wallet_overview_panel(), right_column[0]);
                frame.render_widget(self.wallet_receive_panel(), right_column[1]);
                frame.render_widget(self.wallet_withdraw_panel(), right_column[2]);
                frame.render_widget(self.wallet_recovery_panel(), right_column[3]);
            }
            SidebarView::Inspect => {
                let models_height = (self.model_lines().len() as u16 + 2).clamp(10, 15);
                let summary_height = (self.summary_lines().len() as u16 + 2).clamp(6, 8);
                let right_column = Layout::vertical([
                    Constraint::Length(models_height),
                    Constraint::Length(summary_height),
                    Constraint::Min(8),
                ])
                .split(middle[1]);
                frame.render_widget(self.models_panel(), right_column[0]);
                frame.render_widget(self.summary_panel(), right_column[1]);
                frame.render_widget(self.diagnostics_panel(), right_column[2]);
            }
        }
        self.bottom_pane.render(
            frame,
            vertical[2],
            shell_border(),
            shell_accent(),
            Some("Ask Gemma or type /help"),
        );
        frame.render_widget(self.footer_panel(), vertical[3]);
    }

    fn header_panel(&self) -> Paragraph<'static> {
        Paragraph::new(Text::from(self.header_lines()))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .title(panel_title("Mission Control"))
                    .style(shell_border()),
            )
            .wrap(Wrap { trim: false })
    }

    fn header_height(&self) -> u16 {
        4
    }

    fn header_lines(&self) -> Vec<Line<'static>> {
        let (operator_state, _) = self.operator_state_label_and_detail();
        let animation_phase = self.animation_phase();
        let animation_tick = self.animation_tick();

        let mut top_spans = vec![
            Span::styled("Pylon", shell_accent()),
            Span::raw("  "),
            Span::styled(operator_state.clone(), state_badge_style(&operator_state)),
            Span::raw("  "),
            Span::styled(self.hero_status_copy(), muted_text()),
        ];
        top_spans.extend(mission_control_signal_spans(
            operator_state.as_str(),
            animation_tick,
        ));
        let top_line = Line::from(top_spans);

        let mut bottom_spans = Vec::new();
        if let Some(refresh_at) = self.last_refresh_at {
            bottom_spans.push(Span::styled(
                format!("refresh {}", format_duration(refresh_at.elapsed())),
                muted_text(),
            ));
        } else {
            bottom_spans.push(Span::styled(
                format!("booting{}", animated_boot_suffix(animation_phase)),
                muted_text(),
            ));
        }
        if self.refresh_in_flight {
            bottom_spans.push(Span::raw("  "));
            bottom_spans.push(Span::styled(
                format!("refreshing{}", animated_boot_suffix(animation_phase)),
                muted_text(),
            ));
        }
        if let Some((amount_sats, _)) = self.visible_paid_moment() {
            bottom_spans.push(Span::raw("  "));
            bottom_spans.push(Span::styled(
                format!("+{} landed", format_sats(amount_sats)),
                success_accent(),
            ));
        } else if let Some((_, to_rank, _)) = self.visible_rank_up_moment() {
            bottom_spans.push(Span::raw("  "));
            bottom_spans.push(Span::styled(format!("ascended to {to_rank}"), shell_accent()));
        }
        if self.last_error.is_some() {
            bottom_spans.push(Span::raw("  "));
            bottom_spans.push(Span::styled(
                "refresh error",
                Style::default().fg(Color::Rgb(0xff, 0x9b, 0x7a)),
            ));
        }

        vec![top_line, Line::from(bottom_spans)]
    }

    fn summary_panel(&self) -> Paragraph<'static> {
        panel("Node", Text::from(self.summary_lines()))
    }

    fn models_panel(&self) -> Paragraph<'static> {
        panel("Gemma Models", Text::from(self.model_lines()))
    }

    fn operator_panel(&self) -> Paragraph<'static> {
        panel("Earnings", Text::from(self.operator_lines()))
    }

    fn wallet_overview_panel(&self) -> Paragraph<'static> {
        panel("Wallet", Text::from(self.wallet_overview_lines()))
    }

    fn wallet_card_panel(&self) -> Paragraph<'static> {
        panel("Wallet", Text::from(self.wallet_card_lines()))
    }

    fn wallet_receive_panel(&self) -> Paragraph<'static> {
        panel("Receive", Text::from(self.wallet_receive_lines()))
    }

    fn wallet_withdraw_panel(&self) -> Paragraph<'static> {
        panel("Withdraw", Text::from(self.wallet_withdraw_lines()))
    }

    fn wallet_recovery_panel(&self) -> Paragraph<'static> {
        panel("Recovery", Text::from(self.wallet_recovery_lines()))
    }

    fn rank_panel(&self) -> Paragraph<'static> {
        panel("Stacker Rank", Text::from(self.rank_lines()))
    }

    fn diagnostics_panel(&self) -> Paragraph<'static> {
        panel("Diagnostics", Text::from(self.diagnostics_lines()))
    }

    fn transcript_panel(&self) -> Paragraph<'static> {
        Paragraph::new(self.transcript_body())
            .scroll((self.transcript_scroll_y, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .title(panel_title(self.transcript_panel_title()))
                    .style(shell_border()),
            )
            .wrap(Wrap { trim: false })
    }

    fn footer_panel(&self) -> Paragraph<'static> {
        let mut spans = Vec::new();
        for (index, (key, label)) in self.footer_segments().into_iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(format!(" {key} "), shell_accent()));
            spans.push(Span::raw(label));
        }
        Paragraph::new(Line::from(spans))
        .block(Block::default().style(shell_border()))
    }

    fn footer_segments(&self) -> Vec<(&'static str, &'static str)> {
        match self.sidebar_view {
            SidebarView::Operate => vec![
                ("Ctrl+C", "quit"),
                ("Tab", "wallet"),
                ("jobs", "online"),
                ("/wallet receive", "cash in"),
                ("PgUp/PgDn", "scroll"),
            ],
            SidebarView::Wallet => vec![
                ("Ctrl+C", "quit"),
                ("Tab", "inspect"),
                ("/wallet receive", "addresses"),
                ("/wallet invoice", "request"),
                ("/wallet withdraw", "send out"),
            ],
            SidebarView::Inspect => vec![
                ("Ctrl+C", "quit"),
                ("Tab", "operate"),
                ("PgUp/PgDn", "scroll"),
                ("/wallet", "view"),
                ("/help", "commands"),
            ],
        }
    }

    fn hero_status_copy(&self) -> String {
        let (state_label, detail) = self.operator_state_label_and_detail();
        match state_label.as_str() {
            "Listening for work" => "Listening across relays for paid work".to_string(),
            "Earning now" => "Local Gemma is actively working a paid request".to_string(),
            "Waiting for payout" => "Work is done. Waiting for sats to settle".to_string(),
            "Ready to earn" => "Standing by for the next paid match".to_string(),
            "Preparing to earn" => detail.unwrap_or_else(|| "Booting local earnings lane".to_string()),
            "Needs attention" => detail.unwrap_or_else(|| "A local runtime issue needs attention".to_string()),
            _ => detail.unwrap_or(state_label),
        }
    }

    fn total_earnings_label(&self) -> String {
        if self.operator_stats.total_earnings_sats > 0 {
            if self.operator_stats.wallet_balance_live
                && matches!(
                    self.operator_stats.wallet_runtime_status.as_deref(),
                    Some("connected")
                )
            {
                format_sats(self.operator_stats.total_earnings_sats)
            } else {
                format!("{} retained", format_sats(self.operator_stats.total_earnings_sats))
            }
        } else {
            "0 sats".to_string()
        }
    }

    fn visible_paid_moment(&self) -> Option<(u64, Instant)> {
        self.latest_paid_moment
            .filter(|(_, until)| Instant::now() < *until)
    }

    fn paid_moment_is_visible(&self) -> bool {
        self.visible_paid_moment().is_some()
    }

    fn visible_rank_up_moment(&self) -> Option<(String, String, Instant)> {
        self.latest_rank_up_moment
            .as_ref()
            .filter(|(_, _, until)| Instant::now() < *until)
            .map(|(from, to, until)| (from.clone(), to.clone(), *until))
    }

    fn summary_lines(&self) -> Vec<Line<'static>> {
        let gemma = gemma4_status(self.loaded.as_ref());
        let health_label = if self.last_error.is_some() {
            "needs attention"
        } else if gemma.loaded {
            "healthy"
        } else {
            "warming up"
        };
        let lines = vec![
            Line::from(vec![
                key_label("Health"),
                Span::styled(health_label, state_badge_style(health_label)),
            ]),
            Line::from(vec![
                key_label("CPU"),
                Span::raw(format!(
                    "{}, {} usage",
                    self.system_stats
                        .cpu_brand
                        .as_deref()
                        .unwrap_or("unknown cpu"),
                    self.system_stats
                        .cpu_usage_percent
                        .map(format_percent)
                        .unwrap_or_else(|| "unknown".to_string())
                )),
            ]),
            Line::from(vec![
                key_label("Memory"),
                Span::raw(format!(
                    "{} / {}",
                    self.system_stats
                        .used_memory_bytes
                        .map(format_byte_size)
                        .unwrap_or_else(|| "unknown".to_string()),
                    self.system_stats
                        .total_memory_bytes
                        .map(format_byte_size)
                        .unwrap_or_else(|| "unknown".to_string())
                )),
            ]),
            Line::from(vec![
                key_label("GPU"),
                Span::raw(
                    self.system_stats
                        .gpu_summary
                        .as_deref()
                        .unwrap_or("not detected")
                        .to_string(),
                ),
            ]),
        ];
        lines
    }

    fn diagnostics_lines(&self) -> Vec<Line<'static>> {
        let gemma = gemma4_status(self.loaded.as_ref());
        let mut lines = vec![
            Line::from(format!(
                "host: {}  os: {}  arch: {}",
                self.system_stats.host_name.as_deref().unwrap_or("unknown"),
                self.system_stats.os_version.as_deref().unwrap_or("unknown"),
                self.system_stats.cpu_arch.as_deref().unwrap_or("unknown"),
            )),
            Line::from(format!(
                "uptime: {}  kernel: {}",
                self.system_stats
                    .uptime_seconds
                    .map(format_uptime)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .kernel_version
                    .as_deref()
                    .unwrap_or("unknown"),
            )),
            Line::from(format!("gemma loaded: {}", if gemma.loaded { "yes" } else { "no" })),
            Line::from(format!(
                "models: {}",
                comma_or_none(gemma.models.as_slice())
            )),
            Line::from(format!(
                "cpu: {}",
                self.system_stats
                    .cpu_brand
                    .as_deref()
                    .unwrap_or("unknown cpu"),
            )),
            Line::from(format!(
                "cores: {} physical / {} logical  freq: {}",
                self.system_stats
                    .physical_cpus
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats.logical_cpus,
                self.system_stats
                    .cpu_frequency_mhz
                    .map(format_frequency)
                    .unwrap_or_else(|| "unknown".to_string()),
            )),
            Line::from(format!(
                "usage: {}  load: {}",
                self.system_stats
                    .cpu_usage_percent
                    .map(format_percent)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .load_average
                    .map(format_load_average)
                    .unwrap_or_else(|| "unknown".to_string())
            )),
            Line::from(format!(
                "memory: {} used / {} total",
                self.system_stats
                    .used_memory_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .total_memory_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string())
            )),
            Line::from(format!(
                "swap: {} used / {} total",
                self.system_stats
                    .used_swap_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .total_swap_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string())
            )),
            Line::from(format!(
                "gpu: {}",
                self.system_stats
                    .gpu_summary
                    .as_deref()
                    .unwrap_or("not detected")
            )),
            Line::from(format!(
                "network: {}",
                self.system_stats
                    .network_summary
                    .as_deref()
                    .unwrap_or("unavailable")
            )),
            Line::from(format!(
                "disk: {}",
                self.system_stats
                    .disk_summary
                    .as_deref()
                    .unwrap_or("unavailable")
            )),
            Line::from(format!(
                "thermal: {}",
                self.system_stats
                    .thermal_summary
                    .as_deref()
                    .unwrap_or("unavailable")
            )),
        ];
        if let Some(power) = self.system_stats.power_summary.as_deref() {
            lines.push(Line::from(format!("power: {power}")));
        }
        if let Some(draw) = self.system_stats.power_draw_summary.as_deref() {
            lines.push(Line::from(format!("draw: {draw}")));
        }
        if let Some(error) = self.last_error.as_deref() {
            lines.push(Line::from(format!("refresh error: {error}")));
        } else {
            lines.push(Line::from(gemma.note));
        }
        lines
    }

    fn model_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if self.startup_status_is_loading() {
            lines.push(Line::from("runtime: loading current status"));
            lines.push(Line::from(""));
        } else {
            let runtime_ready = gemma_runtime_ready_models(self.loaded.as_ref());
            lines.push(Line::from(format!(
                "runtime ready: {}",
                if runtime_ready.is_empty() {
                    "none".to_string()
                } else {
                    runtime_ready.join(", ")
                }
            )));
            lines.push(Line::from(""));
        }
        for (index, spec) in pylon::gemma_download_specs().iter().enumerate() {
            if index > 0 {
                lines.push(Line::from(""));
            }
            let installed_bytes = self.installed_gemma_models.get(spec.id).copied();
            if let Some(progress) = self.gemma_downloads.get(spec.id) {
                lines.push(Line::from(format!("{}  downloading", spec.id)));
                lines.push(Line::from(format!(
                    "  {}  {}",
                    spec.quantization,
                    download_progress_label(progress)
                )));
                continue;
            }
            if let Some(file_bytes) = installed_bytes {
                lines.push(Line::from(format!("{}  installed", spec.id)));
                lines.push(Line::from(format!(
                    "  {}  {}",
                    spec.quantization,
                    format_byte_size(file_bytes)
                )));
            } else {
                lines.push(Line::from(format!("{}  missing", spec.id)));
                lines.push(Line::from(format!(
                    "  {}  /download {}",
                    spec.quantization, spec.id
                )));
            }
        }
        lines
    }

    fn operator_lines(&self) -> Vec<Line<'static>> {
        let animation_phase = self.animation_phase();
        let total_earnings = self.total_earnings_label();
        let mut lines = vec![];
        lines.extend(vec![
            Line::from(vec![
                key_label("Session stack"),
                Span::styled(
                    format_sats(self.operator_stats.session_earnings_sats),
                    if self.paid_moment_is_visible() {
                        pulse_highlight_text()
                    } else {
                        emphasis_text()
                    },
                ),
                Span::styled(
                    animated_stack_gain_suffix(animation_phase, self.paid_moment_is_visible()),
                    success_accent(),
                ),
            ]),
            Line::from(vec![
                key_label("Lifetime stack"),
                Span::styled(total_earnings, emphasis_text()),
            ]),
            Line::from(vec![
                key_label("Active now"),
                Span::raw(format!(
                    "{} processing, {} awaiting payout",
                    format_u64_with_commas(self.operator_stats.processing_jobs),
                    format_u64_with_commas(self.operator_stats.awaiting_payment_jobs)
                )),
                Span::styled(
                    animated_right_now_suffix(
                        self.operator_stats.processing_jobs,
                        self.operator_stats.awaiting_payment_jobs,
                        animation_phase,
                    ),
                    if self.operator_stats.processing_jobs > 0 {
                        shell_accent()
                    } else {
                        muted_text()
                    },
                ),
            ]),
        ]);
        if let Some((amount_sats, _)) = self.visible_paid_moment() {
            lines.push(Line::from(vec![
                key_label("Fresh payout"),
                Span::styled(format!("+{} just landed", format_sats(amount_sats)), success_accent()),
            ]));
        }
        if let Some(wallet_error) = self.last_wallet_error.as_deref() {
            if self.operator_stats.wallet_balance.is_none() {
                lines.push(Line::from(vec![
                    key_label("Wallet"),
                    Span::raw(format!("Issue: {wallet_error}")),
                ]));
            }
        }
        lines
    }

    fn wallet_overview_lines(&self) -> Vec<Line<'static>> {
        let wallet_status = self
            .wallet_surface
            .runtime_status
            .as_deref()
            .unwrap_or("warming up");
        let wallet_status_label = wallet_status.to_string();
        let balance = self
            .wallet_surface
            .balance
            .as_ref()
            .map(|balance| format_sats(balance.total_sats))
            .unwrap_or_else(|| "unavailable".to_string());
        let network = self
            .wallet_surface
            .network
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let mix = self
            .wallet_surface
            .balance
            .as_ref()
            .map(|balance| {
                format!(
                    "{} Spark, {} Lightning, {} on-chain",
                    format_sats(balance.spark_sats),
                    format_sats(balance.lightning_sats),
                    format_sats(balance.onchain_sats)
                )
            })
            .unwrap_or_else(|| "Balance mix appears after the next wallet refresh.".to_string());
        let mut lines = vec![
            Line::from(vec![
                key_label("Status"),
                Span::styled(wallet_status_label, state_badge_style(wallet_status)),
            ]),
            Line::from(vec![key_label("Network"), Span::raw(network)]),
            Line::from(vec![
                key_label("Total balance"),
                Span::styled(balance, emphasis_text()),
            ]),
        ];
        if let Some(last_paid) = self.latest_wallet_receive_summary() {
            lines.push(Line::from(vec![key_label("Last paid"), Span::raw(last_paid)]));
        }
        lines.extend([
            Line::from(vec![key_label("Balance mix"), Span::raw(mix)]),
        ]);
        if let Some(detail) = self.wallet_surface.runtime_detail.as_deref() {
            lines.push(Line::from(vec![
                key_label("Runtime"),
                Span::raw(detail.to_string()),
            ]));
        } else if let Some(error) = self.wallet_surface.last_error.as_deref() {
            lines.push(Line::from(vec![
                key_label("Runtime"),
                Span::raw(error.to_string()),
            ]));
        }
        lines
    }

    fn wallet_card_lines(&self) -> Vec<Line<'static>> {
        let wallet_status = self
            .wallet_surface
            .runtime_status
            .as_deref()
            .unwrap_or("warming up");
        let total_balance = self
            .wallet_surface
            .balance
            .as_ref()
            .map(|balance| format_sats(balance.total_sats))
            .unwrap_or_else(|| "unavailable".to_string());
        let receive_hint = if self.wallet_surface.spark_address.is_some()
            || self.wallet_surface.bitcoin_address.is_some()
        {
            "Spark + Bitcoin ready".to_string()
        } else {
            "/wallet receive".to_string()
        };
        let mut lines = vec![
            Line::from(vec![
                key_label("Status"),
                Span::styled(wallet_status.to_string(), state_badge_style(wallet_status)),
            ]),
            Line::from(vec![
                key_label("Total balance"),
                Span::styled(total_balance, emphasis_text()),
            ]),
            Line::from(vec![key_label("Receive"), Span::raw(receive_hint)]),
            Line::from(vec![
                key_label("Withdraw"),
                Span::raw("/wallet withdraw <lightning_invoice>"),
            ]),
        ];
        if let Some(last_paid) = self.latest_wallet_receive_summary() {
            lines.push(Line::from(vec![key_label("Last paid"), Span::raw(last_paid)]));
        }
        lines
    }

    fn wallet_receive_lines(&self) -> Vec<Line<'static>> {
        let spark_address = self
            .wallet_surface
            .spark_address
            .as_deref()
            .map(abbreviate_wallet_value)
            .unwrap_or_else(|| "Run /wallet receive to refresh addresses".to_string());
        let bitcoin_address = self
            .wallet_surface
            .bitcoin_address
            .as_deref()
            .map(abbreviate_wallet_value)
            .unwrap_or_else(|| "Run /wallet receive to refresh addresses".to_string());
        let fresh_invoice = self
            .wallet_surface
            .latest_invoice
            .as_ref()
            .map(|invoice| format!("{} ready", format_sats(invoice.amount_sats)))
            .unwrap_or_else(|| "Run /wallet invoice <sats> for a Lightning invoice".to_string());
        vec![
            Line::from(vec![key_label("Spark address"), Span::raw(spark_address)]),
            Line::from(vec![
                key_label("Bitcoin address"),
                Span::raw(bitcoin_address),
            ]),
            Line::from(vec![key_label("Fresh invoice"), Span::raw(fresh_invoice)]),
            Line::from("[TIP] /wallet receive shows full addresses in the transcript.".to_string()),
        ]
    }

    fn wallet_withdraw_lines(&self) -> Vec<Line<'static>> {
        let recent_flow = self
            .wallet_surface
            .recent_payments
            .first()
            .map(|payment| {
                format!(
                    "{} {} {} ago",
                    payment.direction,
                    format_sats(payment.amount_sats),
                    format_elapsed_since_ms(payment.updated_at_ms, current_epoch_ms_u64())
                )
            })
            .unwrap_or_else(|| "No retained wallet payments yet.".to_string());
        vec![
            Line::from(vec![
                key_label("Send to wallet"),
                Span::raw("/wallet withdraw <lightning_invoice>"),
            ]),
            Line::from(vec![
                key_label("Direct pay"),
                Span::raw("/wallet pay <lightning_invoice>"),
            ]),
            Line::from(vec![key_label("Recent flow"), Span::raw(recent_flow)]),
            Line::from(
                "[TIP] Paste a Lightning invoice from your real wallet to move sats out."
                    .to_string(),
            ),
        ]
    }

    fn wallet_recovery_lines(&self) -> Vec<Line<'static>> {
        let path = self
            .wallet_surface
            .identity_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Waiting for local config".to_string());
        vec![
            Line::from(vec![key_label("Phrase"), Span::raw("Hidden by default")]),
            Line::from(vec![key_label("Stored at"), Span::raw(path)]),
            Line::from(vec![
                key_label("Reveal"),
                Span::raw("/wallet recovery reveal"),
            ]),
            Line::from(
                "[WARN] The recovery phrase controls both this wallet and this node identity."
                    .to_string(),
            ),
        ]
    }

    fn latest_wallet_receive_summary(&self) -> Option<String> {
        self.wallet_surface
            .recent_payments
            .iter()
            .filter(|payment| payment.direction.eq_ignore_ascii_case("receive"))
            .filter(|payment| {
                payment.status.eq_ignore_ascii_case("completed")
                    || payment.status.eq_ignore_ascii_case("settled")
            })
            .max_by_key(|payment| payment.updated_at_ms)
            .or_else(|| {
                self.wallet_surface
                    .recent_payments
                    .iter()
                    .filter(|payment| payment.direction.eq_ignore_ascii_case("receive"))
                    .max_by_key(|payment| payment.updated_at_ms)
            })
            .map(|payment| {
                format!(
                    "{} {} ago",
                    format_sats(payment.amount_sats),
                    format_elapsed_since_ms(payment.updated_at_ms, current_epoch_ms_u64())
                )
            })
    }

    fn rank_lines(&self) -> Vec<Line<'static>> {
        let progress = stacker_rank_progress(self.operator_stats.total_earnings_sats);
        let animation_tick = self.animation_tick();
        let mut lines = Vec::new();
        let rank_up_visible = self.visible_rank_up_moment().is_some();
        if let Some((from_rank, to_rank, _)) = self.visible_rank_up_moment() {
            lines.push(Line::from(vec![
                key_label("Ascension"),
                Span::styled(format!("{from_rank} -> {to_rank}"), pulse_highlight_text()),
            ]));
        }
        lines.extend([
            Line::from(vec![
                key_label("Stacker Status"),
                Span::styled(progress.current.name, emphasis_text()),
            ]),
            Line::from(vec![
                key_label("Lifetime earned"),
                Span::styled(
                    format_sats(self.operator_stats.total_earnings_sats),
                    emphasis_text(),
                ),
            ]),
        ]);

        if let Some(next) = progress.next {
            let sats_remaining = next
                .threshold_sats
                .saturating_sub(self.operator_stats.total_earnings_sats);
            let mut progress_line = vec![
                Span::styled(progress.current.name, muted_text()),
                Span::raw("  "),
            ];
            progress_line.extend(render_rank_progress_spans(
                progress.progress_ratio,
                16,
                animation_tick,
                rank_up_visible,
            ));
            progress_line.extend([
                Span::raw("  "),
                Span::styled(next.name, muted_text()),
            ]);
            lines.push(Line::from(vec![
                key_label("Next unlock"),
                Span::raw(format!("{} in {}", next.name, format_sats(sats_remaining))),
            ]));
            lines.push(Line::from(progress_line));
        } else {
            lines.push(Line::from(vec![
                key_label("Final rank"),
                Span::raw("1 bitcoin stacked"),
            ]));
            let mut progress_line = vec![
                Span::styled(progress.current.name, muted_text()),
                Span::raw("  "),
            ];
            progress_line.extend(render_rank_progress_spans(
                1.0,
                16,
                animation_tick,
                rank_up_visible,
            ));
            progress_line.extend([
                Span::raw("  "),
                Span::styled(progress.current.name, muted_text()),
            ]);
            lines.push(Line::from(progress_line));
            lines.push(Line::from(vec![
                key_label("Crown"),
                Span::raw("The full stack is online"),
            ]));
        }

        lines
    }

    #[allow(dead_code)]
    fn activity_lines(&self) -> Vec<Line<'static>> {
        let animation_phase = self.animation_phase();
        let mut lines = Vec::new();
        if let Some((amount_sats, _)) = self.visible_paid_moment() {
            lines.push(Line::from(Span::styled(
                format!("[PAID] you just stacked {}", format_sats(amount_sats)),
                pulse_highlight_text(),
            )));
        }
        if let Some((from_rank, to_rank, _)) = self.visible_rank_up_moment() {
            lines.push(Line::from(Span::styled(
                format!("[RANK] ascended from {from_rank} to {to_rank}"),
                pulse_highlight_text(),
            )));
        }
        if !self.operator_stats.recent_activity.is_empty() {
            lines.extend(self.operator_stats.recent_activity.iter().enumerate().map(
                |(index, entry)| {
                    let style = if index == 0 && self.activity_pulse_is_visible() {
                        pulse_highlight_text()
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(entry.clone(), style))
                },
            ));
            return lines;
        }

        if let Some(lines) = self.startup_activity_lines() {
            return lines;
        }

        if let Some(command) = self.provider_command_in_flight.as_ref() {
            return vec![
                Line::from(format!(
                    "[LIVE] listening across relays {}",
                    animated_ready_heartbeat(animation_phase)
                )),
                Line::from(format!("[LIVE] {}", command.detail())),
                Line::from("[TIP] Keep Pylon open so matching jobs can land.".to_string()),
            ];
        }

        let quiet_detail = match self.operator_stats.last_provider_event_at_ms {
            Some(at_ms) => format!(
                "No new paid matches in {}.",
                format_elapsed_since_ms(at_ms, current_epoch_ms_u64())
            ),
            None => "No paid matches yet.".to_string(),
        };

        vec![
            Line::from(format!(
                "[READY] standing by for paid work {}",
                animated_ready_heartbeat(animation_phase)
            )),
            Line::from(format!("[MARKET] {quiet_detail}{}", animated_quiet_suffix(animation_phase))),
            Line::from("[TIP] Keep Pylon open so starter jobs can land.".to_string()),
        ]
    }

    fn operator_state_label_and_detail(&self) -> (String, Option<String>) {
        if self.operator_stats.processing_jobs > 0 {
            let count = format_u64_with_commas(self.operator_stats.processing_jobs);
            let job_label = if self.operator_stats.processing_jobs == 1 {
                "job"
            } else {
                "jobs"
            };
            return (
                "Earning now".to_string(),
                Some(format!("{count} {job_label} in progress on this node")),
            );
        }
        if self.operator_stats.awaiting_payment_jobs > 0 {
            let count = format_u64_with_commas(self.operator_stats.awaiting_payment_jobs);
            let job_label = if self.operator_stats.awaiting_payment_jobs == 1 {
                "job is"
            } else {
                "jobs are"
            };
            return (
                "Waiting for payout".to_string(),
                Some(format!("{count} completed {job_label} waiting for settlement")),
            );
        }
        if let Some(detail) = self.startup_detail() {
            return ("Preparing to earn".to_string(), Some(detail));
        }
        if let Some(command) = self.provider_command_in_flight.as_ref() {
            return (command.state_label().to_string(), Some(command.detail()));
        }
        match self.operator_stats.runtime_status.as_deref() {
            Some("online") if self.operator_stats.provider_presence_online => (
                "Ready to earn".to_string(),
                Some("Online for paid jobs.".to_string()),
            ),
            Some("online") => (
                "Ready to earn".to_string(),
                Some("Connecting provider presence.".to_string()),
            ),
            Some("ready") => (
                "Ready to earn".to_string(),
                Some("Online setup is ready.".to_string()),
            ),
            Some("paused") => ("Paused".to_string(), None),
            Some("degraded") => (
                "Needs attention".to_string(),
                self.operator_stats.runtime_error.clone(),
            ),
            Some("error") => (
                "Needs attention".to_string(),
                self.operator_stats.runtime_error.clone(),
            ),
            Some("unconfigured") => (
                "Needs attention".to_string(),
                self.operator_stats.runtime_error.clone(),
            ),
            Some(other) => (other.to_string(), self.operator_stats.runtime_error.clone()),
            None => (
                self.operator_stats.desired_mode.label().to_string(),
                self.operator_stats.runtime_error.clone(),
            ),
        }
    }

    fn startup_status_is_loading(&self) -> bool {
        self.last_refresh_at.is_none() && (self.refresh_in_flight || self.loaded.is_none())
    }

    fn startup_detail(&self) -> Option<String> {
        if self.last_refresh_at.is_none() && self.operator_stats.runtime_status.is_none() {
            return Some("Starting local checks for Gemma, wallet, and node status.".to_string());
        }
        if self.refresh_in_flight
            && self.loaded.is_none()
            && self.operator_stats.runtime_status.is_none()
        {
            return Some("Loading node status and checking local Gemma supply.".to_string());
        }
        if self.refresh_in_flight && self.operator_stats.runtime_status.is_none() {
            return Some("Refreshing runtime status and wallet state.".to_string());
        }
        None
    }

    #[allow(dead_code)]
    fn startup_activity_lines(&self) -> Option<Vec<Line<'static>>> {
        let spinner = animated_boot_spinner(self.animation_phase());
        if self.last_refresh_at.is_none() && self.operator_stats.runtime_status.is_none() {
            return Some(vec![
                Line::from(format!("[LIVE] {spinner} checking local Pylon setup")),
                Line::from(format!("[LIVE] {spinner} looking for local Gemma availability")),
                Line::from("[WAIT] Opening the shell and preparing earnings state.".to_string()),
            ]);
        }
        if self.refresh_in_flight
            && self.loaded.is_none()
            && self.operator_stats.runtime_status.is_none()
        {
            return Some(vec![
                Line::from(format!("[LIVE] {spinner} loading node status")),
                Line::from(format!("[LIVE] {spinner} checking wallet and runtime state")),
                Line::from(
                    "[WAIT] This usually clears as soon as the first refresh finishes."
                        .to_string(),
                ),
            ]);
        }
        if self.refresh_in_flight && self.operator_stats.runtime_status.is_none() {
            return Some(vec![
                Line::from(format!("[LIVE] {spinner} refreshing provider readiness")),
                Line::from(format!("[LIVE] {spinner} confirming local Gemma connectivity")),
                Line::from("[WAIT] Still preparing this node to earn.".to_string()),
            ]);
        }
        None
    }

    fn transcript_body(&self) -> Text<'static> {
        if self.transcript.is_empty() {
            Text::from(vec![
                Line::from(vec![
                    Span::styled("[system]", muted_text()),
                    Span::raw(" "),
                    Span::styled("Shell Ready", shell_accent()),
                ]),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Ask Gemma or type /help.", emphasis_text()),
                ]),
            ])
        } else {
            self.transcript.as_text_with_motion(self.animation_tick())
        }
    }

    fn transcript_panel_title(&self) -> String {
        if self.transcript_follow_latest || self.transcript_scroll_y >= self.transcript_max_scroll_y
        {
            return String::from("Transcript");
        }
        let hidden_rows = self
            .transcript_max_scroll_y
            .saturating_sub(self.transcript_scroll_y);
        format!("Transcript ^ {hidden_rows} rows above")
    }

    fn sync_transcript_scroll_after_update(&mut self) {
        self.refresh_transcript_scroll_metrics();
        if self.transcript_follow_latest {
            self.transcript_scroll_y = self.transcript_max_scroll_y;
        } else {
            self.transcript_scroll_y = self.transcript_scroll_y.min(self.transcript_max_scroll_y);
        }
    }

    fn scroll_transcript_up(&mut self, amount: u16) {
        self.transcript_follow_latest = false;
        self.transcript_scroll_y = self.transcript_scroll_y.saturating_sub(amount);
    }

    fn scroll_transcript_down(&mut self, amount: u16) {
        self.transcript_scroll_y = self
            .transcript_scroll_y
            .saturating_add(amount)
            .min(self.transcript_max_scroll_y);
        self.transcript_follow_latest = self.transcript_scroll_y >= self.transcript_max_scroll_y;
    }

    fn update_transcript_layout(&mut self, area: Rect) {
        self.transcript_wrap_width = transcript_wrap_width(area);
        self.transcript_viewport_height = transcript_viewport_height(area);
        self.refresh_transcript_scroll_metrics();
        if self.transcript_follow_latest {
            self.transcript_scroll_y = self.transcript_max_scroll_y;
        } else {
            self.transcript_scroll_y = self.transcript_scroll_y.min(self.transcript_max_scroll_y);
        }
    }

    fn refresh_transcript_scroll_metrics(&mut self) {
        if self.transcript_wrap_width == 0 || self.transcript_viewport_height == 0 {
            self.transcript_max_scroll_y = 0;
            return;
        }
        self.transcript_max_scroll_y = max_transcript_scroll_y(
            self.rendered_transcript_row_count(self.transcript_wrap_width),
            self.transcript_viewport_height,
        );
    }

    fn rendered_transcript_row_count(&self, wrap_width: u16) -> usize {
        self.transcript_body()
            .lines
            .iter()
            .map(ToString::to_string)
            .map(|line| wrapped_row_count(line.as_str(), wrap_width))
            .sum()
    }

    fn animation_phase(&self) -> usize {
        self.animation_tick() % 4
    }

    fn animation_tick(&self) -> usize {
        (Instant::now()
            .duration_since(self.animation_started_at)
            .as_millis()
            / 180) as usize
    }

    #[allow(dead_code)]
    fn activity_pulse_is_visible(&self) -> bool {
        let Some(until) = self.live_activity_pulse_until else {
            return false;
        };
        if Instant::now() >= until {
            return false;
        }
        self.animation_phase() % 2 == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct StackerRankProgress {
    current: StackerRank,
    next: Option<StackerRank>,
    progress_sats: u64,
    segment_goal_sats: u64,
    progress_ratio: f64,
}

fn stacker_rank_progress(lifetime_sats: u64) -> StackerRankProgress {
    let mut current = STACKER_RANKS[0];
    let mut next = None;
    for (index, rank) in STACKER_RANKS.iter().enumerate() {
        if lifetime_sats >= rank.threshold_sats {
            current = *rank;
            next = STACKER_RANKS.get(index + 1).copied();
        } else {
            break;
        }
    }

    match next {
        Some(next_rank) => {
            let progress_sats = lifetime_sats.saturating_sub(current.threshold_sats);
            let segment_goal_sats = next_rank.threshold_sats.saturating_sub(current.threshold_sats);
            let progress_ratio = if segment_goal_sats == 0 {
                1.0
            } else {
                (progress_sats as f64 / segment_goal_sats as f64).clamp(0.0, 1.0)
            };
            StackerRankProgress {
                current,
                next: Some(next_rank),
                progress_sats,
                segment_goal_sats,
                progress_ratio,
            }
        }
        None => StackerRankProgress {
            current,
            next: None,
            progress_sats: lifetime_sats.saturating_sub(current.threshold_sats),
            segment_goal_sats: 0,
            progress_ratio: 1.0,
        },
    }
}

fn render_rank_progress_spans(
    progress_ratio: f64,
    width: usize,
    tick: usize,
    celebratory: bool,
) -> Vec<Span<'static>> {
    let width = width.max(5);
    let ratio = progress_ratio.clamp(0.0, 1.0);
    let marker_index = ((width.saturating_sub(1)) as f64 * ratio).round() as usize;
    let glint_index = if celebratory {
        let wave = [marker_index.saturating_sub(2), marker_index.saturating_sub(1), marker_index, (marker_index + 1).min(width - 1), (marker_index + 2).min(width - 1)];
        wave[tick % wave.len()]
    } else {
        tick % width
    };
    let mut out = Vec::with_capacity(width);
    for index in 0..width {
        let span = if index == marker_index {
            let marker = if celebratory && tick % 2 == 0 { "◎" } else { "◉" };
            Span::styled(
                marker,
                if celebratory && tick % 2 == 0 {
                    pulse_highlight_text()
                } else {
                    emphasis_text()
                },
            )
        } else if index == glint_index {
            Span::styled(
                if index < marker_index { "─" } else { "╌" },
                if celebratory {
                    pulse_highlight_text()
                } else {
                    shell_accent()
                },
            )
        } else if index < marker_index {
            Span::styled("─", muted_text())
        } else {
            Span::styled("╌", shell_border())
        };
        out.push(span);
    }
    out
}

fn abbreviate_wallet_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= 26 {
        return trimmed.to_string();
    }
    let prefix = trimmed.chars().take(14).collect::<String>();
    let suffix = trimmed
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn title_case_status(value: &str) -> String {
    value
        .split(['_', ' '])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn animated_right_now_suffix(
    processing_jobs: u64,
    awaiting_payment_jobs: u64,
    phase: usize,
) -> &'static str {
    if processing_jobs > 0 {
        match phase % 4 {
            0 => "  •",
            1 => "  ••",
            2 => "  •••",
            _ => "  ◉",
        }
    } else if awaiting_payment_jobs > 0 {
        match phase % 4 {
            0 => "  ·",
            1 => "  ··",
            2 => "  ···",
            _ => "  ◌",
        }
    } else {
        ""
    }
}

fn animated_stack_gain_suffix(phase: usize, active: bool) -> &'static str {
    if !active {
        return "";
    }
    match phase % 4 {
        0 => "  ▁",
        1 => "  ▃",
        2 => "  ▆",
        _ => "  █",
    }
}

fn mission_control_signal_spans(
    state_label: &str,
    tick: usize,
) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw("  ")];
    match state_label {
        "Ready to earn" => {
            spans.push(Span::styled(animated_ready_heartbeat(tick), muted_text()));
        }
        "Listening for work" => {
            for (index, glyph) in ["·", "·", "·", "·", "·"].into_iter().enumerate() {
                let style = if index == tick % 5 {
                    shell_accent()
                } else {
                    muted_text()
                };
                spans.push(Span::styled(glyph, style));
            }
        }
        "Earning now" => {
            spans.push(Span::styled(animated_active_work_pulse(tick), success_accent()));
        }
        "Waiting for payout" => {
            spans.push(Span::styled(animated_payout_pulse(tick), warning_accent()));
        }
        "Preparing to earn" => {
            spans.push(Span::styled(animated_boot_spinner(tick), muted_text()));
        }
        _ => {}
    }
    spans
}

fn animated_active_work_pulse(tick: usize) -> &'static str {
    match tick % 4 {
        0 => "•",
        1 => "••",
        2 => "•••",
        _ => "◉",
    }
}

fn animated_payout_pulse(tick: usize) -> &'static str {
    match tick % 4 {
        0 => "·",
        1 => "··",
        2 => "···",
        _ => "◌",
    }
}

fn animated_boot_spinner(phase: usize) -> &'static str {
    match phase % 4 {
        0 => "◐",
        1 => "◓",
        2 => "◑",
        _ => "◒",
    }
}

#[allow(dead_code)]
fn animated_ready_heartbeat(phase: usize) -> &'static str {
    match phase % 4 {
        0 => "•",
        1 => "◦",
        2 => "•",
        _ => "◦",
    }
}

#[allow(dead_code)]
fn animated_quiet_suffix(phase: usize) -> &'static str {
    match phase % 4 {
        0 => " .",
        1 => " ..",
        2 => " ...",
        _ => "",
    }
}

fn animated_boot_suffix(phase: usize) -> &'static str {
    match phase % 4 {
        0 => " ◐",
        1 => " ◓",
        2 => " ◑",
        _ => " ◒",
    }
}

fn parse_tui_buyer_job_submit_request(args: &str) -> Result<pylon::BuyerJobSubmitRequest> {
    let mut remainder = args.trim();
    let mut bid_msats = None::<u64>;
    let mut model = None::<String>;
    let mut provider_pubkey = None::<String>;
    let mut output_mime = None::<String>;
    let mut request_json = None::<String>;

    while remainder.starts_with("--") {
        if let Some(value) = remainder.strip_prefix("--bid-msats ") {
            let (raw, tail) = take_next_tui_word(value);
            bid_msats = Some(
                raw.parse::<u64>()
                    .map_err(|_| anyhow!("invalid buyer bid millisats `{raw}`"))?,
            );
            remainder = tail;
            continue;
        }
        if let Some(value) = remainder.strip_prefix("--model ") {
            let (raw, tail) = take_next_tui_word(value);
            model = Some(raw.to_string());
            remainder = tail;
            continue;
        }
        if let Some(value) = remainder.strip_prefix("--provider ") {
            let (raw, tail) = take_next_tui_word(value);
            provider_pubkey = Some(raw.to_string());
            remainder = tail;
            continue;
        }
        if let Some(value) = remainder.strip_prefix("--output ") {
            let (raw, tail) = take_next_tui_word(value);
            output_mime = Some(raw.to_string());
            remainder = tail;
            continue;
        }
        if let Some(value) = remainder.strip_prefix("--request-json ") {
            request_json = Some(value.trim().to_string());
            remainder = "";
            break;
        }
        break;
    }

    let prompt = (!remainder.trim().is_empty()).then(|| remainder.trim().to_string());
    if prompt.is_none() && request_json.is_none() {
        bail!("job submit requires prompt text or --request-json");
    }
    if prompt.is_some() && request_json.is_some() {
        bail!("job submit accepts either prompt text or --request-json, not both");
    }

    Ok(pylon::BuyerJobSubmitRequest {
        prompt,
        request_json,
        bid_msats,
        model,
        provider_pubkey,
        output_mime,
    })
}

fn parse_tui_buyer_job_watch_request(args: &str) -> Result<(Option<String>, u64)> {
    let mut remainder = args.trim();
    let mut request_event_id = None::<String>;
    let mut seconds = 30u64;

    if !remainder.is_empty() && !remainder.starts_with("--") {
        let (raw, tail) = take_next_tui_word(remainder);
        request_event_id = Some(raw.to_string());
        remainder = tail;
    }

    while !remainder.is_empty() {
        if let Some(value) = remainder.strip_prefix("--seconds ") {
            let (raw, tail) = take_next_tui_word(value);
            seconds = raw
                .parse::<u64>()
                .map_err(|_| anyhow!("invalid buyer watch seconds `{raw}`"))?;
            remainder = tail;
            continue;
        }
        bail!("unexpected buyer job watch argument `{remainder}`");
    }

    Ok((request_event_id, seconds.max(1)))
}

fn parse_tui_buyer_job_history_request(args: &str) -> Result<Option<usize>> {
    let mut remainder = args.trim();
    let mut limit = None;
    while !remainder.is_empty() {
        if let Some(value) = remainder.strip_prefix("--limit ") {
            let (raw, tail) = take_next_tui_word(value);
            limit = Some(
                raw.parse::<usize>()
                    .map_err(|_| anyhow!("invalid buyer history limit `{raw}`"))?,
            );
            remainder = tail;
            continue;
        }
        bail!("unexpected buyer job history argument `{remainder}`");
    }
    Ok(limit)
}

fn parse_tui_buyer_job_request_id(args: &str, command: &str) -> Result<String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        bail!("{command} requires <request_event_id>");
    }
    let (request_event_id, remainder) = take_next_tui_word(trimmed);
    if !remainder.is_empty() {
        bail!("unexpected {command} argument `{remainder}`");
    }
    Ok(request_event_id.to_string())
}

fn parse_tui_buyer_job_policy_mode(args: &str) -> Result<pylon::BuyerPaymentPolicyMode> {
    match args.trim() {
        "" | "show" => Ok(pylon::BuyerPaymentPolicyMode::Show),
        "auto" => Ok(pylon::BuyerPaymentPolicyMode::Auto),
        "manual" => Ok(pylon::BuyerPaymentPolicyMode::Manual),
        other => bail!("unknown buyer payment policy mode `{other}`"),
    }
}

fn parse_tui_optional_limit(args: &str, command: &str) -> Result<Option<usize>> {
    let mut remainder = args.trim();
    if let Some(tail) = remainder.strip_prefix("show") {
        if !tail.is_empty() && !tail.starts_with(char::is_whitespace) {
            bail!("unexpected {command} argument `{remainder}`");
        }
        remainder = tail.trim_start();
    }
    if remainder == "show" {
        remainder = "";
    }
    let mut limit = None;
    while !remainder.is_empty() {
        if let Some(value) = remainder.strip_prefix("--limit ") {
            let (raw, tail) = take_next_tui_word(value);
            limit = Some(
                raw.parse::<usize>()
                    .map_err(|_| anyhow!("invalid {command} limit `{raw}`"))?,
            );
            remainder = tail;
            continue;
        }
        bail!("unexpected {command} argument `{remainder}`");
    }
    Ok(limit)
}

fn parse_tui_payout_history_request(args: &str) -> Result<Option<u32>> {
    let mut remainder = args.trim();
    let mut limit = None;
    while !remainder.is_empty() {
        if let Some(value) = remainder.strip_prefix("--limit ") {
            let (raw, tail) = take_next_tui_word(value);
            limit = Some(
                raw.parse::<u32>()
                    .map_err(|_| anyhow!("invalid payout history limit `{raw}`"))?,
            );
            remainder = tail;
            continue;
        }
        bail!("unexpected payout history argument `{remainder}`");
    }
    Ok(limit)
}

fn parse_tui_payout_withdraw_request(args: &str) -> Result<(String, Option<u64>)> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        bail!("payout withdraw requires <payment_request>");
    }
    let (payment_request, mut remainder) = take_next_tui_word(trimmed);
    let mut amount_sats = None;
    while !remainder.is_empty() {
        if let Some(value) = remainder.strip_prefix("--amount-sats ") {
            let (raw, tail) = take_next_tui_word(value);
            amount_sats = Some(
                raw.parse::<u64>()
                    .map_err(|_| anyhow!("invalid payout withdraw amount `{raw}`"))?,
            );
            remainder = tail;
            continue;
        }
        bail!("unexpected payout withdraw argument `{remainder}`");
    }
    Ok((payment_request.to_string(), amount_sats))
}

fn take_next_tui_word(value: &str) -> (&str, &str) {
    let trimmed = value.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(index) => (&trimmed[..index], trimmed[index..].trim_start()),
        None => (trimmed, ""),
    }
}

pub fn usage() -> &'static str {
    "Usage: pylon-tui [--config-path <path>]\n\
Controls:\n\
  Ctrl+C   quit\n\
  Tab      toggle between operate and inspect sidebars\n\
  PgUp/PgDn / wheel  scroll transcript\n\
  Enter    submit composer\n\
  Ctrl+J   insert newline\n\
  Composer:\n\
  [prompt]  stream a reply from local Gemma when weights are loaded\n\
  /help  show available commands\n\
  /model [model]  target a Gemma model for local runtime use\n\
  /uninstall [model]  remove a Gemma model from local cache and runtime\n\
  /provider [scan|run] [--seconds <n>]  inspect or process retained inbound NIP-90 jobs\n\
  /jobs [--limit <n>]  show retained provider job history\n\
  /earnings  show retained provider earnings\n\
  /receipts [--limit <n>]  show retained provider receipts\n\
  /activity [--limit <n>]  show retained relay and settlement activity\n\
  /job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] <prompt>  publish a retained NIP-90 buyer request\n\
  /job watch [<request_event_id>] [--seconds <n>]  stream retained buyer feedback and results into the transcript\n\
  /job history [--limit <n>]  show retained buyer job history from the local ledger\n\
  /job replay <request_event_id>  replay one retained buyer job from local state\n\
  /job approve <request_event_id>  pay one retained buyer invoice\n\
  /job deny <request_event_id>  deny one retained buyer invoice locally\n\
  /job policy [show|auto|manual]  inspect or set buyer auto-pay policy\n\
  /payout [history|withdraw] ...  inspect provider earnings and run withdrawals\n\
  /relay [list|add|remove|refresh]  inspect or update configured relays\n\
  /wallet [show|receive|withdraw|recovery|status|balance|address|invoice|pay|history]  open the wallet surface or run retained Spark wallet commands\n\
  /download [model]  download a Gemma GGUF from Hugging Face into the local Pylon cache\n"
}

fn should_publish_provider_presence(
    desired_mode: ProviderDesiredMode,
    snapshot: Option<&ProviderPersistedSnapshot>,
) -> bool {
    desired_mode == ProviderDesiredMode::Online && snapshot.is_some()
}

pub async fn run_pylon_tui() -> Result<()> {
    run_pylon_tui_with_config(TuiLaunchConfig {
        config_path: pylon::default_config_path(),
    })
    .await
}

pub async fn run_pylon_tui_with_args(args: Vec<String>) -> Result<()> {
    let config = TuiLaunchConfig::from_args(args)?;
    run_pylon_tui_with_config(config).await
}

async fn run_pylon_tui_with_config(config: TuiLaunchConfig) -> Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = AppShell::new(config.config_path);
    let result = run_loop(&mut terminal, &mut app).await;
    app.report_provider_presence_offline().await;
    let cleanup_result = restore_terminal(&mut terminal);

    result.and(cleanup_result)
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut AppShell,
) -> Result<()> {
    app.refresh().await;

    while !app.should_quit() {
        app.drain_worker_events();
        terminal.draw(|frame| app.render(frame))?;
        if event::poll(TICK_RATE)? {
            match event::read()? {
                CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                CrosstermEvent::Mouse(mouse) => app.handle_mouse(mouse),
                CrosstermEvent::Resize(_, _) => app.schedule_refresh_now(),
                _ => {}
            }
        }
        if app.should_refresh() {
            app.refresh().await;
        }
        app.drain_worker_events();
    }

    Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn text_body_lines(value: &str) -> Vec<String> {
    let lines = value.lines().map(ToString::to_string).collect::<Vec<_>>();
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn buyer_job_entry_title(entry: &pylon::BuyerJobWatchEntry) -> String {
    if entry.event_kind == "result" {
        format!("Buyer Result {}", entry.request_event_id)
    } else {
        format!("Buyer Feedback {} {}", entry.request_event_id, entry.status)
    }
}

fn buyer_job_entry_lines(entry: &pylon::BuyerJobWatchEntry) -> Vec<String> {
    let mut lines = vec![
        format!("relay: {}", entry.relay_url.as_deref().unwrap_or("unknown")),
        format!("event_id: {}", entry.event_id),
    ];
    if let Some(provider_pubkey) = entry.provider_pubkey.as_deref() {
        lines.push(format!("provider: {provider_pubkey}"));
    }
    if let Some(amount_msats) = entry.amount_msats {
        lines.push(format!("amount_msats: {amount_msats}"));
    }
    if let Some(bolt11) = entry.bolt11.as_deref() {
        lines.push(format!("bolt11: {bolt11}"));
    }
    if let Some(result_preview) = entry.result_preview.as_deref() {
        lines.push(result_preview.to_string());
    }
    if let Some(detail) = entry.detail.as_deref() {
        lines.push(format!("detail: {detail}"));
    }
    lines
}

fn gemma4_status(loaded: Option<&LoadedState>) -> Gemma4Status {
    let Some(loaded) = loaded else {
        return Gemma4Status {
            note: "Preparing local Pylon state.".to_string(),
            ..Gemma4Status::default()
        };
    };

    let Some(snapshot) = loaded.snapshot.as_ref() else {
        return Gemma4Status {
            note: "No Gemma 4 weights are visible right now.".to_string(),
            ..Gemma4Status::default()
        };
    };

    let mut models = Vec::new();
    collect_gemma4_backend_models(&snapshot.availability.local_gemma, &mut models);
    collect_gemma4_backend_models(&snapshot.availability.apple_foundation_models, &mut models);

    if let Some(default_model) = snapshot
        .availability
        .pooled_inference
        .default_model
        .as_deref()
    {
        if is_gemma4_model(default_model) {
            models.push(default_model.to_string());
        }
    }

    for target in &snapshot.availability.pooled_inference.targetable_models {
        if is_gemma4_model(target.model.as_str()) || is_gemma4_model(target.family.as_str()) {
            models.push(target.model.clone());
        }
    }

    sort_and_dedup(&mut models);
    let loaded_flag = !models.is_empty();
    let note = if loaded_flag {
        "Gemma 4 weights are visible to this node right now.".to_string()
    } else {
        "No Gemma 4 weights are visible right now.".to_string()
    };

    Gemma4Status {
        loaded: loaded_flag,
        models,
        note,
    }
}

fn compute_operator_panel_stats(
    desired_mode: ProviderDesiredMode,
    provider_presence_online: bool,
    wallet_status: Option<&pylon::WalletStatusReport>,
    snapshot: Option<&ProviderPersistedSnapshot>,
    ledger: &pylon::PylonLedger,
    session_started_at_ms: u64,
) -> OperatorPanelStats {
    compute_operator_panel_stats_at(
        desired_mode,
        provider_presence_online,
        wallet_status,
        snapshot,
        ledger,
        session_started_at_ms,
        current_epoch_ms_u64(),
    )
}

fn build_wallet_surface(
    config: &pylon::PylonConfig,
    wallet_status: Option<&pylon::WalletStatusReport>,
    ledger: &pylon::PylonLedger,
) -> WalletSurfaceState {
    WalletSurfaceState {
        runtime_status: wallet_status
            .map(|report| report.runtime_status.clone())
            .or_else(|| ledger.wallet.runtime_status.clone()),
        runtime_detail: wallet_status
            .and_then(|report| report.runtime_detail.clone())
            .or_else(|| ledger.wallet.last_error.clone()),
        network: wallet_status
            .map(|report| report.runtime.network.clone())
            .or_else(|| ledger.wallet.network.clone()),
        balance: wallet_status
            .map(|report| report.balance.clone())
            .or_else(|| {
                ledger
                    .wallet
                    .last_balance_sats
                    .map(|total_sats| pylon::WalletBalanceSnapshot {
                        total_sats,
                        ..pylon::WalletBalanceSnapshot::default()
                    })
            }),
        balance_live: wallet_status.is_some(),
        spark_address: ledger.wallet.spark_address.clone(),
        bitcoin_address: ledger.wallet.bitcoin_address.clone(),
        latest_invoice: ledger.wallet.invoices.first().cloned(),
        recent_payments: if let Some(report) = wallet_status {
            report.recent_payments.clone()
        } else {
            ledger.wallet.payments.iter().take(5).cloned().collect()
        },
        identity_path: Some(config.identity_path.clone()),
        last_error: ledger.wallet.last_error.clone(),
    }
}

fn compute_operator_panel_stats_at(
    desired_mode: ProviderDesiredMode,
    provider_presence_online: bool,
    wallet_status: Option<&pylon::WalletStatusReport>,
    snapshot: Option<&ProviderPersistedSnapshot>,
    ledger: &pylon::PylonLedger,
    session_started_at_ms: u64,
    now_ms: u64,
) -> OperatorPanelStats {
    let since_ms = now_ms.saturating_sub(LOOKBACK_WINDOW_24H_MS);
    let wallet_balance_live = wallet_status.is_some_and(wallet_status_balance_is_authoritative);
    let wallet_runtime_status = wallet_status
        .map(|report| report.runtime_status.clone())
        .or_else(|| ledger.wallet.runtime_status.clone());
    let wallet_balance = wallet_status
        .filter(|report| wallet_status_balance_is_authoritative(report))
        .map(|report| report.balance.clone())
        .or_else(|| {
            ledger
                .wallet
                .last_balance_sats
                .map(|total_sats| pylon::WalletBalanceSnapshot {
                    total_sats,
                    ..pylon::WalletBalanceSnapshot::default()
                })
        });
    let provider_jobs = ledger.jobs.iter().filter(|job| job.direction == "provider");

    let jobs_found_24h = provider_jobs
        .clone()
        .filter(|job| job.created_at_ms >= since_ms)
        .count() as u64;
    let matching_jobs_24h = provider_jobs
        .clone()
        .filter(|job| job.created_at_ms >= since_ms)
        .filter(|job| provider_job_counted_as_matching(job.status.as_str()))
        .count() as u64;
    let jobs_processed_24h = provider_jobs
        .clone()
        .filter(|job| job.updated_at_ms >= since_ms)
        .filter(|job| provider_job_counted_as_processed(job.status.as_str()))
        .count() as u64;
    let awaiting_payment_jobs = provider_jobs
        .clone()
        .filter(|job| job.status == "payment_required")
        .count() as u64;
    let processing_jobs = provider_jobs
        .clone()
        .filter(|job| provider_job_is_in_progress(job.status.as_str()))
        .count() as u64;
    let jobs_settled_24h = ledger
        .settlements
        .iter()
        .filter(|settlement| settlement.direction == "provider")
        .filter(|settlement| provider_settlement_counted_as_settled(settlement.status.as_str()))
        .filter(|settlement| settlement.created_at_ms >= since_ms)
        .map(|settlement| settlement.job_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u64;
    let session_earnings_sats = ledger
        .wallet
        .payments
        .iter()
        .filter(|payment| payment.direction.eq_ignore_ascii_case("receive"))
        .filter(|payment| {
            payment.status.eq_ignore_ascii_case("completed")
                || payment.status.eq_ignore_ascii_case("settled")
        })
        .filter(|payment| payment.updated_at_ms >= session_started_at_ms)
        .map(|payment| payment.amount_sats)
        .sum::<u64>();
    let settled_sats_24h = ledger
        .settlements
        .iter()
        .filter(|settlement| settlement.direction == "provider")
        .filter(|settlement| provider_settlement_counted_as_settled(settlement.status.as_str()))
        .filter(|settlement| settlement.created_at_ms >= since_ms)
        .map(|settlement| settlement.amount_msats / 1_000)
        .sum::<u64>();
    let settled_sats_lifetime = ledger
        .settlements
        .iter()
        .filter(|settlement| settlement.direction == "provider")
        .filter(|settlement| provider_settlement_counted_as_settled(settlement.status.as_str()))
        .map(|settlement| settlement.amount_msats / 1_000)
        .sum::<u64>();
    let snapshot_lifetime_earnings_sats = snapshot
        .and_then(|value| value.earnings.as_ref())
        .map(|earnings| earnings.lifetime_sats)
        .unwrap_or(0);
    let total_earnings_sats = snapshot_lifetime_earnings_sats.max(settled_sats_lifetime);
    let last_job_result = snapshot
        .and_then(|value| value.earnings.as_ref())
        .map(|earnings| earnings.last_job_result.as_str())
        .filter(|status| !status.trim().is_empty() && *status != "none")
        .map(str::to_string)
        .or_else(|| latest_provider_job_status(ledger));
    let last_provider_event_at_ms = ledger
        .jobs
        .iter()
        .filter(|job| job.direction == "provider")
        .map(|job| job.updated_at_ms)
        .max()
        .or_else(|| {
            ledger
                .settlements
                .iter()
                .filter(|settlement| settlement.direction == "provider")
                .map(|settlement| settlement.updated_at_ms)
                .max()
        });
    let recent_activity = recent_provider_activity(ledger, now_ms);

    OperatorPanelStats {
        desired_mode,
        runtime_status: snapshot.and_then(|value| value.runtime.authoritative_status.clone()),
        runtime_error: snapshot.and_then(|value| value.runtime.last_error.clone()),
        backend_label: snapshot
            .map(|value| value.runtime.execution_backend_label.clone())
            .filter(|value| !value.trim().is_empty()),
        provider_presence_online,
        wallet_runtime_status,
        wallet_balance,
        wallet_balance_live,
        jobs_found_24h,
        matching_jobs_24h,
        jobs_processed_24h,
        jobs_settled_24h,
        session_earnings_sats,
        settled_sats_24h,
        settled_sats_lifetime,
        total_earnings_sats,
        awaiting_payment_jobs,
        processing_jobs,
        last_job_result,
        last_provider_event_at_ms,
        recent_activity,
        online_uptime_seconds: snapshot.map(|value| value.runtime.online_uptime_seconds),
    }
}

fn stabilize_operator_panel_stats(
    previous: OperatorPanelStats,
    mut current: OperatorPanelStats,
) -> OperatorPanelStats {
    if current.runtime_status.is_none() {
        current.runtime_status = previous.runtime_status;
        if current.runtime_error.is_none() {
            current.runtime_error = previous.runtime_error;
        }
    }

    let wallet_connected = matches!(current.wallet_runtime_status.as_deref(), Some("connected"));
    if !wallet_connected && previous.wallet_balance.is_some() {
        current.wallet_balance = previous.wallet_balance;
        current.wallet_balance_live = false;
        if current.wallet_runtime_status.is_none() {
            current.wallet_runtime_status = previous.wallet_runtime_status;
        }
    }

    current
}

async fn render_wallet_command_output(
    config_path: &Path,
    command: &pylon::WalletSubcommand,
) -> Result<String> {
    match command {
        pylon::WalletSubcommand::Status { .. } => {
            let report = pylon::load_wallet_status_report(config_path).await?;
            Ok(render_wallet_status_output(&report))
        }
        pylon::WalletSubcommand::Balance { .. } => {
            let report = pylon::load_wallet_status_report(config_path).await?;
            Ok(render_wallet_status_output(&report))
        }
        pylon::WalletSubcommand::Address { .. } => {
            let report = pylon::create_wallet_address_report(config_path).await?;
            Ok(render_wallet_receive_output(&report))
        }
        pylon::WalletSubcommand::Invoice {
            amount_sats,
            description,
            expiry_seconds,
            ..
        } => {
            let report = pylon::create_wallet_invoice_report(
                config_path,
                *amount_sats,
                description.clone(),
                *expiry_seconds,
            )
            .await?;
            Ok(render_wallet_invoice_output(&report))
        }
        pylon::WalletSubcommand::Pay {
            payment_request,
            amount_sats,
            ..
        } => {
            let report =
                pylon::pay_wallet_invoice_report(config_path, payment_request.as_str(), *amount_sats)
                    .await?;
            Ok(render_wallet_pay_output(&report))
        }
        pylon::WalletSubcommand::History { limit, .. } => {
            let report = pylon::load_wallet_history_report(config_path, *limit).await?;
            Ok(render_wallet_history_output(&report))
        }
    }
}

fn render_wallet_status_output(report: &pylon::WalletStatusReport) -> String {
    let mut lines = vec![
        format!("Status: {}", title_case_status(report.runtime_status.as_str())),
        format!("Network: {}", report.runtime.network),
        format!("Total balance: {}", format_sats(report.balance.total_sats)),
        format!(
            "Balance mix: {} Spark, {} Lightning, {} on-chain",
            format_sats(report.balance.spark_sats),
            format_sats(report.balance.lightning_sats),
            format_sats(report.balance.onchain_sats)
        ),
    ];
    if let Some(detail) = report.runtime_detail.as_deref() {
        lines.push(format!("Runtime: {detail}"));
    }
    if !report.recent_payments.is_empty() {
        lines.push(String::new());
        lines.push("Recent payments:".to_string());
        lines.extend(report.recent_payments.iter().take(5).map(|payment| {
            format!(
                "  {} {} {}",
                title_case_status(payment.direction.as_str()),
                format_sats(payment.amount_sats),
                title_case_status(payment.status.as_str())
            )
        }));
    }
    lines.join("\n")
}

fn render_wallet_receive_output(report: &pylon::WalletAddressReport) -> String {
    [
        format!("Network: {}", report.runtime.network),
        String::new(),
        "Receive on Spark:".to_string(),
        report.spark_address.clone(),
        String::new(),
        "Receive on Bitcoin:".to_string(),
        report.bitcoin_address.clone(),
        String::new(),
        "Next: /wallet invoice <sats> to create a Lightning invoice.".to_string(),
    ]
    .join("\n")
}

fn render_wallet_invoice_output(report: &pylon::WalletInvoiceReport) -> String {
    let mut lines = vec![
        format!("Lightning invoice ready for {}", format_sats(report.invoice.amount_sats)),
        format!("Network: {}", report.runtime.network),
    ];
    if let Some(description) = report.invoice.description.as_deref() {
        lines.push(format!("Description: {description}"));
    }
    lines.push(String::new());
    lines.push("Invoice:".to_string());
    lines.push(report.invoice.payment_request.clone());
    lines.join("\n")
}

fn render_wallet_pay_output(report: &pylon::WalletPayReport) -> String {
    let mut lines = vec![
        format!("Lightning withdrawal submitted on {}", report.runtime.network),
        format!("Amount: {}", format_sats(report.payment.amount_sats)),
        format!("Fees: {}", format_sats(report.payment.fees_sats)),
        format!("Wallet balance: {}", format_sats(report.post_balance.total_sats)),
    ];
    if let Some(description) = report.payment.description.as_deref() {
        lines.push(format!("Description: {description}"));
    }
    lines.join("\n")
}

fn render_wallet_history_output(report: &pylon::WalletHistoryReport) -> String {
    let mut lines = vec![format!("Network: {}", report.runtime.network)];
    if report.payments.is_empty() {
        lines.push("History: none".to_string());
        return lines.join("\n");
    }
    lines.push("Recent wallet payments:".to_string());
    lines.extend(report.payments.iter().take(10).map(|payment| {
        format!(
            "  {} {} {}",
            title_case_status(payment.direction.as_str()),
            format_sats(payment.amount_sats),
            title_case_status(payment.status.as_str())
        )
    }));
    lines.join("\n")
}

fn reveal_wallet_recovery_phrase(config_path: &Path) -> Result<String> {
    let config = pylon::load_config_or_default(config_path)?;
    let mnemonic = std::fs::read_to_string(config.identity_path.as_path())
        .with_context(|| {
            format!(
                "failed to read recovery phrase {}",
                config.identity_path.display()
            )
        })?
        .trim()
        .to_string();
    if mnemonic.is_empty() {
        bail!(
            "recovery phrase is empty at {}",
            config.identity_path.display()
        );
    }
    Ok([
        "Recovery phrase".to_string(),
        "Handle with care. This unlocks both your Spark wallet and your node identity."
            .to_string(),
        format!("Stored at: {}", config.identity_path.display()),
        String::new(),
        mnemonic,
    ]
    .join("\n"))
}

fn state_badge_style(label: &str) -> Style {
    match label {
        "Earning now" | "Ready to earn" | "Listening for work" | "healthy" => success_accent(),
        "Waiting for payout" | "warming up" | "Connecting" => warning_accent(),
        "Needs attention" => danger_accent(),
        _ => shell_accent(),
    }
}

fn provider_job_counted_as_matching(status: &str) -> bool {
    status != "observed_drop"
}

fn provider_job_counted_as_processed(status: &str) -> bool {
    matches!(
        status,
        "completed_local"
            | "failed_local"
            | "publish_failed"
            | "delivery_failed_after_payment"
            | "settled"
    )
}

fn provider_job_is_in_progress(status: &str) -> bool {
    matches!(
        status,
        "accepted_local" | "processing_local" | "payment_settled"
    )
}

fn wallet_status_balance_is_authoritative(report: &pylon::WalletStatusReport) -> bool {
    report.runtime_status == "connected" || report.balance.total_sats > 0
}

fn provider_settlement_counted_as_settled(status: &str) -> bool {
    matches!(status, "settled" | "payment_received")
}

fn latest_provider_job_status(ledger: &pylon::PylonLedger) -> Option<String> {
    ledger
        .jobs
        .iter()
        .filter(|job| job.direction == "provider")
        .max_by_key(|job| job.updated_at_ms)
        .map(|job| job.status.clone())
}

fn recent_provider_activity(ledger: &pylon::PylonLedger, now_ms: u64) -> Vec<String> {
    let mut entries = Vec::new();

    for settlement in ledger
        .settlements
        .iter()
        .filter(|settlement| settlement.direction == "provider")
    {
        let amount_sats = settlement.amount_msats / 1_000;
        entries.push((
            settlement.updated_at_ms,
            format!(
                "[PAID] stacked {} {} ago",
                format_sats(amount_sats),
                format_elapsed_since_ms(settlement.updated_at_ms, now_ms)
            ),
        ));
    }

    for job in ledger.jobs.iter().filter(|job| job.direction == "provider") {
        if let Some(label) = provider_job_activity_label(job) {
            entries.push((
                job.updated_at_ms,
                format!(
                    "{label} {} ago",
                    format_elapsed_since_ms(job.updated_at_ms, now_ms)
                ),
            ));
        }
    }

    entries.sort_by(|left, right| right.0.cmp(&left.0));
    entries
        .into_iter()
        .map(|(_, label)| label)
        .take(3)
        .collect()
}

fn provider_job_activity_label(job: &pylon::PylonLedgerJob) -> Option<String> {
    match job.status.as_str() {
        "accepted_local" => Some("[LIVE] matched a paid request".to_string()),
        "processing_local" => Some("[LIVE] local Gemma is processing".to_string()),
        "payment_required" => Some("[WAIT] result delivered, payout pending".to_string()),
        "completed_local" => Some("[DONE] delivered a local result".to_string()),
        "settled" => None,
        "failed_local" => Some("[WARN] local run hit a failure".to_string()),
        _ => None,
    }
}

fn current_epoch_ms_u64() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn format_elapsed_since_ms(at_ms: u64, now_ms: u64) -> String {
    let elapsed_ms = now_ms.saturating_sub(at_ms);
    format_duration(Duration::from_millis(elapsed_ms))
}

fn local_chat_request_messages(
    history: &[pylon::LocalGemmaChatMessage],
    prompt: &str,
) -> Vec<pylon::LocalGemmaChatMessage> {
    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(pylon::LocalGemmaChatMessage::system(
        LOCAL_CHAT_PLAIN_TEXT_POLICY,
    ));
    messages.extend(history.iter().cloned());
    messages.push(pylon::LocalGemmaChatMessage::user(prompt));
    messages
}

async fn sync_provider_presence_for_refresh(
    config_path: &Path,
    provider_presence_session_id: &str,
    desired_mode: ProviderDesiredMode,
    snapshot: Option<&ProviderPersistedSnapshot>,
    provider_presence_online: bool,
    heartbeat_due: bool,
) -> bool {
    if !should_publish_provider_presence(desired_mode, snapshot) {
        if provider_presence_online {
            let _ = pylon::report_provider_presence_offline_for_config(
                config_path,
                provider_presence_session_id,
            )
            .await;
        }
        return false;
    }

    match snapshot {
        Some(snapshot) if !provider_presence_online || heartbeat_due => {
            pylon::report_provider_presence_heartbeat_for_snapshot(
                config_path,
                provider_presence_session_id,
                snapshot,
            )
            .await
            .is_ok()
        }
        Some(_) => provider_presence_online,
        None => false,
    }
}

fn collect_gemma4_backend_models(backend: &ProviderBackendHealth, models: &mut Vec<String>) {
    if let Some(model) = backend.ready_model.as_deref() {
        if is_gemma4_model(model) {
            models.push(model.to_string());
        }
    }
    for model in &backend.available_models {
        if is_gemma4_model(model.as_str()) {
            models.push(model.clone());
        }
    }
}

fn gemma_runtime_ready_models(loaded: Option<&LoadedState>) -> Vec<String> {
    let Some(snapshot) = loaded.and_then(|loaded| loaded.snapshot.as_ref()) else {
        return Vec::new();
    };
    let mut models = Vec::new();
    collect_gemma4_backend_ready_models(&snapshot.availability.local_gemma, &mut models);
    collect_gemma4_backend_ready_models(&snapshot.availability.apple_foundation_models, &mut models);
    sort_and_dedup(&mut models);
    models
}

fn collect_gemma4_backend_ready_models(backend: &ProviderBackendHealth, models: &mut Vec<String>) {
    if let Some(model) = backend.ready_model.as_deref() {
        if is_gemma4_model(model) {
            models.push(model.to_string());
        }
    }
}

fn is_gemma4_model(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.contains("gemma4") || normalized.contains("gemma-4")
}

fn detect_gpu_summary() -> Result<String> {
    if cfg!(target_os = "macos") {
        return detect_macos_gpu_summary();
    }
    detect_nvidia_gpu_summary()
}

fn detect_macos_gpu_summary() -> Result<String> {
    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json", "-detailLevel", "mini"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("system_profiler failed"));
    }
    let payload = serde_json::from_slice::<Value>(&output.stdout)?;
    let Some(entries) = payload.get("SPDisplaysDataType").and_then(Value::as_array) else {
        return Err(anyhow!("SPDisplaysDataType missing"));
    };

    let mut summaries = entries
        .iter()
        .filter_map(|entry| {
            let model = entry
                .get("sppci_model")
                .or_else(|| entry.get("_name"))
                .and_then(Value::as_str)?;
            let vram = entry
                .get("spdisplays_vram")
                .or_else(|| entry.get("spdisplays_vram_shared"))
                .and_then(Value::as_str);
            Some(match vram {
                Some(vram) if !vram.trim().is_empty() => format!("{model} ({vram})"),
                _ => model.to_string(),
            })
        })
        .collect::<Vec<_>>();

    sort_and_dedup(&mut summaries);
    if summaries.is_empty() {
        return Err(anyhow!("no gpu entries reported"));
    }
    Ok(summaries.join(", "))
}

fn detect_nvidia_gpu_summary() -> Result<String> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.free,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("nvidia-smi unavailable"));
    }
    let stdout = String::from_utf8(output.stdout)?;
    let mut lines = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    sort_and_dedup(&mut lines);
    if lines.is_empty() {
        return Err(anyhow!("no nvidia gpu rows reported"));
    }
    Ok(lines.join(", "))
}

fn detect_disk_summary(disks: &[sysinfo::Disk], path: &Path) -> Option<String> {
    let disk = select_disk_for_path(disks, path)?;
    Some(format!(
        "{} free / {} total @ {}",
        format_byte_size(disk.available_space()),
        format_byte_size(disk.total_space()),
        disk.mount_point().display()
    ))
}

fn detect_disk_io_summary(disks: &[sysinfo::Disk], path: &Path) -> Option<String> {
    let disk = select_disk_for_path(disks, path)?;
    let usage = disk.usage();
    Some(format!(
        "read Δ{} write Δ{} total read {} total write {}",
        format_byte_size(usage.read_bytes),
        format_byte_size(usage.written_bytes),
        format_byte_size(usage.total_read_bytes),
        format_byte_size(usage.total_written_bytes),
    ))
}

fn select_disk_for_path<'a>(disks: &'a [sysinfo::Disk], path: &Path) -> Option<&'a sysinfo::Disk> {
    disks
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .or_else(|| {
            disks
                .iter()
                .find(|disk| disk.mount_point() == Path::new("/"))
        })
}

fn detect_network_summary(networks: &Networks) -> Option<String> {
    let mut received = 0_u64;
    let mut transmitted = 0_u64;
    let mut total_received = 0_u64;
    let mut total_transmitted = 0_u64;
    let mut interface_count = 0_usize;

    for (name, network) in networks {
        if is_ignored_network_interface(name.as_str()) {
            continue;
        }
        interface_count += 1;
        received += network.received();
        transmitted += network.transmitted();
        total_received += network.total_received();
        total_transmitted += network.total_transmitted();
    }

    if interface_count == 0 {
        return None;
    }

    Some(format!(
        "{} interfaces  rx Δ{} tx Δ{} total rx {} total tx {}",
        interface_count,
        format_byte_size(received),
        format_byte_size(transmitted),
        format_byte_size(total_received),
        format_byte_size(total_transmitted),
    ))
}

fn is_ignored_network_interface(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized.starts_with("lo")
}

fn detect_thermal_summary(components: &Components) -> Option<String> {
    let hottest = components
        .iter()
        .filter_map(|component| {
            component
                .temperature()
                .map(|temperature| (component, temperature))
        })
        .max_by(|(_, left), (_, right)| left.total_cmp(right))?;
    let critical = hottest
        .0
        .critical()
        .map(|value| format!(" crit {:.1}C", value))
        .unwrap_or_default();
    Some(format!(
        "{} {:.1}C{}",
        hottest.0.label(),
        hottest.1,
        critical
    ))
}

fn detect_power_status() -> (Option<String>, Option<String>) {
    if cfg!(target_os = "macos") {
        let summary = detect_macos_power_summary().ok();
        return (summary, None);
    }
    if let Ok(summary) = detect_nvidia_power_summary() {
        return (Some(String::from("GPU power telemetry")), Some(summary));
    }
    (None, None)
}

fn detect_nvidia_power_summary() -> Result<String> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,power.draw,power.limit",
            "--format=csv,noheader,nounits",
        ])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("nvidia-smi unavailable"));
    }
    let stdout = String::from_utf8(output.stdout)?;
    let mut rows = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.split(',').map(str::trim);
            let name = parts.next()?;
            let draw = parts.next().unwrap_or("unknown");
            let limit = parts.next().unwrap_or("unknown");
            Some(format!("{name} {draw}W / {limit}W"))
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(anyhow!("no nvidia power rows reported"));
    }
    sort_and_dedup(&mut rows);
    Ok(rows.join(", "))
}

fn detect_macos_power_summary() -> Result<String> {
    let output = Command::new("pmset").args(["-g", "batt"]).output()?;
    if !output.status.success() {
        return Err(anyhow!("pmset failed"));
    }
    let stdout = String::from_utf8(output.stdout)?;
    let mut lines = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let source_line = lines
        .next()
        .ok_or_else(|| anyhow!("pmset source missing"))?;
    let detail_line = lines.next().unwrap_or_default();
    let source = source_line
        .split('\'')
        .nth(1)
        .unwrap_or(source_line)
        .to_string();
    let detail = detail_line
        .split('\t')
        .nth(1)
        .unwrap_or(detail_line)
        .split(" present:")
        .next()
        .unwrap_or(detail_line)
        .trim();
    if detail.is_empty() {
        Ok(source)
    } else {
        Ok(format!("{source}  {detail}"))
    }
}

fn sort_and_dedup(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn comma_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    format!("{}d", hours / 24)
}

fn format_sats(value: u64) -> String {
    format!("{} sats", format_u64_with_commas(value))
}

fn format_u64_with_commas(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn format_byte_size(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let value = value as f64;
    if value >= TIB {
        format!("{:.1} TiB", value / TIB)
    } else if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{} B", value as u64)
    }
}

fn format_frequency(mhz: u64) -> String {
    if mhz >= 1000 {
        format!("{:.2} GHz", mhz as f64 / 1000.0)
    } else {
        format!("{mhz} MHz")
    }
}

fn format_percent(value: f32) -> String {
    format!("{value:.0}%")
}

fn format_load_average((one, five, fifteen): (f64, f64, f64)) -> String {
    format!("{one:.2} / {five:.2} / {fifteen:.2}")
}

fn installed_gemma_models(config_path: &Path) -> BTreeMap<String, u64> {
    pylon::gemma_local_installations(config_path)
        .into_iter()
        .filter_map(|installation| {
            installation
                .file_bytes
                .map(|file_bytes| (installation.spec.id.to_string(), file_bytes))
        })
        .collect()
}

fn available_download_ids() -> String {
    pylon::gemma_download_specs()
        .iter()
        .map(|spec| spec.id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn download_progress_label(progress: &GemmaDownloadProgressState) -> String {
    match progress.total_bytes {
        Some(total_bytes) if total_bytes > 0 => {
            let percent = progress.downloaded_bytes as f64 / total_bytes as f64 * 100.0;
            format!(
                "{:.0}%  {}/{}",
                percent,
                format_byte_size(progress.downloaded_bytes),
                format_byte_size(total_bytes)
            )
        }
        _ => format!("{} downloaded", format_byte_size(progress.downloaded_bytes)),
    }
}

fn relay_report_lines(report: &pylon::RelayReport) -> Vec<String> {
    let mut lines = vec![format!(
        "connect timeout: {}s  ledger: {}",
        report.relay_config.connect_timeout_seconds, report.relay_config.ledger_path
    )];
    if report.relays.is_empty() {
        lines.push(String::from("no relays configured"));
        return lines;
    }
    for relay in &report.relays {
        lines.push(String::new());
        lines.push(format!(
            "{}  state={}  auth={}",
            relay.url, relay.state, relay.auth_state
        ));
        if let Some(detail) = relay.detail.as_deref() {
            lines.push(format!("  detail: {detail}"));
        }
        if let Some(last_error) = relay.last_error.as_deref() {
            lines.push(format!("  error: {last_error}"));
        }
    }
    lines
}

fn wallet_command_title(command: &pylon::WalletSubcommand) -> String {
    match command {
        pylon::WalletSubcommand::Status { .. } => "Wallet Status",
        pylon::WalletSubcommand::Balance { .. } => "Wallet Balance",
        pylon::WalletSubcommand::Address { .. } => "Wallet Address",
        pylon::WalletSubcommand::Invoice { .. } => "Wallet Invoice",
        pylon::WalletSubcommand::Pay { .. } => "Wallet Pay",
        pylon::WalletSubcommand::History { .. } => "Wallet History",
    }
    .to_string()
}

fn transcript_wrap_width(area: Rect) -> u16 {
    area.width.saturating_sub(4).max(1)
}

fn transcript_viewport_height(area: Rect) -> u16 {
    area.height.saturating_sub(2).max(1)
}

fn max_transcript_scroll_y(rendered_rows: usize, viewport_height: u16) -> u16 {
    rendered_rows
        .saturating_sub(usize::from(viewport_height))
        .min(u16::MAX as usize) as u16
}

fn wrapped_row_count(line: &str, wrap_width: u16) -> usize {
    let wrap_width = usize::from(wrap_width.max(1));
    let display_width = UnicodeWidthStr::width(line);
    display_width.max(1).div_ceil(wrap_width)
}

#[derive(Clone, Debug, PartialEq)]
struct ChatMetricsSummary {
    ttft_seconds: Option<f64>,
    total_seconds: f64,
    tokens_per_second: Option<f64>,
}

fn active_chat_title(model: &str, metrics: Option<&ChatMetricsSummary>) -> String {
    let mut title = format!("Local Gemma {model}");
    if let Some(metrics) = metrics {
        if let Some(ttft_seconds) = metrics.ttft_seconds {
            title.push_str(format!("  ttft {:.2}s", ttft_seconds).as_str());
        }
        title.push_str(format!("  total {:.2}s", metrics.total_seconds).as_str());
        if let Some(tokens_per_second) = metrics.tokens_per_second {
            title.push_str(format!("  {:.1} tok/s", tokens_per_second).as_str());
        }
    }
    title
}

fn summarize_chat_metrics(metrics: ActiveChatMetrics, text: &str) -> ChatMetricsSummary {
    let finished_at = Instant::now();
    let total_seconds = finished_at
        .saturating_duration_since(metrics.started_at)
        .as_secs_f64();
    let ttft_seconds = metrics.first_token_at.map(|first| {
        first
            .saturating_duration_since(metrics.started_at)
            .as_secs_f64()
    });
    let estimated_tokens = estimate_token_count(text);
    let generation_seconds = metrics
        .first_token_at
        .map(|first| finished_at.saturating_duration_since(first).as_secs_f64())
        .filter(|seconds| *seconds > 0.0);
    let tokens_per_second = generation_seconds
        .and_then(|seconds| (estimated_tokens > 0).then_some(estimated_tokens as f64 / seconds));
    ChatMetricsSummary {
        ttft_seconds,
        total_seconds,
        tokens_per_second,
    }
}

fn estimate_token_count(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let whitespace_tokens = trimmed.split_whitespace().count();
    let char_estimate = trimmed.chars().count().div_ceil(4);
    whitespace_tokens.max(char_estimate)
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveChatMetrics, AppShell, ChatMetricsSummary, ComposerSubmission,
        LOCAL_CHAT_PLAIN_TEXT_POLICY, OperatorPanelStats, ProviderCommandInFlight,
        WalletSurfaceState, WorkerEvent, active_chat_title, animated_right_now_suffix,
        animated_stack_gain_suffix, compute_operator_panel_stats_at, current_epoch_ms_u64,
        estimate_token_count, local_chat_request_messages, max_transcript_scroll_y,
        mission_control_signal_spans,
        parse_tui_buyer_job_history_request, parse_tui_buyer_job_policy_mode,
        parse_tui_buyer_job_request_id, parse_tui_buyer_job_submit_request,
        parse_tui_buyer_job_watch_request, parse_tui_optional_limit,
        parse_tui_payout_history_request, parse_tui_payout_withdraw_request,
        recent_provider_activity, render_rank_progress_spans, should_publish_provider_presence,
        stacker_rank_progress,
        stabilize_operator_panel_stats, summarize_chat_metrics, transcript_viewport_height,
        transcript_wrap_width, wrapped_row_count,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use openagents_provider_substrate::{ProviderDesiredMode, ProviderPersistedSnapshot};
    use ratatui::layout::Rect;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn transcript_text(app: &AppShell) -> String {
        app.transcript
            .as_text()
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn summary_lines_focus_on_system_and_gemma_state() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.system_stats.host_name = Some("rig".to_string());
        app.system_stats.os_version = Some("macOS 15".to_string());
        app.system_stats.cpu_arch = Some("arm64".to_string());
        app.system_stats.uptime_seconds = Some(3_660);
        app.system_stats.kernel_version = Some("Darwin 24".to_string());
        app.system_stats.cpu_brand = Some("Apple M4".to_string());
        app.system_stats.logical_cpus = 12;
        app.system_stats.cpu_usage_percent = Some(42.0);
        app.system_stats.load_average = Some((1.0, 0.8, 0.6));
        app.system_stats.used_memory_bytes = Some(8 * 1024 * 1024 * 1024);
        app.system_stats.total_memory_bytes = Some(16 * 1024 * 1024 * 1024);
        app.system_stats.used_swap_bytes = Some(0);
        app.system_stats.total_swap_bytes = Some(0);
        app.system_stats.gpu_summary = Some("Apple GPU ready".to_string());
        app.system_stats.network_summary = Some("wifi up".to_string());
        app.system_stats.disk_summary = Some("120 GiB free on /".to_string());
        app.system_stats.thermal_summary = Some("stable".to_string());
        app.loaded = Some(super::LoadedState {
            snapshot: Some(Default::default()),
            wallet_status: None,
        });

        let summary = app
            .summary_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(summary.contains("Health: warming up"));
        assert!(summary.contains("CPU: Apple M4, 42% usage"));
        assert!(summary.contains("Memory: 8.0 GiB / 16.0 GiB"));
        assert!(summary.contains("GPU: Apple GPU ready"));
        assert!(!summary.contains("Gemma runtime:"));
        assert!(!summary.contains("Cache:"));
        assert!(!summary.contains("Network:"));
        assert!(!summary.contains("Disk:"));
        assert!(!summary.contains("wallet:"));
    }

    #[test]
    fn operator_panel_shows_balance_and_24h_counters() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.last_refresh_at = Some(Instant::now());
        app.operator_stats = OperatorPanelStats {
            desired_mode: ProviderDesiredMode::Online,
            runtime_status: Some("online".to_string()),
            runtime_error: None,
            backend_label: Some("local_gemma".to_string()),
            provider_presence_online: true,
            wallet_runtime_status: Some("connected".to_string()),
            wallet_balance: Some(pylon::WalletBalanceSnapshot {
                spark_sats: 21,
                lightning_sats: 34,
                onchain_sats: 55,
                total_sats: 110,
            }),
            wallet_balance_live: true,
            jobs_found_24h: 8,
            matching_jobs_24h: 3,
            jobs_processed_24h: 2,
            jobs_settled_24h: 1,
            session_earnings_sats: 21,
            settled_sats_24h: 21,
            settled_sats_lifetime: 110,
            total_earnings_sats: 110,
            awaiting_payment_jobs: 0,
            processing_jobs: 0,
            last_job_result: Some("settled".to_string()),
            last_provider_event_at_ms: Some(1_762_700_500_000),
            recent_activity: vec![String::from("settled 21 sats 6m ago")],
            online_uptime_seconds: Some(60),
        };

        let sidebar = app
            .operator_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(sidebar.contains("Session stack: 21 sats"));
        assert!(sidebar.contains("Lifetime stack: 110 sats"));
        assert!(sidebar.contains("Active now: 0 processing, 0 awaiting payout"));
        assert!(!sidebar.contains("Status:"));
        assert!(!sidebar.contains("Mission:"));
        assert!(!sidebar.contains("recent activity"));
        assert!(!sidebar.contains("proof:"));
        assert!(!sidebar.contains("last result:"));
        assert!(!sidebar.contains("online for"));
        assert!(!sidebar.contains("runtime:"));
        assert!(!sidebar.contains("mode:"));
        assert!(!sidebar.contains("provider:"));
    }

    #[test]
    fn right_now_suffix_only_animates_when_work_is_active() {
        assert_eq!(animated_right_now_suffix(0, 0, 0), "");
        assert_eq!(animated_right_now_suffix(1, 0, 0), "  •");
        assert_eq!(animated_right_now_suffix(1, 0, 2), "  •••");
        assert_eq!(animated_right_now_suffix(0, 1, 2), "  ···");
    }

    #[test]
    fn motion_helpers_match_shell_states() {
        let ready = mission_control_signal_spans("Ready to earn", 0)
            .iter()
            .map(ToString::to_string)
            .collect::<String>();
        let listening = mission_control_signal_spans("Listening for work", 1)
            .iter()
            .map(ToString::to_string)
            .collect::<String>();
        assert!(ready.contains('•'));
        assert!(listening.contains('·'));
        assert!(animated_stack_gain_suffix(0, true).contains('▁'));
        assert_eq!(animated_stack_gain_suffix(0, false), "");
    }

    #[test]
    fn rank_progress_rail_glints_and_marks_position() {
        let spans = render_rank_progress_spans(0.4, 10, 3, true);
        let rendered = spans.iter().map(ToString::to_string).collect::<String>();
        assert!(rendered.contains('◉') || rendered.contains('◎'));
        assert!(rendered.contains('─'));
        assert!(rendered.contains('╌'));
    }

    #[test]
    fn activity_pulse_appears_when_recent_activity_changes() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.handle_worker_event(WorkerEvent::RefreshCompleted {
            loaded: None,
            installed_gemma_models: Default::default(),
            operator_stats: OperatorPanelStats {
                recent_activity: vec![String::from("[LIVE] accepted a job 1s ago")],
                ..OperatorPanelStats::default()
            },
            wallet_surface: WalletSurfaceState::default(),
            last_error: None,
            last_wallet_error: None,
            provider_presence_online: false,
            nexus_treasury_health: None,
        });

        assert!(app.live_activity_pulse_until.is_some());
    }

    #[test]
    fn paid_and_rank_moments_activate_after_refresh_growth() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.last_refresh_at = Some(Instant::now());
        app.operator_stats.session_earnings_sats = 2;
        app.operator_stats.total_earnings_sats = 900;

        app.handle_worker_event(WorkerEvent::RefreshCompleted {
            loaded: None,
            installed_gemma_models: Default::default(),
            operator_stats: OperatorPanelStats {
                session_earnings_sats: 6,
                total_earnings_sats: 1_200,
                recent_activity: vec![String::from("[PAID] stacked 4 sats just now")],
                ..OperatorPanelStats::default()
            },
            wallet_surface: WalletSurfaceState::default(),
            last_error: None,
            last_wallet_error: None,
            provider_presence_online: true,
            nexus_treasury_health: None,
        });

        assert!(app.visible_paid_moment().is_some());
        let rank_moment = app.visible_rank_up_moment().expect("rank-up moment");
        assert_eq!(rank_moment.0, "Pleb");
        assert_eq!(rank_moment.1, "Drifter");
    }

    #[test]
    fn stacker_rank_progress_advances_through_tiers() {
        let pleb = stacker_rank_progress(644);
        assert_eq!(pleb.current.name, "Pleb");
        assert_eq!(pleb.next.map(|rank| rank.name), Some("Drifter"));

        let operator = stacker_rank_progress(1_250_000);
        assert_eq!(operator.current.name, "Operator");
        assert_eq!(operator.next.map(|rank| rank.name), Some("Captain"));

        let king = stacker_rank_progress(100_000_000);
        assert_eq!(king.current.name, "King");
        assert!(king.next.is_none());
        assert_eq!(king.progress_ratio, 1.0);
    }

    #[test]
    fn compute_operator_panel_stats_uses_retained_balance_until_live_balance_is_authoritative() {
        let now_ms = 1_762_700_500_000_u64;
        let mut ledger = pylon::PylonLedger::default();
        ledger.wallet.last_balance_sats = Some(377);

        let wallet_status = pylon::WalletStatusReport {
            runtime: pylon::WalletRuntimeSurface::default(),
            runtime_status: "disconnected".to_string(),
            runtime_detail: Some("syncing".to_string()),
            balance: pylon::WalletBalanceSnapshot::default(),
            recent_payments: Vec::new(),
        };

        let stats = compute_operator_panel_stats_at(
            ProviderDesiredMode::Online,
            true,
            Some(&wallet_status),
            None,
            &ledger,
            0,
            now_ms,
        );

        assert!(!stats.wallet_balance_live);
        assert_eq!(
            stats.wallet_balance.as_ref().map(|balance| balance.total_sats),
            Some(377)
        );
    }

    #[test]
    fn compute_operator_panel_stats_keeps_lifetime_earned_independent_from_wallet_balance() {
        let now_ms = 1_762_700_500_000_u64;
        let mut ledger = pylon::PylonLedger::default();
        ledger.settlements.push(pylon::PylonSettlementRecord {
            settlement_id: "settlement-001".to_string(),
            job_id: "job-paid".to_string(),
            direction: "provider".to_string(),
            status: "settled".to_string(),
            amount_msats: 55_000,
            payment_reference: Some("payment-001".to_string()),
            receipt_detail: Some("provider payout settled".to_string()),
            created_at_ms: now_ms - 5_000,
            updated_at_ms: now_ms - 4_000,
        });

        let wallet_status = pylon::WalletStatusReport {
            runtime: pylon::WalletRuntimeSurface::default(),
            runtime_status: "connected".to_string(),
            runtime_detail: Some("wallet synced after withdrawal".to_string()),
            balance: pylon::WalletBalanceSnapshot {
                spark_sats: 8,
                lightning_sats: 0,
                onchain_sats: 0,
                total_sats: 8,
            },
            recent_payments: Vec::new(),
        };

        let stats = compute_operator_panel_stats_at(
            ProviderDesiredMode::Online,
            true,
            Some(&wallet_status),
            None,
            &ledger,
            0,
            now_ms,
        );

        assert_eq!(
            stats.wallet_balance.as_ref().map(|balance| balance.total_sats),
            Some(8)
        );
        assert_eq!(stats.settled_sats_lifetime, 55);
        assert_eq!(stats.total_earnings_sats, 55);
    }

    #[test]
    fn model_panel_separates_runtime_ready_from_optional_cache_entries() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        let mut snapshot = ProviderPersistedSnapshot::default();
        snapshot.availability.local_gemma.ready_model = Some("gemma4:e4b".to_string());
        snapshot
            .availability
            .local_gemma
            .available_models = vec!["gemma4:e4b".to_string()];
        app.loaded = Some(super::LoadedState {
            snapshot: Some(snapshot),
            wallet_status: None,
        });

        let models = app
            .model_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(models.contains("runtime ready: gemma4:e4b"));
        assert!(models.contains("gemma-4-e4b  missing"));
    }

    #[test]
    fn operator_panel_marks_run_activity_and_payment_waits_honestly() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        let (state, detail) = app.operator_state_label_and_detail();
        assert_eq!(state, "Preparing to earn");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.contains("Starting local checks"))
        );
        app.operator_stats = OperatorPanelStats {
            desired_mode: ProviderDesiredMode::Online,
            runtime_status: Some("online".to_string()),
            awaiting_payment_jobs: 1,
            wallet_runtime_status: Some("connected".to_string()),
            ..OperatorPanelStats::default()
        };

        let (state, detail) = app.operator_state_label_and_detail();
        assert_eq!(state, "Waiting for payout");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.contains("waiting for settlement"))
        );

        app.operator_stats.awaiting_payment_jobs = 0;
        app.provider_command_in_flight = Some(ProviderCommandInFlight::Run {
            started_at: Instant::now() - Duration::from_secs(2),
            seconds: 5,
        });
        let (state, detail) = app.operator_state_label_and_detail();
        assert_eq!(state, "Listening for work");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.contains("running retained provider intake"))
        );

        app.provider_command_in_flight = None;
        app.operator_stats = OperatorPanelStats {
            desired_mode: ProviderDesiredMode::Online,
            runtime_status: Some("online".to_string()),
            provider_presence_online: true,
            wallet_runtime_status: Some("connected".to_string()),
            ..OperatorPanelStats::default()
        };
        let (state, detail) = app.operator_state_label_and_detail();
        assert_eq!(state, "Ready to earn");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.contains("Online for paid jobs"))
        );

        app.operator_stats = OperatorPanelStats {
            desired_mode: ProviderDesiredMode::Online,
            runtime_status: Some("online".to_string()),
            provider_presence_online: false,
            wallet_runtime_status: Some("connected".to_string()),
            ..OperatorPanelStats::default()
        };
        let (state, detail) = app.operator_state_label_and_detail();
        assert_eq!(state, "Ready to earn");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.contains("Connecting provider presence"))
        );
    }

    #[test]
    fn treasury_warning_does_not_leak_internal_status_to_default_ui() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.last_refresh_at = Some(Instant::now());
        app.nexus_treasury_health = Some(pylon::NexusTreasuryHealthSnapshot {
            payout_loop_health: "warning".to_string(),
            degraded_reason: Some("wallet_snapshot_stale:12345".to_string()),
        });
        app.operator_stats = OperatorPanelStats {
            desired_mode: ProviderDesiredMode::Online,
            runtime_status: Some("online".to_string()),
            provider_presence_online: true,
            wallet_runtime_status: Some("connected".to_string()),
            ..OperatorPanelStats::default()
        };

        let header = app
            .header_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let earnings = app
            .operator_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let activity = app
            .activity_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let user_surface = format!("{header}\n{earnings}\n{activity}");

        assert!(user_surface.contains("Ready to earn"));
        assert!(user_surface.contains("Standing by for the next paid match"));
        assert!(user_surface.contains("Session stack: 0 sats"));
        assert!(user_surface.contains("Lifetime stack: 0 sats"));
        for forbidden in [
            "Nexus",
            "treasury",
            "degraded",
            "paused",
            "recovery",
            "snapshot",
            "stale",
            "sync",
            "/provider run",
        ] {
            assert!(
                !user_surface.contains(forbidden),
                "default user surface leaked internal text: {forbidden}"
            );
        }
    }

    #[test]
    fn activity_panel_shows_recent_events_or_quiet_ready_state() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.operator_stats.recent_activity = vec![
            String::from("[PAID] stacked 21 sats 6m ago"),
            String::from("[LIVE] matched a paid request 8m ago"),
        ];
        let activity = app
            .activity_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(activity.contains("[PAID] stacked 21 sats 6m ago"));
        assert!(activity.contains("[LIVE] matched a paid request 8m ago"));

        app.operator_stats.recent_activity.clear();
        app.operator_stats.last_provider_event_at_ms = None;
        app.last_refresh_at = None;
        let quiet = app
            .activity_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(quiet.contains("[LIVE]"));
        assert!(quiet.contains("checking local Pylon setup"));
        assert!(quiet.contains("looking for local Gemma availability"));
        assert!(quiet.contains("preparing earnings state"));

        app.last_refresh_at = Some(Instant::now());
        app.operator_stats.runtime_status = Some("online".to_string());
        let ready = app
            .activity_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(ready.contains("[READY]"));
        assert!(ready.contains("standing by for paid work"));
        assert!(ready.contains("[MARKET]"));
        assert!(ready.contains("No paid matches yet"));
    }

    #[test]
    fn footer_hints_change_with_sidebar_view() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));

        let operate = app.footer_segments();
        assert!(operate.contains(&("jobs", "online")));
        assert!(operate.contains(&("/wallet receive", "cash in")));

        app.sidebar_view = super::SidebarView::Wallet;
        let wallet = app.footer_segments();
        assert!(wallet.contains(&("/wallet invoice", "request")));
        assert!(wallet.contains(&("/wallet withdraw", "send out")));

        app.sidebar_view = super::SidebarView::Inspect;
        let inspect = app.footer_segments();
        assert!(inspect.contains(&("/wallet", "view")));
        assert!(inspect.contains(&("/help", "commands")));
    }

    #[test]
    fn rank_panel_shows_progress_toward_next_tier() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.operator_stats.total_earnings_sats = 644;
        let panel = app
            .rank_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(panel.contains("Stacker Status: Pleb"));
        assert!(panel.contains("Lifetime earned: 644 sats"));
        assert!(panel.contains("Next unlock: Drifter in 356 sats"));
        assert!(!panel.contains("Remaining:"));
        assert!(!panel.contains("Track:"));
        assert!(!panel.contains("Signal:"));
        assert!(panel.contains("Pleb"));
        assert!(panel.contains("Drifter"));
        assert!(panel.contains("◉"));
    }

    #[test]
    fn stabilize_operator_panel_stats_keeps_last_wallet_balance_during_disconnect() {
        let previous = OperatorPanelStats {
            wallet_runtime_status: Some("connected".to_string()),
            wallet_balance_live: true,
            wallet_balance: Some(pylon::WalletBalanceSnapshot {
                total_sats: 628,
                ..pylon::WalletBalanceSnapshot::default()
            }),
            runtime_status: Some("online".to_string()),
            ..OperatorPanelStats::default()
        };
        let current = OperatorPanelStats {
            wallet_runtime_status: Some("disconnected".to_string()),
            wallet_balance_live: true,
            wallet_balance: Some(pylon::WalletBalanceSnapshot::default()),
            ..OperatorPanelStats::default()
        };

        let stabilized = stabilize_operator_panel_stats(previous, current);
        assert_eq!(
            stabilized.wallet_balance.as_ref().map(|balance| balance.total_sats),
            Some(628)
        );
        assert!(!stabilized.wallet_balance_live);
    }

    #[test]
    fn tab_toggles_sidebar_view() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        assert_eq!(app.sidebar_view, super::SidebarView::Operate);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.sidebar_view, super::SidebarView::Wallet);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.sidebar_view, super::SidebarView::Inspect);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.sidebar_view, super::SidebarView::Operate);
    }

    #[test]
    fn wallet_view_surfaces_addresses_and_recovery_gate() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.wallet_surface = WalletSurfaceState {
            runtime_status: Some("connected".to_string()),
            network: Some("mainnet".to_string()),
            balance: Some(pylon::WalletBalanceSnapshot {
                spark_sats: 664,
                total_sats: 664,
                ..pylon::WalletBalanceSnapshot::default()
            }),
            spark_address: Some(
                "spark1pgss97gxzmrydeh2ypjkeu9jqeve8rfy9nzy2apkks836apnwyreqrlyrtjzcu"
                    .to_string(),
            ),
            bitcoin_address: Some(
                "bc1psxsk8uzdmcg4jq03p7hf0a99r469te0ew40w32yersvlzlr697nqh94nge"
                    .to_string(),
            ),
            identity_path: Some(PathBuf::from("/tmp/pylon-test/identity.mnemonic")),
            ..WalletSurfaceState::default()
        };

        let wallet = app
            .wallet_overview_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(wallet.contains("Status:"));
        assert!(wallet.contains("Total balance: 664 sats"));

        let receive = app
            .wallet_receive_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(receive.contains("Spark address:"));
        assert!(receive.contains("spark1pgss97gx"));
        assert!(receive.contains("Bitcoin address:"));

        let recovery = app
            .wallet_recovery_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(recovery.contains("Phrase: Hidden by default"));
        assert!(recovery.contains("/wallet recovery reveal"));
    }

    #[test]
    fn wallet_card_surfaces_balance_and_clear_actions() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.wallet_surface = WalletSurfaceState {
            runtime_status: Some("connected".to_string()),
            balance: Some(pylon::WalletBalanceSnapshot {
                total_sats: 664,
                ..pylon::WalletBalanceSnapshot::default()
            }),
            spark_address: Some("spark1example".to_string()),
            bitcoin_address: Some("bc1example".to_string()),
            recent_payments: vec![
                pylon::PylonWalletPaymentRecord {
                    payment_id: "pay-older".to_string(),
                    direction: "receive".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 21,
                    fees_sats: 0,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    created_at_ms: current_epoch_ms_u64().saturating_sub(120_000),
                    updated_at_ms: current_epoch_ms_u64().saturating_sub(120_000),
                },
                pylon::PylonWalletPaymentRecord {
                    payment_id: "pay-newer".to_string(),
                    direction: "receive".to_string(),
                    status: "completed".to_string(),
                    amount_sats: 34,
                    fees_sats: 0,
                    method: "lightning".to_string(),
                    description: None,
                    invoice: None,
                    created_at_ms: current_epoch_ms_u64().saturating_sub(60_000),
                    updated_at_ms: current_epoch_ms_u64().saturating_sub(60_000),
                },
            ],
            ..WalletSurfaceState::default()
        };

        let card = app
            .wallet_card_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(card.contains("Status:"));
        assert!(card.contains("Total balance: 664 sats"));
        assert!(card.contains("Receive: Spark + Bitcoin ready"));
        assert!(card.contains("Withdraw: /wallet withdraw <lightning_invoice>"));
        assert!(card.contains("Last paid: 34 sats"));
    }

    #[test]
    fn recent_provider_activity_prefers_paid_settlements_over_duplicate_paid_job_labels() {
        let now_ms = current_epoch_ms_u64();
        let mut ledger = pylon::PylonLedger::default();
        ledger.settlements.push(pylon::PylonSettlementRecord {
            settlement_id: "set-1".to_string(),
            job_id: "job-1".to_string(),
            direction: "provider".to_string(),
            status: "settled".to_string(),
            amount_msats: 7_000,
            payment_reference: None,
            receipt_detail: None,
            created_at_ms: now_ms.saturating_sub(2_000),
            updated_at_ms: now_ms.saturating_sub(1_000),
        });
        let mut settled = pylon::PylonLedgerJob::new("job-1", "provider", 7_000, "settled");
        settled.updated_at_ms = now_ms.saturating_sub(500);
        ledger.jobs.push(settled);

        let activity = recent_provider_activity(&ledger, now_ms).join("\n");
        assert!(activity.contains("[PAID] stacked 7 sats"));
        assert!(!activity.contains("completed a paid job"));
    }

    #[test]
    fn compute_operator_panel_stats_counts_retained_provider_activity() {
        let now_ms = 1_762_700_500_000_u64;
        let mut ledger = pylon::PylonLedger::default();

        let mut settled = pylon::PylonLedgerJob::new("job-settled", "provider", 5050, "settled");
        settled.created_at_ms = now_ms - 1_000;
        settled.updated_at_ms = now_ms - 500;

        let mut awaiting =
            pylon::PylonLedgerJob::new("job-awaiting", "provider", 5050, "payment_required");
        awaiting.created_at_ms = now_ms - 2_000;
        awaiting.updated_at_ms = now_ms - 1_500;

        let mut dropped =
            pylon::PylonLedgerJob::new("job-dropped", "provider", 5050, "observed_drop");
        dropped.created_at_ms = now_ms - 3_000;
        dropped.updated_at_ms = now_ms - 2_500;

        let mut stale = pylon::PylonLedgerJob::new("job-stale", "provider", 5050, "settled");
        stale.created_at_ms = now_ms - super::LOOKBACK_WINDOW_24H_MS - 10;
        stale.updated_at_ms = now_ms - super::LOOKBACK_WINDOW_24H_MS - 5;
        ledger.jobs = vec![settled, awaiting, dropped, stale];

        ledger.settlements.push(pylon::PylonSettlementRecord {
            settlement_id: "settlement-001".to_string(),
            job_id: "job-settled".to_string(),
            direction: "provider".to_string(),
            status: "settled".to_string(),
            amount_msats: 21_000,
            payment_reference: Some("payment-001".to_string()),
            receipt_detail: Some("invoice completed in local wallet".to_string()),
            created_at_ms: now_ms - 600,
            updated_at_ms: now_ms - 400,
        });
        ledger.wallet.payments.push(pylon::PylonWalletPaymentRecord {
            payment_id: "payment-001".to_string(),
            direction: "receive".to_string(),
            status: "completed".to_string(),
            amount_sats: 4,
            fees_sats: 0,
            method: "spark".to_string(),
            description: None,
            invoice: None,
            created_at_ms: now_ms - 800,
            updated_at_ms: now_ms - 700,
        });

        let stats = compute_operator_panel_stats_at(
            ProviderDesiredMode::Online,
            true,
            None,
            None,
            &ledger,
            now_ms - 1_000,
            now_ms,
        );

        assert_eq!(stats.jobs_found_24h, 3);
        assert_eq!(stats.matching_jobs_24h, 2);
        assert_eq!(stats.jobs_processed_24h, 1);
        assert_eq!(stats.jobs_settled_24h, 1);
        assert_eq!(stats.session_earnings_sats, 4);
        assert_eq!(stats.settled_sats_24h, 21);
        assert_eq!(stats.settled_sats_lifetime, 21);
        assert_eq!(stats.total_earnings_sats, 21);
        assert_eq!(stats.awaiting_payment_jobs, 1);
        assert_eq!(stats.processing_jobs, 0);
        assert_eq!(stats.last_job_result.as_deref(), Some("settled"));
        assert_eq!(stats.last_provider_event_at_ms, Some(now_ms - 500));
        assert!(
            stats
                .recent_activity
                .iter()
                .any(|entry| entry.contains("[PAID] stacked 21 sats"))
        );
    }

    #[test]
    fn compute_operator_panel_stats_counts_paid_settlements_by_created_time() {
        let now_ms = 1_762_700_500_000_u64;
        let mut ledger = pylon::PylonLedger::default();

        let mut paid =
            pylon::PylonLedgerJob::new("job-paid", "provider", 5050, "completed_local");
        paid.created_at_ms = now_ms - 5_000;
        paid.updated_at_ms = now_ms - 4_000;
        ledger.jobs = vec![paid];

        ledger.settlements.push(pylon::PylonSettlementRecord {
            settlement_id: "settlement-paid".to_string(),
            job_id: "job-paid".to_string(),
            direction: "provider".to_string(),
            status: "payment_received".to_string(),
            amount_msats: 42_000,
            payment_reference: Some("payment-paid".to_string()),
            receipt_detail: Some("invoice completed in local wallet".to_string()),
            created_at_ms: now_ms - 1_000,
            updated_at_ms: now_ms,
        });
        ledger.settlements.push(pylon::PylonSettlementRecord {
            settlement_id: "settlement-old".to_string(),
            job_id: "job-old".to_string(),
            direction: "provider".to_string(),
            status: "settled".to_string(),
            amount_msats: 21_000,
            payment_reference: Some("payment-old".to_string()),
            receipt_detail: Some("older settlement".to_string()),
            created_at_ms: now_ms - super::LOOKBACK_WINDOW_24H_MS - 1,
            updated_at_ms: now_ms,
        });

        let stats = compute_operator_panel_stats_at(
            ProviderDesiredMode::Online,
            true,
            None,
            None,
            &ledger,
            0,
            now_ms,
        );

        assert_eq!(stats.jobs_settled_24h, 1);
    }

    #[test]
    fn provider_presence_only_publishes_while_explicitly_online() {
        let snapshot = ProviderPersistedSnapshot::default();

        assert!(
            should_publish_provider_presence(ProviderDesiredMode::Online, Some(&snapshot)),
            "online mode should publish provider presence when a snapshot exists"
        );
        assert!(
            !should_publish_provider_presence(ProviderDesiredMode::Offline, Some(&snapshot)),
            "offline mode should stop publishing provider presence even if a snapshot exists"
        );
        assert!(
            !should_publish_provider_presence(ProviderDesiredMode::Paused, Some(&snapshot)),
            "paused mode should stop publishing provider presence even if a snapshot exists"
        );
        assert!(
            !should_publish_provider_presence(ProviderDesiredMode::Online, None),
            "online mode still requires a snapshot before publishing provider presence"
        );
    }

    #[test]
    fn plain_text_submission_starts_chat_prompt() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.handle_submission(ComposerSubmission {
            text: String::from("hello"),
            slash_command: None,
        });

        let transcript = transcript_text(&app);
        assert!(transcript.contains("[user] Prompt"));
        assert!(transcript.contains("hello"));
        assert!(transcript.contains("[system] Chat Error"));
        assert!(transcript.contains("No local Gemma weights are visible right now."));
    }

    #[test]
    fn local_chat_request_messages_prepend_plain_text_policy() {
        let history = vec![
            pylon::LocalGemmaChatMessage::user("who are you"),
            pylon::LocalGemmaChatMessage::assistant("I am Gemma 4."),
        ];

        let messages = local_chat_request_messages(history.as_slice(), "say that in french");

        assert_eq!(
            messages,
            vec![
                pylon::LocalGemmaChatMessage::system(LOCAL_CHAT_PLAIN_TEXT_POLICY),
                pylon::LocalGemmaChatMessage::user("who are you"),
                pylon::LocalGemmaChatMessage::assistant("I am Gemma 4."),
                pylon::LocalGemmaChatMessage::user("say that in french"),
            ]
        );
        assert_eq!(
            history,
            vec![
                pylon::LocalGemmaChatMessage::user("who are you"),
                pylon::LocalGemmaChatMessage::assistant("I am Gemma 4."),
            ]
        );
    }

    #[test]
    fn unknown_command_is_rejected_locally() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.handle_submission(ComposerSubmission {
            text: String::from("/plan hello"),
            slash_command: Some(String::from("plan")),
        });

        let transcript = transcript_text(&app);
        assert!(transcript.contains("[user] Command /plan"));
        assert!(transcript.contains("Unknown command /plan"));
    }

    #[test]
    fn tui_job_submit_parser_supports_flags_and_prompt() {
        let request = parse_tui_buyer_job_submit_request(
            "--bid-msats 21000 --model gemma4:e4b --provider provider-001 hello buyer",
        )
        .expect("parse buyer prompt request");
        assert_eq!(
            request,
            pylon::BuyerJobSubmitRequest {
                prompt: Some("hello buyer".to_string()),
                request_json: None,
                bid_msats: Some(21_000),
                model: Some("gemma4:e4b".to_string()),
                provider_pubkey: Some("provider-001".to_string()),
                output_mime: None,
            }
        );
    }

    #[test]
    fn tui_job_submit_parser_supports_structured_json_mode() {
        let request = parse_tui_buyer_job_submit_request("--request-json {\"prompt\":\"json\"}")
            .expect("parse buyer json request");
        assert_eq!(
            request,
            pylon::BuyerJobSubmitRequest {
                prompt: None,
                request_json: Some("{\"prompt\":\"json\"}".to_string()),
                bid_msats: None,
                model: None,
                provider_pubkey: None,
                output_mime: None,
            }
        );
    }

    #[test]
    fn tui_job_watch_parser_supports_request_id_and_seconds() {
        let parsed =
            parse_tui_buyer_job_watch_request("job-001 --seconds 12").expect("parse watch args");
        assert_eq!(parsed, (Some("job-001".to_string()), 12));
    }

    #[test]
    fn tui_job_history_parser_supports_optional_limit() {
        assert_eq!(
            parse_tui_buyer_job_history_request("").expect("no limit"),
            None
        );
        assert_eq!(
            parse_tui_buyer_job_history_request("--limit 5").expect("limit"),
            Some(5)
        );
    }

    #[test]
    fn tui_job_request_id_parser_supports_approve_and_deny() {
        assert_eq!(
            parse_tui_buyer_job_request_id("job-001", "job approve").expect("approve id"),
            "job-001"
        );
        assert_eq!(
            parse_tui_buyer_job_request_id("job-002", "job deny").expect("deny id"),
            "job-002"
        );
    }

    #[test]
    fn tui_job_policy_parser_supports_show_auto_and_manual() {
        assert_eq!(
            parse_tui_buyer_job_policy_mode("").expect("show"),
            pylon::BuyerPaymentPolicyMode::Show
        );
        assert_eq!(
            parse_tui_buyer_job_policy_mode("auto").expect("auto"),
            pylon::BuyerPaymentPolicyMode::Auto
        );
        assert_eq!(
            parse_tui_buyer_job_policy_mode("manual").expect("manual"),
            pylon::BuyerPaymentPolicyMode::Manual
        );
    }

    #[test]
    fn tui_payout_parsers_support_history_and_withdraw() {
        assert_eq!(
            parse_tui_payout_history_request("--limit 4").expect("payout limit"),
            Some(4)
        );
        assert_eq!(
            parse_tui_payout_withdraw_request("lnbc1test --amount-sats 21")
                .expect("payout withdraw"),
            ("lnbc1test".to_string(), Some(21))
        );
    }

    #[test]
    fn tui_transcript_view_parser_supports_optional_limit() {
        assert_eq!(
            parse_tui_optional_limit("", "jobs").expect("no limit"),
            None
        );
        assert_eq!(
            parse_tui_optional_limit("show --limit 6", "activity").expect("activity limit"),
            Some(6)
        );
    }

    #[test]
    fn stream_events_commit_assistant_reply() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.pending_chat_prompt = Some(String::from("hello"));
        app.active_chat_metrics = Some(ActiveChatMetrics {
            started_at: Instant::now() - Duration::from_secs(2),
            first_token_at: Some(Instant::now() - Duration::from_secs(1)),
        });
        app.handle_worker_event(WorkerEvent::StreamStarted(String::from("gemma4:e4b")));
        app.handle_worker_event(WorkerEvent::StreamDelta(String::from("hello ")));
        app.handle_worker_event(WorkerEvent::StreamDelta(String::from("world")));
        app.handle_worker_event(WorkerEvent::StreamFinished);

        let transcript = transcript_text(&app);
        assert!(transcript.contains("[assistant] Local Gemma gemma4:e4b"));
        assert!(transcript.contains("ttft "));
        assert!(transcript.contains("total "));
        assert!(transcript.contains("tok/s"));
        assert!(transcript.contains("hello world"));
        assert_eq!(
            app.chat_history,
            vec![
                pylon::LocalGemmaChatMessage::user("hello"),
                pylon::LocalGemmaChatMessage::assistant("hello world"),
            ]
        );
    }

    #[test]
    fn manual_scroll_pauses_follow_until_return_to_bottom() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        for index in 0..40 {
            app.push_system_message("Line", format!("older line {index}"));
        }
        app.update_transcript_layout(Rect::new(0, 0, 40, 12));
        app.scroll_transcript_up(6);

        assert!(!app.transcript_follow_latest);
        assert!(app.transcript_panel_title().starts_with("Transcript ^ "));

        let scrolled_offset = app.transcript_scroll_y;
        app.push_system_message("Line", "streaming marker");
        assert_eq!(app.transcript_scroll_y, scrolled_offset);
        assert!(!app.transcript_follow_latest);

        app.scroll_transcript_down(u16::MAX);
        assert!(app.transcript_follow_latest);
        assert_eq!(app.transcript_panel_title(), "Transcript");
    }

    #[test]
    fn plain_character_keys_do_not_trigger_global_hotkeys() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let transcript = transcript_text(&app);
        assert!(!app.should_quit());
        assert!(transcript.contains("[user] Prompt"));
        assert!(transcript.contains("qr"));
    }

    #[test]
    fn wrapped_row_count_counts_visual_rows() {
        assert_eq!(wrapped_row_count("", 10), 1);
        assert_eq!(wrapped_row_count("hello", 10), 1);
        assert_eq!(wrapped_row_count("abcdefghij", 10), 1);
        assert_eq!(wrapped_row_count("abcdefghijk", 10), 2);
    }

    #[test]
    fn transcript_layout_uses_inner_panel_size() {
        let area = Rect::new(0, 0, 67, 20);
        assert_eq!(transcript_wrap_width(area), 63);
        assert_eq!(transcript_viewport_height(area), 18);
        assert_eq!(max_transcript_scroll_y(40, 18), 22);
    }

    #[test]
    fn long_wrapped_lines_extend_scroll_range() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.push_system_message("Line", "x".repeat(200));
        app.update_transcript_layout(Rect::new(0, 0, 40, 10));

        assert!(app.transcript_max_scroll_y > 0);
        assert_eq!(app.transcript_scroll_y, app.transcript_max_scroll_y);
    }

    #[test]
    fn transcript_follow_latest_advances_immediately_when_new_output_arrives() {
        let mut app = AppShell::new(PathBuf::from("/tmp/pylon-test"));
        app.update_transcript_layout(Rect::new(0, 0, 40, 10));
        for index in 0..20 {
            app.push_system_message("Wallet", format!("older output line {index}"));
        }

        let previous_max = app.transcript_max_scroll_y;
        assert_eq!(app.transcript_scroll_y, previous_max);

        app.push_system_message(
            "Wallet Error",
            "failed to send spark payment: invoice may already be paid",
        );

        assert!(app.transcript_max_scroll_y > previous_max);
        assert_eq!(app.transcript_scroll_y, app.transcript_max_scroll_y);
        assert!(app.transcript_panel_title() == "Transcript");
    }

    #[test]
    fn active_chat_title_formats_metrics_on_one_line() {
        let title = active_chat_title(
            "gemma4:e4b",
            Some(&ChatMetricsSummary {
                ttft_seconds: Some(0.42),
                total_seconds: 3.18,
                tokens_per_second: Some(27.6),
            }),
        );
        assert_eq!(
            title,
            "Local Gemma gemma4:e4b  ttft 0.42s  total 3.18s  27.6 tok/s"
        );
    }

    #[test]
    fn estimate_token_count_uses_text_size_floor() {
        assert_eq!(estimate_token_count(""), 0);
        assert_eq!(estimate_token_count("hello world"), 3);
    }

    #[test]
    fn summarize_chat_metrics_reports_ttft_total_and_rate() {
        let summary = summarize_chat_metrics(
            ActiveChatMetrics {
                started_at: Instant::now() - Duration::from_millis(1400),
                first_token_at: Some(Instant::now() - Duration::from_millis(900)),
            },
            "hello world from gemma",
        );
        assert!(summary.ttft_seconds.unwrap_or_default() >= 0.4);
        assert!(summary.total_seconds >= 1.3);
        assert!(summary.tokens_per_second.unwrap_or_default() > 0.0);
    }
}
