//! Nostr client wrapper
//!
//! Provides high-level Nostr operations for the wallet

use crate::core::identity::UnifiedIdentity;
use anyhow::{Context, Result};
use serde_json::json;

/// User profile (kind:0 metadata)
#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
}

impl Profile {
    /// Create profile from JSON content
    pub fn from_json(content: &str) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_str(content)?;

        Ok(Self {
            name: value.get("name").and_then(|v| v.as_str()).map(String::from),
            about: value
                .get("about")
                .and_then(|v| v.as_str())
                .map(String::from),
            picture: value
                .get("picture")
                .and_then(|v| v.as_str())
                .map(String::from),
            nip05: value
                .get("nip05")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }

    /// Convert profile to JSON content
    pub fn to_json(&self) -> String {
        let mut obj = json!({});

        if let Some(name) = &self.name {
            obj["name"] = json!(name);
        }
        if let Some(about) = &self.about {
            obj["about"] = json!(about);
        }
        if let Some(picture) = &self.picture {
            obj["picture"] = json!(picture);
        }
        if let Some(nip05) = &self.nip05 {
            obj["nip05"] = json!(nip05);
        }

        obj.to_string()
    }

    /// Merge updates into this profile
    pub fn merge(&mut self, updates: ProfileUpdate) {
        if let Some(name) = updates.name {
            self.name = Some(name);
        }
        if let Some(about) = updates.about {
            self.about = Some(about);
        }
        if let Some(picture) = updates.picture {
            self.picture = Some(picture);
        }
        if let Some(nip05) = updates.nip05 {
            self.nip05 = Some(nip05);
        }
    }

    /// Check if profile is empty
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.about.is_none()
            && self.picture.is_none()
            && self.nip05.is_none()
    }
}

/// Profile update fields
#[derive(Debug, Clone, Default)]
pub struct ProfileUpdate {
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
}

/// Contact entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Contact {
    pub pubkey: String,
    pub relay: Option<String>,
    pub petname: Option<String>,
}

/// Create a kind:0 metadata event
pub fn create_profile_event(identity: &UnifiedIdentity, profile: &Profile) -> Result<nostr::Event> {
    let content = profile.to_json();

    let template = nostr::EventTemplate {
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        kind: 0, // Metadata
        tags: vec![],
        content,
    };

    identity
        .sign_event(template)
        .context("Failed to sign profile event")
}

/// Create a kind:1 text note event
#[allow(dead_code)]
pub fn create_note_event(identity: &UnifiedIdentity, content: &str) -> Result<nostr::Event> {
    let template = nostr::EventTemplate {
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        kind: 1, // Text note
        tags: vec![],
        content: content.to_string(),
    };

    identity
        .sign_event(template)
        .context("Failed to sign note event")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_to_json() {
        let profile = Profile {
            name: Some("Alice".to_string()),
            about: Some("Test user".to_string()),
            picture: None,
            nip05: None,
        };

        let json = profile.to_json();
        assert!(json.contains("Alice"));
        assert!(json.contains("Test user"));
    }

    #[test]
    fn test_profile_from_json() {
        let json = r#"{"name":"Bob","about":"Another test"}"#;
        let profile = Profile::from_json(json).unwrap();

        assert_eq!(profile.name, Some("Bob".to_string()));
        assert_eq!(profile.about, Some("Another test".to_string()));
    }

    #[test]
    fn test_profile_merge() {
        let mut profile = Profile {
            name: Some("Alice".to_string()),
            about: Some("Original".to_string()),
            picture: None,
            nip05: None,
        };

        let update = ProfileUpdate {
            name: None,
            about: Some("Updated".to_string()),
            picture: Some("https://example.com/pic.jpg".to_string()),
            nip05: None,
        };

        profile.merge(update);

        assert_eq!(profile.name, Some("Alice".to_string())); // Unchanged
        assert_eq!(profile.about, Some("Updated".to_string())); // Updated
        assert_eq!(
            profile.picture,
            Some("https://example.com/pic.jpg".to_string())
        ); // New
    }
}
