//! Identity discovery - Nostr keys and wallet status.

use crate::manifest::IdentityManifest;

/// Discover identity from pylon or environment.
pub async fn discover_identity() -> anyhow::Result<IdentityManifest> {
    // Check for pylon identity at ~/.pylon/
    let pylon_dir = dirs::home_dir().map(|h| h.join(".pylon"));

    if let Some(pylon_dir) = pylon_dir {
        if pylon_dir.exists() {
            let keys_file = pylon_dir.join("keys.json");
            if keys_file.exists() {
                // Parse keys to get npub
                if let Ok(content) = std::fs::read_to_string(&keys_file) {
                    if let Ok(keys) = serde_json::from_str::<serde_json::Value>(&content) {
                        let npub = keys.get("npub").and_then(|v| v.as_str()).map(String::from);

                        return Ok(IdentityManifest {
                            initialized: true,
                            npub,
                            wallet_balance_sats: None, // Would need spark to check
                            network: None,
                        });
                    }
                }
            }
        }
    }

    // Check for NOSTR_NSEC environment variable
    if std::env::var("NOSTR_NSEC").is_ok() {
        return Ok(IdentityManifest {
            initialized: true,
            npub: None, // Would need to derive from nsec
            wallet_balance_sats: None,
            network: None,
        });
    }

    Ok(IdentityManifest::unknown())
}
