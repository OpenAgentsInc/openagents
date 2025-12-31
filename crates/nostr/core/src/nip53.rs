//! NIP-53: Live Activities
//!
//! This NIP introduces event kinds to advertise live spaces and the participation
//! of pubkeys in them, including live streaming, meetings, and chat.
//!
//! ## Event Types
//!
//! - **Live Streaming Event** (kind 30311): Addressable event for live streams
//! - **Live Chat Message** (kind 1311): Chat messages in a live stream
//! - **Meeting Space** (kind 30312): Virtual interactive room/space
//! - **Meeting Room Event** (kind 30313): Scheduled/ongoing meeting
//! - **Room Presence** (kind 10312): User presence indicator
//!
//! ## Example
//!
//! ```
//! use nostr::nip53::{LiveStreamingEvent, LiveStatus, LiveParticipant};
//!
//! // Create a live streaming event
//! let mut stream = LiveStreamingEvent::new("stream-1", "My Live Stream");
//! stream.status = LiveStatus::Live;
//! stream.streaming_url = Some("https://example.com/stream.m3u8".to_string());
//! stream.add_participant(LiveParticipant::new("pubkey-hex", "Host"));
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind for live streaming events
pub const KIND_LIVE_STREAMING: u16 = 30311;

/// Kind for live chat messages
pub const KIND_LIVE_CHAT_MESSAGE: u16 = 1311;

/// Kind for meeting spaces
pub const KIND_MEETING_SPACE: u16 = 30312;

/// Kind for meeting room events
pub const KIND_MEETING_ROOM_EVENT: u16 = 30313;

/// Kind for room presence
pub const KIND_ROOM_PRESENCE: u16 = 10312;

/// Errors that can occur during NIP-53 operations.
#[derive(Debug, Error)]
pub enum Nip53Error {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Status of a live activity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LiveStatus {
    /// Activity is planned but not yet started
    #[default]
    Planned,
    /// Activity is currently live
    Live,
    /// Activity has ended
    Ended,
}

impl LiveStatus {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            LiveStatus::Planned => "planned",
            LiveStatus::Live => "live",
            LiveStatus::Ended => "ended",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self, Nip53Error> {
        match s {
            "planned" => Ok(LiveStatus::Planned),
            "live" => Ok(LiveStatus::Live),
            "ended" => Ok(LiveStatus::Ended),
            _ => Err(Nip53Error::InvalidStatus(s.to_string())),
        }
    }
}

/// Status of a meeting space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpaceStatus {
    /// Space is open to participants
    #[default]
    Open,
    /// Space is private/restricted
    Private,
    /// Space is closed/not in operation
    Closed,
}

impl SpaceStatus {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            SpaceStatus::Open => "open",
            SpaceStatus::Private => "private",
            SpaceStatus::Closed => "closed",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self, Nip53Error> {
        match s {
            "open" => Ok(SpaceStatus::Open),
            "private" => Ok(SpaceStatus::Private),
            "closed" => Ok(SpaceStatus::Closed),
            _ => Err(Nip53Error::InvalidStatus(s.to_string())),
        }
    }
}

/// Participant in a live activity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveParticipant {
    /// Pubkey of the participant (32-bytes hex)
    pub pubkey: String,
    /// Optional relay URL
    pub relay: Option<String>,
    /// Role in the activity (e.g., Host, Speaker, Participant)
    pub role: String,
    /// Optional proof of agreement to participate
    pub proof: Option<String>,
}

impl LiveParticipant {
    /// Create a new participant
    pub fn new(pubkey: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay: None,
            role: role.into(),
            proof: None,
        }
    }

    /// Set the relay URL
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.relay = Some(relay.into());
        self
    }

    /// Set the proof
    pub fn with_proof(mut self, proof: impl Into<String>) -> Self {
        self.proof = Some(proof.into());
        self
    }
}

/// Live streaming event (kind 30311)
///
/// An addressable event that advertises the content and participants of a live stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LiveStreamingEvent {
    /// Unique identifier (d tag)
    pub d: String,
    /// Title of the live stream
    pub title: String,
    /// Description of the stream
    pub summary: Option<String>,
    /// Preview image URL
    pub image: Option<String>,
    /// Hashtags for categorization
    pub hashtags: Vec<String>,
    /// Streaming URL
    pub streaming_url: Option<String>,
    /// Recording URL (for after the stream ends)
    pub recording_url: Option<String>,
    /// Start timestamp (Unix seconds)
    pub starts: Option<u64>,
    /// End timestamp (Unix seconds)
    pub ends: Option<u64>,
    /// Status of the stream
    pub status: LiveStatus,
    /// Current number of participants
    pub current_participants: Option<u64>,
    /// Total number of participants
    pub total_participants: Option<u64>,
    /// Participants with roles
    pub participants: Vec<LiveParticipant>,
    /// Relay URLs
    pub relays: Vec<String>,
    /// Event IDs of pinned chat messages
    pub pinned_messages: Vec<String>,
}

impl LiveStreamingEvent {
    /// Create a new live streaming event
    pub fn new(d: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            d: d.into(),
            title: title.into(),
            status: LiveStatus::Planned,
            ..Default::default()
        }
    }

    /// Add a participant
    pub fn add_participant(&mut self, participant: LiveParticipant) {
        self.participants.push(participant);
    }

    /// Validate the event
    pub fn validate(&self) -> Result<(), Nip53Error> {
        if self.d.is_empty() {
            return Err(Nip53Error::MissingField("d".to_string()));
        }
        if self.title.is_empty() {
            return Err(Nip53Error::MissingField("title".to_string()));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.d.clone()],
            vec!["title".to_string(), self.title.clone()],
        ];

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        if let Some(image) = &self.image {
            tags.push(vec!["image".to_string(), image.clone()]);
        }

        for hashtag in &self.hashtags {
            tags.push(vec!["t".to_string(), hashtag.clone()]);
        }

        if let Some(streaming_url) = &self.streaming_url {
            tags.push(vec!["streaming".to_string(), streaming_url.clone()]);
        }

        if let Some(recording_url) = &self.recording_url {
            tags.push(vec!["recording".to_string(), recording_url.clone()]);
        }

        if let Some(starts) = self.starts {
            tags.push(vec!["starts".to_string(), starts.to_string()]);
        }

        if let Some(ends) = self.ends {
            tags.push(vec!["ends".to_string(), ends.to_string()]);
        }

        tags.push(vec!["status".to_string(), self.status.as_str().to_string()]);

        if let Some(current) = self.current_participants {
            tags.push(vec![
                "current_participants".to_string(),
                current.to_string(),
            ]);
        }

        if let Some(total) = self.total_participants {
            tags.push(vec!["total_participants".to_string(), total.to_string()]);
        }

        for participant in &self.participants {
            let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
            if let Some(relay) = &participant.relay {
                tag.push(relay.clone());
            } else {
                tag.push(String::new());
            }
            tag.push(participant.role.clone());
            if let Some(proof) = &participant.proof {
                tag.push(proof.clone());
            }
            tags.push(tag);
        }

        if !self.relays.is_empty() {
            let mut relay_tag = vec!["relays".to_string()];
            relay_tag.extend(self.relays.clone());
            tags.push(relay_tag);
        }

        for pinned in &self.pinned_messages {
            tags.push(vec!["pinned".to_string(), pinned.clone()]);
        }

        tags
    }
}

/// Live chat message (kind 1311)
///
/// A message in a live stream's chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveChatMessage {
    /// Reference to the live activity (a tag)
    pub activity_ref: String,
    /// Optional relay URL for the activity
    pub activity_relay: Option<String>,
    /// Optional parent message ID (e tag)
    pub reply_to: Option<String>,
}

impl LiveChatMessage {
    /// Create a new live chat message
    pub fn new(activity_ref: impl Into<String>) -> Self {
        Self {
            activity_ref: activity_ref.into(),
            activity_relay: None,
            reply_to: None,
        }
    }

    /// Set the activity relay
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.activity_relay = Some(relay.into());
        self
    }

    /// Set the reply-to message ID
    pub fn with_reply_to(mut self, reply_to: impl Into<String>) -> Self {
        self.reply_to = Some(reply_to.into());
        self
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        let mut a_tag = vec!["a".to_string(), self.activity_ref.clone()];
        if let Some(relay) = &self.activity_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        if let Some(reply_to) = &self.reply_to {
            tags.push(vec!["e".to_string(), reply_to.clone()]);
        }

        tags
    }
}

/// Meeting space (kind 30312)
///
/// Configuration and properties of a virtual interactive space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MeetingSpace {
    /// Unique identifier (d tag)
    pub d: String,
    /// Name of the room
    pub room: String,
    /// Description of the room
    pub summary: Option<String>,
    /// Preview image URL
    pub image: Option<String>,
    /// Room status
    pub status: SpaceStatus,
    /// URL to access the room
    pub service_url: String,
    /// Optional API endpoint for status/info
    pub endpoint: Option<String>,
    /// Hashtags
    pub hashtags: Vec<String>,
    /// Providers/participants with roles
    pub participants: Vec<LiveParticipant>,
    /// Relay URLs
    pub relays: Vec<String>,
}

impl MeetingSpace {
    /// Create a new meeting space
    pub fn new(
        d: impl Into<String>,
        room: impl Into<String>,
        service_url: impl Into<String>,
    ) -> Self {
        Self {
            d: d.into(),
            room: room.into(),
            service_url: service_url.into(),
            status: SpaceStatus::Open,
            ..Default::default()
        }
    }

    /// Add a participant
    pub fn add_participant(&mut self, participant: LiveParticipant) {
        self.participants.push(participant);
    }

    /// Validate the meeting space
    pub fn validate(&self) -> Result<(), Nip53Error> {
        if self.d.is_empty() {
            return Err(Nip53Error::MissingField("d".to_string()));
        }
        if self.room.is_empty() {
            return Err(Nip53Error::MissingField("room".to_string()));
        }
        if self.service_url.is_empty() {
            return Err(Nip53Error::MissingField("service_url".to_string()));
        }
        if self.participants.is_empty() {
            return Err(Nip53Error::MissingField(
                "participants (at least one)".to_string(),
            ));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![
            vec!["d".to_string(), self.d.clone()],
            vec!["room".to_string(), self.room.clone()],
            vec!["status".to_string(), self.status.as_str().to_string()],
            vec!["service".to_string(), self.service_url.clone()],
        ];

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        if let Some(image) = &self.image {
            tags.push(vec!["image".to_string(), image.clone()]);
        }

        if let Some(endpoint) = &self.endpoint {
            tags.push(vec!["endpoint".to_string(), endpoint.clone()]);
        }

        for hashtag in &self.hashtags {
            tags.push(vec!["t".to_string(), hashtag.clone()]);
        }

        for participant in &self.participants {
            let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
            if let Some(relay) = &participant.relay {
                tag.push(relay.clone());
            } else {
                tag.push(String::new());
            }
            tag.push(participant.role.clone());
            if let Some(proof) = &participant.proof {
                tag.push(proof.clone());
            }
            tags.push(tag);
        }

        if !self.relays.is_empty() {
            let mut relay_tag = vec!["relays".to_string()];
            relay_tag.extend(self.relays.clone());
            tags.push(relay_tag);
        }

        tags
    }
}

/// Meeting room event (kind 30313)
///
/// A scheduled or ongoing meeting within a space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MeetingRoomEvent {
    /// Unique identifier (d tag)
    pub d: String,
    /// Reference to parent space (a tag)
    pub space_ref: String,
    /// Optional relay URL for the space
    pub space_relay: Option<String>,
    /// Title of the meeting
    pub title: String,
    /// Description of the meeting
    pub summary: Option<String>,
    /// Preview image URL
    pub image: Option<String>,
    /// Start timestamp (Unix seconds)
    pub starts: u64,
    /// End timestamp (Unix seconds)
    pub ends: Option<u64>,
    /// Status of the meeting
    pub status: LiveStatus,
    /// Total number of participants
    pub total_participants: Option<u64>,
    /// Current number of participants
    pub current_participants: Option<u64>,
    /// Participants with roles
    pub participants: Vec<LiveParticipant>,
}

impl MeetingRoomEvent {
    /// Create a new meeting room event
    pub fn new(
        d: impl Into<String>,
        space_ref: impl Into<String>,
        title: impl Into<String>,
        starts: u64,
    ) -> Self {
        Self {
            d: d.into(),
            space_ref: space_ref.into(),
            title: title.into(),
            starts,
            status: LiveStatus::Planned,
            ..Default::default()
        }
    }

    /// Add a participant
    pub fn add_participant(&mut self, participant: LiveParticipant) {
        self.participants.push(participant);
    }

    /// Validate the meeting room event
    pub fn validate(&self) -> Result<(), Nip53Error> {
        if self.d.is_empty() {
            return Err(Nip53Error::MissingField("d".to_string()));
        }
        if self.space_ref.is_empty() {
            return Err(Nip53Error::MissingField("space_ref".to_string()));
        }
        if self.title.is_empty() {
            return Err(Nip53Error::MissingField("title".to_string()));
        }
        Ok(())
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.d.clone()]];

        let mut a_tag = vec!["a".to_string(), self.space_ref.clone()];
        if let Some(relay) = &self.space_relay {
            a_tag.push(relay.clone());
        }
        tags.push(a_tag);

        tags.push(vec!["title".to_string(), self.title.clone()]);

        if let Some(summary) = &self.summary {
            tags.push(vec!["summary".to_string(), summary.clone()]);
        }

        if let Some(image) = &self.image {
            tags.push(vec!["image".to_string(), image.clone()]);
        }

        tags.push(vec!["starts".to_string(), self.starts.to_string()]);

        if let Some(ends) = self.ends {
            tags.push(vec!["ends".to_string(), ends.to_string()]);
        }

        tags.push(vec!["status".to_string(), self.status.as_str().to_string()]);

        if let Some(total) = self.total_participants {
            tags.push(vec!["total_participants".to_string(), total.to_string()]);
        }

        if let Some(current) = self.current_participants {
            tags.push(vec![
                "current_participants".to_string(),
                current.to_string(),
            ]);
        }

        for participant in &self.participants {
            let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
            if let Some(relay) = &participant.relay {
                tag.push(relay.clone());
            } else {
                tag.push(String::new());
            }
            tag.push(participant.role.clone());
            tags.push(tag);
        }

        tags
    }
}

/// Room presence (kind 10312)
///
/// Signals presence of a user in a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomPresence {
    /// Reference to the room (a tag)
    pub room_ref: String,
    /// Optional relay URL for the room
    pub room_relay: Option<String>,
    /// Whether the user has raised their hand
    pub hand_raised: bool,
}

impl RoomPresence {
    /// Create a new room presence
    pub fn new(room_ref: impl Into<String>) -> Self {
        Self {
            room_ref: room_ref.into(),
            room_relay: None,
            hand_raised: false,
        }
    }

    /// Set the room relay
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.room_relay = Some(relay.into());
        self
    }

    /// Set hand raised status
    pub fn with_hand_raised(mut self, raised: bool) -> Self {
        self.hand_raised = raised;
        self
    }

    /// Convert to tags for event creation
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        let mut a_tag = vec!["a".to_string(), self.room_ref.clone()];
        if let Some(relay) = &self.room_relay {
            a_tag.push(relay.clone());
        }
        a_tag.push("root".to_string());
        tags.push(a_tag);

        if self.hand_raised {
            tags.push(vec!["hand".to_string(), "1".to_string()]);
        }

        tags
    }
}

/// Check if a kind is a NIP-53 kind
pub fn is_nip53_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_LIVE_STREAMING
            | KIND_LIVE_CHAT_MESSAGE
            | KIND_MEETING_SPACE
            | KIND_MEETING_ROOM_EVENT
            | KIND_ROOM_PRESENCE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_status() {
        assert_eq!(LiveStatus::Planned.as_str(), "planned");
        assert_eq!(LiveStatus::Live.as_str(), "live");
        assert_eq!(LiveStatus::Ended.as_str(), "ended");

        assert_eq!(
            LiveStatus::from_str("planned").unwrap(),
            LiveStatus::Planned
        );
        assert_eq!(LiveStatus::from_str("live").unwrap(), LiveStatus::Live);
        assert_eq!(LiveStatus::from_str("ended").unwrap(), LiveStatus::Ended);
        assert!(LiveStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_space_status() {
        assert_eq!(SpaceStatus::Open.as_str(), "open");
        assert_eq!(SpaceStatus::Private.as_str(), "private");
        assert_eq!(SpaceStatus::Closed.as_str(), "closed");

        assert_eq!(SpaceStatus::from_str("open").unwrap(), SpaceStatus::Open);
        assert_eq!(
            SpaceStatus::from_str("private").unwrap(),
            SpaceStatus::Private
        );
        assert_eq!(
            SpaceStatus::from_str("closed").unwrap(),
            SpaceStatus::Closed
        );
        assert!(SpaceStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_live_participant() {
        let participant = LiveParticipant::new("pubkey123", "Host")
            .with_relay("wss://relay.example.com")
            .with_proof("proof123");

        assert_eq!(participant.pubkey, "pubkey123");
        assert_eq!(participant.role, "Host");
        assert_eq!(
            participant.relay,
            Some("wss://relay.example.com".to_string())
        );
        assert_eq!(participant.proof, Some("proof123".to_string()));
    }

    #[test]
    fn test_live_streaming_event_new() {
        let event = LiveStreamingEvent::new("stream-1", "My Stream");
        assert_eq!(event.d, "stream-1");
        assert_eq!(event.title, "My Stream");
        assert_eq!(event.status, LiveStatus::Planned);
    }

    #[test]
    fn test_live_streaming_event_validate() {
        let event = LiveStreamingEvent::new("stream-1", "My Stream");
        assert!(event.validate().is_ok());

        let invalid = LiveStreamingEvent::default();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_live_streaming_event_to_tags() {
        let mut event = LiveStreamingEvent::new("stream-1", "My Stream");
        event.status = LiveStatus::Live;
        event.streaming_url = Some("https://example.com/stream.m3u8".to_string());
        event.add_participant(LiveParticipant::new("pubkey123", "Host"));

        let tags = event.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "stream-1".to_string()]));
        assert!(tags.contains(&vec!["title".to_string(), "My Stream".to_string()]));
        assert!(tags.contains(&vec!["status".to_string(), "live".to_string()]));
        assert!(tags.contains(&vec![
            "streaming".to_string(),
            "https://example.com/stream.m3u8".to_string()
        ]));
    }

    #[test]
    fn test_live_chat_message() {
        let msg = LiveChatMessage::new("30311:pubkey:stream-1")
            .with_relay("wss://relay.example.com")
            .with_reply_to("event-id-123");

        let tags = msg.to_tags();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0][0], "a");
        assert_eq!(tags[0][1], "30311:pubkey:stream-1");
        assert_eq!(tags[1][0], "e");
        assert_eq!(tags[1][1], "event-id-123");
    }

    #[test]
    fn test_meeting_space_new() {
        let space = MeetingSpace::new("room-1", "Conference Room", "https://meet.example.com");
        assert_eq!(space.d, "room-1");
        assert_eq!(space.room, "Conference Room");
        assert_eq!(space.service_url, "https://meet.example.com");
        assert_eq!(space.status, SpaceStatus::Open);
    }

    #[test]
    fn test_meeting_space_validate() {
        let mut space = MeetingSpace::new("room-1", "Conference Room", "https://meet.example.com");
        space.add_participant(LiveParticipant::new("pubkey123", "Host"));
        assert!(space.validate().is_ok());

        let invalid = MeetingSpace::default();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_meeting_space_to_tags() {
        let mut space = MeetingSpace::new("room-1", "Conference Room", "https://meet.example.com");
        space.add_participant(LiveParticipant::new("pubkey123", "Host"));

        let tags = space.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "room-1".to_string()]));
        assert!(tags.contains(&vec!["room".to_string(), "Conference Room".to_string()]));
        assert!(tags.contains(&vec![
            "service".to_string(),
            "https://meet.example.com".to_string()
        ]));
        assert!(tags.contains(&vec!["status".to_string(), "open".to_string()]));
    }

    #[test]
    fn test_meeting_room_event_new() {
        let event = MeetingRoomEvent::new(
            "meeting-1",
            "30312:pubkey:room-1",
            "Team Meeting",
            1686840000,
        );
        assert_eq!(event.d, "meeting-1");
        assert_eq!(event.space_ref, "30312:pubkey:room-1");
        assert_eq!(event.title, "Team Meeting");
        assert_eq!(event.starts, 1686840000);
        assert_eq!(event.status, LiveStatus::Planned);
    }

    #[test]
    fn test_meeting_room_event_validate() {
        let event = MeetingRoomEvent::new(
            "meeting-1",
            "30312:pubkey:room-1",
            "Team Meeting",
            1686840000,
        );
        assert!(event.validate().is_ok());

        let invalid = MeetingRoomEvent::default();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_meeting_room_event_to_tags() {
        let event = MeetingRoomEvent::new(
            "meeting-1",
            "30312:pubkey:room-1",
            "Team Meeting",
            1686840000,
        );

        let tags = event.to_tags();
        assert!(tags.contains(&vec!["d".to_string(), "meeting-1".to_string()]));
        assert!(tags.contains(&vec!["title".to_string(), "Team Meeting".to_string()]));
        assert!(tags.contains(&vec!["starts".to_string(), "1686840000".to_string()]));
        assert!(tags.contains(&vec!["status".to_string(), "planned".to_string()]));
    }

    #[test]
    fn test_room_presence() {
        let presence = RoomPresence::new("30312:pubkey:room-1")
            .with_relay("wss://relay.example.com")
            .with_hand_raised(true);

        let tags = presence.to_tags();
        assert_eq!(tags[0][0], "a");
        assert_eq!(tags[0][1], "30312:pubkey:room-1");
        assert_eq!(tags[0][3], "root");
        assert!(tags.contains(&vec!["hand".to_string(), "1".to_string()]));
    }

    #[test]
    fn test_is_nip53_kind() {
        assert!(is_nip53_kind(KIND_LIVE_STREAMING));
        assert!(is_nip53_kind(KIND_LIVE_CHAT_MESSAGE));
        assert!(is_nip53_kind(KIND_MEETING_SPACE));
        assert!(is_nip53_kind(KIND_MEETING_ROOM_EVENT));
        assert!(is_nip53_kind(KIND_ROOM_PRESENCE));
        assert!(!is_nip53_kind(1));
    }
}
