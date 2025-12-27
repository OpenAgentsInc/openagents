//! Autopilot - OpenAgents autonomous agent with auth and preflight checking

use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Instant;
use std::io::{BufRead, BufReader};
use tracing::{info, warn, debug};
use wgpui::{
    Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size, TextSystem,
};
use wgpui::components::hud::{
    CornerConfig, DotsGrid, DotsOrigin, DotShape, DrawDirection, Frame, FrameAnimation,
};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use autopilot::{auth, PreflightConfig};

/// Shorten a path by replacing home directory with ~
fn shorten_path(path: &Path) -> String {
    let path_str = path.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if path_str.starts_with(&home) {
            return path_str.replacen(&home, "~", 1);
        }
    }
    path_str
}

fn main() {
    // Initialize logging with filter to suppress noisy external crates
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(
                        "autopilot=debug,openagents=debug,wgpui=info,cosmic_text=warn,wgpu=warn,info"
                    )
                })
        )
        .with_target(true)
        .init();

    info!("Starting Autopilot");

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

/// Log line for terminal display
#[derive(Clone)]
struct LogLine {
    text: String,
    timestamp: f32,
    status: LogStatus,
}

#[derive(Clone, Copy, PartialEq)]
enum LogStatus {
    Pending,
    Success,
    Error,
    Info,
    Thinking,
}

struct StartupState {
    lines: Vec<LogLine>,
    phase: StartupPhase,
    phase_started: f32,
    preflight_config: Option<PreflightConfig>,
    stream_receiver: Option<mpsc::Receiver<StreamToken>>,
    gpt_oss_buffer: String,
    issue_summary: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum StartupPhase {
    CheckingOpenCode,
    CheckingOpenAgents,
    CopyingAuth,
    AuthComplete,
    RunningPreflight,
    PreflightComplete,
    AnalyzingIssues,
    StreamingAnalysis,
    Complete,
}

enum StreamToken {
    Chunk(String),
    Done,
    Error(String),
}

impl StartupState {
    fn new() -> Self {
        Self {
            lines: vec![],
            phase: StartupPhase::CheckingOpenCode,
            phase_started: 0.0,
            preflight_config: None,
            stream_receiver: None,
            gpt_oss_buffer: String::new(),
            issue_summary: None,
        }
    }

    fn add_line(&mut self, text: &str, status: LogStatus, elapsed: f32) {
        self.lines.push(LogLine {
            text: text.to_string(),
            timestamp: elapsed,
            status,
        });
    }

    fn tick(&mut self, elapsed: f32) {
        let phase_time = elapsed - self.phase_started;

        match self.phase {
            StartupPhase::CheckingOpenCode => {
                if self.lines.is_empty() {
                    self.add_line("Checking OpenCode auth...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.5 {
                    let opencode_path = auth::opencode_auth_path();
                    let status = auth::check_opencode_auth();

                    if let Some(line) = self.lines.last_mut() {
                        line.status = LogStatus::Info;
                    }

                    match status {
                        auth::AuthStatus::Found { ref providers } => {
                            self.add_line(
                                &format!("  Found at {}", shorten_path(&opencode_path)),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.add_line(
                                &format!("  Providers: {}", providers.join(", ")),
                                LogStatus::Success,
                                elapsed,
                            );
                        }
                        auth::AuthStatus::NotFound => {
                            self.add_line(
                                &format!("  Not found at {}", shorten_path(&opencode_path)),
                                LogStatus::Error,
                                elapsed,
                            );
                        }
                        auth::AuthStatus::Error(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                        }
                        _ => {}
                    }

                    self.phase = StartupPhase::CheckingOpenAgents;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::CheckingOpenAgents => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("OpenAgents auth")) {
                    self.add_line("Checking OpenAgents auth...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.8 {
                    let openagents_path = auth::openagents_auth_path();
                    let status = auth::check_openagents_auth();

                    if let Some(line) = self.lines.last_mut() {
                        if line.status == LogStatus::Pending {
                            line.status = LogStatus::Info;
                        }
                    }

                    match status {
                        auth::AuthStatus::Found { ref providers } => {
                            self.add_line(
                                &format!("  Found at {}", shorten_path(&openagents_path)),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.add_line(
                                &format!("  Providers: {}", providers.join(", ")),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.phase = StartupPhase::AuthComplete;
                            self.phase_started = elapsed;
                        }
                        auth::AuthStatus::NotFound => {
                            self.add_line("  Not configured yet", LogStatus::Info, elapsed);
                            self.phase = StartupPhase::CopyingAuth;
                            self.phase_started = elapsed;
                        }
                        auth::AuthStatus::Error(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                            self.phase = StartupPhase::RunningPreflight;
                            self.phase_started = elapsed;
                        }
                        _ => {}
                    }
                }
            }

            StartupPhase::CopyingAuth => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("Copying")) {
                    self.add_line("Copying credentials from OpenCode...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.8 {
                    if let Some(line) = self.lines.last_mut() {
                        if line.status == LogStatus::Pending {
                            line.status = LogStatus::Info;
                        }
                    }

                    match auth::copy_opencode_auth() {
                        Ok(auth::AuthStatus::Copied { providers }) => {
                            self.add_line(
                                &format!("  Imported {} provider(s)", providers.len()),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.add_line(
                                &format!("  Saved to {}", shorten_path(&auth::openagents_auth_path())),
                                LogStatus::Success,
                                elapsed,
                            );
                        }
                        Ok(auth::AuthStatus::NotFound) => {
                            self.add_line("  No credentials to copy", LogStatus::Error, elapsed);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                        }
                    }

                    self.phase = StartupPhase::AuthComplete;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::AuthComplete => {
                if phase_time > 0.3 && !self.lines.iter().any(|l| l.text.contains("Auth ready") || l.text.contains("Anthropic auth not")) {
                    if auth::has_anthropic_auth() {
                        info!("Anthropic auth is ready");
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Auth ready.", LogStatus::Success, elapsed);
                    } else {
                        warn!("Anthropic auth not configured");
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Anthropic auth not configured.", LogStatus::Error, elapsed);
                        self.add_line("Run: opencode auth login", LogStatus::Info, elapsed);
                    }
                    self.phase = StartupPhase::RunningPreflight;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::RunningPreflight => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("Running preflight")) {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Running preflight checks...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.8 && self.preflight_config.is_none() {
                    if let Some(line) = self.lines.last_mut() {
                        if line.status == LogStatus::Pending {
                            line.status = LogStatus::Info;
                        }
                    }

                    let cwd = std::env::current_dir().unwrap_or_default();
                    debug!("Running preflight for {:?}", cwd);

                    match PreflightConfig::run(&cwd) {
                        Ok(config) => {
                            self.display_preflight_results(&config, elapsed);
                            self.preflight_config = Some(config);
                        }
                        Err(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                        }
                    }

                    self.phase = StartupPhase::PreflightComplete;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::PreflightComplete => {
                if phase_time > 0.3 {
                    let gpt_oss_available = self.preflight_config.as_ref()
                        .map(|c| c.inference.local_backends.iter().any(|b| b.name == "gpt-oss" && b.available))
                        .unwrap_or(false);

                    if gpt_oss_available && self.issue_summary.is_none() {
                        self.phase = StartupPhase::AnalyzingIssues;
                        self.phase_started = elapsed;
                    } else {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                        self.phase = StartupPhase::Complete;
                        self.phase_started = elapsed;
                    }
                }
            }

            StartupPhase::AnalyzingIssues => {
                if !self.lines.iter().any(|l| l.text.contains("Analyzing issues")) {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Analyzing issues with gpt-oss...", LogStatus::Pending, elapsed);

                    let cwd = std::env::current_dir().unwrap_or_default();
                    if let Some(summary) = query_issue_summary(&cwd) {
                        self.issue_summary = Some(summary.clone());
                        let (tx, rx) = mpsc::channel();
                        self.stream_receiver = Some(rx);
                        
                        std::thread::spawn(move || {
                            stream_gpt_oss_analysis(&summary, tx);
                        });

                        self.phase = StartupPhase::StreamingAnalysis;
                        self.phase_started = elapsed;
                    } else {
                        if let Some(line) = self.lines.last_mut() {
                            line.status = LogStatus::Info;
                        }
                        self.add_line("  No issues database found", LogStatus::Info, elapsed);
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                        self.phase = StartupPhase::Complete;
                        self.phase_started = elapsed;
                    }
                }
            }

            StartupPhase::StreamingAnalysis => {
                let mut tokens = Vec::new();
                if let Some(ref rx) = self.stream_receiver {
                    while let Ok(token) = rx.try_recv() {
                        tokens.push(token);
                    }
                }

                let mut done = false;
                for token in tokens {
                    match token {
                        StreamToken::Chunk(text) => {
                            self.gpt_oss_buffer.push_str(&text);
                            self.update_streaming_line(elapsed);
                        }
                        StreamToken::Done => {
                            self.finalize_streaming();
                            self.stream_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            done = true;
                            break;
                        }
                        StreamToken::Error(e) => {
                            self.add_line(&format!("  Error: {}", e), LogStatus::Error, elapsed);
                            self.stream_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            done = true;
                            break;
                        }
                    }
                }
                if done { return; }
            }

            StartupPhase::Complete => {}
        }
    }

    fn update_streaming_line(&mut self, elapsed: f32) {
        let segments = parse_harmony_stream(&self.gpt_oss_buffer);
        
        let start_idx = self.lines.iter().position(|l| l.text.contains("Analyzing issues"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());
        
        self.lines.truncate(start_idx);
        
        for segment in &segments {
            if segment.content.is_empty() {
                continue;
            }

            let is_thinking = segment.channel == "analysis" || segment.channel == "commentary";
            let status = if is_thinking { LogStatus::Thinking } else { LogStatus::Info };

            for line in segment.content.lines() {
                if !line.trim().is_empty() {
                    if is_thinking {
                        self.add_line(&format!("  > {}", line), status, elapsed);
                    } else {
                        self.add_line(&format!("  {}", line), status, elapsed);
                    }
                }
            }
        }
    }

    fn finalize_streaming(&mut self) {
        if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Analyzing issues")) {
            line.status = LogStatus::Success;
        }
    }

    fn display_preflight_results(&mut self, config: &PreflightConfig, elapsed: f32) {
        if let Some(ref git) = config.git {
            if let Some(ref branch) = git.branch {
                self.add_line(&format!("  Git: {} branch", branch), LogStatus::Success, elapsed);
            }
            if git.has_changes {
                self.add_line("  Git: has uncommitted changes", LogStatus::Info, elapsed);
            }
        }

        if let Some(ref project) = config.project {
            if project.has_directives {
                self.add_line(
                    &format!("  Project: {} directives", project.directive_count),
                    LogStatus::Success,
                    elapsed,
                );
            }
            if project.has_autopilot_db {
                self.add_line("  Project: autopilot.db found", LogStatus::Success, elapsed);
            }
        }

        self.add_line("", LogStatus::Info, elapsed);
        self.add_line("Inference backends:", LogStatus::Info, elapsed);

        for backend in &config.inference.local_backends {
            if backend.available {
                let models_str = if backend.models.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", backend.models.join(", "))
                };
                self.add_line(
                    &format!("  [OK] {}{}", backend.name, models_str),
                    LogStatus::Success,
                    elapsed,
                );
            } else {
                self.add_line(
                    &format!("  [--] {} (not running)", backend.name),
                    LogStatus::Info,
                    elapsed,
                );
            }
        }

        if !config.inference.cloud_providers.is_empty() {
            self.add_line(
                &format!("  Cloud: {}", config.inference.cloud_providers.join(", ")),
                LogStatus::Success,
                elapsed,
            );
        }

        self.add_line("", LogStatus::Info, elapsed);
        self.add_line("Tools:", LogStatus::Info, elapsed);

        if let Some(ref claude) = config.tools.claude {
            self.add_line(&format!("  [OK] claude: {}", shorten_path(&claude.path)), LogStatus::Success, elapsed);
        }
        if let Some(ref codex) = config.tools.codex {
            self.add_line(&format!("  [OK] codex: {}", shorten_path(&codex.path)), LogStatus::Success, elapsed);
        }
        if let Some(ref opencode) = config.tools.opencode {
            self.add_line(&format!("  [OK] opencode: {}", shorten_path(&opencode.path)), LogStatus::Success, elapsed);
        }
    }
}

fn query_issue_summary(cwd: &Path) -> Option<String> {
    let db_path = cwd.join(".openagents/autopilot.db");
    if !db_path.exists() {
        return None;
    }

    let output = std::process::Command::new("sqlite3")
        .arg(&db_path)
        .arg("SELECT status, COUNT(*) FROM issues GROUP BY status; SELECT '---'; SELECT number, substr(title,1,50), status, priority FROM issues WHERE status != 'done' ORDER BY CASE priority WHEN 'urgent' THEN 1 WHEN 'high' THEN 2 WHEN 'medium' THEN 3 ELSE 4 END LIMIT 10;")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    
    let mut done = 0;
    let mut in_progress = 0;
    let mut open = 0;
    let mut active_issues = Vec::new();
    let mut in_active = false;

    for line in raw.lines() {
        if line == "---" {
            in_active = true;
            continue;
        }
        if !in_active {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                let count: i32 = parts[1].parse().unwrap_or(0);
                match parts[0] {
                    "done" => done = count,
                    "in_progress" => in_progress = count,
                    "open" => open = count,
                    _ => {}
                }
            }
        } else {
            active_issues.push(line.to_string());
        }
    }

    let total = done + in_progress + open;
    if total == 0 {
        return None;
    }

    let mut summary = format!(
        "Issue Status: {} total, {} done ({}%), {} in-progress, {} open\n\nActive issues:\n",
        total, done, (done * 100) / total, in_progress, open
    );

    for issue in active_issues {
        summary.push_str(&format!("- {}\n", issue));
    }

    summary.push_str("\nProvide brief analysis: health, top priority, risks.");
    Some(summary)
}

fn stream_gpt_oss_analysis(summary: &str, tx: mpsc::Sender<StreamToken>) {
    use std::io::Write;
    use std::net::TcpStream;

    let request_body = serde_json::json!({
        "model": "gpt-oss-120b-mxfp4.gguf",
        "messages": [
            {"role": "system", "content": "You are a concise project analyst. Give brief insights in 3-4 sentences."},
            {"role": "user", "content": summary}
        ],
        "max_tokens": 300,
        "temperature": 0.3,
        "stream": true
    });

    let body = request_body.to_string();
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: localhost:8000\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );

    let stream = match TcpStream::connect("localhost:8000") {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(StreamToken::Error(e.to_string()));
            return;
        }
    };

    let mut stream_clone = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(StreamToken::Error(e.to_string()));
            return;
        }
    };

    if let Err(e) = stream_clone.write_all(request.as_bytes()) {
        let _ = tx.send(StreamToken::Error(e.to_string()));
        return;
    }

    let reader = BufReader::new(stream);
    let mut headers_done = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if !headers_done {
            if line.is_empty() {
                headers_done = true;
            }
            continue;
        }

        if line.starts_with("data: ") {
            let data = &line[6..];
            if data == "[DONE]" {
                let _ = tx.send(StreamToken::Done);
                return;
            }

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                    let _ = tx.send(StreamToken::Chunk(content.to_string()));
                }
            }
        }
    }

    let _ = tx.send(StreamToken::Done);
}

struct HarmonySegment {
    channel: String,
    content: String,
}

fn parse_harmony_stream(text: &str) -> Vec<HarmonySegment> {
    let mut segments = Vec::new();
    let mut current_channel = String::new();
    let mut current_content = String::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if let Some(channel_start) = remaining.find("<|channel|>") {
            if !current_content.is_empty() && !current_channel.is_empty() {
                segments.push(HarmonySegment {
                    channel: current_channel.clone(),
                    content: current_content.trim().to_string(),
                });
                current_content.clear();
            }

            let after_channel = &remaining[channel_start + 11..];
            
            if let Some(msg_start) = after_channel.find("<|message|>") {
                current_channel = after_channel[..msg_start].to_string();
                remaining = &after_channel[msg_start + 11..];
            } else {
                let end = after_channel.find("<|").unwrap_or(after_channel.len());
                current_channel = after_channel[..end].to_string();
                remaining = &after_channel[end..];
            }
        } else if let Some(end_pos) = remaining.find("<|end|>") {
            current_content.push_str(&remaining[..end_pos]);
            if !current_content.is_empty() {
                segments.push(HarmonySegment {
                    channel: current_channel.clone(),
                    content: current_content.trim().to_string(),
                });
                current_content.clear();
            }
            remaining = &remaining[end_pos + 7..];
        } else if let Some(tag_start) = remaining.find("<|") {
            current_content.push_str(&remaining[..tag_start]);
            if let Some(tag_end) = remaining[tag_start..].find("|>") {
                remaining = &remaining[tag_start + tag_end + 2..];
            } else {
                break;
            }
        } else {
            current_content.push_str(remaining);
            break;
        }
    }

    if !current_content.is_empty() {
        segments.push(HarmonySegment {
            channel: if current_channel.is_empty() { "final".to_string() } else { current_channel },
            content: current_content.trim().to_string(),
        });
    }

    segments
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    start_time: Instant,
    startup_state: StartupState,
    scroll_offset: f32,
    auto_scroll: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Autopilot")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 1000));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                start_time: Instant::now(),
                startup_state: StartupState::new(),
                scroll_offset: 0.0,
                auto_scroll: true,
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Handle scroll wheel
                let scroll_amount = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                // User is scrolling - disable auto-scroll
                if scroll_amount.abs() > 0.1 {
                    state.auto_scroll = false;
                }

                // Update scroll offset (inverted for natural scrolling)
                state.scroll_offset = (state.scroll_offset - scroll_amount).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let width = state.config.width as f32;
                let height = state.config.height as f32;

                let elapsed = state.start_time.elapsed().as_secs_f32();
                let dots_progress = ease_out_cubic((elapsed / 1.5).min(1.0));
                let frame_progress = ease_out_cubic(((elapsed - 0.8) / 1.0).clamp(0.0, 1.0));

                if frame_progress > 0.7 {
                    let startup_elapsed = elapsed - 1.8;
                    if startup_elapsed > 0.0 {
                        state.startup_state.tick(startup_elapsed);
                    }
                }

                let mut scene = Scene::new();
                let (max_scroll, _) = render(
                    &mut scene,
                    &mut state.text_system,
                    width,
                    height,
                    dots_progress,
                    frame_progress,
                    &state.startup_state,
                    state.scroll_offset,
                    state.auto_scroll,
                );

                // Clamp scroll offset to valid range
                state.scroll_offset = state.scroll_offset.min(max_scroll).max(0.0);

                // Re-enable auto-scroll if we're at the bottom
                if state.scroll_offset >= max_scroll - 1.0 {
                    state.auto_scroll = true;
                }

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

                state.renderer.resize(&state.queue, Size::new(width, height), 1.0);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                state.renderer.prepare(&state.device, &scene);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    if text.chars().count() <= max_chars {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let indent = "  ";

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let current_len = current_line.chars().count();
        let is_continuation = !lines.is_empty();
        let effective_max = if is_continuation { max_chars - 2 } else { max_chars };

        if word_len > effective_max {
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }

            let mut chars = word.chars().peekable();
            while chars.peek().is_some() {
                let is_cont = !lines.is_empty();
                let max = if is_cont { max_chars - 2 } else { max_chars };
                let chunk: String = chars.by_ref().take(max).collect();
                if is_cont {
                    lines.push(format!("{}{}", indent, chunk));
                } else {
                    lines.push(chunk);
                }
            }
            continue;
        }

        let space_needed = if current_line.is_empty() { 0 } else { 1 };
        if current_len + space_needed + word_len <= effective_max {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        } else {
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        if lines.is_empty() {
            lines.push(current_line);
        } else {
            lines.push(format!("{}{}", indent, current_line));
        }
    }

    for i in 1..lines.len() {
        if !lines[i].starts_with(indent) {
            lines[i] = format!("{}{}", indent, lines[i]);
        }
    }

    lines
}

fn render(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    width: f32,
    height: f32,
    dots_progress: f32,
    frame_progress: f32,
    startup_state: &StartupState,
    scroll_offset: f32,
    auto_scroll: bool,
) -> (f32, f32) {
    // Black background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height))
            .with_background(Hsla::new(0.0, 0.0, 0.0, 1.0)),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);

    // DotsGrid background
    let mut dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 0.25, 0.2))
        .shape(DotShape::Circle)
        .distance(48.0)
        .size(2.0)
        .origin(DotsOrigin::Center)
        .easing(Easing::EaseOut)
        .animation_progress(dots_progress);

    dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);

    // Center frame (1000x600)
    if frame_progress > 0.0 {
        let frame_w = 1000.0;
        let frame_h = 600.0;
        let frame_x = (width - frame_w) / 2.0;
        let frame_y = (height - frame_h) / 2.0;

        let line_color = Hsla::new(0.0, 0.0, 0.7, frame_progress);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.95 * frame_progress);
        let glow_color = Hsla::new(180.0, 0.6, 0.5, 0.3 * frame_progress);

        let mut frame = Frame::nefrex()
            .line_color(line_color)
            .bg_color(bg_color)
            .glow_color(glow_color)
            .stroke_width(1.5)
            .corner_config(CornerConfig::all())
            .square_size(10.0)
            .small_line_length(10.0)
            .large_line_length(40.0)
            .animation_mode(FrameAnimation::Assemble)
            .draw_direction(DrawDirection::CenterOut)
            .animation_progress(frame_progress);

        frame.paint(Bounds::new(frame_x, frame_y, frame_w, frame_h), &mut cx);

        // Terminal log lines inside frame
        if frame_progress > 0.5 {
            let text_alpha = ((frame_progress - 0.5) * 2.0).min(1.0);
            let line_height = 22.0;
            let font_size = 12.0;
            let padding = 16.0;
            let text_area_x = frame_x + padding;
            let text_area_y = frame_y + padding;
            let text_area_w = frame_w - padding * 2.0;
            let text_area_h = frame_h - padding * 2.0;

            let char_width = 7.2;
            let max_chars = (text_area_w / char_width) as usize;
            let max_visible_lines = (text_area_h / line_height) as usize;

            struct WrappedLine {
                text: String,
                color: Hsla,
            }

            let mut all_wrapped: Vec<WrappedLine> = Vec::new();

            for log_line in &startup_state.lines {
                let color = match log_line.status {
                    LogStatus::Pending => Hsla::new(45.0, 0.9, 0.65, text_alpha),
                    LogStatus::Success => Hsla::new(120.0, 0.7, 0.6, text_alpha),
                    LogStatus::Error => Hsla::new(0.0, 0.8, 0.6, text_alpha),
                    LogStatus::Info => Hsla::new(0.0, 0.0, 0.7, text_alpha),
                    LogStatus::Thinking => Hsla::new(270.0, 0.5, 0.6, text_alpha * 0.7),
                };

                let prefix = match log_line.status {
                    LogStatus::Pending => "> ",
                    _ => "  ",
                };

                let full_text = format!("{}{}", prefix, log_line.text);
                let wrapped = wrap_text(&full_text, max_chars);

                for line in wrapped {
                    all_wrapped.push(WrappedLine { text: line, color });
                }
            }

            let total_visual_lines = all_wrapped.len();
            let content_height = total_visual_lines as f32 * line_height;
            let max_scroll = (content_height - text_area_h).max(0.0);

            let start_idx = if auto_scroll {
                total_visual_lines.saturating_sub(max_visible_lines)
            } else {
                let scroll_lines = (scroll_offset / line_height) as usize;
                scroll_lines.min(total_visual_lines.saturating_sub(max_visible_lines))
            };

            for (i, wrapped_line) in all_wrapped.iter().skip(start_idx).take(max_visible_lines + 1).enumerate() {
                let y = text_area_y + (i as f32 * line_height);

                if y > frame_y + frame_h - padding {
                    break;
                }

                let text_run = cx.text.layout(&wrapped_line.text, Point::new(text_area_x, y), font_size, wrapped_line.color);
                cx.scene.draw_text(text_run);
            }

            return (max_scroll, content_height);
        }
    }

    (0.0, 0.0)
}
