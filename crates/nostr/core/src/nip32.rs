//! NIP-32: Labeling
//!
//! This NIP defines kind 1985 for labeling events, pubkeys, relays, and topics.
//! Supports content moderation, categorization, license assignment, and reputation systems.
//!
//! ## Features
//!
//! - Label namespaces (L tag) for organized categorization
//! - Labels (l tag) for specific classifications
//! - Multiple target types (events, pubkeys, relays, topics, addresses)
//! - Self-reporting labels on any event kind
//! - Standard namespaces (ISO standards, reverse domain notation)
//!
//! ## Examples
//!
//! ```
//! use nostr::nip32::{Label, LabelEvent, LabelTarget};
//!
//! // Create a moderation label for a chat event
//! let label = Label::new("approve", "nip28.moderation");
//! let mut event = LabelEvent::new(
//!     vec![label],
//!     vec![LabelTarget::event("event-id-123", Some("wss://relay.example.com"))]
//! );
//! event.set_content("This message is appropriate");
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind number for label events.
pub const KIND_LABEL: u64 = 1985;

/// Default namespace for user-generated content.
pub const UGC_NAMESPACE: &str = "ugc";

/// Errors that can occur during NIP-32 operations.
#[derive(Debug, Error)]
pub enum Nip32Error {
    #[error("label must have a value")]
    MissingLabelValue,

    #[error("label event must have at least one target")]
    NoTargets,

    #[error("label namespace cannot be empty")]
    EmptyNamespace,

    #[error("label with namespace must have matching mark")]
    MissingNamespaceMark,
}

/// Check if a kind is a NIP-32 label event.
pub fn is_label_kind(kind: u64) -> bool {
    kind == KIND_LABEL
}

/// A label with optional namespace.
///
/// Labels are short, meaningful strings used to categorize content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    /// The label value (e.g., "approve", "MIT", "IT-MI")
    pub value: String,

    /// Optional namespace (e.g., "license", "ISO-3166-2", "com.example.ontology")
    /// If None, "ugc" is implied
    pub namespace: Option<String>,
}

impl Label {
    /// Create a new label with namespace.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip32::Label;
    ///
    /// let label = Label::new("MIT", "license");
    /// assert_eq!(label.value, "MIT");
    /// assert_eq!(label.namespace, Some("license".to_string()));
    /// ```
    pub fn new(value: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            namespace: Some(namespace.into()),
        }
    }

    /// Create a user-generated content label (no explicit namespace).
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip32::Label;
    ///
    /// let label = Label::ugc("spam");
    /// assert_eq!(label.value, "spam");
    /// assert_eq!(label.namespace, None);
    /// ```
    pub fn ugc(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            namespace: None,
        }
    }

    /// Check if this is a tag association label (namespace starts with #).
    pub fn is_tag_association(&self) -> bool {
        self.namespace
            .as_ref()
            .map(|ns| ns.starts_with('#'))
            .unwrap_or(false)
    }

    /// Get the effective namespace (returns "ugc" if None).
    pub fn effective_namespace(&self) -> &str {
        self.namespace.as_deref().unwrap_or(UGC_NAMESPACE)
    }

    /// Validate the label.
    pub fn validate(&self) -> Result<(), Nip32Error> {
        if self.value.is_empty() {
            return Err(Nip32Error::MissingLabelValue);
        }
        if let Some(ns) = &self.namespace
            && ns.is_empty()
        {
            return Err(Nip32Error::EmptyNamespace);
        }
        Ok(())
    }
}

/// Target for a label event.
///
/// Labels can be applied to events, pubkeys, relays, topics, or addressable events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelTarget {
    /// Event by ID
    Event { id: String, relay: Option<String> },
    /// Public key
    Pubkey {
        pubkey: String,
        relay: Option<String>,
    },
    /// Addressable event
    Address {
        address: String,
        relay: Option<String>,
    },
    /// Relay URL
    Relay(String),
    /// Topic/hashtag
    Topic(String),
}

impl LabelTarget {
    /// Create an event target.
    pub fn event(id: impl Into<String>, relay: Option<impl Into<String>>) -> Self {
        Self::Event {
            id: id.into(),
            relay: relay.map(|r| r.into()),
        }
    }

    /// Create a pubkey target.
    pub fn pubkey(pubkey: impl Into<String>, relay: Option<impl Into<String>>) -> Self {
        Self::Pubkey {
            pubkey: pubkey.into(),
            relay: relay.map(|r| r.into()),
        }
    }

    /// Create an address target.
    pub fn address(address: impl Into<String>, relay: Option<impl Into<String>>) -> Self {
        Self::Address {
            address: address.into(),
            relay: relay.map(|r| r.into()),
        }
    }

    /// Create a relay target.
    pub fn relay(url: impl Into<String>) -> Self {
        Self::Relay(url.into())
    }

    /// Create a topic target.
    pub fn topic(topic: impl Into<String>) -> Self {
        Self::Topic(topic.into())
    }

    /// Convert to tag format.
    pub fn to_tag(&self) -> Vec<String> {
        match self {
            Self::Event { id, relay } => {
                let mut tag = vec!["e".to_string(), id.clone()];
                if let Some(r) = relay {
                    tag.push(r.clone());
                }
                tag
            }
            Self::Pubkey { pubkey, relay } => {
                let mut tag = vec!["p".to_string(), pubkey.clone()];
                if let Some(r) = relay {
                    tag.push(r.clone());
                }
                tag
            }
            Self::Address { address, relay } => {
                let mut tag = vec!["a".to_string(), address.clone()];
                if let Some(r) = relay {
                    tag.push(r.clone());
                }
                tag
            }
            Self::Relay(url) => vec!["r".to_string(), url.clone()],
            Self::Topic(topic) => vec!["t".to_string(), topic.clone()],
        }
    }
}

/// A label event (kind 1985).
///
/// Used to attach labels to events, pubkeys, relays, or topics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelEvent {
    /// Labels to apply
    pub labels: Vec<Label>,

    /// Targets being labeled
    pub targets: Vec<LabelTarget>,

    /// Optional explanation or discussion
    pub content: String,
}

impl LabelEvent {
    /// Create a new label event.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip32::{Label, LabelEvent, LabelTarget};
    ///
    /// let labels = vec![Label::new("approve", "nip28.moderation")];
    /// let targets = vec![LabelTarget::event("event-id", Some("wss://relay.example.com"))];
    /// let event = LabelEvent::new(labels, targets);
    /// ```
    pub fn new(labels: Vec<Label>, targets: Vec<LabelTarget>) -> Self {
        Self {
            labels,
            targets,
            content: String::new(),
        }
    }

    /// Set the content (explanation).
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }

    /// Set the content (builder pattern).
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Add a label.
    pub fn add_label(&mut self, label: Label) {
        self.labels.push(label);
    }

    /// Add a target.
    pub fn add_target(&mut self, target: LabelTarget) {
        self.targets.push(target);
    }

    /// Get all unique namespaces used by labels.
    pub fn namespaces(&self) -> Vec<String> {
        let mut namespaces: Vec<String> = self
            .labels
            .iter()
            .filter_map(|l| l.namespace.clone())
            .collect();
        namespaces.sort();
        namespaces.dedup();
        namespaces
    }

    /// Check if this event uses a single namespace (recommended).
    pub fn is_single_namespace(&self) -> bool {
        let namespaces = self.namespaces();
        namespaces.len() <= 1
    }

    /// Validate the label event.
    pub fn validate(&self) -> Result<(), Nip32Error> {
        if self.targets.is_empty() {
            return Err(Nip32Error::NoTargets);
        }

        // Validate all labels
        for label in &self.labels {
            label.validate()?;
        }

        // Check that labels with namespaces have matching marks
        let namespaces: Vec<&str> = self
            .labels
            .iter()
            .filter_map(|l| l.namespace.as_deref())
            .collect();

        for label in &self.labels {
            if let Some(ns) = &label.namespace
                && !namespaces.contains(&ns.as_str())
            {
                return Err(Nip32Error::MissingNamespaceMark);
            }
        }

        Ok(())
    }

    /// Convert to Nostr event tags.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add namespace tags (L)
        let namespaces = self.namespaces();
        for namespace in namespaces {
            tags.push(vec!["L".to_string(), namespace]);
        }

        // Add label tags (l)
        for label in &self.labels {
            let mut tag = vec!["l".to_string(), label.value.clone()];
            if let Some(ns) = &label.namespace {
                tag.push(ns.clone());
            }
            tags.push(tag);
        }

        // Add target tags
        for target in &self.targets {
            tags.push(target.to_tag());
        }

        tags
    }
}

/// Self-reporting labels for non-1985 events.
///
/// Any event can include L and l tags to label itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfLabel {
    /// Labels applied to this event by its author
    pub labels: Vec<Label>,
}

impl SelfLabel {
    /// Create a new self-label.
    pub fn new(labels: Vec<Label>) -> Self {
        Self { labels }
    }

    /// Create a single self-label.
    pub fn single(label: Label) -> Self {
        Self {
            labels: vec![label],
        }
    }

    /// Add a label.
    pub fn add_label(&mut self, label: Label) {
        self.labels.push(label);
    }

    /// Convert to tags for inclusion in any event.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add namespace tags (L)
        let mut namespaces: Vec<String> = self
            .labels
            .iter()
            .filter_map(|l| l.namespace.clone())
            .collect();
        namespaces.sort();
        namespaces.dedup();

        for namespace in namespaces {
            tags.push(vec!["L".to_string(), namespace]);
        }

        // Add label tags (l)
        for label in &self.labels {
            let mut tag = vec!["l".to_string(), label.value.clone()];
            if let Some(ns) = &label.namespace {
                tag.push(ns.clone());
            }
            tags.push(tag);
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_label_kind() {
        assert!(is_label_kind(1985));
        assert!(!is_label_kind(1));
        assert!(!is_label_kind(1984));
    }

    #[test]
    fn test_label_new() {
        let label = Label::new("MIT", "license");
        assert_eq!(label.value, "MIT");
        assert_eq!(label.namespace, Some("license".to_string()));
        assert!(!label.is_tag_association());
        assert_eq!(label.effective_namespace(), "license");
    }

    #[test]
    fn test_label_ugc() {
        let label = Label::ugc("spam");
        assert_eq!(label.value, "spam");
        assert_eq!(label.namespace, None);
        assert_eq!(label.effective_namespace(), "ugc");
    }

    #[test]
    fn test_label_tag_association() {
        let label = Label::new("permies", "#t");
        assert!(label.is_tag_association());
    }

    #[test]
    fn test_label_validate() {
        let label = Label::new("MIT", "license");
        assert!(label.validate().is_ok());

        let label = Label::new("", "license");
        assert!(label.validate().is_err());

        let label = Label::new("MIT", "");
        assert!(label.validate().is_err());
    }

    #[test]
    fn test_label_target_event() {
        let target = LabelTarget::event("event-id", Some("wss://relay.example.com"));
        let tag = target.to_tag();
        assert_eq!(tag, vec!["e", "event-id", "wss://relay.example.com"]);

        let target = LabelTarget::event("event-id", None::<String>);
        let tag = target.to_tag();
        assert_eq!(tag, vec!["e", "event-id"]);
    }

    #[test]
    fn test_label_target_pubkey() {
        let target = LabelTarget::pubkey("pubkey-hex", Some("wss://relay.example.com"));
        let tag = target.to_tag();
        assert_eq!(tag, vec!["p", "pubkey-hex", "wss://relay.example.com"]);
    }

    #[test]
    fn test_label_target_address() {
        let target = LabelTarget::address("30023:pubkey:d-tag", None::<String>);
        let tag = target.to_tag();
        assert_eq!(tag, vec!["a", "30023:pubkey:d-tag"]);
    }

    #[test]
    fn test_label_target_relay() {
        let target = LabelTarget::relay("wss://relay.example.com");
        let tag = target.to_tag();
        assert_eq!(tag, vec!["r", "wss://relay.example.com"]);
    }

    #[test]
    fn test_label_target_topic() {
        let target = LabelTarget::topic("bitcoin");
        let tag = target.to_tag();
        assert_eq!(tag, vec!["t", "bitcoin"]);
    }

    #[test]
    fn test_label_event_new() {
        let labels = vec![Label::new("approve", "nip28.moderation")];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets);

        assert_eq!(event.labels.len(), 1);
        assert_eq!(event.targets.len(), 1);
        assert_eq!(event.content, "");
    }

    #[test]
    fn test_label_event_with_content() {
        let labels = vec![Label::new("approve", "nip28.moderation")];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets).with_content("This is appropriate");

        assert_eq!(event.content, "This is appropriate");
    }

    #[test]
    fn test_label_event_add_label() {
        let labels = vec![Label::new("approve", "nip28.moderation")];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let mut event = LabelEvent::new(labels, targets);

        event.add_label(Label::ugc("good"));
        assert_eq!(event.labels.len(), 2);
    }

    #[test]
    fn test_label_event_namespaces() {
        let labels = vec![
            Label::new("approve", "nip28.moderation"),
            Label::new("MIT", "license"),
            Label::new("reject", "nip28.moderation"),
        ];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets);

        let namespaces = event.namespaces();
        assert_eq!(namespaces.len(), 2);
        assert!(namespaces.contains(&"license".to_string()));
        assert!(namespaces.contains(&"nip28.moderation".to_string()));
    }

    #[test]
    fn test_label_event_is_single_namespace() {
        let labels = vec![
            Label::new("approve", "nip28.moderation"),
            Label::new("reject", "nip28.moderation"),
        ];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets);
        assert!(event.is_single_namespace());

        let labels = vec![
            Label::new("approve", "nip28.moderation"),
            Label::new("MIT", "license"),
        ];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets);
        assert!(!event.is_single_namespace());
    }

    #[test]
    fn test_label_event_validate() {
        let labels = vec![Label::new("approve", "nip28.moderation")];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets);
        assert!(event.validate().is_ok());

        // No targets
        let labels = vec![Label::new("approve", "nip28.moderation")];
        let event = LabelEvent::new(labels, vec![]);
        assert!(event.validate().is_err());

        // Invalid label
        let labels = vec![Label::new("", "nip28.moderation")];
        let targets = vec![LabelTarget::event("event-id", None::<String>)];
        let event = LabelEvent::new(labels, targets);
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_label_event_to_tags() {
        let labels = vec![
            Label::new("approve", "nip28.moderation"),
            Label::new("MIT", "license"),
        ];
        let targets = vec![
            LabelTarget::event("event-id", Some("wss://relay.example.com")),
            LabelTarget::pubkey("pubkey-hex", None::<String>),
        ];
        let event = LabelEvent::new(labels, targets);

        let tags = event.to_tags();

        // Should have L tags for namespaces
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "L" && tag[1] == "license")
        );
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "L" && tag[1] == "nip28.moderation")
        );

        // Should have l tags for labels
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "l"
            && tag[1] == "approve"
            && tag[2] == "nip28.moderation"));
        assert!(
            tags.iter().any(|tag| tag.len() == 3
                && tag[0] == "l"
                && tag[1] == "MIT"
                && tag[2] == "license")
        );

        // Should have target tags
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "e"
            && tag[1] == "event-id"
            && tag[2] == "wss://relay.example.com"));
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "p" && tag[1] == "pubkey-hex")
        );
    }

    #[test]
    fn test_label_event_tag_association() {
        let labels = vec![Label::new("permies", "#t")];
        let targets = vec![
            LabelTarget::pubkey("pubkey1", None::<String>),
            LabelTarget::pubkey("pubkey2", None::<String>),
        ];
        let event = LabelEvent::new(labels, targets);

        let tags = event.to_tags();

        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "L" && tag[1] == "#t")
        );
        assert!(
            tags.iter().any(|tag| tag.len() == 3
                && tag[0] == "l"
                && tag[1] == "permies"
                && tag[2] == "#t")
        );
    }

    #[test]
    fn test_self_label_new() {
        let labels = vec![Label::new("IT-MI", "ISO-3166-2")];
        let self_label = SelfLabel::new(labels);
        assert_eq!(self_label.labels.len(), 1);
    }

    #[test]
    fn test_self_label_single() {
        let label = Label::new("en", "ISO-639-1");
        let self_label = SelfLabel::single(label);
        assert_eq!(self_label.labels.len(), 1);
    }

    #[test]
    fn test_self_label_to_tags() {
        let labels = vec![
            Label::new("IT-MI", "ISO-3166-2"),
            Label::new("en", "ISO-639-1"),
        ];
        let self_label = SelfLabel::new(labels);

        let tags = self_label.to_tags();

        // Should have L tags
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "L" && tag[1] == "ISO-3166-2")
        );
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "L" && tag[1] == "ISO-639-1")
        );

        // Should have l tags
        assert!(tags.iter().any(|tag| tag.len() == 3
            && tag[0] == "l"
            && tag[1] == "IT-MI"
            && tag[2] == "ISO-3166-2"));
        assert!(
            tags.iter().any(|tag| tag.len() == 3
                && tag[0] == "l"
                && tag[1] == "en"
                && tag[2] == "ISO-639-1")
        );
    }

    #[test]
    fn test_self_label_ugc() {
        let labels = vec![Label::ugc("bitcoin"), Label::ugc("news")];
        let self_label = SelfLabel::new(labels);

        let tags = self_label.to_tags();

        // UGC labels should not have L tags (namespace is implicit)
        assert!(!tags.iter().any(|tag| tag[0] == "L"));

        // Should have l tags without namespace
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "l" && tag[1] == "bitcoin")
        );
        assert!(
            tags.iter()
                .any(|tag| tag.len() == 2 && tag[0] == "l" && tag[1] == "news")
        );
    }
}
