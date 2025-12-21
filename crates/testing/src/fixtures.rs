//! Test Fixtures and Factories
//!
//! Provides factories for generating test data.

use fake::{Fake, Faker};

/// Generate a test keypair (deterministic for testing)
pub fn test_keypair() -> ([u8; 32], [u8; 33]) {
    let secret_key = [42u8; 32]; // Fixed for reproducibility
    let public_key = [0u8; 33]; // Placeholder
    (secret_key, public_key)
}

/// Generate a test npub (bech32 encoded public key)
pub fn test_npub() -> String {
    // Placeholder - would use actual bech32 encoding
    "npub1test123456789abcdefghijk".to_string()
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
pub fn test_bitcoin_address() -> String {
    // Placeholder - would generate valid testnet address
    "tb1qtest123456789abcdefghijk".to_string()
}

/// Generate a test Lightning invoice
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
    fn test_random_text_generates_different_values() {
        let text1 = random_text(10, 20);
        let text2 = random_text(10, 20);
        // Note: This might occasionally fail due to randomness
        // In practice, collision is extremely unlikely
        assert_ne!(text1, text2);
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
