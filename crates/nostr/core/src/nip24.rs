//! NIP-24: Extra Metadata Fields and Tags
//!
//! This NIP documents extra optional fields for metadata events and common tags
//! that have become de facto standards.
//!
//! ## Kind 0 Extra Fields
//!
//! - display_name: Alternative, bigger name with richer characters
//! - website: Web URL related to the author
//! - banner: Wide profile banner image URL
//! - bot: Boolean indicating automated content
//! - birthday: Birth date object with year, month, day
//!
//! ## Common Tags
//!
//! - r: Web URL reference
//! - i: External ID reference (NIP-73)
//! - title: Name for sets, events, listings
//! - t: Hashtag (lowercase)
//!
//! ## Examples
//!
//! ```
//! use nostr::nip24::{ExtraMetadata, Birthday};
//!
//! let mut metadata = ExtraMetadata::default();
//! metadata.display_name = Some("Alice 🎨".to_string());
//! metadata.website = Some("https://alice.example.com".to_string());
//! metadata.bot = Some(false);
//! metadata.birthday = Some(Birthday::new(1990, Some(5), Some(15)));
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

/// Errors that can occur during NIP-24 operations.
#[derive(Debug, Error)]
pub enum Nip24Error {
    #[error("invalid birthday format")]
    InvalidBirthday,

    #[error("hashtag must be lowercase")]
    UppercaseHashtag,

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Birthday information for a profile.
///
/// Any field can be omitted for privacy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Birthday {
    /// Birth year (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,

    /// Birth month (1-12, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<u8>,

    /// Birth day (1-31, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day: Option<u8>,
}

impl Birthday {
    /// Create a new birthday with all fields.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip24::Birthday;
    ///
    /// let birthday = Birthday::new(1990, Some(5), Some(15));
    /// assert_eq!(birthday.year, Some(1990));
    /// assert_eq!(birthday.month, Some(5));
    /// assert_eq!(birthday.day, Some(15));
    /// ```
    pub fn new(year: u16, month: Option<u8>, day: Option<u8>) -> Self {
        Self {
            year: Some(year),
            month,
            day,
        }
    }

    /// Create with only year.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip24::Birthday;
    ///
    /// let birthday = Birthday::year_only(1990);
    /// assert_eq!(birthday.year, Some(1990));
    /// assert_eq!(birthday.month, None);
    /// ```
    pub fn year_only(year: u16) -> Self {
        Self {
            year: Some(year),
            month: None,
            day: None,
        }
    }

    /// Create with year and month.
    pub fn year_month(year: u16, month: u8) -> Self {
        Self {
            year: Some(year),
            month: Some(month),
            day: None,
        }
    }

    /// Validate the birthday.
    pub fn validate(&self) -> Result<(), Nip24Error> {
        if let Some(month) = self.month {
            if !(1..=12).contains(&month) {
                return Err(Nip24Error::InvalidBirthday);
            }
        }

        if let Some(day) = self.day {
            if !(1..=31).contains(&day) {
                return Err(Nip24Error::InvalidBirthday);
            }
        }

        Ok(())
    }
}

/// Extra metadata fields for kind 0 events.
///
/// These are optional fields that extend the base NIP-01 metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraMetadata {
    /// Alternative display name with richer characters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Website URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,

    /// Banner image URL (wide ~1024x768)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<String>,

    /// Boolean indicating automated/bot content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot: Option<bool>,

    /// Birthday information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<Birthday>,
}

impl ExtraMetadata {
    /// Create new empty extra metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set display name (builder pattern).
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    /// Set website (builder pattern).
    pub fn with_website(mut self, website: impl Into<String>) -> Self {
        self.website = Some(website.into());
        self
    }

    /// Set banner (builder pattern).
    pub fn with_banner(mut self, banner: impl Into<String>) -> Self {
        self.banner = Some(banner.into());
        self
    }

    /// Set bot flag (builder pattern).
    pub fn with_bot(mut self, bot: bool) -> Self {
        self.bot = Some(bot);
        self
    }

    /// Set birthday (builder pattern).
    pub fn with_birthday(mut self, birthday: Birthday) -> Self {
        self.birthday = Some(birthday);
        self
    }

    /// Merge with existing metadata JSON.
    ///
    /// Adds extra fields to a base metadata object.
    pub fn merge_into(&self, metadata: &mut Map<String, Value>) -> Result<(), Nip24Error> {
        if let Some(display_name) = &self.display_name {
            metadata.insert(
                "display_name".to_string(),
                Value::String(display_name.clone()),
            );
        }

        if let Some(website) = &self.website {
            metadata.insert("website".to_string(), Value::String(website.clone()));
        }

        if let Some(banner) = &self.banner {
            metadata.insert("banner".to_string(), Value::String(banner.clone()));
        }

        if let Some(bot) = self.bot {
            metadata.insert("bot".to_string(), Value::Bool(bot));
        }

        if let Some(birthday) = &self.birthday {
            birthday.validate()?;
            metadata.insert("birthday".to_string(), serde_json::to_value(birthday)?);
        }

        Ok(())
    }

    /// Parse from metadata JSON.
    pub fn from_metadata(metadata: &Map<String, Value>) -> Self {
        Self {
            display_name: metadata
                .get("display_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            website: metadata
                .get("website")
                .and_then(|v| v.as_str())
                .map(String::from),
            banner: metadata
                .get("banner")
                .and_then(|v| v.as_str())
                .map(String::from),
            bot: metadata.get("bot").and_then(|v| v.as_bool()),
            birthday: metadata
                .get("birthday")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
        }
    }

    /// Validate all fields.
    pub fn validate(&self) -> Result<(), Nip24Error> {
        if let Some(birthday) = &self.birthday {
            birthday.validate()?;
        }
        Ok(())
    }
}

/// Validate and normalize a hashtag.
///
/// Hashtags must be lowercase per NIP-24.
pub fn validate_hashtag(tag: &str) -> Result<String, Nip24Error> {
    let lowercase = tag.to_lowercase();
    if tag != lowercase {
        return Err(Nip24Error::UppercaseHashtag);
    }
    Ok(lowercase)
}

/// Normalize a hashtag to lowercase.
pub fn normalize_hashtag(tag: &str) -> String {
    tag.to_lowercase()
}

/// Remove deprecated fields from metadata.
///
/// Removes: displayName, username
pub fn remove_deprecated_fields(metadata: &mut Map<String, Value>) {
    metadata.remove("displayName");
    metadata.remove("username");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_birthday_new() {
        let birthday = Birthday::new(1990, Some(5), Some(15));
        assert_eq!(birthday.year, Some(1990));
        assert_eq!(birthday.month, Some(5));
        assert_eq!(birthday.day, Some(15));
        assert!(birthday.validate().is_ok());
    }

    #[test]
    fn test_birthday_year_only() {
        let birthday = Birthday::year_only(1990);
        assert_eq!(birthday.year, Some(1990));
        assert_eq!(birthday.month, None);
        assert_eq!(birthday.day, None);
    }

    #[test]
    fn test_birthday_year_month() {
        let birthday = Birthday::year_month(1990, 5);
        assert_eq!(birthday.year, Some(1990));
        assert_eq!(birthday.month, Some(5));
        assert_eq!(birthday.day, None);
    }

    #[test]
    fn test_birthday_validate() {
        let birthday = Birthday::new(1990, Some(5), Some(15));
        assert!(birthday.validate().is_ok());

        let birthday = Birthday::new(1990, Some(13), Some(15));
        assert!(birthday.validate().is_err());

        let birthday = Birthday::new(1990, Some(5), Some(32));
        assert!(birthday.validate().is_err());
    }

    #[test]
    fn test_extra_metadata_new() {
        let metadata = ExtraMetadata::new();
        assert_eq!(metadata.display_name, None);
        assert_eq!(metadata.website, None);
        assert_eq!(metadata.banner, None);
        assert_eq!(metadata.bot, None);
        assert_eq!(metadata.birthday, None);
    }

    #[test]
    fn test_extra_metadata_builder() {
        let metadata = ExtraMetadata::new()
            .with_display_name("Alice 🎨")
            .with_website("https://alice.example.com")
            .with_banner("https://alice.example.com/banner.jpg")
            .with_bot(false)
            .with_birthday(Birthday::new(1990, Some(5), Some(15)));

        assert_eq!(metadata.display_name, Some("Alice 🎨".to_string()));
        assert_eq!(
            metadata.website,
            Some("https://alice.example.com".to_string())
        );
        assert_eq!(
            metadata.banner,
            Some("https://alice.example.com/banner.jpg".to_string())
        );
        assert_eq!(metadata.bot, Some(false));
        assert!(metadata.birthday.is_some());
    }

    #[test]
    fn test_extra_metadata_merge_into() {
        let mut base = Map::new();
        base.insert("name".to_string(), Value::String("alice".to_string()));

        let extra = ExtraMetadata::new()
            .with_display_name("Alice 🎨")
            .with_website("https://alice.example.com")
            .with_bot(false);

        extra.merge_into(&mut base).unwrap();

        assert_eq!(base.get("name").unwrap().as_str(), Some("alice"));
        assert_eq!(base.get("display_name").unwrap().as_str(), Some("Alice 🎨"));
        assert_eq!(
            base.get("website").unwrap().as_str(),
            Some("https://alice.example.com")
        );
        assert_eq!(base.get("bot").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_extra_metadata_from_metadata() {
        let mut map = Map::new();
        map.insert(
            "display_name".to_string(),
            Value::String("Alice 🎨".to_string()),
        );
        map.insert(
            "website".to_string(),
            Value::String("https://alice.example.com".to_string()),
        );
        map.insert("bot".to_string(), Value::Bool(false));
        map.insert(
            "birthday".to_string(),
            serde_json::json!({"year": 1990, "month": 5, "day": 15}),
        );

        let extra = ExtraMetadata::from_metadata(&map);

        assert_eq!(extra.display_name, Some("Alice 🎨".to_string()));
        assert_eq!(extra.website, Some("https://alice.example.com".to_string()));
        assert_eq!(extra.bot, Some(false));
        assert!(extra.birthday.is_some());

        let birthday = extra.birthday.unwrap();
        assert_eq!(birthday.year, Some(1990));
        assert_eq!(birthday.month, Some(5));
        assert_eq!(birthday.day, Some(15));
    }

    #[test]
    fn test_validate_hashtag() {
        assert!(validate_hashtag("bitcoin").is_ok());
        assert!(validate_hashtag("nostr").is_ok());

        assert!(validate_hashtag("Bitcoin").is_err());
        assert!(validate_hashtag("NOSTR").is_err());
    }

    #[test]
    fn test_normalize_hashtag() {
        assert_eq!(normalize_hashtag("Bitcoin"), "bitcoin");
        assert_eq!(normalize_hashtag("NOSTR"), "nostr");
        assert_eq!(normalize_hashtag("bitcoin"), "bitcoin");
    }

    #[test]
    fn test_remove_deprecated_fields() {
        let mut metadata = Map::new();
        metadata.insert("name".to_string(), Value::String("alice".to_string()));
        metadata.insert(
            "displayName".to_string(),
            Value::String("Alice".to_string()),
        );
        metadata.insert(
            "username".to_string(),
            Value::String("alice123".to_string()),
        );

        remove_deprecated_fields(&mut metadata);

        assert!(metadata.contains_key("name"));
        assert!(!metadata.contains_key("displayName"));
        assert!(!metadata.contains_key("username"));
    }

    #[test]
    fn test_extra_metadata_validate() {
        let metadata = ExtraMetadata::new()
            .with_display_name("Alice")
            .with_birthday(Birthday::new(1990, Some(5), Some(15)));

        assert!(metadata.validate().is_ok());

        let metadata = ExtraMetadata::new().with_birthday(Birthday::new(1990, Some(13), Some(15)));

        assert!(metadata.validate().is_err());
    }

    #[test]
    fn test_birthday_serialization() {
        let birthday = Birthday::new(1990, Some(5), Some(15));
        let json = serde_json::to_value(&birthday).unwrap();

        assert_eq!(json["year"], 1990);
        assert_eq!(json["month"], 5);
        assert_eq!(json["day"], 15);
    }

    #[test]
    fn test_birthday_partial_serialization() {
        let birthday = Birthday::year_only(1990);
        let json = serde_json::to_value(&birthday).unwrap();

        assert_eq!(json["year"], 1990);
        assert!(json.get("month").is_none());
        assert!(json.get("day").is_none());
    }

    #[test]
    fn test_extra_metadata_bot_automation() {
        let bot_metadata = ExtraMetadata::new()
            .with_display_name("News Bot")
            .with_bot(true);

        assert_eq!(bot_metadata.bot, Some(true));

        let human_metadata = ExtraMetadata::new()
            .with_display_name("Alice")
            .with_bot(false);

        assert_eq!(human_metadata.bot, Some(false));
    }
}
