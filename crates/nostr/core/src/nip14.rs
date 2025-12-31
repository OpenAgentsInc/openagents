//! NIP-14: Subject Tag in Text Events
//!
//! This module implements NIP-14, which defines the use of subject tags in text events.
//! Similar to email, subjects provide a concise summary that can be displayed in threaded
//! message lists instead of showing the first few words of the message content.
//!
//! ## Usage
//!
//! - Subject tags are used in kind:1 (text note) events
//! - Subjects should generally be shorter than 80 characters
//! - When replying, clients should replicate the subject and may prepend "Re:"
//!
//! # Example
//!
//! ```
//! use nostr_core::nip14::{add_subject, get_subject, create_reply_subject};
//! use nostr_core::Event;
//!
//! # fn example(mut event: Event) {
//! // Add a subject to an event
//! add_subject(&mut event, "Important announcement");
//!
//! // Get the subject from an event
//! if let Some(subject) = get_subject(&event) {
//!     println!("Subject: {}", subject);
//! }
//!
//! // Create a reply subject
//! let reply_subject = create_reply_subject(&event);
//! # }
//! ```

use crate::nip01::Event;

/// Tag name for subject
pub const SUBJECT_TAG: &str = "subject";

/// Recommended maximum length for subjects (in characters)
pub const RECOMMENDED_MAX_LENGTH: usize = 80;

/// Reply prefix (prepended to subject when replying)
pub const REPLY_PREFIX: &str = "Re: ";

/// Add a subject tag to an event.
///
/// This adds or replaces the subject tag on the event.
pub fn add_subject(event: &mut Event, subject: impl Into<String>) {
    let subject = subject.into();

    // Remove existing subject tag if present
    event
        .tags
        .retain(|tag| !(tag.len() >= 1 && tag[0] == SUBJECT_TAG));

    // Add new subject tag
    event.tags.push(vec![SUBJECT_TAG.to_string(), subject]);
}

/// Get the subject from an event.
///
/// Returns None if the event has no subject tag.
pub fn get_subject(event: &Event) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == SUBJECT_TAG)
        .map(|tag| tag[1].clone())
}

/// Check if an event has a subject.
pub fn has_subject(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.len() >= 2 && tag[0] == SUBJECT_TAG)
}

/// Create a reply subject from an event's subject.
///
/// If the event has a subject, returns it with "Re: " prepended (unless it already starts with "Re:").
/// If the event has no subject, returns None.
pub fn create_reply_subject(event: &Event) -> Option<String> {
    get_subject(event).map(|subject| {
        if subject.starts_with(REPLY_PREFIX) || subject.to_lowercase().starts_with("re:") {
            subject
        } else {
            format!("{}{}", REPLY_PREFIX, subject)
        }
    })
}

/// Check if a subject exceeds the recommended maximum length.
///
/// Returns true if the subject is longer than RECOMMENDED_MAX_LENGTH characters.
pub fn is_subject_too_long(subject: &str) -> bool {
    subject.chars().count() > RECOMMENDED_MAX_LENGTH
}

/// Truncate a subject to the recommended maximum length.
///
/// Adds "..." at the end if truncated.
pub fn truncate_subject(subject: &str, max_length: usize) -> String {
    let chars: Vec<char> = subject.chars().collect();
    if chars.len() <= max_length {
        subject.to_string()
    } else if max_length >= 3 {
        chars[..max_length - 3].iter().collect::<String>() + "..."
    } else {
        chars[..max_length].iter().collect()
    }
}

/// Truncate a subject to the recommended maximum length (80 chars).
pub fn truncate_subject_recommended(subject: &str) -> String {
    truncate_subject(subject, RECOMMENDED_MAX_LENGTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_event() -> Event {
        Event {
            id: String::new(),
            kind: 1, // Text note
            pubkey: "pubkey123".to_string(),
            tags: vec![],
            content: "This is a test note".to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            sig: String::new(),
        }
    }

    #[test]
    fn test_add_subject() {
        let mut event = mock_event();

        add_subject(&mut event, "Test Subject");

        assert!(has_subject(&event));
        assert_eq!(get_subject(&event), Some("Test Subject".to_string()));
    }

    #[test]
    fn test_add_subject_replaces_existing() {
        let mut event = mock_event();

        add_subject(&mut event, "First Subject");
        add_subject(&mut event, "Second Subject");

        // Should only have one subject tag
        let subject_tags: Vec<_> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 1 && tag[0] == SUBJECT_TAG)
            .collect();

        assert_eq!(subject_tags.len(), 1);
        assert_eq!(get_subject(&event), Some("Second Subject".to_string()));
    }

    #[test]
    fn test_get_subject_no_subject() {
        let event = mock_event();
        assert_eq!(get_subject(&event), None);
        assert!(!has_subject(&event));
    }

    #[test]
    fn test_create_reply_subject() {
        let mut event = mock_event();

        add_subject(&mut event, "Original Subject");

        let reply_subject = create_reply_subject(&event).unwrap();
        assert_eq!(reply_subject, "Re: Original Subject");
    }

    #[test]
    fn test_create_reply_subject_already_has_re() {
        let mut event = mock_event();

        add_subject(&mut event, "Re: Original Subject");

        let reply_subject = create_reply_subject(&event).unwrap();
        // Should not add another "Re: "
        assert_eq!(reply_subject, "Re: Original Subject");
    }

    #[test]
    fn test_create_reply_subject_case_insensitive() {
        let mut event = mock_event();

        add_subject(&mut event, "re: Original Subject");

        let reply_subject = create_reply_subject(&event).unwrap();
        // Should not add another "Re: " even with different case
        assert_eq!(reply_subject, "re: Original Subject");
    }

    #[test]
    fn test_create_reply_subject_no_subject() {
        let event = mock_event();
        assert_eq!(create_reply_subject(&event), None);
    }

    #[test]
    fn test_is_subject_too_long() {
        let short_subject = "Short subject";
        let long_subject = "a".repeat(100);

        assert!(!is_subject_too_long(short_subject));
        assert!(is_subject_too_long(&long_subject));
        assert!(!is_subject_too_long(&"a".repeat(80))); // Exactly 80 is ok
        assert!(is_subject_too_long(&"a".repeat(81))); // 81 is too long
    }

    #[test]
    fn test_truncate_subject() {
        let long_subject = "This is a very long subject that exceeds the maximum length";

        let truncated = truncate_subject(long_subject, 20);
        assert_eq!(truncated.chars().count(), 20);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_subject_short() {
        let short_subject = "Short";

        let truncated = truncate_subject(short_subject, 20);
        assert_eq!(truncated, "Short");
    }

    #[test]
    fn test_truncate_subject_recommended() {
        let long_subject = "a".repeat(100);

        let truncated = truncate_subject_recommended(&long_subject);
        assert_eq!(truncated.chars().count(), 80);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_subject_exact_length() {
        let subject = "a".repeat(80);

        let truncated = truncate_subject_recommended(&subject);
        assert_eq!(truncated, subject);
        assert!(!truncated.ends_with("..."));
    }
}
