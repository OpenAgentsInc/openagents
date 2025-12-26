//! CLI structure and command definitions for autopilot

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "autopilot")]
#[command(about = "Run autonomous tasks with Claude and log trajectories")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a task and log the trajectory
    Run {
        /// The task/prompt to execute
        #[arg(required = true)]
        prompt: String,

        /// Project name (loads cwd and issues_db from project)
        #[arg(short, long)]
        project: Option<String>,

        /// Working directory (default: current directory or from project)
        #[arg(short, long)]
        cwd: Option<PathBuf>,

        /// Agent to use (claude, codex, gpt-oss, or fm-bridge)
        #[arg(long, default_value = "claude")]
        agent: String,

        /// Model to use (sonnet, opus, haiku, or full model ID)
        #[arg(short, long, default_value_t = default_model())]
        model: String,

        /// Maximum turns
        #[arg(long, default_value_t = default_max_turns())]
        max_turns: u32,

        /// Maximum budget in USD
        #[arg(long, default_value_t = default_max_budget())]
        max_budget: f64,

        /// Output directory for logs (default: docs/logs/YYYYMMDD/)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,

        /// Custom slug for filename (auto-generated if not provided)
        #[arg(long)]
        slug: Option<String>,

        /// Skip saving output files (just stream to stdout)
        #[arg(long)]
        dry_run: bool,

        /// Verbose output (show all messages)
        #[arg(short, long)]
        verbose: bool,

        /// Enable issue tracking tools via MCP
        #[arg(long)]
        with_issues: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long, env = "ISSUES_DB")]
        issues_db: Option<PathBuf>,

        /// Full auto mode: continuously work on issues and discover new work
        #[arg(long, default_value_t = default_full_auto())]
        full_auto: bool,

        /// Disable APM (Actions Per Minute) tracking
        #[arg(long)]
        no_apm: bool,

        /// Publish trajectory to Nostr relays (NIP-SA kind:38030/38031)
        #[arg(long)]
        publish_trajectory: bool,
    },
    /// Replay a saved trajectory for debugging
    Replay {
        /// Path to trajectory JSON file
        #[arg(required = true)]
        trajectory: PathBuf,

        /// View mode: interactive (default), list, or summary
        #[arg(short, long, default_value = "interactive")]
        mode: String,
    },
    /// Compare two trajectories side-by-side
    Compare {
        /// Path to first trajectory JSON file
        #[arg(required = true)]
        trajectory1: PathBuf,

        /// Path to second trajectory JSON file
        #[arg(required = true)]
        trajectory2: PathBuf,
    },
    /// Analyze trajectory metrics
    Analyze {
        /// Path to trajectory JSON file or directory
        #[arg(required = true)]
        path: PathBuf,

        /// Aggregate metrics across all files in directory
        #[arg(long)]
        aggregate: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Resume a previous session
    Resume {
        /// Path to .json or .rlog trajectory file
        #[arg(required_unless_present = "continue_last")]
        trajectory: Option<PathBuf>,

        /// Continue most recent session (no file needed)
        #[arg(long, short = 'c')]
        continue_last: bool,

        /// Working directory (default: from trajectory or current)
        #[arg(short = 'd', long)]
        cwd: Option<PathBuf>,

        /// Additional prompt to send on resume
        #[arg(short, long)]
        prompt: Option<String>,

        /// Maximum budget in USD
        #[arg(long, default_value_t = default_max_budget())]
        max_budget: f64,

        /// Enable issue tracking tools via MCP
        #[arg(long)]
        with_issues: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long, env = "ISSUES_DB")]
        issues_db: Option<PathBuf>,
    },
    /// Manage issues
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// Manage projects
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// View sessions
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    /// Manage directives
    Directive {
        #[command(subcommand)]
        command: DirectiveCommands,
    },
    /// Manage metrics
    Metrics {
        #[command(subcommand)]
        command: MetricsCommands,
    },
    /// APM (Actions Per Minute) tracking and statistics
    Apm {
        #[command(subcommand)]
        command: ApmCommands,
    },
    /// Manage trajectory logs
    Logs {
        #[command(subcommand)]
        command: LogsCommands,
    },
    /// Send notification alerts
    Notify {
        /// Notification title
        #[arg(short, long)]
        title: String,

        /// Notification message
        #[arg(short, long)]
        message: String,

        /// Severity level (info, warning, error, critical)
        #[arg(short, long, default_value = "info")]
        severity: String,

        /// Webhook URL (can be specified multiple times)
        #[arg(short, long)]
        webhook: Vec<String>,

        /// Path to notification config file (default: ~/.openagents/notifications.toml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Additional metadata as key=value pairs
        #[arg(long)]
        metadata: Vec<String>,
    },
    /// Manage GitHub repository connections
    Github {
        #[command(subcommand)]
        command: GithubCommands,
    },
}

#[derive(Subcommand)]
pub enum IssueCommands {
    /// List issues
    List {
        /// Filter by status (open, in_progress, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List auto-created issues (from anomaly detection)
    ListAuto {
        /// Filter by status (open, in_progress, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Create a new issue
    Create {
        /// Issue title
        #[arg(required = true)]
        title: String,

        /// Issue description
        #[arg(short, long)]
        description: Option<String>,

        /// Priority (urgent, high, medium, low)
        #[arg(short, long, default_value = "medium")]
        priority: String,

        /// Issue type (task, bug, feature)
        #[arg(short = 't', long, default_value = "task")]
        issue_type: String,

        /// Agent to assign (claude or codex)
        #[arg(short, long, default_value = "claude")]
        agent: String,

        /// Directive to link to (e.g., d-001)
        #[arg(long)]
        directive: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Claim an issue
    Claim {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Run ID (default: manual-<timestamp>)
        #[arg(short, long)]
        run_id: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Mark an issue as complete
    Complete {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Block an issue
    Block {
        /// Issue number
        #[arg(required = true)]
        number: i32,

        /// Reason for blocking
        #[arg(required = true)]
        reason: String,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Release stale in_progress issues back to open
    Release {
        /// Minutes after which an issue is considered stale (default: 60)
        #[arg(short, long, default_value = "60")]
        stale_minutes: i32,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Get the next ready issue
    Ready {
        /// Filter by agent (claude or codex)
        #[arg(short, long)]
        agent: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Export issues to JSON
    Export {
        /// Output file path (default: .openagents/issues.json)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include completed issues
        #[arg(long)]
        include_completed: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Import issues from JSON
    Import {
        /// Input file path (default: .openagents/issues.json)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Force update existing issues with same UUID
        #[arg(long)]
        force: bool,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Add a new project
    Add {
        /// Project name
        #[arg(required = true)]
        name: String,

        /// Project path
        #[arg(short, long, required = true)]
        path: PathBuf,

        /// Project description
        #[arg(short, long)]
        description: Option<String>,

        /// Default model (sonnet, opus, haiku)
        #[arg(short, long)]
        model: Option<String>,

        /// Default budget in USD
        #[arg(short, long)]
        budget: Option<f64>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List all projects
    List {
        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Remove a project
    Remove {
        /// Project name
        #[arg(required = true)]
        name: String,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum SessionCommands {
    /// List sessions
    List {
        /// Filter by project name
        #[arg(short, long)]
        project: Option<String>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show session details
    Show {
        /// Session ID (or prefix)
        #[arg(required = true)]
        id: String,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum DirectiveCommands {
    /// List all directives
    List {
        /// Filter by status (active, paused, completed)
        #[arg(short, long)]
        status: Option<String>,

        /// Path to issues database (for progress calculation)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show directive details
    Show {
        /// Directive ID (e.g., 'd-001')
        #[arg(required = true)]
        id: String,

        /// Path to issues database (for progress calculation)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Create a new directive
    Create {
        /// Directive ID (e.g., 'd-001')
        #[arg(required = true)]
        id: String,

        /// Directive title
        #[arg(required = true)]
        title: String,

        /// Priority (urgent, high, medium, low)
        #[arg(short, long, default_value = "medium")]
        priority: String,
    },
    /// Pause a directive
    Pause {
        /// Directive ID
        #[arg(required = true)]
        id: String,
    },
    /// Complete a directive
    Complete {
        /// Directive ID
        #[arg(required = true)]
        id: String,
    },
    /// Resume a paused directive
    Resume {
        /// Directive ID
        #[arg(required = true)]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum MetricsCommands {
    /// Import metrics from trajectory logs
    Import {
        /// Directory containing trajectory logs (default: docs/logs/YYYYMMDD)
        #[arg(required = true)]
        log_dir: PathBuf,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Backfill metrics from all existing trajectory logs in docs/logs/
    Backfill {
        /// Root logs directory (default: docs/logs/)
        #[arg(long, default_value = "docs/logs")]
        logs_root: PathBuf,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show detailed metrics for a session
    Show {
        /// Session ID
        #[arg(required = true)]
        session_id: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show quick statistics for a session (concise view)
    Stats {
        /// Session ID (default: most recent session)
        session_id: Option<String>,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List all recorded sessions
    List {
        /// Filter by status (completed, crashed, budget_exhausted, max_turns, running)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by issue number
        #[arg(long)]
        issue: Option<i32>,

        /// Filter by directive ID (e.g., d-004)
        #[arg(long)]
        directive: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Analyze aggregate metrics and detect regressions
    Analyze {
        /// Time period (7d, 30d, last-week, this-week)
        #[arg(short, long, default_value = "7d")]
        period: String,

        /// Compare two date ranges (format: YYYY-MM-DD..YYYY-MM-DD)
        #[arg(long)]
        compare: Option<String>,

        /// Filter by issue number
        #[arg(long)]
        issue: Option<i32>,

        /// Filter by directive ID (e.g., d-004)
        #[arg(long)]
        directive: Option<String>,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Show only high error rate sessions (>10% tool errors)
        #[arg(long)]
        errors: bool,

        /// Show detected anomalies (>2 std dev from baseline)
        #[arg(long)]
        anomalies: bool,
    },
    /// Show trends by comparing periods
    Trends {
        /// Recent period (7d, 30d, last-week, this-week)
        #[arg(short = 'r', long, default_value = "this-week")]
        recent: String,

        /// Baseline period (7d, 30d, last-week, this-week)
        #[arg(short = 'b', long, default_value = "last-week")]
        baseline: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Automatically create improvement issues from detected anomalies
    CreateIssues {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long, env = "ISSUES_DB")]
        issues_db: Option<PathBuf>,

        /// Dry run - show what would be created without creating issues
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate automated weekly trend report
    Report {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Output directory for report (default: docs/autopilot/reports)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run learning pipeline to analyze sessions and propose improvements
    Learn {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        metrics_db: Option<PathBuf>,

        /// Specific session IDs to analyze (default: last 50 sessions)
        #[arg(long)]
        sessions: Vec<String>,

        /// Number of recent sessions to analyze if no specific sessions provided
        #[arg(long, default_value_t = 50)]
        limit: usize,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Export metrics to JSON/CSV for external analysis
    Export {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Time period (7d, 30d, last-week, this-week, or 'all')
        #[arg(short, long, default_value = "all")]
        period: String,

        /// Output format (json or csv)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Backfill APM data for existing sessions
    BackfillApm {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Recommend which model to use for a task
    RecommendModel {
        /// Issue title
        #[arg(required = true)]
        title: String,

        /// Issue description or full task description
        #[arg(short, long, default_value = "")]
        description: String,

        /// Directive ID (e.g., d-004) for context
        #[arg(long)]
        directive: Option<String>,

        /// Prefer cost-optimized recommendations
        #[arg(long)]
        prefer_cost: bool,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (text or json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Backfill metrics from existing trajectory logs in docs/logs/
    BackfillFromLogs {
        /// Directory containing trajectory logs (default: docs/logs/)
        #[arg(long, default_value = "docs/logs")]
        logs_dir: PathBuf,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Update baseline metrics from session data for regression detection
    UpdateBaselines {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Minimum number of samples required to calculate baseline (default: 10)
        #[arg(long, default_value_t = 10)]
        min_samples: usize,
    },

    /// Alert management commands
    #[command(subcommand)]
    Alerts(AlertCommands),

    /// Show aggregate metrics for a specific issue
    IssueMetrics {
        /// Issue number
        #[arg(required = true)]
        issue_number: i32,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show aggregate metrics for a specific directive
    DirectiveMetrics {
        /// Directive ID (e.g., d-004)
        #[arg(required = true)]
        directive_id: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List all issues with their aggregate metrics
    ByIssue {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// List all directives with their aggregate metrics
    ByDirective {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show improvement velocity over time
    Velocity {
        /// Time period to analyze (7d, 30d, this-week, last-week)
        #[arg(short, long, default_value = "this-week")]
        period: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Number of historical snapshots to show
        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        /// Threshold for celebration (velocity score >)
        #[arg(long, default_value_t = 0.5)]
        celebrate_threshold: f64,

        /// Threshold for progress message (velocity score >)
        #[arg(long, default_value_t = 0.2)]
        progress_threshold: f64,

        /// Threshold for warning message (velocity score <)
        #[arg(long, default_value_t = -0.3)]
        warning_threshold: f64,
    },
}

#[derive(Subcommand)]
pub enum ApmCommands {
    /// Show APM statistics for different time windows
    Stats {
        /// Source to display (autopilot, claude_code, combined)
        #[arg(short, long)]
        source: Option<String>,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List APM sessions
    Sessions {
        /// Source filter (autopilot, claude_code)
        #[arg(short, long)]
        source: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show detailed APM breakdown for a session
    Show {
        /// Session ID
        #[arg(required = true)]
        session_id: String,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Export APM data in various formats
    Export {
        /// Output format (json, csv, tsv)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file path (if not specified, writes to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Source filter (autopilot, claude_code)
        #[arg(short, long)]
        source: Option<String>,

        /// Time window filter (session, 1h, 6h, 1d, 1w, 1m, lifetime)
        #[arg(short, long)]
        window: Option<String>,

        /// Start date filter (RFC3339 or YYYY-MM-DD format)
        #[arg(long)]
        start_date: Option<String>,

        /// End date filter (RFC3339 or YYYY-MM-DD format)
        #[arg(long)]
        end_date: Option<String>,

        /// Include raw event data (not just aggregated snapshots)
        #[arg(long)]
        include_events: bool,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Watch APM stats in real-time
    Watch {
        /// Refresh interval in seconds
        #[arg(short, long, default_value_t = 2)]
        interval: u64,

        /// Source to monitor (autopilot, claude_code)
        #[arg(short, long)]
        source: Option<String>,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Manage APM baselines
    Baseline {
        #[command(subcommand)]
        command: BaselineCommands,
    },
    /// Regenerate all APM snapshots from recorded events
    RegenerateSnapshots {
        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show personal best records
    Best {
        /// Metric to show (apm, velocity_score, or all)
        #[arg(short, long)]
        metric: Option<String>,

        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum BaselineCommands {
    /// List all APM baselines
    List {
        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Set a new APM baseline
    Set {
        /// Baseline ID (e.g., "openagents", "autopilot-default")
        #[arg(required = true)]
        id: String,

        /// Human-readable name
        #[arg(required = true)]
        name: String,

        /// Source type (autopilot, claude_code, combined)
        #[arg(short, long, required = true)]
        source: String,

        /// Median APM value
        #[arg(short, long, required = true)]
        median: f64,

        /// Minimum acceptable APM (optional, defaults to 80% of median)
        #[arg(long)]
        min: Option<f64>,

        /// Maximum APM threshold (optional, defaults to 150% of median)
        #[arg(long)]
        max: Option<f64>,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show baseline details
    Show {
        /// Baseline ID
        #[arg(required = true)]
        id: String,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Delete a baseline
    Delete {
        /// Baseline ID
        #[arg(required = true)]
        id: String,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Check current APM against baseline
    Check {
        /// Baseline ID to check against
        #[arg(required = true)]
        baseline_id: String,

        /// Current APM value (or use latest session if not provided)
        #[arg(short, long)]
        apm: Option<f64>,

        /// Path to database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum AlertCommands {
    /// List all configured alert rules
    List {
        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Add a new alert rule
    Add {
        /// Metric name to monitor
        #[arg(long)]
        metric: String,

        /// Alert type (threshold, regression, rate_of_change)
        #[arg(long)]
        alert_type: String,

        /// Severity (warning, error, critical)
        #[arg(long)]
        severity: String,

        /// Threshold value
        #[arg(long)]
        threshold: f64,

        /// Description of what this alert detects
        #[arg(long)]
        description: String,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Remove an alert rule
    Remove {
        /// Alert rule ID
        rule_id: i64,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Show alert history
    History {
        /// Filter by session ID
        #[arg(long)]
        session: Option<String>,

        /// Filter by metric name
        #[arg(long)]
        metric: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value_t = 50)]
        limit: usize,

        /// Path to metrics database (default: autopilot-metrics.db)
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum LogsCommands {
    /// Show log statistics
    Stats {
        /// Logs directory (default: docs/logs)
        #[arg(short, long)]
        logs_dir: Option<PathBuf>,
    },
    /// Archive old logs (compress to .gz)
    Archive {
        /// Age in days before archiving (default: 30)
        #[arg(short, long, default_value_t = 30)]
        days: i64,

        /// Logs directory (default: docs/logs)
        #[arg(short, long)]
        logs_dir: Option<PathBuf>,

        /// Dry run (show what would be archived)
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean up old archived logs
    Cleanup {
        /// Age in days before deletion (default: 90)
        #[arg(short, long, default_value_t = 90)]
        days: i64,

        /// Logs directory (default: docs/logs)
        #[arg(short, long)]
        logs_dir: Option<PathBuf>,

        /// Path to issues database (default: autopilot.db in workspace root)
        #[arg(long)]
        db: Option<PathBuf>,

        /// Dry run (show what would be deleted)
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum GithubCommands {
    /// Connect a GitHub repository
    Connect {
        /// Repository URL or owner/repo format
        #[arg(required = true)]
        repo: String,

        /// Path to metrics database (for storing connection)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// List connected repositories
    List {
        /// Path to metrics database
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Disconnect a repository
    Disconnect {
        /// Repository full name (owner/repo)
        #[arg(required = true)]
        repo: String,

        /// Path to metrics database
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Show repository status
    Status {
        /// Repository full name (owner/repo)
        #[arg(required = true)]
        repo: String,

        /// Path to metrics database
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

// Default value functions
fn default_model() -> String {
    std::env::var("AUTOPILOT_MODEL").unwrap_or_else(|_| "sonnet".to_string())
}

fn default_full_auto() -> bool {
    std::env::var("AUTOPILOT_FULL_AUTO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(false)
}

fn default_max_turns() -> u32 {
    std::env::var("AUTOPILOT_MAX_TURNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100)
}

fn default_max_budget() -> f64 {
    std::env::var("AUTOPILOT_MAX_BUDGET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5.0)
}
