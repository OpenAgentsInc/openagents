//! Secret redaction engine for trajectory data
//!
//! This module provides open-source secret detection and redaction
//! for trajectory contributions. All redaction happens locally before
//! any data leaves the user's machine.
//!
//! # Examples
//!
//! Basic usage with standard redaction level:
//!
//! ```no_run
//! use marketplace::trajectories::{RedactionEngine, RedactionLevel};
//!
//! let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
//! let content = "My API key is sk-1234567890abcdef and my password is hunter2";
//! let result = engine.redact(content).unwrap();
//!
//! println!("Redacted {} secrets", result.secrets_redacted);
//! println!("Secret types: {:?}", result.secret_types);
//! ```
//!
//! Using custom patterns for domain-specific secrets:
//!
//! ```no_run
//! use marketplace::trajectories::{RedactionEngine, RedactionLevel};
//!
//! // Redact custom internal IDs
//! let custom_patterns = vec![
//!     r"INTERNAL-\d{6}".to_string(),
//!     r"PROJECT-[A-Z]{4}-\d{4}".to_string(),
//! ];
//!
//! let engine = RedactionEngine::new(
//!     RedactionLevel::Strict,
//!     custom_patterns
//! ).unwrap();
//!
//! let content = "Reference INTERNAL-123456 and PROJECT-ACME-2024";
//! let result = engine.redact(content).unwrap();
//! assert!(result.secrets_redacted >= 2);
//! ```

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Redaction level determines aggressiveness of pattern matching
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionLevel {
    /// Standard redaction (common patterns)
    Standard,
    /// Strict redaction (conservative, may over-redact)
    Strict,
    /// Paranoid redaction (maximum safety, heavy over-redaction)
    Paranoid,
}

impl FromStr for RedactionLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "strict" => Ok(Self::Strict),
            "paranoid" => Ok(Self::Paranoid),
            _ => Err(format!("Unknown redaction level: {}", s)),
        }
    }
}

/// Result of a redaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionResult {
    /// Original content length
    pub original_length: usize,

    /// Redacted content length
    pub redacted_length: usize,

    /// Number of secrets redacted
    pub secrets_redacted: usize,

    /// Types of secrets found
    pub secret_types: Vec<String>,

    /// Redacted content
    pub content: String,
}

/// Secret detection and redaction engine
pub struct RedactionEngine {
    level: RedactionLevel,
    custom_patterns: Vec<Regex>,
}

impl RedactionEngine {
    /// Create a new redaction engine
    pub fn new(level: RedactionLevel, custom_patterns: Vec<String>) -> Result<Self> {
        let custom_patterns = custom_patterns
            .into_iter()
            .map(|p| Regex::new(&p))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            level,
            custom_patterns,
        })
    }

    /// Redact secrets from content
    pub fn redact(&self, content: &str) -> Result<RedactionResult> {
        let original_length = content.len();
        let mut redacted = content.to_string();
        let mut secrets_redacted = 0;
        let mut secret_types = Vec::new();

        // Apply built-in patterns
        for pattern in self.get_patterns() {
            let (new_content, count) =
                self.apply_pattern(&redacted, &pattern.regex, pattern.replacement);
            if count > 0 {
                redacted = new_content;
                secrets_redacted += count;
                if !secret_types.contains(&pattern.name) {
                    secret_types.push(pattern.name.clone());
                }
            }
        }

        // Apply custom patterns
        for (i, regex) in self.custom_patterns.iter().enumerate() {
            let (new_content, count) = self.apply_pattern(&redacted, regex, "[REDACTED-CUSTOM]");
            if count > 0 {
                redacted = new_content;
                secrets_redacted += count;
                let type_name = format!("custom-{}", i + 1);
                if !secret_types.contains(&type_name) {
                    secret_types.push(type_name);
                }
            }
        }

        Ok(RedactionResult {
            original_length,
            redacted_length: redacted.len(),
            secrets_redacted,
            secret_types,
            content: redacted,
        })
    }

    /// Apply a single pattern and count replacements
    fn apply_pattern(&self, content: &str, regex: &Regex, replacement: &str) -> (String, usize) {
        let mut count = 0;
        let result = regex.replace_all(content, |_: &regex::Captures| {
            count += 1;
            replacement
        });
        (result.to_string(), count)
    }

    /// Get patterns for the configured redaction level
    fn get_patterns(&self) -> Vec<RedactionPattern> {
        let mut patterns = standard_patterns();

        if self.level == RedactionLevel::Strict || self.level == RedactionLevel::Paranoid {
            patterns.extend(strict_patterns());
        }

        if self.level == RedactionLevel::Paranoid {
            patterns.extend(paranoid_patterns());
        }

        patterns
    }
}

/// A redaction pattern
struct RedactionPattern {
    name: String,
    regex: Regex,
    replacement: &'static str,
}

/// Standard redaction patterns (common secrets)
/// NOTE: Order matters! More specific patterns must come before general ones.
fn standard_patterns() -> Vec<RedactionPattern> {
    vec![
        // Private keys (catch-all for PEM format)
        RedactionPattern {
            name: "private-key".to_string(),
            regex: Regex::new(r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]+?-----END (RSA |EC |OPENSSH )?PRIVATE KEY-----").unwrap(),
            replacement: "[REDACTED-PRIVATE-KEY]",
        },
        RedactionPattern {
            name: "ssh_key".to_string(),
            regex: Regex::new(r"-----BEGIN OPENSSH PRIVATE KEY-----[\s\S]+?-----END OPENSSH PRIVATE KEY-----").unwrap(),
            replacement: "[REDACTED-SSH-KEY]",
        },
        // Specific API keys (must come before generic sk- pattern)
        RedactionPattern {
            name: "anthropic_key".to_string(),
            regex: Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
            replacement: "[REDACTED-ANTHROPIC-KEY]",
        },
        RedactionPattern {
            name: "stripe_key".to_string(),
            regex: Regex::new(r"[sp]k_(test|live)_[a-zA-Z0-9]{20,}").unwrap(),
            replacement: "[REDACTED-STRIPE-KEY]",
        },
        // Generic OpenAI key (after more specific patterns)
        RedactionPattern {
            name: "openai_key".to_string(),
            regex: Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(),
            replacement: "[REDACTED-OPENAI-KEY]",
        },
        RedactionPattern {
            name: "aws-key".to_string(),
            regex: Regex::new(r"(?i)(AKIA[0-9A-Z]{16})").unwrap(),
            replacement: "[REDACTED-AWS-KEY]",
        },
        RedactionPattern {
            name: "github_token".to_string(),
            regex: Regex::new(r"gh[pousr]_[a-zA-Z0-9]{20,}").unwrap(),
            replacement: "[REDACTED-GITHUB-TOKEN]",
        },
        RedactionPattern {
            name: "slack_token".to_string(),
            regex: Regex::new(r"xox[baprs]-[a-zA-Z0-9-]+").unwrap(),
            replacement: "[REDACTED-SLACK-TOKEN]",
        },
        RedactionPattern {
            name: "google_api_key".to_string(),
            regex: Regex::new(r"AIza[a-zA-Z0-9_-]{20,}").unwrap(),
            replacement: "[REDACTED-GOOGLE-API-KEY]",
        },
        RedactionPattern {
            name: "mailgun_key".to_string(),
            regex: Regex::new(r"key-[a-zA-Z0-9]{32}").unwrap(),
            replacement: "[REDACTED-MAILGUN-KEY]",
        },
        RedactionPattern {
            name: "twilio_key".to_string(),
            regex: Regex::new(r"SK[a-z0-9]{32}").unwrap(),
            replacement: "[REDACTED-TWILIO-KEY]",
        },
        RedactionPattern {
            name: "jwt".to_string(),
            regex: Regex::new(r"eyJ[a-zA-Z0-9_-]+\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+").unwrap(),
            replacement: "[REDACTED-JWT]",
        },
        RedactionPattern {
            name: "password".to_string(),
            regex: Regex::new(r#"(?i)(password|passwd|pwd)[\s:=]+['"]([^'"]{8,})"#).unwrap(),
            replacement: "[REDACTED-PASSWORD]",
        },
        RedactionPattern {
            name: "bearer_token".to_string(),
            regex: Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_.-=]{20,}").unwrap(),
            replacement: "[REDACTED-BEARER-TOKEN]",
        },
        // Generic patterns (last resort, after specific patterns)
        RedactionPattern {
            name: "api_key".to_string(),
            regex: Regex::new(r#"(?i)(api[_-]?key|apikey)[\s:=]+['"]?([a-zA-Z0-9_-]{20,})"#).unwrap(),
            replacement: "[REDACTED-API-KEY]",
        },
        RedactionPattern {
            name: "secret_key".to_string(),
            regex: Regex::new(r#"(?i)(secret[_-]?key|secretkey)[\s:=]+['"]?([a-zA-Z0-9_-]{20,})"#).unwrap(),
            replacement: "[REDACTED-SECRET-KEY]",
        },
    ]
}

/// Strict patterns (more conservative)
fn strict_patterns() -> Vec<RedactionPattern> {
    vec![
        RedactionPattern {
            name: "bitcoin_privkey".to_string(),
            regex: Regex::new(r"\b[5KL][1-9A-HJ-NP-Za-km-z]{50,51}\b").unwrap(),
            replacement: "[REDACTED-BITCOIN-KEY]",
        },
        RedactionPattern {
            name: "nostr_nsec".to_string(),
            regex: Regex::new(r"nsec1[a-z0-9]{58,}").unwrap(),
            replacement: "[REDACTED-NOSTR-KEY]",
        },
        RedactionPattern {
            name: "lightning_invoice".to_string(),
            regex: Regex::new(r"ln(bc|tb|bcrt)[a-z0-9]{100,}").unwrap(),
            replacement: "[REDACTED-LIGHTNING-INVOICE]",
        },
        RedactionPattern {
            name: "hex_string".to_string(),
            regex: Regex::new(r"\b[a-fA-F0-9]{64,}\b").unwrap(),
            replacement: "[REDACTED-HEX]",
        },
        RedactionPattern {
            name: "base64".to_string(),
            regex: Regex::new(r"\b[A-Za-z0-9+/]{40,}={0,2}\b").unwrap(),
            replacement: "[REDACTED-BASE64]",
        },
        RedactionPattern {
            name: "ipv4".to_string(),
            regex: Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            replacement: "[REDACTED-IP]",
        },
        RedactionPattern {
            name: "uuid".to_string(),
            regex: Regex::new(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")
                .unwrap(),
            replacement: "[REDACTED-UUID]",
        },
    ]
}

/// Paranoid patterns (maximum safety, may over-redact)
fn paranoid_patterns() -> Vec<RedactionPattern> {
    vec![
        RedactionPattern {
            name: "url".to_string(),
            regex: Regex::new(r"https?://[^\s]+").unwrap(),
            replacement: "[REDACTED-URL]",
        },
        RedactionPattern {
            name: "email".to_string(),
            regex: Regex::new(r"\b[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}\b").unwrap(),
            replacement: "[REDACTED-EMAIL]",
        },
        RedactionPattern {
            name: "long_alphanumeric".to_string(),
            regex: Regex::new(r"\b[a-zA-Z0-9]{25,}\b").unwrap(),
            replacement: "[REDACTED-TOKEN]",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "API_KEY=myapikey_1234567890abcdefghij";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-API-KEY]"));
        assert!(!result.content.contains("myapikey_"));
        assert_eq!(result.secrets_redacted, 1);
    }

    #[test]
    fn test_private_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBg...\n-----END PRIVATE KEY-----";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-PRIVATE-KEY]"));
        assert!(!result.content.contains("MIIEvgIBADANBg"));
    }

    #[test]
    fn test_github_token_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "token: ghp_1234567890abcdefghijklmnopqrstuv";
        let result = engine.redact(content).unwrap();

        eprintln!("Original: {}", content);
        eprintln!("Redacted: {}", result.content);
        eprintln!("Secrets redacted: {}", result.secrets_redacted);
        eprintln!("Secret types: {:?}", result.secret_types);

        assert!(result.content.contains("[REDACTED-GITHUB-TOKEN]"));
        assert!(!result.content.contains("ghp_"));
    }

    #[test]
    fn test_custom_pattern() {
        let custom = vec![r"CUSTOM-\d+".to_string()];
        let engine = RedactionEngine::new(RedactionLevel::Standard, custom).unwrap();
        let content = "My secret is CUSTOM-12345";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-CUSTOM]"));
        assert!(!result.content.contains("CUSTOM-12345"));
    }

    #[test]
    fn test_strict_mode() {
        let engine = RedactionEngine::new(RedactionLevel::Strict, vec![]).unwrap();
        let content = "IP address: 192.168.1.1";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-IP]"));
    }

    #[test]
    fn test_anthropic_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "ANTHROPIC_API_KEY=sk-ant-api03-1234567890abcdefghijklmnopqrstuvwxyz1234567890abcdefghijklmnopqrstuvwxyz1234567890abcd";
        let result = engine.redact(content).unwrap();

        eprintln!("Anthropic - Secrets redacted: {}", result.secrets_redacted);
        eprintln!("Anthropic - Content: {}", result.content);

        assert!(result.content.contains("[REDACTED-ANTHROPIC-KEY]"));
        assert!(!result.content.contains("sk-ant-"));
    }

    #[test]
    fn test_openai_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "OPENAI_API_KEY=sk-1234567890abcdefghijklmnopqrstuvwxyz";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-OPENAI-KEY]"));
        assert!(!result.content.contains("sk-1234567890"));
    }

    #[test]
    fn test_aws_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-AWS-KEY]"));
        assert!(!result.content.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_slack_token_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "SLACK_TOKEN=xoxb-EXAMPLE-TOKEN-NOT-REAL";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-SLACK-TOKEN]"));
        assert!(!result.content.contains("xoxb-"));
    }

    #[test]
    fn test_stripe_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "STRIPE_SECRET_KEY=sk_test_abcdefghijklmnopqrstuvwxyz123456";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-STRIPE-KEY]"));
        assert!(!result.content.contains("sk_test_"));
    }

    #[test]
    fn test_google_api_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "GOOGLE_API_KEY=AIzaSyDdI0hCZtE6vySjMm-WEfRq3CPzqKqqsHI";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-GOOGLE-API-KEY]"));
        assert!(!result.content.contains("AIzaSy"));
    }

    #[test]
    fn test_bitcoin_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Strict, vec![]).unwrap();
        let content = "BTC_PRIVKEY=5HueCGU8rMjxEXxiPuD5BDku4MkFqeZyd4dZ1jvhTVqvbTLvyTJ";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-BITCOIN-KEY]"));
        assert!(!result.content.contains("5HueCGU8"));
    }

    #[test]
    fn test_nostr_nsec_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Strict, vec![]).unwrap();
        let content = "NOSTR_KEY=nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-NOSTR-KEY]"));
        assert!(!result.content.contains("nsec1"));
    }

    #[test]
    fn test_multiple_secrets() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = r#"
            ANTHROPIC_API_KEY=sk-ant-api03-abc123xyz789def456ghi012jkl345mno678pqr901stu234vwx567yza890bcd123efg456hij789klm012nop345qrs678tuv901wxy234
            GITHUB_TOKEN=ghp_1234567890abcdefghijklmnopqrstuv
            AWS_KEY=AKIAIOSFODNN7EXAMPLE
        "#;
        let result = engine.redact(content).unwrap();

        assert!(result.secrets_redacted >= 3);
        assert!(result.content.contains("[REDACTED-ANTHROPIC-KEY]"));
        assert!(result.content.contains("[REDACTED-GITHUB-TOKEN]"));
        assert!(result.content.contains("[REDACTED-AWS-KEY]"));
        assert!(!result.content.contains("sk-ant-"));
        assert!(!result.content.contains("ghp_"));
        assert!(!result.content.contains("AKIAIOSFODNN7"));
    }

    #[test]
    fn test_ssh_private_key_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\n-----END OPENSSH PRIVATE KEY-----";
        let result = engine.redact(content).unwrap();

        // SSH keys are caught by both ssh_key and private-key patterns
        // Either redaction is acceptable
        assert!(
            result.content.contains("[REDACTED-SSH-KEY]")
                || result.content.contains("[REDACTED-PRIVATE-KEY]")
        );
        assert!(!result.content.contains("b3BlbnNzaC"));
    }

    #[test]
    fn test_jwt_redaction() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-JWT]"));
        assert!(
            !result
                .content
                .contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
        );
    }

    #[test]
    fn test_redaction_level_escalation() {
        let standard_engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let strict_engine = RedactionEngine::new(RedactionLevel::Strict, vec![]).unwrap();
        let paranoid_engine = RedactionEngine::new(RedactionLevel::Paranoid, vec![]).unwrap();

        let content = "Contact me at user@example.com with IP 192.168.1.1";

        let standard_result = standard_engine.redact(content).unwrap();
        let strict_result = strict_engine.redact(content).unwrap();
        let paranoid_result = paranoid_engine.redact(content).unwrap();

        // Standard: no email/IP redaction
        assert!(standard_result.content.contains("user@example.com"));
        assert!(standard_result.content.contains("192.168.1.1"));

        // Strict: IP redaction but no email
        assert!(strict_result.content.contains("user@example.com"));
        assert!(strict_result.content.contains("[REDACTED-IP]"));

        // Paranoid: both redacted
        assert!(paranoid_result.content.contains("[REDACTED-EMAIL]"));
        assert!(paranoid_result.content.contains("[REDACTED-IP]"));
    }

    #[test]
    fn test_dry_run_statistics() {
        let engine = RedactionEngine::new(RedactionLevel::Standard, vec![]).unwrap();
        let content = r#"
            API_KEY=myapikey_1234567890abcdefghij
            SECRET_KEY=secret_abcdefghijklmnopqrstuvwxyz
            GITHUB_TOKEN=ghp_1234567890abcdefghijklmnopqrstuv
        "#;
        let result = engine.redact(content).unwrap();

        // Verify statistics are accurate
        assert_eq!(result.secrets_redacted, 3);
        assert_eq!(result.secret_types.len(), 3);
        assert!(result.secret_types.contains(&"api_key".to_string()));
        assert!(result.secret_types.contains(&"secret_key".to_string()));
        assert!(result.secret_types.contains(&"github_token".to_string()));
    }
}
