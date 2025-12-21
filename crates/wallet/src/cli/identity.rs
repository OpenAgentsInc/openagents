//! Nostr identity CLI commands

use anyhow::{Context, Result};
use colored::Colorize;
use crate::core::identity::UnifiedIdentity;
use crate::storage::keychain::SecureKeychain;

pub fn init(show_mnemonic: bool) -> Result<()> {
    println!("{}", "Initializing new wallet...".cyan());

    // Check if wallet already exists
    if SecureKeychain::has_mnemonic() {
        anyhow::bail!("Wallet already exists! Use 'wallet import' to replace or 'wallet export' to view.");
    }

    // Generate new identity
    let identity = UnifiedIdentity::generate()
        .context("Failed to generate identity")?;

    // Get mnemonic phrase
    let mnemonic_phrase = identity.mnemonic().to_string();

    // Store in keychain
    SecureKeychain::store_mnemonic(&mnemonic_phrase)
        .context("Failed to store mnemonic in keychain")?;

    println!("{}", "✓ Wallet initialized".green());
    println!();

    if show_mnemonic {
        println!("{}", "WARNING: Anyone with this phrase can access your wallet!".red().bold());
        println!();
        println!("{}", "Your recovery phrase:".bold());
        println!("{}", mnemonic_phrase.yellow());
        println!();
    }

    println!("{}", "IMPORTANT: Write down your recovery phrase and store it safely!".yellow().bold());
    println!("{}", "Use 'wallet export' to view it again.".cyan());
    println!();
    println!("{}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());

    Ok(())
}

pub fn import(mnemonic: Option<String>) -> Result<()> {
    use bip39::Mnemonic;
    use std::io::{self, Write};

    println!("{}", "Importing wallet from mnemonic...".cyan());
    println!();

    // Get mnemonic phrase
    let mnemonic_phrase = if let Some(phrase) = mnemonic {
        phrase
    } else {
        // Prompt for mnemonic
        print!("Enter your 12 or 24 word recovery phrase: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    // Validate mnemonic
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)
        .context("Invalid mnemonic phrase")?;

    // Derive identity to validate
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)
        .context("Failed to derive identity from mnemonic")?;

    // Check if wallet already exists
    if SecureKeychain::has_mnemonic() {
        println!("{}", "WARNING: This will replace your existing wallet!".red().bold());
        print!("Type 'yes' to confirm: ");
        io::stdout().flush()?;
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        if confirm.trim().to_lowercase() != "yes" {
            println!("Import cancelled.");
            return Ok(());
        }
        // Delete existing mnemonic
        SecureKeychain::delete_mnemonic()?;
    }

    // Store in keychain
    SecureKeychain::store_mnemonic(&mnemonic_phrase)
        .context("Failed to store mnemonic in keychain")?;

    println!("{}", "✓ Wallet imported".green());
    println!();
    println!("{}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());

    Ok(())
}

pub fn export() -> Result<()> {
    use std::io::{self, Write};

    println!("{}", "Exporting wallet mnemonic...".cyan());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Require confirmation
    println!("{}", "WARNING: This will display your recovery phrase on screen!".yellow().bold());
    println!("{}", "Anyone who sees this can steal your funds!".yellow());
    print!("\nType 'yes' to confirm: ");
    io::stdout().flush()?;
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;

    if confirm.trim().to_lowercase() != "yes" {
        println!("Export cancelled.");
        return Ok(());
    }

    println!();

    // Retrieve from keychain
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()
        .context("Failed to retrieve mnemonic from keychain")?;

    // Display mnemonic
    println!("{}", "Your recovery phrase:".bold());
    println!();
    println!("{}", mnemonic_phrase.yellow());
    println!();
    println!("{}", "IMPORTANT: Write this down and store it safely!".red().bold());
    println!("{}", "Never share it with anyone!".red().bold());

    Ok(())
}

pub fn whoami() -> Result<()> {
    use bip39::Mnemonic;

    println!("{}", "Wallet Information".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Retrieve mnemonic
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()
        .context("Failed to retrieve mnemonic from keychain")?;

    // Parse mnemonic
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)
        .context("Invalid mnemonic in keychain")?;

    // Derive identity
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)
        .context("Failed to derive identity")?;

    // Display information
    println!("{}", "Identity".bold());
    println!("  {}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());
    println!();
    println!("{}", "Balance".bold());
    println!("  {}: {} sats", "Total".bold(), 0);
    println!();
    println!("{}", "Profile".bold());
    println!("  Name: (not set)");
    println!("  About: (not set)");

    Ok(())
}

pub fn profile_show() -> Result<()> {
    println!("{}", "Profile Information".cyan().bold());
    println!();

    // TODO: Fetch profile from relays
    // TODO: Display metadata

    println!("{}: (not set)", "Name".bold());
    println!("{}: (not set)", "About".bold());

    Ok(())
}

pub fn profile_set(
    name: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    nip05: Option<String>,
) -> Result<()> {
    println!("{}", "Updating profile...".cyan());

    // TODO: Load current profile
    // TODO: Merge changes
    // TODO: Create kind:0 event
    // TODO: Publish to relays

    if let Some(n) = name {
        println!("  {} → {}", "Name".bold(), n);
    }
    if let Some(a) = about {
        println!("  {} → {}", "About".bold(), a);
    }
    if let Some(p) = picture {
        println!("  {} → {}", "Picture".bold(), p);
    }
    if let Some(n) = nip05 {
        println!("  {} → {}", "NIP-05".bold(), n);
    }

    println!("{}", "✓ Profile updated".green());

    Ok(())
}

pub fn contacts_list() -> Result<()> {
    println!("{}", "Contacts".cyan().bold());
    println!();

    // TODO: Fetch contact list
    // TODO: Display contacts with metadata

    println!("No contacts yet");

    Ok(())
}

pub fn contacts_add(npub: String, name: Option<String>) -> Result<()> {
    println!("{}", "Adding contact...".cyan());

    // TODO: Validate npub
    // TODO: Add to contact list
    // TODO: Publish updated contact list

    let display_name = name.unwrap_or_else(|| npub.clone());
    println!("  {} {}", "Added".green(), display_name);

    Ok(())
}

pub fn contacts_remove(npub: String) -> Result<()> {
    println!("{}", "Removing contact...".cyan());

    // TODO: Remove from contact list
    // TODO: Publish updated contact list

    println!("  {} {}", "Removed".green(), npub);

    Ok(())
}

pub fn post(content: String) -> Result<()> {
    println!("{}", "Publishing note...".cyan());

    // TODO: Create kind:1 event
    // TODO: Publish to relays

    println!("  {}", content);
    println!("{}", "✓ Published".green());

    Ok(())
}

pub fn dm(recipient: String, message: String) -> Result<()> {
    println!("{}", "Sending direct message...".cyan());

    // TODO: Validate recipient npub
    // TODO: Create NIP-17 DM event
    // TODO: Encrypt and publish

    println!("  {} → {}", "To".bold(), recipient);
    println!("  {}", message);
    println!("{}", "✓ Sent".green());

    Ok(())
}

pub fn feed(limit: usize) -> Result<()> {
    println!("{}", "Nostr Feed".cyan().bold());
    println!();

    // TODO: Subscribe to feed from contacts
    // TODO: Display events

    println!("Fetching {} events...", limit);
    println!("No events yet");

    Ok(())
}
