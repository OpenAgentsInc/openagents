use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;
use chrono::Local;
use tracing::{info, warn, debug};

use crate::auth;
use crate::claude::{ClaudeToken, ClaudeEvent, run_claude_planning, run_claude_execution, run_claude_review};
use crate::logger::{SessionLogger, generate_session_id};
use crate::preflight::PreflightConfig;
use crate::report::{AfterActionReport, collect_session_stats, generate_suggested_next_steps, generate_questions_for_user};
use crate::streaming::{StreamToken, query_issue_summary, stream_gpt_oss_analysis, parse_harmony_stream, extract_final_content};
use crate::utils::shorten_path;
use crate::verification::{TerminationChecklist, VerificationRunner, generate_fix_prompt, should_force_stop};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ClaudeModel {
    #[default]
    Sonnet,
    Opus,
}

impl ClaudeModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaudeModel::Sonnet => "claude-sonnet-4-5-20250929",
            ClaudeModel::Opus => "claude-opus-4-5-20250929",
        }
    }
}

fn extract_tool_display(name: &str, params: &str) -> String {
    let truncated = if params.len() > 60 {
        format!("{}...", &params[..57])
    } else {
        params.to_string()
    };
    
    match name {
        "Read" | "read" | "Edit" | "edit" | "Write" | "write" => {
            shorten_path(&PathBuf::from(&truncated))
        }
        _ => truncated,
    }
}

#[derive(Clone)]
pub struct LogLine {
    pub text: String,
    #[allow(dead_code)]
    pub timestamp: f32,
    pub status: LogStatus,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LogStatus {
    Pending,
    Success,
    Error,
    Info,
    Thinking,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum StartupPhase {
    CheckingOpenCode,
    CheckingOpenAgents,
    CopyingAuth,
    AuthComplete,
    RunningPreflight,
    PreflightComplete,
    // Pylon integration phases
    CheckingPylon,
    StartingPylon,
    DetectingCompute,
    ComputeMixReady,
    // Continue with existing phases
    AnalyzingIssues,
    StreamingAnalysis,
    PlanningWithClaude,
    StreamingClaudePlan,
    WritingPlan,
    ExecutingPlan,
    StreamingExecution,
    ReviewingWork,
    StreamingReview,
    VerifyingCompletion,
    FixingVerificationFailures,
    StreamingFix,
    GeneratingReport,
    Complete,
}

pub struct StartupState {
    pub lines: Vec<LogLine>,
    pub phase: StartupPhase,
    pub phase_started: f32,
    pub preflight_config: Option<PreflightConfig>,
    pub model: ClaudeModel,
    stream_receiver: Option<mpsc::Receiver<StreamToken>>,
    gpt_oss_buffer: String,
    issue_summary: Option<String>,
    gpt_oss_assessment: Option<String>,
    claude_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub claude_events: Vec<ClaudeEvent>,
    pub claude_full_text: String,
    pub plan_path: Option<PathBuf>,
    exec_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub exec_events: Vec<ClaudeEvent>,
    pub exec_full_text: String,
    review_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub review_events: Vec<ClaudeEvent>,
    pub review_full_text: String,
    pub iteration: u32,
    pub session_logger: Option<SessionLogger>,
    pub session_id: String,
    pub start_time: chrono::DateTime<Local>,
    pub start_instant: Instant,
    verification_runner: Option<VerificationRunner>,
    pub last_checklist: Option<TerminationChecklist>,
    fix_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub fix_events: Vec<ClaudeEvent>,
    pub fix_full_text: String,
    pub force_stopped: bool,
    pub force_stop_reason: Option<String>,
    pub report_path: Option<PathBuf>,
    // Pylon integration
    pub compute_mix: Option<crate::preflight::ComputeMix>,
    pylon_started: bool,
}

impl StartupState {
    pub fn new() -> Self {
        Self::with_model(ClaudeModel::default())
    }
    
    pub fn with_model(model: ClaudeModel) -> Self {
        let session_id = generate_session_id();
        let session_logger = SessionLogger::new(&session_id).ok();
        let start_time = Local::now();
        let start_instant = Instant::now();
        
        Self {
            lines: vec![],
            phase: StartupPhase::CheckingOpenCode,
            phase_started: 0.0,
            preflight_config: None,
            model,
            stream_receiver: None,
            gpt_oss_buffer: String::new(),
            issue_summary: None,
            gpt_oss_assessment: None,
            claude_receiver: None,
            claude_events: Vec::new(),
            claude_full_text: String::new(),
            plan_path: None,
            exec_receiver: None,
            exec_events: Vec::new(),
            exec_full_text: String::new(),
            review_receiver: None,
            review_events: Vec::new(),
            review_full_text: String::new(),
            iteration: 1,
            session_logger,
            session_id,
            start_time,
            start_instant,
            verification_runner: None,
            last_checklist: None,
            fix_receiver: None,
            fix_events: Vec::new(),
            fix_full_text: String::new(),
            force_stopped: false,
            force_stop_reason: None,
            report_path: None,
            compute_mix: None,
            pylon_started: false,
        }
    }

    pub fn add_line(&mut self, text: &str, status: LogStatus, elapsed: f32) {
        self.lines.push(LogLine {
            text: text.to_string(),
            timestamp: elapsed,
            status,
        });
    }

    pub fn tick(&mut self, elapsed: f32) {
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
                    // Transition to pylon integration phases
                    self.phase = StartupPhase::CheckingPylon;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::CheckingPylon => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("Checking local pylon")) {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Checking local pylon...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.6 {
                    if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Checking local pylon")) {
                        line.status = LogStatus::Info;
                    }

                    if crate::pylon_integration::check_pylon_running() {
                        self.add_line("  Pylon daemon is running", LogStatus::Success, elapsed);

                        // Get detailed status if available
                        if let Some(info) = crate::pylon_integration::get_pylon_status() {
                            if let Some(uptime) = info.uptime_secs {
                                let hours = uptime / 3600;
                                let mins = (uptime % 3600) / 60;
                                if hours > 0 {
                                    self.add_line(&format!("  Uptime: {}h {}m", hours, mins), LogStatus::Info, elapsed);
                                } else {
                                    self.add_line(&format!("  Uptime: {}m", mins), LogStatus::Info, elapsed);
                                }
                            }
                            if info.jobs_completed > 0 {
                                self.add_line(&format!("  Jobs completed: {}", info.jobs_completed), LogStatus::Info, elapsed);
                            }
                        }

                        self.phase = StartupPhase::DetectingCompute;
                    } else {
                        self.add_line("  Pylon not running", LogStatus::Info, elapsed);
                        self.phase = StartupPhase::StartingPylon;
                    }
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StartingPylon => {
                if !self.lines.iter().any(|l| l.text.contains("Starting pylon")) {
                    self.add_line("Starting pylon daemon...", LogStatus::Pending, elapsed);

                    // Start pylon in background
                    match crate::pylon_integration::start_pylon() {
                        Ok(()) => {
                            self.pylon_started = true;
                        }
                        Err(e) => {
                            warn!("Failed to start pylon: {}", e);
                        }
                    }
                }

                if phase_time > 2.0 {
                    if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Starting pylon")) {
                        line.status = LogStatus::Info;
                    }

                    if crate::pylon_integration::check_pylon_running() {
                        self.add_line("  Pylon started successfully", LogStatus::Success, elapsed);
                    } else {
                        self.add_line("  Pylon not started (continuing anyway)", LogStatus::Info, elapsed);
                    }

                    self.phase = StartupPhase::DetectingCompute;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::DetectingCompute => {
                if phase_time < 0.3 {
                    return;
                }

                if !self.lines.iter().any(|l| l.text.contains("Detecting compute backends")) {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Detecting compute backends...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.8 {
                    if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Detecting compute backends")) {
                        line.status = LogStatus::Info;
                    }

                    // Detect local backends
                    let backends = crate::pylon_integration::detect_local_backends();
                    let mut available_backends = Vec::new();

                    for backend in &backends {
                        if backend.available {
                            let models_str = if backend.models.is_empty() {
                                String::new()
                            } else {
                                format!(" - {}", backend.models.join(", "))
                            };
                            self.add_line(
                                &format!("  [OK] {} ({}){}", backend.name, backend.endpoint.as_deref().unwrap_or(""), models_str),
                                LogStatus::Success,
                                elapsed,
                            );
                            available_backends.push(backend.clone());
                        } else {
                            self.add_line(
                                &format!("  [--] {} (not running)", backend.name),
                                LogStatus::Info,
                                elapsed,
                            );
                        }
                    }

                    // Discover swarm providers (synchronous for now)
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Querying NIP-89 swarm providers...", LogStatus::Pending, elapsed);
                    let swarm_providers = crate::pylon_integration::discover_swarm_providers();

                    if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Querying NIP-89")) {
                        line.status = LogStatus::Info;
                    }

                    if swarm_providers.is_empty() {
                        self.add_line("  No remote providers discovered", LogStatus::Info, elapsed);
                    } else {
                        self.add_line(&format!("  Found {} remote provider(s)", swarm_providers.len()), LogStatus::Success, elapsed);
                    }

                    // Get cloud providers from preflight
                    let cloud_providers = self.preflight_config
                        .as_ref()
                        .map(|c| c.inference.cloud_providers.clone())
                        .unwrap_or_default();

                    // Get pylon info
                    let pylon_info = crate::pylon_integration::get_pylon_status();

                    // Build compute mix
                    self.compute_mix = Some(crate::preflight::ComputeMix {
                        pylon: pylon_info,
                        local_backends: backends,
                        cloud_providers: cloud_providers.clone(),
                        swarm_providers,
                    });

                    self.phase = StartupPhase::ComputeMixReady;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::ComputeMixReady => {
                if phase_time > 0.3 {
                    // Display compute mix summary
                    // Clone the data we need to avoid borrow issues
                    let mix_summary = self.compute_mix.as_ref().map(|mix| {
                        let local_names: Vec<_> = mix.local_backends
                            .iter()
                            .filter(|b| b.available)
                            .map(|b| {
                                if b.models.is_empty() {
                                    b.name.clone()
                                } else {
                                    format!("{} ({})", b.name, b.models.first().unwrap_or(&String::new()))
                                }
                            })
                            .collect();
                        let cloud = mix.cloud_providers.clone();
                        let swarm_count = mix.swarm_providers.len();
                        (local_names, cloud, swarm_count)
                    });

                    if let Some((local_names, cloud_providers, swarm_count)) = mix_summary {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Compute mix:", LogStatus::Info, elapsed);

                        // Local summary
                        if !local_names.is_empty() {
                            self.add_line(&format!("  Local: {}", local_names.join(", ")), LogStatus::Success, elapsed);
                        }

                        // Cloud summary
                        if !cloud_providers.is_empty() {
                            self.add_line(&format!("  Cloud: {}", cloud_providers.join(", ")), LogStatus::Success, elapsed);
                        }

                        // Swarm summary
                        if swarm_count > 0 {
                            self.add_line(&format!("  Swarm: {} providers via NIP-89", swarm_count), LogStatus::Success, elapsed);
                        }
                    }

                    // Continue to Claude phases if auth available
                    if auth::has_anthropic_auth() {
                        self.phase = StartupPhase::PlanningWithClaude;
                        self.phase_started = elapsed;
                    } else {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Claude auth not available.", LogStatus::Info, elapsed);
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
                            
                            let assessment = extract_final_content(&self.gpt_oss_buffer);
                            self.gpt_oss_assessment = Some(assessment);
                            
                            if auth::has_anthropic_auth() {
                                self.phase = StartupPhase::PlanningWithClaude;
                            } else {
                                self.add_line("", LogStatus::Info, elapsed);
                                self.add_line("Claude auth not available - skipping planning.", LogStatus::Info, elapsed);
                                self.add_line("", LogStatus::Info, elapsed);
                                self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                                self.phase = StartupPhase::Complete;
                            }
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

            StartupPhase::PlanningWithClaude => {
                if !self.lines.iter().any(|l| l.text.contains("Creating plan with Claude")) {
                    self.add_line("", LogStatus::Info, elapsed);
                    let iteration = self.iteration;
                    if iteration == 1 {
                        self.add_line("Creating plan with Claude...", LogStatus::Pending, elapsed);
                    } else {
                        self.add_line(&format!("Creating plan (iteration {}) with Claude...", iteration), LogStatus::Pending, elapsed);
                    }

                    let assessment = self.gpt_oss_assessment.clone().unwrap_or_default();
                    let issue_summary = self.issue_summary.clone().unwrap_or_default();
                    let cwd = std::env::current_dir().unwrap_or_default();
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    
                    let (tx, rx) = mpsc::channel();
                    self.claude_receiver = Some(rx);

                    std::thread::spawn(move || {
                        run_claude_planning(&cwd, &issue_summary, &assessment, model, tx, logger);
                    });

                    self.phase = StartupPhase::StreamingClaudePlan;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingClaudePlan => {
                let mut tokens = Vec::new();
                if let Some(ref rx) = self.claude_receiver {
                    while let Ok(token) = rx.try_recv() {
                        tokens.push(token);
                    }
                }

                for token in tokens {
                    match token {
                        ClaudeToken::Chunk(text) => {
                            self.claude_full_text.push_str(&text);
                            if let Some(ClaudeEvent::Text(s)) = self.claude_events.last_mut() {
                                s.push_str(&text);
                            } else {
                                self.claude_events.push(ClaudeEvent::Text(text));
                            }
                            self.update_claude_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            self.claude_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                            });
                            self.update_claude_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone { name } => {
                            for event in self.claude_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            self.update_claude_streaming_line(elapsed);
                        }
                        ClaudeToken::Done(plan) => {
                            self.claude_receiver = None;
                            if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Creating plan with Claude")) {
                                line.status = LogStatus::Success;
                            }
                            
                            let now = Local::now();
                            let date_dir = now.format("%Y%m%d").to_string();
                            let time_slug = now.format("%H%M%S").to_string();
                            
                            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                            let plans_dir = PathBuf::from(&home).join(".openagents/plans").join(&date_dir);
                            
                            if let Err(e) = std::fs::create_dir_all(&plans_dir) {
                                self.add_line(&format!("  Error creating dir: {}", e), LogStatus::Error, elapsed);
                                self.phase = StartupPhase::Complete;
                                self.phase_started = elapsed;
                                return;
                            }
                            
                            let plan_file = plans_dir.join(format!("{}-autopilot-plan.md", time_slug));
                            self.plan_path = Some(plan_file.clone());
                            
                            if let Err(e) = std::fs::write(&plan_file, &plan) {
                                self.add_line(&format!("  Error writing plan: {}", e), LogStatus::Error, elapsed);
                            } else {
                                self.add_line(&format!("  Plan saved: {}", shorten_path(&plan_file)), LogStatus::Success, elapsed);
                            }
                            
                            self.phase = StartupPhase::WritingPlan;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(&format!("  Claude error: {}", e), LogStatus::Error, elapsed);
                            self.claude_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            return;
                        }
                    }
                }
            }

            StartupPhase::WritingPlan => {
                self.add_line("", LogStatus::Info, elapsed);
                
                let summary_lines: Vec<String> = self.claude_full_text
                    .lines()
                    .filter(|l| l.starts_with("##") || l.starts_with("- ") || l.starts_with("1."))
                    .take(8)
                    .map(|s| s.to_string())
                    .collect();
                
                if !summary_lines.is_empty() {
                    self.add_line("Plan summary:", LogStatus::Info, elapsed);
                    for line in summary_lines {
                        self.add_line(&format!("  {}", line), LogStatus::Info, elapsed);
                    }
                }
                
                self.add_line("", LogStatus::Info, elapsed);
                self.phase = StartupPhase::ExecutingPlan;
                self.phase_started = elapsed;
            }

            StartupPhase::ExecutingPlan => {
                // Check for this specific iteration to allow multiple iterations
                let iteration = self.iteration;
                let exec_marker = if iteration == 1 {
                    "Executing plan with Claude...".to_string()
                } else {
                    format!("Executing plan (iteration {})", iteration)
                };
                if !self.lines.iter().any(|l| l.text.contains(&exec_marker)) {
                    if iteration == 1 {
                        self.add_line("Executing plan with Claude...", LogStatus::Pending, elapsed);
                    } else {
                        self.add_line(&format!("Executing plan (iteration {}) with Claude...", iteration), LogStatus::Pending, elapsed);
                    }
                    
                    let plan = self.claude_full_text.clone();
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    
                    let (tx, rx) = mpsc::channel();
                    self.exec_receiver = Some(rx);
                    
                    std::thread::spawn(move || {
                        run_claude_execution(&plan, model, tx, logger);
                    });
                    
                    self.phase = StartupPhase::StreamingExecution;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingExecution => {
                let mut tokens = Vec::new();
                if let Some(ref rx) = self.exec_receiver {
                    while let Ok(token) = rx.try_recv() {
                        tokens.push(token);
                    }
                }

                for token in tokens {
                    match token {
                        ClaudeToken::Chunk(text) => {
                            self.exec_full_text.push_str(&text);
                            if let Some(ClaudeEvent::Text(s)) = self.exec_events.last_mut() {
                                s.push_str(&text);
                            } else {
                                self.exec_events.push(ClaudeEvent::Text(text));
                            }
                            self.update_exec_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            self.exec_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                            });
                            self.update_exec_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone { name } => {
                            for event in self.exec_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            self.update_exec_streaming_line(elapsed);
                        }
                        ClaudeToken::Done(_result) => {
                            self.exec_receiver = None;
                            if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Executing plan")) {
                                line.status = LogStatus::Success;
                            }
                            
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line(&format!("Execution complete (iteration {}).", self.iteration), LogStatus::Success, elapsed);
                            self.phase = StartupPhase::ReviewingWork;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(&format!("  Execution error: {}", e), LogStatus::Error, elapsed);
                            self.exec_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Execution failed.", LogStatus::Error, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            return;
                        }
                    }
                }
            }

            StartupPhase::ReviewingWork => {
                let review_marker = format!("Reviewing work (iteration {})", self.iteration);
                if !self.lines.iter().any(|l| l.text.contains(&review_marker)) {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line(&format!("Reviewing work (iteration {})...", self.iteration), LogStatus::Pending, elapsed);
                    
                    let plan = self.claude_full_text.clone();
                    let exec_result = self.exec_full_text.clone();
                    let iteration = self.iteration;
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    
                    let (tx, rx) = mpsc::channel();
                    self.review_receiver = Some(rx);
                    
                    std::thread::spawn(move || {
                        run_claude_review(&plan, &exec_result, iteration, model, tx, logger);
                    });
                    
                    self.phase = StartupPhase::StreamingReview;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingReview => {
                let mut tokens = Vec::new();
                if let Some(ref rx) = self.review_receiver {
                    while let Ok(token) = rx.try_recv() {
                        tokens.push(token);
                    }
                }

                for token in tokens {
                    match token {
                        ClaudeToken::Chunk(text) => {
                            self.review_full_text.push_str(&text);
                            if let Some(ClaudeEvent::Text(s)) = self.review_events.last_mut() {
                                s.push_str(&text);
                            } else {
                                self.review_events.push(ClaudeEvent::Text(text));
                            }
                            self.update_review_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            self.review_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                            });
                            self.update_review_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone { name } => {
                            for event in self.review_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            self.update_review_streaming_line(elapsed);
                        }
                        ClaudeToken::Done(review_result) => {
                            self.review_receiver = None;
                            if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Reviewing work")) {
                                line.status = LogStatus::Success;
                            }
                            
                            if review_result.contains("CYCLE COMPLETE") {
                                self.add_line("", LogStatus::Info, elapsed);
                                self.add_line("Review says work complete. Running verification...", LogStatus::Info, elapsed);
                                self.phase = StartupPhase::VerifyingCompletion;
                                self.phase_started = elapsed;
                            } else {
                                self.add_line("", LogStatus::Info, elapsed);
                                self.add_line("Review complete. Starting next iteration...", LogStatus::Info, elapsed);
                                
                                self.iteration += 1;
                                self.claude_full_text = review_result;
                                self.claude_events.clear();
                                self.exec_events.clear();
                                self.exec_full_text.clear();
                                self.review_events.clear();
                                self.review_full_text.clear();
                                
                                self.phase = StartupPhase::WritingPlan;
                                self.phase_started = elapsed;
                            }
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(&format!("  Review error: {}", e), LogStatus::Error, elapsed);
                            self.review_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Review failed. Stopping.", LogStatus::Error, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            return;
                        }
                    }
                }
            }

            StartupPhase::VerifyingCompletion => {
                if self.verification_runner.is_none() {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Running verification checks...", LogStatus::Pending, elapsed);
                    
                    let cwd = std::env::current_dir().unwrap_or_default();
                    self.verification_runner = Some(VerificationRunner::new(&cwd));
                }
                
                let mut runner = self.verification_runner.take().unwrap();
                
                let default_checklist = TerminationChecklist {
                    build_clean: crate::verification::CheckResult::pass(""),
                    clippy_clean: crate::verification::CheckResult::pass(""),
                    tests_passing: crate::verification::CheckResult::pass(""),
                    coverage_adequate: crate::verification::CheckResult::pass(""),
                    no_stubs: crate::verification::CheckResult::pass(""),
                    todos_complete: crate::verification::CheckResult::pass(""),
                    user_stories_complete: crate::verification::CheckResult::pass(""),
                    issues_complete: crate::verification::CheckResult::pass(""),
                    git_clean: crate::verification::CheckResult::pass(""),
                    git_pushed: crate::verification::CheckResult::pass(""),
                };
                
                if let Some(reason) = should_force_stop(
                    self.last_checklist.as_ref().unwrap_or(&default_checklist),
                    &runner,
                ) {
                    self.force_stopped = true;
                    self.force_stop_reason = Some(reason.clone());
                    self.add_line(&format!("  {}", reason), LogStatus::Error, elapsed);
                    self.verification_runner = Some(runner);
                    self.phase = StartupPhase::GeneratingReport;
                    self.phase_started = elapsed;
                    return;
                }
                
                let checklist = runner.run_all_checks();
                self.last_checklist = Some(checklist.clone());
                
                if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Running verification")) {
                    line.status = LogStatus::Success;
                }
                
                self.add_line(&format!("  {}", checklist.summary()), LogStatus::Info, elapsed);
                
                if checklist.all_passed() {
                    self.add_line("  All checks passed!", LogStatus::Success, elapsed);
                    self.verification_runner = Some(runner);
                    self.phase = StartupPhase::GeneratingReport;
                    self.phase_started = elapsed;
                } else {
                    let failures = checklist.failing_checks();
                    for (name, result) in &failures {
                        self.add_line(&format!("  FAIL {}: {}", name, result.message), LogStatus::Error, elapsed);
                    }
                    
                    let first_failure = failures.first().map(|(n, _)| *n).unwrap_or("unknown");
                    if runner.track_failure(first_failure) {
                        self.force_stopped = true;
                        self.force_stop_reason = Some(format!(
                            "Stuck on '{}' check for 6 iterations",
                            first_failure
                        ));
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Stuck on same failure. Generating report...", LogStatus::Error, elapsed);
                        self.verification_runner = Some(runner);
                        self.phase = StartupPhase::GeneratingReport;
                        self.phase_started = elapsed;
                    } else {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Generating fix plan...", LogStatus::Info, elapsed);
                        self.verification_runner = Some(runner);
                        self.phase = StartupPhase::FixingVerificationFailures;
                        self.phase_started = elapsed;
                    }
                }
            }

            StartupPhase::FixingVerificationFailures => {
                if !self.lines.iter().any(|l| l.text.contains("Fixing verification failures")) {
                    self.add_line(&format!("Fixing verification failures (iteration {})...", self.iteration), LogStatus::Pending, elapsed);
                    
                    let checklist = self.last_checklist.clone().unwrap_or_else(|| TerminationChecklist {
                        build_clean: crate::verification::CheckResult::fail("", ""),
                        clippy_clean: crate::verification::CheckResult::fail("", ""),
                        tests_passing: crate::verification::CheckResult::fail("", ""),
                        coverage_adequate: crate::verification::CheckResult::fail("", ""),
                        no_stubs: crate::verification::CheckResult::fail("", ""),
                        todos_complete: crate::verification::CheckResult::fail("", ""),
                        user_stories_complete: crate::verification::CheckResult::fail("", ""),
                        issues_complete: crate::verification::CheckResult::fail("", ""),
                        git_clean: crate::verification::CheckResult::fail("", ""),
                        git_pushed: crate::verification::CheckResult::fail("", ""),
                    });
                    
                    let fix_prompt = generate_fix_prompt(&checklist, self.iteration);
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    
                    let (tx, rx) = mpsc::channel();
                    self.fix_receiver = Some(rx);
                    
                    std::thread::spawn(move || {
                        run_claude_execution(&fix_prompt, model, tx, logger);
                    });
                    
                    self.phase = StartupPhase::StreamingFix;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingFix => {
                let mut tokens = Vec::new();
                if let Some(ref rx) = self.fix_receiver {
                    while let Ok(token) = rx.try_recv() {
                        tokens.push(token);
                    }
                }

                for token in tokens {
                    match token {
                        ClaudeToken::Chunk(text) => {
                            self.fix_full_text.push_str(&text);
                            if let Some(ClaudeEvent::Text(s)) = self.fix_events.last_mut() {
                                s.push_str(&text);
                            } else {
                                self.fix_events.push(ClaudeEvent::Text(text));
                            }
                            self.update_fix_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            self.fix_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                            });
                            self.update_fix_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone { name } => {
                            for event in self.fix_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            self.update_fix_streaming_line(elapsed);
                        }
                        ClaudeToken::Done(_result) => {
                            self.fix_receiver = None;
                            if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Fixing verification")) {
                                line.status = LogStatus::Success;
                            }
                            
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Fix attempt complete. Re-verifying...", LogStatus::Info, elapsed);
                            
                            self.iteration += 1;
                            self.fix_events.clear();
                            self.fix_full_text.clear();
                            
                            self.phase = StartupPhase::VerifyingCompletion;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(&format!("  Fix error: {}", e), LogStatus::Error, elapsed);
                            self.fix_receiver = None;
                            self.force_stopped = true;
                            self.force_stop_reason = Some(format!("Fix attempt failed: {}", e));
                            self.phase = StartupPhase::GeneratingReport;
                            self.phase_started = elapsed;
                            return;
                        }
                    }
                }
            }

            StartupPhase::GeneratingReport => {
                if self.report_path.is_none() {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Generating after-action report...", LogStatus::Pending, elapsed);
                    
                    let cwd = std::env::current_dir().unwrap_or_default();
                    let stats = collect_session_stats(
                        &cwd,
                        &self.session_id,
                        self.start_time,
                        self.iteration,
                    );
                    
                    let checklist = self.last_checklist.clone().unwrap_or_else(|| TerminationChecklist {
                        build_clean: crate::verification::CheckResult::pass("Not checked"),
                        clippy_clean: crate::verification::CheckResult::pass("Not checked"),
                        tests_passing: crate::verification::CheckResult::pass("Not checked"),
                        coverage_adequate: crate::verification::CheckResult::pass("Not checked"),
                        no_stubs: crate::verification::CheckResult::pass("Not checked"),
                        todos_complete: crate::verification::CheckResult::pass("Not checked"),
                        user_stories_complete: crate::verification::CheckResult::pass("Not checked"),
                        issues_complete: crate::verification::CheckResult::pass("Not checked"),
                        git_clean: crate::verification::CheckResult::pass("Not checked"),
                        git_pushed: crate::verification::CheckResult::pass("Not checked"),
                    });
                    
                    let suggested_next_steps = generate_suggested_next_steps(&checklist);
                    let questions_for_user = generate_questions_for_user(
                        &checklist,
                        self.force_stopped,
                        &self.force_stop_reason,
                    );
                    
                    let log_path = self.session_logger
                        .as_ref()
                        .map(|l| l.log_path.clone())
                        .unwrap_or_else(|| PathBuf::from("unknown"));
                    
                    let report = AfterActionReport {
                        stats,
                        checklist,
                        force_stopped: self.force_stopped,
                        force_stop_reason: self.force_stop_reason.clone(),
                        suggested_next_steps,
                        questions_for_user,
                        log_path,
                    };
                    
                    match report.save(&cwd) {
                        Ok(path) => {
                            self.report_path = Some(path.clone());
                            if let Some(line) = self.lines.iter_mut().find(|l| l.text.contains("Generating after-action")) {
                                line.status = LogStatus::Success;
                            }
                            self.add_line(&format!("  Report saved: {}", shorten_path(&path)), LogStatus::Success, elapsed);
                        }
                        Err(e) => {
                            self.add_line(&format!("  Failed to save report: {}", e), LogStatus::Error, elapsed);
                        }
                    }
                    
                    self.add_line("", LogStatus::Info, elapsed);
                    if self.force_stopped {
                        self.add_line("Session stopped (see report for details).", LogStatus::Error, elapsed);
                    } else {
                        self.add_line("Session complete!", LogStatus::Success, elapsed);
                    }
                    self.add_line(&format!("Total iterations: {}", self.iteration), LogStatus::Info, elapsed);
                    self.add_line(&format!("Runtime: {:.1} hours", self.start_instant.elapsed().as_secs_f32() / 3600.0), LogStatus::Info, elapsed);
                    
                    if let Some(ref logger) = self.session_logger {
                        self.add_line(&format!("Session log: {}", shorten_path(&logger.log_path)), LogStatus::Info, elapsed);
                    }
                    
                    self.phase = StartupPhase::Complete;
                    self.phase_started = elapsed;
                }
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

    pub fn update_claude_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self.lines.iter().position(|l| l.text.contains("Creating plan with Claude"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());
        
        self.lines.truncate(start_idx);
        
        let tool_count = self.claude_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { .. })).count();
        let done_count = self.claude_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. })).count();
        let text_count = self.claude_full_text.lines().filter(|l| !l.trim().is_empty()).count();
        
        if tool_count > 0 || text_count > 0 {
            self.add_line(&format!("  {} tools ({} done), {} lines output", tool_count, done_count, text_count), LogStatus::Thinking, elapsed);
        }
        
        let events = self.claude_events.clone();
        let start = if events.len() > 12 { events.len() - 12 } else { 0 };
        
        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 { lines.len() - 10 } else { 0 };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  > {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { name, params, done } => {
                    let status = if *done { "done" } else { "..." };
                    let params_display = extract_tool_display(name, params);
                    self.add_line(&format!("  [{}] {} {}", name, params_display, status), LogStatus::Info, elapsed);
                }
            }
        }
    }

    pub fn update_exec_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self.lines.iter().position(|l| l.text.contains("Executing plan"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());
        
        self.lines.truncate(start_idx);
        
        let tool_count = self.exec_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { .. })).count();
        let done_count = self.exec_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. })).count();
        let text_count = self.exec_full_text.lines().filter(|l| !l.trim().is_empty()).count();
        
        if tool_count > 0 || text_count > 0 {
            self.add_line(&format!("  {} tools ({} done), {} lines output", tool_count, done_count, text_count), LogStatus::Thinking, elapsed);
        }
        
        let events = self.exec_events.clone();
        let start = if events.len() > 12 { events.len() - 12 } else { 0 };
        
        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 { lines.len() - 10 } else { 0 };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  > {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { name, params, done } => {
                    let status = if *done { "done" } else { "..." };
                    let params_display = extract_tool_display(name, params);
                    self.add_line(&format!("  [{}] {} {}", name, params_display, status), LogStatus::Info, elapsed);
                }
            }
        }
    }

    pub fn update_review_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self.lines.iter().position(|l| l.text.contains("Reviewing work"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());
        
        self.lines.truncate(start_idx);
        
        let tool_count = self.review_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { .. })).count();
        let done_count = self.review_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. })).count();
        let text_count = self.review_full_text.lines().filter(|l| !l.trim().is_empty()).count();
        
        if tool_count > 0 || text_count > 0 {
            self.add_line(&format!("  {} tools ({} done), {} lines output", tool_count, done_count, text_count), LogStatus::Thinking, elapsed);
        }
        
        let events = self.review_events.clone();
        let start = if events.len() > 12 { events.len() - 12 } else { 0 };
        
        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 { lines.len() - 10 } else { 0 };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  > {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { name, params, done } => {
                    let status = if *done { "done" } else { "..." };
                    let params_display = extract_tool_display(name, params);
                    self.add_line(&format!("  [{}] {} {}", name, params_display, status), LogStatus::Info, elapsed);
                }
            }
        }
    }

    pub fn update_fix_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self.lines.iter().position(|l| l.text.contains("Fixing verification"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());
        
        self.lines.truncate(start_idx);
        
        let tool_count = self.fix_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { .. })).count();
        let done_count = self.fix_events.iter().filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. })).count();
        let text_count = self.fix_full_text.lines().filter(|l| !l.trim().is_empty()).count();
        
        if tool_count > 0 || text_count > 0 {
            self.add_line(&format!("  {} tools ({} done), {} lines output", tool_count, done_count, text_count), LogStatus::Thinking, elapsed);
        }
        
        let events = self.fix_events.clone();
        let start = if events.len() > 12 { events.len() - 12 } else { 0 };
        
        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 { lines.len() - 10 } else { 0 };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  > {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { name, params, done } => {
                    let status = if *done { "done" } else { "..." };
                    let params_display = extract_tool_display(name, params);
                    self.add_line(&format!("  [{}] {} {}", name, params_display, status), LogStatus::Info, elapsed);
                }
            }
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

impl Default for StartupState {
    fn default() -> Self {
        Self::new()
    }
}
