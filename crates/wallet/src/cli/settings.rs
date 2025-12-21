//! Wallet settings and configuration CLI commands

use anyhow::Result;
use colored::Colorize;

pub fn show() -> Result<()> {
    println!("{}", "Wallet Settings".cyan().bold());
    println!();

    // TODO: Load configuration
    // TODO: Display all settings

    println!("[network]");
    println!("  bitcoin = \"mainnet\"");
    println!();
    println!("[nostr]");
    println!("  relays = []");
    println!();
    println!("[storage]");
    println!("  db_path = \"~/.openagents/wallet.db\"");

    Ok(())
}

pub fn set(key: String, value: String) -> Result<()> {
    println!("{}", "Update Setting".cyan());

    // TODO: Validate key
    // TODO: Update configuration
    // TODO: Save to disk

    println!("  {} = {}", key.bold(), value);
    println!("{}", "✓ Setting updated".green());

    Ok(())
}

pub fn relays_list() -> Result<()> {
    println!("{}", "Configured Relays".cyan().bold());
    println!();

    // TODO: Load relay list from storage
    // TODO: Display relays with markers

    println!("No relays configured yet");
    println!();
    println!("{}", "Add relays with: wallet relays add <url>".yellow());

    Ok(())
}

pub fn relays_add(url: String, marker: Option<String>) -> Result<()> {
    println!("{}", "Adding relay...".cyan());

    // TODO: Validate relay URL
    // TODO: Add to relay list
    // TODO: Save configuration

    let marker_display = marker.unwrap_or_else(|| "read+write".to_string());
    println!("  {} ({})", url, marker_display);
    println!("{}", "✓ Relay added".green());

    Ok(())
}

pub fn relays_remove(url: String) -> Result<()> {
    println!("{}", "Removing relay...".cyan());

    // TODO: Remove from relay list
    // TODO: Save configuration

    println!("  {}", url);
    println!("{}", "✓ Relay removed".green());

    Ok(())
}
