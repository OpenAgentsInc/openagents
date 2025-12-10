//! Session management CLI commands
//!
//! Implements CLI-020..026 user stories

use crate::{CliError, CliResult, OutputFormat, output::*};
use clap::Subcommand;
use colored::Colorize;
use serde::Serialize;
use std::sync::Arc;

use orchestrator::{InMemorySessionStore, SessionStore, SessionSummary};

/// Session management commands
#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// List sessions (CLI-020)
    List {
        /// Maximum number of sessions to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,

        /// Show only completed sessions
        #[arg(long)]
        completed: bool,

        /// Show only failed sessions
        #[arg(long)]
        failed: bool,
    },

    /// Show session details (CLI-021)
    Show {
        /// Session ID
        id: String,
    },

    /// Resume a paused session (CLI-022)
    Resume {
        /// Session ID
        id: String,
    },

    /// Replay a session (read-only) (CLI-023)
    Replay {
        /// Session ID
        id: String,

        /// Replay speed (1.0 = realtime)
        #[arg(short, long, default_value = "1.0")]
        speed: f32,
    },

    /// Delete a session (CLI-024)
    Delete {
        /// Session ID
        id: String,

        /// Force delete without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Export session data (CLI-025)
    Export {
        /// Session ID
        id: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Export format (json, jsonl, atif)
        #[arg(short, long, default_value = "json")]
        format: String,
    },

    /// Show session statistics (CLI-026)
    Stats {
        /// Time period (1h, 6h, 24h, 7d, 30d, all)
        #[arg(short, long, default_value = "24h")]
        period: String,
    },
}

/// Session list output
#[derive(Serialize)]
struct SessionListOutput {
    sessions: Vec<SessionSummaryOutput>,
    total: usize,
}

impl std::fmt::Display for SessionListOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use tabled::{Table, settings::Style};

        if self.sessions.is_empty() {
            return writeln!(f, "No sessions found");
        }

        let table = Table::new(&self.sessions)
            .with(Style::rounded())
            .to_string();

        writeln!(f, "{}", table)?;
        writeln!(f, "\nTotal: {} sessions", self.total)
    }
}

/// Session summary for table display
#[derive(Serialize, tabled::Tabled)]
struct SessionSummaryOutput {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "Tasks")]
    tasks: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(rename = "Tokens")]
    tokens: String,
    #[tabled(rename = "Cost")]
    cost: String,
}

impl From<&SessionSummary> for SessionSummaryOutput {
    fn from(s: &SessionSummary) -> Self {
        Self {
            id: truncate(&s.id, 12),
            state: format_status(&format!("{:?}", s.state).to_lowercase()),
            tasks: format!("{}/{}", s.tasks_completed, s.tasks_completed + s.tasks_failed),
            duration: format_duration(s.duration_secs),
            tokens: format_tokens(s.tokens_used),
            cost: format!("${:.4}", s.estimated_cost_usd),
        }
    }
}

/// Session detail output
#[derive(Serialize)]
struct SessionDetailOutput {
    id: String,
    state: String,
    started_at: String,
    ended_at: Option<String>,
    duration_secs: u64,
    tasks_completed: usize,
    tasks_failed: usize,
    tool_calls: usize,
    tokens_used: u64,
    estimated_cost: f64,
    last_error: Option<String>,
}

impl std::fmt::Display for SessionDetailOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", "Session Details".bold())?;
        writeln!(f, "─────────────────────────────────")?;
        writeln!(f, "{:16} {}", "ID:".dimmed(), self.id)?;
        writeln!(f, "{:16} {}", "State:".dimmed(), format_status(&self.state))?;
        writeln!(f, "{:16} {}", "Started:".dimmed(), self.started_at)?;

        if let Some(ref ended) = self.ended_at {
            writeln!(f, "{:16} {}", "Ended:".dimmed(), ended)?;
        }

        writeln!(f, "{:16} {}", "Duration:".dimmed(), format_duration(self.duration_secs))?;
        writeln!(f, "{:16} {}", "Tasks Done:".dimmed(), self.tasks_completed.to_string().green())?;
        writeln!(f, "{:16} {}", "Tasks Failed:".dimmed(),
            if self.tasks_failed > 0 {
                self.tasks_failed.to_string().red()
            } else {
                self.tasks_failed.to_string().normal()
            })?;
        writeln!(f, "{:16} {}", "Tool Calls:".dimmed(), self.tool_calls)?;
        writeln!(f, "{:16} {}", "Tokens:".dimmed(), format_tokens(self.tokens_used))?;
        writeln!(f, "{:16} ${:.4}", "Est. Cost:".dimmed(), self.estimated_cost)?;

        if let Some(ref error) = self.last_error {
            writeln!(f, "{:16} {}", "Last Error:".dimmed(), error.red())?;
        }

        Ok(())
    }
}

/// Session statistics output
#[derive(Serialize)]
struct SessionStatsOutput {
    period: String,
    total_sessions: usize,
    completed_sessions: usize,
    failed_sessions: usize,
    total_tasks: usize,
    total_tokens: u64,
    total_cost: f64,
    avg_tasks_per_session: f32,
    avg_duration_secs: u64,
}

impl std::fmt::Display for SessionStatsOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", format!("Session Statistics ({})", self.period).bold())?;
        writeln!(f, "─────────────────────────────────")?;
        writeln!(f, "{:20} {}", "Total Sessions:".dimmed(), self.total_sessions)?;
        writeln!(f, "{:20} {}", "Completed:".dimmed(), self.completed_sessions.to_string().green())?;
        writeln!(f, "{:20} {}", "Failed:".dimmed(),
            if self.failed_sessions > 0 {
                self.failed_sessions.to_string().red()
            } else {
                self.failed_sessions.to_string().normal()
            })?;
        writeln!(f, "{:20} {}", "Total Tasks:".dimmed(), self.total_tasks)?;
        writeln!(f, "{:20} {}", "Avg Tasks/Session:".dimmed(), format!("{:.1}", self.avg_tasks_per_session))?;
        writeln!(f, "{:20} {}", "Avg Duration:".dimmed(), format_duration(self.avg_duration_secs))?;
        writeln!(f, "{:20} {}", "Total Tokens:".dimmed(), format_tokens(self.total_tokens))?;
        writeln!(f, "{:20} ${:.2}", "Total Cost:".dimmed(), self.total_cost)?;
        Ok(())
    }
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Execute a session command
pub async fn execute(cmd: SessionCommand, _workdir: &str, format: OutputFormat) -> CliResult<()> {
    // For now, use in-memory store (TODO: implement file-based persistence)
    let store: Arc<dyn SessionStore> = Arc::new(InMemorySessionStore::new());

    match cmd {
        SessionCommand::List { limit, completed, failed } => {
            let sessions = store.list_recent(limit).await?;

            let filtered: Vec<_> = sessions.iter()
                .filter(|s| {
                    if completed {
                        matches!(s.state, orchestrator::SessionState::Completed)
                    } else if failed {
                        matches!(s.state, orchestrator::SessionState::Failed)
                    } else {
                        true
                    }
                })
                .collect();

            let output = SessionListOutput {
                total: filtered.len(),
                sessions: filtered.iter().map(|s| SessionSummaryOutput::from(*s)).collect(),
            };

            print_output(&output, format);
        }

        SessionCommand::Show { id } => {
            let session = store.load(&id).await?
                .ok_or_else(|| CliError::SessionNotFound(id.clone()))?;

            let output = SessionDetailOutput {
                id: session.id.clone(),
                state: format!("{:?}", session.state).to_lowercase(),
                started_at: format_timestamp(&session.started_at),
                ended_at: session.ended_at.map(|t| format_timestamp(&t)),
                duration_secs: session.duration_secs(),
                tasks_completed: session.tasks_completed,
                tasks_failed: session.tasks_failed,
                tool_calls: session.tool_calls,
                tokens_used: session.tokens_used.total(),
                estimated_cost: session.tokens_used.estimate_cost_usd(),
                last_error: session.last_error.clone(),
            };

            print_output(&output, format);
        }

        SessionCommand::Resume { id } => {
            print_info(&format!("Resuming session: {}", id));

            let session = store.load(&id).await?
                .ok_or_else(|| CliError::SessionNotFound(id.clone()))?;

            if session.state != orchestrator::SessionState::Paused {
                return Err(CliError::InvalidArgument(
                    format!("Session {} is not paused (state: {:?})", id, session.state)
                ));
            }

            // TODO: Actually resume the session
            print_warning("Session resume not yet implemented");
        }

        SessionCommand::Replay { id, speed } => {
            print_info(&format!("Replaying session: {} (speed: {}x)", id, speed));

            let _session = store.load(&id).await?
                .ok_or_else(|| CliError::SessionNotFound(id.clone()))?;

            // TODO: Implement session replay
            print_warning("Session replay not yet implemented");
        }

        SessionCommand::Delete { id, force } => {
            if !force {
                print_warning(&format!("This will delete session {}. Use --force to confirm.", id));
                return Ok(());
            }

            store.delete(&id).await?;
            print_success(&format!("Deleted session: {}", id));
        }

        SessionCommand::Export { id, output, format: export_format } => {
            let session = store.load(&id).await?
                .ok_or_else(|| CliError::SessionNotFound(id.clone()))?;

            let data = match export_format.as_str() {
                "json" => serde_json::to_string_pretty(&session)?,
                "jsonl" => serde_json::to_string(&session)?,
                "atif" => {
                    // TODO: Convert to ATIF format
                    return Err(CliError::InvalidArgument("ATIF export not yet implemented".into()));
                }
                _ => return Err(CliError::InvalidArgument(format!("Unknown format: {}", export_format))),
            };

            if let Some(path) = output {
                std::fs::write(&path, &data)?;
                print_success(&format!("Exported session to: {}", path));
            } else {
                println!("{}", data);
            }
        }

        SessionCommand::Stats { period } => {
            let sessions = store.list_recent(1000).await?;

            // Calculate stats
            let total_sessions = sessions.len();
            let completed_sessions = sessions.iter()
                .filter(|s| matches!(s.state, orchestrator::SessionState::Completed))
                .count();
            let failed_sessions = sessions.iter()
                .filter(|s| matches!(s.state, orchestrator::SessionState::Failed))
                .count();
            let total_tasks: usize = sessions.iter().map(|s| s.tasks_completed).sum();
            let total_tokens: u64 = sessions.iter().map(|s| s.tokens_used).sum();
            let total_cost: f64 = sessions.iter().map(|s| s.estimated_cost_usd).sum();

            let avg_tasks = if total_sessions > 0 {
                total_tasks as f32 / total_sessions as f32
            } else {
                0.0
            };

            let avg_duration = if total_sessions > 0 {
                sessions.iter().map(|s| s.duration_secs).sum::<u64>() / total_sessions as u64
            } else {
                0
            };

            let output = SessionStatsOutput {
                period,
                total_sessions,
                completed_sessions,
                failed_sessions,
                total_tasks,
                total_tokens,
                total_cost,
                avg_tasks_per_session: avg_tasks,
                avg_duration_secs: avg_duration,
            };

            print_output(&output, format);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }
}
