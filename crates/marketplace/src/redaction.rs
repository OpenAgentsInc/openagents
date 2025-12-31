//! Redaction engine interface for removing secrets and PII from session data
//!
//! Provides types and traits for detecting and redacting sensitive information
//! before contributing session data to the marketplace.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during redaction
#[derive(Debug, Error)]
pub enum RedactionError {
    #[error("Redaction failed: {0}")]
    RedactionFailed(String),

    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("Hash computation failed: {0}")]
    HashError(String),

    #[error("Unsafe content detected: {0}")]
    UnsafeContent(String),
}

/// Types of data that can be redacted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionType {
    /// API keys and tokens
    ApiKey,

    /// Passwords and secrets
    Password,

    /// Private cryptographic keys
    PrivateKey,

    /// Email addresses
    Email,

    /// Phone numbers
    PhoneNumber,

    /// Credit card numbers
    CreditCard,

    /// Social security numbers
    SocialSecurity,

    /// IP addresses
    IpAddress,

    /// File paths (potentially revealing structure)
    FilePath,

    /// Custom pattern-based redaction
    CustomPattern,
}

impl RedactionType {
    /// Get a human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::ApiKey => "API Key",
            Self::Password => "Password",
            Self::PrivateKey => "Private Key",
            Self::Email => "Email Address",
            Self::PhoneNumber => "Phone Number",
            Self::CreditCard => "Credit Card",
            Self::SocialSecurity => "Social Security Number",
            Self::IpAddress => "IP Address",
            Self::FilePath => "File Path",
            Self::CustomPattern => "Custom Pattern",
        }
    }

    /// Get the default placeholder for this type
    pub fn default_placeholder(&self) -> &'static str {
        match self {
            Self::ApiKey => "[REDACTED_API_KEY]",
            Self::Password => "[REDACTED_PASSWORD]",
            Self::PrivateKey => "[REDACTED_PRIVATE_KEY]",
            Self::Email => "[REDACTED_EMAIL]",
            Self::PhoneNumber => "[REDACTED_PHONE]",
            Self::CreditCard => "[REDACTED_CC]",
            Self::SocialSecurity => "[REDACTED_SSN]",
            Self::IpAddress => "[REDACTED_IP]",
            Self::FilePath => "[REDACTED_PATH]",
            Self::CustomPattern => "[REDACTED]",
        }
    }

    /// Check if this type should always block contribution
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::ApiKey
                | Self::Password
                | Self::PrivateKey
                | Self::CreditCard
                | Self::SocialSecurity
        )
    }
}

/// Record of a single redaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRecord {
    /// Type of redaction applied
    pub redaction_type: RedactionType,

    /// Number of instances redacted
    pub count: u32,

    /// Placeholder used for replacement
    pub placeholder: String,

    /// Optional context about the redaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl RedactionRecord {
    /// Create a new redaction record
    pub fn new(redaction_type: RedactionType, count: u32) -> Self {
        Self {
            redaction_type,
            count,
            placeholder: redaction_type.default_placeholder().to_string(),
            context: None,
        }
    }

    /// Create with a custom placeholder
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Add context information
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// Result of a redaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionResult {
    /// Hash of the original content
    pub original_hash: String,

    /// Hash of the redacted content
    pub redacted_hash: String,

    /// List of all redactions applied
    pub redactions_applied: Vec<RedactionRecord>,

    /// Whether the content is safe to contribute
    pub safe_for_contribution: bool,

    /// Warnings or notes about the redaction
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl RedactionResult {
    /// Create a new redaction result
    pub fn new(original_hash: impl Into<String>, redacted_hash: impl Into<String>) -> Self {
        Self {
            original_hash: original_hash.into(),
            redacted_hash: redacted_hash.into(),
            redactions_applied: Vec::new(),
            safe_for_contribution: true,
            warnings: Vec::new(),
        }
    }

    /// Add a redaction record
    pub fn add_redaction(&mut self, record: RedactionRecord) {
        // Mark as unsafe if critical secrets were found
        if record.redaction_type.is_critical() && record.count > 0 {
            self.safe_for_contribution = false;
        }
        self.redactions_applied.push(record);
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Get total number of redactions
    pub fn total_redactions(&self) -> u32 {
        self.redactions_applied.iter().map(|r| r.count).sum()
    }

    /// Check if any critical secrets were found
    pub fn has_critical_secrets(&self) -> bool {
        self.redactions_applied
            .iter()
            .any(|r| r.redaction_type.is_critical() && r.count > 0)
    }
}

/// Text span indicating location of detected secret
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSpan {
    /// Start position (character offset)
    pub start: usize,

    /// End position (character offset)
    pub end: usize,

    /// Line number (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,

    /// Column number (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

impl TextSpan {
    /// Create a new text span
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            line: None,
            column: None,
        }
    }

    /// Add line and column information
    pub fn with_position(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Get the length of the span
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Check if the span is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Detection of a potential secret in content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretDetection {
    /// Type of secret detected
    pub detection_type: RedactionType,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Location in the text
    pub location: TextSpan,

    /// Optional preview (first/last few chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

impl SecretDetection {
    /// Create a new secret detection
    pub fn new(detection_type: RedactionType, confidence: f32, location: TextSpan) -> Self {
        Self {
            detection_type,
            confidence,
            location,
            preview: None,
        }
    }

    /// Add a preview
    pub fn with_preview(mut self, preview: impl Into<String>) -> Self {
        self.preview = Some(preview.into());
        self
    }

    /// Check if this is a high-confidence detection
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }
}

/// Custom redaction pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRedactionPattern {
    /// Pattern name/identifier
    pub name: String,

    /// Regex pattern to match
    pub pattern: String,

    /// Placeholder to use for replacements
    pub placeholder: String,

    /// Whether this pattern is case-sensitive
    #[serde(default = "default_true")]
    pub case_sensitive: bool,
}

fn default_true() -> bool {
    true
}

impl CustomRedactionPattern {
    /// Create a new custom pattern
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        placeholder: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            placeholder: placeholder.into(),
            case_sensitive: true,
        }
    }

    /// Make the pattern case-insensitive
    pub fn case_insensitive(mut self) -> Self {
        self.case_sensitive = false;
        self
    }
}

/// User preferences for redaction behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionPreferences {
    /// Automatically redact detected secrets
    #[serde(default = "default_true")]
    pub auto_redact: bool,

    /// Additional custom patterns to apply
    #[serde(default)]
    pub additional_patterns: Vec<String>,

    /// Require manual review before submitting
    #[serde(default = "default_true")]
    pub review_before_submit: bool,

    /// Redaction types that should block contribution entirely
    #[serde(default)]
    pub block_on_detection: Vec<RedactionType>,

    /// Minimum confidence threshold for detection (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
}

fn default_confidence() -> f32 {
    0.7
}

impl Default for RedactionPreferences {
    fn default() -> Self {
        Self {
            auto_redact: true,
            additional_patterns: Vec::new(),
            review_before_submit: true,
            block_on_detection: vec![
                RedactionType::ApiKey,
                RedactionType::Password,
                RedactionType::PrivateKey,
                RedactionType::CreditCard,
                RedactionType::SocialSecurity,
            ],
            min_confidence: 0.7,
        }
    }
}

impl RedactionPreferences {
    /// Create new preferences with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a custom pattern
    pub fn add_pattern(&mut self, pattern: impl Into<String>) {
        self.additional_patterns.push(pattern.into());
    }

    /// Add a blocking redaction type
    pub fn add_blocking_type(&mut self, redaction_type: RedactionType) {
        if !self.block_on_detection.contains(&redaction_type) {
            self.block_on_detection.push(redaction_type);
        }
    }

    /// Check if a detection should block contribution
    pub fn should_block(&self, detection: &SecretDetection) -> bool {
        detection.confidence >= self.min_confidence
            && self.block_on_detection.contains(&detection.detection_type)
    }
}

/// Trait for implementing redaction engines
pub trait RedactionEngine {
    /// Redact secrets and PII from content
    fn redact(&self, content: &str) -> Result<RedactionResult, RedactionError>;

    /// Detect secrets without redacting
    fn detect_secrets(&self, content: &str) -> Result<Vec<SecretDetection>, RedactionError>;

    /// Add a custom redaction pattern
    fn add_pattern(&mut self, pattern: CustomRedactionPattern) -> Result<(), RedactionError>;

    /// Get the current preferences
    fn preferences(&self) -> &RedactionPreferences;

    /// Update preferences
    fn set_preferences(&mut self, preferences: RedactionPreferences);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction_type_properties() {
        assert_eq!(RedactionType::ApiKey.name(), "API Key");
        assert_eq!(
            RedactionType::ApiKey.default_placeholder(),
            "[REDACTED_API_KEY]"
        );
        assert!(RedactionType::ApiKey.is_critical());
        assert!(!RedactionType::Email.is_critical());
    }

    #[test]
    fn test_redaction_record() {
        let record = RedactionRecord::new(RedactionType::Password, 3)
            .with_placeholder("[PASSWORD]")
            .with_context("Found in config file");

        assert_eq!(record.redaction_type, RedactionType::Password);
        assert_eq!(record.count, 3);
        assert_eq!(record.placeholder, "[PASSWORD]");
        assert_eq!(record.context.as_deref(), Some("Found in config file"));
    }

    #[test]
    fn test_redaction_result() {
        let mut result = RedactionResult::new("hash1", "hash2");

        assert!(result.safe_for_contribution);
        assert_eq!(result.total_redactions(), 0);
        assert!(!result.has_critical_secrets());

        result.add_redaction(RedactionRecord::new(RedactionType::Email, 2));
        assert_eq!(result.total_redactions(), 2);
        assert!(result.safe_for_contribution); // Email is not critical

        result.add_redaction(RedactionRecord::new(RedactionType::ApiKey, 1));
        assert_eq!(result.total_redactions(), 3);
        assert!(!result.safe_for_contribution); // API key is critical
        assert!(result.has_critical_secrets());
    }

    #[test]
    fn test_redaction_result_warnings() {
        let mut result = RedactionResult::new("hash1", "hash2");
        result.add_warning("Potential secret detected");
        result.add_warning("Review recommended");

        assert_eq!(result.warnings.len(), 2);
    }

    #[test]
    fn test_text_span() {
        let span = TextSpan::new(10, 25).with_position(3, 5);

        assert_eq!(span.start, 10);
        assert_eq!(span.end, 25);
        assert_eq!(span.len(), 15);
        assert!(!span.is_empty());
        assert_eq!(span.line, Some(3));
        assert_eq!(span.column, Some(5));

        let empty_span = TextSpan::new(10, 10);
        assert!(empty_span.is_empty());
    }

    #[test]
    fn test_secret_detection() {
        let span = TextSpan::new(0, 10);
        let detection =
            SecretDetection::new(RedactionType::ApiKey, 0.95, span).with_preview("sk_test_...");

        assert_eq!(detection.detection_type, RedactionType::ApiKey);
        assert_eq!(detection.confidence, 0.95);
        assert!(detection.is_high_confidence());
        assert_eq!(detection.preview.as_deref(), Some("sk_test_..."));

        let low_confidence = SecretDetection::new(RedactionType::Email, 0.6, TextSpan::new(0, 5));
        assert!(!low_confidence.is_high_confidence());
    }

    #[test]
    fn test_custom_redaction_pattern() {
        let pattern = CustomRedactionPattern::new("internal-id", r"INT-\d{6}", "[INTERNAL_ID]");

        assert_eq!(pattern.name, "internal-id");
        assert!(pattern.case_sensitive);

        let case_insensitive = pattern.clone().case_insensitive();
        assert!(!case_insensitive.case_sensitive);
    }

    #[test]
    fn test_redaction_preferences_default() {
        let prefs = RedactionPreferences::default();

        assert!(prefs.auto_redact);
        assert!(prefs.review_before_submit);
        assert_eq!(prefs.min_confidence, 0.7);
        assert_eq!(prefs.block_on_detection.len(), 5);
        assert!(prefs.block_on_detection.contains(&RedactionType::ApiKey));
    }

    #[test]
    fn test_redaction_preferences_blocking() {
        let mut prefs = RedactionPreferences::new();
        prefs.add_blocking_type(RedactionType::Email);

        let api_key_detection =
            SecretDetection::new(RedactionType::ApiKey, 0.9, TextSpan::new(0, 10));
        assert!(prefs.should_block(&api_key_detection));

        let email_detection = SecretDetection::new(RedactionType::Email, 0.8, TextSpan::new(0, 20));
        assert!(prefs.should_block(&email_detection));

        let low_confidence = SecretDetection::new(RedactionType::ApiKey, 0.5, TextSpan::new(0, 10));
        assert!(!prefs.should_block(&low_confidence)); // Below threshold
    }

    #[test]
    fn test_redaction_preferences_patterns() {
        let mut prefs = RedactionPreferences::new();
        prefs.add_pattern(r"CUSTOM-\d+");
        prefs.add_pattern(r"SECRET_\w+");

        assert_eq!(prefs.additional_patterns.len(), 2);
    }

    #[test]
    fn test_redaction_result_serde() {
        let result = RedactionResult::new("abc123", "def456");
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RedactionResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.original_hash, deserialized.original_hash);
        assert_eq!(result.redacted_hash, deserialized.redacted_hash);
    }

    #[test]
    fn test_secret_detection_serde() {
        let detection = SecretDetection::new(RedactionType::Password, 0.85, TextSpan::new(5, 15));
        let json = serde_json::to_string(&detection).unwrap();
        let deserialized: SecretDetection = serde_json::from_str(&json).unwrap();

        assert_eq!(detection.detection_type, deserialized.detection_type);
        assert_eq!(detection.confidence, deserialized.confidence);
    }
}
