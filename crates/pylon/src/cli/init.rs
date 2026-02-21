//! pylon init - Initialize provider identity

use clap::Args;
use runtime::UnifiedIdentity;

/// Arguments for the init command
#[derive(Args)]
pub struct InitArgs {
    /// Import existing seed phrase instead of generating new
    #[arg(long)]
    pub import: bool,

    /// Force overwrite existing identity
    #[arg(long, short)]
    pub force: bool,
}

/// Run the init command
pub async fn run(args: InitArgs) -> anyhow::Result<()> {
    use crate::config::PylonConfig;

    // Check for existing identity
    let config = PylonConfig::load()?;
    let data_dir = config.data_path()?;
    let identity_file = data_dir.join("identity.mnemonic");

    if identity_file.exists() && !args.force {
        println!("Identity already exists at {:?}", identity_file);
        println!("Use --force to overwrite");
        return Ok(());
    }

    // Create data directory
    std::fs::create_dir_all(&data_dir)?;

    let identity = if args.import {
        // Import existing seed phrase
        println!("Enter your BIP-39 seed phrase (12 or 24 words):");
        let mut seed_phrase = String::new();
        std::io::stdin().read_line(&mut seed_phrase)?;
        let seed_phrase = seed_phrase.trim();

        UnifiedIdentity::from_mnemonic(seed_phrase, "")
            .map_err(|e| anyhow::anyhow!("Invalid seed phrase: {}", e))?
    } else {
        // Generate new identity
        println!("Generating new identity...");
        UnifiedIdentity::generate()
            .map_err(|e| anyhow::anyhow!("Failed to generate identity: {}", e))?
    };

    // Display the mnemonic for backup
    if !args.import {
        println!("\n⚠️  IMPORTANT: Write down these words and store them securely!");
        println!("This is your seed phrase - it controls your identity and funds.\n");
        println!("  {}\n", identity.mnemonic());
    }

    // Display identity info
    let npub = identity
        .npub()
        .map_err(|e| anyhow::anyhow!("Failed to get npub: {}", e))?;
    println!("Nostr Public Key (npub): {}", npub);
    println!("Nostr Public Key (hex):  {}", identity.public_key_hex());

    // Save identity (plaintext for now - TODO: add encryption)
    // WARNING: In production, this should be encrypted!
    println!("\n⚠️  Saving mnemonic to {:?}", identity_file);
    println!("   This file contains your private key. Keep it secure!");
    std::fs::write(&identity_file, identity.mnemonic())?;

    // Restrict file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&identity_file, std::fs::Permissions::from_mode(0o600))?;
    }

    // Save default config
    config.save()?;

    println!("\n✅ Identity initialized successfully!");
    println!("   Config: {:?}", PylonConfig::config_path()?);
    println!("   Identity: {:?}", identity_file);
    println!("\nRun 'pylon start' to begin earning.");

    Ok(())
}
