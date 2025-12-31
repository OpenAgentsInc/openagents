//! Reputation CLI commands

use crate::core::nip32_reputation::{ReputationAggregator, TrustTier};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;

/// Reputation commands
#[derive(Debug, Args)]
pub struct ReputationCommands {
    #[command(subcommand)]
    pub command: ReputationSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ReputationSubcommand {
    /// View reputation for a provider
    View {
        /// Provider pubkey (hex format)
        pubkey: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List providers by trust tier
    ListByTier {
        /// Trust tier (new, established, trusted, expert)
        tier: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List top-ranked providers
    TopProviders {
        /// Number of providers to show
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show reputation statistics
    Stats {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

impl ReputationCommands {
    /// Execute the reputation command
    pub async fn execute(self) -> Result<()> {
        match self.command {
            ReputationSubcommand::View { pubkey, json } => execute_view(&pubkey, json).await,
            ReputationSubcommand::ListByTier { tier, json } => {
                execute_list_by_tier(&tier, json).await
            }
            ReputationSubcommand::TopProviders { limit, json } => {
                execute_top_providers(limit, json).await
            }
            ReputationSubcommand::Stats { json } => execute_stats(json).await,
        }
    }
}

/// Execute view command
async fn execute_view(pubkey: &str, json: bool) -> Result<()> {
    // In production, load from database or Nostr relays
    let aggregator = load_reputation_data()?;

    let metrics = aggregator
        .get_metrics(pubkey)
        .ok_or_else(|| anyhow::anyhow!("No reputation data found for pubkey: {}", pubkey))?;

    if json {
        let json_output = serde_json::to_string_pretty(&metrics)?;
        println!("{}", json_output);
        return Ok(());
    }

    println!("{}", "Provider Reputation".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    println!("{}", "Pubkey".bright_white().bold());
    println!("  {}", metrics.provider_pubkey);
    println!();

    println!("{}", "Trust Tier".bright_green().bold());
    let tier_str = match metrics.trust_tier {
        TrustTier::New => "New".yellow(),
        TrustTier::Established => "Established".blue(),
        TrustTier::Trusted => "Trusted".green(),
        TrustTier::Expert => "Expert".bright_green().bold(),
    };
    println!("  {}", tier_str);
    println!();

    println!("{}", "Job History".bright_cyan().bold());
    println!("  Total jobs:      {}", metrics.jobs_completed);
    println!(
        "  Successful:      {}",
        metrics.jobs_succeeded.to_string().green()
    );
    println!(
        "  Failed:          {}",
        metrics.jobs_failed.to_string().red()
    );
    println!(
        "  Success rate:    {:.1}%",
        (metrics.success_rate * 100.0).to_string().bright_white()
    );
    println!();

    if metrics.review_count > 0 {
        println!("{}", "Reviews".bright_magenta().bold());
        println!("  Average rating:  {:.1}/5.0", metrics.avg_rating);
        println!("  Total reviews:   {}", metrics.review_count);
        println!();
    }

    if !metrics.skill_ratings.is_empty() {
        println!("{}", "Skill Ratings".bright_yellow().bold());
        for (skill_id, quality) in &metrics.skill_ratings {
            println!("  {}: {:?}", skill_id, quality);
        }
        println!();
    }

    println!("{}", "Discovery Weight".bright_blue().bold());
    println!("  {:.2}x", metrics.discovery_weight());
    println!();

    Ok(())
}

/// Execute list by tier command
async fn execute_list_by_tier(tier_str: &str, json: bool) -> Result<()> {
    let tier = TrustTier::from_str(tier_str).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid tier: {}. Use: new, established, trusted, expert",
            tier_str
        )
    })?;

    let aggregator = load_reputation_data()?;
    let providers = aggregator.get_by_tier(tier);

    if json {
        let json_output = serde_json::to_string_pretty(&providers)?;
        println!("{}", json_output);
        return Ok(());
    }

    println!("{}", format!("{:?} Tier Providers", tier).cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    if providers.is_empty() {
        println!("{}", "No providers found in this tier".yellow());
        return Ok(());
    }

    println!(
        "{:<66} {:>12} {:>10}",
        "PUBKEY".bright_white().bold(),
        "SUCCESS RATE".bright_white().bold(),
        "JOBS".bright_white().bold()
    );
    println!("{}", "─".repeat(90).bright_black());

    let providers_count = providers.len();
    for metrics in providers {
        let pubkey_short = if metrics.provider_pubkey.len() > 64 {
            &metrics.provider_pubkey[..64]
        } else {
            &metrics.provider_pubkey
        };

        println!(
            "{:<66} {:>11}% {:>10}",
            pubkey_short,
            format!("{:.1}", metrics.success_rate * 100.0).bright_white(),
            metrics.jobs_completed.to_string().bright_white()
        );
    }

    println!();
    println!(
        "{}",
        format!("Found {} providers", providers_count).bright_black()
    );
    println!();

    Ok(())
}

/// Execute top providers command
async fn execute_top_providers(limit: usize, json: bool) -> Result<()> {
    let aggregator = load_reputation_data()?;
    let mut all_ranked = aggregator.get_all_ranked();
    all_ranked.truncate(limit);

    if json {
        let json_output = serde_json::to_string_pretty(&all_ranked)?;
        println!("{}", json_output);
        return Ok(());
    }

    println!("{}", "Top Providers by Reputation".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    if all_ranked.is_empty() {
        println!("{}", "No reputation data available".yellow());
        return Ok(());
    }

    println!(
        "{:<4} {:<66} {:<14} {:>12} {:>10}",
        "RANK".bright_white().bold(),
        "PUBKEY".bright_white().bold(),
        "TIER".bright_white().bold(),
        "SUCCESS RATE".bright_white().bold(),
        "WEIGHT".bright_white().bold()
    );
    println!("{}", "─".repeat(110).bright_black());

    for (i, metrics) in all_ranked.iter().enumerate() {
        let rank = i + 1;
        let pubkey_short = if metrics.provider_pubkey.len() > 64 {
            &metrics.provider_pubkey[..64]
        } else {
            &metrics.provider_pubkey
        };

        let tier_str = match metrics.trust_tier {
            TrustTier::New => "New".to_string().yellow(),
            TrustTier::Established => "Established".to_string().blue(),
            TrustTier::Trusted => "Trusted".to_string().green(),
            TrustTier::Expert => "Expert".to_string().bright_green(),
        };

        println!(
            "{:<4} {:<66} {:<14} {:>11}% {:>10}",
            rank.to_string().bright_white(),
            pubkey_short,
            tier_str,
            format!("{:.1}", metrics.success_rate * 100.0).bright_white(),
            format!("{:.2}x", metrics.discovery_weight()).bright_white()
        );
    }

    println!();
    println!(
        "{}",
        format!("Showing top {} providers", all_ranked.len()).bright_black()
    );
    println!();

    Ok(())
}

/// Execute stats command
async fn execute_stats(json: bool) -> Result<()> {
    let aggregator = load_reputation_data()?;

    let total_providers = aggregator.get_all_ranked().len();
    let new_count = aggregator.get_by_tier(TrustTier::New).len();
    let established_count = aggregator.get_by_tier(TrustTier::Established).len();
    let trusted_count = aggregator.get_by_tier(TrustTier::Trusted).len();
    let expert_count = aggregator.get_by_tier(TrustTier::Expert).len();

    if json {
        let stats = serde_json::json!({
            "total_providers": total_providers,
            "new": new_count,
            "established": established_count,
            "trusted": trusted_count,
            "expert": expert_count,
        });
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("{}", "Marketplace Reputation Statistics".cyan().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    println!();

    println!("{}", "Total Providers".bright_white().bold());
    println!("  {}", total_providers.to_string().bright_white());
    println!();

    println!("{}", "By Trust Tier".bright_cyan().bold());
    println!("  {:<15} {}", "New:", new_count.to_string().yellow());
    println!(
        "  {:<15} {}",
        "Established:",
        established_count.to_string().blue()
    );
    println!("  {:<15} {}", "Trusted:", trusted_count.to_string().green());
    println!(
        "  {:<15} {}",
        "Expert:",
        expert_count.to_string().bright_green()
    );
    println!();

    Ok(())
}

/// Load reputation data from storage
///
/// In production, this would load from:
/// 1. Local database cache
/// 2. Nostr relays (NIP-32 label events)
/// 3. Aggregate and update cache
fn load_reputation_data() -> Result<ReputationAggregator> {
    // For now, return empty aggregator
    // TODO: Implement actual loading from database/Nostr
    Ok(ReputationAggregator::new())
}
