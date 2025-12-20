//! Secret redaction for trajectory logs
//!
//! Redacts sensitive information like API keys, tokens, and passwords
//! before saving to rlog files.

use once_cell::sync::Lazy;
use regex::Regex;

/// Redaction placeholder
const REDACTED: &str = "[REDACTED]";

/// Pattern definitions for secret detection
struct SecretPattern {
    /// Name of the secret type (for debugging)
    #[allow(dead_code)]
    name: &'static str,
    pattern: Regex,
}

/// All secret patterns to check
static SECRET_PATTERNS: Lazy<Vec<SecretPattern>> = Lazy::new(|| {
    vec![
        // API Keys with prefixes
        SecretPattern {
            name: "anthropic_api_key",
            pattern: Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
        },
        SecretPattern {
            name: "openai_api_key",
            pattern: Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(),
        },
        SecretPattern {
            name: "github_token",
            pattern: Regex::new(r"gh[pousr]_[a-zA-Z0-9]{36,}").unwrap(),
        },
        SecretPattern {
            name: "github_classic_token",
            pattern: Regex::new(r"ghp_[a-zA-Z0-9]{36}").unwrap(),
        },
        SecretPattern {
            name: "aws_access_key",
            pattern: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
        },
        SecretPattern {
            name: "aws_secret_key",
            pattern: Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key['"]?\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})"#).unwrap(),
        },
        SecretPattern {
            name: "stripe_api_key",
            pattern: Regex::new(r"sk_live_[a-zA-Z0-9]{24,}").unwrap(),
        },
        SecretPattern {
            name: "stripe_test_key",
            pattern: Regex::new(r"sk_test_[a-zA-Z0-9]{24,}").unwrap(),
        },
        SecretPattern {
            name: "slack_token",
            pattern: Regex::new(r"xox[baprs]-[0-9]+-[0-9]+-[a-zA-Z0-9]+").unwrap(),
        },
        SecretPattern {
            name: "slack_webhook",
            pattern: Regex::new(r"https://hooks\.slack\.com/services/T[a-zA-Z0-9_]+/B[a-zA-Z0-9_]+/[a-zA-Z0-9_]+").unwrap(),
        },
        SecretPattern {
            name: "discord_token",
            pattern: Regex::new(r"[MN][A-Za-z\d]{23,}\.[\w-]{6}\.[\w-]{27}").unwrap(),
        },
        SecretPattern {
            name: "discord_webhook",
            pattern: Regex::new(r"https://discord(?:app)?\.com/api/webhooks/\d+/[A-Za-z0-9_-]+").unwrap(),
        },
        SecretPattern {
            name: "google_api_key",
            pattern: Regex::new(r"AIza[0-9A-Za-z_-]{35}").unwrap(),
        },
        SecretPattern {
            name: "heroku_api_key",
            pattern: Regex::new(r#"(?i)heroku[_-]?api[_-]?key['"]?\s*[:=]\s*['"]?([a-f0-9-]{36})"#).unwrap(),
        },
        SecretPattern {
            name: "twilio_api_key",
            pattern: Regex::new(r"SK[a-f0-9]{32}").unwrap(),
        },
        SecretPattern {
            name: "sendgrid_api_key",
            pattern: Regex::new(r"SG\.[a-zA-Z0-9_-]{22}\.[a-zA-Z0-9_-]{43}").unwrap(),
        },
        SecretPattern {
            name: "mailgun_api_key",
            pattern: Regex::new(r"key-[a-zA-Z0-9]{32}").unwrap(),
        },
        SecretPattern {
            name: "npm_token",
            pattern: Regex::new(r"npm_[a-zA-Z0-9]{36}").unwrap(),
        },
        SecretPattern {
            name: "pypi_token",
            pattern: Regex::new(r"pypi-[a-zA-Z0-9_-]{100,}").unwrap(),
        },
        SecretPattern {
            name: "jwt_token",
            pattern: Regex::new(r"eyJ[a-zA-Z0-9_-]+\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+").unwrap(),
        },
        SecretPattern {
            name: "private_key_header",
            pattern: Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").unwrap(),
        },
        SecretPattern {
            name: "base64_secret",
            pattern: Regex::new(r#"(?i)(secret|password|token|apikey|api_key|auth)['"]?\s*[:=]\s*['"]?([A-Za-z0-9+/]{40,}={0,2})"#).unwrap(),
        },
        // Generic patterns (more likely to have false positives, so be careful)
        SecretPattern {
            name: "password_assignment",
            pattern: Regex::new(r#"(?i)(password|passwd|pwd)['"]?\s*[:=]\s*['"]([^'"]{8,})['"]"#).unwrap(),
        },
        SecretPattern {
            name: "bearer_token",
            pattern: Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-.]+").unwrap(),
        },
        SecretPattern {
            name: "basic_auth",
            pattern: Regex::new(r"(?i)basic\s+[a-zA-Z0-9+/=]{20,}").unwrap(),
        },
        // Database connection strings
        SecretPattern {
            name: "postgres_url",
            pattern: Regex::new(r"postgres(?:ql)?://[^:]+:[^@]+@[^\s]+").unwrap(),
        },
        SecretPattern {
            name: "mysql_url",
            pattern: Regex::new(r"mysql://[^:]+:[^@]+@[^\s]+").unwrap(),
        },
        SecretPattern {
            name: "mongodb_url",
            pattern: Regex::new(r"mongodb(?:\+srv)?://[^:]+:[^@]+@[^\s]+").unwrap(),
        },
        SecretPattern {
            name: "redis_url",
            pattern: Regex::new(r"redis://:[^@]+@[^\s]+").unwrap(),
        },
        // SSH keys
        SecretPattern {
            name: "ssh_private_key",
            pattern: Regex::new(r"-----BEGIN (?:RSA|DSA|EC|OPENSSH) PRIVATE KEY-----").unwrap(),
        },
    ]
});

/// Redact secrets from text
pub fn redact_secrets(text: &str) -> String {
    let mut result = text.to_string();

    for pattern in SECRET_PATTERNS.iter() {
        result = pattern.pattern.replace_all(&result, REDACTED).to_string();
    }

    result
}

/// Redact secrets from a JSON value (for tool inputs)
pub fn redact_json_value(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match value {
        Value::String(s) => Value::String(redact_secrets(s)),
        Value::Array(arr) => Value::Array(arr.iter().map(redact_json_value).collect()),
        Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (k, v) in obj {
                // Always redact values for sensitive keys
                let new_value = if is_sensitive_key(k) {
                    match v {
                        Value::String(_) => Value::String(REDACTED.to_string()),
                        Value::Number(_) => Value::String(REDACTED.to_string()),
                        _ => redact_json_value(v),
                    }
                } else {
                    redact_json_value(v)
                };
                new_obj.insert(k.clone(), new_value);
            }
            Value::Object(new_obj)
        }
        other => other.clone(),
    }
}

/// Check if a key name suggests it contains sensitive data
fn is_sensitive_key(key: &str) -> bool {
    let key_lower = key.to_lowercase();
    let sensitive_patterns = [
        "password",
        "passwd",
        "pwd",
        "secret",
        "token",
        "apikey",
        "api_key",
        "api-key",
        "auth",
        "credential",
        "private_key",
        "private-key",
        "privatekey",
        "access_key",
        "access-key",
        "accesskey",
        "secret_key",
        "secret-key",
        "secretkey",
    ];

    sensitive_patterns.iter().any(|p| key_lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_anthropic_api_key() {
        let text = "My API key is sk-ant-api03-abcdefghijklmnopqrstuvwxyz123456";
        let redacted = redact_secrets(text);
        assert_eq!(redacted, "My API key is [REDACTED]");
    }

    #[test]
    fn test_openai_api_key() {
        let text = "export OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz123456";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("sk-abcdefghij"));
    }

    #[test]
    fn test_github_token() {
        let text = "GITHUB_TOKEN=ghp_abcdefghijklmnopqrstuvwxyz1234567890";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("ghp_"));
    }

    #[test]
    fn test_aws_access_key() {
        let text = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_jwt_token() {
        let text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let redacted = redact_secrets(text);
        // Should redact both bearer and JWT
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("eyJ"));
    }

    #[test]
    fn test_postgres_url() {
        let text = "DATABASE_URL=postgres://user:password123@localhost:5432/mydb";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("password123"));
    }

    #[test]
    fn test_private_key() {
        let text = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpA...";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_password_assignment() {
        let text = r#"password = "super_secret_pass""#;
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("super_secret_pass"));
    }

    #[test]
    fn test_no_false_positives() {
        let text = "This is a normal text without any secrets.";
        let redacted = redact_secrets(text);
        assert_eq!(text, redacted);
    }

    #[test]
    fn test_json_value_redaction() {
        let value = json!({
            "name": "test",
            "api_key": "sk-ant-api03-secret123456789012345678901234",
            "password": "mysecretpassword",
            "data": {
                "token": "should_be_redacted",
                "normal": "keep this"
            }
        });

        let redacted = redact_json_value(&value);

        assert_eq!(redacted["name"], "test");
        assert_eq!(redacted["api_key"], "[REDACTED]");
        assert_eq!(redacted["password"], "[REDACTED]");
        assert_eq!(redacted["data"]["token"], "[REDACTED]");
        assert_eq!(redacted["data"]["normal"], "keep this");
    }

    #[test]
    fn test_sensitive_key_detection() {
        assert!(is_sensitive_key("password"));
        assert!(is_sensitive_key("api_key"));
        assert!(is_sensitive_key("API_KEY"));
        assert!(is_sensitive_key("secretToken"));
        assert!(is_sensitive_key("auth_token"));

        assert!(!is_sensitive_key("name"));
        assert!(!is_sensitive_key("file_path"));
        assert!(!is_sensitive_key("content"));
    }

    #[test]
    fn test_stripe_keys() {
        // Build test patterns dynamically to avoid triggering secret scanners
        let prefix = "sk_live_";
        let suffix = "X".repeat(24);
        let text = format!("STRIPE_KEY={}{}", prefix, suffix);
        let redacted = redact_secrets(&text);
        assert!(redacted.contains("[REDACTED]"));

        let prefix2 = "sk_test_";
        let text2 = format!("STRIPE_TEST={}{}", prefix2, suffix);
        let redacted2 = redact_secrets(&text2);
        assert!(redacted2.contains("[REDACTED]"));
    }

    #[test]
    fn test_slack_token() {
        // Build test pattern dynamically to avoid triggering secret scanners
        let parts = ["xoxb", "111111111111", "2222222222222", "ZZZZZZZZZZZZZZZZ"];
        let text = format!("SLACK_TOKEN={}", parts.join("-"));
        let redacted = redact_secrets(&text);
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_sendgrid_api_key() {
        // SendGrid format: SG.<22chars>.<43chars>
        let text = "SENDGRID_API_KEY=SG.abcdefghijklmnopqrstuv.abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGH";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
    }
}
