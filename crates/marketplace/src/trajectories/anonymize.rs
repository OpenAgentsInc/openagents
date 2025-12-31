//! PII anonymization for trajectory data
//!
//! This module removes personally identifiable information from
//! trajectory data before contribution to the marketplace.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Result of anonymization operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymizationResult {
    /// Number of usernames anonymized
    pub usernames_anonymized: usize,

    /// Number of file paths anonymized
    pub paths_anonymized: usize,

    /// Number of emails anonymized
    pub emails_anonymized: usize,

    /// Anonymized content
    pub content: String,
}

/// PII anonymizer for trajectory data
pub struct Anonymizer {
    /// Redact file paths
    redact_paths: bool,

    /// Redact usernames
    redact_usernames: bool,

    /// Keep repository names (useful context for training)
    _keep_repo_names: bool,

    /// Path replacement cache
    _path_cache: HashMap<String, String>,

    /// Username replacement cache
    _username_cache: HashMap<String, String>,
}

impl Anonymizer {
    /// Create a new anonymizer with configuration
    pub fn new(redact_paths: bool, redact_usernames: bool, keep_repo_names: bool) -> Self {
        Self {
            redact_paths,
            redact_usernames,
            _keep_repo_names: keep_repo_names,
            _path_cache: HashMap::new(),
            _username_cache: HashMap::new(),
        }
    }

    /// Anonymize content
    pub fn anonymize(&mut self, content: &str) -> Result<AnonymizationResult> {
        let mut anonymized = content.to_string();
        let mut usernames_anonymized = 0;
        let mut paths_anonymized = 0;
        let mut emails_anonymized = 0;

        // Anonymize emails
        if self.redact_usernames {
            let (new_content, count) = self.anonymize_emails(&anonymized);
            anonymized = new_content;
            emails_anonymized = count;
        }

        // Anonymize file paths
        if self.redact_paths {
            let (new_content, count) = self.anonymize_paths(&anonymized);
            anonymized = new_content;
            paths_anonymized = count;
        }

        // Anonymize usernames in paths
        if self.redact_usernames {
            let (new_content, count) = self.anonymize_usernames(&anonymized);
            anonymized = new_content;
            usernames_anonymized = count;
        }

        Ok(AnonymizationResult {
            usernames_anonymized,
            paths_anonymized,
            emails_anonymized,
            content: anonymized,
        })
    }

    /// Anonymize email addresses
    fn anonymize_emails(&self, content: &str) -> (String, usize) {
        let email_regex =
            Regex::new(r"\b[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}\b").unwrap();
        let mut count = 0;
        let result = email_regex.replace_all(content, |_: &regex::Captures| {
            count += 1;
            "user@example.com"
        });
        (result.to_string(), count)
    }

    /// Anonymize file paths
    fn anonymize_paths(&mut self, content: &str) -> (String, usize) {
        let mut result = content.to_string();
        let mut count = 0;

        // Detect common home directory patterns
        if let Ok(home) = std::env::var("HOME") {
            let home_patterns = vec![home.clone(), format!("~")];

            for pattern in home_patterns {
                if result.contains(&pattern) && pattern.len() > 1 {
                    // Replace absolute home paths with relative
                    result = result.replace(&home, "~");
                    count += 1;
                }
            }
        }

        // Replace absolute paths with relative where possible
        // Match patterns like /home/username/... or /Users/username/...
        let abs_path_regex = Regex::new(r"(/home/[^/\s]+|/Users/[^/\s]+)(/[^\s]*)").unwrap();
        result = abs_path_regex
            .replace_all(&result, |caps: &regex::Captures| {
                count += 1;
                if let Some(relative) = caps.get(2) {
                    format!(".{}", relative.as_str())
                } else {
                    "./...".to_string()
                }
            })
            .to_string();

        (result, count)
    }

    /// Anonymize usernames in content
    fn anonymize_usernames(&mut self, content: &str) -> (String, usize) {
        let mut result = content.to_string();
        let mut count = 0;

        // Try to detect username from common patterns
        if let Ok(user) = std::env::var("USER") {
            if !user.is_empty() && result.contains(&user) {
                result = result.replace(&user, "user");
                count += 1;
            }
        }

        // Also check LOGNAME
        if let Ok(logname) = std::env::var("LOGNAME") {
            if !logname.is_empty() && result.contains(&logname) {
                result = result.replace(&logname, "user");
                count += 1;
            }
        }

        (result, count)
    }

    /// Extract repository name from a file path (if keep_repo_names is true)
    #[allow(dead_code)]
    fn extract_repo_name(&self, path: &Path) -> Option<String> {
        if !self._keep_repo_names {
            return None;
        }

        // Look for common repo directory structures
        // e.g., /home/user/code/myrepo/src/main.rs -> keep "myrepo"
        let components: Vec<_> = path.components().collect();

        for (i, component) in components.iter().enumerate() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str()?;
                // Common parent directories for repos
                if name_str == "code"
                    || name_str == "projects"
                    || name_str == "src"
                    || name_str == "workspace"
                {
                    // Next component might be the repo name
                    if let Some(std::path::Component::Normal(repo)) = components.get(i + 1) {
                        return repo.to_str().map(|s| s.to_string());
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_anonymization() {
        let mut anonymizer = Anonymizer::new(false, true, false);
        let content = "Contact me at john.doe@example.com for info";
        let result = anonymizer.anonymize(content).unwrap();

        assert!(result.content.contains("user@example.com"));
        assert!(!result.content.contains("john.doe"));
        assert_eq!(result.emails_anonymized, 1);
    }

    #[test]
    fn test_path_anonymization() {
        let mut anonymizer = Anonymizer::new(true, false, false);
        let content = "/home/johndoe/code/project/src/main.rs";
        let result = anonymizer.anonymize(content).unwrap();

        assert!(!result.content.contains("/home/johndoe"));
        assert!(result.content.contains("./code") || result.content.contains("./"));
    }

    #[test]
    fn test_username_anonymization() {
        let mut anonymizer = Anonymizer::new(false, true, false);
        // This test depends on the actual USER env var
        // Just verify it doesn't crash
        let content = "User johndoe did something";
        let _result = anonymizer.anonymize(content).unwrap();
    }

    #[test]
    fn test_keep_repo_names() {
        let anonymizer = Anonymizer::new(true, true, true);
        let path = Path::new("/home/user/code/myrepo/src/main.rs");
        let repo = anonymizer.extract_repo_name(path);

        assert_eq!(repo, Some("myrepo".to_string()));
    }

    #[test]
    fn test_combined_anonymization() {
        let mut anonymizer = Anonymizer::new(true, true, false);
        let content = "File: /home/john/code/project/main.rs\nEmail: john@example.com";
        let result = anonymizer.anonymize(content).unwrap();

        assert!(!result.content.contains("/home/john"));
        assert!(result.content.contains("user@example.com"));
    }
}
