//! NIP-05 identifier verification
//!
//! Implements NIP-05 verification to map Nostr pubkeys to DNS-based identifiers

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// NIP-05 verification result
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Nip05Verification {
    pub identifier: String,
    pub verified: bool,
    pub checked_at: SystemTime,
}

/// Response from .well-known/nostr.json endpoint
#[derive(Debug, Deserialize)]
struct WellKnownResponse {
    names: HashMap<String, String>,
    #[allow(dead_code)]
    relays: Option<HashMap<String, Vec<String>>>,
}

/// Verify a NIP-05 identifier against a pubkey
///
/// Format: `<local-part>@<domain>`
/// Fetches: `https://<domain>/.well-known/nostr.json?name=<local-part>`
/// Validates: returned pubkey matches the provided pubkey
pub fn verify_nip05(identifier: &str, pubkey: &str) -> Result<bool> {
    // Parse identifier
    let parts: Vec<&str> = identifier.split('@').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid NIP-05 identifier format. Expected: name@domain.com");
    }

    let local_part = parts[0];
    let domain = parts[1];

    // Validate local part (must be a-z0-9-_.)
    if !local_part
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        anyhow::bail!("Invalid characters in local part. Only a-z0-9-_. allowed");
    }

    // Build URL
    let url = format!(
        "https://{}/.well-known/nostr.json?name={}",
        domain, local_part
    );

    // Make HTTP request (blocking)
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none()) // NIP-05: MUST NOT follow redirects
        .timeout(Duration::from_secs(10))
        .build()?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .with_context(|| format!("Failed to fetch NIP-05 from {}", url))?;

    // Check status
    if !response.status().is_success() {
        anyhow::bail!("HTTP {} from {}", response.status(), url);
    }

    // Parse JSON
    let well_known: WellKnownResponse = response
        .json()
        .with_context(|| format!("Failed to parse JSON from {}", url))?;

    // Check if name exists in response
    let returned_pubkey = well_known
        .names
        .get(local_part)
        .ok_or_else(|| anyhow::anyhow!("Name '{}' not found in response", local_part))?;

    // Compare pubkeys (case-insensitive hex comparison)
    let verified = returned_pubkey.to_lowercase() == pubkey.to_lowercase();

    Ok(verified)
}

/// Cache for NIP-05 verification results
#[derive(Debug, Serialize, Deserialize)]
pub struct Nip05Cache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CacheEntry {
    identifier: String,
    pubkey: String,
    verified: bool,
    checked_at: u64, // Unix timestamp
}

impl Nip05Cache {
    /// Load cache from file
    pub fn load() -> Result<Self> {
        let cache_path = Self::cache_path()?;

        if !cache_path.exists() {
            return Ok(Self {
                entries: HashMap::new(),
            });
        }

        let content = std::fs::read_to_string(&cache_path)?;
        let cache: Nip05Cache = serde_json::from_str(&content)?;

        Ok(cache)
    }

    /// Save cache to file
    pub fn save(&self) -> Result<()> {
        let cache_path = Self::cache_path()?;

        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&cache_path, content)?;

        Ok(())
    }

    /// Get cache file path
    fn cache_path() -> Result<std::path::PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".openagents").join("nip05_cache.json"))
    }

    /// Check if verification is cached and still valid (24 hour TTL)
    pub fn get(&self, identifier: &str, pubkey: &str) -> Option<bool> {
        let key = format!("{}@{}", identifier, pubkey);
        let entry = self.entries.get(&key)?;

        // Check if entry is still valid (24 hours)
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();

        let age = now.saturating_sub(entry.checked_at);
        if age > 24 * 3600 {
            // Expired
            return None;
        }

        Some(entry.verified)
    }

    /// Add verification result to cache
    pub fn set(&mut self, identifier: &str, pubkey: &str, verified: bool) {
        let key = format!("{}@{}", identifier, pubkey);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.entries.insert(
            key,
            CacheEntry {
                identifier: identifier.to_string(),
                pubkey: pubkey.to_string(),
                verified,
                checked_at: now,
            },
        );
    }
}

/// Verify NIP-05 with caching
pub fn verify_nip05_cached(identifier: &str, pubkey: &str) -> Result<bool> {
    // Try to load from cache
    let mut cache = Nip05Cache::load().unwrap_or_else(|_| Nip05Cache {
        entries: HashMap::new(),
    });

    // Check cache
    if let Some(cached_result) = cache.get(identifier, pubkey) {
        return Ok(cached_result);
    }

    // Not in cache or expired, verify
    let verified = verify_nip05(identifier, pubkey)?;

    // Update cache
    cache.set(identifier, pubkey, verified);
    if let Err(e) = cache.save() {
        tracing::debug!("Failed to save NIP-05 cache: {}", e);
    }

    Ok(verified)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identifier() {
        let identifier = "bob@example.com";
        let parts: Vec<&str> = identifier.split('@').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "bob");
        assert_eq!(parts[1], "example.com");
    }

    #[test]
    fn test_invalid_identifier() {
        let result = verify_nip05("invalid", "abc123");
        assert!(result.is_err());
    }

    #[test]
    fn test_cache() {
        let mut cache = Nip05Cache {
            entries: HashMap::new(),
        };

        cache.set("bob@example.com", "pubkey123", true);

        let result = cache.get("bob@example.com", "pubkey123");
        assert_eq!(result, Some(true));
    }
}
