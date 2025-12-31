//! NIP-16: Event Treatment (Ephemeral and Replaceable Events)
//!
//! **DEPRECATED:** This NIP has been moved to NIP-01.
//!
//! NIP-16 originally defined how different categories of events should be handled
//! by relays based on their kind numbers. This functionality is now part of the
//! core Nostr protocol specification (NIP-01).
//!
//! ## Event Categories
//!
//! Events are classified into four categories based on their kind number:
//!
//! ### Regular Events (kinds: 1-999 except 0 and 3, 1000-9999)
//! All events are stored by relays with no replacement logic.
//!
//! ### Replaceable Events (kinds: 0, 3, 10000-19999)
//! For each combination of pubkey and kind, only the latest event MUST be stored by relays.
//! When timestamps match, the event with the lowest lexical ID is retained.
//!
//! ### Ephemeral Events (kinds: 20000-29999)
//! Events that are not expected to be stored by relays. They are broadcast to
//! connected clients with matching filters but not persisted.
//!
//! ### Addressable/Parameterized Replaceable Events (kinds: 30000-39999)
//! Identified by combination of kind, pubkey, and "d" tag value. Only the latest
//! event per unique identifier combination is retained.
//!
//! ## Usage
//!
//! ```
//! use nostr_core::nip16::{is_ephemeral, is_replaceable, is_addressable};
//!
//! let kind = 20001; // Ephemeral event kind
//! assert!(is_ephemeral(kind));
//! assert!(!is_replaceable(kind));
//! ```

use thiserror::Error;

/// Errors that can occur during NIP-16 operations.
#[derive(Debug, Error)]
pub enum Nip16Error {
    #[error("invalid event kind for operation")]
    InvalidKind,
}

/// Check if an event kind is ephemeral (20000-29999).
///
/// Ephemeral events are not expected to be stored by relays.
///
/// # Example
///
/// ```
/// use nostr_core::nip16::is_ephemeral;
///
/// assert!(is_ephemeral(20000));
/// assert!(is_ephemeral(25000));
/// assert!(is_ephemeral(29999));
/// assert!(!is_ephemeral(30000));
/// assert!(!is_ephemeral(1));
/// ```
pub fn is_ephemeral(kind: u16) -> bool {
    (20000..30000).contains(&kind)
}

/// Check if an event kind is replaceable (0, 3, 10000-19999).
///
/// For replaceable events, only the latest event per pubkey+kind combination
/// should be stored by relays.
///
/// # Example
///
/// ```
/// use nostr_core::nip16::is_replaceable;
///
/// assert!(is_replaceable(0));    // Metadata
/// assert!(is_replaceable(3));    // Contacts
/// assert!(is_replaceable(10000));
/// assert!(is_replaceable(15000));
/// assert!(is_replaceable(19999));
/// assert!(!is_replaceable(1));   // Regular note
/// assert!(!is_replaceable(20000)); // Ephemeral
/// ```
pub fn is_replaceable(kind: u16) -> bool {
    kind == 0 || kind == 3 || (10000..20000).contains(&kind)
}

/// Check if an event kind is addressable/parameterized replaceable (30000-39999).
///
/// Addressable events are identified by pubkey + kind + d-tag combination.
///
/// # Example
///
/// ```
/// use nostr_core::nip16::is_addressable;
///
/// assert!(is_addressable(30000));
/// assert!(is_addressable(35000));
/// assert!(is_addressable(39999));
/// assert!(!is_addressable(29999));
/// assert!(!is_addressable(40000));
/// ```
pub fn is_addressable(kind: u16) -> bool {
    (30000..40000).contains(&kind)
}

/// Check if an event kind is regular (not ephemeral, replaceable, or addressable).
///
/// Regular events are all events that don't fall into the other categories.
///
/// # Example
///
/// ```
/// use nostr_core::nip16::is_regular;
///
/// assert!(is_regular(1));      // Text note
/// assert!(is_regular(2));      // Recommend relay
/// assert!(is_regular(1000));
/// assert!(is_regular(9999));
/// assert!(!is_regular(0));     // Replaceable
/// assert!(!is_regular(20000)); // Ephemeral
/// assert!(!is_regular(30000)); // Addressable
/// ```
pub fn is_regular(kind: u16) -> bool {
    !is_ephemeral(kind) && !is_replaceable(kind) && !is_addressable(kind)
}

/// Get the event category name for a given kind.
///
/// # Example
///
/// ```
/// use nostr_core::nip16::get_event_category;
///
/// assert_eq!(get_event_category(1), "regular");
/// assert_eq!(get_event_category(0), "replaceable");
/// assert_eq!(get_event_category(20001), "ephemeral");
/// assert_eq!(get_event_category(30023), "addressable");
/// ```
pub fn get_event_category(kind: u16) -> &'static str {
    if is_ephemeral(kind) {
        "ephemeral"
    } else if is_addressable(kind) {
        "addressable"
    } else if is_replaceable(kind) {
        "replaceable"
    } else {
        "regular"
    }
}

/// Event category classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCategory {
    /// Regular events (stored without replacement)
    Regular,
    /// Replaceable events (only latest per pubkey+kind)
    Replaceable,
    /// Ephemeral events (not stored)
    Ephemeral,
    /// Addressable/parameterized replaceable (latest per pubkey+kind+d-tag)
    Addressable,
}

impl EventCategory {
    /// Classify an event kind into its category.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip16::EventCategory;
    ///
    /// assert_eq!(EventCategory::from_kind(1), EventCategory::Regular);
    /// assert_eq!(EventCategory::from_kind(0), EventCategory::Replaceable);
    /// assert_eq!(EventCategory::from_kind(20000), EventCategory::Ephemeral);
    /// assert_eq!(EventCategory::from_kind(30023), EventCategory::Addressable);
    /// ```
    pub fn from_kind(kind: u16) -> Self {
        if is_ephemeral(kind) {
            Self::Ephemeral
        } else if is_addressable(kind) {
            Self::Addressable
        } else if is_replaceable(kind) {
            Self::Replaceable
        } else {
            Self::Regular
        }
    }

    /// Check if events of this category should be stored by relays.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip16::EventCategory;
    ///
    /// assert!(EventCategory::Regular.should_store());
    /// assert!(EventCategory::Replaceable.should_store());
    /// assert!(!EventCategory::Ephemeral.should_store());
    /// assert!(EventCategory::Addressable.should_store());
    /// ```
    pub fn should_store(&self) -> bool {
        !matches!(self, Self::Ephemeral)
    }

    /// Check if events of this category have replacement logic.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr_core::nip16::EventCategory;
    ///
    /// assert!(!EventCategory::Regular.has_replacement());
    /// assert!(EventCategory::Replaceable.has_replacement());
    /// assert!(!EventCategory::Ephemeral.has_replacement());
    /// assert!(EventCategory::Addressable.has_replacement());
    /// ```
    pub fn has_replacement(&self) -> bool {
        matches!(self, Self::Replaceable | Self::Addressable)
    }

    /// Get the category name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Replaceable => "replaceable",
            Self::Ephemeral => "ephemeral",
            Self::Addressable => "addressable",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ephemeral() {
        // Ephemeral range
        assert!(is_ephemeral(20000));
        assert!(is_ephemeral(25000));
        assert!(is_ephemeral(29999));

        // Not ephemeral
        assert!(!is_ephemeral(19999));
        assert!(!is_ephemeral(30000));
        assert!(!is_ephemeral(1));
        assert!(!is_ephemeral(0));
    }

    #[test]
    fn test_is_replaceable() {
        // Replaceable special kinds
        assert!(is_replaceable(0));
        assert!(is_replaceable(3));

        // Replaceable range
        assert!(is_replaceable(10000));
        assert!(is_replaceable(15000));
        assert!(is_replaceable(19999));

        // Not replaceable
        assert!(!is_replaceable(1));
        assert!(!is_replaceable(2));
        assert!(!is_replaceable(9999));
        assert!(!is_replaceable(20000));
        assert!(!is_replaceable(30000));
    }

    #[test]
    fn test_is_addressable() {
        // Addressable range
        assert!(is_addressable(30000));
        assert!(is_addressable(35000));
        assert!(is_addressable(39999));

        // Not addressable
        assert!(!is_addressable(29999));
        assert!(!is_addressable(40000));
        assert!(!is_addressable(1));
        assert!(!is_addressable(0));
    }

    #[test]
    fn test_is_regular() {
        // Regular events
        assert!(is_regular(1));
        assert!(is_regular(2));
        assert!(is_regular(4));
        assert!(is_regular(1000));
        assert!(is_regular(9999));

        // Not regular
        assert!(!is_regular(0)); // Replaceable
        assert!(!is_regular(3)); // Replaceable
        assert!(!is_regular(10000)); // Replaceable
        assert!(!is_regular(20000)); // Ephemeral
        assert!(!is_regular(30000)); // Addressable
    }

    #[test]
    fn test_get_event_category() {
        assert_eq!(get_event_category(1), "regular");
        assert_eq!(get_event_category(0), "replaceable");
        assert_eq!(get_event_category(3), "replaceable");
        assert_eq!(get_event_category(10000), "replaceable");
        assert_eq!(get_event_category(20000), "ephemeral");
        assert_eq!(get_event_category(25000), "ephemeral");
        assert_eq!(get_event_category(30000), "addressable");
        assert_eq!(get_event_category(35000), "addressable");
    }

    #[test]
    fn test_event_category_from_kind() {
        assert_eq!(EventCategory::from_kind(1), EventCategory::Regular);
        assert_eq!(EventCategory::from_kind(0), EventCategory::Replaceable);
        assert_eq!(EventCategory::from_kind(3), EventCategory::Replaceable);
        assert_eq!(EventCategory::from_kind(10000), EventCategory::Replaceable);
        assert_eq!(EventCategory::from_kind(20000), EventCategory::Ephemeral);
        assert_eq!(EventCategory::from_kind(30000), EventCategory::Addressable);
    }

    #[test]
    fn test_event_category_should_store() {
        assert!(EventCategory::Regular.should_store());
        assert!(EventCategory::Replaceable.should_store());
        assert!(!EventCategory::Ephemeral.should_store());
        assert!(EventCategory::Addressable.should_store());
    }

    #[test]
    fn test_event_category_has_replacement() {
        assert!(!EventCategory::Regular.has_replacement());
        assert!(EventCategory::Replaceable.has_replacement());
        assert!(!EventCategory::Ephemeral.has_replacement());
        assert!(EventCategory::Addressable.has_replacement());
    }

    #[test]
    fn test_event_category_as_str() {
        assert_eq!(EventCategory::Regular.as_str(), "regular");
        assert_eq!(EventCategory::Replaceable.as_str(), "replaceable");
        assert_eq!(EventCategory::Ephemeral.as_str(), "ephemeral");
        assert_eq!(EventCategory::Addressable.as_str(), "addressable");
    }

    #[test]
    fn test_boundary_cases() {
        // Test boundaries between categories
        assert!(is_regular(9999));
        assert!(is_replaceable(10000));

        assert!(is_replaceable(19999));
        assert!(is_ephemeral(20000));

        assert!(is_ephemeral(29999));
        assert!(is_addressable(30000));

        assert!(is_addressable(39999));
        assert!(is_regular(40000));
    }

    #[test]
    fn test_common_kinds() {
        // Test some common event kinds
        assert_eq!(EventCategory::from_kind(0), EventCategory::Replaceable); // Metadata
        assert_eq!(EventCategory::from_kind(1), EventCategory::Regular); // Text note
        assert_eq!(EventCategory::from_kind(3), EventCategory::Replaceable); // Contacts
        assert_eq!(EventCategory::from_kind(4), EventCategory::Regular); // Encrypted DM
        assert_eq!(EventCategory::from_kind(5), EventCategory::Regular); // Deletion
        assert_eq!(EventCategory::from_kind(7), EventCategory::Regular); // Reaction

        // Common replaceable kinds
        assert_eq!(EventCategory::from_kind(10002), EventCategory::Replaceable); // Relay list

        // Common ephemeral kinds
        assert_eq!(EventCategory::from_kind(20001), EventCategory::Ephemeral); // Ephemeral

        // Common addressable kinds
        assert_eq!(EventCategory::from_kind(30023), EventCategory::Addressable); // Long-form
        assert_eq!(EventCategory::from_kind(30078), EventCategory::Addressable); // App data
    }
}
