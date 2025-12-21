//! Secret redaction engine for trajectory data
//!
//! This module provides open-source secret detection and redaction
//! for trajectory contributions. All redaction happens locally before
//! any data leaves the user's machine.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

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

impl RedactionLevel {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "strict" => Some(Self::Strict),
            "paranoid" => Some(Self::Paranoid),
            _ => None,
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
            let (new_content, count) = self.apply_pattern(&redacted, &pattern.regex, &pattern.replacement);
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
fn standard_patterns() -> Vec<RedactionPattern> {
    vec![
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
        RedactionPattern {
            name: "private-key".to_string(),
            regex: Regex::new(r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]+?-----END (RSA |EC |OPENSSH )?PRIVATE KEY-----").unwrap(),
            replacement: "[REDACTED-PRIVATE-KEY]",
        },
        RedactionPattern {
            name: "aws-key".to_string(),
            regex: Regex::new(r"(?i)(AKIA[0-9A-Z]{16})").unwrap(),
            replacement: "[REDACTED-AWS-KEY]",
        },
        RedactionPattern {
            name: "github_token".to_string(),
            regex: Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
            replacement: "[REDACTED-GITHUB-TOKEN]",
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
    ]
}

/// Strict patterns (more conservative)
fn strict_patterns() -> Vec<RedactionPattern> {
    vec![
        RedactionPattern {
            name: "hex_string".to_string(),
            regex: Regex::new(r"\b[a-fA-F0-9]{32,}\b").unwrap(),
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
        let content = "API_KEY=sk_test_1234567890abcdefghij";
        let result = engine.redact(content).unwrap();

        assert!(result.content.contains("[REDACTED-API-KEY]"));
        assert!(!result.content.contains("sk_test"));
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
}
