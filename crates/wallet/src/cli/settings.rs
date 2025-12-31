//! Wallet settings and configuration CLI commands

use crate::storage::config::WalletConfig;
use anyhow::Result;
use colored::Colorize;

pub fn show() -> Result<()> {
    println!("{}", "Wallet Settings".cyan().bold());
    println!();

    let config = WalletConfig::load()?;

    println!("[network]");
    println!("  bitcoin = \"{}\"", config.network.bitcoin);
    println!();
    println!("[nostr]");
    println!("  relays = [");
    for relay in &config.nostr.relays {
        println!("    \"{}\",", relay);
    }
    println!("  ]");
    println!();
    println!("[storage]");
    println!("  db_path = \"{}\"", config.storage.db_path);
    println!("  backup_enabled = {}", config.storage.backup_enabled);
    println!();
    println!("[security]");
    println!(
        "  max_send_sats = {}",
        format_optional_sats(config.security.max_send_sats)
    );
    println!(
        "  confirm_large_sats = {}",
        format_optional_sats(config.security.confirm_large_sats)
    );

    Ok(())
}

pub fn set(key: String, value: String) -> Result<()> {
    let mut config = WalletConfig::load()?;

    match key.as_str() {
        "security.max_send_sats" | "max_send_sats" => {
            config.security.max_send_sats = parse_optional_sats(&value)?;
        }
        "security.confirm_large_sats" | "confirm_large_sats" => {
            config.security.confirm_large_sats = parse_optional_sats(&value)?;
        }
        _ => {
            anyhow::bail!(
                "Unknown setting '{}'. Supported keys: security.max_send_sats, security.confirm_large_sats.",
                key
            );
        }
    }

    config.save()?;
    println!("{}", "✓ Setting updated".green());
    Ok(())
}

pub fn relays_list() -> Result<()> {
    println!("{}", "Configured Relays".cyan().bold());
    println!();

    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!("No relays configured yet");
        println!();
        println!("{}", "Add relays with: wallet relays add <url>".yellow());
    } else {
        for (i, relay) in config.nostr.relays.iter().enumerate() {
            println!("  {}. {}", i + 1, relay);
        }
        println!();
        println!("{} relay(s) configured", config.nostr.relays.len());
    }

    Ok(())
}

pub fn relays_add(url: String, _marker: Option<String>) -> Result<()> {
    println!("{}", "Adding relay...".cyan());

    // Validate URL format
    if !url.starts_with("wss://") && !url.starts_with("ws://") {
        anyhow::bail!("Relay URL must start with wss:// or ws://");
    }

    let mut config = WalletConfig::load()?;

    // Check if relay already exists
    if config.nostr.relays.contains(&url) {
        anyhow::bail!("Relay already configured: {}", url);
    }

    // Add relay
    config.nostr.relays.push(url.clone());
    config.save()?;

    println!("  {}", url);
    println!("{}", "✓ Relay added".green());

    Ok(())
}

pub fn relays_remove(url: String) -> Result<()> {
    println!("{}", "Removing relay...".cyan());

    let mut config = WalletConfig::load()?;

    // Find and remove relay
    let original_len = config.nostr.relays.len();
    config.nostr.relays.retain(|r| r != &url);

    if config.nostr.relays.len() == original_len {
        anyhow::bail!("Relay not found: {}", url);
    }

    config.save()?;

    println!("  {}", url);
    println!("{}", "✓ Relay removed".green());

    Ok(())
}

fn parse_optional_sats(value: &str) -> Result<Option<u64>> {
    let trimmed = value.trim().to_lowercase();
    if trimmed.is_empty() || trimmed == "none" || trimmed == "off" || trimmed == "null" {
        return Ok(None);
    }

    let cleaned = trimmed.replace('_', "").replace(',', "");
    let sats: u64 = cleaned
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid sats value: {}", value))?;
    if sats == 0 {
        anyhow::bail!("Value must be greater than zero or 'none'.");
    }
    Ok(Some(sats))
}

fn format_optional_sats(value: Option<u64>) -> String {
    match value {
        Some(sats) => sats.to_string(),
        None => "none".to_string(),
    }
}
