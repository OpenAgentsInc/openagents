use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use openagents_provider_substrate::{
    ProviderBackendHealth, ProviderDesiredMode, ProviderMode, ProviderPersistedSnapshot,
    ProviderStatusResponse, provider_runtime_state_label,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

const TICK_RATE: Duration = Duration::from_millis(200);
const REFRESH_RATE: Duration = Duration::from_secs(2);

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

#[derive(Clone, Debug)]
struct AppShell {
    config_path: PathBuf,
    loaded: Option<LoadedState>,
    last_refresh_at: Option<Instant>,
    last_error: Option<String>,
    next_refresh_at: Instant,
    should_quit: bool,
}

impl AppShell {
    fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            loaded: None,
            last_refresh_at: None,
            last_error: None,
            next_refresh_at: Instant::now(),
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
        match pylon::load_config_and_status(self.config_path.as_path()).await {
            Ok((config, status)) => {
                self.loaded = Some(LoadedState { config, status });
                self.last_error = None;
                self.last_refresh_at = Some(Instant::now());
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
                self.last_refresh_at = Some(Instant::now());
            }
        }
        self.next_refresh_at = Instant::now() + REFRESH_RATE;
    }

    fn render(&self, frame: &mut Frame<'_>) {
        let shell = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .title(Line::from(vec![
                Span::styled(" Pylon ", shell_accent()),
                Span::styled(" local operator shell ", shell_border()),
            ]))
            .style(shell_border());
        let area = frame.area();
        let inner = shell.inner(area);
        frame.render_widget(shell, area);

        let vertical = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(inner);
        let columns = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(vertical[1]);
        let right = Layout::vertical([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(columns[1]);

        frame.render_widget(self.header_panel(), vertical[0]);
        frame.render_widget(self.node_panel(), columns[0]);
        frame.render_widget(self.stats_panel(), right[0]);
        frame.render_widget(self.notes_panel(), right[1]);
        frame.render_widget(self.footer_panel(), vertical[2]);
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

    fn node_panel(&self) -> Paragraph<'static> {
        panel("Node", Text::from(self.node_lines()))
    }

    fn stats_panel(&self) -> Paragraph<'static> {
        panel("Stats", Text::from(self.stats_lines()))
    }

    fn notes_panel(&self) -> Paragraph<'static> {
        panel("Backends", Text::from(self.notes_lines()))
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

    fn node_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(format!(
            "config path: {}",
            self.config_path.display()
        ))];
        if let Some(loaded) = self.loaded.as_ref() {
            lines.push(Line::from(format!(
                "node label: {}",
                loaded.config.node_label
            )));
            lines.push(Line::from(format!(
                "config file: {}",
                yes_no(self.config_path.exists())
            )));
            lines.push(Line::from(format!(
                "identity file: {}",
                yes_no(loaded.config.identity_path.exists())
            )));
            lines.push(Line::from(format!("state: {}", loaded.state_label())));
            lines.push(Line::from(format!(
                "desired mode: {}",
                loaded.status.desired_mode.label()
            )));
            lines.push(Line::from(format!(
                "admin listen: {}",
                loaded
                    .status
                    .listen_addr
                    .as_deref()
                    .unwrap_or(loaded.config.admin_listen_addr.as_str())
            )));
            lines.push(Line::from(format!(
                "execution backend: {}",
                loaded
                    .snapshot()
                    .map(|snapshot| snapshot.runtime.execution_backend_label.as_str())
                    .unwrap_or("unknown")
            )));
            lines.push(Line::from(format!(
                "snapshot age: {}",
                loaded
                    .snapshot()
                    .map(|snapshot| age_label(snapshot.captured_at_ms))
                    .unwrap_or_else(|| "none".to_string())
            )));
            if let Some(snapshot) = loaded.snapshot() {
                if let Some(reason_code) = snapshot.runtime.degraded_reason_code.as_deref() {
                    lines.push(Line::from(format!("reason code: {reason_code}")));
                }
                if let Some(last_error) = snapshot.runtime.last_error.as_deref() {
                    lines.push(Line::from(format!("runtime error: {last_error}")));
                }
                if !snapshot.runtime.provider_blocker_codes.is_empty() {
                    lines.push(Line::from(format!(
                        "blockers: {}",
                        snapshot.runtime.provider_blocker_codes.join(", ")
                    )));
                }
            }
            if let Some(destination) = loaded.config.payout_destination.as_deref() {
                lines.push(Line::from(format!("payout destination: {destination}")));
            }
        } else {
            lines.push(Line::from("provider state unavailable"));
        }
        if let Some(error) = self.last_error.as_deref() {
            lines.push(Line::from(""));
            lines.push(Line::from(format!("last refresh error: {error}")));
        }
        lines
    }

    fn stats_lines(&self) -> Vec<Line<'static>> {
        let Some(loaded) = self.loaded.as_ref() else {
            return vec![Line::from("waiting for provider status")];
        };
        let Some(snapshot) = loaded.snapshot() else {
            return vec![Line::from("no persisted provider snapshot yet")];
        };
        let eligible_products = snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.eligible)
            .count();
        let backend_ready_products = snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.backend_ready)
            .count();
        let available_rows = snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.available_quantity > 0)
            .count();
        let mut lines = vec![
            Line::from(format!(
                "visible products: {}",
                snapshot.inventory_rows.len()
            )),
            Line::from(format!("eligible products: {eligible_products}")),
            Line::from(format!("backend-ready products: {backend_ready_products}")),
            Line::from(format!("active inventory rows: {available_rows}")),
            Line::from(format!("recent jobs: {}", snapshot.recent_jobs.len())),
            Line::from(format!("receipts: {}", snapshot.receipts.len())),
            Line::from(format!("payout rows: {}", snapshot.payouts.len())),
            Line::from(format!("health events: {}", snapshot.health_events.len())),
            Line::from(format!("queue depth: {}", snapshot.runtime.queue_depth)),
            Line::from(format!(
                "online uptime: {}",
                format_duration(Duration::from_secs(snapshot.runtime.online_uptime_seconds))
            )),
        ];
        if let Some(earnings) = snapshot.earnings.as_ref() {
            lines.push(Line::from(format!("sats today: {}", earnings.sats_today)));
            lines.push(Line::from(format!(
                "lifetime sats: {}",
                earnings.lifetime_sats
            )));
            lines.push(Line::from(format!("jobs today: {}", earnings.jobs_today)));
            lines.push(Line::from(format!(
                "last job result: {}",
                earnings.last_job_result
            )));
        }
        lines
    }

    fn notes_lines(&self) -> Vec<Line<'static>> {
        let Some(loaded) = self.loaded.as_ref() else {
            return vec![Line::from("No provider status yet.")];
        };
        let mut lines = vec![Line::from(operator_note(
            loaded,
            self.config_path.as_path(),
        ))];
        if let Some(snapshot) = loaded.snapshot() {
            lines.push(Line::from(""));
            lines.extend(backend_lines(snapshot));
        }
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

fn operator_note(loaded: &LoadedState, config_path: &Path) -> String {
    if !config_path.exists() {
        return format!(
            "No config file exists at {}. Initialize the headless Pylon CLI to create config and identity.",
            config_path.display()
        );
    }
    let Some(snapshot) = loaded.snapshot() else {
        return "The node has no persisted provider snapshot yet. Start the headless serve loop to publish state.".to_string();
    };
    let eligible_products = snapshot
        .inventory_rows
        .iter()
        .filter(|row| row.eligible)
        .count();
    if snapshot.inventory_rows.is_empty() {
        return "No compute products are visible yet. Backend detection is still unconfigured or offline.".to_string();
    }
    if eligible_products == 0 {
        return "Products are visible, but none are eligible to sell yet.".to_string();
    }
    if loaded.status.desired_mode != ProviderDesiredMode::Online {
        return format!(
            "{eligible_products} eligible products are visible, but desired mode is {}.",
            loaded.status.desired_mode.label()
        );
    }
    if snapshot.runtime.mode != ProviderMode::Online {
        return "Desired mode is online, but the headless serve loop is not currently reporting an online runtime.".to_string();
    }
    format!(
        "{eligible_products} eligible products are visible. The headless serve loop can advertise this supply."
    )
}

fn backend_lines(snapshot: &ProviderPersistedSnapshot) -> Vec<Line<'static>> {
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
        lines.push(Line::from(format!(
            "pooled inference: members {}  warm replicas {}  local state {}",
            snapshot.availability.pooled_inference.member_count,
            snapshot.availability.pooled_inference.warm_replica_count,
            snapshot.availability.pooled_inference.local_serving_state
        )));
    }
    let declared_classes = snapshot.availability.sandbox.declared_execution_classes();
    let ready_classes = snapshot.availability.sandbox.ready_execution_classes();
    if !declared_classes.is_empty() || !snapshot.availability.sandbox.runtimes.is_empty() {
        let declared = declared_classes
            .iter()
            .map(|class| class.product_id().to_string())
            .collect::<Vec<_>>();
        let ready = ready_classes
            .iter()
            .map(|class| class.product_id().to_string())
            .collect::<Vec<_>>();
        lines.push(Line::from(format!(
            "sandbox: runtimes {}  declared {}  ready {}",
            snapshot.availability.sandbox.runtimes.len(),
            if declared.is_empty() {
                "none".to_string()
            } else {
                declared.join(", ")
            },
            if ready.is_empty() {
                "none".to_string()
            } else {
                ready.join(", ")
            }
        )));
    }
    if let Some(last_error) = snapshot.availability.sandbox.last_scan_error.as_deref() {
        lines.push(Line::from(format!("sandbox scan error: {last_error}")));
    }
    lines
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
    if let Some(message) = health.availability_message.as_deref() {
        line.push_str(format!("  note {message}").as_str());
    } else if let Some(last_error) = health.last_error.as_deref() {
        line.push_str(format!("  error {last_error}").as_str());
    }
    line
}

fn age_label(captured_at_ms: i64) -> String {
    if captured_at_ms <= 0 {
        return "none".to_string();
    }
    let now_ms = now_epoch_ms();
    let delta_ms = now_ms.saturating_sub(captured_at_ms);
    format_duration(Duration::from_millis(delta_ms as u64))
}

fn now_epoch_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
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

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
