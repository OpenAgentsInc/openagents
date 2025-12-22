//! Earnings CLI commands

use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::fmt;

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
            EarningsSubcommand::History { json, limit, source } => {
                execute_history(json, limit, source).await
            }
            EarningsSubcommand::Withdraw { amount, invoice, yes } => {
                execute_withdraw(amount, invoice, yes).await
            }
            EarningsSubcommand::Export { output, format, from, to } => {
                execute_export(&output, &format, from, to).await
            }
        }
    }
}

/// Revenue source type
#[derive(Debug, Clone, Copy)]
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
async fn execute_dashboard(_json: bool, period: &str) -> Result<()> {
    println!("{}", "Marketplace Earnings Dashboard".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    // Mock data - in production, query from earnings database
    let total_earned = 125_000u64;
    let available_balance = 25_000u64;
    let withdrawn = 100_000u64;

    println!("{}", format!("Period: {}", period).bright_white().bold());
    println!();

    println!("{}", "Total Earned".bright_green().bold());
    println!("  {} sats", total_earned.to_string().bright_white());
    println!();

    println!("{}", "Available Balance".bright_yellow().bold());
    println!("  {} sats", available_balance.to_string().bright_white());
    println!();

    println!("{}", "Already Withdrawn".bright_blue().bold());
    println!("  {} sats", withdrawn.to_string().bright_white());
    println!();

    println!("{}", "Earnings by Source".bright_cyan().bold());
    println!("  {:<15} {:>10}", "Compute:", "45,000 sats");
    println!("  {:<15} {:>10}", "Skills:", "30,000 sats");
    println!("  {:<15} {:>10}", "Data:", "20,000 sats");
    println!("  {:<15} {:>10}", "Trajectories:", "30,000 sats");
    println!();

    println!("{}", "Recent Activity".bright_magenta().bold());
    println!("  Last 7 days:     +15,000 sats");
    println!("  Last 30 days:    +55,000 sats");
    println!();

    println!("{}", "Next Actions".bright_yellow());
    println!("  • Run {} to see detailed history", "marketplace earnings history".cyan());
    println!("  • Run {} to withdraw funds", "marketplace earnings withdraw".cyan());
    println!();

    Ok(())
}

/// Execute history command
async fn execute_history(_json: bool, limit: usize, source_filter: Option<String>) -> Result<()> {
    println!("{}", "Earnings History".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    // Mock data - in production, query from earnings database
    let records = [
        EarningRecord {
            timestamp: 1703001000,
            source: RevenueSource::Compute,
            description: "LLM inference job completed".to_string(),
            amount_sats: 5_000,
            job_id: Some("job_abc123".to_string()),
        },
        EarningRecord {
            timestamp: 1703002000,
            source: RevenueSource::Trajectories,
            description: "Trajectory contribution accepted".to_string(),
            amount_sats: 2_500,
            job_id: None,
        },
        EarningRecord {
            timestamp: 1703003000,
            source: RevenueSource::Skills,
            description: "Skill license fee".to_string(),
            amount_sats: 1_000,
            job_id: Some("lic_xyz789".to_string()),
        },
        EarningRecord {
            timestamp: 1703004000,
            source: RevenueSource::Data,
            description: "Dataset sale".to_string(),
            amount_sats: 10_000,
            job_id: Some("sale_def456".to_string()),
        },
    ];

    // Filter by source if specified
    let filtered: Vec<_> = if let Some(ref filter) = source_filter {
        records
            .iter()
            .filter(|r| r.source.as_str() == filter.as_str())
            .take(limit)
            .collect()
    } else {
        records.iter().take(limit).collect()
    };

    if filtered.is_empty() {
        println!("{}", "No earnings records found".yellow());
        return Ok(());
    }

    println!("{:<12} {:<15} {:<40} {:>12}",
        "DATE".bright_white().bold(),
        "SOURCE".bright_white().bold(),
        "DESCRIPTION".bright_white().bold(),
        "AMOUNT".bright_white().bold()
    );
    println!("{}", "─".repeat(85).bright_black());

    let filtered_len = filtered.len();

    for record in filtered {
        let date = chrono::DateTime::from_timestamp(record.timestamp as i64, 0)
            .unwrap()
            .format("%Y-%m-%d");

        let source_str = match record.source {
            RevenueSource::Compute => record.source.to_string().bright_blue(),
            RevenueSource::Skills => record.source.to_string().bright_green(),
            RevenueSource::Data => record.source.to_string().bright_magenta(),
            RevenueSource::Trajectories => record.source.to_string().bright_cyan(),
        };

        println!(
            "{:<12} {:<15} {:<40} {:>12}",
            date.to_string(),
            source_str,
            record.description,
            format!("{} sats", record.amount_sats).bright_white()
        );
    }

    println!();
    println!("{}", format!("Showing {} of available records", filtered_len).bright_black());

    if let Some(filter) = source_filter {
        println!("{}", format!("Filtered by source: {}", filter).bright_black());
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
    println!("  Amount:    {} sats", withdraw_amount.to_string().bright_white());
    println!("  Available: {} sats", available_balance.to_string().bright_yellow());
    println!();

    if let Some(ref inv) = invoice {
        println!("  Invoice:   {}...{}", &inv[..20.min(inv.len())], if inv.len() > 40 { &inv[inv.len()-20..] } else { "" });
        println!();
    } else {
        println!("{}", "No invoice provided. Will withdraw to default Lightning address.".yellow());
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
    println!("  Amount withdrawn: {} sats", withdraw_amount.to_string().bright_white());
    println!("  New balance:      {} sats", (available_balance - withdraw_amount).to_string().bright_yellow());
    println!();
    println!("{}", "Payment should arrive within 1-2 minutes.".bright_black());
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
        "csv" | "json" => {},
        _ => anyhow::bail!("Unsupported format: {}. Use 'csv' or 'json'.", format),
    }

    println!("{}", "Exporting data...".bright_blue());

    // Mock data - in production, query and export real data
    let mock_csv = "date,source,description,amount_sats,job_id\n\
        2024-12-19,compute,LLM inference job,5000,job_abc123\n\
        2024-12-19,trajectories,Trajectory contribution,2500,\n\
        2024-12-19,skills,Skill license fee,1000,lic_xyz789\n\
        2024-12-19,data,Dataset sale,10000,sale_def456\n";

    std::fs::write(output_path, mock_csv)?;

    println!();
    println!("{}", "✓ Export complete!".bright_green().bold());
    println!();
    println!("  File:    {}", output_path);
    println!("  Records: 4");
    println!();

    Ok(())
}
