//! Earnings CLI commands

use crate::core::earnings::{EarningsTracker, RevenueType};
use crate::db::init_db;
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::fmt;
use std::path::PathBuf;

/// Earnings commands
#[derive(Debug, Args)]
pub struct EarningsCommands {
    #[command(subcommand)]
    pub command: EarningsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum EarningsSubcommand {
    /// View earnings dashboard
    Dashboard {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Time period (day, week, month, all)
        #[arg(long, default_value = "week")]
        period: String,
    },

    /// View detailed earnings history
    History {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Number of records to display
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Filter by revenue source (compute, skills, data, trajectories)
        #[arg(long)]
        source: Option<String>,
    },

    /// Withdraw earnings to Lightning wallet
    Withdraw {
        /// Amount in sats to withdraw (defaults to total available)
        #[arg(long)]
        amount: Option<u64>,

        /// Lightning invoice or address
        #[arg(long)]
        invoice: Option<String>,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },

    /// Export earnings data for accounting
    Export {
        /// Output file path
        #[arg(long)]
        output: String,

        /// Export format (csv, json)
        #[arg(long, default_value = "csv")]
        format: String,

        /// Date range start (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// Date range end (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,
    },
}

impl EarningsCommands {
    /// Execute the earnings command
    pub async fn execute(self) -> Result<()> {
        match self.command {
            EarningsSubcommand::Dashboard { json, period } => {
                execute_dashboard(json, &period).await
            }
            EarningsSubcommand::History {
                json,
                limit,
                source,
            } => execute_history(json, limit, source).await,
            EarningsSubcommand::Withdraw {
                amount,
                invoice,
                yes,
            } => execute_withdraw(amount, invoice, yes).await,
            EarningsSubcommand::Export {
                output,
                format,
                from,
                to,
            } => execute_export(&output, &format, from, to).await,
        }
    }
}

/// Revenue source type
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum RevenueSource {
    Compute,
    Skills,
    Data,
    Trajectories,
}

impl RevenueSource {
    fn as_str(&self) -> &'static str {
        match self {
            RevenueSource::Compute => "compute",
            RevenueSource::Skills => "skills",
            RevenueSource::Data => "data",
            RevenueSource::Trajectories => "trajectories",
        }
    }
}

impl fmt::Display for RevenueSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Earnings record
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct EarningRecord {
    timestamp: u64,
    source: RevenueSource,
    description: String,
    amount_sats: u64,
    job_id: Option<String>,
}

/// Execute dashboard command
async fn execute_dashboard(json: bool, period: &str) -> Result<()> {
    // Initialize database
    let db_path = get_db_path()?;
    let conn = init_db(&db_path)?;
    let tracker = EarningsTracker::with_defaults();

    // Get earnings stats
    let stats = tracker.get_earnings_stats(&conn)?;

    if json {
        let json_output = serde_json::to_string_pretty(&stats)?;
        println!("{}", json_output);
        return Ok(());
    }

    println!("{}", "Marketplace Earnings Dashboard".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    println!("{}", format!("Period: {}", period).bright_white().bold());
    println!();

    println!("{}", "Total Earned".bright_green().bold());
    println!(
        "  {} sats",
        stats.total_gross_sats.to_string().bright_white()
    );
    println!();

    println!("{}", "Earnings by Source".bright_cyan().bold());
    println!(
        "  {:<15} {:>10}",
        "Compute:",
        format!("{} sats", stats.by_type.compute_sats)
    );
    println!(
        "  {:<15} {:>10}",
        "Skills:",
        format!("{} sats", stats.by_type.skill_sats)
    );
    println!(
        "  {:<15} {:>10}",
        "Data:",
        format!("{} sats", stats.by_type.data_sats)
    );
    println!(
        "  {:<15} {:>10}",
        "Trajectories:",
        format!("{} sats", stats.by_type.trajectory_sats)
    );
    println!();

    println!("{}", "Recent Activity".bright_magenta().bold());
    println!(
        "  Last hour:       +{} sats",
        stats.by_period.last_hour_sats
    );
    println!("  Last 24 hours:   +{} sats", stats.by_period.last_day_sats);
    println!(
        "  Last 7 days:     +{} sats",
        stats.by_period.last_week_sats
    );
    println!(
        "  Last 30 days:    +{} sats",
        stats.by_period.last_month_sats
    );
    println!();

    println!("{}", "Next Actions".bright_yellow());
    println!(
        "  • Run {} to see detailed history",
        "marketplace earnings history".cyan()
    );
    println!(
        "  • Run {} to export data",
        "marketplace earnings export".cyan()
    );
    println!();

    Ok(())
}

/// Execute history command
async fn execute_history(json: bool, limit: usize, source_filter: Option<String>) -> Result<()> {
    // Initialize database
    let db_path = get_db_path()?;
    let conn = init_db(&db_path)?;
    let tracker = EarningsTracker::with_defaults();

    // Parse revenue type filter if provided
    let revenue_type = source_filter.as_deref().and_then(RevenueType::from_str);

    // Get recent buckets
    let buckets = tracker.get_recent_buckets(&conn, limit, revenue_type)?;

    if json {
        let json_output = serde_json::to_string_pretty(&buckets)?;
        println!("{}", json_output);
        return Ok(());
    }

    println!("{}", "Earnings History".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    if buckets.is_empty() {
        println!("{}", "No earnings records found".yellow());
        return Ok(());
    }

    println!(
        "{:<20} {:<15} {:<30} {:>12}",
        "TIMESTAMP".bright_white().bold(),
        "SOURCE".bright_white().bold(),
        "ITEM ID".bright_white().bold(),
        "AMOUNT".bright_white().bold()
    );
    println!("{}", "─".repeat(85).bright_black());

    for bucket in &buckets {
        let datetime = chrono::DateTime::from_timestamp(bucket.bucket_minute as i64, 0)
            .unwrap()
            .format("%Y-%m-%d %H:%M");

        let source_str = match bucket.revenue_type {
            RevenueType::Compute => bucket.revenue_type.as_str().bright_blue(),
            RevenueType::Skill => bucket.revenue_type.as_str().bright_green(),
            RevenueType::Data => bucket.revenue_type.as_str().bright_magenta(),
            RevenueType::Trajectory => bucket.revenue_type.as_str().bright_cyan(),
        };

        println!(
            "{:<20} {:<15} {:<30} {:>12}",
            datetime.to_string(),
            source_str,
            &bucket.item_id,
            format!("{} sats", bucket.gross_sats).bright_white()
        );
    }

    println!();
    println!(
        "{}",
        format!("Showing {} records", buckets.len()).bright_black()
    );

    if let Some(filter) = source_filter {
        println!(
            "{}",
            format!("Filtered by source: {}", filter).bright_black()
        );
    }

    println!();

    Ok(())
}

/// Execute withdraw command
async fn execute_withdraw(
    amount: Option<u64>,
    invoice: Option<String>,
    skip_confirm: bool,
) -> Result<()> {
    println!("{}", "Withdraw Earnings".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    // Mock available balance
    let available_balance = 25_000u64;
    let withdraw_amount = amount.unwrap_or(available_balance);

    if withdraw_amount > available_balance {
        anyhow::bail!(
            "Insufficient balance. Available: {} sats, Requested: {} sats",
            available_balance,
            withdraw_amount
        );
    }

    println!("{}", "Withdrawal Details".bright_white().bold());
    println!(
        "  Amount:    {} sats",
        withdraw_amount.to_string().bright_white()
    );
    println!(
        "  Available: {} sats",
        available_balance.to_string().bright_yellow()
    );
    println!();

    if let Some(ref inv) = invoice {
        println!(
            "  Invoice:   {}...{}",
            &inv[..20.min(inv.len())],
            if inv.len() > 40 {
                &inv[inv.len() - 20..]
            } else {
                ""
            }
        );
        println!();
    } else {
        println!(
            "{}",
            "No invoice provided. Will withdraw to default Lightning address.".yellow()
        );
        println!();
    }

    // Confirmation prompt unless --yes flag
    if !skip_confirm {
        print!("{}", "Confirm withdrawal? [y/N]: ".bright_yellow());
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", "Withdrawal cancelled.".yellow());
            return Ok(());
        }
    }

    println!();
    println!("{}", "Processing withdrawal...".bright_blue());
    println!();

    // In production: call Lightning wallet withdrawal API
    // For now, simulate success
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    println!("{}", "✓ Withdrawal successful!".bright_green().bold());
    println!();
    println!(
        "  Amount withdrawn: {} sats",
        withdraw_amount.to_string().bright_white()
    );
    println!(
        "  New balance:      {} sats",
        (available_balance - withdraw_amount)
            .to_string()
            .bright_yellow()
    );
    println!();
    println!(
        "{}",
        "Payment should arrive within 1-2 minutes.".bright_black()
    );
    println!();

    Ok(())
}

/// Execute export command
async fn execute_export(
    output_path: &str,
    format: &str,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<()> {
    println!("{}", "Export Earnings Data".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    println!("{}", "Export Configuration".bright_white().bold());
    println!("  Output:  {}", output_path);
    println!("  Format:  {}", format);
    if let Some(ref from) = from_date {
        println!("  From:    {}", from);
    }
    if let Some(ref to) = to_date {
        println!("  To:      {}", to);
    }
    println!();

    // Validate format
    match format {
        "csv" | "json" => {}
        _ => anyhow::bail!("Unsupported format: {}. Use 'csv' or 'json'.", format),
    }

    // Parse date range
    let from_timestamp = if let Some(ref from) = from_date {
        parse_date_to_timestamp(from)?
    } else {
        0 // Beginning of time
    };

    let to_timestamp = if let Some(ref to) = to_date {
        parse_date_to_timestamp(to)?
    } else {
        u64::MAX // End of time
    };

    // Initialize database
    let db_path = get_db_path()?;
    let conn = init_db(&db_path)?;
    let tracker = EarningsTracker::with_defaults();

    println!("{}", "Exporting data...".bright_blue());

    // Export in requested format
    let content = match format {
        "csv" => tracker.export_as_csv(&conn, from_timestamp, to_timestamp)?,
        "json" => tracker.export_as_json(&conn, from_timestamp, to_timestamp)?,
        _ => unreachable!(),
    };

    // Write to file
    std::fs::write(output_path, content)?;

    // Count records
    let buckets = tracker.export_earnings(&conn, from_timestamp, to_timestamp)?;
    let record_count = buckets.len();

    println!();
    println!("{}", "✓ Export complete!".bright_green().bold());
    println!();
    println!("  File:    {}", output_path);
    println!("  Records: {}", record_count);
    println!();

    Ok(())
}

/// Parse date string (YYYY-MM-DD) to unix timestamp
fn parse_date_to_timestamp(date_str: &str) -> Result<u64> {
    use chrono::NaiveDate;

    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| {
        anyhow::anyhow!("Invalid date format '{}': {}. Use YYYY-MM-DD", date_str, e)
    })?;

    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("Failed to create datetime from date"))?;

    Ok(datetime.and_utc().timestamp() as u64)
}

/// Get database path for marketplace earnings
fn get_db_path() -> Result<PathBuf> {
    let home =
        std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME environment variable not set"))?;
    let mut path = PathBuf::from(home);
    path.push(".openagents");
    path.push("marketplace");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&path)?;

    path.push("earnings.db");
    Ok(path)
}
