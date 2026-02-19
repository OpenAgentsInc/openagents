//! NIP-78: Application-specific Data
//!
//! This NIP defines kind 30078 for storing arbitrary application-specific data.
//! Apps can use Nostr relays as a personal database without needing interoperability.
//!
//! ## Features
//!
//! - Addressable events (kind 30078) for app-specific storage
//! - Flexible content format (any string or JSON)
//! - Arbitrary tags for custom metadata
//! - D tag for app name and context identification
//!
//! ## Use Cases
//!
//! - User settings for Nostr clients
//! - Dynamic parameters for apps
//! - Private data storage for non-Nostr apps
//! - RemoteStorage-like capabilities
//!
//! ## Examples
//!
//! ```
//! use nostr::nip78::AppData;
//!
//! // Store user settings for an app
//! let settings = AppData::new(
//!     "myapp:settings:theme",
//!     r#"{"theme": "dark", "fontSize": 14}"#
//! );
//!
//! // Store app configuration
//! let config = AppData::new("myapp:config:v1", "custom config data");
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Kind number for application-specific data.
pub const KIND_APP_DATA: u64 = 30078;

/// Errors that can occur during NIP-78 operations.
#[derive(Debug, Error)]
pub enum Nip78Error {
    #[error("d tag identifier cannot be empty")]
    EmptyIdentifier,

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Check if a kind is a NIP-78 application data kind.
pub fn is_app_data_kind(kind: u64) -> bool {
    kind == KIND_APP_DATA
}

/// Application-specific data (kind 30078).
///
/// This is an addressable event that can store arbitrary data for applications.
/// The d tag identifies the app and context, while content and tags are flexible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppData {
    /// The d tag identifier (app name and context)
    /// Examples: "myapp:settings", "myapp:config:v1", "nostr-client:preferences"
    pub identifier: String,

    /// Arbitrary content (can be JSON, plain text, or any format)
    pub content: String,

    /// Additional custom tags
    pub tags: Vec<Vec<String>>,
}

impl AppData {
    /// Create new application-specific data.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip78::AppData;
    ///
    /// let data = AppData::new("myapp:settings", r#"{"theme": "dark"}"#);
    /// assert_eq!(data.identifier, "myapp:settings");
    /// ```
    pub fn new(identifier: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            content: content.into(),
            tags: Vec::new(),
        }
    }

    /// Create with JSON content.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip78::AppData;
    /// use serde_json::json;
    ///
    /// let data = AppData::with_json(
    ///     "myapp:settings",
    ///     &json!({"theme": "dark", "fontSize": 14})
    /// ).unwrap();
    /// ```
    pub fn with_json(identifier: impl Into<String>, value: &Value) -> Result<Self, Nip78Error> {
        Ok(Self {
            identifier: identifier.into(),
            content: serde_json::to_string(value)?,
            tags: Vec::new(),
        })
    }

    /// Add a custom tag.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip78::AppData;
    ///
    /// let mut data = AppData::new("myapp:data", "content");
    /// data.add_tag(vec!["custom".to_string(), "value".to_string()]);
    /// ```
    pub fn add_tag(&mut self, tag: Vec<String>) {
        self.tags.push(tag);
    }

    /// Add custom tags (builder pattern).
    pub fn with_tags(mut self, tags: Vec<Vec<String>>) -> Self {
        self.tags = tags;
        self
    }

    /// Parse content as JSON.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip78::AppData;
    ///
    /// let data = AppData::new("myapp:settings", r#"{"theme": "dark"}"#);
    /// let json = data.parse_json().unwrap();
    /// assert_eq!(json["theme"], "dark");
    /// ```
    pub fn parse_json(&self) -> Result<Value, Nip78Error> {
        Ok(serde_json::from_str(&self.content)?)
    }

    /// Check if content is valid JSON.
    pub fn is_json(&self) -> bool {
        serde_json::from_str::<Value>(&self.content).is_ok()
    }

    /// Validate the app data.
    pub fn validate(&self) -> Result<(), Nip78Error> {
        if self.identifier.is_empty() {
            return Err(Nip78Error::EmptyIdentifier);
        }
        Ok(())
    }

    /// Convert to Nostr event tags.
    ///
    /// Returns d tag plus any custom tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.identifier.clone()]];
        tags.extend(self.tags.clone());
        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_app_data_kind() {
        assert!(is_app_data_kind(30078));
        assert!(!is_app_data_kind(1));
        assert!(!is_app_data_kind(30077));
    }

    #[test]
    fn test_app_data_new() {
        let data = AppData::new("myapp:settings", "content");
        assert_eq!(data.identifier, "myapp:settings");
        assert_eq!(data.content, "content");
        assert!(data.tags.is_empty());
    }

    #[test]
    fn test_app_data_with_json() {
        let value = json!({"theme": "dark", "fontSize": 14});
        let data = AppData::with_json("myapp:settings", &value).unwrap();

        assert_eq!(data.identifier, "myapp:settings");
        assert!(data.is_json());

        let parsed = data.parse_json().unwrap();
        assert_eq!(parsed["theme"], "dark");
        assert_eq!(parsed["fontSize"], 14);
    }

    #[test]
    fn test_app_data_add_tag() {
        let mut data = AppData::new("myapp:data", "content");
        data.add_tag(vec!["custom".to_string(), "value".to_string()]);
        data.add_tag(vec!["another".to_string(), "tag".to_string()]);

        assert_eq!(data.tags.len(), 2);
        assert_eq!(data.tags[0], vec!["custom", "value"]);
        assert_eq!(data.tags[1], vec!["another", "tag"]);
    }

    #[test]
    fn test_app_data_with_tags() {
        let tags = vec![
            vec!["custom".to_string(), "value".to_string()],
            vec!["another".to_string(), "tag".to_string()],
        ];
        let data = AppData::new("myapp:data", "content").with_tags(tags);

        assert_eq!(data.tags.len(), 2);
    }

    #[test]
    fn test_app_data_parse_json() {
        let data = AppData::new("myapp:settings", r#"{"theme": "dark", "fontSize": 14}"#);
        let json = data.parse_json().unwrap();

        assert_eq!(json["theme"], "dark");
        assert_eq!(json["fontSize"], 14);
    }

    #[test]
    fn test_app_data_parse_json_invalid() {
        let data = AppData::new("myapp:settings", "not json");
        assert!(data.parse_json().is_err());
    }

    #[test]
    fn test_app_data_is_json() {
        let data = AppData::new("myapp:settings", r#"{"valid": "json"}"#);
        assert!(data.is_json());

        let data = AppData::new("myapp:settings", "not json");
        assert!(!data.is_json());
    }

    #[test]
    fn test_app_data_validate() {
        let data = AppData::new("myapp:settings", "content");
        assert!(data.validate().is_ok());

        let data = AppData::new("", "content");
        assert!(data.validate().is_err());
    }

    #[test]
    fn test_app_data_to_tags() {
        let data = AppData::new("myapp:settings:theme", "content")
            .with_tags(vec![vec!["custom".to_string(), "value".to_string()]]);

        let tags = data.to_tags();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], vec!["d", "myapp:settings:theme"]);
        assert_eq!(tags[1], vec!["custom", "value"]);
    }

    #[test]
    fn test_app_data_identifier_formats() {
        // Test various identifier formats
        let data = AppData::new("myapp:settings", "content");
        assert!(data.validate().is_ok());

        let data = AppData::new("nostr-client:preferences:theme", "content");
        assert!(data.validate().is_ok());

        let data = AppData::new("app.example.com:config:v1", "content");
        assert!(data.validate().is_ok());
    }

    #[test]
    fn test_app_data_plain_text() {
        let data = AppData::new("myapp:note", "This is plain text content");
        assert_eq!(data.content, "This is plain text content");
        assert!(!data.is_json());
    }

    #[test]
    fn test_app_data_json_string() {
        let value = json!("simple string value");
        let data = AppData::with_json("myapp:text", &value).unwrap();
        assert!(data.is_json());

        let parsed = data.parse_json().unwrap();
        assert_eq!(parsed, "simple string value");
    }

    #[test]
    fn test_app_data_complex_json() {
        let value = json!({
            "user": {
                "name": "Alice",
                "settings": {
                    "theme": "dark",
                    "notifications": true
                }
            },
            "version": 2
        });

        let data = AppData::with_json("myapp:user-settings", &value).unwrap();
        assert!(data.is_json());

        let parsed = data.parse_json().unwrap();
        assert_eq!(parsed["user"]["name"], "Alice");
        assert_eq!(parsed["user"]["settings"]["theme"], "dark");
        assert_eq!(parsed["version"], 2);
    }

    #[test]
    fn test_app_data_empty_content() {
        let data = AppData::new("myapp:marker", "");
        assert!(data.validate().is_ok());
        assert_eq!(data.content, "");
    }

    #[test]
    fn test_app_data_use_case_settings() {
        // User settings use case
        let settings = json!({
            "theme": "dark",
            "fontSize": 14,
            "language": "en"
        });

        let data = AppData::with_json("nostr-client:settings", &settings).unwrap();
        assert_eq!(data.identifier, "nostr-client:settings");
        assert!(data.is_json());
    }

    #[test]
    fn test_app_data_use_case_dynamic_params() {
        // Dynamic parameters use case
        let params = json!({
            "featureFlags": {
                "newUI": true,
                "betaFeatures": false
            },
            "apiEndpoint": "https://api.example.com/v2"
        });

        let mut data = AppData::with_json("myapp:config:production", &params).unwrap();
        data.add_tag(vec!["version".to_string(), "2.0".to_string()]);

        assert!(data.is_json());
        assert_eq!(data.tags.len(), 1);
    }

    #[test]
    fn test_app_data_use_case_private_data() {
        // Private data storage use case
        let private_data = AppData::new(
            "notes-app:draft:2024-01-15",
            "This is my private note content that has nothing to do with Nostr",
        );

        assert_eq!(private_data.identifier, "notes-app:draft:2024-01-15");
        assert!(!private_data.is_json());
    }
}
