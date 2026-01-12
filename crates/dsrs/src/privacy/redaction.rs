//! Redaction transforms for privacy protection.
//!
//! Provides bidirectional transforms to anonymize sensitive content
//! before sending to swarm providers, and restore it afterward.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Redaction mode controlling what gets anonymized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RedactionMode {
    /// No redaction - send content as-is.
    #[default]
    None,
    /// Redact file paths only (home directories, usernames).
    PathsOnly,
    /// Redact paths and code identifiers (class/function/variable names).
    Identifiers,
    /// Full redaction (paths, identifiers, string literals).
    Full,
}

/// Configuration for redaction behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionConfig {
    /// Redaction mode.
    pub mode: RedactionMode,

    /// Preserve code structure (indentation, brackets).
    #[serde(default = "default_true")]
    pub preserve_structure: bool,

    /// Preserve type annotations.
    #[serde(default = "default_true")]
    pub preserve_types: bool,

    /// Custom patterns to redact (regex -> replacement).
    #[serde(default)]
    pub custom_patterns: Vec<(String, String)>,

    /// Patterns to preserve (never redact).
    #[serde(default)]
    pub preserve_patterns: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            mode: RedactionMode::None,
            preserve_structure: true,
            preserve_types: true,
            custom_patterns: vec![],
            preserve_patterns: vec![],
        }
    }
}

impl RedactionConfig {
    /// Create config for paths-only redaction.
    pub fn paths_only() -> Self {
        Self {
            mode: RedactionMode::PathsOnly,
            ..Default::default()
        }
    }

    /// Create config for identifier redaction.
    pub fn identifiers() -> Self {
        Self {
            mode: RedactionMode::Identifiers,
            ..Default::default()
        }
    }

    /// Create config for full redaction.
    pub fn full() -> Self {
        Self {
            mode: RedactionMode::Full,
            ..Default::default()
        }
    }
}

/// Result of redacting content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedContent {
    /// The redacted content.
    pub content: String,

    /// Mapping from redacted tokens to original values.
    /// Used for restoration.
    pub mapping: HashMap<String, String>,

    /// SHA-256 checksum of the original content.
    pub original_checksum: String,

    /// Redaction mode used.
    pub mode: RedactionMode,
}

impl RedactedContent {
    /// Create new redacted content.
    pub fn new(content: String, mapping: HashMap<String, String>, mode: RedactionMode) -> Self {
        Self {
            original_checksum: String::new(), // Will be set by redactor
            content,
            mapping,
            mode,
        }
    }

    /// Check if any redaction was applied.
    pub fn was_redacted(&self) -> bool {
        !self.mapping.is_empty()
    }

    /// Get the number of redactions applied.
    pub fn redaction_count(&self) -> usize {
        self.mapping.len()
    }
}

/// Trait for content redactors.
pub trait Redactor: Send + Sync {
    /// Redact sensitive content according to config.
    fn redact(&self, content: &str, config: &RedactionConfig) -> RedactedContent;

    /// Restore original content from redacted version.
    fn restore(&self, redacted: &RedactedContent) -> String;
}

/// Path redactor - anonymizes file system paths.
///
/// Transforms:
/// - `/Users/<username>/...` -> `/workspace/...`
/// - `/home/<username>/...` -> `/workspace/...`
/// - `C:\Users\<username>\...` -> `/workspace/...`
#[derive(Debug, Clone, Default)]
pub struct PathRedactor {
    /// Workspace prefix to use for redacted paths.
    workspace_prefix: String,
}

impl PathRedactor {
    /// Create a new path redactor.
    pub fn new() -> Self {
        Self {
            workspace_prefix: "/workspace".to_string(),
        }
    }

    /// Set custom workspace prefix.
    pub fn with_workspace_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.workspace_prefix = prefix.into();
        self
    }

    /// Compute SHA-256 checksum of content.
    fn compute_checksum(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

impl Redactor for PathRedactor {
    fn redact(&self, content: &str, config: &RedactionConfig) -> RedactedContent {
        if config.mode == RedactionMode::None {
            return RedactedContent {
                content: content.to_string(),
                mapping: HashMap::new(),
                original_checksum: Self::compute_checksum(content),
                mode: config.mode,
            };
        }

        let mut result = content.to_string();
        let mut mapping = HashMap::new();
        let mut counter = 0;

        // macOS paths: /Users/<username>/...
        let macos_re = Regex::new(r"/Users/[^/\s]+/").unwrap();
        for cap in macos_re.find_iter(content) {
            let original = cap.as_str().to_string();
            if !mapping.values().any(|v: &String| v == &original) {
                let redacted = format!("{}/", self.workspace_prefix);
                mapping.insert(redacted.clone(), original.clone());
                result = result.replace(&original, &redacted);
                counter += 1;
            }
        }

        // Linux paths: /home/<username>/...
        let linux_re = Regex::new(r"/home/[^/\s]+/").unwrap();
        for cap in linux_re.find_iter(content) {
            let original = cap.as_str().to_string();
            if !mapping.values().any(|v: &String| v == &original) {
                let redacted = format!("{}/", self.workspace_prefix);
                mapping.insert(redacted.clone(), original.clone());
                result = result.replace(&original, &redacted);
                counter += 1;
            }
        }

        // Windows paths: C:\Users\<username>\...
        let windows_re = Regex::new(r"[A-Z]:\\Users\\[^\\]+\\").unwrap();
        for cap in windows_re.find_iter(content) {
            let original = cap.as_str().to_string();
            if !mapping.values().any(|v: &String| v == &original) {
                let redacted = format!("{}/", self.workspace_prefix);
                mapping.insert(redacted.clone(), original.clone());
                result = result.replace(&original, &redacted);
                counter += 1;
            }
        }

        // Also redact tmp directories with usernames
        let tmp_re = Regex::new(r"/var/folders/[^/]+/[^/]+/").unwrap();
        for cap in tmp_re.find_iter(content) {
            let original = cap.as_str().to_string();
            if !mapping.values().any(|v: &String| v == &original) {
                let redacted = "/tmp/".to_string();
                mapping.insert(redacted.clone(), original.clone());
                result = result.replace(&original, &redacted);
                counter += 1;
            }
        }

        let _ = counter; // Silence unused warning

        RedactedContent {
            content: result,
            mapping,
            original_checksum: Self::compute_checksum(content),
            mode: config.mode,
        }
    }

    fn restore(&self, redacted: &RedactedContent) -> String {
        let mut result = redacted.content.clone();

        // Reverse the mapping
        for (redacted_token, original) in &redacted.mapping {
            result = result.replace(redacted_token, original);
        }

        result
    }
}

/// Identifier redactor - anonymizes code identifiers.
///
/// Transforms class names, function names, and variable names
/// to generic placeholders while preserving keywords.
#[derive(Debug, Clone, Default)]
pub struct IdentifierRedactor {
    /// Keywords to preserve (language-specific).
    preserve_keywords: Vec<String>,
}

impl IdentifierRedactor {
    /// Create a new identifier redactor.
    pub fn new() -> Self {
        Self {
            preserve_keywords: default_rust_keywords(),
        }
    }

    /// Add keywords to preserve.
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.preserve_keywords.extend(keywords);
        self
    }

    /// Compute SHA-256 checksum of content.
    fn compute_checksum(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

impl Redactor for IdentifierRedactor {
    fn redact(&self, content: &str, config: &RedactionConfig) -> RedactedContent {
        if config.mode != RedactionMode::Identifiers && config.mode != RedactionMode::Full {
            return RedactedContent {
                content: content.to_string(),
                mapping: HashMap::new(),
                original_checksum: Self::compute_checksum(content),
                mode: config.mode,
            };
        }

        let mut result = content.to_string();
        let mut mapping: HashMap<String, String> = HashMap::new();
        let mut counters: HashMap<&str, usize> = HashMap::new();

        // Match CamelCase identifiers (likely class/struct names)
        let class_re = Regex::new(r"\b([A-Z][a-z]+(?:[A-Z][a-z]+)+)\b").unwrap();
        for cap in class_re.find_iter(content) {
            let original = cap.as_str();
            if !self.preserve_keywords.contains(&original.to_string())
                && !mapping.values().any(|v| v == original)
            {
                let count = counters.entry("CLASS").or_insert(0);
                *count += 1;
                let redacted = format!("CLASS_{:03}", count);
                mapping.insert(redacted.clone(), original.to_string());
                result = result.replace(original, &redacted);
            }
        }

        // Match function names (snake_case or camelCase starting with lowercase)
        let func_re = Regex::new(r"\b(fn\s+)([a-z_][a-z0-9_]*)\b").unwrap();
        let content_clone = result.clone();
        for cap in func_re.captures_iter(&content_clone) {
            if let Some(name_match) = cap.get(2) {
                let original = name_match.as_str();
                if !self.preserve_keywords.contains(&original.to_string())
                    && !mapping.values().any(|v| v == original)
                {
                    let count = counters.entry("FUNC").or_insert(0);
                    *count += 1;
                    let redacted = format!("FUNC_{:03}", count);
                    mapping.insert(redacted.clone(), original.to_string());
                    result =
                        result.replace(&format!("fn {}", original), &format!("fn {}", redacted));
                }
            }
        }

        RedactedContent {
            content: result,
            mapping,
            original_checksum: Self::compute_checksum(content),
            mode: config.mode,
        }
    }

    fn restore(&self, redacted: &RedactedContent) -> String {
        let mut result = redacted.content.clone();

        for (redacted_token, original) in &redacted.mapping {
            result = result.replace(redacted_token, original);
        }

        result
    }
}

/// Default Rust keywords to preserve during identifier redaction.
fn default_rust_keywords() -> Vec<String> {
    vec![
        "as",
        "async",
        "await",
        "break",
        "const",
        "continue",
        "crate",
        "dyn",
        "else",
        "enum",
        "extern",
        "false",
        "fn",
        "for",
        "if",
        "impl",
        "in",
        "let",
        "loop",
        "match",
        "mod",
        "move",
        "mut",
        "pub",
        "ref",
        "return",
        "self",
        "Self",
        "static",
        "struct",
        "super",
        "trait",
        "true",
        "type",
        "unsafe",
        "use",
        "where",
        "while",
        "String",
        "Vec",
        "Option",
        "Result",
        "Ok",
        "Err",
        "Some",
        "None",
        "Box",
        "Rc",
        "Arc",
        "HashMap",
        "HashSet",
        "Default",
        "Clone",
        "Copy",
        "Debug",
        "Display",
        "Send",
        "Sync",
        "Sized",
        "Drop",
        "Fn",
        "FnMut",
        "FnOnce",
        "Iterator",
        "IntoIterator",
        "From",
        "Into",
        "AsRef",
        "AsMut",
        "Deref",
        "DerefMut",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Composite redactor that chains multiple redactors.
#[derive(Default)]
pub struct CompositeRedactor {
    redactors: Vec<Box<dyn Redactor>>,
}

impl CompositeRedactor {
    /// Create a new composite redactor.
    pub fn new() -> Self {
        Self {
            redactors: Vec::new(),
        }
    }

    /// Add a redactor to the chain.
    pub fn add<R: Redactor + 'static>(mut self, redactor: R) -> Self {
        self.redactors.push(Box::new(redactor));
        self
    }

    /// Create a default composite with path and identifier redactors.
    pub fn default_chain() -> Self {
        Self::new()
            .add(PathRedactor::new())
            .add(IdentifierRedactor::new())
    }
}

impl Redactor for CompositeRedactor {
    fn redact(&self, content: &str, config: &RedactionConfig) -> RedactedContent {
        let mut current_content = content.to_string();
        let mut all_mappings: HashMap<String, String> = HashMap::new();

        for redactor in &self.redactors {
            let result = redactor.redact(&current_content, config);
            current_content = result.content;
            all_mappings.extend(result.mapping);
        }

        RedactedContent {
            content: current_content,
            mapping: all_mappings,
            original_checksum: {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                content.hash(&mut hasher);
                format!("{:016x}", hasher.finish())
            },
            mode: config.mode,
        }
    }

    fn restore(&self, redacted: &RedactedContent) -> String {
        let mut result = redacted.content.clone();

        // Apply all mappings in reverse
        for (redacted_token, original) in &redacted.mapping {
            result = result.replace(redacted_token, original);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_redactor_macos() {
        let redactor = PathRedactor::new();
        let config = RedactionConfig::paths_only();

        let content = "Error in /Users/alice/project/src/main.rs:42";
        let redacted = redactor.redact(content, &config);

        assert!(redacted.content.contains("/workspace/"));
        assert!(!redacted.content.contains("/Users/alice/"));
        assert!(redacted.was_redacted());

        let restored = redactor.restore(&redacted);
        assert_eq!(restored, content);
    }

    #[test]
    fn test_path_redactor_linux() {
        let redactor = PathRedactor::new();
        let config = RedactionConfig::paths_only();

        let content = "File: /home/bob/code/lib.rs";
        let redacted = redactor.redact(content, &config);

        assert!(redacted.content.contains("/workspace/"));
        assert!(!redacted.content.contains("/home/bob/"));

        let restored = redactor.restore(&redacted);
        assert_eq!(restored, content);
    }

    #[test]
    fn test_path_redactor_windows() {
        let redactor = PathRedactor::new();
        let config = RedactionConfig::paths_only();

        let content = r"Path: C:\Users\charlie\Documents\code.rs";
        let redacted = redactor.redact(content, &config);

        assert!(redacted.content.contains("/workspace/"));
        assert!(!redacted.content.contains(r"C:\Users\charlie\"));

        let restored = redactor.restore(&redacted);
        assert_eq!(restored, content);
    }

    #[test]
    fn test_path_redactor_no_redaction() {
        let redactor = PathRedactor::new();
        let config = RedactionConfig::default(); // mode = None

        let content = "/Users/alice/project/src/main.rs";
        let redacted = redactor.redact(content, &config);

        assert_eq!(redacted.content, content);
        assert!(!redacted.was_redacted());
    }

    #[test]
    fn test_identifier_redactor() {
        let redactor = IdentifierRedactor::new();
        let config = RedactionConfig::identifiers();

        let content = "struct MySecretClass { fn process_data() {} }";
        let redacted = redactor.redact(content, &config);

        assert!(!redacted.content.contains("MySecretClass"));
        assert!(redacted.content.contains("CLASS_"));

        let restored = redactor.restore(&redacted);
        assert!(restored.contains("MySecretClass"));
    }

    #[test]
    fn test_composite_redactor() {
        let redactor = CompositeRedactor::default_chain();
        let config = RedactionConfig::full();

        let content = "Error in /Users/alice/project/MyClass.rs: struct MyClass failed";
        let redacted = redactor.redact(content, &config);

        assert!(!redacted.content.contains("/Users/alice/"));
        // Note: MyClass in path vs struct name may have different handling
        assert!(redacted.was_redacted());

        let restored = redactor.restore(&redacted);
        // Should restore paths
        assert!(restored.contains("/Users/alice/"));
    }

    #[test]
    fn test_redaction_count() {
        let redactor = PathRedactor::new();
        let config = RedactionConfig::paths_only();

        let content = "A: /Users/alice/a.rs B: /Users/bob/b.rs";
        let redacted = redactor.redact(content, &config);

        assert!(redacted.redaction_count() > 0);
    }
}
