//! NIP-52: Calendar Events
//!
//! This NIP defines calendar events representing occurrences at specific moments
//! or between moments. Calendar events are addressable and deletable per NIP-09.
//!
//! ## Event Types
//!
//! - **Date-Based Calendar Event** (kind 31922): All-day or multi-day events
//! - **Time-Based Calendar Event** (kind 31923): Events with specific start/end times
//! - **Calendar** (kind 31924): Collection of calendar events
//! - **Calendar Event RSVP** (kind 31925): Response to a calendar event
//!
//! ## Example
//!
//! ```
//! use nostr::nip52::{DateBasedCalendarEvent, TimeBasedCalendarEvent, RsvpStatus};
//!
//! // Create a date-based event (e.g., vacation)
//! let vacation = DateBasedCalendarEvent {
//!     d: "vacation-2024".to_string(),
//!     title: "Summer Vacation".to_string(),
//!     start: "2024-07-01".to_string(),
//!     end: Some("2024-07-15".to_string()),
//!     summary: Some("Family trip to the beach".to_string()),
//!     ..Default::default()
//! };
//!
//! // Create a time-based event (e.g., meeting)
//! let meeting = TimeBasedCalendarEvent {
//!     d: "team-meeting".to_string(),
//!     title: "Weekly Team Sync".to_string(),
//!     start: 1686840000,
//!     end: Some(1686843600),
//!     start_tzid: Some("America/New_York".to_string()),
//!     ..Default::default()
//! };
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for date-based calendar events
pub const KIND_DATE_BASED_CALENDAR_EVENT: u16 = 31922;

/// Kind for time-based calendar events
pub const KIND_TIME_BASED_CALENDAR_EVENT: u16 = 31923;

/// Kind for calendar (collection of events)
pub const KIND_CALENDAR: u16 = 31924;

/// Kind for calendar event RSVP
pub const KIND_CALENDAR_EVENT_RSVP: u16 = 31925;

/// Errors that can occur during NIP-52 operations.
#[derive(Debug, Error)]
pub enum Nip52Error {
    #[error("invalid date format: {0}")]
    InvalidDateFormat(String),

    #[error("start must be before end")]
    StartAfterEnd,

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid RSVP status: {0}")]
    InvalidRsvpStatus(String),

    #[error("invalid free/busy status: {0}")]
    InvalidFreeBusyStatus(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Participant in a calendar event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    /// Pubkey of the participant (32-bytes hex)
    pub pubkey: String,
    /// Optional recommended relay URL
    pub relay: Option<String>,
    /// Role of the participant in the event
    pub role: Option<String>,
}

impl Participant {
    /// Create a new participant
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay: None,
            role: None,
        }
    }

    /// Set the relay URL
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }

    /// Set the role
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }
}

/// Date-based calendar event (kind 31922)
///
/// For all-day or multi-day events where time and time zone are not significant.
/// Examples: anniversaries, public holidays, vacation days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DateBasedCalendarEvent {
    /// Unique identifier (d tag)
    pub d: String,
    /// Title of the calendar event (required)
    pub title: String,
    /// Inclusive start date in ISO 8601 format (YYYY-MM-DD)
    pub start: String,
    /// Exclusive end date in ISO 8601 format (YYYY-MM-DD)
    /// If omitted, event ends on the same date as start
    pub end: Option<String>,
    /// Brief description of the calendar event
    pub summary: Option<String>,
    /// URL of an image to use for the event
    pub image: Option<String>,
    /// Locations of the calendar event
    pub locations: Vec<String>,
    /// Geohash to associate event with a searchable physical location
    pub geohash: Option<String>,
    /// Participants in the event
    pub participants: Vec<Participant>,
    /// Hashtags to categorize the event
    pub hashtags: Vec<String>,
    /// References/links to web pages, documents, etc.
    pub references: Vec<String>,
    /// Calendar references (a tags to kind 31924)
    pub calendar_refs: Vec<String>,
}

impl DateBasedCalendarEvent {
    /// Create a new date-based calendar event
    pub fn new(d: impl Into<String>, title: impl Into<String>, start: impl Into<String>) -> Self {
        Self {
            d: d.into(),
            title: title.into(),
            start: start.into(),
            ..Default::default()
        }
    }

    /// Validate the calendar event
    pub fn validate(&self) -> Result<(), Nip52Error> {
        if self.d.is_empty() {
            return Err(Nip52Error::MissingField("d".to_string()));
        }
        if self.title.is_empty() {
            return Err(Nip52Error::MissingField("title".to_string()));
        }
        if self.start.is_empty() {
            return Err(Nip52Error::MissingField("start".to_string()));
        }

        // Validate date format (basic check for YYYY-MM-DD)
        if !is_valid_date_format(&self.start) {
            return Err(Nip52Error::InvalidDateFormat(self.start.clone()));
        }

        if let Some(end) = &self.end {
            if !is_valid_date_format(end) {
                return Err(Nip52Error::InvalidDateFormat(end.clone()));
            }
            // Check that start < end
            if self.start >= *end {
                return Err(Nip52Error::StartAfterEnd);
            }
        }

        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.d.clone()],
            vec!["title".to_string(), self.title.clone()],
            vec!["start".to_string(), self.start.clone()],
        ];

        if let Some(end) = &self.end {
            tags.push(vec!["end".to_string(), end.clone()]);
        }

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        if let Some(image) = &self.image {
            tags.push(vec!["image".to_string(), image.clone()]);
        }

        for location in &self.locations {
            tags.push(vec!["location".to_string(), location.clone()]);
        }

        if let Some(geohash) = &self.geohash {
            tags.push(vec!["g".to_string(), geohash.clone()]);
        }

        for participant in &self.participants {
            let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
            if let Some(relay) = &participant.relay {
                tag.push(relay.clone());
            }
            if let Some(role) = &participant.role {
                if participant.relay.is_none() {
                    tag.push(String::new()); // Empty relay placeholder
                }
                tag.push(role.clone());
            }
            tags.push(tag);
        }

        for hashtag in &self.hashtags {
            tags.push(vec!["t".to_string(), hashtag.clone()]);
        }

        for reference in &self.references {
            tags.push(vec!["r".to_string(), reference.clone()]);
        }

        for calendar_ref in &self.calendar_refs {
            tags.push(vec!["a".to_string(), calendar_ref.clone()]);
        }

        tags
    }
}

/// Time-based calendar event (kind 31923)
///
/// For events that span between a start time and end time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TimeBasedCalendarEvent {
    /// Unique identifier (d tag)
    pub d: String,
    /// Title of the calendar event (required)
    pub title: String,
    /// Inclusive start Unix timestamp in seconds
    pub start: u64,
    /// Exclusive end Unix timestamp in seconds
    /// If omitted, the event ends instantaneously
    pub end: Option<u64>,
    /// Time zone of the start timestamp (IANA Time Zone Database)
    pub start_tzid: Option<String>,
    /// Time zone of the end timestamp (IANA Time Zone Database)
    /// If omitted and start_tzid is provided, uses start_tzid
    pub end_tzid: Option<String>,
    /// Brief description of the calendar event
    pub summary: Option<String>,
    /// URL of an image to use for the event
    pub image: Option<String>,
    /// Locations of the calendar event
    pub locations: Vec<String>,
    /// Geohash to associate event with a searchable physical location
    pub geohash: Option<String>,
    /// Participants in the event
    pub participants: Vec<Participant>,
    /// Hashtags to categorize the event
    pub hashtags: Vec<String>,
    /// References/links to web pages, documents, etc.
    pub references: Vec<String>,
    /// Calendar references (a tags to kind 31924)
    pub calendar_refs: Vec<String>,
}

impl TimeBasedCalendarEvent {
    /// Create a new time-based calendar event
    pub fn new(d: impl Into<String>, title: impl Into<String>, start: u64) -> Self {
        Self {
            d: d.into(),
            title: title.into(),
            start,
            ..Default::default()
        }
    }

    /// Validate the calendar event
    pub fn validate(&self) -> Result<(), Nip52Error> {
        if self.d.is_empty() {
            return Err(Nip52Error::MissingField("d".to_string()));
        }
        if self.title.is_empty() {
            return Err(Nip52Error::MissingField("title".to_string()));
        }

        // Check that start < end
        if let Some(end) = self.end
            && self.start >= end
        {
            return Err(Nip52Error::StartAfterEnd);
        }

        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.d.clone()],
            vec!["title".to_string(), self.title.clone()],
            vec!["start".to_string(), self.start.to_string()],
        ];

        if let Some(end) = self.end {
            tags.push(vec!["end".to_string(), end.to_string()]);
        }

        if let Some(start_tzid) = &self.start_tzid {
            tags.push(vec!["start_tzid".to_string(), start_tzid.clone()]);
        }

        if let Some(end_tzid) = &self.end_tzid {
            tags.push(vec!["end_tzid".to_string(), end_tzid.clone()]);
        }

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        if let Some(image) = &self.image {
            tags.push(vec!["image".to_string(), image.clone()]);
        }

        for location in &self.locations {
            tags.push(vec!["location".to_string(), location.clone()]);
        }

        if let Some(geohash) = &self.geohash {
            tags.push(vec!["g".to_string(), geohash.clone()]);
        }

        for participant in &self.participants {
            let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
            if let Some(relay) = &participant.relay {
                tag.push(relay.clone());
            }
            if let Some(role) = &participant.role {
                if participant.relay.is_none() {
                    tag.push(String::new()); // Empty relay placeholder
                }
                tag.push(role.clone());
            }
            tags.push(tag);
        }

        for hashtag in &self.hashtags {
            tags.push(vec!["t".to_string(), hashtag.clone()]);
        }

        for reference in &self.references {
            tags.push(vec!["r".to_string(), reference.clone()]);
        }

        for calendar_ref in &self.calendar_refs {
            tags.push(vec!["a".to_string(), calendar_ref.clone()]);
        }

        tags
    }
}

/// Calendar (kind 31924)
///
/// A collection of calendar events represented as a custom addressable list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Calendar {
    /// Unique identifier (d tag)
    pub d: String,
    /// Calendar title (required)
    pub title: String,
    /// References to calendar events (a tags)
    /// Format: `<kind>:<pubkey>:<d-identifier>`
    pub event_refs: Vec<CalendarEventRef>,
}

/// Reference to a calendar event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventRef {
    /// Event coordinates (e.g., "31922:pubkey:event-id")
    pub coordinates: String,
    /// Optional relay URL
    pub relay: Option<String>,
}

impl CalendarEventRef {
    /// Create a new calendar event reference
    pub fn new(coordinates: impl Into<String>) -> Self {
        Self {
            coordinates: coordinates.into(),
            relay: None,
        }
    }

    /// Set the relay URL
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }
}

impl Calendar {
    /// Create a new calendar
    pub fn new(d: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            d: d.into(),
            title: title.into(),
            event_refs: Vec::new(),
        }
    }

    /// Add an event reference to the calendar
    pub fn add_event(&mut self, event_ref: CalendarEventRef) {
        self.event_refs.push(event_ref);
    }

    /// Validate the calendar
    pub fn validate(&self) -> Result<(), Nip52Error> {
        if self.d.is_empty() {
            return Err(Nip52Error::MissingField("d".to_string()));
        }
        if self.title.is_empty() {
            return Err(Nip52Error::MissingField("title".to_string()));
        }

        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.d.clone()],
            vec!["title".to_string(), self.title.clone()],
        ];

        for event_ref in &self.event_refs {
            let mut tag = vec!["a".to_string(), event_ref.coordinates.clone()];
            if let Some(relay) = &event_ref.relay {
                tag.push(relay.clone());
            }
            tags.push(tag);
        }

        tags
    }
}

/// RSVP status for a calendar event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RsvpStatus {
    /// User has accepted the invitation
    Accepted,
    /// User has declined the invitation
    Declined,
    /// User's attendance is tentative
    Tentative,
}

impl RsvpStatus {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            RsvpStatus::Accepted => "accepted",
            RsvpStatus::Declined => "declined",
            RsvpStatus::Tentative => "tentative",
        }
    }

}

impl std::str::FromStr for RsvpStatus {
    type Err = Nip52Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "accepted" => Ok(RsvpStatus::Accepted),
            "declined" => Ok(RsvpStatus::Declined),
            "tentative" => Ok(RsvpStatus::Tentative),
            _ => Err(Nip52Error::InvalidRsvpStatus(s.to_string())),
        }
    }
}

/// Free/busy status for a calendar event RSVP
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FreeBusyStatus {
    /// User would be free during the event
    Free,
    /// User would be busy during the event
    Busy,
}

impl FreeBusyStatus {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            FreeBusyStatus::Free => "free",
            FreeBusyStatus::Busy => "busy",
        }
    }

}

impl std::str::FromStr for FreeBusyStatus {
    type Err = Nip52Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "free" => Ok(FreeBusyStatus::Free),
            "busy" => Ok(FreeBusyStatus::Busy),
            _ => Err(Nip52Error::InvalidFreeBusyStatus(s.to_string())),
        }
    }
}

/// Calendar event RSVP (kind 31925)
///
/// A response to a calendar event to indicate attendance intention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventRsvp {
    /// Unique identifier (d tag)
    pub d: String,
    /// Calendar event coordinates (a tag, required)
    /// Format: `<kind>:<pubkey>:<d-identifier>`
    pub event_coordinates: String,
    /// Optional relay URL for the event coordinates
    pub event_relay: Option<String>,
    /// Event ID of the specific calendar event revision (e tag, optional)
    pub event_id: Option<String>,
    /// Optional relay URL for the event ID
    pub event_id_relay: Option<String>,
    /// RSVP status (required)
    pub status: RsvpStatus,
    /// Free/busy status (optional, must be omitted if status is declined)
    pub free_busy: Option<FreeBusyStatus>,
    /// Pubkey of the calendar event author (p tag, optional)
    pub event_author: Option<String>,
    /// Optional relay URL for the event author
    pub event_author_relay: Option<String>,
}

impl CalendarEventRsvp {
    /// Create a new calendar event RSVP
    pub fn new(
        d: impl Into<String>,
        event_coordinates: impl Into<String>,
        status: RsvpStatus,
    ) -> Self {
        Self {
            d: d.into(),
            event_coordinates: event_coordinates.into(),
            event_relay: None,
            event_id: None,
            event_id_relay: None,
            status,
            free_busy: None,
            event_author: None,
            event_author_relay: None,
        }
    }

    /// Set the event ID
    pub fn with_event_id(mut self, event_id: impl Into<String>) -> Self {
        self.event_id = Some(event_id.into());
        self
    }

    /// Set the free/busy status
    pub fn with_free_busy(mut self, free_busy: FreeBusyStatus) -> Self {
        self.free_busy = Some(free_busy);
        self
    }

    /// Set the event author
    pub fn with_event_author(mut self, event_author: impl Into<String>) -> Self {
        self.event_author = Some(event_author.into());
        self
    }

    /// Validate the RSVP
    pub fn validate(&self) -> Result<(), Nip52Error> {
        if self.d.is_empty() {
            return Err(Nip52Error::MissingField("d".to_string()));
        }
        if self.event_coordinates.is_empty() {
            return Err(Nip52Error::MissingField("event_coordinates".to_string()));
        }

        // Free/busy must be omitted if status is declined
        if self.status == RsvpStatus::Declined && self.free_busy.is_some() {
            return Err(Nip52Error::Serialization(
                "free_busy must be omitted when status is declined".to_string(),
            ));
        }

        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.d.clone()]];

        // a tag (required)
        let mut a_tag = vec!["a".to_string(), self.event_coordinates.clone()];
        if let Some(relay) = &self.event_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        // e tag (optional)
        if let Some(event_id) = &self.event_id {
            let mut e_tag = vec!["e".to_string(), event_id.clone()];
            if let Some(relay) = &self.event_id_relay {
                e_tag.push(relay.clone());
            }
            tags.push(e_tag);
        }

        // status tag (required)
        tags.push(vec!["status".to_string(), self.status.as_str().to_string()]);

        // fb tag (optional)
        if let Some(free_busy) = &self.free_busy {
            tags.push(vec!["fb".to_string(), free_busy.as_str().to_string()]);
        }

        // p tag (optional)
        if let Some(event_author) = &self.event_author {
            let mut p_tag = vec!["p".to_string(), event_author.clone()];
            if let Some(relay) = &self.event_author_relay {
                p_tag.push(relay.clone());
            }
            tags.push(p_tag);
        }

        tags
    }
}

/// Check if a date string is in valid YYYY-MM-DD format
fn is_valid_date_format(date: &str) -> bool {
    if date.len() != 10 {
        return false;
    }

    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return false;
    }

    // Basic validation: YYYY-MM-DD
    parts[0].len() == 4
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].len() == 2
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].len() == 2
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// Check if a kind is a calendar event kind
pub fn is_calendar_event_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_DATE_BASED_CALENDAR_EVENT | KIND_TIME_BASED_CALENDAR_EVENT
    )
}

/// Check if a kind is a NIP-52 kind
pub fn is_nip52_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_DATE_BASED_CALENDAR_EVENT
            | KIND_TIME_BASED_CALENDAR_EVENT
            | KIND_CALENDAR
            | KIND_CALENDAR_EVENT_RSVP
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_date_format_validation() {
        assert!(is_valid_date_format("2024-01-15"));
        assert!(is_valid_date_format("2024-12-31"));
        assert!(!is_valid_date_format("2024-1-15"));
        assert!(!is_valid_date_format("24-01-15"));
        assert!(!is_valid_date_format("2024/01/15"));
        assert!(!is_valid_date_format("invalid"));
    }

    #[test]
    fn test_date_based_event_new() {
        let event = DateBasedCalendarEvent::new("vacation", "Summer Vacation", "2024-07-01");
        assert_eq!(event.d, "vacation");
        assert_eq!(event.title, "Summer Vacation");
        assert_eq!(event.start, "2024-07-01");
    }

    #[test]
    fn test_date_based_event_validate() {
        let mut event = DateBasedCalendarEvent::new("test", "Test Event", "2024-07-01");
        assert!(event.validate().is_ok());

        event.end = Some("2024-07-15".to_string());
        assert!(event.validate().is_ok());

        // Invalid: end before start
        event.end = Some("2024-06-01".to_string());
        assert!(event.validate().is_err());

        // Invalid: bad date format
        event.start = "2024-7-1".to_string();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_date_based_event_to_tags() {
        let event = DateBasedCalendarEvent {
            d: "test".to_string(),
            title: "Test Event".to_string(),
            start: "2024-07-01".to_string(),
            end: Some("2024-07-15".to_string()),
            summary: Some("A test event".to_string()),
            locations: vec!["Beach".to_string()],
            participants: vec![Participant::new("pubkey123").with_role("organizer")],
            hashtags: vec!["vacation".to_string()],
            ..Default::default()
        };

        let tags = event.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "test".to_string()]));
        assert!(tags.contains(&vec!["title".to_string(), "Test Event".to_string()]));
        assert!(tags.contains(&vec!["start".to_string(), "2024-07-01".to_string()]));
        assert!(tags.contains(&vec!["end".to_string(), "2024-07-15".to_string()]));
        assert!(tags.contains(&vec!["summary".to_string(), "A test event".to_string()]));
        assert!(tags.contains(&vec!["location".to_string(), "Beach".to_string()]));
        assert!(tags.contains(&vec!["t".to_string(), "vacation".to_string()]));
    }

    #[test]
    fn test_time_based_event_new() {
        let event = TimeBasedCalendarEvent::new("meeting", "Team Meeting", 1686840000);
        assert_eq!(event.d, "meeting");
        assert_eq!(event.title, "Team Meeting");
        assert_eq!(event.start, 1686840000);
    }

    #[test]
    fn test_time_based_event_validate() {
        let mut event = TimeBasedCalendarEvent::new("test", "Test Event", 1686840000);
        assert!(event.validate().is_ok());

        event.end = Some(1686843600);
        assert!(event.validate().is_ok());

        // Invalid: end before start
        event.end = Some(1686830000);
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_time_based_event_to_tags() {
        let event = TimeBasedCalendarEvent {
            d: "test".to_string(),
            title: "Test Meeting".to_string(),
            start: 1686840000,
            end: Some(1686843600),
            start_tzid: Some("America/New_York".to_string()),
            summary: Some("Weekly sync".to_string()),
            ..Default::default()
        };

        let tags = event.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "test".to_string()]));
        assert!(tags.contains(&vec!["title".to_string(), "Test Meeting".to_string()]));
        assert!(tags.contains(&vec!["start".to_string(), "1686840000".to_string()]));
        assert!(tags.contains(&vec!["end".to_string(), "1686843600".to_string()]));
        assert!(tags.contains(&vec![
            "start_tzid".to_string(),
            "America/New_York".to_string()
        ]));
    }

    #[test]
    fn test_calendar_new() {
        let calendar = Calendar::new("personal", "Personal Calendar");
        assert_eq!(calendar.d, "personal");
        assert_eq!(calendar.title, "Personal Calendar");
        assert_eq!(calendar.event_refs.len(), 0);
    }

    #[test]
    fn test_calendar_add_event() {
        let mut calendar = Calendar::new("work", "Work Calendar");
        calendar.add_event(CalendarEventRef::new("31923:pubkey:meeting1"));
        calendar.add_event(
            CalendarEventRef::new("31922:pubkey:holiday1").with_relay("wss://relay.example.com"),
        );

        assert_eq!(calendar.event_refs.len(), 2);
        assert_eq!(calendar.event_refs[0].coordinates, "31923:pubkey:meeting1");
        assert_eq!(
            calendar.event_refs[1].relay,
            Some("wss://relay.example.com".to_string())
        );
    }

    #[test]
    fn test_calendar_to_tags() {
        let mut calendar = Calendar::new("test", "Test Calendar");
        calendar.add_event(CalendarEventRef::new("31923:pubkey:event1"));

        let tags = calendar.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "test".to_string()]));
        assert!(tags.contains(&vec!["title".to_string(), "Test Calendar".to_string()]));
        assert!(tags.contains(&vec!["a".to_string(), "31923:pubkey:event1".to_string()]));
    }

    #[test]
    fn test_rsvp_status() {
        assert_eq!(RsvpStatus::Accepted.as_str(), "accepted");
        assert_eq!(RsvpStatus::Declined.as_str(), "declined");
        assert_eq!(RsvpStatus::Tentative.as_str(), "tentative");

        assert!(matches!(
            RsvpStatus::from_str("accepted"),
            Ok(RsvpStatus::Accepted)
        ));
        assert!(matches!(
            RsvpStatus::from_str("declined"),
            Ok(RsvpStatus::Declined)
        ));
        assert!(matches!(
            RsvpStatus::from_str("tentative"),
            Ok(RsvpStatus::Tentative)
        ));
        assert!(RsvpStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_free_busy_status() {
        assert_eq!(FreeBusyStatus::Free.as_str(), "free");
        assert_eq!(FreeBusyStatus::Busy.as_str(), "busy");

        assert!(matches!(
            FreeBusyStatus::from_str("free"),
            Ok(FreeBusyStatus::Free)
        ));
        assert!(matches!(
            FreeBusyStatus::from_str("busy"),
            Ok(FreeBusyStatus::Busy)
        ));
        assert!(FreeBusyStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_rsvp_new() {
        let rsvp = CalendarEventRsvp::new("rsvp1", "31923:pubkey:event1", RsvpStatus::Accepted);
        assert_eq!(rsvp.d, "rsvp1");
        assert_eq!(rsvp.event_coordinates, "31923:pubkey:event1");
        assert_eq!(rsvp.status, RsvpStatus::Accepted);
    }

    #[test]
    fn test_rsvp_validate() {
        let rsvp = CalendarEventRsvp::new("rsvp1", "31923:pubkey:event1", RsvpStatus::Accepted);
        assert!(rsvp.validate().is_ok());

        // Invalid: free_busy set when status is declined
        let invalid_rsvp =
            CalendarEventRsvp::new("rsvp2", "31923:pubkey:event1", RsvpStatus::Declined)
                .with_free_busy(FreeBusyStatus::Busy);
        assert!(invalid_rsvp.validate().is_err());
    }

    #[test]
    fn test_rsvp_to_tags() {
        let rsvp = CalendarEventRsvp::new("rsvp1", "31923:pubkey:event1", RsvpStatus::Accepted)
            .with_event_id("event-id-123")
            .with_free_busy(FreeBusyStatus::Busy)
            .with_event_author("author-pubkey");

        let tags = rsvp.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "rsvp1".to_string()]));
        assert!(tags.contains(&vec!["a".to_string(), "31923:pubkey:event1".to_string()]));
        assert!(tags.contains(&vec!["e".to_string(), "event-id-123".to_string()]));
        assert!(tags.contains(&vec!["status".to_string(), "accepted".to_string()]));
        assert!(tags.contains(&vec!["fb".to_string(), "busy".to_string()]));
        assert!(tags.contains(&vec!["p".to_string(), "author-pubkey".to_string()]));
    }

    #[test]
    fn test_is_calendar_event_kind() {
        assert!(is_calendar_event_kind(KIND_DATE_BASED_CALENDAR_EVENT));
        assert!(is_calendar_event_kind(KIND_TIME_BASED_CALENDAR_EVENT));
        assert!(!is_calendar_event_kind(KIND_CALENDAR));
        assert!(!is_calendar_event_kind(KIND_CALENDAR_EVENT_RSVP));
        assert!(!is_calendar_event_kind(1));
    }

    #[test]
    fn test_is_nip52_kind() {
        assert!(is_nip52_kind(KIND_DATE_BASED_CALENDAR_EVENT));
        assert!(is_nip52_kind(KIND_TIME_BASED_CALENDAR_EVENT));
        assert!(is_nip52_kind(KIND_CALENDAR));
        assert!(is_nip52_kind(KIND_CALENDAR_EVENT_RSVP));
        assert!(!is_nip52_kind(1));
    }

    #[test]
    fn test_participant() {
        let participant = Participant::new("pubkey123")
            .with_relay("wss://relay.example.com")
            .with_role("organizer");

        assert_eq!(participant.pubkey, "pubkey123");
        assert_eq!(
            participant.relay,
            Some("wss://relay.example.com".to_string())
        );
        assert_eq!(participant.role, Some("organizer".to_string()));
    }
}
