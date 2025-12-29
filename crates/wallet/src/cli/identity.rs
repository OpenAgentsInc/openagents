//! Nostr identity CLI commands

use anyhow::{Context, Result};
use colored::Colorize;
use crate::cli::load_mnemonic;
use crate::cli::error::{WalletError, format_error_with_hint};
use crate::core::identity::UnifiedIdentity;
use spark::{Network, SparkSigner, SparkWallet, WalletConfig as SparkWalletConfig};
use crate::storage::identities::{
    current_identity, register_identity, remove_identity as unregister_identity,
    set_current_identity, IdentityRegistry, DEFAULT_IDENTITY_NAME,
};
use crate::storage::keychain::SecureKeychain;
use std::io::IsTerminal;
use std::path::PathBuf;

fn spark_address_from_mnemonic(mnemonic: &str) -> Option<String> {
    let signer = SparkSigner::from_mnemonic(mnemonic, "").ok()?;
    let network = if std::env::var("MAINNET").is_ok() {
        Network::Mainnet
    } else {
        Network::Regtest
    };
    let config = SparkWalletConfig {
        network,
        api_key: std::env::var("BREEZ_API_KEY").ok(),
        ..Default::default()
    };
    let runtime = tokio::runtime::Runtime::new().ok()?;
    runtime.block_on(async {
        let wallet = SparkWallet::new(signer, config).await.ok()?;
        wallet.get_spark_address().await.ok()
    })
}

fn validate_identity_name(name: &str) -> Result<(), WalletError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(WalletError::InvalidIdentityName(
            "Identity name cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > 50 {
        return Err(WalletError::InvalidIdentityName(
            "Identity name too long (max 50 characters)".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(WalletError::InvalidIdentityName(
            "Identity name must be ASCII letters, numbers, '-' or '_'".to_string(),
        ));
    }
    Ok(())
}

pub fn init(show_mnemonic: bool) -> Result<()> {
    println!("{}", "Initializing new wallet...".cyan());

    if SecureKeychain::has_mnemonic_for(DEFAULT_IDENTITY_NAME) {
        let error = WalletError::WalletAlreadyExists;
        eprintln!("{}", format_error_with_hint(&error));
        return Err(error.into());
    }

    let identity = UnifiedIdentity::generate()
        .context("Failed to generate identity")?;

    let mnemonic_phrase = identity.mnemonic().to_string();

    SecureKeychain::store_mnemonic(&mnemonic_phrase)
        .context("Failed to store mnemonic in keychain")?;
    register_identity(DEFAULT_IDENTITY_NAME, true)
        .context("Failed to register default identity")?;

    println!("{} Wallet initialized", "✓".green());
    println!();

    if show_mnemonic {
        println!("{}", "WARNING: Anyone with this phrase can access your wallet!".red().bold());
        println!();
        println!("{}", "Your recovery phrase:".bold());
        println!("{}", mnemonic_phrase.yellow());
        println!();
    }

    println!("{}", "IMPORTANT: Write down your recovery phrase and store it safely!".yellow().bold());
    println!("{}", "Use 'openagents wallet export' to view it again.".cyan());
    println!();
    println!("{}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());

    Ok(())
}

pub fn import(mnemonic: Option<String>) -> Result<()> {
    use bip39::Mnemonic;
    use std::io::{self, Write};

    println!("{}", "Importing wallet from mnemonic...".cyan());
    println!();

    let mnemonic_phrase = if let Some(phrase) = mnemonic {
        phrase
    } else {
        print!("Enter your 12 or 24 word recovery phrase: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    let mnemonic = Mnemonic::parse(&mnemonic_phrase).map_err(|e| {
        let error = WalletError::InvalidMnemonic(e.to_string());
        eprintln!("{}", format_error_with_hint(&error));
        anyhow::anyhow!("{}", e)
    })?;

    let identity = UnifiedIdentity::from_mnemonic(mnemonic)
        .context("Failed to derive identity from mnemonic")?;

    if SecureKeychain::has_mnemonic_for(DEFAULT_IDENTITY_NAME) {
        println!("{}", "WARNING: This will replace your existing wallet!".red().bold());
        print!("Type 'yes' to confirm: ");
        io::stdout().flush()?;
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        if confirm.trim().to_lowercase() != "yes" {
            println!("Import cancelled.");
            return Ok(());
        }
        SecureKeychain::delete_mnemonic_for(DEFAULT_IDENTITY_NAME)?;
    }

    SecureKeychain::store_mnemonic(&mnemonic_phrase)
        .context("Failed to store mnemonic in keychain")?;
    register_identity(DEFAULT_IDENTITY_NAME, true)
        .context("Failed to register default identity")?;

    println!("{} Wallet imported", "✓".green());
    println!();
    println!("{}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());

    Ok(())
}

pub fn identities_list() -> Result<()> {
    let registry = IdentityRegistry::load()?;
    println!("{}", "Identities".cyan().bold());
    println!();

    if registry.identities().is_empty() {
        println!("  No identities configured.");
        println!("  Use 'openagents wallet identity create <name>' to add one.");
        return Ok(());
    }

    for name in registry.identities() {
        if name == registry.current() {
            println!("* {}", name);
        } else {
            println!("  {}", name);
        }
    }

    Ok(())
}

pub fn identity_current() -> Result<()> {
    let current = current_identity()?;
    println!("Current identity: {}", current);
    Ok(())
}

pub fn identity_create(name: String, show_mnemonic: bool) -> Result<()> {
    if let Err(e) = validate_identity_name(&name) {
        eprintln!("{}", format_error_with_hint(&e));
        return Err(e.into());
    }

    let mut registry = IdentityRegistry::load()?;
    if registry.contains(&name) {
        let error = WalletError::IdentityAlreadyExists(name.clone());
        eprintln!("{}", format_error_with_hint(&error));
        return Err(error.into());
    }

    println!("{}", format!("Creating identity '{}'...", name).cyan());

    let identity = UnifiedIdentity::generate()
        .context("Failed to generate identity")?;
    let mnemonic_phrase = identity.mnemonic().to_string();

    SecureKeychain::store_mnemonic_for(&name, &mnemonic_phrase)
        .context("Failed to store mnemonic in keychain")?;
    registry.add_identity(&name)?;
    registry.set_current(&name)?;
    registry.save()?;

    println!("{} Identity created", "✓".green());
    println!("{}", format!("Active identity: {}", name).cyan());
    println!();
    println!("{}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());

    if show_mnemonic {
        println!();
        println!("{}", "WARNING: Anyone with this phrase can access your wallet!".red().bold());
        println!("{}", "Your recovery phrase:".bold());
        println!("{}", mnemonic_phrase.yellow());
    }

    Ok(())
}

pub fn identity_import(name: String, mnemonic: Option<String>) -> Result<()> {
    use bip39::Mnemonic;
    use std::io::{self, Write};

    if let Err(e) = validate_identity_name(&name) {
        eprintln!("{}", format_error_with_hint(&e));
        return Err(e.into());
    }

    let mut registry = IdentityRegistry::load()?;
    if registry.contains(&name) {
        let error = WalletError::IdentityAlreadyExists(name.clone());
        eprintln!("{}", format_error_with_hint(&error));
        return Err(error.into());
    }

    println!("{}", format!("Importing identity '{}'...", name).cyan());
    println!();

    let mnemonic_phrase = if let Some(phrase) = mnemonic {
        phrase
    } else {
        print!("Enter your 12 or 24 word recovery phrase: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    let mnemonic = Mnemonic::parse(&mnemonic_phrase).map_err(|e| {
        let error = WalletError::InvalidMnemonic(e.to_string());
        eprintln!("{}", format_error_with_hint(&error));
        anyhow::anyhow!("{}", e)
    })?;

    let identity = UnifiedIdentity::from_mnemonic(mnemonic)
        .context("Failed to derive identity from mnemonic")?;

    SecureKeychain::store_mnemonic_for(&name, &mnemonic_phrase)
        .context("Failed to store mnemonic in keychain")?;
    registry.add_identity(&name)?;
    registry.set_current(&name)?;
    registry.save()?;

    println!("{} Identity imported", "✓".green());
    println!("{}", format!("Active identity: {}", name).cyan());
    println!("{}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());

    Ok(())
}

pub fn identity_use(name: String) -> Result<()> {
    if let Err(e) = validate_identity_name(&name) {
        eprintln!("{}", format_error_with_hint(&e));
        return Err(e.into());
    }

    set_current_identity(&name).map_err(|e| {
        let error = WalletError::IdentityNotFound(name.clone());
        eprintln!("{}", format_error_with_hint(&error));
        anyhow::anyhow!("{}", e)
    })?;

    println!("{} Active identity set to '{}'.", "✓".green(), name);
    Ok(())
}

pub fn identity_remove(name: String, yes: bool) -> Result<()> {
    use std::io::{self, Write};

    if let Err(e) = validate_identity_name(&name) {
        eprintln!("{}", format_error_with_hint(&e));
        return Err(e.into());
    }

    if !yes {
        if std::io::stdin().is_terminal() {
            print!("Type 'yes' to remove identity '{}': ", name);
            io::stdout().flush()?;
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;
            if confirm.trim().to_lowercase() != "yes" {
                println!("Remove cancelled.");
                return Ok(());
            }
        } else {
            let error = WalletError::ConfirmationRequired;
            eprintln!("{}", format_error_with_hint(&error));
            return Err(error.into());
        }
    }

    unregister_identity(&name).map_err(|e| {
        let error = WalletError::IdentityNotFound(name.clone());
        eprintln!("{}", format_error_with_hint(&error));
        anyhow::anyhow!("{}", e)
    })?;

    SecureKeychain::delete_mnemonic_for(&name)?;
    println!("{} Identity '{}' removed.", "✓".green(), name);
    Ok(())
}

pub fn export() -> Result<()> {
    use std::io::{self, Write};

    println!("{}", "Exporting wallet mnemonic...".cyan());
    println!();

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
    let mnemonic_phrase = load_mnemonic()?;

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

    // Retrieve mnemonic
    let mnemonic_phrase = load_mnemonic()?;

    // Parse mnemonic
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)
        .context("Invalid mnemonic in keychain")?;

    // Derive identity
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)
        .context("Failed to derive identity")?;

    let npub = identity.npub().context("Failed to encode npub")?;

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
    let identity_name = current_identity().unwrap_or_else(|_| DEFAULT_IDENTITY_NAME.to_string());
    println!("  {}: {}", "Active Identity".bold(), identity_name);
    println!("  {}: {}", "Nostr npub".bold(), npub);
    println!("  {}: {}", "Nostr Public Key".bold(), identity.nostr_public_key());
    let spark_address = spark_address_from_mnemonic(&mnemonic_phrase)
        .unwrap_or_else(|| "unavailable".to_string());
    println!("  {}: {}", "Spark Address (Lightning)".bold(), spark_address);
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

    // Load identity for pubkey
    let mnemonic_phrase = load_mnemonic()?;
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

    // Load identity
    let mnemonic_phrase = load_mnemonic()?;
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

fn contacts_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let identity = current_identity().unwrap_or_else(|_| DEFAULT_IDENTITY_NAME.to_string());
    Ok(home.join(".openagents").join("contacts").join(format!("{}.json", identity)))
}

fn load_contacts() -> Result<Vec<nostr::Contact>> {
    let path = contacts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&path)?;
    let contacts = serde_json::from_str(&contents)?;
    Ok(contacts)
}

fn save_contacts(contacts: &[nostr::Contact]) -> Result<()> {
    let path = contacts_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(contacts)?;
    std::fs::write(&path, contents)?;
    Ok(())
}

fn parse_contact_pubkey(value: &str) -> Result<String> {
    if value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(value.to_string());
    }

    match nostr::decode(value) {
        Ok(nostr::Nip19Entity::Pubkey(pk)) => Ok(hex::encode(pk)),
        Ok(nostr::Nip19Entity::Profile(profile)) => Ok(hex::encode(profile.pubkey)),
        _ => anyhow::bail!("Invalid contact. Expected npub, nprofile, or 64-char hex pubkey."),
    }
}

fn create_contact_list_event(
    identity: &UnifiedIdentity,
    contacts: &[nostr::Contact],
) -> Result<nostr::Event> {
    let tags = contacts.iter().map(|contact| contact.to_tag()).collect();
    let template = nostr::EventTemplate {
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        kind: nostr::CONTACT_LIST_KIND,
        tags,
        content: "".to_string(),
    };

    identity.sign_event(template)
        .context("Failed to sign contact list event")
}

pub fn contacts_list() -> Result<()> {
    println!("{}", "Contacts".cyan().bold());

    let contacts = load_contacts()?;

    if contacts.is_empty() {
        println!("No contacts yet. Use 'wallet contacts add <npub>' to follow someone.");
        return Ok(());
    }

    println!("Following: {}", contacts.len());
    println!();

    for (index, contact) in contacts.iter().enumerate() {
        if let Some(name) = &contact.petname {
            println!(
                "{}. {} ({})",
                index + 1,
                name,
                contact.pubkey
            );
        } else {
            println!("{}. {}", index + 1, contact.pubkey);
        }
    }

    Ok(())
}

pub fn contacts_add(npub: String, name: Option<String>) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;

    println!("{}", "Adding contact...".cyan());

    let pubkey_hex = parse_contact_pubkey(&npub)?;
    let mut contacts = load_contacts()?;

    if let Some(existing) = contacts.iter_mut().find(|contact| contact.pubkey == pubkey_hex) {
        existing.petname = name.clone();
    } else {
        contacts.push(nostr::Contact {
            pubkey: pubkey_hex.clone(),
            relay_url: None,
            petname: name.clone(),
        });
    }

    save_contacts(&contacts)?;

    let mnemonic_phrase = load_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;
    let event = create_contact_list_event(&identity, &contacts)?;

    println!("{} Followed {}", "✓".green(), pubkey_hex);

    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!();
        println!("{}", "Note: No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        println!("{}: {}", "Event ID".bold(), event.id);
        return Ok(());
    }

    println!();
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

    Ok(())
}

pub fn contacts_remove(npub: String) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;

    println!("{}", "Removing contact...".cyan());

    let pubkey_hex = parse_contact_pubkey(&npub)?;
    let mut contacts = load_contacts()?;
    let original_len = contacts.len();

    contacts.retain(|contact| contact.pubkey != pubkey_hex);

    if contacts.len() == original_len {
        println!("Contact not found.");
        return Ok(());
    }

    save_contacts(&contacts)?;

    let mnemonic_phrase = load_mnemonic()?;
    let mnemonic = Mnemonic::parse(&mnemonic_phrase)?;
    let identity = UnifiedIdentity::from_mnemonic(mnemonic)?;
    let event = create_contact_list_event(&identity, &contacts)?;

    println!("{} Unfollowed {}", "✓".green(), pubkey_hex);

    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        println!();
        println!("{}", "Note: No relays configured. Use 'wallet relays add <url>' to add relays.".yellow());
        println!("{}: {}", "Event ID".bold(), event.id);
        return Ok(());
    }

    println!();
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

    Ok(())
}

pub fn post(content: String) -> Result<()> {
    use bip39::Mnemonic;
    use crate::core::nostr::create_note_event;
    use crate::core::client::NostrClient;
    use crate::storage::config::WalletConfig;

    println!("{}", "Publishing note...".cyan());

    // Load identity
    let mnemonic_phrase = load_mnemonic()?;
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

    // Parse recipient npub
    let recipient_pubkey = match nostr::decode(&recipient) {
        Ok(nostr::Nip19Entity::Pubkey(pk)) => pk,
        Ok(nostr::Nip19Entity::Profile(p)) => p.pubkey,
        _ => anyhow::bail!("Invalid recipient. Expected npub or nprofile."),
    };

    // Load identity
    let mnemonic_str = load_mnemonic()?;
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

    // Load identity
    let mnemonic_str = load_mnemonic()?;
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

    // Load identity
    let mnemonic_str = load_mnemonic()?;
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
        .unwrap_or(DateTime::UNIX_EPOCH);
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
