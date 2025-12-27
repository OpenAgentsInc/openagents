use std::path::PathBuf;
use std::sync::mpsc;
use chrono::Local;
use tracing::{info, warn, debug};

use crate::auth;
use crate::claude::{ClaudeToken, ClaudeEvent, run_claude_planning};
use crate::preflight::PreflightConfig;
use crate::streaming::{StreamToken, query_issue_summary, stream_gpt_oss_analysis, parse_harmony_stream, extract_final_content};
use crate::utils::shorten_path;

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

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum StartupPhase {
    CheckingOpenCode,
    CheckingOpenAgents,
    CopyingAuth,
    AuthComplete,
    RunningPreflight,
    PreflightComplete,
    AnalyzingIssues,
    StreamingAnalysis,
    PlanningWithClaude,
    StreamingClaudePlan,
    WritingPlan,
    Complete,
}

pub struct StartupState {
    pub lines: Vec<LogLine>,
    pub phase: StartupPhase,
    pub phase_started: f32,
    pub preflight_config: Option<PreflightConfig>,
    stream_receiver: Option<mpsc::Receiver<StreamToken>>,
    gpt_oss_buffer: String,
    issue_summary: Option<String>,
    gpt_oss_assessment: Option<String>,
    claude_receiver: Option<mpsc::Receiver<ClaudeToken>>,
    pub claude_events: Vec<ClaudeEvent>,
    pub claude_full_text: String,
    pub plan_path: Option<PathBuf>,
}

impl StartupState {
    pub fn new() -> Self {
        Self {
            lines: vec![],
            phase: StartupPhase::CheckingOpenCode,
            phase_started: 0.0,
            preflight_config: None,
            stream_receiver: None,
            gpt_oss_buffer: String::new(),
            issue_summary: None,
            gpt_oss_assessment: None,
            claude_receiver: None,
            claude_events: Vec::new(),
            claude_full_text: String::new(),
            plan_path: None,
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
                    self.add_line("Creating plan with Claude...", LogStatus::Pending, elapsed);

                    let assessment = self.gpt_oss_assessment.clone().unwrap_or_default();
                    let issue_summary = self.issue_summary.clone().unwrap_or_default();
                    let cwd = std::env::current_dir().unwrap_or_default();
                    
                    let (tx, rx) = mpsc::channel();
                    self.claude_receiver = Some(rx);

                    std::thread::spawn(move || {
                        run_claude_planning(&cwd, &issue_summary, &assessment, tx);
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
                self.add_line("Planning complete. Review plan and run autopilot to execute.", LogStatus::Success, elapsed);
                self.phase = StartupPhase::Complete;
                self.phase_started = elapsed;
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
                    let params_display = if params.len() > 42 { 
                        format!("{}...", &params[..39]) 
                    } else { 
                        params.clone() 
                    };
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
