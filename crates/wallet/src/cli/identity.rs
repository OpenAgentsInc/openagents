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
    use crate::core::nostr::Profile;
    use crate::core::nip05::verify_nip05_cached;
    use crate::storage::config::WalletConfig;

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

    // Try to load profile
    let config = WalletConfig::load()?;
    let profile_path = config.profile_path()?;

    let profile = if profile_path.exists() {
        let content = std::fs::read_to_string(&profile_path)?;
        Profile::from_json(&content)?
    } else {
        Profile::default()
    };

    // Display information
    println!("{}", "Identity".bold());
    println!("  {}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());
    println!();
    println!("{}", "Balance".bold());
    println!("  {}: {} sats", "Total".bold(), 0);
    println!();
    println!("{}", "Profile".bold());
    if profile.is_empty() {
        println!("  (not set - use 'wallet profile set' to create)");
    } else {
        if let Some(name) = &profile.name {
            println!("  {}: {}", "Name".bold(), name);
        }
        if let Some(about) = &profile.about {
            println!("  {}: {}", "About".bold(), about);
        }
        if let Some(nip05) = &profile.nip05 {
            // Verify NIP-05
            let verified = verify_nip05_cached(nip05, identity.nostr_public_key())
                .unwrap_or(false);

            if verified {
                println!("  {}: {} {}", "NIP-05".bold(), nip05, "✓".green());
            } else {
                println!("  {}: {} {}", "NIP-05".bold(), nip05, "✗".dimmed());
            }
        }
    }

    Ok(())
}

pub fn profile_show() -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::nostr::Profile;
    use crate::core::nip05::verify_nip05_cached;
    use crate::storage::config::WalletConfig;

    println!("{}", "Profile Information".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity for pubkey
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;

    // Try to load profile from config
    let config = WalletConfig::load()?;
    let profile_path = config.profile_path()?;

    let profile = if profile_path.exists() {
        let content = std::fs::read_to_string(&profile_path)?;
        Profile::from_json(&content)?
    } else {
        Profile::default()
    };

    // Display profile
    if profile.is_empty() {
        println!("  Profile not set. Use 'wallet profile set' to create one.");
    } else {
        if let Some(name) = &profile.name {
            println!("  {}: {}", "Name".bold(), name);
        }
        if let Some(about) = &profile.about {
            println!("  {}: {}", "About".bold(), about);
        }
        if let Some(picture) = &profile.picture {
            println!("  {}: {}", "Picture".bold(), picture);
        }
        if let Some(nip05) = &profile.nip05 {
            // Verify NIP-05
            let verified = verify_nip05_cached(nip05, identity.nostr_public_key())
                .unwrap_or(false);

            if verified {
                println!("  {}: {} {}", "NIP-05".bold(), nip05, "✓".green());
            } else {
                println!("  {}: {} {}", "NIP-05".bold(), nip05, "✗".red());
            }
        }
    }

    Ok(())
}

pub fn profile_set(
    name: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    nip05: Option<String>,
) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::nostr::{Profile, ProfileUpdate, create_profile_event};
    use crate::storage::config::WalletConfig;

    println!("{}", "Updating profile...".cyan());

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;

    // Load or create profile
    let config = WalletConfig::load()?;
    let profile_path = config.profile_path()?;

    let mut profile = if profile_path.exists() {
        let content = std::fs::read_to_string(&profile_path)?;
        Profile::from_json(&content)?
    } else {
        Profile::default()
    };

    // Merge updates
    let updates = ProfileUpdate {
        name: name.clone(),
        about: about.clone(),
        picture: picture.clone(),
        nip05: nip05.clone(),
    };
    profile.merge(updates);

    // Save profile locally
    if let Some(parent) = profile_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&profile_path, profile.to_json())?;

    // Create and sign the event
    let event = create_profile_event(&identity, &profile)?;

    println!();
    if let Some(n) = &name {
        println!("  {} → {}", "Name".bold(), n);
    }
    if let Some(a) = &about {
        println!("  {} → {}", "About".bold(), a);
    }
    if let Some(p) = &picture {
        println!("  {} → {}", "Picture".bold(), p);
    }
    if let Some(n) = &nip05 {
        println!("  {} → {}", "NIP-05".bold(), n);
    }

    println!();
    println!("{}", "✓ Profile updated locally".green());

    // Publish to relays
    if !config.nostr.relays.is_empty() {
        println!();
        println!("{}", "Publishing to relays...".cyan());

        use crate::core::client::NostrClient;

        let client = NostrClient::new(config.nostr.relays.clone());
        let rt = tokio::runtime::Runtime::new()?;
        let results = rt.block_on(client.publish_event(&event))?;

        let mut success_count = 0;
        let mut fail_count = 0;

        for result in results {
            if result.is_success() {
                println!("  {} {}", "✓".green(), result.relay_url);
                success_count += 1;
            } else {
                let error = result.error_message().unwrap_or_else(|| "Unknown error".to_string());
                println!("  {} {} - {}", "✗".red(), result.relay_url, error);
                fail_count += 1;
            }
        }

        println!();
        if success_count > 0 {
            println!("{} Published to {}/{} relay(s)", "✓".green(), success_count, success_count + fail_count);
        }
        if fail_count > 0 {
            println!("{} {} relay(s) failed", "⚠".yellow(), fail_count);
        }
    } else {
        println!();
        println!("{}", "Note: No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
    }

    println!();
    println!("{}: {}", "Event ID".bold(), event.id);

    Ok(())
}

pub fn contacts_list() -> Result<()> {
    // Contact list management requires Nostr relay integration which is not yet implemented.
    // Per d-012 (No Stubs), we return an explicit error instead of pretending to work.
    Err(anyhow::anyhow!(
        "Contact list management not yet implemented. Requires Nostr relay client integration for fetching and publishing kind:3 contact list events."
    ))
}

pub fn contacts_add(_npub: String, _name: Option<String>) -> Result<()> {
    // Contact list management requires Nostr relay integration which is not yet implemented.
    // Per d-012 (No Stubs), we return an explicit error instead of pretending to work.
    Err(anyhow::anyhow!(
        "Contact list management not yet implemented. Requires Nostr relay client integration for fetching and publishing kind:3 contact list events."
    ))
}

pub fn contacts_remove(_npub: String) -> Result<()> {
    // Contact list management requires Nostr relay integration which is not yet implemented.
    // Per d-012 (No Stubs), we return an explicit error instead of pretending to work.
    Err(anyhow::anyhow!(
        "Contact list management not yet implemented. Requires Nostr relay client integration for fetching and publishing kind:3 contact list events."
    ))
}

pub fn post(content: String) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::nostr::create_note_event;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;

    println!("{}", "Publishing note...".cyan());

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity
    let mnemonic_phrase = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;

    // Create note event
    let event = create_note_event(&identity, &content)?;

    println!("  {}", content);
    println!();

    // Load config and publish to relays
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!("{}", "⚠ No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        println!("{}: {}", "Event ID".bold(), event.id);
        return Ok(());
    }

    println!("{}", "Publishing to relays...".cyan());

    let client = NostrClient::new(config.nostr.relays.clone());
    let rt = tokio::runtime::Runtime::new()?;
    let results = rt.block_on(client.publish_event(&event))?;

    let mut success_count = 0;
    let mut fail_count = 0;

    for result in results {
        if result.is_success() {
            println!("  {} {}", "✓".green(), result.relay_url);
            success_count += 1;
        } else {
            let error = result.error_message().unwrap_or_else(|| "Unknown error".to_string());
            println!("  {} {} - {}", "✗".red(), result.relay_url, error);
            fail_count += 1;
        }
    }

    println!();
    if success_count > 0 {
        println!("{} Published to {}/{} relay(s)", "✓".green(), success_count, success_count + fail_count);
    }
    if fail_count > 0 {
        println!("{} {} relay(s) failed", "⚠".yellow(), fail_count);
    }

    println!();
    println!("{}: {}", "Event ID".bold(), event.id);

    Ok(())
}

pub fn dm_send(recipient: String, message: String) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;

    println!("{}", "Sending encrypted DM (NIP-04)...".cyan());

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Parse recipient npub
    let recipient_pubkey = match nostr::decode(&recipient) {
        Ok(nostr::Nip19Entity::Pubkey(pk)) => pk,
        Ok(nostr::Nip19Entity::Profile(p)) => p.pubkey,
        _ => anyhow::bail!("Invalid recipient. Expected npub or nprofile."),
    };

    // Load identity
    let mnemonic_str = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_str)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;

    // Get sender's private key bytes (nostr_secret_key returns hex string)
    let sender_privkey_hex = identity.nostr_secret_key();
    let sender_privkey_vec = hex::decode(sender_privkey_hex)?;
    let sender_privkey: [u8; 32] = sender_privkey_vec.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;

    // Convert recipient pubkey to compressed format (33 bytes with 0x02 or 0x03 prefix)
    // The NIP-04 encrypt function expects a secp256k1 public key
    let mut recipient_pk_compressed = [0u8; 33];
    recipient_pk_compressed[0] = 0x02; // Even y-coordinate (standard convention for x-only pubkeys)
    recipient_pk_compressed[1..].copy_from_slice(&recipient_pubkey);

    // Encrypt message using NIP-04
    let encrypted_content = nostr::encrypt(&sender_privkey, &recipient_pk_compressed, &message)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    // Create kind:4 event with encrypted content and p-tag
    let template = nostr::EventTemplate {
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        kind: nostr::ENCRYPTED_DM_KIND,
        tags: vec![
            vec!["p".to_string(), hex::encode(recipient_pubkey)],
        ],
        content: encrypted_content,
    };

    let event = identity.sign_event(template)
        .context("Failed to sign DM event")?;

    println!("  {} → {}", "To".bold(), &recipient[..20]);
    println!("  {}", message);
    println!();

    // Load config and publish to relays
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!("{}", "⚠ No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        println!("{}: {}", "Event ID".bold(), event.id);
        return Ok(());
    }

    println!("{}", "Publishing to relays...".cyan());

    let client = NostrClient::new(config.nostr.relays.clone());
    let rt = tokio::runtime::Runtime::new()?;
    let results = rt.block_on(client.publish_event(&event))?;

    let mut success_count = 0;
    let mut fail_count = 0;

    for result in results {
        if result.is_success() {
            success_count += 1;
        } else {
            fail_count += 1;
        }
    }

    println!();
    if success_count > 0 {
        println!("{} Sent to {}/{} relay(s)", "✓".green(), success_count, success_count + fail_count);
    }
    if fail_count > 0 {
        println!("{} {} relay(s) failed", "⚠".yellow(), fail_count);
    }

    println!();
    println!("{}: {}", "Event ID".bold(), event.id);

    Ok(())
}

pub fn dm_list(limit: usize) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;
    use chrono::{DateTime, Utc};

    println!("{}", "Direct Messages".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity
    let mnemonic_str = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_str)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;

    let our_pubkey_hex = identity.nostr_public_key();

    // Load config
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!("{}", "⚠ No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        return Ok(());
    }

    println!("Fetching encrypted DMs from relays...");
    println!();

    // Fetch DMs (kind:4 events with p tag matching our pubkey)
    let client = NostrClient::new(config.nostr.relays.clone());
    let rt = tokio::runtime::Runtime::new()?;

    // Create filter for DMs sent to us
    let filter = serde_json::json!({
        "kinds": [nostr::ENCRYPTED_DM_KIND],
        "#p": [our_pubkey_hex],
        "limit": limit
    });

    let events = rt.block_on(client.fetch_events(vec![filter]))?;

    if events.is_empty() {
        println!("No direct messages found");
        return Ok(());
    }

    // Get sender's private key for decryption
    let sender_privkey_hex = identity.nostr_secret_key();
    let sender_privkey_vec = hex::decode(sender_privkey_hex)?;
    let sender_privkey: [u8; 32] = sender_privkey_vec.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;

    // Display DMs
    for (i, event) in events.iter().enumerate() {
        // Format timestamp
        let dt = DateTime::<Utc>::from_timestamp(event.created_at as i64, 0)
            .unwrap_or(DateTime::UNIX_EPOCH);
        let time_str = dt.format("%Y-%m-%d %H:%M:%S UTC");

        // Truncate sender pubkey for display
        let sender_short = if event.pubkey.len() > 16 {
            format!("{}...", &event.pubkey[..16])
        } else {
            event.pubkey.clone()
        };

        // Try to parse sender pubkey and decrypt
        let decrypted = if let Ok(sender_bytes) = hex::decode(&event.pubkey) {
            if sender_bytes.len() == 32 {
                let mut sender_pk_compressed = [0u8; 33];
                sender_pk_compressed[0] = 0x02; // Even y-coordinate
                sender_pk_compressed[1..].copy_from_slice(&sender_bytes);

                nostr::decrypt(&sender_privkey, &sender_pk_compressed, &event.content)
                    .unwrap_or_else(|_| "[Decryption failed]".to_string())
            } else {
                "[Invalid sender pubkey]".to_string()
            }
        } else {
            "[Invalid sender pubkey]".to_string()
        };

        println!("{} {}", format!("{}.", i + 1).bold(), time_str.to_string().dimmed());
        println!("  {} {}", "From:".bold(), sender_short.dimmed());
        println!("  {} {}", "ID:".bold(), event.id.dimmed());
        println!("  {}", decrypted);
        println!();
    }

    println!("{} message(s) displayed", events.len());

    Ok(())
}

pub fn dm_read(event_id: String) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;
    use chrono::{DateTime, Utc};

    println!("{}", "Reading Direct Message".cyan().bold());
    println!();

    // Check if wallet exists
    if !SecureKeychain::has_mnemonic() {
        anyhow::bail!("No wallet found. Use 'wallet init' to create one.");
    }

    // Load identity
    let mnemonic_str = SecureKeychain::retrieve_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_str)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;

    // Load config
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!("{}", "⚠ No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        return Ok(());
    }

    // Fetch specific event by ID
    let client = NostrClient::new(config.nostr.relays.clone());
    let rt = tokio::runtime::Runtime::new()?;

    let filter = serde_json::json!({
        "ids": [event_id],
        "kinds": [nostr::ENCRYPTED_DM_KIND]
    });

    let events = rt.block_on(client.fetch_events(vec![filter]))?;

    if events.is_empty() {
        println!("Message not found");
        return Ok(());
    }

    let event = &events[0];

    // Get sender's private key for decryption
    let sender_privkey_hex = identity.nostr_secret_key();
    let sender_privkey_vec = hex::decode(sender_privkey_hex)?;
    let sender_privkey: [u8; 32] = sender_privkey_vec.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;

    // Decrypt message
    let decrypted = if let Ok(sender_bytes) = hex::decode(&event.pubkey) {
        if sender_bytes.len() == 32 {
            let mut sender_pk_compressed = [0u8; 33];
            sender_pk_compressed[0] = 0x02; // Even y-coordinate
            sender_pk_compressed[1..].copy_from_slice(&sender_bytes);

            nostr::decrypt(&sender_privkey, &sender_pk_compressed, &event.content)
                .unwrap_or_else(|_| "[Decryption failed]".to_string())
        } else {
            "[Invalid sender pubkey]".to_string()
        }
    } else {
        "[Invalid sender pubkey]".to_string()
    };

    // Format timestamp
    let dt = DateTime::<Utc>::from_timestamp(event.created_at as i64, 0)
        .unwrap_or_else(|| DateTime::UNIX_EPOCH);
    let time_str = dt.format("%Y-%m-%d %H:%M:%S UTC");

    println!("{} {}", "From:".bold(), event.pubkey);
    println!("{} {}", "Time:".bold(), time_str);
    println!("{} {}", "Event ID:".bold(), event.id);
    println!();
    println!("{}", decrypted);

    Ok(())
}

pub fn feed(limit: usize) -> Result<()> {
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;
    use chrono::{DateTime, Utc};

    println!("{}", "Nostr Feed".cyan().bold());
    println!();

    // Load config
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!("{}", "⚠ No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        return Ok(());
    }

    println!("Fetching {} events from {}...", limit, config.nostr.relays[0]);
    println!();

    // Fetch feed
    let client = NostrClient::new(config.nostr.relays.clone());
    let rt = tokio::runtime::Runtime::new()?;
    let events = rt.block_on(client.fetch_feed(limit))?;

    if events.is_empty() {
        println!("No events found");
        return Ok(());
    }

    // Display events
    for (i, event) in events.iter().enumerate() {
        // Format timestamp
        let dt = DateTime::<Utc>::from_timestamp(event.created_at as i64, 0)
            .unwrap_or(DateTime::UNIX_EPOCH);
        let time_str = dt.format("%Y-%m-%d %H:%M:%S UTC");

        // Truncate pubkey for display
        let pubkey_short = if event.pubkey.len() > 16 {
            format!("{}...", &event.pubkey[..16])
        } else {
            event.pubkey.clone()
        };

        println!("{} {}", format!("{}.", i + 1).bold(), time_str.to_string().dimmed());
        println!("  {} {}", "From:".bold(), pubkey_short.dimmed());
        println!("  {}", event.content);
        println!();
    }

    println!("{} events displayed", events.len());

    Ok(())
}
