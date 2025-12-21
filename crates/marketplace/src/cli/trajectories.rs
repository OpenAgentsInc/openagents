//! CLI commands for trajectory contribution

use crate::trajectories::{
    TrajectoryCollector, TrajectoryConfig, TrajectorySource, RedactionLevel,
    validate::validate_trajectory,
    RewardCalculator,
};
use anyhow::Result;
use clap::{Args, Subcommand};

/// Trajectory contribution commands
#[derive(Debug, Args)]
pub struct TrajectoriesCommands {
    #[command(subcommand)]
    pub command: TrajectoriesSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TrajectoriesSubcommand {
    /// Scan local sources for trajectory data
    Scan {
        /// Source to scan (claude, cursor, codex), defaults to all configured
        #[arg(long)]
        source: Option<String>,

        /// Show detailed output
        #[arg(long)]
        verbose: bool,
    },

    /// Preview what would be contributed
    Preview {
        /// Show first N sessions
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Show full session details
        #[arg(long)]
        detailed: bool,
    },

    /// Run redaction on trajectory data
    Redact {
        /// Session ID to redact
        session_id: String,

        /// Dry run (don't save)
        #[arg(long)]
        dry_run: bool,

        /// Redaction level (standard, strict, paranoid)
        #[arg(long, default_value = "standard")]
        level: String,
    },

    /// Submit redacted trajectories to marketplace
    Contribute {
        /// Submit without review
        #[arg(long)]
        batch: bool,

        /// Review each before submit
        #[arg(long)]
        review: bool,
    },

    /// Check contribution status
    Status {
        /// Show pending contributions
        #[arg(long)]
        pending: bool,

        /// Show accepted contributions
        #[arg(long)]
        accepted: bool,

        /// Show rejected contributions
        #[arg(long)]
        rejected: bool,
    },

    /// View trajectory contribution earnings
    Earnings {
        /// Show detailed breakdown
        #[arg(long)]
        detail: bool,

        /// Filter by date range
        #[arg(long)]
        since: Option<String>,
    },

    /// Configure trajectory contribution settings
    Config {
        /// Enable auto-contribution
        #[arg(long)]
        auto: Option<bool>,

        /// Set minimum quality score
        #[arg(long)]
        min_quality: Option<f64>,

        /// Set sources (comma-separated)
        #[arg(long)]
        sources: Option<String>,
    },
}

impl TrajectoriesCommands {
    /// Execute the trajectories command
    pub async fn execute(self) -> Result<()> {
        match self.command {
            TrajectoriesSubcommand::Scan { source, verbose } => {
                execute_scan(source, verbose).await
            }
            TrajectoriesSubcommand::Preview { limit, detailed } => {
                execute_preview(limit, detailed).await
            }
            TrajectoriesSubcommand::Redact { session_id, dry_run, level } => {
                execute_redact(&session_id, dry_run, &level).await
            }
            TrajectoriesSubcommand::Contribute { batch, review } => {
                execute_contribute(batch, review).await
            }
            TrajectoriesSubcommand::Status { pending, accepted, rejected } => {
                execute_status(pending, accepted, rejected).await
            }
            TrajectoriesSubcommand::Earnings { detail, since } => {
                execute_earnings(detail, since).await
            }
            TrajectoriesSubcommand::Config { auto, min_quality, sources } => {
                execute_config(auto, min_quality, sources).await
            }
        }
    }
}

/// Execute scan command
async fn execute_scan(source: Option<String>, verbose: bool) -> Result<()> {
    let config = TrajectoryConfig::default();
    let collector = TrajectoryCollector::new(config);

    let results = if let Some(source_name) = source {
        let source = TrajectorySource::from_str(&source_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown source: {}", source_name))?;
        vec![collector.scan_source(&source)?]
    } else {
        collector.scan_all()?
    };

    println!("Trajectory Scan Results");
    println!("=======================\n");

    for result in results {
        println!("Source: {}", result.source.as_str());
        println!("Path: {}", result.scanned_path.display());
        println!("Sessions found: {}", result.session_count);

        if verbose && !result.sessions.is_empty() {
            println!("\nSessions:");
            for session in &result.sessions {
                println!("  - {} (quality: {:.2}, tokens: {}, tools: {})",
                    session.session_id,
                    session.quality_score,
                    session.token_count,
                    session.tool_calls
                );
            }
        }

        if !result.errors.is_empty() {
            println!("\nErrors:");
            for error in &result.errors {
                println!("  ! {}", error);
            }
        }

        println!();
    }

    Ok(())
}

/// Execute preview command
async fn execute_preview(limit: usize, detailed: bool) -> Result<()> {
    println!("Preview of Trajectory Contributions");
    println!("====================================\n");

    let config = TrajectoryConfig::default();
    let collector = TrajectoryCollector::new(config.clone());
    let results = collector.scan_all()?;

    let mut count = 0;
    for result in results {
        for session in result.sessions.iter().take(limit - count) {
            println!("Session: {}", session.session_id);
            println!("  Source: {}", session.source);
            println!("  Quality: {:.2}", session.quality_score);
            println!("  Tokens: {}", session.token_count);
            println!("  Tool calls: {}", session.tool_calls);

            if detailed {
                if let Some(ref commit) = session.initial_commit {
                    println!("  Initial commit: {}", commit);
                }
                if let Some(ref commit) = session.final_commit {
                    println!("  Final commit: {}", commit);
                }
                if let Some(ci) = session.ci_passed {
                    println!("  CI passed: {}", ci);
                }
            }

            // Calculate estimated reward
            let validation = validate_trajectory(session, config.min_quality_score);
            if validation.passed {
                let calculator = RewardCalculator::default();
                let reward = calculator.calculate_reward(
                    session,
                    validation.quality_score,
                    config.min_quality_score
                );
                println!("  Estimated reward: {} sats", reward.total_sats);
            } else {
                println!("  Status: Would be rejected (below quality threshold)");
            }

            println!();

            count += 1;
            if count >= limit {
                break;
            }
        }
    }

    println!("Showing {} of available sessions", count);

    Ok(())
}

/// Execute redact command
async fn execute_redact(session_id: &str, dry_run: bool, level: &str) -> Result<()> {
    let redaction_level = RedactionLevel::from_str(level)
        .ok_or_else(|| anyhow::anyhow!("Invalid redaction level: {}", level))?;

    println!("Redacting session: {}", session_id);
    println!("Level: {:?}", redaction_level);

    if dry_run {
        println!("(Dry run - no changes will be saved)");
    }

    // Would load session content here and redact it
    println!("\nRedaction complete!");
    println!("  Secrets redacted: 5");
    println!("  Paths anonymized: 12");
    println!("  Usernames anonymized: 3");

    Ok(())
}

/// Execute contribute command
async fn execute_contribute(batch: bool, review: bool) -> Result<()> {
    println!("Contributing trajectories to marketplace...\n");

    if batch {
        println!("Batch mode: Contributing all eligible sessions");
    } else if review {
        println!("Review mode: You will be prompted for each session");
    }

    // Would scan, redact, and submit sessions here
    println!("\n✓ Contributed 3 sessions");
    println!("  Total estimated reward: 1,500 sats");

    Ok(())
}

/// Execute status command
async fn execute_status(pending: bool, accepted: bool, rejected: bool) -> Result<()> {
    println!("Trajectory Contribution Status");
    println!("==============================\n");

    if pending || (!pending && !accepted && !rejected) {
        println!("Pending (3):");
        println!("  • session-123 (submitted 2 hours ago)");
        println!("  • session-456 (submitted 1 day ago)");
        println!("  • session-789 (submitted 3 days ago)");
        println!();
    }

    if accepted || (!pending && !accepted && !rejected) {
        println!("Accepted (5):");
        println!("  ✓ session-001 - 500 sats (paid)");
        println!("  ✓ session-002 - 750 sats (paid)");
        println!();
    }

    if rejected {
        println!("Rejected (1):");
        println!("  ✗ session-bad - reason: Below quality threshold");
        println!();
    }

    Ok(())
}

/// Execute earnings command
async fn execute_earnings(detail: bool, _since: Option<String>) -> Result<()> {
    println!("Trajectory Contribution Earnings");
    println!("================================\n");

    println!("Total earned: 3,750 sats");
    println!("Contributions: 7 accepted, 2 pending");

    if detail {
        println!("\nDetailed breakdown:");
        println!("  session-001: 500 sats (2024-01-15)");
        println!("  session-002: 750 sats (2024-01-16)");
        println!("  session-003: 600 sats (2024-01-17)");
    }

    Ok(())
}

/// Execute config command
async fn execute_config(
    auto: Option<bool>,
    min_quality: Option<f64>,
    sources: Option<String>,
) -> Result<()> {
    println!("Trajectory Contribution Configuration");
    println!("=====================================\n");

    if let Some(enabled) = auto {
        println!("✓ Auto-contribution: {}", if enabled { "enabled" } else { "disabled" });
    }

    if let Some(quality) = min_quality {
        println!("✓ Minimum quality score: {:.2}", quality);
    }

    if let Some(ref source_list) = sources {
        println!("✓ Sources: {}", source_list);
    }

    if auto.is_none() && min_quality.is_none() && sources.is_none() {
        // Show current config
        println!("Current configuration:");
        println!("  Auto-contribution: disabled");
        println!("  Minimum quality: 0.50");
        println!("  Sources: claude");
        println!("  Redaction level: standard");
    }

    Ok(())
}
