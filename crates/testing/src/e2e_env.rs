//! E2E Environment Setup
//!
//! Provides pre-configured environments for end-to-end testing of different
//! OpenAgents subsystems.

use crate::{CliHarness, MockRelay, RegtestFaucet};
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// E2E testing environment with CLI harness and optional services
pub struct E2EEnvironment {
    /// CLI harness for running commands
    pub cli: CliHarness,
    /// Mock Nostr relay (if started)
    pub mock_relay: Option<MockRelay>,
    /// Regtest faucet (if available)
    pub faucet: Option<RegtestFaucet>,
    /// Database path
    pub db_path: PathBuf,
    /// Log directory for trajectory files
    pub log_dir: PathBuf,
    /// Temp directory holder (for cleanup)
    _temp_dir: TempDir,
}

impl E2EEnvironment {
    /// Create environment for autopilot testing
    ///
    /// Includes:
    /// - CLI harness with autopilot env vars
    /// - Log directory for trajectory files
    /// - Database path
    pub async fn for_autopilot() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let db_path = temp_path.join("autopilot.db");
        let log_dir = temp_path.join("logs");
        std::fs::create_dir_all(&log_dir)?;

        let mut cli = CliHarness::with_workdir(temp_path.to_path_buf()).await?;

        // Set autopilot-specific environment variables
        cli.set_env("AUTOPILOT_DB", db_path.to_string_lossy().to_string());
        cli.set_env("AUTOPILOT_LOG_DIR", log_dir.to_string_lossy().to_string());
        cli.set_env("AUTOPILOT_NON_INTERACTIVE", "1");

        // Set path to autopilot binary (for `openagents autopilot run`)
        if let Some(autopilot_bin) = Self::find_autopilot_bin() {
            cli.set_env(
                "OPENAGENTS_AUTOPILOT_BIN",
                autopilot_bin.to_string_lossy().to_string(),
            );
        }

        Ok(Self {
            cli,
            mock_relay: None,
            faucet: None,
            db_path,
            log_dir,
            _temp_dir: temp_dir,
        })
    }

    /// Create environment for autopilot testing with mock relay
    ///
    /// Same as `for_autopilot` but also starts a mock Nostr relay
    pub async fn for_autopilot_with_relay() -> Result<Self> {
        let mut env = Self::for_autopilot().await?;

        let relay = MockRelay::start().await;
        let relay_url = relay.url().to_string();

        env.cli.set_env("NOSTR_RELAY_URL", relay_url);
        env.mock_relay = Some(relay);

        Ok(env)
    }

    /// Create environment for wallet testing
    ///
    /// Includes:
    /// - CLI harness with wallet env vars
    /// - Faucet (if credentials available)
    pub async fn for_wallet() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let db_path = temp_path.join("wallet.db");
        let log_dir = temp_path.join("logs");
        std::fs::create_dir_all(&log_dir)?;

        let mut cli = CliHarness::with_workdir(temp_path.to_path_buf()).await?;

        // Set wallet-specific environment variables
        cli.set_env("WALLET_DB", db_path.to_string_lossy().to_string());
        cli.set_env("WALLET_DATA_DIR", temp_path.to_string_lossy().to_string());

        // Try to create faucet
        let faucet = RegtestFaucet::new().ok();

        Ok(Self {
            cli,
            mock_relay: None,
            faucet,
            db_path,
            log_dir,
            _temp_dir: temp_dir,
        })
    }

    /// Create environment for marketplace testing
    ///
    /// Includes:
    /// - CLI harness
    /// - Mock relay for NIP-90 job requests
    pub async fn for_marketplace() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let db_path = temp_path.join("marketplace.db");
        let log_dir = temp_path.join("logs");
        std::fs::create_dir_all(&log_dir)?;

        let mut cli = CliHarness::with_workdir(temp_path.to_path_buf()).await?;

        // Start mock relay for marketplace
        let relay = MockRelay::start().await;
        let relay_url = relay.url().to_string();

        cli.set_env("NOSTR_RELAY_URL", relay_url);
        cli.set_env(
            "MARKETPLACE_DATA_DIR",
            temp_path.to_string_lossy().to_string(),
        );

        Ok(Self {
            cli,
            mock_relay: Some(relay),
            faucet: None,
            db_path,
            log_dir,
            _temp_dir: temp_dir,
        })
    }

    /// Create environment for daemon testing
    ///
    /// Similar to autopilot but with daemon-specific setup
    pub async fn for_daemon() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let db_path = temp_path.join("daemon.db");
        let log_dir = temp_path.join("logs");
        std::fs::create_dir_all(&log_dir)?;

        let mut cli = CliHarness::with_workdir(temp_path.to_path_buf()).await?;

        // Set daemon-specific environment variables
        cli.set_env("AUTOPILOT_DB", db_path.to_string_lossy().to_string());
        cli.set_env("AUTOPILOT_LOG_DIR", log_dir.to_string_lossy().to_string());
        cli.set_env("DAEMON_WORKDIR", temp_path.to_string_lossy().to_string());

        Ok(Self {
            cli,
            mock_relay: None,
            faucet: None,
            db_path,
            log_dir,
            _temp_dir: temp_dir,
        })
    }

    /// Create a minimal environment for quick tests
    pub async fn minimal() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let db_path = temp_path.join("test.db");
        let log_dir = temp_path.join("logs");
        std::fs::create_dir_all(&log_dir)?;

        let cli = CliHarness::with_workdir(temp_path.to_path_buf()).await?;

        Ok(Self {
            cli,
            mock_relay: None,
            faucet: None,
            db_path,
            log_dir,
            _temp_dir: temp_dir,
        })
    }

    /// Get the temp directory path
    pub fn temp_path(&self) -> &std::path::Path {
        self._temp_dir.path()
    }

    /// Find the autopilot binary in common locations
    fn find_autopilot_bin() -> Option<PathBuf> {
        let candidates = [
            // Debug build
            PathBuf::from("target/debug/autopilot"),
            // Release build
            PathBuf::from("target/release/autopilot"),
            // From workspace root (when running from crate dir)
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target/debug/autopilot"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target/release/autopilot"),
        ];

        for path in &candidates {
            if path.exists() {
                return Some(path.clone());
            }
        }

        None
    }

    /// Get trajectory files created during testing
    pub fn get_trajectory_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if self.log_dir.exists() {
            for entry in walkdir::WalkDir::new(&self.log_dir)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().map(|e| e == "rlog").unwrap_or(false) {
                    files.push(path.to_path_buf());
                }
            }
        }

        Ok(files)
    }

    /// Cleanup the environment
    ///
    /// Shuts down the mock relay if running.
    /// The temp directory will be cleaned up on drop.
    pub async fn cleanup(self) -> Result<()> {
        if let Some(relay) = self.mock_relay {
            relay.shutdown().await;
        }
        Ok(())
    }
}

/// Helper to extract issue ID from CLI output
///
/// Looks for patterns like "Issue #123" or "issue_123" or "ID: 123"
pub fn extract_issue_id(output: &str) -> Option<String> {
    // Try various patterns
    let patterns = [
        r"Issue #(\d+)",
        r"issue_(\d+)",
        r"ID:\s*(\d+)",
        r"Created issue (\d+)",
        r"issue (\d+)",
    ];

    for pattern in &patterns {
        if let Some(caps) = regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(output))
        {
            if let Some(id) = caps.get(1) {
                return Some(id.as_str().to_string());
            }
        }
    }

    // Also try to find any numeric ID in the output
    if let Some(caps) = regex::Regex::new(r"(\d+)")
        .ok()
        .and_then(|re| re.captures(output))
    {
        if let Some(id) = caps.get(1) {
            return Some(id.as_str().to_string());
        }
    }

    None
}

/// Helper to extract Bitcoin address from CLI output
pub fn extract_address(output: &str) -> Option<String> {
    // Look for Bitcoin addresses (tb1..., bc1..., 1..., 3...)
    let patterns = [
        r"(tb1[a-z0-9]{39,})",                 // Testnet bech32
        r"(bc1[a-z0-9]{39,})",                 // Mainnet bech32
        r"([123][a-km-zA-HJ-NP-Z1-9]{25,34})", // Legacy
    ];

    for pattern in &patterns {
        if let Some(caps) = regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(output))
        {
            if let Some(addr) = caps.get(1) {
                return Some(addr.as_str().to_string());
            }
        }
    }

    None
}

/// Helper to extract sats amount from CLI output
pub fn extract_sats(output: &str) -> Option<u64> {
    // Try various patterns for amounts (comma patterns first to match full numbers with commas)
    let patterns = [
        r"(\d[\d,]*)\s*satoshis?",
        r"(\d[\d,]*)\s*sats?",
        r"Balance:\s*(\d[\d,]*)",
        r"(\d+)\s*sats?",
    ];

    for pattern in &patterns {
        if let Some(caps) = regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(output))
        {
            if let Some(amount) = caps.get(1) {
                let cleaned = amount.as_str().replace(',', "");
                if let Ok(sats) = cleaned.parse::<u64>() {
                    return Some(sats);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_issue_id() {
        assert_eq!(
            extract_issue_id("Created Issue #123"),
            Some("123".to_string())
        );
        assert_eq!(
            extract_issue_id("issue_456 created"),
            Some("456".to_string())
        );
        assert_eq!(extract_issue_id("ID: 789"), Some("789".to_string()));
        assert_eq!(extract_issue_id("no id here"), None);
    }

    #[test]
    fn test_extract_address() {
        assert_eq!(
            extract_address("Send to: tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx"),
            Some("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".to_string())
        );
        assert_eq!(extract_address("no address"), None);
    }

    #[test]
    fn test_extract_sats() {
        assert_eq!(extract_sats("Balance: 1000 sats"), Some(1000));
        assert_eq!(extract_sats("10,000 satoshis available"), Some(10000));
        assert_eq!(extract_sats("no amount"), None);
    }

    #[tokio::test]
    async fn test_for_autopilot_creates_dirs() {
        if CliHarness::find_binary().is_err() {
            println!("Binary not found, skipping test");
            return;
        }

        let env = E2EEnvironment::for_autopilot().await.unwrap();
        assert!(env.log_dir.exists());
        assert!(env.temp_path().exists());
    }
}
