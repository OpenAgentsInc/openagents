mod bottom_pane;
mod slash_commands;
mod transcript;

use std::collections::BTreeMap;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use bottom_pane::{BottomPane, ComposerSubmission};
use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, MouseEvent, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use openagents_provider_substrate::{ProviderBackendHealth, ProviderPersistedSnapshot};
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
const REFRESH_RATE: Duration = Duration::from_secs(2);
const GPU_REFRESH_RATE: Duration = Duration::from_secs(30);

fn shell_border() -> Style {
    Style::default().fg(Color::Rgb(0x73, 0xc2, 0xfb))
}

fn shell_accent() -> Style {
    Style::default()
        .fg(Color::Rgb(0xf8, 0xf4, 0xe3))
        .bg(Color::Rgb(0x13, 0x26, 0x3a))
        .add_modifier(Modifier::BOLD)
}

fn panel(title: &str, body: Text<'static>) -> Paragraph<'static> {
    Paragraph::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1))
                .title(format!("─ {title} "))
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
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Gemma4Status {
    loaded: bool,
    models: Vec<String>,
    note: String,
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
    next_refresh_at: Instant,
    next_gpu_refresh_at: Instant,
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
    transcript_follow_latest: bool,
    transcript_scroll_y: u16,
    transcript_wrap_width: u16,
    transcript_viewport_height: u16,
    transcript_max_scroll_y: u16,
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
            vec![String::from("Type a prompt. /help shows commands.")],
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
            next_refresh_at: Instant::now(),
            next_gpu_refresh_at: Instant::now(),
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
            transcript_follow_latest: true,
            transcript_scroll_y: 0,
            transcript_wrap_width: 0,
            transcript_viewport_height: 0,
            transcript_max_scroll_y: 0,
        }
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn should_refresh(&self) -> bool {
        Instant::now() >= self.next_refresh_at
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
                SlashCommandId::Announce => {
                    self.handle_announce_command(args);
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

        let mut messages = self.chat_history.clone();
        messages.push(pylon::LocalGemmaChatMessage::user(prompt.clone()));
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
            WorkerEvent::RelayRefreshFinished { report } => {
                self.push_system_lines("Relays", relay_report_lines(&report));
            }
            WorkerEvent::RelayRefreshFailed { error } => {
                self.push_system_message("Relay Error", error);
            }
            WorkerEvent::AnnouncementFinished { output } => {
                self.push_system_lines("Announcement", text_body_lines(output.as_str()));
            }
            WorkerEvent::AnnouncementFailed { error } => {
                self.push_system_message("Announcement Error", error);
            }
            WorkerEvent::WalletCommandFinished { title, output } => {
                self.push_system_lines(title, text_body_lines(output.as_str()));
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

    fn handle_wallet_command(&mut self, args: String) {
        let mut argv = vec![String::from("wallet")];
        if args.trim().is_empty() {
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
                pylon::run_wallet_command(config_path.as_path(), &command).await
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

    async fn refresh(&mut self) {
        self.refresh_system_stats();
        self.installed_gemma_models = installed_gemma_models(self.config_path.as_path());
        match pylon::ensure_local_setup(self.config_path.as_path()) {
            Ok(_) => match pylon::load_config_and_status(self.config_path.as_path()).await {
                Ok((_, status)) => {
                    self.loaded = Some(LoadedState {
                        snapshot: status.snapshot,
                    });
                    self.last_error = None;
                }
                Err(error) => {
                    self.loaded = Some(LoadedState { snapshot: None });
                    self.last_error = Some(error.to_string());
                }
            },
            Err(error) => {
                self.loaded = None;
                self.last_error = Some(error.to_string());
            }
        }
        self.last_refresh_at = Some(Instant::now());
        self.next_refresh_at = Instant::now() + REFRESH_RATE;
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
            ]))
            .style(shell_border());
        let area = frame.area();
        let inner = shell.inner(area);
        frame.render_widget(shell, area);

        let vertical = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(self.bottom_pane.height()),
            Constraint::Length(2),
        ])
        .split(inner);
        let middle = Layout::horizontal([Constraint::Percentage(67), Constraint::Percentage(33)])
            .split(vertical[1]);
        self.update_transcript_layout(middle[0]);
        let system_height = (self.summary_lines().len() as u16 + 2).clamp(8, 16);
        let right_column =
            Layout::vertical([Constraint::Length(system_height), Constraint::Min(10)])
                .split(middle[1]);

        frame.render_widget(self.header_panel(), vertical[0]);
        frame.render_widget(self.transcript_panel(), middle[0]);
        frame.render_widget(self.summary_panel(), right_column[0]);
        frame.render_widget(self.models_panel(), right_column[1]);
        self.bottom_pane.render(
            frame,
            vertical[2],
            shell_border(),
            shell_accent(),
            Some("Type a prompt. /help shows commands. Enter submits. Ctrl+J inserts a newline."),
        );
        frame.render_widget(self.footer_panel(), vertical[3]);
    }

    fn header_panel(&self) -> Paragraph<'static> {
        let mut spans = vec![Span::styled("Pylon", shell_accent())];
        if let Some(refresh_at) = self.last_refresh_at {
            spans.push(Span::raw(format!(
                "  refreshed {} ago",
                format_duration(refresh_at.elapsed())
            )));
        } else {
            spans.push(Span::raw("  booting"));
        }
        if self.last_error.is_some() {
            spans.push(Span::styled(
                "  refresh error",
                Style::default().fg(Color::Rgb(0xff, 0x9b, 0x7a)),
            ));
        }
        Paragraph::new(Line::from(spans))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .title("─ Status ")
                    .style(shell_border()),
            )
            .wrap(Wrap { trim: false })
    }

    fn summary_panel(&self) -> Paragraph<'static> {
        panel("Gemma + System", Text::from(self.summary_lines()))
    }

    fn models_panel(&self) -> Paragraph<'static> {
        panel("Gemma Models", Text::from(self.model_lines()))
    }

    fn transcript_panel(&self) -> Paragraph<'static> {
        Paragraph::new(self.transcript_body())
            .scroll((self.transcript_scroll_y, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .title(format!("─ {} ", self.transcript_panel_title()))
                    .style(shell_border()),
            )
            .wrap(Wrap { trim: false })
    }

    fn footer_panel(&self) -> Paragraph<'static> {
        Paragraph::new(Line::from(vec![
            Span::styled(" Ctrl+C ", shell_accent()),
            Span::raw("quit  "),
            Span::styled(" PgUp/PgDn ", shell_accent()),
            Span::raw("scroll  "),
            Span::styled(" Enter ", shell_accent()),
            Span::raw("submit  "),
            Span::styled(" /download ", shell_accent()),
            Span::raw("pull weights"),
        ]))
        .block(Block::default().style(shell_border()))
    }

    fn summary_lines(&self) -> Vec<Line<'static>> {
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
            Line::from(format!(
                "gemma loaded: {}",
                if gemma.loaded { "yes" } else { "no" }
            )),
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
                "memory: {} free / {} total  used: {}",
                self.system_stats
                    .available_memory_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .total_memory_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .used_memory_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string())
            )),
            Line::from(format!(
                "swap: {} free / {} total  used: {}",
                self.system_stats
                    .free_swap_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .total_swap_bytes
                    .map(format_byte_size)
                    .unwrap_or_else(|| "unknown".to_string()),
                self.system_stats
                    .used_swap_bytes
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
                "disk: {}",
                self.system_stats
                    .disk_summary
                    .as_deref()
                    .unwrap_or("unavailable")
            )),
            Line::from(format!(
                "disk io: {}",
                self.system_stats
                    .disk_io_summary
                    .as_deref()
                    .unwrap_or("unavailable")
            )),
            Line::from(format!(
                "network: {}",
                self.system_stats
                    .network_summary
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
            Line::from(gemma.note),
        ];
        if let Some(power) = self.system_stats.power_summary.as_deref() {
            lines.insert(
                lines.len().saturating_sub(1),
                Line::from(format!("power: {power}")),
            );
        }
        if let Some(draw) = self.system_stats.power_draw_summary.as_deref() {
            lines.insert(
                lines.len().saturating_sub(1),
                Line::from(format!("draw: {draw}")),
            );
        }
        if let Some(error) = self.last_error.as_deref() {
            lines.push(Line::from(format!("refresh error: {error}")));
        }
        lines
    }

    fn model_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
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
                lines.push(Line::from(format!(
                    "  {}",
                    download_progress_bar(progress, 12)
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

    fn transcript_body(&self) -> Text<'static> {
        if self.transcript.is_empty() {
            Text::from(vec![
                Line::from("Submitted prompts stay here."),
                Line::from("Live assistant output will stream here when chat is running."),
            ])
        } else {
            self.transcript.as_text()
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
        self.transcript_max_scroll_y = max_transcript_scroll_y(
            self.rendered_transcript_row_count(self.transcript_wrap_width),
            self.transcript_viewport_height,
        );
        if self.transcript_follow_latest {
            self.transcript_scroll_y = self.transcript_max_scroll_y;
        } else {
            self.transcript_scroll_y = self.transcript_scroll_y.min(self.transcript_max_scroll_y);
        }
    }

    fn rendered_transcript_row_count(&self, wrap_width: u16) -> usize {
        self.transcript_body()
            .lines
            .iter()
            .map(ToString::to_string)
            .map(|line| wrapped_row_count(line.as_str(), wrap_width))
            .sum()
    }
}

pub fn usage() -> &'static str {
    "Usage: pylon-tui [--config-path <path>]\n\
Controls:\n\
  Ctrl+C   quit\n\
  PgUp/PgDn / wheel  scroll transcript\n\
  Enter    submit composer\n\
  Ctrl+J   insert newline\n\
Composer:\n\
  [prompt]  stream a reply from local Gemma when weights are loaded\n\
  /help  show available commands\n\
  /relay [list|add|remove|refresh]  inspect or update configured relays\n\
  /wallet [status|balance|address|invoice|pay|history]  run retained Spark wallet commands\n\
  /download [model]  download a Gemma GGUF from Hugging Face into the local Pylon cache\n"
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

    let result = run_loop(&mut terminal, config).await;
    let cleanup_result = restore_terminal(&mut terminal);

    result.and(cleanup_result)
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    config: TuiLaunchConfig,
) -> Result<()> {
    let mut app = AppShell::new(config.config_path);
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
    collect_gemma4_backend_models(&snapshot.availability.gpt_oss, &mut models);
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

fn download_progress_bar(progress: &GemmaDownloadProgressState, width: usize) -> String {
    let Some(total_bytes) = progress.total_bytes.filter(|value| *value > 0) else {
        return "[............]".to_string();
    };
    let ratio = (progress.downloaded_bytes as f64 / total_bytes as f64).clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    format!(
        "[{}{}]",
        "#".repeat(filled.min(width)),
        ".".repeat(width.saturating_sub(filled.min(width)))
    )
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
        ActiveChatMetrics, AppShell, ChatMetricsSummary, ComposerSubmission, WorkerEvent,
        active_chat_title, estimate_token_count, max_transcript_scroll_y, summarize_chat_metrics,
        transcript_viewport_height, transcript_wrap_width, wrapped_row_count,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
