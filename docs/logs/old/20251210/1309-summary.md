he plan and current project focus, here are the options:

  Option 1: Test HillClimber Improvements (Recommended)

  The smart orchestration harness was just implemented. We should test it against regex-log to see if auto-verify helps the FM iterate toward
  solutions.

  cargo run -p hillclimber -- --tasks regex-log --max-runs 1 --max-turns 15

  Option 2: Continue Phase 4 - LLM Providers

  Add OpenRouter/OpenAI providers to the LLM crate:
  - LLM-001..005: Provider selection, fallback
  - LLM-020..024: Token accounting, cost tracking

  Option 3: Continue Phase 4 - Sandbox

  Implement container isolation for safe agent execution:
  - ORCH-080..082: Sandbox execution
  - CONF-030..033: Sandbox configuration

  Option 4: Build TBCC Dashboard

  TerminalBench Command Center for visualizing runs:
  - TBCC-001..005: Dashboard views
  - TBCC-010..014: Task browser
  - TBCC-020..024: Run browser

  Option 5: Complete CLI

  Add remaining CLI commands:
  - CLI-030..032: Worktree management commands

  ---
  My recommendation: Option 1 - Test the HillClimber improvements first. The whole point of the smart harness was to help reach 100% on
  Terminal-Bench. We should validate it works before building more infrastructure.

  What would you like to do?

> give me a full writeup of what you chagned

⏺ Full Writeup: December 10, 2025 Changes

  Overview

  Today's work implemented ~70 user stories across 4 commits, creating 2 new Rust crates and enhancing the HillClimber orchestration system.

  ---
  Commit 1: Phase 2 - HUD APM Widget & Keyboard Shortcuts

  Commit: 5a792c2df
  Files Changed: 4

  New File: crates/hud/src/apm_widget.rs (402 lines)

  Created an Actions Per Minute (APM) tracking widget inspired by StarCraft:

  /// APM velocity levels with color coding
  pub enum ApmLevel {
      Baseline,  // 0-5 APM (gray) - minimal activity
      Active,    // 5-15 APM (blue) - normal work
      High,      // 15-30 APM (green) - productive flow
      Elite,     // 30+ APM (gold) - peak performance
  }

  /// Tracks session APM metrics
  pub struct ApmState {
      pub current_apm: f32,
      pub session_apm: f32,
      pub peak_apm: f32,
      pub actions_this_minute: u32,
      pub total_actions: u64,
      pub session_start: DateTime<Utc>,
  }

  /// Historical snapshots for comparison
  pub struct ApmSnapshot {
      pub hour_ago: Option<f32>,
      pub six_hours_ago: Option<f32>,
      pub day_ago: Option<f32>,
  }

  /// Compare MechaCoder vs Claude Code efficiency
  pub struct ApmComparison {
      pub mechacoder_apm: f32,
      pub claude_code_apm: f32,
      pub efficiency_ratio: f32,
  }

  The widget renders real-time APM with:
  - Color-coded velocity indicator
  - Session statistics
  - Historical comparisons
  - MechaCoder vs Claude Code efficiency ratio

  Modified: crates/hud/src/graph_view.rs (+271 lines)

  Added keyboard shortcuts using GPUI's action system:

  // New imports
  use gpui::{actions, FocusHandle, Focusable};

  // Action definitions
  actions!(graph_view, [
      SelectAll,      // Cmd+A - select all nodes
      DeselectAll,    // Escape - clear selection
      ZoomIn,         // Cmd+= - zoom in
      ZoomOut,        // Cmd+- - zoom out
      ResetView,      // Cmd+0 - reset to default
      ZoomToFit,      // Fit all nodes in view
  ]);

  // Focusable implementation for keyboard input
  impl Focusable for GraphView {
      fn focus_handle(&self, cx: &App) -> FocusHandle {
          self.focus_handle.clone()
      }
  }

  // Action handlers
  fn handle_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
      self.selected_nodes = self.nodes.keys().cloned().collect();
      cx.notify();
  }

  fn handle_zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
      self.zoom = (self.zoom * 1.2).min(MAX_ZOOM);
      cx.notify();
  }

  Modified: crates/hud/src/lib.rs (+3 lines)

  mod apm_widget;
  pub use apm_widget::*;

  Modified: crates/theme/src/lib.rs (+16 lines)

  Added APM-specific theme colors:

  // APM Widget colors
  pub const APM_WIDGET_BG: Hsla = hsla(220.0 / 360.0, 0.15, 0.12, 1.0);
  pub const APM_WIDGET_BORDER: Hsla = hsla(220.0 / 360.0, 0.20, 0.25, 1.0);

  // APM Level colors
  pub const APM_BASELINE: Hsla = hsla(0.0, 0.0, 0.5, 1.0);      // Gray
  pub const APM_ACTIVE: Hsla = hsla(210.0 / 360.0, 0.7, 0.5, 1.0);   // Blue
  pub const APM_HIGH: Hsla = hsla(120.0 / 360.0, 0.6, 0.45, 1.0);    // Green
  pub const APM_ELITE: Hsla = hsla(45.0 / 360.0, 0.9, 0.55, 1.0);    // Gold

  ---
  Commit 2: Phase 3 - CLI Crate

  Commit: be127e97d
  Files Changed: 10 (8 new, 2 modified)

  New Crate: crates/cli/

  Complete command-line interface for OpenAgents.

  Cargo.toml

  [dependencies]
  tasks.workspace = true
  config.workspace = true
  tools.workspace = true
  llm.workspace = true
  orchestrator.workspace = true
  clap.workspace = true
  tokio.workspace = true
  serde.workspace = true
  serde_json.workspace = true
  colored = "2.1"
  tabled = "0.15"

  src/lib.rs - Main CLI Structure

  /// OpenAgents CLI - Autonomous coding agent toolkit
  #[derive(Parser, Debug)]
  #[command(name = "oa")]
  pub struct Cli {
      #[command(subcommand)]
      pub command: Commands,

      /// Working directory (defaults to current directory)
      #[arg(short, long, global = true)]
      pub workdir: Option<String>,

      /// Output format (text, json)
      #[arg(short, long, global = true, default_value = "text")]
      pub format: OutputFormat,

      /// Verbose output
      #[arg(short, long, global = true)]
      pub verbose: bool,
  }

  #[derive(Subcommand, Debug)]
  pub enum Commands {
      /// Task management commands
      #[command(subcommand)]
      Tasks(TasksCommand),

      /// MechaCoder agent commands
      #[command(subcommand)]
      Mecha(MechaCommand),

      /// Session management commands
      #[command(subcommand)]
      Session(SessionCommand),
  }

  pub enum OutputFormat {
      Text,
      Json,
  }

  src/error.rs - Error Types

  #[derive(Error, Debug)]
  pub enum CliError {
      #[error("Task not found: {0}")]
      TaskNotFound(String),

      #[error("Session not found: {0}")]
      SessionNotFound(String),

      #[error("Invalid argument: {0}")]
      InvalidArgument(String),

      #[error("Configuration error: {0}")]
      ConfigError(String),

      #[error("Task error: {0}")]
      TaskError(#[from] tasks::TaskError),

      #[error("Orchestrator error: {0}")]
      OrchestratorError(#[from] orchestrator::OrchestratorError),
  }

  src/output.rs - Formatting Utilities

  /// Format and print output based on format type
  pub fn print_output<T: Serialize + std::fmt::Display>(value: &T, format: OutputFormat) {
      match format {
          OutputFormat::Text => println!("{}", value),
          OutputFormat::Json => {
              if let Ok(json) = serde_json::to_string_pretty(value) {
                  println!("{}", json);
              }
          }
      }
  }

  pub fn print_success(msg: &str) { println!("{} {}", "✓".green(), msg); }
  pub fn print_error(msg: &str) { eprintln!("{} {}", "✗".red(), msg); }
  pub fn print_warning(msg: &str) { println!("{} {}", "!".yellow(), msg); }
  pub fn print_info(msg: &str) { println!("{} {}", "ℹ".blue(), msg); }

  pub fn format_status(status: &str) -> String {
      match status {
          "ready" => status.green().to_string(),
          "in_progress" => status.yellow().to_string(),
          "blocked" => status.red().to_string(),
          "done" => status.cyan().to_string(),
          _ => status.to_string(),
      }
  }

  pub fn format_duration(seconds: u64) -> String {
      if seconds < 60 { format!("{}s", seconds) }
      else if seconds < 3600 { format!("{}m {}s", seconds / 60, seconds % 60) }
      else { format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60) }
  }

  src/tasks_cmd.rs - Task Management (CLI-001..007)

  #[derive(Subcommand, Debug)]
  pub enum TasksCommand {
      /// List tasks with optional filtering (CLI-001)
      List {
          #[arg(short, long)]
          status: Option<String>,
          #[arg(short, long)]
          priority: Option<String>,
          #[arg(short, long)]
          tag: Option<String>,
          #[arg(long)]
          ready: bool,
          #[arg(short = 'n', long, default_value = "20")]
          limit: usize,
      },

      /// Add a new task (CLI-002)
      Add {
          title: String,
          #[arg(short, long)]
          description: Option<String>,
          #[arg(short, long, default_value = "P1")]
          priority: String,
          #[arg(short, long)]
          tags: Option<String>,
      },

      /// Start a task (CLI-003)
      Start { id: String },

      /// Complete a task (CLI-004)
      Complete { id: String, #[arg(short, long)] notes: Option<String> },

      /// Block a task (CLI-005)
      Block { id: String, reason: String },

      /// Show task details (CLI-006)
      Show { id: String },

      /// Delete a task (CLI-007)
      Delete { id: String, #[arg(short, long)] force: bool },
  }

  Implementation includes:
  - Table output with tabled crate
  - Status/priority parsing with validation
  - Integration with SqliteRepository
  - Color-coded output

  src/mechacoder_cmd.rs - Agent Commands (CLI-010..015)

  #[derive(Subcommand, Debug)]
  pub enum MechaCommand {
      /// Run MechaCoder agent (CLI-010)
      Run {
          #[arg(short, long)]
          task: Option<String>,
          #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
          model: String,
          #[arg(long)]
          safe: bool,           // CLI-012: Safe mode
          #[arg(long)]
          dry_run: bool,        // CLI-013: Dry run
          #[arg(long)]
          max_tasks: Option<usize>,  // CLI-014
          #[arg(long)]
          max_tokens: Option<u64>,
          #[arg(long)]
          max_duration: Option<u64>,
      },

      /// Run parallel agents (CLI-011)
      Parallel {
          #[arg(short, long, default_value = "2")]
          agents: usize,
          #[arg(long)]
          safe: bool,
          #[arg(long)]
          cc_only: bool,
      },

      /// Watch mode (CLI-015)
      Watch {
          #[arg(short, long, default_value = "30")]
          interval: u64,
          #[arg(long)]
          safe: bool,
      },

      /// Show agent status
      Status,
  }

  src/session_cmd.rs - Session Management (CLI-020..026)

  #[derive(Subcommand, Debug)]
  pub enum SessionCommand {
      /// List sessions (CLI-020)
      List {
          #[arg(short = 'n', long, default_value = "10")]
          limit: usize,
          #[arg(long)]
          completed: bool,
          #[arg(long)]
          failed: bool,
      },

      /// Show session details (CLI-021)
      Show { id: String },

      /// Resume a paused session (CLI-022)
      Resume { id: String },

      /// Replay a session (CLI-023)
      Replay {
          id: String,
          #[arg(short, long, default_value = "1.0")]
          speed: f32,
      },

      /// Delete a session (CLI-024)
      Delete { id: String, #[arg(short, long)] force: bool },

      /// Export session data (CLI-025)
      Export {
          id: String,
          #[arg(short, long)]
          output: Option<String>,
          #[arg(short, long, default_value = "json")]
          format: String,  // json, jsonl, atif
      },

      /// Show session statistics (CLI-026)
      Stats {
          #[arg(short, long, default_value = "24h")]
          period: String,
      },
  }

  ---
  Commit 3: Phase 4 - Parallel Crate

  Commit: 2c1d70b2c
  Files Changed: 9 (7 new, 2 modified)

  New Crate: crates/parallel/

  Worktree-based parallel agent execution system.

  src/error.rs - Error Types

  #[derive(Error, Debug)]
  pub enum ParallelError {
      #[error("Git error: {0}")]
      GitError(#[from] git2::Error),

      #[error("Worktree error: {0}")]
      WorktreeError(String),

      #[error("Agent error: {0}")]
      AgentError(String),

      #[error("Merge conflict in {files:?}")]
      MergeConflict { files: Vec<String> },

      #[error("Task assignment error: {0}")]
      TaskAssignmentError(String),

      #[error("All agents failed")]
      AllAgentsFailed,

      #[error("Timeout waiting for agents")]
      Timeout,
  }

  src/worktree.rs - Git Worktree Management (PAR-010..013)

  /// Manages git worktrees for parallel agent execution
  pub struct WorktreeManager {
      repo_path: PathBuf,
      worktree_base: PathBuf,
      active_worktrees: Vec<WorktreeInfo>,
  }

  #[derive(Debug, Clone)]
  pub struct WorktreeInfo {
      pub id: String,
      pub path: PathBuf,
      pub branch: String,
      pub in_use: bool,
  }

  impl WorktreeManager {
      /// Create a new worktree for an agent (PAR-010)
      pub fn create_worktree(&mut self, agent_id: &str) -> ParallelResult<WorktreeInfo> {
          let branch_name = format!("agent/{}", agent_id);
          let worktree_path = self.worktree_base.join(agent_id);

          // Create branch from HEAD
          let repo = Repository::open(&self.repo_path)?;
          let head = repo.head()?;
          let head_commit = head.peel_to_commit()?;
          repo.branch(&branch_name, &head_commit, false)?;

          // Create worktree using git command
          std::process::Command::new("git")
              .current_dir(&self.repo_path)
              .args(["worktree", "add", "-B", &branch_name])
              .arg(&worktree_path)
              .output()?;

          Ok(WorktreeInfo { id, path, branch, in_use: true })
      }

      /// Remove a worktree (PAR-011)
      pub fn remove_worktree(&mut self, agent_id: &str) -> ParallelResult<()>;

      /// Merge worktree changes back to main (PAR-012)
      pub fn merge_to_main(&self, agent_id: &str, commit_message: &str) -> ParallelResult<String>;

      /// Cleanup all worktrees
      pub fn cleanup_all(&mut self) -> ParallelResult<()>;
  }

  src/agent_pool.rs - Agent Pool Management (PAR-001..005)

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AgentState {
      Idle,
      Working,
      Completed,
      Failed,
      ShuttingDown,
  }

  #[derive(Debug, Clone)]
  pub struct AgentConfig {
      pub id: String,
      pub worktree_path: PathBuf,
      pub branch: String,
      pub max_tasks: Option<usize>,
      pub use_claude_code: bool,
  }

  pub struct PoolAgent {
      pub config: AgentConfig,
      pub state: AgentState,
      pub stats: AgentStats,
      pub current_task: Option<String>,
  }

  pub struct AgentPool {
      agents: Arc<RwLock<HashMap<String, PoolAgent>>>,
      max_agents: usize,
      completion_tx: mpsc::Sender<TaskCompletion>,
      completion_rx: Arc<Mutex<mpsc::Receiver<TaskCompletion>>>,
  }

  impl AgentPool {
      /// Get an available agent with load balancing (PAR-002)
      pub async fn get_available_agent(&self) -> Option<String> {
          let agents = self.agents.read().await;
          agents.iter()
              .filter(|(_, a)| a.is_available())
              .min_by_key(|(_, a)| a.stats.tasks_completed)  // Least loaded
              .map(|(id, _)| id.clone())
      }

      /// Assign task to agent
      pub async fn assign_task(&self, agent_id: &str, task_id: &str) -> ParallelResult<()>;

      /// Report task completion
      pub async fn report_completion(&self, completion: TaskCompletion) -> ParallelResult<()>;

      /// Get pool statistics (PAR-005)
      pub async fn stats(&self) -> PoolStats;
  }

  #[derive(Debug, Clone, Default)]
  pub struct PoolStats {
      pub total_agents: usize,
      pub idle_agents: usize,
      pub working_agents: usize,
      pub completed_agents: usize,
      pub failed_agents: usize,
      pub total_tasks_completed: usize,
      pub total_tasks_failed: usize,
      pub total_tokens_used: u64,
  }

  src/orchestrator.rs - Parallel Orchestrator

  #[derive(Debug, Clone)]
  pub struct ParallelConfig {
      pub max_agents: usize,
      pub max_tasks_per_agent: Option<usize>,
      pub max_total_tasks: Option<usize>,
      pub max_tokens_per_agent: Option<u64>,
      pub max_duration_secs: Option<u64>,
      pub auto_merge: bool,
      pub use_claude_code: bool,
      pub safe_mode: bool,
      pub dry_run: bool,
  }

  pub struct ParallelOrchestrator {
      config: ParallelConfig,
      working_dir: PathBuf,
      worktree_manager: Arc<RwLock<WorktreeManager>>,
      agent_pool: Arc<AgentPool>,
      task_repo: Arc<dyn TaskRepository>,
      state: Arc<RwLock<ParallelState>>,
  }

  impl ParallelOrchestrator {
      /// Initialize agents and worktrees (PAR-001, PAR-010)
      pub async fn initialize(&mut self) -> ParallelResult<()> {
          for i in 0..self.config.max_agents {
              let agent_id = format!("agent-{}", i);
              let worktree = self.worktree_manager.write().await
                  .create_worktree(&agent_id)?;

              self.agent_pool.add_agent(AgentConfig {
                  id: agent_id,
                  worktree_path: worktree.path,
                  branch: worktree.branch,
                  max_tasks: self.config.max_tasks_per_agent,
                  use_claude_code: self.config.use_claude_code,
              }).await?;
          }
          Ok(())
      }

      /// Run parallel execution (PAR-002, PAR-004)
      pub async fn run(&mut self) -> ParallelResult<ParallelResult_> {
          // Main loop: assign tasks to available agents
          loop {
              if self.should_stop().await { break; }

              let ready_tasks = self.task_repo.ready_tasks(TaskFilter::default())?;

              for task in ready_tasks {
                  if let Some(agent_id) = self.agent_pool.get_available_agent().await {
                      self.task_repo.start(&task.id)?;
                      self.agent_pool.assign_task(&agent_id, &task.id).await?;
                      // Spawn agent execution...
                  }
              }
          }

          // Merge completed work if configured (PAR-012)
          if self.config.auto_merge {
              self.merge_all().await?;
          }

          self.cleanup().await?;
          Ok(result)
      }
  }

  ---
  Commit 4: Smart Orchestration Harness

  Commit: 23b09c2b8
  Files Changed: 5

  Problem Solved

  The Apple FM is too weak to follow workflow instructions. Despite explicit prompts saying "MUST call verify_progress after EVERY write_file", it
   doesn't. It writes files but never verifies, so it never gets feedback to improve.

  Solution: Deterministic Workflow Enforcement

  Instead of relying on the FM to choose tools correctly, the harness now enforces the workflow.

  Modified: crates/hillclimber/src/orchestrator.rs (+154 lines)

  Added auto-verify after every successful write_file:

  // Inside main execution loop, after execute_action returns:

  // Auto-run verify_progress after successful write_file
  if action.tool_name == "write_file" && result.success {
      tracing::debug!("Auto-verifying after write_file");

      let auto_verify = FMAction {
          tool_name: "verify_progress".to_string(),
          tool_args: serde_json::json!({}),
          reasoning: Some("Automatic post-write verification".to_string()),
      };

      let verify_result = self.execute_action(&auto_verify, &task.verification).await?;

      // Format for history
      let verify_display = format_action_for_display("verify_progress", &serde_json::json!({}));
      let verify_summary = if verify_result.success {
          format!("OK: {}", &verify_result.output[..verify_result.output.len().min(200)])
      } else {
          format!("FAILED: {}", verify_result.output)
      };
      state.previous_actions.push(format!("{} -> {}", verify_display, verify_summary));

      // Parse and handle verification result
      if let Some(eval) = self.parse_verification_result(&verify_result) {
          self.emitter.on_verify_complete(eval.tests_passing, eval.tests_total, eval.progress);

          if eval.progress > state.best_progress {
              state.best_progress = eval.progress;
              state.turns_since_improvement = 0;
          } else {
              state.turns_since_improvement += 1;
          }

          state.last_evaluation = Some(eval.clone());

          // Check for completion
          if eval.passed {
              return Ok(MAPOrchestratorResult {
                  passed: true,
                  progress: 1.0,
                  turns: state.total_turns,
                  final_files: state.modified_files.clone(),
                  evaluations: vec![],
              });
          }
      }
  }

  Modified: crates/hillclimber/src/monitor.rs (+106 lines)

  Added tool sequencing rules:

  /// Enforce tool sequencing rules
  fn check_tool_sequence(ctx: &ActionContext) -> Option<MonitorDecision> {
      if ctx.previous_actions.is_empty() {
          return None;
      }

      let last_action = ctx.previous_actions.last().unwrap();

      // Rule 1: Can't call write_file twice without verify in between
      if ctx.tool_name == "write_file" && last_action.contains("write_file") {
          if !last_action.contains("verify_progress") && !last_action.contains("AUTO_VERIFY") {
              return Some(MonitorDecision::deny_with_suggestion(
                  "Cannot write_file twice without verifying".to_string(),
                  "Call verify_progress() to check your previous changes first".to_string(),
              ));
          }
      }

      // Rule 2: Can't read_file more than twice for same path
      if ctx.tool_name == "read_file" {
          let path = ctx.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
          let read_count = ctx.previous_actions.iter()
              .filter(|a| a.contains("read_file") && a.contains(path))
              .count();

          if read_count >= 2 {
              return Some(MonitorDecision::deny_with_suggestion(
                  format!("Already read {} twice", path),
                  "The file either doesn't exist or you've seen its contents. Try write_file instead.".to_string(),
              ));
          }
      }

      None
  }

  // Added to VALIDATION_RULES as first item for priority
  const VALIDATION_RULES: &[ValidationRule] = &[
      check_tool_sequence,        // NEW - highest priority
      check_workspace_bounds,
      check_dangerous_commands,
      check_repetition,
      check_test_before_submit,
  ];

  Modified: crates/hillclimber/src/prompt.rs (+128 lines)

  Updated workflow hints to be clearer:

  // Enhanced workflow section
  fn build_workflow_hints() -> String {
      r#"## Workflow

  1. Write your solution to the target file
  2. AUTOMATIC: verify_progress runs after every write
  3. Read test feedback and improve
  4. Repeat until all tests pass

  The harness automatically verifies after every write - you just need to:
  - Write good solutions
  - Read the test feedback
  - Iterate based on failures"#.to_string()
  }

  Modified: crates/hillclimber/src/decomposer.rs (+19 lines)

  Updated subtask generation hints.

  Modified: crates/commander/src/main.rs (+149 lines)

  Added verbose logging and debug output for HillClimber runs.

  ---
  Summary Statistics

  | Metric                   | Value             |
  |--------------------------|-------------------|
  | Total Commits            | 4                 |
  | New Files Created        | 15                |
  | Files Modified           | 9                 |
  | Lines Added              | ~2,900            |
  | New Tests                | 21                |
  | User Stories Implemented | ~70               |
  | New Crates               | 2 (cli, parallel) |

  Test Coverage

  | Crate    | Tests |
  |----------|-------|
  | cli      | 7     |
  | parallel | 7     |
  | hud      | 71    |
  | testgen  | 43    |

  All tests passing.
