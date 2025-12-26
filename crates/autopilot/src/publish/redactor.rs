//! Secret Redactor Wrapper
//!
//! Wraps the main redact module for use in the publish pipeline.

use crate::redact;

/// Pattern for redaction (for display purposes)
#[derive(Debug, Clone)]
pub struct RedactionPattern {
    /// Pattern name
    pub name: String,
    /// Description
    pub description: String,
}

/// Secret redactor for replay publishing
#[derive(Debug, Clone, Default)]
pub struct SecretRedactor {
    /// Additional custom patterns (not implemented yet)
    custom_patterns: Vec<String>,
}

impl SecretRedactor {
    /// Create a new secret redactor
    pub fn new() -> Self {
        Self::default()
    }

    /// Redact secrets from text
    pub fn redact(&self, text: &str) -> String {
        redact::redact_secrets(text)
    }

    /// Count how many secrets would be redacted
    pub fn count_redactions(&self, text: &str) -> usize {
        let original = text;
        let redacted = self.redact(text);

        // Count occurrences of [REDACTED] in the output
        redacted.matches("[REDACTED]").count()
    }

    /// Check if text contains any secrets
    pub fn has_secrets(&self, text: &str) -> bool {
        self.count_redactions(text) > 0
    }

    /// Get list of built-in patterns (for documentation)
    pub fn builtin_patterns() -> Vec<RedactionPattern> {
        vec![
            RedactionPattern {
                name: "API Keys".to_string(),
                description: "OpenAI, Anthropic, Google API keys".to_string(),
            },
            RedactionPattern {
                name: "GitHub Tokens".to_string(),
                description: "Personal access tokens, OAuth tokens".to_string(),
            },
            RedactionPattern {
                name: "AWS Credentials".to_string(),
                description: "Access keys and secret keys".to_string(),
            },
            RedactionPattern {
                name: "Database URLs".to_string(),
                description: "PostgreSQL, MySQL, MongoDB connection strings".to_string(),
            },
            RedactionPattern {
                name: "Private Keys".to_string(),
                description: "RSA, SSH, PEM private keys".to_string(),
            },
            RedactionPattern {
                name: "JWT Tokens".to_string(),
                description: "JSON Web Tokens".to_string(),
            },
            RedactionPattern {
                name: "Webhook URLs".to_string(),
                description: "Slack, Discord webhook URLs".to_string(),
            },
            RedactionPattern {
                name: "Password Assignments".to_string(),
                description: "password=, secret=, etc.".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redactor_creation() {
        let redactor = SecretRedactor::new();
        assert!(redactor.custom_patterns.is_empty());
    }

    #[test]
    fn test_redact_api_key() {
        let redactor = SecretRedactor::new();
        let input = "My API key is sk-abc123xyz789abcdefghijklmnop";
        let output = redactor.redact(input);
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("sk-abc123"));
    }

    #[test]
    fn test_redact_github_token() {
        let redactor = SecretRedactor::new();
        let input = "Token: ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let output = redactor.redact(input);
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn test_count_redactions() {
        let redactor = SecretRedactor::new();
        let input =
            "Key1: sk-key123abc456def789ghi012jklmnopqrst Key2: ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let count = redactor.count_redactions(input);
        assert!(count >= 1);
    }

    #[test]
    fn test_has_secrets() {
        let redactor = SecretRedactor::new();

        assert!(redactor.has_secrets("API key: sk-abc123456789012345678901234567890"));
        assert!(!redactor.has_secrets("This is just normal text"));
    }

    #[test]
    fn test_no_false_positives() {
        let redactor = SecretRedactor::new();
        let safe_text = "Hello, this is a normal message with no secrets.";
        let output = redactor.redact(safe_text);
        assert_eq!(output, safe_text);
    }

    #[test]
    fn test_builtin_patterns() {
        let patterns = SecretRedactor::builtin_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.name.contains("API")));
    }
}
