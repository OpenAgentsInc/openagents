//! FROSTR threshold signing CLI commands
//!
//! This module provides CLI commands for managing FROSTR threshold key shares
//! and performing threshold signature operations.

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use colored::Colorize;
use frostr::credential::{GroupCredential, ShareCredential};
use frostr::keygen::{FrostShare, generate_key_shares};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const KEYRING_SERVICE: &str = "openagents-wallet";
const FROSTR_SHARE_KEY: &str = "frostr-share";
const FROSTR_PEERS_KEY: &str = "frostr-peers";

/// A stored peer for threshold signing coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPeer {
    /// Peer's Nostr public key (32 bytes, hex encoded)
    pub pubkey: String,
    /// Preferred relays for this peer
    pub relays: Vec<String>,
    /// Optional human-readable name
    pub name: Option<String>,
    /// When this peer was added (Unix timestamp)
    pub added_at: u64,
}

/// Collection of stored peers
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredPeers {
    pub peers: Vec<StoredPeer>,
}

impl StoredPeers {
    /// Load peers from keychain
    pub fn load() -> Result<Self> {
        let entry = Entry::new(KEYRING_SERVICE, FROSTR_PEERS_KEY)?;
        match entry.get_password() {
            Ok(encoded) => {
                let bytes = BASE64.decode(&encoded).context("Failed to decode peers")?;
                postcard::from_bytes(&bytes).context("Failed to deserialize peers")
            }
            Err(_) => Ok(Self::default()),
        }
    }

    /// Save peers to keychain
    pub fn save(&self) -> Result<()> {
        let bytes = postcard::to_stdvec(self).context("Failed to serialize peers")?;
        let encoded = BASE64.encode(&bytes);
        let entry = Entry::new(KEYRING_SERVICE, FROSTR_PEERS_KEY)?;
        entry
            .set_password(&encoded)
            .context("Failed to save peers to keychain")?;
        Ok(())
    }

    /// Add a peer (updates if pubkey exists)
    pub fn add(&mut self, peer: StoredPeer) {
        // Remove existing peer with same pubkey
        self.peers.retain(|p| p.pubkey != peer.pubkey);
        self.peers.push(peer);
    }

    /// Remove a peer by pubkey
    pub fn remove(&mut self, pubkey: &str) -> bool {
        let len_before = self.peers.len();
        self.peers.retain(|p| p.pubkey != pubkey);
        self.peers.len() < len_before
    }

    /// Find a peer by pubkey
    pub fn find(&self, pubkey: &str) -> Option<&StoredPeer> {
        self.peers.iter().find(|p| p.pubkey == pubkey)
    }
}

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
        let key_package_data =
            postcard::to_stdvec(&share.key_package).context("Failed to serialize KeyPackage")?;

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
        let bytes = BASE64.decode(encoded).context("Failed to decode base64")?;
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
        println!(
            "\n{} Group credential (share with others):",
            "EXPORT".bright_cyan()
        );
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

    println!("{} Parsed share credential:", "[OK]".bright_green());
    println!("  Index: {}", share_cred.index);
    println!(
        "  Group PK: {}...",
        &hex::encode(&share_cred.group_pk)[..16]
    );

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

        println!("{} Full share imported successfully", "[OK]".bright_green());
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
            println!(
                "\n{} Group credential (safe to share):",
                "PUBLIC".bright_cyan()
            );
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
                println!(
                    "{} Unknown credential format in keychain",
                    "ERR".bright_red()
                );
            }
        }
    }

    Ok(())
}

/// Sign an event hash using threshold shares
pub async fn sign(event_hash_hex: String) -> Result<()> {
    println!("{} Initiating threshold signing...", "SIGN".bright_blue());

    // Decode event hash
    let event_hash_bytes =
        hex::decode(&event_hash_hex).context("Invalid event hash (must be 64-character hex)")?;

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

    // Show peer status
    let peers = StoredPeers::load()?;
    if peers.peers.is_empty() {
        println!(
            "\n{} Threshold Peers: none configured",
            "PEERS".bright_yellow()
        );
        println!("  Run 'openagents wallet frostr peers add <npub>' to add peers");
    } else {
        println!(
            "\n{} Threshold Peers: {} configured",
            "PEERS".bright_cyan(),
            peers.peers.len()
        );
        for peer in &peers.peers {
            let name = peer.name.as_deref().unwrap_or("(unnamed)");
            let relay_count = if peer.relays.is_empty() {
                "default relays".to_string()
            } else {
                format!("{} relay(s)", peer.relays.len())
            };
            println!("  - {} ({}...): {}", name, &peer.pubkey[..8], relay_count);
        }
    }

    println!(
        "\n{} Relay Connections: (not configured)",
        "TODO".bright_yellow()
    );

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
        let stored =
            StoredFrostShare::from_frost_share(&shares[0]).expect("from_frost_share failed");

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
        assert_eq!(
            decoded.public_key_package_data,
            stored.public_key_package_data
        );
    }

    #[test]
    fn test_stored_frost_share_to_group_credential() {
        let shares = generate_key_shares(2, 3).expect("keygen failed");
        let stored =
            StoredFrostShare::from_frost_share(&shares[0]).expect("from_frost_share failed");

        let group_cred = stored
            .to_group_credential()
            .expect("to_group_credential failed");

        assert_eq!(group_cred.threshold, 2);
        assert_eq!(group_cred.total, 3);

        // Verify bech32 encoding works
        let bech32 = group_cred.to_bech32().expect("to_bech32 failed");
        assert!(bech32.starts_with("bfgroup1"));
    }

    #[test]
    fn test_key_package_deserialization() {
        let shares = generate_key_shares(2, 3).expect("keygen failed");
        let stored =
            StoredFrostShare::from_frost_share(&shares[0]).expect("from_frost_share failed");

        // Verify KeyPackage can be deserialized
        let key_package: frostr::frost::keys::KeyPackage =
            postcard::from_bytes(&stored.key_package_data)
                .expect("KeyPackage deserialization failed");

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

    #[test]
    fn test_stored_peers_add_and_find() {
        let mut peers = StoredPeers::default();
        assert!(peers.peers.is_empty());

        // Add a peer
        let peer1 = StoredPeer {
            pubkey: "a".repeat(64),
            relays: vec!["wss://relay.example.com".to_string()],
            name: Some("Alice".to_string()),
            added_at: 1234567890,
        };
        peers.add(peer1.clone());

        assert_eq!(peers.peers.len(), 1);
        assert!(peers.find(&"a".repeat(64)).is_some());
        assert_eq!(
            peers.find(&"a".repeat(64)).unwrap().name,
            Some("Alice".to_string())
        );

        // Add another peer
        let peer2 = StoredPeer {
            pubkey: "b".repeat(64),
            relays: vec![],
            name: None,
            added_at: 1234567891,
        };
        peers.add(peer2);
        assert_eq!(peers.peers.len(), 2);

        // Update existing peer (same pubkey)
        let peer1_updated = StoredPeer {
            pubkey: "a".repeat(64),
            relays: vec!["wss://new-relay.example.com".to_string()],
            name: Some("Alice Updated".to_string()),
            added_at: 1234567892,
        };
        peers.add(peer1_updated);

        // Should still have 2 peers (updated, not added)
        assert_eq!(peers.peers.len(), 2);
        assert_eq!(
            peers.find(&"a".repeat(64)).unwrap().name,
            Some("Alice Updated".to_string())
        );
    }

    #[test]
    fn test_stored_peers_remove() {
        let mut peers = StoredPeers::default();

        // Add peers
        peers.add(StoredPeer {
            pubkey: "a".repeat(64),
            relays: vec![],
            name: Some("Alice".to_string()),
            added_at: 1,
        });
        peers.add(StoredPeer {
            pubkey: "b".repeat(64),
            relays: vec![],
            name: Some("Bob".to_string()),
            added_at: 2,
        });
        assert_eq!(peers.peers.len(), 2);

        // Remove a peer
        let removed = peers.remove(&"a".repeat(64));
        assert!(removed);
        assert_eq!(peers.peers.len(), 1);
        assert!(peers.find(&"a".repeat(64)).is_none());
        assert!(peers.find(&"b".repeat(64)).is_some());

        // Try to remove non-existent peer
        let removed_again = peers.remove(&"a".repeat(64));
        assert!(!removed_again);
        assert_eq!(peers.peers.len(), 1);
    }

    #[test]
    fn test_stored_peers_serialization() {
        let mut peers = StoredPeers::default();
        peers.add(StoredPeer {
            pubkey: "a".repeat(64),
            relays: vec![
                "wss://relay1.com".to_string(),
                "wss://relay2.com".to_string(),
            ],
            name: Some("Test Peer".to_string()),
            added_at: 1234567890,
        });

        // Serialize
        let bytes = postcard::to_stdvec(&peers).expect("serialization failed");

        // Deserialize
        let decoded: StoredPeers = postcard::from_bytes(&bytes).expect("deserialization failed");

        assert_eq!(decoded.peers.len(), 1);
        assert_eq!(decoded.peers[0].pubkey, "a".repeat(64));
        assert_eq!(decoded.peers[0].relays.len(), 2);
        assert_eq!(decoded.peers[0].name, Some("Test Peer".to_string()));
    }

    #[test]
    fn test_parse_nostr_pubkey_hex() {
        // Valid 64-char hex
        let hex_pubkey = "a".repeat(64);
        let result = parse_nostr_pubkey(&hex_pubkey).expect("should parse hex");
        assert_eq!(result, hex_pubkey);

        // Invalid hex (wrong length)
        let short_hex = "a".repeat(32);
        assert!(parse_nostr_pubkey(&short_hex).is_err());

        // Invalid chars
        let invalid = "z".repeat(64);
        assert!(parse_nostr_pubkey(&invalid).is_err());
    }

    #[test]
    fn test_parse_nostr_pubkey_npub() {
        // Valid npub (from NIP-19 test vectors)
        let npub = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";
        let result = parse_nostr_pubkey(npub).expect("should parse npub");
        assert_eq!(
            result,
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
        );
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
                        println!(
                            "{} Legacy metadata (no group key stored):",
                            "WARN".bright_yellow()
                        );
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

/// Parse a Nostr public key from npub bech32 or hex format
fn parse_nostr_pubkey(input: &str) -> Result<String> {
    if input.starts_with("npub1") {
        // Decode bech32 npub using nostr crate's NIP-19 decoder
        let decoded = nostr::decode(input).context("Invalid npub format")?;
        match decoded {
            nostr::Nip19Entity::Pubkey(pk) => Ok(hex::encode(pk)),
            _ => anyhow::bail!("Expected npub, got different bech32 type"),
        }
    } else if input.len() == 64 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        // Already hex format
        Ok(input.to_string())
    } else {
        anyhow::bail!("Invalid public key format. Use npub1... or 64-char hex")
    }
}

/// Add a threshold signing peer
pub async fn peers_add(npub: String, relays: Vec<String>, name: Option<String>) -> Result<()> {
    println!("{} Adding FROSTR peer...", "PEERS".bright_blue());

    // Parse the public key
    let pubkey = parse_nostr_pubkey(&npub)?;
    println!(
        "{} Parsed pubkey: {}...",
        "[OK]".bright_green(),
        &pubkey[..16]
    );

    // Load existing peers
    let mut peers = StoredPeers::load()?;

    // Check if updating existing peer
    let is_update = peers.find(&pubkey).is_some();

    // Create the peer entry
    let peer = StoredPeer {
        pubkey: pubkey.clone(),
        relays: relays.clone(),
        name: name.clone(),
        added_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Add and save
    peers.add(peer);
    peers.save()?;

    if is_update {
        println!("{} Updated peer:", "[OK]".bright_green());
    } else {
        println!("{} Added peer:", "[OK]".bright_green());
    }

    if let Some(n) = &name {
        println!("  Name:   {}", n);
    }
    println!("  Pubkey: {}...", &pubkey[..16]);

    if relays.is_empty() {
        println!("  Relays: (default relays will be used)");
    } else {
        println!("  Relays:");
        for relay in &relays {
            println!("    - {}", relay);
        }
    }

    println!(
        "\n{} Total peers: {}",
        "INFO".bright_blue(),
        peers.peers.len()
    );

    Ok(())
}

/// List threshold signing peers
pub async fn peers_list() -> Result<()> {
    println!("{} FROSTR Threshold Peers\n", "PEERS".bright_blue());

    let peers = StoredPeers::load()?;

    if peers.peers.is_empty() {
        println!("{} No peers configured", "WARN".bright_yellow());
        println!("  Use 'openagents wallet frostr peers add <npub>' to add peers");
        return Ok(());
    }

    for (i, peer) in peers.peers.iter().enumerate() {
        let display_name = peer.name.as_deref().unwrap_or("(unnamed)");
        println!(
            "{} Peer {} - {}",
            "[PEER]".bright_cyan(),
            i + 1,
            display_name
        );
        println!("  Pubkey: {}", peer.pubkey);

        if peer.relays.is_empty() {
            println!("  Relays: (default)");
        } else {
            println!("  Relays:");
            for relay in &peer.relays {
                println!("    - {}", relay);
            }
        }

        // Format added time
        let added = chrono::DateTime::from_timestamp(peer.added_at as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!("  Added:  {}", added);
        println!();
    }

    println!(
        "{} Total: {} peer(s)",
        "INFO".bright_blue(),
        peers.peers.len()
    );

    Ok(())
}

/// Remove a threshold signing peer
pub async fn peers_remove(npub: String) -> Result<()> {
    println!("{} Removing FROSTR peer...", "PEERS".bright_blue());

    // Parse the public key
    let pubkey = parse_nostr_pubkey(&npub)?;

    // Load existing peers
    let mut peers = StoredPeers::load()?;

    // Check if peer exists
    if let Some(peer) = peers.find(&pubkey) {
        let name = peer.name.clone();
        if peers.remove(&pubkey) {
            peers.save()?;
            println!(
                "{} Removed peer: {}",
                "[OK]".bright_green(),
                name.as_deref().unwrap_or(&format!("{}...", &pubkey[..16]))
            );
            println!(
                "{} Remaining peers: {}",
                "INFO".bright_blue(),
                peers.peers.len()
            );
        }
    } else {
        println!(
            "{} Peer not found: {}...",
            "WARN".bright_yellow(),
            &pubkey[..16]
        );
    }

    Ok(())
}
