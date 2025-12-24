//! FROSTR threshold signing CLI commands
//!
//! This module provides CLI commands for managing FROSTR threshold key shares
//! and performing threshold signature operations.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use colored::Colorize;
use frostr::credential::{GroupCredential, ShareCredential};
use frostr::keygen::{generate_key_shares, FrostShare};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const KEYRING_SERVICE: &str = "openagents-wallet";
const FROSTR_SHARE_KEY: &str = "frostr-share";

/// Stored representation of a FROST share for keychain persistence
///
/// This struct contains all data needed to reconstruct a FrostShare for signing.
/// The key_package_data and public_key_package_data are serialized using postcard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFrostShare {
    /// Minimum signers required (threshold k)
    pub threshold: u16,
    /// Total number of shares (n)
    pub total: u16,
    /// Participant ID (1-based index)
    pub participant_id: u8,
    /// Group public key (hex encoded, 64 chars)
    pub group_pubkey: String,
    /// Serialized KeyPackage bytes (postcard encoded)
    pub key_package_data: Vec<u8>,
    /// Serialized PublicKeyPackage bytes (postcard encoded)
    pub public_key_package_data: Vec<u8>,
}

impl StoredFrostShare {
    /// Create from a FrostShare
    pub fn from_frost_share(share: &FrostShare) -> Result<Self> {
        // Serialize KeyPackage
        let key_package_data = postcard::to_stdvec(&share.key_package)
            .context("Failed to serialize KeyPackage")?;

        // Serialize PublicKeyPackage
        let public_key_package_data = postcard::to_stdvec(&share.public_key_package)
            .context("Failed to serialize PublicKeyPackage")?;

        // Get group public key
        let group_verifying_key = share.public_key_package.verifying_key();
        let group_pubkey_bytes = group_verifying_key
            .serialize()
            .map_err(|e| anyhow::anyhow!("Failed to serialize group key: {:?}", e))?;
        let group_pubkey = hex::encode(group_pubkey_bytes);

        Ok(Self {
            threshold: share.threshold,
            total: share.total,
            participant_id: share.participant_id,
            group_pubkey,
            key_package_data,
            public_key_package_data,
        })
    }

    /// Serialize to base64 for keychain storage
    pub fn to_base64(&self) -> Result<String> {
        let bytes = postcard::to_stdvec(self).context("Failed to serialize StoredFrostShare")?;
        Ok(BASE64.encode(&bytes))
    }

    /// Deserialize from base64
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let bytes = BASE64
            .decode(encoded)
            .context("Failed to decode base64")?;
        postcard::from_bytes(&bytes).context("Failed to deserialize StoredFrostShare")
    }

    /// Convert to a GroupCredential for export
    ///
    /// Note: The stored group_pubkey is 33 bytes (compressed SEC1 format).
    /// GroupCredential expects 32 bytes (x-only format), so we strip the prefix.
    pub fn to_group_credential(&self) -> Result<GroupCredential> {
        let mut group_pk = [0u8; 32];
        let pk_bytes = hex::decode(&self.group_pubkey).context("Invalid group pubkey hex")?;

        // Handle both formats:
        // - 33 bytes (compressed SEC1: prefix + x-coordinate)
        // - 32 bytes (x-only)
        match pk_bytes.len() {
            33 => {
                // Compressed SEC1 format: skip the prefix byte (0x02 or 0x03)
                group_pk.copy_from_slice(&pk_bytes[1..33]);
            }
            32 => {
                // Already x-only format
                group_pk.copy_from_slice(&pk_bytes);
            }
            _ => {
                anyhow::bail!(
                    "Group pubkey must be 32 or 33 bytes, got {}",
                    pk_bytes.len()
                );
            }
        }

        Ok(GroupCredential {
            threshold: self.threshold as u32,
            total: self.total as u32,
            group_pk,
        })
    }
}

/// Generate threshold key shares (k-of-n)
pub async fn keygen(threshold: u16, total: u16) -> Result<()> {
    println!(
        "{} Generating {}-of-{} threshold shares...",
        "FROSTR".bright_blue(),
        threshold,
        total
    );

    // Generate shares using FROSTR keygen (convert u16 to u32)
    let shares = generate_key_shares(threshold as u32, total as u32)
        .context("Failed to generate key shares")?;

    println!(
        "{} Generated {} shares successfully",
        "[OK]".bright_green(),
        shares.len()
    );

    // Get group public key for display
    let group_verifying_key = shares[0].public_key_package.verifying_key();
    let group_pubkey_bytes = group_verifying_key
        .serialize()
        .map_err(|e| anyhow::anyhow!("Failed to serialize group key: {:?}", e))?;
    let group_pubkey = hex::encode(&group_pubkey_bytes);

    println!("\n{} Group Public Key:", "GROUP".bright_cyan());
    println!("  {}", group_pubkey);

    // Show each share
    for (i, share) in shares.iter().enumerate() {
        println!(
            "\n{} Share {} (participant {}):",
            "SHARE".bright_yellow(),
            i + 1,
            share.participant_id
        );
        println!("  Threshold: {}/{}", share.threshold, share.total);
    }

    // Prompt to save local share (share 1) to keychain
    print!(
        "\n{} Save share 1 to secure keychain? [Y/n] ",
        "SAVE".bright_blue()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().is_empty() || input.trim().eq_ignore_ascii_case("y") {
        // Serialize and store the full share
        let stored_share = StoredFrostShare::from_frost_share(&shares[0])?;
        let encoded = stored_share.to_base64()?;

        // Save to keychain
        let entry = Entry::new(KEYRING_SERVICE, FROSTR_SHARE_KEY)?;
        entry
            .set_password(&encoded)
            .context("Failed to save share to keychain")?;

        println!(
            "{} Share 1 saved to keychain ({} bytes)",
            "[OK]".bright_green(),
            encoded.len()
        );

        // Show bech32 group credential for sharing
        let group_cred = stored_share.to_group_credential()?;
        let group_bech32 = group_cred.to_bech32()?;
        println!("\n{} Group credential (share with others):", "EXPORT".bright_cyan());
        println!("  {}", group_bech32);
    } else {
        println!("{} Skipped keychain storage", "SKIP".bright_yellow());
    }

    println!(
        "\n{} Keep other shares secure and distribute to threshold peers",
        "WARN".bright_yellow()
    );
    println!(
        "{} Any {}-of-{} shares can sign, but {} cannot",
        "INFO".bright_blue(),
        threshold,
        total,
        threshold - 1
    );

    Ok(())
}

/// Import a FROSTR share credential
pub async fn import_share(credential: String) -> Result<()> {
    println!("{} Importing FROSTR share...", "IMPORT".bright_blue());

    // Try to parse as bech32 share credential
    let share_cred = ShareCredential::from_bech32(&credential)
        .context("Invalid share credential format. Expected bfshare1...")?;

    println!(
        "{} Parsed share credential:",
        "[OK]".bright_green()
    );
    println!("  Index: {}", share_cred.index);
    println!("  Group PK: {}...", &hex::encode(&share_cred.group_pk)[..16]);

    // Note: ShareCredential contains simplified data (index, secret, group_pk)
    // It does NOT contain the full KeyPackage needed for FROST signing.
    // Full import would require the KeyPackage to be exported separately.
    println!(
        "\n{} ShareCredential import is for backup/recovery only",
        "INFO".bright_blue()
    );
    println!("  For full signing capability, use 'wallet frostr keygen' to generate shares");
    println!("  or import a full share backup (base64 encoded StoredFrostShare)");

    // Check if input might be a base64-encoded StoredFrostShare
    if let Ok(stored) = StoredFrostShare::from_base64(&credential) {
        println!(
            "\n{} Detected full share backup, importing...",
            "[OK]".bright_green()
        );

        let entry = Entry::new(KEYRING_SERVICE, FROSTR_SHARE_KEY)?;
        entry
            .set_password(&credential)
            .context("Failed to save share to keychain")?;

        println!(
            "{} Full share imported successfully",
            "[OK]".bright_green()
        );
        println!("  Threshold: {}/{}", stored.threshold, stored.total);
        println!("  Participant: {}", stored.participant_id);
        println!("  Group PK: {}...", &stored.group_pubkey[..16]);

        return Ok(());
    }

    Ok(())
}

/// Export the local FROSTR share
pub async fn export_share() -> Result<()> {
    println!("{} Exporting FROSTR share...", "EXPORT".bright_blue());

    // Retrieve from keychain
    let entry = Entry::new(KEYRING_SERVICE, FROSTR_SHARE_KEY)?;
    let encoded = entry
        .get_password()
        .context("No share found in keychain. Run 'wallet frostr keygen' first.")?;

    // Try to parse as StoredFrostShare
    match StoredFrostShare::from_base64(&encoded) {
        Ok(stored) => {
            println!("{} Share found in keychain:\n", "[OK]".bright_green());
            println!("  Threshold:   {}-of-{}", stored.threshold, stored.total);
            println!("  Participant: {}", stored.participant_id);
            println!("  Group PK:    {}", stored.group_pubkey);

            // Show group credential (safe to share)
            let group_cred = stored.to_group_credential()?;
            let group_bech32 = group_cred.to_bech32()?;
            println!("\n{} Group credential (safe to share):", "PUBLIC".bright_cyan());
            println!("  {}", group_bech32);

            // Show full backup (SENSITIVE!)
            println!(
                "\n{} Full share backup (KEEP SECRET!):",
                "SECRET".bright_red()
            );
            println!("  {}", encoded);
        }
        Err(_) => {
            // Legacy format - just a marker
            if encoded.starts_with("frostr-share:") {
                println!(
                    "{} Legacy share marker found (metadata only):",
                    "WARN".bright_yellow()
                );
                println!("  {}", encoded);
                println!(
                    "\n{} Re-run 'wallet frostr keygen' to store full share",
                    "INFO".bright_blue()
                );
            } else {
                println!("{} Unknown credential format in keychain", "ERR".bright_red());
            }
        }
    }

    Ok(())
}

/// Sign an event hash using threshold shares
pub async fn sign(event_hash_hex: String) -> Result<()> {
    println!("{} Initiating threshold signing...", "SIGN".bright_blue());

    // Decode event hash
    let event_hash_bytes = hex::decode(&event_hash_hex)
        .context("Invalid event hash (must be 64-character hex)")?;

    if event_hash_bytes.len() != 32 {
        anyhow::bail!("Event hash must be exactly 32 bytes (64 hex characters)");
    }

    let mut event_hash = [0u8; 32];
    event_hash.copy_from_slice(&event_hash_bytes);

    println!("{} Event hash: {}", "HASH".bright_blue(), event_hash_hex);

    // Load local share from keychain
    let entry = Entry::new(KEYRING_SERVICE, FROSTR_SHARE_KEY)?;
    let encoded = entry
        .get_password()
        .context("No share found in keychain. Run 'wallet frostr keygen' first.")?;

    let stored = StoredFrostShare::from_base64(&encoded)
        .context("Invalid share in keychain. Re-run 'wallet frostr keygen' to update.")?;

    println!(
        "{} Loaded share: {}-of-{} (participant {})",
        "[OK]".bright_green(),
        stored.threshold,
        stored.total,
        stored.participant_id
    );

    // Deserialize KeyPackage for signing
    let _key_package: frostr::frost::keys::KeyPackage =
        postcard::from_bytes(&stored.key_package_data)
            .context("Failed to deserialize KeyPackage")?;

    let _public_key_package: frostr::frost::keys::PublicKeyPackage =
        postcard::from_bytes(&stored.public_key_package_data)
            .context("Failed to deserialize PublicKeyPackage")?;

    println!("{} KeyPackage loaded successfully", "[OK]".bright_green());

    // TODO: Integrate with BifrostNode for actual threshold signing
    // This requires:
    // 1. Nostr relay configuration
    // 2. Peer public keys for the signing quorum
    // 3. BifrostNode to coordinate the signing protocol
    println!("\n{} Threshold signing requires:", "INFO".bright_yellow());
    println!("  1. Nostr relay configuration (for peer communication)");
    println!("  2. Peer registration (pubkeys of other share holders)");
    println!("  3. BifrostNode integration (for protocol coordination)");

    println!(
        "\n{} Use 'openagents autopilot' for automated threshold signing",
        "TIP".bright_blue()
    );

    Ok(())
}

/// Show FROSTR node status and peer connectivity
pub async fn status() -> Result<()> {
    println!("{} FROSTR Node Status\n", "STATUS".bright_blue());

    // Check if local share exists
    let entry = Entry::new(KEYRING_SERVICE, FROSTR_SHARE_KEY)?;
    match entry.get_password() {
        Ok(encoded) => {
            match StoredFrostShare::from_base64(&encoded) {
                Ok(stored) => {
                    println!("{} Local Share:", "[OK]".bright_green());
                    println!("  Threshold:   {}-of-{}", stored.threshold, stored.total);
                    println!("  Participant: {}", stored.participant_id);
                    println!("  Group PK:    {}...", &stored.group_pubkey[..16]);
                    println!("  Key data:    {} bytes", stored.key_package_data.len());
                }
                Err(_) => {
                    // Legacy format
                    if encoded.starts_with("frostr-share:") {
                        let parts: Vec<&str> = encoded.split(':').collect();
                        if parts.len() == 3 {
                            println!("{} Legacy Share (metadata only):", "WARN".bright_yellow());
                            println!("  Threshold: {}-of-{}", parts[1], parts[2]);
                            println!(
                                "\n{} Re-run 'wallet frostr keygen' for full share",
                                "TIP".bright_blue()
                            );
                        }
                    } else {
                        println!("{} Unknown credential format", "ERR".bright_red());
                    }
                }
            }
        }
        Err(_) => {
            println!("{} No local share found", "WARN".bright_yellow());
            println!("  Run 'openagents wallet frostr keygen' to generate shares");
        }
    }

    println!("\n{} Relay Connections: (not configured)", "TODO".bright_yellow());
    println!("{} Threshold Peers: (not configured)", "TODO".bright_yellow());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use frostr::keygen::generate_key_shares;

    #[test]
    fn test_stored_frost_share_serialization_round_trip() {
        // Generate a 2-of-3 threshold share
        let shares = generate_key_shares(2, 3).expect("keygen failed");

        // Convert to StoredFrostShare
        let stored = StoredFrostShare::from_frost_share(&shares[0]).expect("from_frost_share failed");

        assert_eq!(stored.threshold, 2);
        assert_eq!(stored.total, 3);
        assert_eq!(stored.participant_id, 1);
        assert!(!stored.group_pubkey.is_empty());
        assert!(!stored.key_package_data.is_empty());
        assert!(!stored.public_key_package_data.is_empty());

        // Serialize to base64
        let encoded = stored.to_base64().expect("to_base64 failed");

        // Deserialize back
        let decoded = StoredFrostShare::from_base64(&encoded).expect("from_base64 failed");

        // Verify round-trip
        assert_eq!(decoded.threshold, stored.threshold);
        assert_eq!(decoded.total, stored.total);
        assert_eq!(decoded.participant_id, stored.participant_id);
        assert_eq!(decoded.group_pubkey, stored.group_pubkey);
        assert_eq!(decoded.key_package_data, stored.key_package_data);
        assert_eq!(decoded.public_key_package_data, stored.public_key_package_data);
    }

    #[test]
    fn test_stored_frost_share_to_group_credential() {
        let shares = generate_key_shares(2, 3).expect("keygen failed");
        let stored = StoredFrostShare::from_frost_share(&shares[0]).expect("from_frost_share failed");

        let group_cred = stored.to_group_credential().expect("to_group_credential failed");

        assert_eq!(group_cred.threshold, 2);
        assert_eq!(group_cred.total, 3);

        // Verify bech32 encoding works
        let bech32 = group_cred.to_bech32().expect("to_bech32 failed");
        assert!(bech32.starts_with("bfgroup1"));
    }

    #[test]
    fn test_key_package_deserialization() {
        let shares = generate_key_shares(2, 3).expect("keygen failed");
        let stored = StoredFrostShare::from_frost_share(&shares[0]).expect("from_frost_share failed");

        // Verify KeyPackage can be deserialized
        let key_package: frostr::frost::keys::KeyPackage =
            postcard::from_bytes(&stored.key_package_data).expect("KeyPackage deserialization failed");

        // Verify PublicKeyPackage can be deserialized
        let public_key_package: frostr::frost::keys::PublicKeyPackage =
            postcard::from_bytes(&stored.public_key_package_data)
                .expect("PublicKeyPackage deserialization failed");

        // Verify the group public key matches
        let group_pk_bytes = public_key_package
            .verifying_key()
            .serialize()
            .expect("serialize failed");
        let group_pk_hex = hex::encode(group_pk_bytes);
        assert_eq!(group_pk_hex, stored.group_pubkey);

        // Verify key_package has correct identifier
        let _identifier = key_package.identifier();
    }
}

/// Show available FROSTR group credentials (if any)
pub async fn list_groups() -> Result<()> {
    println!("{} FROSTR Group Credentials\n", "GROUPS".bright_blue());

    // List group credential info from local share
    let entry = Entry::new(KEYRING_SERVICE, FROSTR_SHARE_KEY)?;
    match entry.get_password() {
        Ok(encoded) => match StoredFrostShare::from_base64(&encoded) {
            Ok(stored) => {
                println!("{} Active group:", "[OK]".bright_green());
                println!("  Threshold:   {}-of-{}", stored.threshold, stored.total);
                println!("  Group PK:    {}", stored.group_pubkey);

                // Show bech32 group credential
                let group_cred = stored.to_group_credential()?;
                let group_bech32 = group_cred.to_bech32()?;
                println!("\n{} Group credential:", "EXPORT".bright_cyan());
                println!("  {}", group_bech32);
            }
            Err(_) => {
                if encoded.starts_with("frostr-share:") {
                    let parts: Vec<&str> = encoded.split(':').collect();
                    if parts.len() == 3 {
                        println!("{} Legacy metadata (no group key stored):", "WARN".bright_yellow());
                        println!("  Threshold: {}-of-{}", parts[1], parts[2]);
                        println!(
                            "\n{} Re-run 'wallet frostr keygen' to get full credentials",
                            "TIP".bright_blue()
                        );
                    }
                } else {
                    println!("{} Unknown credential format", "ERR".bright_red());
                }
            }
        },
        Err(_) => {
            println!("{} No groups found", "WARN".bright_yellow());
            println!("  Run 'openagents wallet frostr keygen' to create a group");
        }
    }

    Ok(())
}
