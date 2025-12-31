//! Test Fixtures and Factories
//!
//! Provides factories for generating test data.

use fake::Fake;

/// Generate a test keypair (deterministic for testing)
///
/// Returns (secret_key, public_key) where both are valid secp256k1 keys.
/// The secret key is deterministic for test reproducibility.
pub fn test_keypair() -> ([u8; 32], [u8; 32]) {
    let secret_key = [42u8; 32]; // Fixed for reproducibility
    let public_key = nostr::get_public_key(&secret_key)
        .expect("Failed to derive public key from test secret key");
    (secret_key, public_key)
}

/// Generate a test npub (bech32 encoded public key)
///
/// Returns a valid NIP-19 npub using the test keypair's public key.
pub fn test_npub() -> String {
    let (_, public_key) = test_keypair();
    nostr::public_key_to_npub(&public_key).expect("Failed to encode test public key as npub")
}

/// Generate a test event ID
pub fn test_event_id() -> String {
    format!("event_{}", uuid::Uuid::new_v4())
}

/// Generate random text of specified length
pub fn random_text(min_len: usize, max_len: usize) -> String {
    use fake::faker::lorem::en::*;
    Sentence(min_len..max_len).fake()
}

/// Generate a test Bitcoin address
///
/// Returns a valid testnet P2WPKH (native segwit) address derived from the test keypair.
pub fn test_bitcoin_address() -> String {
    use bitcoin::{Address, CompressedPublicKey, Network};

    let (secret_key, _) = test_keypair();

    // Create a CompressedPublicKey from the secret key
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let secret = bitcoin::secp256k1::SecretKey::from_slice(&secret_key)
        .expect("32 bytes, within curve order");
    let public_key = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &secret);
    let compressed_pk = CompressedPublicKey(public_key);

    // Generate P2WPKH address for testnet
    Address::p2wpkh(&compressed_pk, Network::Testnet).to_string()
}

/// Generate a test Lightning invoice
///
/// Note: This is a simplified test format, not a fully valid BOLT-11 invoice.
/// For tests requiring valid invoices, use the actual Lightning SDK.
pub fn test_lightning_invoice(amount_sats: u64) -> String {
    format!("lnbc{}1test", amount_sats)
}

/// Factory for creating test issues
pub struct IssueFactory {
    title: String,
    description: Option<String>,
    priority: String,
}

impl IssueFactory {
    /// Create a new issue factory with defaults
    pub fn new() -> Self {
        Self {
            title: "Test Issue".to_string(),
            description: None,
            priority: "medium".to_string(),
        }
    }

    /// Set the title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the priority
    pub fn priority(mut self, priority: impl Into<String>) -> Self {
        self.priority = priority.into();
        self
    }

    /// Build the test issue data
    pub fn build(self) -> TestIssue {
        TestIssue {
            title: self.title,
            description: self.description,
            priority: self.priority,
        }
    }
}

impl Default for IssueFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Test issue data structure
#[derive(Debug, Clone)]
pub struct TestIssue {
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_is_deterministic() {
        let (sk1, pk1) = test_keypair();
        let (sk2, pk2) = test_keypair();
        assert_eq!(sk1, sk2);
        assert_eq!(pk1, pk2);
    }

    #[test]
    fn test_keypair_produces_valid_public_key() {
        let (secret_key, public_key) = test_keypair();

        // Verify the public key is correctly derived from the secret key
        let derived_pk = nostr::get_public_key(&secret_key).unwrap();
        assert_eq!(public_key, derived_pk);

        // Public key should not be all zeros (would indicate placeholder)
        assert_ne!(public_key, [0u8; 32]);
    }

    #[test]
    fn test_npub_is_valid() {
        let npub = test_npub();

        // Should start with "npub1"
        assert!(npub.starts_with("npub1"));

        // Should be decodable back to a public key
        let decoded = nostr::npub_to_public_key(&npub).unwrap();
        let (_, expected_pk) = test_keypair();
        assert_eq!(decoded, expected_pk);
    }

    #[test]
    fn test_bitcoin_address_is_valid_testnet() {
        let addr = test_bitcoin_address();

        // Testnet P2WPKH addresses start with "tb1q"
        assert!(addr.starts_with("tb1q"));

        // Should be a valid length for bech32
        assert!(addr.len() >= 42 && addr.len() <= 62);
    }

    #[test]
    fn test_bitcoin_address_is_deterministic() {
        let addr1 = test_bitcoin_address();
        let addr2 = test_bitcoin_address();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_random_text_generates_valid_output() {
        // Test that random_text produces output and doesn't panic
        let text = random_text(10, 20);

        // Should produce non-empty text
        assert!(!text.is_empty());

        // Generate multiple samples to verify it works consistently
        for _ in 0..10 {
            let sample = random_text(5, 15);
            assert!(!sample.is_empty());
        }
    }

    #[test]
    fn test_issue_factory() {
        let issue = IssueFactory::new()
            .title("Custom Title")
            .description("Custom Description")
            .priority("high")
            .build();

        assert_eq!(issue.title, "Custom Title");
        assert_eq!(issue.description, Some("Custom Description".to_string()));
        assert_eq!(issue.priority, "high");
    }
}
