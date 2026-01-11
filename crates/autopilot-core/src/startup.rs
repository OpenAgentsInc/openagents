use chrono::Local;
use agent_client_protocol_schema as acp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;
use tracing::{debug, info, warn};

// DSPy integration
use crate::dspy_planning::PlanningResult;

use crate::auth;
use crate::checkpoint::SessionCheckpoint;
use crate::claude::{
    ClaudeEvent, ClaudeToken, ClaudeUsageData, run_claude_execution, run_claude_planning,
    run_claude_review,
};
use crate::logger::{SessionLogger, generate_session_id};
use crate::preflight::PreflightConfig;
use crate::report::{
    AfterActionReport, collect_session_stats, generate_questions_for_user,
    generate_suggested_next_steps,
};
use crate::streaming::query_issue_summary;
use crate::utils::shorten_path;
use crate::verification::{
    TerminationChecklist, VerificationRunner, generate_fix_prompt, should_force_stop,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ClaudeModel {
    #[default]
    Sonnet,
    Opus,
}

impl ClaudeModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaudeModel::Sonnet => "claude-sonnet-4-5-20250929",
            ClaudeModel::Opus => "claude-opus-4-5-20251101",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub text: String,
    #[allow(dead_code)]
    pub timestamp: f32,
    pub status: LogStatus,
    /// Which UI section this line belongs to for collapsible grouping.
    pub section: Option<StartupSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LogStatus {
    Pending,
    Success,
    Error,
    Info,
    Thinking,
}

/// Logical groupings of startup phases for collapsible UI sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StartupSection {
    Auth,
    Preflight,
    Tools,
    Pylon,
    Compute,
    Claude,
}

impl StartupSection {
    /// Generate a summary text for this section based on its detail lines.
    pub fn summary_text(&self, details: &[LogLine]) -> String {
        match self {
            StartupSection::Auth => {
                let has_auth = details.iter().any(|l| l.text.contains("Auth ready"));
                if has_auth {
                    "Auth ready".to_string()
                } else {
                    "Checking auth".to_string()
                }
            }
            StartupSection::Preflight => {
                let directive_count = details
                    .iter()
                    .find(|l| l.text.contains("directives"))
                    .and_then(|l| l.text.split_whitespace().find(|s| s.parse::<u32>().is_ok()))
                    .unwrap_or("0");
                format!("Preflight complete ({} directives)", directive_count)
            }
            StartupSection::Tools => {
                let ok_count = details.iter().filter(|l| l.text.contains("[OK]")).count();
                format!("{} tools ready", ok_count)
            }
            StartupSection::Pylon => {
                let running = details
                    .iter()
                    .any(|l| l.text.contains("running") || l.text.contains("started"));
                if running {
                    "Pylon ready".to_string()
                } else {
                    "Pylon not running".to_string()
                }
            }
            StartupSection::Compute => {
                let backends = details.iter().filter(|l| l.text.contains("[OK]")).count();
                format!("Compute: {} local backend(s)", backends)
            }
            StartupSection::Claude => "Claude session".to_string(),
        }
    }

    /// Determine overall status for this section based on its detail lines.
    pub fn summary_status(&self, details: &[LogLine]) -> LogStatus {
        if details.iter().any(|l| l.status == LogStatus::Error) {
            LogStatus::Error
        } else if details
            .iter()
            .all(|l| l.status == LogStatus::Success || l.status == LogStatus::Info)
        {
            LogStatus::Success
        } else if details.iter().any(|l| l.status == LogStatus::Pending) {
            LogStatus::Pending
        } else {
            LogStatus::Info
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum StartupPhase {
    /// Idle - waiting for user to provide a prompt or select an issue
    Idle,
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

impl StartupPhase {
    /// Map a phase to its logical section for UI grouping.
    pub fn section(&self) -> StartupSection {
        match self {
            StartupPhase::Idle => StartupSection::Claude, // Idle shows in main area

            StartupPhase::CheckingOpenCode
            | StartupPhase::CheckingOpenAgents
            | StartupPhase::CopyingAuth
            | StartupPhase::AuthComplete => StartupSection::Auth,

            StartupPhase::RunningPreflight | StartupPhase::PreflightComplete => {
                StartupSection::Preflight
            }

            StartupPhase::CheckingPylon | StartupPhase::StartingPylon => StartupSection::Pylon,

            StartupPhase::DetectingCompute | StartupPhase::ComputeMixReady => {
                StartupSection::Compute
            }

            _ => StartupSection::Claude,
        }
    }
}

pub struct StartupState {
    pub lines: Vec<LogLine>,
    pub phase: StartupPhase,
    pub phase_started: f32,
    pub preflight_config: Option<PreflightConfig>,
    pub model: ClaudeModel,
    issue_summary: Option<String>,
    claude_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub claude_session_id: Option<String>,
    pub claude_events: Vec<ClaudeEvent>,
    pub claude_full_text: String,
    pub acp_events: Vec<acp::SessionNotification>,
    pub plan_path: Option<PathBuf>,
    exec_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub exec_session_id: Option<String>,
    pub exec_events: Vec<ClaudeEvent>,
    pub exec_full_text: String,
    review_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub review_session_id: Option<String>,
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
    pub fix_session_id: Option<String>,
    pub fix_events: Vec<ClaudeEvent>,
    pub fix_full_text: String,
    acp_pending_tools: Vec<AcpPendingTool>,
    acp_tool_counter: u64,
    pub force_stopped: bool,
    pub force_stop_reason: Option<String>,
    pub report_path: Option<PathBuf>,
    // Pylon integration
    pub compute_mix: Option<crate::preflight::ComputeMix>,
    pylon_started: bool,
    // Session usage tracking (accumulated from ClaudeToken::Usage)
    pub session_usage: ClaudeUsageData,
    /// User-provided prompt for this session (if started from UI)
    pub user_prompt: Option<String>,
    /// DSPy structured planning result
    pub dspy_plan: Option<PlanningResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AcpPendingTool {
    id: acp::ToolCallId,
    name: String,
    phase: &'static str,
    params: String,
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
            issue_summary: None,
            claude_receiver: None,
            claude_session_id: None,
            claude_events: Vec::new(),
            claude_full_text: String::new(),
            acp_events: Vec::new(),
            plan_path: None,
            exec_receiver: None,
            exec_session_id: None,
            exec_events: Vec::new(),
            exec_full_text: String::new(),
            review_receiver: None,
            review_session_id: None,
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
            fix_session_id: None,
            fix_events: Vec::new(),
            fix_full_text: String::new(),
            acp_pending_tools: Vec::new(),
            acp_tool_counter: 0,
            force_stopped: false,
            force_stop_reason: None,
            report_path: None,
            compute_mix: None,
            pylon_started: false,
            session_usage: ClaudeUsageData::default(),
            user_prompt: None,
            dspy_plan: None,
        }
    }

    /// Create a new state in Idle mode, waiting for user input.
    pub fn new_idle(model: ClaudeModel) -> Self {
        let session_id = generate_session_id();
        let session_logger = SessionLogger::new(&session_id).ok();
        let start_time = Local::now();
        let start_instant = Instant::now();

        Self {
            lines: vec![],
            phase: StartupPhase::Idle,
            phase_started: 0.0,
            preflight_config: None,
            model,
            issue_summary: None,
            claude_receiver: None,
            claude_session_id: None,
            claude_events: Vec::new(),
            claude_full_text: String::new(),
            acp_events: Vec::new(),
            plan_path: None,
            exec_receiver: None,
            exec_session_id: None,
            exec_events: Vec::new(),
            exec_full_text: String::new(),
            review_receiver: None,
            review_session_id: None,
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
            fix_session_id: None,
            fix_events: Vec::new(),
            fix_full_text: String::new(),
            acp_pending_tools: Vec::new(),
            acp_tool_counter: 0,
            force_stopped: false,
            force_stop_reason: None,
            report_path: None,
            compute_mix: None,
            pylon_started: false,
            session_usage: ClaudeUsageData::default(),
            user_prompt: None,
            dspy_plan: None,
        }
    }

    /// Create a new state with a specific prompt, ready to start execution.
    pub fn new_with_prompt(prompt: String, model: ClaudeModel) -> Self {
        let mut state = Self::with_model(model);
        state.user_prompt = Some(prompt);
        state
    }

    /// Start execution from Idle state with a prompt.
    pub fn start_with_prompt(&mut self, prompt: String) {
        self.user_prompt = Some(prompt);
        if self.phase == StartupPhase::Idle {
            self.phase = StartupPhase::CheckingOpenCode;
            self.phase_started = self.start_instant.elapsed().as_secs_f32();
        }
    }

    /// Check if currently in idle state.
    pub fn is_idle(&self) -> bool {
        self.phase == StartupPhase::Idle
    }

    /// Check if the session is running (not idle and not complete).
    pub fn is_running(&self) -> bool {
        !matches!(self.phase, StartupPhase::Idle | StartupPhase::Complete)
    }

    pub fn add_line(&mut self, text: &str, status: LogStatus, elapsed: f32) {
        self.lines.push(LogLine {
            text: text.to_string(),
            timestamp: elapsed,
            status,
            section: Some(self.phase.section()),
        });
    }

    /// Add a line with an explicit section override (useful for tool discovery during preflight).
    pub fn add_line_to_section(
        &mut self,
        text: &str,
        status: LogStatus,
        elapsed: f32,
        section: StartupSection,
    ) {
        self.lines.push(LogLine {
            text: text.to_string(),
            timestamp: elapsed,
            status,
            section: Some(section),
        });
    }

    pub fn tick(&mut self, elapsed: f32) {
        let phase_time = elapsed - self.phase_started;

        match self.phase {
            StartupPhase::Idle => {
                // Do nothing in Idle - waiting for user input via start_with_prompt()
            }

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

                if !self
                    .lines
                    .iter()
                    .any(|l| l.text.contains("OpenAgents auth"))
                {
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
                    self.add_line(
                        "Copying credentials from OpenCode...",
                        LogStatus::Pending,
                        elapsed,
                    );
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
                                &format!(
                                    "  Saved to {}",
                                    shorten_path(&auth::openagents_auth_path())
                                ),
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
                if phase_time > 0.3
                    && !self.lines.iter().any(|l| {
                        l.text.contains("Auth ready") || l.text.contains("Anthropic auth not")
                    })
                {
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

                if !self
                    .lines
                    .iter()
                    .any(|l| l.text.contains("Running preflight"))
                {
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

                if !self
                    .lines
                    .iter()
                    .any(|l| l.text.contains("Checking local pylon"))
                {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line("Checking local pylon...", LogStatus::Pending, elapsed);
                }

                if phase_time > 0.6 {
                    if let Some(line) = self
                        .lines
                        .iter_mut()
                        .find(|l| l.text.contains("Checking local pylon"))
                    {
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
                                    self.add_line(
                                        &format!("  Uptime: {}h {}m", hours, mins),
                                        LogStatus::Info,
                                        elapsed,
                                    );
                                } else {
                                    self.add_line(
                                        &format!("  Uptime: {}m", mins),
                                        LogStatus::Info,
                                        elapsed,
                                    );
                                }
                            }
                            if info.jobs_completed > 0 {
                                self.add_line(
                                    &format!("  Jobs completed: {}", info.jobs_completed),
                                    LogStatus::Info,
                                    elapsed,
                                );
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
                    if !crate::pylon_integration::pylon_identity_exists() {
                        self.add_line(
                            "Initializing pylon identity...",
                            LogStatus::Pending,
                            elapsed,
                        );
                        match crate::pylon_integration::init_pylon_identity() {
                            Ok(()) => {
                                if let Some(line) = self
                                    .lines
                                    .iter_mut()
                                    .find(|l| l.text.contains("Initializing pylon identity"))
                                {
                                    line.status = LogStatus::Success;
                                }
                                self.add_line(
                                    "  Pylon identity created (seed phrase printed in terminal)",
                                    LogStatus::Info,
                                    elapsed,
                                );
                            }
                            Err(e) => {
                                if let Some(line) = self
                                    .lines
                                    .iter_mut()
                                    .find(|l| l.text.contains("Initializing pylon identity"))
                                {
                                    line.status = LogStatus::Error;
                                }
                                self.add_line(
                                    &format!("  Failed to initialize pylon identity: {}", e),
                                    LogStatus::Error,
                                    elapsed,
                                );
                                self.phase = StartupPhase::DetectingCompute;
                                self.phase_started = elapsed;
                                return;
                            }
                        }
                    }

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
                    if let Some(line) = self
                        .lines
                        .iter_mut()
                        .find(|l| l.text.contains("Starting pylon"))
                    {
                        line.status = LogStatus::Info;
                    }

                    if crate::pylon_integration::check_pylon_running() {
                        self.add_line("  Pylon started successfully", LogStatus::Success, elapsed);
                    } else {
                        self.add_line(
                            "  Pylon not started (continuing anyway)",
                            LogStatus::Info,
                            elapsed,
                        );
                    }

                    self.phase = StartupPhase::DetectingCompute;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::DetectingCompute => {
                if phase_time < 0.3 {
                    return;
                }

                if !self
                    .lines
                    .iter()
                    .any(|l| l.text.contains("Detecting compute providers"))
                {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line(
                        "Detecting compute providers...",
                        LogStatus::Pending,
                        elapsed,
                    );
                }

                if phase_time > 0.8 {
                    if let Some(line) = self
                        .lines
                        .iter_mut()
                        .find(|l| l.text.contains("Detecting compute providers"))
                    {
                        line.status = LogStatus::Info;
                    }

                    // Discover swarm providers (synchronous for now)
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line(
                        "Querying NIP-89 swarm providers...",
                        LogStatus::Pending,
                        elapsed,
                    );
                    let swarm_providers = crate::pylon_integration::discover_swarm_providers();

                    if let Some(line) = self
                        .lines
                        .iter_mut()
                        .find(|l| l.text.contains("Querying NIP-89"))
                    {
                        line.status = LogStatus::Info;
                    }

                    if swarm_providers.is_empty() {
                        self.add_line("  No remote providers discovered", LogStatus::Info, elapsed);
                    } else {
                        self.add_line(
                            &format!("  Found {} remote provider(s)", swarm_providers.len()),
                            LogStatus::Success,
                            elapsed,
                        );
                    }

                    // Get cloud providers from preflight
                    let cloud_providers = self
                        .preflight_config
                        .as_ref()
                        .map(|c| c.inference.cloud_providers.clone())
                        .unwrap_or_default();

                    // Get pylon info
                    let pylon_info = crate::pylon_integration::get_pylon_status();

                    // Build compute mix
                    self.compute_mix = Some(crate::preflight::ComputeMix {
                        pylon: pylon_info,
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
                        let cloud = mix.cloud_providers.clone();
                        let swarm_count = mix.swarm_providers.len();
                        (cloud, swarm_count)
                    });

                    if let Some((cloud_providers, swarm_count)) = mix_summary {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Compute mix:", LogStatus::Info, elapsed);

                        // Cloud summary
                        if !cloud_providers.is_empty() {
                            self.add_line(
                                &format!("  Cloud: {}", cloud_providers.join(", ")),
                                LogStatus::Success,
                                elapsed,
                            );
                        }

                        // Swarm summary
                        if swarm_count > 0 {
                            self.add_line(
                                &format!("  Swarm: {} providers via NIP-89", swarm_count),
                                LogStatus::Success,
                                elapsed,
                            );
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

            StartupPhase::PlanningWithClaude => {
                if !self
                    .lines
                    .iter()
                    .any(|l| l.text.contains("Creating plan with Claude"))
                {
                    self.add_line("", LogStatus::Info, elapsed);
                    let iteration = self.iteration;
                    if iteration == 1 {
                        self.add_line("Creating plan with Claude...", LogStatus::Pending, elapsed);
                    } else {
                        self.add_line(
                            &format!("Creating plan (iteration {}) with Claude...", iteration),
                            LogStatus::Pending,
                            elapsed,
                        );
                    }

                    let cwd = std::env::current_dir().unwrap_or_default();
                    let issue_summary = self
                        .issue_summary
                        .clone()
                        .or_else(|| query_issue_summary(&cwd))
                        .unwrap_or_default();
                    if !issue_summary.is_empty() {
                        self.issue_summary = Some(issue_summary.clone());
                    }
                    let assessment = String::new();
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    let resume_session_id = self.claude_session_id.clone();

                    let (tx, rx) = mpsc::channel();
                    self.claude_receiver = Some(rx);

                    std::thread::spawn(move || {
                        run_claude_planning(
                            &cwd,
                            &issue_summary,
                            &assessment,
                            model,
                            resume_session_id,
                            tx,
                            logger,
                        );
                    });

                    self.phase = StartupPhase::StreamingClaudePlan;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingClaudePlan => {
                if self.claude_receiver.is_none() {
                    if let Some(session_id) = self.claude_session_id.clone() {
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let issue_summary = self
                            .issue_summary
                            .clone()
                            .or_else(|| query_issue_summary(&cwd))
                            .unwrap_or_default();
                        if !issue_summary.is_empty() {
                            self.issue_summary = Some(issue_summary.clone());
                        }
                        let assessment = String::new();
                        let model = self.model;
                        let logger = self.session_logger.clone();

                        let (tx, rx) = mpsc::channel();
                        self.claude_receiver = Some(rx);

                        std::thread::spawn(move || {
                            run_claude_planning(
                                &cwd,
                                &issue_summary,
                                &assessment,
                                model,
                                Some(session_id),
                                tx,
                                logger,
                            );
                        });
                    }
                }

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
                                self.claude_events.push(ClaudeEvent::Text(text.clone()));
                            }
                            self.push_acp_text("plan", text);
                            self.update_claude_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            let params_clone = params.clone();
                            self.claude_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                                output: None,
                                is_error: false,
                            });
                            self.push_acp_tool_use("plan", name, params_clone);
                            self.update_claude_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone {
                            name,
                            output,
                            is_error,
                        } => {
                            // Find the matching tool and get its params
                            let params = self.claude_events.iter().rev().find_map(|e| {
                                if let ClaudeEvent::Tool {
                                    name: n,
                                    params,
                                    done,
                                    ..
                                } = e
                                {
                                    if n == &name && !*done {
                                        Some(params.clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                            // Mark original as done (for done counting)
                            for event in self.claude_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            // Push completion event with output so shell receives it
                            if let Some(params) = params {
                                let output_clone = output.clone();
                                self.claude_events.push(ClaudeEvent::Tool {
                                    name: name.clone(),
                                    params,
                                    done: true,
                                    output,
                                    is_error,
                                });
                                self.push_acp_tool_done("plan", name, output_clone, is_error);
                            } else {
                                self.push_acp_tool_done("plan", name, output.clone(), is_error);
                            }
                            self.update_claude_streaming_line(elapsed);
                        }
                        ClaudeToken::Progress {
                            tool_name,
                            elapsed_secs,
                        } => {
                            self.claude_events.push(ClaudeEvent::ToolProgress {
                                tool_name: tool_name.clone(),
                                elapsed_secs,
                            });
                            self.push_acp_tool_progress("plan", tool_name, elapsed_secs);
                        }
                        ClaudeToken::SessionId(session_id) => {
                            if self.claude_session_id.as_deref() != Some(session_id.as_str()) {
                                self.add_line(
                                    &format!("  Claude session id (plan): {}", session_id),
                                    LogStatus::Info,
                                    elapsed,
                                );
                            }
                            self.claude_session_id = Some(session_id);
                        }
                        ClaudeToken::Done(plan) => {
                            self.claude_receiver = None;
                            if let Some(line) = self
                                .lines
                                .iter_mut()
                                .find(|l| l.text.contains("Creating plan with Claude"))
                            {
                                line.status = LogStatus::Success;
                            }

                            let now = Local::now();
                            let date_dir = now.format("%Y%m%d").to_string();
                            let time_slug = now.format("%H%M%S").to_string();

                            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                            let plans_dir = PathBuf::from(&home)
                                .join(".openagents/plans")
                                .join(&date_dir);

                            if let Err(e) = std::fs::create_dir_all(&plans_dir) {
                                self.add_line(
                                    &format!("  Error creating dir: {}", e),
                                    LogStatus::Error,
                                    elapsed,
                                );
                                self.phase = StartupPhase::Complete;
                                self.phase_started = elapsed;
                                return;
                            }

                            let plan_file =
                                plans_dir.join(format!("{}-autopilot-plan.md", time_slug));
                            self.plan_path = Some(plan_file.clone());

                            if let Err(e) = std::fs::write(&plan_file, &plan) {
                                self.add_line(
                                    &format!("  Error writing plan: {}", e),
                                    LogStatus::Error,
                                    elapsed,
                                );
                            } else {
                                self.add_line(
                                    &format!("  Plan saved: {}", shorten_path(&plan_file)),
                                    LogStatus::Success,
                                    elapsed,
                                );
                            }

                            self.phase = StartupPhase::WritingPlan;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(
                                &format!("  Claude error: {}", e),
                                LogStatus::Error,
                                elapsed,
                            );
                            self.claude_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Ready for tasks.", LogStatus::Success, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Usage(usage) => {
                            // Accumulate usage data for session stats
                            tracing::info!(
                                "[STARTUP] Received Usage token: input={}, output={}, cost=${:.4}",
                                usage.input_tokens,
                                usage.output_tokens,
                                usage.total_cost_usd
                            );
                            self.session_usage.input_tokens += usage.input_tokens;
                            self.session_usage.output_tokens += usage.output_tokens;
                            self.session_usage.cache_read_tokens += usage.cache_read_tokens;
                            self.session_usage.cache_write_tokens += usage.cache_write_tokens;
                            self.session_usage.total_cost_usd += usage.total_cost_usd;
                            if let Some(duration_ms) = usage.duration_ms {
                                self.session_usage.duration_ms =
                                    Some(self.session_usage.duration_ms.unwrap_or(0) + duration_ms);
                            }
                            if let Some(duration_api_ms) = usage.duration_api_ms {
                                self.session_usage.duration_api_ms = Some(
                                    self.session_usage.duration_api_ms.unwrap_or(0) + duration_api_ms,
                                );
                            }
                            if let Some(num_turns) = usage.num_turns {
                                self.session_usage.num_turns =
                                    Some(self.session_usage.num_turns.unwrap_or(0) + num_turns);
                            }
                            if usage.context_window.is_some() {
                                self.session_usage.context_window = usage.context_window;
                            }
                            self.session_usage.model = usage.model.clone();
                            tracing::info!(
                                "[STARTUP] Accumulated usage: input={}, output={}, cost=${:.4}",
                                self.session_usage.input_tokens,
                                self.session_usage.output_tokens,
                                self.session_usage.total_cost_usd
                            );
                        }
                    }
                }
            }

            StartupPhase::WritingPlan => {
                self.add_line("", LogStatus::Info, elapsed);

                let summary_lines: Vec<String> = self
                    .claude_full_text
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
                        self.add_line(
                            &format!("Executing plan (iteration {}) with Claude...", iteration),
                            LogStatus::Pending,
                            elapsed,
                        );
                    }

                    let plan = self.claude_full_text.clone();
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    let resume_session_id = self.exec_session_id.clone();

                    let (tx, rx) = mpsc::channel();
                    self.exec_receiver = Some(rx);

                    std::thread::spawn(move || {
                        run_claude_execution(&plan, model, resume_session_id, tx, logger);
                    });

                    self.phase = StartupPhase::StreamingExecution;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingExecution => {
                if self.exec_receiver.is_none() {
                    if let Some(session_id) = self.exec_session_id.clone() {
                        let plan = self.claude_full_text.clone();
                        let model = self.model;
                        let logger = self.session_logger.clone();

                        let (tx, rx) = mpsc::channel();
                        self.exec_receiver = Some(rx);

                        std::thread::spawn(move || {
                            run_claude_execution(&plan, model, Some(session_id), tx, logger);
                        });
                    }
                }

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
                                self.exec_events.push(ClaudeEvent::Text(text.clone()));
                            }
                            self.push_acp_text("exec", text);
                            self.update_exec_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            let params_clone = params.clone();
                            self.exec_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                                output: None,
                                is_error: false,
                            });
                            self.push_acp_tool_use("exec", name, params_clone);
                            self.update_exec_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone {
                            name,
                            output,
                            is_error,
                        } => {
                            // Find the matching tool and get its params
                            let params = self.exec_events.iter().rev().find_map(|e| {
                                if let ClaudeEvent::Tool {
                                    name: n,
                                    params,
                                    done,
                                    ..
                                } = e
                                {
                                    if n == &name && !*done {
                                        Some(params.clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                            // Mark original as done
                            for event in self.exec_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            // Push completion event with output
                            if let Some(params) = params {
                                let output_clone = output.clone();
                                self.exec_events.push(ClaudeEvent::Tool {
                                    name: name.clone(),
                                    params,
                                    done: true,
                                    output,
                                    is_error,
                                });
                                self.push_acp_tool_done("exec", name, output_clone, is_error);
                            } else {
                                self.push_acp_tool_done("exec", name, output.clone(), is_error);
                            }
                            self.update_exec_streaming_line(elapsed);
                        }
                        ClaudeToken::Progress {
                            tool_name,
                            elapsed_secs,
                        } => {
                            self.exec_events.push(ClaudeEvent::ToolProgress {
                                tool_name: tool_name.clone(),
                                elapsed_secs,
                            });
                            self.push_acp_tool_progress("exec", tool_name, elapsed_secs);
                        }
                        ClaudeToken::SessionId(session_id) => {
                            if self.exec_session_id.as_deref() != Some(session_id.as_str()) {
                                self.add_line(
                                    &format!("  Claude session id (exec): {}", session_id),
                                    LogStatus::Info,
                                    elapsed,
                                );
                            }
                            self.exec_session_id = Some(session_id);
                        }
                        ClaudeToken::Done(_result) => {
                            self.exec_receiver = None;
                            if let Some(line) = self
                                .lines
                                .iter_mut()
                                .find(|l| l.text.contains("Executing plan"))
                            {
                                line.status = LogStatus::Success;
                            }

                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line(
                                &format!("Execution complete (iteration {}).", self.iteration),
                                LogStatus::Success,
                                elapsed,
                            );
                            self.phase = StartupPhase::ReviewingWork;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(
                                &format!("  Execution error: {}", e),
                                LogStatus::Error,
                                elapsed,
                            );
                            self.exec_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Execution failed.", LogStatus::Error, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Usage(usage) => {
                            // Accumulate usage data for session stats
                            self.session_usage.input_tokens += usage.input_tokens;
                            self.session_usage.output_tokens += usage.output_tokens;
                            self.session_usage.cache_read_tokens += usage.cache_read_tokens;
                            self.session_usage.cache_write_tokens += usage.cache_write_tokens;
                            self.session_usage.total_cost_usd += usage.total_cost_usd;
                            if let Some(duration_ms) = usage.duration_ms {
                                self.session_usage.duration_ms =
                                    Some(self.session_usage.duration_ms.unwrap_or(0) + duration_ms);
                            }
                            if let Some(duration_api_ms) = usage.duration_api_ms {
                                self.session_usage.duration_api_ms = Some(
                                    self.session_usage.duration_api_ms.unwrap_or(0) + duration_api_ms,
                                );
                            }
                            if let Some(num_turns) = usage.num_turns {
                                self.session_usage.num_turns =
                                    Some(self.session_usage.num_turns.unwrap_or(0) + num_turns);
                            }
                            if usage.context_window.is_some() {
                                self.session_usage.context_window = usage.context_window;
                            }
                            self.session_usage.model = usage.model;
                        }
                    }
                }
            }

            StartupPhase::ReviewingWork => {
                let review_marker = format!("Reviewing work (iteration {})", self.iteration);
                if !self.lines.iter().any(|l| l.text.contains(&review_marker)) {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line(
                        &format!("Reviewing work (iteration {})...", self.iteration),
                        LogStatus::Pending,
                        elapsed,
                    );

                    let plan = self.claude_full_text.clone();
                    let exec_result = self.exec_full_text.clone();
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    let resume_session_id = self.review_session_id.clone();

                    let (tx, rx) = mpsc::channel();
                    self.review_receiver = Some(rx);

                    std::thread::spawn(move || {
                        run_claude_review(
                            &plan,
                            model,
                            &exec_result,
                            resume_session_id,
                            tx,
                            logger,
                        );
                    });

                    self.phase = StartupPhase::StreamingReview;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingReview => {
                if self.review_receiver.is_none() {
                    if let Some(session_id) = self.review_session_id.clone() {
                        let plan = self.claude_full_text.clone();
                        let exec_result = self.exec_full_text.clone();
                        let model = self.model;
                        let logger = self.session_logger.clone();

                        let (tx, rx) = mpsc::channel();
                        self.review_receiver = Some(rx);

                        std::thread::spawn(move || {
                            run_claude_review(
                                &plan,
                                model,
                                &exec_result,
                                Some(session_id),
                                tx,
                                logger,
                            );
                        });
                    }
                }

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
                                self.review_events.push(ClaudeEvent::Text(text.clone()));
                            }
                            self.push_acp_text("review", text);
                            self.update_review_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            let params_clone = params.clone();
                            self.review_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                                output: None,
                                is_error: false,
                            });
                            self.push_acp_tool_use("review", name, params_clone);
                            self.update_review_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone {
                            name,
                            output,
                            is_error,
                        } => {
                            // Find the matching tool and get its params
                            let params = self.review_events.iter().rev().find_map(|e| {
                                if let ClaudeEvent::Tool {
                                    name: n,
                                    params,
                                    done,
                                    ..
                                } = e
                                {
                                    if n == &name && !*done {
                                        Some(params.clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                            // Mark original as done
                            for event in self.review_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            // Push completion event with output
                            if let Some(params) = params {
                                let output_clone = output.clone();
                                self.review_events.push(ClaudeEvent::Tool {
                                    name: name.clone(),
                                    params,
                                    done: true,
                                    output,
                                    is_error,
                                });
                                self.push_acp_tool_done("review", name, output_clone, is_error);
                            } else {
                                self.push_acp_tool_done("review", name, output.clone(), is_error);
                            }
                            self.update_review_streaming_line(elapsed);
                        }
                        ClaudeToken::Progress {
                            tool_name,
                            elapsed_secs,
                        } => {
                            self.review_events.push(ClaudeEvent::ToolProgress {
                                tool_name: tool_name.clone(),
                                elapsed_secs,
                            });
                            self.push_acp_tool_progress("review", tool_name, elapsed_secs);
                        }
                        ClaudeToken::SessionId(session_id) => {
                            if self.review_session_id.as_deref() != Some(session_id.as_str()) {
                                self.add_line(
                                    &format!("  Claude session id (review): {}", session_id),
                                    LogStatus::Info,
                                    elapsed,
                                );
                            }
                            self.review_session_id = Some(session_id);
                        }
                        ClaudeToken::Done(review_result) => {
                            self.review_receiver = None;
                            if let Some(line) = self
                                .lines
                                .iter_mut()
                                .find(|l| l.text.contains("Reviewing work"))
                            {
                                line.status = LogStatus::Success;
                            }

                            if review_result.contains("CYCLE COMPLETE") {
                                self.add_line("", LogStatus::Info, elapsed);
                                self.add_line(
                                    "Review says work complete. Running verification...",
                                    LogStatus::Info,
                                    elapsed,
                                );
                                self.phase = StartupPhase::VerifyingCompletion;
                                self.phase_started = elapsed;
                            } else {
                                self.add_line("", LogStatus::Info, elapsed);
                                self.add_line(
                                    "Review complete. Starting next iteration...",
                                    LogStatus::Info,
                                    elapsed,
                                );

                                self.iteration += 1;
                                self.claude_session_id = None;
                                self.exec_session_id = None;
                                self.review_session_id = None;
                                self.fix_session_id = None;
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
                            self.add_line(
                                &format!("  Review error: {}", e),
                                LogStatus::Error,
                                elapsed,
                            );
                            self.review_receiver = None;
                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line("Review failed. Stopping.", LogStatus::Error, elapsed);
                            self.phase = StartupPhase::Complete;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Usage(usage) => {
                            // Accumulate usage data for session stats
                            self.session_usage.input_tokens += usage.input_tokens;
                            self.session_usage.output_tokens += usage.output_tokens;
                            self.session_usage.cache_read_tokens += usage.cache_read_tokens;
                            self.session_usage.cache_write_tokens += usage.cache_write_tokens;
                            self.session_usage.total_cost_usd += usage.total_cost_usd;
                            if let Some(duration_ms) = usage.duration_ms {
                                self.session_usage.duration_ms =
                                    Some(self.session_usage.duration_ms.unwrap_or(0) + duration_ms);
                            }
                            if let Some(duration_api_ms) = usage.duration_api_ms {
                                self.session_usage.duration_api_ms = Some(
                                    self.session_usage.duration_api_ms.unwrap_or(0) + duration_api_ms,
                                );
                            }
                            if let Some(num_turns) = usage.num_turns {
                                self.session_usage.num_turns =
                                    Some(self.session_usage.num_turns.unwrap_or(0) + num_turns);
                            }
                            if usage.context_window.is_some() {
                                self.session_usage.context_window = usage.context_window;
                            }
                            self.session_usage.model = usage.model;
                        }
                    }
                }
            }

            StartupPhase::VerifyingCompletion => {
                if self.verification_runner.is_none() {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line(
                        "Running verification checks...",
                        LogStatus::Pending,
                        elapsed,
                    );

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

                if let Some(line) = self
                    .lines
                    .iter_mut()
                    .find(|l| l.text.contains("Running verification"))
                {
                    line.status = LogStatus::Success;
                }

                self.add_line(
                    &format!("  {}", checklist.summary()),
                    LogStatus::Info,
                    elapsed,
                );

                if checklist.all_passed() {
                    self.add_line("  All checks passed!", LogStatus::Success, elapsed);
                    self.verification_runner = Some(runner);
                    self.phase = StartupPhase::GeneratingReport;
                    self.phase_started = elapsed;
                } else {
                    let failures = checklist.failing_checks();
                    for (name, result) in &failures {
                        self.add_line(
                            &format!("  FAIL {}: {}", name, result.message),
                            LogStatus::Error,
                            elapsed,
                        );
                    }

                    let first_failure = failures.first().map(|(n, _)| *n).unwrap_or("unknown");
                    if runner.track_failure(first_failure) {
                        self.force_stopped = true;
                        self.force_stop_reason = Some(format!(
                            "Stuck on '{}' check for 6 iterations",
                            first_failure
                        ));
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line(
                            "Stuck on same failure. Generating report...",
                            LogStatus::Error,
                            elapsed,
                        );
                        self.verification_runner = Some(runner);
                        self.phase = StartupPhase::GeneratingReport;
                        self.phase_started = elapsed;
                    } else {
                        self.add_line("", LogStatus::Info, elapsed);
                        self.add_line("Generating fix plan...", LogStatus::Info, elapsed);
                        self.fix_session_id = None;
                        self.verification_runner = Some(runner);
                        self.phase = StartupPhase::FixingVerificationFailures;
                        self.phase_started = elapsed;
                    }
                }
            }

            StartupPhase::FixingVerificationFailures => {
                if !self
                    .lines
                    .iter()
                    .any(|l| l.text.contains("Fixing verification failures"))
                {
                    self.add_line(
                        &format!(
                            "Fixing verification failures (iteration {})...",
                            self.iteration
                        ),
                        LogStatus::Pending,
                        elapsed,
                    );

                    let checklist =
                        self.last_checklist
                            .clone()
                            .unwrap_or_else(|| TerminationChecklist {
                                build_clean: crate::verification::CheckResult::fail("", ""),
                                clippy_clean: crate::verification::CheckResult::fail("", ""),
                                tests_passing: crate::verification::CheckResult::fail("", ""),
                                coverage_adequate: crate::verification::CheckResult::fail("", ""),
                                no_stubs: crate::verification::CheckResult::fail("", ""),
                                todos_complete: crate::verification::CheckResult::fail("", ""),
                                user_stories_complete: crate::verification::CheckResult::fail(
                                    "", "",
                                ),
                                issues_complete: crate::verification::CheckResult::fail("", ""),
                                git_clean: crate::verification::CheckResult::fail("", ""),
                                git_pushed: crate::verification::CheckResult::fail("", ""),
                            });

                    let fix_prompt = generate_fix_prompt(&checklist, self.iteration);
                    let model = self.model;
                    let logger = self.session_logger.clone();
                    let resume_session_id = self.fix_session_id.clone();

                    let (tx, rx) = mpsc::channel();
                    self.fix_receiver = Some(rx);

                    std::thread::spawn(move || {
                        run_claude_execution(&fix_prompt, model, resume_session_id, tx, logger);
                    });

                    self.phase = StartupPhase::StreamingFix;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::StreamingFix => {
                if self.fix_receiver.is_none() {
                    if let Some(session_id) = self.fix_session_id.clone() {
                        let fix_prompt = generate_fix_prompt(
                            &self
                                .last_checklist
                                .clone()
                                .unwrap_or_else(|| TerminationChecklist {
                                    build_clean: crate::verification::CheckResult::fail("", ""),
                                    clippy_clean: crate::verification::CheckResult::fail("", ""),
                                    tests_passing: crate::verification::CheckResult::fail("", ""),
                                    coverage_adequate: crate::verification::CheckResult::fail(
                                        "", "",
                                    ),
                                    no_stubs: crate::verification::CheckResult::fail("", ""),
                                    todos_complete: crate::verification::CheckResult::fail("", ""),
                                    user_stories_complete: crate::verification::CheckResult::fail(
                                        "", "",
                                    ),
                                    issues_complete: crate::verification::CheckResult::fail("", ""),
                                    git_clean: crate::verification::CheckResult::fail("", ""),
                                    git_pushed: crate::verification::CheckResult::fail("", ""),
                                }),
                            self.iteration,
                        );
                        let model = self.model;
                        let logger = self.session_logger.clone();

                        let (tx, rx) = mpsc::channel();
                        self.fix_receiver = Some(rx);

                        std::thread::spawn(move || {
                            run_claude_execution(&fix_prompt, model, Some(session_id), tx, logger);
                        });
                    }
                }

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
                                self.fix_events.push(ClaudeEvent::Text(text.clone()));
                            }
                            self.push_acp_text("fix", text);
                            self.update_fix_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolUse { name, params } => {
                            let params_clone = params.clone();
                            self.fix_events.push(ClaudeEvent::Tool {
                                name: name.clone(),
                                params,
                                done: false,
                                output: None,
                                is_error: false,
                            });
                            self.push_acp_tool_use("fix", name, params_clone);
                            self.update_fix_streaming_line(elapsed);
                        }
                        ClaudeToken::ToolDone {
                            name,
                            output,
                            is_error,
                        } => {
                            // Find the matching tool and get its params
                            let params = self.fix_events.iter().rev().find_map(|e| {
                                if let ClaudeEvent::Tool {
                                    name: n,
                                    params,
                                    done,
                                    ..
                                } = e
                                {
                                    if n == &name && !*done {
                                        Some(params.clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                            // Mark original as done
                            for event in self.fix_events.iter_mut().rev() {
                                if let ClaudeEvent::Tool { name: n, done, .. } = event {
                                    if n == &name && !*done {
                                        *done = true;
                                        break;
                                    }
                                }
                            }
                            // Push completion event with output
                            if let Some(params) = params {
                                let output_clone = output.clone();
                                self.fix_events.push(ClaudeEvent::Tool {
                                    name: name.clone(),
                                    params,
                                    done: true,
                                    output,
                                    is_error,
                                });
                                self.push_acp_tool_done("fix", name, output_clone, is_error);
                            } else {
                                self.push_acp_tool_done("fix", name, output.clone(), is_error);
                            }
                            self.update_fix_streaming_line(elapsed);
                        }
                        ClaudeToken::Progress {
                            tool_name,
                            elapsed_secs,
                        } => {
                            self.fix_events.push(ClaudeEvent::ToolProgress {
                                tool_name: tool_name.clone(),
                                elapsed_secs,
                            });
                            self.push_acp_tool_progress("fix", tool_name, elapsed_secs);
                        }
                        ClaudeToken::SessionId(session_id) => {
                            if self.fix_session_id.as_deref() != Some(session_id.as_str()) {
                                self.add_line(
                                    &format!("  Claude session id (fix): {}", session_id),
                                    LogStatus::Info,
                                    elapsed,
                                );
                            }
                            self.fix_session_id = Some(session_id);
                        }
                        ClaudeToken::Done(_result) => {
                            self.fix_receiver = None;
                            if let Some(line) = self
                                .lines
                                .iter_mut()
                                .find(|l| l.text.contains("Fixing verification"))
                            {
                                line.status = LogStatus::Success;
                            }

                            self.add_line("", LogStatus::Info, elapsed);
                            self.add_line(
                                "Fix attempt complete. Re-verifying...",
                                LogStatus::Info,
                                elapsed,
                            );

                            self.iteration += 1;
                            self.fix_events.clear();
                            self.fix_full_text.clear();

                            self.phase = StartupPhase::VerifyingCompletion;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Error(e) => {
                            self.add_line(
                                &format!("  Fix error: {}", e),
                                LogStatus::Error,
                                elapsed,
                            );
                            self.fix_receiver = None;
                            self.force_stopped = true;
                            self.force_stop_reason = Some(format!("Fix attempt failed: {}", e));
                            self.phase = StartupPhase::GeneratingReport;
                            self.phase_started = elapsed;
                            return;
                        }
                        ClaudeToken::Usage(usage) => {
                            // Accumulate usage data for session stats
                            self.session_usage.input_tokens += usage.input_tokens;
                            self.session_usage.output_tokens += usage.output_tokens;
                            self.session_usage.cache_read_tokens += usage.cache_read_tokens;
                            self.session_usage.cache_write_tokens += usage.cache_write_tokens;
                            self.session_usage.total_cost_usd += usage.total_cost_usd;
                            if let Some(duration_ms) = usage.duration_ms {
                                self.session_usage.duration_ms =
                                    Some(self.session_usage.duration_ms.unwrap_or(0) + duration_ms);
                            }
                            if let Some(duration_api_ms) = usage.duration_api_ms {
                                self.session_usage.duration_api_ms = Some(
                                    self.session_usage.duration_api_ms.unwrap_or(0) + duration_api_ms,
                                );
                            }
                            if let Some(num_turns) = usage.num_turns {
                                self.session_usage.num_turns =
                                    Some(self.session_usage.num_turns.unwrap_or(0) + num_turns);
                            }
                            if usage.context_window.is_some() {
                                self.session_usage.context_window = usage.context_window;
                            }
                            self.session_usage.model = usage.model;
                        }
                    }
                }
            }

            StartupPhase::GeneratingReport => {
                if self.report_path.is_none() {
                    self.add_line("", LogStatus::Info, elapsed);
                    self.add_line(
                        "Generating after-action report...",
                        LogStatus::Pending,
                        elapsed,
                    );

                    let cwd = std::env::current_dir().unwrap_or_default();
                    let stats = collect_session_stats(
                        &cwd,
                        &self.session_id,
                        self.start_time,
                        self.iteration,
                    );

                    let checklist =
                        self.last_checklist
                            .clone()
                            .unwrap_or_else(|| TerminationChecklist {
                                build_clean: crate::verification::CheckResult::pass("Not checked"),
                                clippy_clean: crate::verification::CheckResult::pass("Not checked"),
                                tests_passing: crate::verification::CheckResult::pass(
                                    "Not checked",
                                ),
                                coverage_adequate: crate::verification::CheckResult::pass(
                                    "Not checked",
                                ),
                                no_stubs: crate::verification::CheckResult::pass("Not checked"),
                                todos_complete: crate::verification::CheckResult::pass(
                                    "Not checked",
                                ),
                                user_stories_complete: crate::verification::CheckResult::pass(
                                    "Not checked",
                                ),
                                issues_complete: crate::verification::CheckResult::pass(
                                    "Not checked",
                                ),
                                git_clean: crate::verification::CheckResult::pass("Not checked"),
                                git_pushed: crate::verification::CheckResult::pass("Not checked"),
                            });

                    let suggested_next_steps = generate_suggested_next_steps(&checklist);
                    let questions_for_user = generate_questions_for_user(
                        &checklist,
                        self.force_stopped,
                        &self.force_stop_reason,
                    );

                    let log_path = self
                        .session_logger
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
                            if let Some(line) = self
                                .lines
                                .iter_mut()
                                .find(|l| l.text.contains("Generating after-action"))
                            {
                                line.status = LogStatus::Success;
                            }
                            self.add_line(
                                &format!("  Report saved: {}", shorten_path(&path)),
                                LogStatus::Success,
                                elapsed,
                            );
                        }
                        Err(e) => {
                            self.add_line(
                                &format!("  Failed to save report: {}", e),
                                LogStatus::Error,
                                elapsed,
                            );
                        }
                    }

                    self.add_line("", LogStatus::Info, elapsed);
                    if self.force_stopped {
                        self.add_line(
                            "Session stopped (see report for details).",
                            LogStatus::Error,
                            elapsed,
                        );
                    } else {
                        self.add_line("Session complete!", LogStatus::Success, elapsed);
                    }
                    self.add_line(
                        &format!("Total iterations: {}", self.iteration),
                        LogStatus::Info,
                        elapsed,
                    );
                    self.add_line(
                        &format!(
                            "Runtime: {:.1} hours",
                            self.start_instant.elapsed().as_secs_f32() / 3600.0
                        ),
                        LogStatus::Info,
                        elapsed,
                    );

                    if let Some(ref logger) = self.session_logger {
                        self.add_line(
                            &format!("Session log: {}", shorten_path(&logger.log_path)),
                            LogStatus::Info,
                            elapsed,
                        );
                    }

                    self.phase = StartupPhase::Complete;
                    self.phase_started = elapsed;
                }
            }

            StartupPhase::Complete => {}
        }
    }

    fn acp_session_id(&self) -> acp::SessionId {
        acp::SessionId::new(self.session_id.clone())
    }

    fn acp_meta(&self, phase: &'static str) -> acp::Meta {
        let mut meta = acp::Meta::new();
        meta.insert(
            crate::ACP_PHASE_META_KEY.to_string(),
            serde_json::Value::String(phase.to_string()),
        );
        meta.insert(
            crate::ACP_SESSION_META_KEY.to_string(),
            serde_json::Value::String(self.session_id.clone()),
        );
        meta
    }

    fn push_acp_text(&mut self, phase: &'static str, text: String) {
        let content =
            acp::ContentBlock::Text(acp::TextContent::new(text).meta(self.acp_meta(phase)));
        let chunk = acp::ContentChunk::new(content);
        let notification = acp::SessionNotification::new(
            self.acp_session_id(),
            acp::SessionUpdate::AgentMessageChunk(chunk),
        );
        self.acp_events.push(notification);
    }

    fn tool_kind_for_name(name: &str) -> acp::ToolKind {
        match name {
            "Read" | "Glob" | "Grep" => acp::ToolKind::Read,
            "Edit" | "Write" => acp::ToolKind::Edit,
            "Bash" => acp::ToolKind::Execute,
            "Task" => acp::ToolKind::Think,
            _ => acp::ToolKind::Other,
        }
    }

    fn push_acp_tool_use(&mut self, phase: &'static str, name: String, params: String) {
        let id = acp::ToolCallId::new(format!("{}-tool-{}", phase, self.acp_tool_counter));
        self.acp_tool_counter = self.acp_tool_counter.saturating_add(1);
        let raw_input = serde_json::from_str(&params)
            .unwrap_or_else(|_| serde_json::Value::String(params.clone()));
        let tool_call = acp::ToolCall::new(id.clone(), name.clone())
            .kind(Self::tool_kind_for_name(&name))
            .status(acp::ToolCallStatus::InProgress)
            .raw_input(raw_input)
            .meta(self.acp_meta(phase));
        self.acp_events.push(acp::SessionNotification::new(
            self.acp_session_id(),
            acp::SessionUpdate::ToolCall(tool_call),
        ));
        self.acp_pending_tools.push(AcpPendingTool {
            id,
            name,
            phase,
            params,
        });
    }

    fn push_acp_tool_done(
        &mut self,
        phase: &'static str,
        name: String,
        output: Option<String>,
        is_error: bool,
    ) {
        let pending_idx = self
            .acp_pending_tools
            .iter()
            .rposition(|tool| tool.name == name && tool.phase == phase);
        let (tool_id, params) = if let Some(idx) = pending_idx {
            let pending = self.acp_pending_tools.remove(idx);
            (pending.id, pending.params)
        } else {
            let id = acp::ToolCallId::new(format!("{}-tool-{}", phase, self.acp_tool_counter));
            self.acp_tool_counter = self.acp_tool_counter.saturating_add(1);
            (id, String::new())
        };

        let mut fields = acp::ToolCallUpdateFields::new();
        let status = if is_error {
            acp::ToolCallStatus::Failed
        } else {
            acp::ToolCallStatus::Completed
        };
        let raw_input = serde_json::from_str(&params)
            .unwrap_or_else(|_| serde_json::Value::String(params.clone()));
        let raw_output = if let Some(output) = output {
            if is_error {
                serde_json::json!({ "error": output })
            } else {
                serde_json::json!({ "content": output })
            }
        } else if is_error {
            serde_json::json!({ "error": "Tool failed." })
        } else {
            serde_json::json!({ "content": "" })
        };
        fields = fields
            .status(status)
            .raw_output(raw_output)
            .raw_input(raw_input);

        let mut meta = self.acp_meta(phase);
        meta.insert(
            crate::ACP_TOOL_NAME_META_KEY.to_string(),
            serde_json::Value::String(name),
        );
        let update = acp::ToolCallUpdate::new(tool_id, fields).meta(meta);
        self.acp_events.push(acp::SessionNotification::new(
            self.acp_session_id(),
            acp::SessionUpdate::ToolCallUpdate(update),
        ));

        let _ = params;
    }

    fn push_acp_tool_progress(
        &mut self,
        phase: &'static str,
        tool_name: String,
        elapsed_secs: f64,
    ) {
        let pending = self
            .acp_pending_tools
            .iter()
            .rposition(|tool| tool.name == tool_name && tool.phase == phase);
        let Some(idx) = pending else {
            return;
        };
        let tool_id = self.acp_pending_tools[idx].id.clone();
        let mut fields = acp::ToolCallUpdateFields::new();
        fields = fields.status(acp::ToolCallStatus::InProgress);
        let mut meta = self.acp_meta(phase);
        meta.insert(
            crate::ACP_TOOL_PROGRESS_META_KEY.to_string(),
            serde_json::Value::from(elapsed_secs),
        );
        meta.insert(
            crate::ACP_TOOL_NAME_META_KEY.to_string(),
            serde_json::Value::String(tool_name),
        );
        let update = acp::ToolCallUpdate::new(tool_id, fields).meta(meta);
        self.acp_events.push(acp::SessionNotification::new(
            self.acp_session_id(),
            acp::SessionUpdate::ToolCallUpdate(update),
        ));
    }

    pub fn update_claude_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self
            .lines
            .iter()
            .position(|l| l.text.contains("Creating plan with Claude"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());

        self.lines.truncate(start_idx);

        let tool_count = self
            .claude_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { .. }))
            .count();
        let done_count = self
            .claude_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. }))
            .count();
        let text_count = self
            .claude_full_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();

        if tool_count > 0 || text_count > 0 {
            self.add_line(
                &format!(
                    "  {} tools ({} done), {} lines output",
                    tool_count, done_count, text_count
                ),
                LogStatus::Thinking,
                elapsed,
            );
        }

        let events = self.claude_events.clone();
        let start = if events.len() > 12 {
            events.len() - 12
        } else {
            0
        };

        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 {
                        lines.len() - 10
                    } else {
                        0
                    };
                    for line in &lines[line_start..] {
                        // Show text without prefix - tool cards handle tool display
                        self.add_line(&format!("  {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { .. } => {
                    // Tool cards handle tool display via SessionEvents, skip here
                }
                ClaudeEvent::ToolProgress { .. } => {
                    // Tool cards handle progress via SessionEvents
                }
            }
        }
    }

    pub fn update_exec_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self
            .lines
            .iter()
            .position(|l| l.text.contains("Executing plan"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());

        self.lines.truncate(start_idx);

        let tool_count = self
            .exec_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { .. }))
            .count();
        let done_count = self
            .exec_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. }))
            .count();
        let text_count = self
            .exec_full_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();

        if tool_count > 0 || text_count > 0 {
            self.add_line(
                &format!(
                    "  {} tools ({} done), {} lines output",
                    tool_count, done_count, text_count
                ),
                LogStatus::Thinking,
                elapsed,
            );
        }

        let events = self.exec_events.clone();
        let start = if events.len() > 12 {
            events.len() - 12
        } else {
            0
        };

        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 {
                        lines.len() - 10
                    } else {
                        0
                    };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { .. } => {
                    // Tool cards handle tool display via SessionEvents
                }
                ClaudeEvent::ToolProgress { .. } => {
                    // Tool cards handle progress via SessionEvents
                }
            }
        }
    }

    pub fn update_review_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self
            .lines
            .iter()
            .position(|l| l.text.contains("Reviewing work"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());

        self.lines.truncate(start_idx);

        let tool_count = self
            .review_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { .. }))
            .count();
        let done_count = self
            .review_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. }))
            .count();
        let text_count = self
            .review_full_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();

        if tool_count > 0 || text_count > 0 {
            self.add_line(
                &format!(
                    "  {} tools ({} done), {} lines output",
                    tool_count, done_count, text_count
                ),
                LogStatus::Thinking,
                elapsed,
            );
        }

        let events = self.review_events.clone();
        let start = if events.len() > 12 {
            events.len() - 12
        } else {
            0
        };

        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 {
                        lines.len() - 10
                    } else {
                        0
                    };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { .. } => {
                    // Tool cards handle tool display via SessionEvents
                }
                ClaudeEvent::ToolProgress { .. } => {
                    // Tool cards handle progress via SessionEvents
                }
            }
        }
    }

    pub fn update_fix_streaming_line(&mut self, elapsed: f32) {
        let start_idx = self
            .lines
            .iter()
            .position(|l| l.text.contains("Fixing verification"))
            .map(|i| i + 1)
            .unwrap_or(self.lines.len());

        self.lines.truncate(start_idx);

        let tool_count = self
            .fix_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { .. }))
            .count();
        let done_count = self
            .fix_events
            .iter()
            .filter(|e| matches!(e, ClaudeEvent::Tool { done: true, .. }))
            .count();
        let text_count = self
            .fix_full_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();

        if tool_count > 0 || text_count > 0 {
            self.add_line(
                &format!(
                    "  {} tools ({} done), {} lines output",
                    tool_count, done_count, text_count
                ),
                LogStatus::Thinking,
                elapsed,
            );
        }

        let events = self.fix_events.clone();
        let start = if events.len() > 12 {
            events.len() - 12
        } else {
            0
        };

        for event in &events[start..] {
            match event {
                ClaudeEvent::Text(text) => {
                    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                    let line_start = if lines.len() > 10 {
                        lines.len() - 10
                    } else {
                        0
                    };
                    for line in &lines[line_start..] {
                        self.add_line(&format!("  {}", line), LogStatus::Thinking, elapsed);
                    }
                }
                ClaudeEvent::Tool { .. } => {
                    // Tool cards handle tool display via SessionEvents
                }
                ClaudeEvent::ToolProgress { .. } => {
                    // Tool cards handle progress via SessionEvents
                }
            }
        }
    }

    fn display_preflight_results(&mut self, config: &PreflightConfig, elapsed: f32) {
        if let Some(ref git) = config.git {
            if let Some(ref branch) = git.branch {
                self.add_line(
                    &format!("  Git: {} branch", branch),
                    LogStatus::Success,
                    elapsed,
                );
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

        // Inference backends are shown in the DetectingCompute phase
        // with more detailed info (endpoints, models), so skip here

        // Tools section (separate from Preflight for cleaner grouping)
        self.add_line_to_section("Tools:", LogStatus::Info, elapsed, StartupSection::Tools);

        if let Some(ref claude) = config.tools.claude {
            self.add_line_to_section(
                &format!("  [OK] claude: {}", shorten_path(&claude.path)),
                LogStatus::Success,
                elapsed,
                StartupSection::Tools,
            );
        }
        if let Some(ref codex) = config.tools.codex {
            self.add_line_to_section(
                &format!("  [OK] codex: {}", shorten_path(&codex.path)),
                LogStatus::Success,
                elapsed,
                StartupSection::Tools,
            );
        }
        if let Some(ref opencode) = config.tools.opencode {
            self.add_line_to_section(
                &format!("  [OK] opencode: {}", shorten_path(&opencode.path)),
                LogStatus::Success,
                elapsed,
                StartupSection::Tools,
            );
        }
    }

    /// Create a checkpoint from the current state.
    ///
    /// The cursors are provided by the runtime, since they track event delivery.
    pub fn create_checkpoint(
        &self,
        acp_cursor: usize,
        working_dir: PathBuf,
    ) -> SessionCheckpoint {
        let elapsed = self.start_instant.elapsed().as_secs_f32();

        SessionCheckpoint {
            version: crate::checkpoint::CHECKPOINT_VERSION,
            session_id: self.session_id.clone(),
            checkpoint_time: Local::now(),
            original_start_time: self.start_time,
            phase: self.phase,
            phase_started_offset: elapsed - self.phase_started,
            iteration: self.iteration,
            model: self.model,
            // Claude SDK session IDs captured from SDK responses
            claude_session_id: self.claude_session_id.clone(),
            exec_session_id: self.exec_session_id.clone(),
            review_session_id: self.review_session_id.clone(),
            fix_session_id: self.fix_session_id.clone(),
            // Events
            claude_events: self.claude_events.clone(),
            claude_full_text: self.claude_full_text.clone(),
            exec_events: self.exec_events.clone(),
            exec_full_text: self.exec_full_text.clone(),
            review_events: self.review_events.clone(),
            review_full_text: self.review_full_text.clone(),
            fix_events: self.fix_events.clone(),
            fix_full_text: self.fix_full_text.clone(),
            // Cursors
            plan_cursor: 0,
            exec_cursor: 0,
            review_cursor: 0,
            fix_cursor: 0,
            acp_cursor,
            acp_events: self.acp_events.clone(),
            acp_tool_counter: self.acp_tool_counter,
            // State
            lines: self.lines.clone(),
            plan_path: self.plan_path.clone(),
            last_checklist: self.last_checklist.clone(),
            working_dir,
            force_stopped: self.force_stopped,
            force_stop_reason: self.force_stop_reason.clone(),
        }
    }

    /// Restore state from a checkpoint.
    ///
    /// Note: This recreates the state machine but cannot restore mpsc receivers.
    /// The caller must re-establish streaming connections if needed.
    pub fn from_checkpoint(cp: SessionCheckpoint) -> Self {
        let session_logger = SessionLogger::new(&cp.session_id).ok();
        let start_instant = Instant::now();

        Self {
            lines: cp.lines,
            phase: cp.phase,
            phase_started: start_instant.elapsed().as_secs_f32() - cp.phase_started_offset,
            preflight_config: None, // Must be re-run or cached separately
            model: cp.model,
            issue_summary: None, // Could be saved in checkpoint if needed
            claude_receiver: None, // Cannot persist channels
            claude_session_id: cp.claude_session_id,
            claude_events: cp.claude_events,
            claude_full_text: cp.claude_full_text,
            acp_events: cp.acp_events,
            plan_path: cp.plan_path,
            exec_receiver: None,
            exec_session_id: cp.exec_session_id,
            exec_events: cp.exec_events,
            exec_full_text: cp.exec_full_text,
            review_receiver: None,
            review_session_id: cp.review_session_id,
            review_events: cp.review_events,
            review_full_text: cp.review_full_text,
            iteration: cp.iteration,
            session_logger,
            session_id: cp.session_id,
            start_time: cp.original_start_time,
            start_instant,
            verification_runner: None,
            last_checklist: cp.last_checklist,
            fix_receiver: None,
            fix_session_id: cp.fix_session_id,
            fix_events: cp.fix_events,
            fix_full_text: cp.fix_full_text,
            acp_pending_tools: Vec::new(),
            acp_tool_counter: cp.acp_tool_counter,
            force_stopped: cp.force_stopped,
            force_stop_reason: cp.force_stop_reason,
            report_path: None,
            compute_mix: None,
            pylon_started: false,
            session_usage: ClaudeUsageData::default(), // Not persisted in checkpoint yet
            user_prompt: None, // Not persisted in checkpoint yet
            dspy_plan: None, // Not persisted in checkpoint yet
        }
    }

    /// Check if the current phase is resumable.
    ///
    /// Some phases (like mid-stream) may require restarting from the phase beginning.
    pub fn is_phase_resumable(&self) -> bool {
        matches!(
            self.phase,
            StartupPhase::PlanningWithClaude
                | StartupPhase::ExecutingPlan
                | StartupPhase::ReviewingWork
                | StartupPhase::FixingVerificationFailures
                | StartupPhase::Complete
        )
    }
}

impl Default for StartupState {
    fn default() -> Self {
        Self::new()
    }
}
