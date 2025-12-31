//! NIP-39: External Identities in Profiles
//!
//! This NIP defines the `i` tag for kind 0 metadata events to link Nostr identities
//! to external platforms like GitHub, Twitter, Mastodon, and Telegram.
//!
//! ## Features
//!
//! - External identity claims with proofs
//! - Platform-specific identity types (GitHub, Twitter, Mastodon, Telegram)
//! - Proof URL generation for verification
//! - Identity normalization
//!
//! ## Examples
//!
//! ```
//! use nostr::nip39::{ExternalIdentity, GitHubIdentity, TwitterIdentity};
//!
//! // Create a GitHub identity claim
//! let github = GitHubIdentity::new("semisol", "9721ce4ee4fceb91c9711ca2a6c9a5ab");
//! let identity = ExternalIdentity::GitHub(github);
//!
//! // Create a Twitter identity claim
//! let twitter = TwitterIdentity::new("semisol_public", "1619358434134196225");
//! let identity = ExternalIdentity::Twitter(twitter);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during NIP-39 operations.
#[derive(Debug, Error)]
pub enum Nip39Error {
    #[error("platform name cannot be empty")]
    EmptyPlatform,

    #[error("identity cannot be empty")]
    EmptyIdentity,

    #[error("proof cannot be empty")]
    EmptyProof,

    #[error("platform name contains invalid character ':'")]
    InvalidPlatformName,

    #[error("invalid platform:identity format")]
    InvalidFormat,
}

/// Normalize an identity string (lowercase).
pub fn normalize_identity(identity: &str) -> String {
    identity.to_lowercase()
}

/// Validate a platform name.
///
/// Platform names SHOULD only include a-z, 0-9 and ._-/
/// Platform names MUST NOT include :
pub fn validate_platform_name(platform: &str) -> Result<(), Nip39Error> {
    if platform.is_empty() {
        return Err(Nip39Error::EmptyPlatform);
    }

    if platform.contains(':') {
        return Err(Nip39Error::InvalidPlatformName);
    }

    Ok(())
}

/// A GitHub identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubIdentity {
    /// GitHub username
    pub username: String,
    /// GitHub Gist ID containing the proof
    pub gist_id: String,
}

impl GitHubIdentity {
    /// Create a new GitHub identity.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip39::GitHubIdentity;
    ///
    /// let identity = GitHubIdentity::new("semisol", "9721ce4ee4fceb91c9711ca2a6c9a5ab");
    /// assert_eq!(identity.username, "semisol");
    /// ```
    pub fn new(username: impl Into<String>, gist_id: impl Into<String>) -> Self {
        Self {
            username: normalize_identity(&username.into()),
            gist_id: gist_id.into(),
        }
    }

    /// Get the proof URL.
    ///
    /// Returns: `https://gist.github.com/<username>/<gist_id>`
    pub fn proof_url(&self) -> String {
        format!("https://gist.github.com/{}/{}", self.username, self.gist_id)
    }

    /// Get the platform:identity string.
    pub fn platform_identity(&self) -> String {
        format!("github:{}", self.username)
    }

    /// Validate the identity.
    pub fn validate(&self) -> Result<(), Nip39Error> {
        if self.username.is_empty() {
            return Err(Nip39Error::EmptyIdentity);
        }
        if self.gist_id.is_empty() {
            return Err(Nip39Error::EmptyProof);
        }
        Ok(())
    }
}

/// A Twitter identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwitterIdentity {
    /// Twitter username
    pub username: String,
    /// Tweet ID containing the proof
    pub tweet_id: String,
}

impl TwitterIdentity {
    /// Create a new Twitter identity.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip39::TwitterIdentity;
    ///
    /// let identity = TwitterIdentity::new("semisol_public", "1619358434134196225");
    /// assert_eq!(identity.username, "semisol_public");
    /// ```
    pub fn new(username: impl Into<String>, tweet_id: impl Into<String>) -> Self {
        Self {
            username: normalize_identity(&username.into()),
            tweet_id: tweet_id.into(),
        }
    }

    /// Get the proof URL.
    ///
    /// Returns: `https://twitter.com/<username>/status/<tweet_id>`
    pub fn proof_url(&self) -> String {
        format!(
            "https://twitter.com/{}/status/{}",
            self.username, self.tweet_id
        )
    }

    /// Get the platform:identity string.
    pub fn platform_identity(&self) -> String {
        format!("twitter:{}", self.username)
    }

    /// Validate the identity.
    pub fn validate(&self) -> Result<(), Nip39Error> {
        if self.username.is_empty() {
            return Err(Nip39Error::EmptyIdentity);
        }
        if self.tweet_id.is_empty() {
            return Err(Nip39Error::EmptyProof);
        }
        Ok(())
    }
}

/// A Mastodon identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MastodonIdentity {
    /// Mastodon instance (e.g., "bitcoinhackers.org")
    pub instance: String,
    /// Mastodon username (without @)
    pub username: String,
    /// Post ID containing the proof
    pub post_id: String,
}

impl MastodonIdentity {
    /// Create a new Mastodon identity.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip39::MastodonIdentity;
    ///
    /// let identity = MastodonIdentity::new(
    ///     "bitcoinhackers.org",
    ///     "semisol",
    ///     "109775066355589974"
    /// );
    /// assert_eq!(identity.instance, "bitcoinhackers.org");
    /// assert_eq!(identity.username, "semisol");
    /// ```
    pub fn new(
        instance: impl Into<String>,
        username: impl Into<String>,
        post_id: impl Into<String>,
    ) -> Self {
        Self {
            instance: instance.into(),
            username: normalize_identity(&username.into()),
            post_id: post_id.into(),
        }
    }

    /// Get the proof URL.
    ///
    /// Returns: `https://<instance>/@<username>/<post_id>`
    pub fn proof_url(&self) -> String {
        format!(
            "https://{}/@{}/{}",
            self.instance, self.username, self.post_id
        )
    }

    /// Get the platform:identity string.
    ///
    /// Returns: `mastodon:<instance>/@<username>`
    pub fn platform_identity(&self) -> String {
        format!("mastodon:{}/@{}", self.instance, self.username)
    }

    /// Validate the identity.
    pub fn validate(&self) -> Result<(), Nip39Error> {
        if self.instance.is_empty() {
            return Err(Nip39Error::EmptyIdentity);
        }
        if self.username.is_empty() {
            return Err(Nip39Error::EmptyIdentity);
        }
        if self.post_id.is_empty() {
            return Err(Nip39Error::EmptyProof);
        }
        Ok(())
    }
}

/// A Telegram identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramIdentity {
    /// Telegram user ID
    pub user_id: String,
    /// Channel/group reference
    pub channel_ref: String,
    /// Message ID in the channel
    pub message_id: String,
}

impl TelegramIdentity {
    /// Create a new Telegram identity.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip39::TelegramIdentity;
    ///
    /// let identity = TelegramIdentity::new("1087295469", "nostrdirectory", "770");
    /// assert_eq!(identity.user_id, "1087295469");
    /// ```
    pub fn new(
        user_id: impl Into<String>,
        channel_ref: impl Into<String>,
        message_id: impl Into<String>,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            channel_ref: channel_ref.into(),
            message_id: message_id.into(),
        }
    }

    /// Get the proof string.
    ///
    /// Returns: `<channel_ref>/<message_id>`
    pub fn proof_string(&self) -> String {
        format!("{}/{}", self.channel_ref, self.message_id)
    }

    /// Get the proof URL.
    ///
    /// Returns: `https://t.me/<channel_ref>/<message_id>`
    pub fn proof_url(&self) -> String {
        format!("https://t.me/{}", self.proof_string())
    }

    /// Get the platform:identity string.
    pub fn platform_identity(&self) -> String {
        format!("telegram:{}", self.user_id)
    }

    /// Validate the identity.
    pub fn validate(&self) -> Result<(), Nip39Error> {
        if self.user_id.is_empty() {
            return Err(Nip39Error::EmptyIdentity);
        }
        if self.channel_ref.is_empty() || self.message_id.is_empty() {
            return Err(Nip39Error::EmptyProof);
        }
        Ok(())
    }
}

/// A generic external identity for custom platforms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericIdentity {
    /// Platform name (e.g., "reddit", "discord")
    pub platform: String,
    /// Identity on that platform
    pub identity: String,
    /// Proof string
    pub proof: String,
}

impl GenericIdentity {
    /// Create a new generic identity.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip39::GenericIdentity;
    ///
    /// let identity = GenericIdentity::new("reddit", "my_username", "proof_id");
    /// assert_eq!(identity.platform, "reddit");
    /// ```
    pub fn new(
        platform: impl Into<String>,
        identity: impl Into<String>,
        proof: impl Into<String>,
    ) -> Self {
        Self {
            platform: platform.into(),
            identity: normalize_identity(&identity.into()),
            proof: proof.into(),
        }
    }

    /// Get the platform:identity string.
    pub fn platform_identity(&self) -> String {
        format!("{}:{}", self.platform, self.identity)
    }

    /// Validate the identity.
    pub fn validate(&self) -> Result<(), Nip39Error> {
        validate_platform_name(&self.platform)?;
        if self.identity.is_empty() {
            return Err(Nip39Error::EmptyIdentity);
        }
        if self.proof.is_empty() {
            return Err(Nip39Error::EmptyProof);
        }
        Ok(())
    }
}

/// An external identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExternalIdentity {
    GitHub(GitHubIdentity),
    Twitter(TwitterIdentity),
    Mastodon(MastodonIdentity),
    Telegram(TelegramIdentity),
    Generic(GenericIdentity),
}

impl ExternalIdentity {
    /// Parse from platform:identity and proof strings.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip39::ExternalIdentity;
    ///
    /// let identity = ExternalIdentity::parse(
    ///     "github:semisol",
    ///     "9721ce4ee4fceb91c9711ca2a6c9a5ab"
    /// ).unwrap();
    /// ```
    pub fn parse(platform_identity: &str, proof: &str) -> Result<Self, Nip39Error> {
        let parts: Vec<&str> = platform_identity.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Nip39Error::InvalidFormat);
        }

        let platform = parts[0];
        let identity = parts[1];

        match platform {
            "github" => Ok(Self::GitHub(GitHubIdentity::new(identity, proof))),
            "twitter" => Ok(Self::Twitter(TwitterIdentity::new(identity, proof))),
            "mastodon" => {
                // Format: instance/@username
                let parts: Vec<&str> = identity.splitn(2, "/@").collect();
                if parts.len() != 2 {
                    return Err(Nip39Error::InvalidFormat);
                }
                Ok(Self::Mastodon(MastodonIdentity::new(
                    parts[0], parts[1], proof,
                )))
            }
            "telegram" => {
                // Proof format: channel/message
                let proof_parts: Vec<&str> = proof.splitn(2, '/').collect();
                if proof_parts.len() != 2 {
                    return Err(Nip39Error::InvalidFormat);
                }
                Ok(Self::Telegram(TelegramIdentity::new(
                    identity,
                    proof_parts[0],
                    proof_parts[1],
                )))
            }
            _ => Ok(Self::Generic(GenericIdentity::new(
                platform, identity, proof,
            ))),
        }
    }

    /// Get the platform:identity string.
    pub fn platform_identity(&self) -> String {
        match self {
            Self::GitHub(id) => id.platform_identity(),
            Self::Twitter(id) => id.platform_identity(),
            Self::Mastodon(id) => id.platform_identity(),
            Self::Telegram(id) => id.platform_identity(),
            Self::Generic(id) => id.platform_identity(),
        }
    }

    /// Get the proof string.
    pub fn proof(&self) -> String {
        match self {
            Self::GitHub(id) => id.gist_id.clone(),
            Self::Twitter(id) => id.tweet_id.clone(),
            Self::Mastodon(id) => id.post_id.clone(),
            Self::Telegram(id) => id.proof_string(),
            Self::Generic(id) => id.proof.clone(),
        }
    }

    /// Validate the identity.
    pub fn validate(&self) -> Result<(), Nip39Error> {
        match self {
            Self::GitHub(id) => id.validate(),
            Self::Twitter(id) => id.validate(),
            Self::Mastodon(id) => id.validate(),
            Self::Telegram(id) => id.validate(),
            Self::Generic(id) => id.validate(),
        }
    }

    /// Convert to tag format for kind 0 metadata events.
    ///
    /// Returns: `["i", "platform:identity", "proof"]`
    pub fn to_tag(&self) -> Vec<String> {
        vec!["i".to_string(), self.platform_identity(), self.proof()]
    }

    /// Parse from tag format.
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip39Error> {
        if tag.len() < 3 || tag[0] != "i" {
            return Err(Nip39Error::InvalidFormat);
        }

        Self::parse(&tag[1], &tag[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_identity() {
        assert_eq!(normalize_identity("SemiSol"), "semisol");
        assert_eq!(normalize_identity("UPPERCASE"), "uppercase");
        assert_eq!(normalize_identity("lowercase"), "lowercase");
    }

    #[test]
    fn test_validate_platform_name() {
        assert!(validate_platform_name("github").is_ok());
        assert!(validate_platform_name("my-platform").is_ok());
        assert!(validate_platform_name("my.platform").is_ok());
        assert!(validate_platform_name("my_platform").is_ok());
        assert!(validate_platform_name("my/platform").is_ok());

        assert!(validate_platform_name("").is_err());
        assert!(validate_platform_name("my:platform").is_err());
    }

    #[test]
    fn test_github_identity() {
        let identity = GitHubIdentity::new("semisol", "9721ce4ee4fceb91c9711ca2a6c9a5ab");
        assert_eq!(identity.username, "semisol");
        assert_eq!(identity.gist_id, "9721ce4ee4fceb91c9711ca2a6c9a5ab");
        assert_eq!(identity.platform_identity(), "github:semisol");
        assert_eq!(
            identity.proof_url(),
            "https://gist.github.com/semisol/9721ce4ee4fceb91c9711ca2a6c9a5ab"
        );
        assert!(identity.validate().is_ok());
    }

    #[test]
    fn test_twitter_identity() {
        let identity = TwitterIdentity::new("semisol_public", "1619358434134196225");
        assert_eq!(identity.username, "semisol_public");
        assert_eq!(identity.tweet_id, "1619358434134196225");
        assert_eq!(identity.platform_identity(), "twitter:semisol_public");
        assert_eq!(
            identity.proof_url(),
            "https://twitter.com/semisol_public/status/1619358434134196225"
        );
        assert!(identity.validate().is_ok());
    }

    #[test]
    fn test_mastodon_identity() {
        let identity = MastodonIdentity::new("bitcoinhackers.org", "semisol", "109775066355589974");
        assert_eq!(identity.instance, "bitcoinhackers.org");
        assert_eq!(identity.username, "semisol");
        assert_eq!(identity.post_id, "109775066355589974");
        assert_eq!(
            identity.platform_identity(),
            "mastodon:bitcoinhackers.org/@semisol"
        );
        assert_eq!(
            identity.proof_url(),
            "https://bitcoinhackers.org/@semisol/109775066355589974"
        );
        assert!(identity.validate().is_ok());
    }

    #[test]
    fn test_telegram_identity() {
        let identity = TelegramIdentity::new("1087295469", "nostrdirectory", "770");
        assert_eq!(identity.user_id, "1087295469");
        assert_eq!(identity.channel_ref, "nostrdirectory");
        assert_eq!(identity.message_id, "770");
        assert_eq!(identity.platform_identity(), "telegram:1087295469");
        assert_eq!(identity.proof_string(), "nostrdirectory/770");
        assert_eq!(identity.proof_url(), "https://t.me/nostrdirectory/770");
        assert!(identity.validate().is_ok());
    }

    #[test]
    fn test_generic_identity() {
        let identity = GenericIdentity::new("reddit", "my_username", "proof123");
        assert_eq!(identity.platform, "reddit");
        assert_eq!(identity.identity, "my_username");
        assert_eq!(identity.proof, "proof123");
        assert_eq!(identity.platform_identity(), "reddit:my_username");
        assert!(identity.validate().is_ok());
    }

    #[test]
    fn test_external_identity_parse_github() {
        let identity =
            ExternalIdentity::parse("github:semisol", "9721ce4ee4fceb91c9711ca2a6c9a5ab").unwrap();

        match identity {
            ExternalIdentity::GitHub(id) => {
                assert_eq!(id.username, "semisol");
                assert_eq!(id.gist_id, "9721ce4ee4fceb91c9711ca2a6c9a5ab");
            }
            _ => panic!("Expected GitHub identity"),
        }
    }

    #[test]
    fn test_external_identity_parse_twitter() {
        let identity =
            ExternalIdentity::parse("twitter:semisol_public", "1619358434134196225").unwrap();

        match identity {
            ExternalIdentity::Twitter(id) => {
                assert_eq!(id.username, "semisol_public");
                assert_eq!(id.tweet_id, "1619358434134196225");
            }
            _ => panic!("Expected Twitter identity"),
        }
    }

    #[test]
    fn test_external_identity_parse_mastodon() {
        let identity =
            ExternalIdentity::parse("mastodon:bitcoinhackers.org/@semisol", "109775066355589974")
                .unwrap();

        match identity {
            ExternalIdentity::Mastodon(id) => {
                assert_eq!(id.instance, "bitcoinhackers.org");
                assert_eq!(id.username, "semisol");
                assert_eq!(id.post_id, "109775066355589974");
            }
            _ => panic!("Expected Mastodon identity"),
        }
    }

    #[test]
    fn test_external_identity_parse_telegram() {
        let identity =
            ExternalIdentity::parse("telegram:1087295469", "nostrdirectory/770").unwrap();

        match identity {
            ExternalIdentity::Telegram(id) => {
                assert_eq!(id.user_id, "1087295469");
                assert_eq!(id.channel_ref, "nostrdirectory");
                assert_eq!(id.message_id, "770");
            }
            _ => panic!("Expected Telegram identity"),
        }
    }

    #[test]
    fn test_external_identity_parse_generic() {
        let identity = ExternalIdentity::parse("reddit:my_username", "proof123").unwrap();

        match identity {
            ExternalIdentity::Generic(id) => {
                assert_eq!(id.platform, "reddit");
                assert_eq!(id.identity, "my_username");
                assert_eq!(id.proof, "proof123");
            }
            _ => panic!("Expected Generic identity"),
        }
    }

    #[test]
    fn test_external_identity_platform_identity() {
        let identity = ExternalIdentity::GitHub(GitHubIdentity::new("semisol", "gist123"));
        assert_eq!(identity.platform_identity(), "github:semisol");

        let identity =
            ExternalIdentity::Twitter(TwitterIdentity::new("semisol_public", "tweet123"));
        assert_eq!(identity.platform_identity(), "twitter:semisol_public");
    }

    #[test]
    fn test_external_identity_proof() {
        let identity = ExternalIdentity::GitHub(GitHubIdentity::new("semisol", "gist123"));
        assert_eq!(identity.proof(), "gist123");

        let identity = ExternalIdentity::Telegram(TelegramIdentity::new("123", "channel", "456"));
        assert_eq!(identity.proof(), "channel/456");
    }

    #[test]
    fn test_external_identity_to_tag() {
        let identity = ExternalIdentity::GitHub(GitHubIdentity::new(
            "semisol",
            "9721ce4ee4fceb91c9711ca2a6c9a5ab",
        ));
        let tag = identity.to_tag();
        assert_eq!(
            tag,
            vec!["i", "github:semisol", "9721ce4ee4fceb91c9711ca2a6c9a5ab"]
        );
    }

    #[test]
    fn test_external_identity_from_tag() {
        let tag = vec![
            "i".to_string(),
            "github:semisol".to_string(),
            "9721ce4ee4fceb91c9711ca2a6c9a5ab".to_string(),
        ];
        let identity = ExternalIdentity::from_tag(&tag).unwrap();

        match identity {
            ExternalIdentity::GitHub(id) => {
                assert_eq!(id.username, "semisol");
                assert_eq!(id.gist_id, "9721ce4ee4fceb91c9711ca2a6c9a5ab");
            }
            _ => panic!("Expected GitHub identity"),
        }
    }

    #[test]
    fn test_external_identity_validate() {
        let identity = ExternalIdentity::GitHub(GitHubIdentity::new("semisol", "gist123"));
        assert!(identity.validate().is_ok());

        let identity = ExternalIdentity::GitHub(GitHubIdentity::new("", "gist123"));
        assert!(identity.validate().is_err());

        let identity = ExternalIdentity::GitHub(GitHubIdentity::new("semisol", ""));
        assert!(identity.validate().is_err());
    }

    #[test]
    fn test_identity_normalization() {
        let identity = GitHubIdentity::new("SemiSol", "gist123");
        assert_eq!(identity.username, "semisol");

        let identity = TwitterIdentity::new("SemiSol_Public", "tweet123");
        assert_eq!(identity.username, "semisol_public");
    }
}
