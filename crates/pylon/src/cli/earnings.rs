//! pylon earnings - View provider earnings
//!
//! Shows earnings summary and history for provider mode.

use clap::Args;

use crate::daemon::db_path;
use crate::db::PylonDb;

/// Arguments for the earnings command
#[derive(Args)]
pub struct EarningsArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show last N earnings (default: 10)
    #[arg(long, short, default_value = "10")]
    pub limit: usize,
}

/// Run the earnings command
pub async fn run(args: EarningsArgs) -> anyhow::Result<()> {
    let db_file = db_path()?;

    if !db_file.exists() {
        println!("No earnings data found.");
        println!("Start the provider with 'pylon start' to begin earning.");
        return Ok(());
    }

    let db = PylonDb::open(db_file)?;
    let summary = db.get_earnings_summary()?;
    let recent = db.get_recent_earnings(args.limit)?;

    if args.json {
        let json = serde_json::json!({
            "summary": {
                "total_sats": summary.total_sats,
                "total_msats": summary.total_msats,
                "job_count": summary.job_count,
                "by_source": summary.by_source,
            },
            "recent": recent.iter().map(|e| serde_json::json!({
                "id": e.id,
                "job_id": e.job_id,
                "amount_msats": e.amount_msats,
                "source": format!("{:?}", e.source).to_lowercase(),
                "earned_at": e.earned_at,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Pylon Earnings");
        println!("==============\n");

        println!("Summary:");
        println!("  Total earned: {} sats ({} msats)", summary.total_sats, summary.total_msats);
        println!("  Jobs completed: {}", summary.job_count);
        println!();

        if !summary.by_source.is_empty() {
            println!("By Source:");
            for (source, amount) in &summary.by_source {
                println!("  {}: {} sats", source, amount / 1000);
            }
            println!();
        }

        if recent.is_empty() {
            println!("No recent earnings.");
        } else {
            println!("Recent Earnings (last {}):", args.limit);
            println!("{:<8} {:<10} {:<20}", "SATS", "SOURCE", "TIME AGO");
            println!("{}", "-".repeat(38));

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            for earning in recent {
                let ago = now - earning.earned_at;
                let ago_str = if ago < 60 {
                    format!("{} seconds", ago)
                } else if ago < 3600 {
                    format!("{} minutes", ago / 60)
                } else if ago < 86400 {
                    format!("{} hours", ago / 3600)
                } else {
                    format!("{} days", ago / 86400)
                };

                let source = format!("{:?}", earning.source).to_lowercase();
                println!(
                    "{:<8} {:<10} {}",
                    earning.amount_msats / 1000,
                    source,
                    ago_str
                );
            }
        }
    }

    Ok(())
}
