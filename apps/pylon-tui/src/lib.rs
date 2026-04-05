use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use openagents_provider_substrate::{
    ProviderBackendHealth, ProviderPersistedSnapshot, ProviderStatusResponse,
    provider_runtime_state_label,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde_json::Value;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

const TICK_RATE: Duration = Duration::from_millis(200);
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
    config: pylon::PylonConfig,
    status: ProviderStatusResponse,
}

impl LoadedState {
    fn state_label(&self) -> String {
        provider_runtime_state_label(&self.status)
    }

    fn snapshot(&self) -> Option<&ProviderPersistedSnapshot> {
        self.status.snapshot.as_ref()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Gemma4Status {
    loaded: bool,
    models: Vec<String>,
    sources: Vec<String>,
    note: String,
}

#[derive(Clone, Debug, Default)]
struct LiveSystemStats {
    cpu_brand: Option<String>,
    logical_cpus: usize,
    cpu_usage_percent: Option<f32>,
    load_average: Option<(f64, f64, f64)>,
    available_memory_bytes: Option<u64>,
    total_memory_bytes: Option<u64>,
    gpu_summary: Option<String>,
    gpu_scan_error: Option<String>,
}

struct AppShell {
    config_path: PathBuf,
    loaded: Option<LoadedState>,
    system: System,
    system_stats: LiveSystemStats,
    last_refresh_at: Option<Instant>,
    last_error: Option<String>,
    next_refresh_at: Instant,
    next_gpu_refresh_at: Instant,
    should_quit: bool,
}

impl AppShell {
    fn new(config_path: PathBuf) -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        Self {
            config_path,
            loaded: None,
            system,
            system_stats: LiveSystemStats::default(),
            last_refresh_at: None,
            last_error: None,
            next_refresh_at: Instant::now(),
            next_gpu_refresh_at: Instant::now(),
            should_quit: false,
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

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('r') => self.schedule_refresh_now(),
            _ => {}
        }
    }

    async fn refresh(&mut self) {
        self.refresh_system_stats();
        match pylon::load_config_and_status(self.config_path.as_path()).await {
            Ok((config, status)) => {
                self.loaded = Some(LoadedState { config, status });
                self.last_error = None;
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
            }
        }
        self.last_refresh_at = Some(Instant::now());
        self.next_refresh_at = Instant::now() + REFRESH_RATE;
    }

    fn refresh_system_stats(&mut self) {
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();

        self.system_stats.logical_cpus = self.system.cpus().len();
        self.system_stats.cpu_brand = self.system.cpus().iter().find_map(|cpu| {
            let brand = cpu.brand().trim();
            (!brand.is_empty()).then(|| brand.to_string())
        });
        self.system_stats.cpu_usage_percent =
            (!self.system.cpus().is_empty()).then(|| self.system.global_cpu_usage());

        let load = System::load_average();
        self.system_stats.load_average = Some((load.one, load.five, load.fifteen));
        self.system_stats.available_memory_bytes = Some(self.system.available_memory());
        self.system_stats.total_memory_bytes = Some(self.system.total_memory());

        if Instant::now() >= self.next_gpu_refresh_at || self.system_stats.gpu_summary.is_none() {
            match detect_gpu_summary() {
                Ok(summary) => {
                    self.system_stats.gpu_summary = Some(summary);
                    self.system_stats.gpu_scan_error = None;
                }
                Err(error) => {
                    self.system_stats.gpu_scan_error = Some(error.to_string());
                }
            }
            self.next_gpu_refresh_at = Instant::now() + GPU_REFRESH_RATE;
        }
    }

    fn render(&self, frame: &mut Frame<'_>) {
        let shell = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .title(Line::from(vec![
                Span::styled(" Pylon ", shell_accent()),
                Span::styled(" gemma + system ", shell_border()),
            ]))
            .style(shell_border());
        let area = frame.area();
        let inner = shell.inner(area);
        frame.render_widget(shell, area);

        let vertical = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(inner);
        let lower = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(vertical[2]);

        frame.render_widget(self.header_panel(), vertical[0]);
        frame.render_widget(self.gemma_panel(), vertical[1]);
        frame.render_widget(self.system_panel(), lower[0]);
        frame.render_widget(self.backends_panel(), lower[1]);
        frame.render_widget(self.footer_panel(), vertical[3]);
    }

    fn header_panel(&self) -> Paragraph<'static> {
        let mut spans = vec![Span::styled("Pylon", shell_accent())];
        if let Some(loaded) = self.loaded.as_ref() {
            spans.push(Span::raw(format!(
                "  state {}  desired {}",
                loaded.state_label(),
                loaded.status.desired_mode.label()
            )));
        } else {
            spans.push(Span::raw("  loading provider state"));
        }
        if let Some(refresh_at) = self.last_refresh_at {
            spans.push(Span::raw(format!(
                "  refreshed {} ago",
                format_duration(refresh_at.elapsed())
            )));
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

    fn gemma_panel(&self) -> Paragraph<'static> {
        panel("Gemma 4", Text::from(self.gemma_lines()))
    }

    fn system_panel(&self) -> Paragraph<'static> {
        panel("System", Text::from(self.system_lines()))
    }

    fn backends_panel(&self) -> Paragraph<'static> {
        panel("Backends", Text::from(self.backend_lines()))
    }

    fn footer_panel(&self) -> Paragraph<'static> {
        Paragraph::new(Line::from(vec![
            Span::styled(" q ", shell_accent()),
            Span::raw("quit  "),
            Span::styled(" r ", shell_accent()),
            Span::raw("refresh"),
        ]))
        .block(Block::default().style(shell_border()))
    }

    fn gemma_lines(&self) -> Vec<Line<'static>> {
        let gemma = gemma4_status(self.loaded.as_ref(), self.config_path.as_path());
        let mut lines = vec![Line::from(format!(
            "loaded: {}",
            if gemma.loaded { "yes" } else { "no" }
        ))];
        lines.push(Line::from(format!(
            "models: {}",
            comma_or_none(gemma.models.as_slice())
        )));
        lines.push(Line::from(format!(
            "sources: {}",
            comma_or_none(gemma.sources.as_slice())
        )));
        if let Some(loaded) = self.loaded.as_ref() {
            lines.push(Line::from(format!(
                "provider state: {} / {}",
                loaded.state_label(),
                loaded.status.desired_mode.label()
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(gemma.note));
        if let Some(error) = self.last_error.as_deref() {
            lines.push(Line::from(""));
            lines.push(Line::from(format!("refresh error: {error}")));
        }
        lines
    }

    fn system_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from(format!(
            "cpu: {}",
            self.system_stats
                .cpu_brand
                .as_deref()
                .unwrap_or("unknown cpu")
        )));
        lines.push(Line::from(format!(
            "logical cores: {}",
            self.system_stats.logical_cpus
        )));
        lines.push(Line::from(format!(
            "cpu usage: {}",
            self.system_stats
                .cpu_usage_percent
                .map(format_percent)
                .unwrap_or_else(|| "unknown".to_string())
        )));
        lines.push(Line::from(format!(
            "load avg: {}",
            self.system_stats
                .load_average
                .map(format_load_average)
                .unwrap_or_else(|| "unknown".to_string())
        )));
        lines.push(Line::from(format!(
            "memory available: {}",
            self.system_stats
                .available_memory_bytes
                .zip(self.system_stats.total_memory_bytes)
                .map(|(available, total)| {
                    format!("{} / {}", format_bytes(available), format_bytes(total))
                })
                .unwrap_or_else(|| "unknown".to_string())
        )));
        lines.push(Line::from(format!(
            "gpu: {}",
            self.system_stats
                .gpu_summary
                .as_deref()
                .unwrap_or("not detected")
        )));
        if let Some(error) = self.system_stats.gpu_scan_error.as_deref() {
            lines.push(Line::from(format!("gpu note: {error}")));
        }
        lines
    }

    fn backend_lines(&self) -> Vec<Line<'static>> {
        let Some(loaded) = self.loaded.as_ref() else {
            return vec![Line::from("waiting for provider status")];
        };
        let Some(snapshot) = loaded.snapshot() else {
            return vec![Line::from(
                "no provider snapshot yet; start the headless serve loop to publish backend state",
            )];
        };

        let mut lines = vec![
            Line::from(backend_line("gpt-oss", &snapshot.availability.gpt_oss)),
            Line::from(backend_line(
                "apple fm",
                &snapshot.availability.apple_foundation_models,
            )),
        ];

        if snapshot
            .availability
            .pooled_inference
            .has_authoritative_state()
        {
            let pooled_models = snapshot
                .availability
                .pooled_inference
                .targetable_models
                .iter()
                .map(|target| target.model.clone())
                .collect::<Vec<_>>();
            lines.push(Line::from(format!(
                "pooled: members {}  warm replicas {}  models {}",
                snapshot.availability.pooled_inference.member_count,
                snapshot.availability.pooled_inference.warm_replica_count,
                comma_or_none(pooled_models.as_slice())
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(provider_note(
            loaded,
            self.config_path.as_path(),
        )));
        lines
    }
}

pub fn usage() -> &'static str {
    "Usage: pylon-tui [--config-path <path>]\n\
Controls:\n\
  q / Esc  quit\n\
  r        refresh now\n"
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
    execute!(stdout, EnterAlternateScreen)?;

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
        terminal.draw(|frame| app.render(frame))?;
        if event::poll(TICK_RATE)? {
            match event::read()? {
                CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code);
                }
                CrosstermEvent::Resize(_, _) => app.schedule_refresh_now(),
                _ => {}
            }
        }
        if app.should_refresh() {
            app.refresh().await;
        }
    }

    Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn gemma4_status(loaded: Option<&LoadedState>, config_path: &Path) -> Gemma4Status {
    if !config_path.exists() {
        return Gemma4Status {
            note: format!(
                "No config file exists at {}. Initialize Pylon first with `cargo pylon-headless init`.",
                config_path.display()
            ),
            ..Gemma4Status::default()
        };
    }

    let Some(loaded) = loaded else {
        return Gemma4Status {
            note: "Waiting for provider status.".to_string(),
            ..Gemma4Status::default()
        };
    };

    let Some(snapshot) = loaded.snapshot() else {
        return Gemma4Status {
            note:
                "No provider snapshot yet. Start the headless serve loop to publish backend state."
                    .to_string(),
            ..Gemma4Status::default()
        };
    };

    let mut models = Vec::new();
    let mut sources = Vec::new();

    collect_gemma4_backend_models(
        &snapshot.availability.gpt_oss,
        "gpt-oss",
        &mut models,
        &mut sources,
    );
    collect_gemma4_backend_models(
        &snapshot.availability.apple_foundation_models,
        "apple fm",
        &mut models,
        &mut sources,
    );

    if let Some(default_model) = snapshot
        .availability
        .pooled_inference
        .default_model
        .as_deref()
    {
        if is_gemma4_model(default_model) {
            models.push(default_model.to_string());
            sources.push("pooled inference".to_string());
        }
    }
    for target in &snapshot.availability.pooled_inference.targetable_models {
        if is_gemma4_model(target.model.as_str()) || is_gemma4_model(target.family.as_str()) {
            models.push(target.model.clone());
            sources.push("pooled inference".to_string());
        }
    }

    sort_and_dedup(&mut models);
    sort_and_dedup(&mut sources);

    let loaded_flag = !models.is_empty();
    let note = if loaded_flag {
        "Gemma 4 weights are visible to this node right now.".to_string()
    } else if snapshot.runtime.degraded_reason_code.as_deref() == Some("UNCONFIGURED") {
        "Pylon is still unconfigured, so no Gemma 4-serving path is visible yet.".to_string()
    } else {
        "No Gemma 4 model is visible across gpt-oss, Apple FM, or pooled inference right now."
            .to_string()
    };

    Gemma4Status {
        loaded: loaded_flag,
        models,
        sources,
        note,
    }
}

fn collect_gemma4_backend_models(
    backend: &ProviderBackendHealth,
    source: &str,
    models: &mut Vec<String>,
    sources: &mut Vec<String>,
) {
    if let Some(model) = backend.ready_model.as_deref() {
        if is_gemma4_model(model) {
            models.push(model.to_string());
            sources.push(source.to_string());
        }
    }
    for model in &backend.available_models {
        if is_gemma4_model(model.as_str()) {
            models.push(model.clone());
            sources.push(source.to_string());
        }
    }
}

fn is_gemma4_model(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.contains("gemma4") || normalized.contains("gemma-4")
}

fn provider_note(loaded: &LoadedState, config_path: &Path) -> String {
    if !config_path.exists() {
        return format!(
            "No config file exists at {}. Initialize Pylon first.",
            config_path.display()
        );
    }
    let Some(snapshot) = loaded.snapshot() else {
        return "The headless serve loop has not published a provider snapshot yet.".to_string();
    };
    if loaded.state_label() == "unconfigured" {
        return "Pylon is unconfigured. Create config and identity before expecting backend truth."
            .to_string();
    }
    if snapshot.runtime.last_error.is_some() {
        return "Provider state is degraded. Check the backend lines above.".to_string();
    }
    "Backend summaries refresh live with the local system stats pane.".to_string()
}

fn backend_line(label: &str, health: &ProviderBackendHealth) -> String {
    let state = if health.ready {
        "ready"
    } else if health.reachable {
        "reachable"
    } else {
        "unreachable"
    };
    let model = health
        .ready_model
        .as_deref()
        .or(health.configured_model.as_deref())
        .unwrap_or("none");
    let mut line = format!("{label}: {state}  model {model}");
    if let Some(latency_ms_p50) = health.latency_ms_p50 {
        line.push_str(format!("  p50 {latency_ms_p50}ms").as_str());
    }
    if let Some(last_error) = health.last_error.as_deref() {
        line.push_str(format!("  error {last_error}").as_str());
    } else if let Some(message) = health.availability_message.as_deref() {
        line.push_str(format!("  note {message}").as_str());
    }
    line
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

fn format_bytes(value: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.1} GiB", value as f64 / GIB)
}

fn format_percent(value: f32) -> String {
    format!("{value:.0}%")
}

fn format_load_average((one, five, fifteen): (f64, f64, f64)) -> String {
    format!("{one:.2} / {five:.2} / {fifteen:.2}")
}
