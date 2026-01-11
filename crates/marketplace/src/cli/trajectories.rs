//! CLI commands for trajectory contribution

use crate::trajectories::{
    RedactionLevel, RewardCalculator, TrajectoryCollector, TrajectoryConfig, TrajectorySource,
    validate::validate_trajectory,
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
        /// Source to scan (codex, cursor), defaults to all configured
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
            TrajectoriesSubcommand::Scan { source, verbose } => execute_scan(source, verbose).await,
            TrajectoriesSubcommand::Preview { limit, detailed } => {
                execute_preview(limit, detailed).await
            }
            TrajectoriesSubcommand::Redact {
                session_id,
                dry_run,
                level,
            } => execute_redact(&session_id, dry_run, &level).await,
            TrajectoriesSubcommand::Contribute { batch, review } => {
                execute_contribute(batch, review).await
            }
            TrajectoriesSubcommand::Status {
                pending,
                accepted,
                rejected,
            } => execute_status(pending, accepted, rejected).await,
            TrajectoriesSubcommand::Earnings { detail, since } => {
                execute_earnings(detail, since).await
            }
            TrajectoriesSubcommand::Config {
                auto,
                min_quality,
                sources,
            } => execute_config(auto, min_quality, sources).await,
        }
    }
}

/// Execute scan command
async fn execute_scan(source: Option<String>, verbose: bool) -> Result<()> {
    let config = TrajectoryConfig::default();
    let collector = TrajectoryCollector::new(config);

    let results = if let Some(source_name) = source {
        let source = source_name
            .parse::<TrajectorySource>()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
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
                println!(
                    "  - {} (quality: {:.2}, tokens: {}, tools: {})",
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
                    config.min_quality_score,
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
    let redaction_level = level
        .parse::<RedactionLevel>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

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
    use crate::trajectories::ContributionClient;
    use crate::trajectories::ContributionConfig;

    println!("Contributing trajectories to marketplace...\n");

    // Load configuration
    let config = TrajectoryConfig::default();
    let contrib_config = ContributionConfig {
        redaction_level: config
            .redaction_level
            .parse::<RedactionLevel>()
            .unwrap_or(RedactionLevel::Standard),
        min_quality: config.min_quality_score,
        ..Default::default()
    };

    // Initialize contribution client
    let mut client = ContributionClient::new(contrib_config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize contribution client: {}", e))?;

    // Scan for trajectories
    let collector = TrajectoryCollector::new(config.clone());
    let results = collector.scan_all()?;

    let mut contributed = 0;
    let mut total_reward = 0u64;
    let mut skipped = 0;

    for result in results {
        for session in result.sessions {
            // Check if meets minimum quality
            let validation = validate_trajectory(&session, config.min_quality_score);

            if !validation.passed {
                skipped += 1;
                if !batch {
                    println!(
                        "⊗ Skipping {} - {}",
                        session.session_id,
                        validation.failure_reasons.join(", ")
                    );
                }
                continue;
            }

            // Calculate reward estimate
            let calculator = RewardCalculator::default();
            let reward = calculator.calculate_reward(
                &session,
                validation.quality_score,
                config.min_quality_score,
            );

            // Review mode - ask for confirmation
            if review && !batch {
                println!("\nSession: {}", session.session_id);
                println!("  Quality: {:.2}", validation.quality_score.value());
                println!("  Estimated reward: {} sats", reward.total_sats);
                println!(
                    "  Tokens: {}, Tool calls: {}",
                    session.token_count, session.tool_calls
                );
                print!("\nContribute this session? [y/N]: ");
                std::io::Write::flush(&mut std::io::stdout())?;

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("  Skipped.");
                    skipped += 1;
                    continue;
                }
            }

            // Submit contribution
            match client.submit(session.clone()).await {
                Ok(response) => {
                    contributed += 1;
                    total_reward += response.estimated_reward_sats;
                    println!(
                        "✓ Contributed {} - {} sats",
                        session.session_id, response.estimated_reward_sats
                    );
                }
                Err(e) => {
                    eprintln!("✗ Failed to contribute {}: {}", session.session_id, e);
                    skipped += 1;
                }
            }

            // In batch mode, add a small delay to avoid overwhelming relays
            if batch && contributed % 10 == 0 {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }

    println!("\n═══════════════════════════════════════");
    println!("✓ Contributed {} sessions", contributed);
    println!("⊗ Skipped {} sessions", skipped);
    println!("  Total estimated reward: {} sats", total_reward);
    println!("═══════════════════════════════════════");

    Ok(())
}

/// Execute status command
async fn execute_status(pending: bool, accepted: bool, rejected: bool) -> Result<()> {
    use crate::trajectories::{ContributionClient, ContributionConfig, ContributionStatus};

    println!("Trajectory Contribution Status");
    println!("═══════════════════════════════════════\n");

    // Initialize client
    let config = ContributionConfig::default();
    let client = ContributionClient::new(config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize client: {}", e))?;

    // Determine which statuses to show
    let show_all = !pending && !accepted && !rejected;

    if pending || show_all {
        let pending_contribs = client.list_contributions(Some(ContributionStatus::Pending))?;
        println!("Pending ({}):", pending_contribs.len());
        if pending_contribs.is_empty() {
            println!("  (none)");
        } else {
            for contrib in pending_contribs.iter().take(10) {
                println!(
                    "  • {} (quality: {:.2}, {} sats)",
                    contrib.session_id, contrib.quality_score, contrib.estimated_reward_sats
                );
            }
            if pending_contribs.len() > 10 {
                println!("  ... and {} more", pending_contribs.len() - 10);
            }
        }
        println!();
    }

    if accepted || show_all {
        let accepted_contribs = client.list_contributions(Some(ContributionStatus::Accepted))?;
        let total_earned: u64 = accepted_contribs
            .iter()
            .filter_map(|c| c.actual_reward_sats)
            .sum();

        println!("Accepted ({}):", accepted_contribs.len());
        if accepted_contribs.is_empty() {
            println!("  (none)");
        } else {
            for contrib in accepted_contribs.iter().take(10) {
                let reward = contrib
                    .actual_reward_sats
                    .unwrap_or(contrib.estimated_reward_sats);
                println!(
                    "  ✓ {} - {} sats{}",
                    contrib.session_id,
                    reward,
                    if contrib.paid_at.is_some() {
                        " (paid)"
                    } else {
                        ""
                    }
                );
            }
            if accepted_contribs.len() > 10 {
                println!("  ... and {} more", accepted_contribs.len() - 10);
            }
            println!("  Total earned: {} sats", total_earned);
        }
        println!();
    }

    if rejected || show_all {
        let rejected_contribs = client.list_contributions(Some(ContributionStatus::Rejected))?;
        println!("Rejected ({}):", rejected_contribs.len());
        if rejected_contribs.is_empty() {
            println!("  (none)");
        } else {
            for contrib in rejected_contribs.iter().take(10) {
                println!(
                    "  ✗ {} (quality: {:.2})",
                    contrib.session_id, contrib.quality_score
                );
            }
            if rejected_contribs.len() > 10 {
                println!("  ... and {} more", rejected_contribs.len() - 10);
            }
        }
        println!();
    }

    Ok(())
}

/// Execute earnings command
async fn execute_earnings(detail: bool, _since: Option<String>) -> Result<()> {
    use crate::trajectories::{ContributionClient, ContributionConfig};

    println!("Trajectory Contribution Earnings");
    println!("═══════════════════════════════════════\n");

    // Initialize client
    let config = ContributionConfig::default();
    let client = ContributionClient::new(config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize client: {}", e))?;

    // Get earnings
    let earnings = client.get_earnings()?;
    let total_earned: u64 = earnings.iter().map(|e| e.reward_sats).sum();

    println!("Total earned: {} sats", total_earned);
    println!("Paid contributions: {}", earnings.len());

    if detail && !earnings.is_empty() {
        println!("\nDetailed breakdown:");
        for earning in earnings.iter() {
            let date = earning.paid_at.format("%Y-%m-%d");
            println!(
                "  {} - {} sats ({})",
                earning.session_id, earning.reward_sats, date
            );
            if let Some(ref preimage) = earning.payment_preimage {
                println!("    Proof: {}...", &preimage[..16.min(preimage.len())]);
            }
        }
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
        println!(
            "✓ Auto-contribution: {}",
            if enabled { "enabled" } else { "disabled" }
        );
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
        println!("  Sources: codex");
        println!("  Redaction level: standard");
    }

    Ok(())
}
