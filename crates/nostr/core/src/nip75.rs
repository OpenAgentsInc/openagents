//! NIP-75: Zap Goals
//!
//! Defines events for creating fundraising goals where users can contribute
//! by zapping the goal event.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/75.md>

use crate::Event;
use thiserror::Error;

/// Event kind for zap goals
pub const ZAP_GOAL_KIND: u16 = 9041;

/// Tag name for goal references
pub const GOAL_TAG: &str = "goal";

/// Tag name for target amount
pub const AMOUNT_TAG: &str = "amount";

/// Tag name for relays
pub const RELAYS_TAG: &str = "relays";

/// Tag name for closed_at timestamp
pub const CLOSED_AT_TAG: &str = "closed_at";

/// Tag name for image
pub const IMAGE_TAG: &str = "image";

/// Tag name for summary
pub const SUMMARY_TAG: &str = "summary";

/// Errors that can occur during NIP-75 operations
#[derive(Debug, Error)]
pub enum Nip75Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("invalid amount: {0}")]
    InvalidAmount(String),
}

/// A zap goal event (kind 9041)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZapGoal {
    pub event: Event,
    pub amount: u64,
    pub relays: Vec<String>,
    pub closed_at: Option<u64>,
    pub image: Option<String>,
    pub summary: Option<String>,
}

impl ZapGoal {
    /// Create a zap goal from an event
    pub fn from_event(event: Event) -> Result<Self, Nip75Error> {
        if event.kind != ZAP_GOAL_KIND {
            return Err(Nip75Error::InvalidKind {
                expected: ZAP_GOAL_KIND,
                actual: event.kind,
            });
        }

        // Find amount tag (required)
        let mut amount = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == AMOUNT_TAG && tag.len() > 1 {
                amount = Some(
                    tag[1]
                        .parse::<u64>()
                        .map_err(|_| Nip75Error::InvalidAmount(tag[1].clone()))?,
                );
                break;
            }
        }

        let amount = amount.ok_or_else(|| Nip75Error::MissingTag("amount".to_string()))?;

        // Find relays tag (required)
        let mut relays = Vec::new();
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == RELAYS_TAG {
                // Skip the first element (tag name) and collect the rest as relay URLs
                for i in 1..tag.len() {
                    relays.push(tag[i].clone());
                }
                break;
            }
        }

        if relays.is_empty() {
            return Err(Nip75Error::MissingTag("relays".to_string()));
        }

        // Find closed_at tag (optional)
        let mut closed_at = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == CLOSED_AT_TAG && tag.len() > 1 {
                if let Ok(timestamp) = tag[1].parse::<u64>() {
                    closed_at = Some(timestamp);
                }
                break;
            }
        }

        // Find image tag (optional)
        let mut image = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == IMAGE_TAG && tag.len() > 1 {
                image = Some(tag[1].clone());
                break;
            }
        }

        // Find summary tag (optional)
        let mut summary = None;
        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == SUMMARY_TAG && tag.len() > 1 {
                summary = Some(tag[1].clone());
                break;
            }
        }

        Ok(Self {
            event,
            amount,
            relays,
            closed_at,
            image,
            summary,
        })
    }

    /// Get the target amount in millisats
    pub fn get_amount(&self) -> u64 {
        self.amount
    }

    /// Get the list of relays for zap tallying
    pub fn get_relays(&self) -> &[String] {
        &self.relays
    }

    /// Get the closing timestamp (if set)
    pub fn get_closed_at(&self) -> Option<u64> {
        self.closed_at
    }

    /// Get the image URL (if set)
    pub fn get_image(&self) -> Option<&str> {
        self.image.as_deref()
    }

    /// Get the summary (if set)
    pub fn get_summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Get the description from the event content
    pub fn get_description(&self) -> &str {
        &self.event.content
    }

    /// Get the author's public key
    pub fn get_author(&self) -> &str {
        &self.event.pubkey
    }

    /// Get the creation timestamp
    pub fn get_created_at(&self) -> u64 {
        self.event.created_at
    }

    /// Check if the goal is closed at a given timestamp
    pub fn is_closed(&self, timestamp: u64) -> bool {
        if let Some(closed_at) = self.closed_at {
            timestamp > closed_at
        } else {
            false
        }
    }

    /// Check if the goal is currently closed
    pub fn is_closed_now(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.is_closed(now)
    }

    /// Validate the zap goal structure
    pub fn validate(&self) -> Result<(), Nip75Error> {
        if self.event.kind != ZAP_GOAL_KIND {
            return Err(Nip75Error::InvalidKind {
                expected: ZAP_GOAL_KIND,
                actual: self.event.kind,
            });
        }

        if self.relays.is_empty() {
            return Err(Nip75Error::MissingTag("relays".to_string()));
        }

        Ok(())
    }
}

/// Extract goal reference from an event (returns goal event ID and optional relay hint)
pub fn get_goal_reference(event: &Event) -> Option<(String, Option<String>)> {
    for tag in &event.tags {
        if !tag.is_empty() && tag[0] == GOAL_TAG && tag.len() > 1 {
            let goal_id = tag[1].clone();
            let relay_hint = if tag.len() > 2 {
                Some(tag[2].clone())
            } else {
                None
            };
            return Some((goal_id, relay_hint));
        }
    }
    None
}

/// Add a goal reference tag to an event
pub fn add_goal_tag(tags: &mut Vec<Vec<String>>, goal_id: String, relay_hint: Option<String>) {
    let mut tag = vec![GOAL_TAG.to_string(), goal_id];
    if let Some(hint) = relay_hint {
        tag.push(hint);
    }
    tags.push(tag);
}

/// Check if an event kind is a zap goal
pub fn is_zap_goal_kind(kind: u16) -> bool {
    kind == ZAP_GOAL_KIND
}

/// Helper function to create amount tag
pub fn create_amount_tag(amount: u64) -> Vec<String> {
    vec![AMOUNT_TAG.to_string(), amount.to_string()]
}

/// Helper function to create relays tag
pub fn create_relays_tag(relays: Vec<String>) -> Vec<String> {
    let mut tag = vec![RELAYS_TAG.to_string()];
    tag.extend(relays);
    tag
}

/// Helper function to create closed_at tag
pub fn create_closed_at_tag(timestamp: u64) -> Vec<String> {
    vec![CLOSED_AT_TAG.to_string(), timestamp.to_string()]
}

/// Helper function to create image tag
pub fn create_image_tag(url: String) -> Vec<String> {
    vec![IMAGE_TAG.to_string(), url]
}

/// Helper function to create summary tag
pub fn create_summary_tag(summary: String) -> Vec<String> {
    vec![SUMMARY_TAG.to_string(), summary]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_zap_goal_event(
        amount: u64,
        relays: Vec<String>,
        description: &str,
        closed_at: Option<u64>,
        image: Option<String>,
        summary: Option<String>,
    ) -> Event {
        let mut tags = vec![create_amount_tag(amount), create_relays_tag(relays)];

        if let Some(ts) = closed_at {
            tags.push(create_closed_at_tag(ts));
        }

        if let Some(img) = image {
            tags.push(create_image_tag(img));
        }

        if let Some(sum) = summary {
            tags.push(create_summary_tag(sum));
        }

        Event {
            id: "goal_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: ZAP_GOAL_KIND,
            tags,
            content: description.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_zap_goal_from_event_minimal() {
        let event = create_test_zap_goal_event(
            210000,
            vec![
                "wss://alicerelay.example.com".to_string(),
                "wss://bobrelay.example.com".to_string(),
            ],
            "Nostrasia travel expenses",
            None,
            None,
            None,
        );

        let goal = ZapGoal::from_event(event).unwrap();

        assert_eq!(goal.get_amount(), 210000);
        assert_eq!(goal.get_relays().len(), 2);
        assert_eq!(goal.get_relays()[0], "wss://alicerelay.example.com");
        assert_eq!(goal.get_relays()[1], "wss://bobrelay.example.com");
        assert_eq!(goal.get_description(), "Nostrasia travel expenses");
        assert!(goal.get_closed_at().is_none());
        assert!(goal.get_image().is_none());
        assert!(goal.get_summary().is_none());
    }

    #[test]
    fn test_zap_goal_with_all_fields() {
        let event = create_test_zap_goal_event(
            500000,
            vec!["wss://relay.example.com".to_string()],
            "Full description here",
            Some(1704067200),
            Some("https://example.com/image.jpg".to_string()),
            Some("Brief summary".to_string()),
        );

        let goal = ZapGoal::from_event(event).unwrap();

        assert_eq!(goal.get_amount(), 500000);
        assert_eq!(goal.get_relays().len(), 1);
        assert_eq!(goal.get_closed_at(), Some(1704067200));
        assert_eq!(goal.get_image(), Some("https://example.com/image.jpg"));
        assert_eq!(goal.get_summary(), Some("Brief summary"));
    }

    #[test]
    fn test_zap_goal_missing_amount() {
        let event = Event {
            id: "goal_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: ZAP_GOAL_KIND,
            tags: vec![create_relays_tag(vec![
                "wss://relay.example.com".to_string(),
            ])],
            content: "Description".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = ZapGoal::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip75Error::MissingTag(ref tag) if tag == "amount"
        ));
    }

    #[test]
    fn test_zap_goal_missing_relays() {
        let event = Event {
            id: "goal_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: ZAP_GOAL_KIND,
            tags: vec![create_amount_tag(210000)],
            content: "Description".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = ZapGoal::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip75Error::MissingTag(ref tag) if tag == "relays"
        ));
    }

    #[test]
    fn test_zap_goal_invalid_kind() {
        let mut event = create_test_zap_goal_event(
            210000,
            vec!["wss://relay.example.com".to_string()],
            "Description",
            None,
            None,
            None,
        );
        event.kind = 1;

        let result = ZapGoal::from_event(event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip75Error::InvalidKind {
                expected: ZAP_GOAL_KIND,
                actual: 1
            }
        ));
    }

    #[test]
    fn test_zap_goal_invalid_amount() {
        let event = Event {
            id: "goal_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: ZAP_GOAL_KIND,
            tags: vec![
                vec![AMOUNT_TAG.to_string(), "not_a_number".to_string()],
                create_relays_tag(vec!["wss://relay.example.com".to_string()]),
            ],
            content: "Description".to_string(),
            sig: "test_sig".to_string(),
        };

        let result = ZapGoal::from_event(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip75Error::InvalidAmount(_)));
    }

    #[test]
    fn test_zap_goal_validate() {
        let event = create_test_zap_goal_event(
            210000,
            vec!["wss://relay.example.com".to_string()],
            "Description",
            None,
            None,
            None,
        );

        let goal = ZapGoal::from_event(event).unwrap();
        assert!(goal.validate().is_ok());
    }

    #[test]
    fn test_zap_goal_get_author() {
        let event = create_test_zap_goal_event(
            210000,
            vec!["wss://relay.example.com".to_string()],
            "Description",
            None,
            None,
            None,
        );

        let goal = ZapGoal::from_event(event).unwrap();
        assert_eq!(goal.get_author(), "author_pubkey");
    }

    #[test]
    fn test_zap_goal_get_created_at() {
        let event = create_test_zap_goal_event(
            210000,
            vec!["wss://relay.example.com".to_string()],
            "Description",
            None,
            None,
            None,
        );

        let goal = ZapGoal::from_event(event).unwrap();
        assert_eq!(goal.get_created_at(), 1675642635);
    }

    #[test]
    fn test_zap_goal_is_closed() {
        let event = create_test_zap_goal_event(
            210000,
            vec!["wss://relay.example.com".to_string()],
            "Description",
            Some(1700000000),
            None,
            None,
        );

        let goal = ZapGoal::from_event(event).unwrap();
        assert!(!goal.is_closed(1699999999));
        assert!(goal.is_closed(1700000001));
    }

    #[test]
    fn test_zap_goal_is_not_closed_without_timestamp() {
        let event = create_test_zap_goal_event(
            210000,
            vec!["wss://relay.example.com".to_string()],
            "Description",
            None,
            None,
            None,
        );

        let goal = ZapGoal::from_event(event).unwrap();
        assert!(!goal.is_closed(u64::MAX));
    }

    #[test]
    fn test_get_goal_reference() {
        let event = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: 30023,
            tags: vec![vec![
                GOAL_TAG.to_string(),
                "goal_event_id".to_string(),
                "wss://relay.example.com".to_string(),
            ]],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        let (goal_id, relay_hint) = get_goal_reference(&event).unwrap();
        assert_eq!(goal_id, "goal_event_id");
        assert_eq!(relay_hint, Some("wss://relay.example.com".to_string()));
    }

    #[test]
    fn test_get_goal_reference_without_relay() {
        let event = Event {
            id: "event_id".to_string(),
            pubkey: "author_pubkey".to_string(),
            created_at: 1675642635,
            kind: 30023,
            tags: vec![vec![GOAL_TAG.to_string(), "goal_event_id".to_string()]],
            content: "Content".to_string(),
            sig: "test_sig".to_string(),
        };

        let (goal_id, relay_hint) = get_goal_reference(&event).unwrap();
        assert_eq!(goal_id, "goal_event_id");
        assert_eq!(relay_hint, None);
    }

    #[test]
    fn test_add_goal_tag() {
        let mut tags = Vec::new();
        add_goal_tag(
            &mut tags,
            "goal_event_id".to_string(),
            Some("wss://relay.example.com".to_string()),
        );

        assert_eq!(tags.len(), 1);
        assert_eq!(
            tags[0],
            vec![
                GOAL_TAG.to_string(),
                "goal_event_id".to_string(),
                "wss://relay.example.com".to_string()
            ]
        );
    }

    #[test]
    fn test_add_goal_tag_without_relay() {
        let mut tags = Vec::new();
        add_goal_tag(&mut tags, "goal_event_id".to_string(), None);

        assert_eq!(tags.len(), 1);
        assert_eq!(
            tags[0],
            vec![GOAL_TAG.to_string(), "goal_event_id".to_string()]
        );
    }

    #[test]
    fn test_is_zap_goal_kind() {
        assert!(is_zap_goal_kind(ZAP_GOAL_KIND));
        assert!(!is_zap_goal_kind(1));
        assert!(!is_zap_goal_kind(9735));
    }
}
