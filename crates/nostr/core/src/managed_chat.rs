//! Managed chat helpers for NIP-28 channels inside NIP-29 rooms.
//!
//! This module provides protocol-level builders, parsers, and ordering helpers for
//! OpenAgents managed chat. It intentionally stays below app/UI logic while
//! encoding the `h` and `oa-*` tags described in the managed-chat contract.

use std::{cmp::Ordering, collections::BTreeMap, fmt, str::FromStr};

use thiserror::Error;

use crate::{
    nip01::{Event, UnsignedEvent},
    nip28::{
        ChannelMetadata, KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE, KIND_CHANNEL_METADATA,
        Nip28Error,
    },
    nip29::validate_group_id,
    tag_parsing::{find_tag_value, is_tag, tag_field},
};

/// Group-context tag for managed chat events.
pub const TAG_GROUP_CONTEXT: &str = "h";
/// OpenAgents room mode tag.
pub const TAG_OA_ROOM_MODE: &str = "oa-room-mode";
/// OpenAgents channel slug tag.
pub const TAG_OA_SLUG: &str = "oa-slug";
/// OpenAgents channel type tag.
pub const TAG_OA_CHANNEL_TYPE: &str = "oa-channel-type";
/// OpenAgents category id tag.
pub const TAG_OA_CATEGORY: &str = "oa-category";
/// OpenAgents category label tag.
pub const TAG_OA_CATEGORY_LABEL: &str = "oa-category-label";
/// OpenAgents channel position tag.
pub const TAG_OA_POSITION: &str = "oa-position";

const ROOM_MODE_MANAGED_CHANNEL: &str = "managed-channel";
const REPLY_MARKER: &str = "reply";
const ROOT_MARKER: &str = "root";
const UNCATEGORIZED_BUCKET: &str = "_uncategorized";

/// Errors that can occur while working with managed chat events.
#[derive(Debug, Error)]
pub enum ManagedChatError {
    #[error("invalid kind: expected {expected}, got {got}")]
    InvalidKind { expected: String, got: u16 },

    #[error("invalid group id: {0}")]
    InvalidGroupId(String),

    #[error("invalid event id: {0}")]
    InvalidEventId(String),

    #[error("invalid pubkey: {0}")]
    InvalidPubkey(String),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag: {0}")]
    InvalidTag(String),

    #[error("invalid relay url: {0}")]
    InvalidRelayUrl(String),

    #[error("unsupported room mode for managed channel helper: {0}")]
    UnsupportedRoomMode(String),

    #[error("invalid channel metadata: {0}")]
    InvalidMetadata(String),
}

impl From<Nip28Error> for ManagedChatError {
    fn from(value: Nip28Error) -> Self {
        Self::InvalidMetadata(value.to_string())
    }
}

/// Result type for managed chat helpers.
pub type ManagedChatResult<T> = std::result::Result<T, ManagedChatError>;

type Result<T> = ManagedChatResult<T>;

/// Supported room transports in the desktop room model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManagedRoomMode {
    #[default]
    ManagedChannel,
    Dm,
    SecureGroup,
}

impl ManagedRoomMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManagedChannel => ROOM_MODE_MANAGED_CHANNEL,
            Self::Dm => "dm",
            Self::SecureGroup => "secure-group",
        }
    }
}

impl fmt::Display for ManagedRoomMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ManagedRoomMode {
    type Err = ManagedChatError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            ROOM_MODE_MANAGED_CHANNEL => Ok(Self::ManagedChannel),
            "dm" => Ok(Self::Dm),
            "secure-group" => Ok(Self::SecureGroup),
            other => Err(ManagedChatError::UnsupportedRoomMode(other.to_string())),
        }
    }
}

/// OpenAgents channel type hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedChannelType {
    Text,
    Announcement,
    Ops,
    Support,
    System,
    Other(String),
}

impl ManagedChannelType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Announcement => "announcement",
            Self::Ops => "ops",
            Self::Support => "support",
            Self::System => "system",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl fmt::Display for ManagedChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ManagedChannelType {
    type Err = ManagedChatError;

    fn from_str(value: &str) -> Result<Self> {
        if value.trim().is_empty() {
            return Err(ManagedChatError::InvalidTag(
                TAG_OA_CHANNEL_TYPE.to_string(),
            ));
        }

        Ok(match value {
            "text" => Self::Text,
            "announcement" => Self::Announcement,
            "ops" => Self::Ops,
            "support" => Self::Support,
            "system" => Self::System,
            other => Self::Other(other.to_string()),
        })
    }
}

/// Projection hints attached to managed channels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedChannelHints {
    pub slug: Option<String>,
    pub channel_type: Option<ManagedChannelType>,
    pub category_id: Option<String>,
    pub category_label: Option<String>,
    pub position: Option<u32>,
    pub room_mode: ManagedRoomMode,
}

impl ManagedChannelHints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_slug(mut self, slug: impl Into<String>) -> Self {
        self.slug = Some(slug.into());
        self
    }

    pub fn with_channel_type(mut self, channel_type: ManagedChannelType) -> Self {
        self.channel_type = Some(channel_type);
        self
    }

    pub fn with_category_id(mut self, category_id: impl Into<String>) -> Self {
        self.category_id = Some(category_id.into());
        self
    }

    pub fn with_category_label(mut self, category_label: impl Into<String>) -> Self {
        self.category_label = Some(category_label.into());
        self
    }

    pub fn with_position(mut self, position: u32) -> Self {
        self.position = Some(position);
        self
    }

    pub fn with_room_mode(mut self, room_mode: ManagedRoomMode) -> Self {
        self.room_mode = room_mode;
        self
    }

    fn validate(&self) -> Result<()> {
        validate_managed_room_mode(self.room_mode)?;
        validate_optional_non_empty(self.slug.as_deref(), TAG_OA_SLUG)?;
        validate_optional_non_empty(self.category_id.as_deref(), TAG_OA_CATEGORY)?;
        validate_optional_non_empty(self.category_label.as_deref(), TAG_OA_CATEGORY_LABEL)?;
        if let Some(channel_type) = &self.channel_type {
            if channel_type.as_str().trim().is_empty() {
                return Err(ManagedChatError::InvalidTag(
                    TAG_OA_CHANNEL_TYPE.to_string(),
                ));
            }
        }
        Ok(())
    }

    fn to_tags(&self, include_room_mode: bool) -> Result<Vec<Vec<String>>> {
        self.validate()?;

        let mut tags = Vec::new();
        if include_room_mode {
            tags.push(vec![
                TAG_OA_ROOM_MODE.to_string(),
                self.room_mode.as_str().to_string(),
            ]);
        }
        if let Some(slug) = &self.slug {
            tags.push(vec![TAG_OA_SLUG.to_string(), slug.clone()]);
        }
        if let Some(channel_type) = &self.channel_type {
            tags.push(vec![
                TAG_OA_CHANNEL_TYPE.to_string(),
                channel_type.as_str().to_string(),
            ]);
        }
        if let Some(category_id) = &self.category_id {
            tags.push(vec![TAG_OA_CATEGORY.to_string(), category_id.clone()]);
        }
        if let Some(category_label) = &self.category_label {
            tags.push(vec![
                TAG_OA_CATEGORY_LABEL.to_string(),
                category_label.clone(),
            ]);
        }
        if let Some(position) = self.position {
            tags.push(vec![TAG_OA_POSITION.to_string(), position.to_string()]);
        }
        Ok(tags)
    }

    fn from_tags(tags: &[Vec<String>], require_room_mode: bool) -> Result<Self> {
        let room_mode = match find_tag_value(tags, TAG_OA_ROOM_MODE) {
            Some(value) => ManagedRoomMode::from_str(value)?,
            None if require_room_mode => {
                return Err(ManagedChatError::MissingTag(TAG_OA_ROOM_MODE.to_string()));
            }
            None => ManagedRoomMode::ManagedChannel,
        };
        validate_managed_room_mode(room_mode)?;

        let slug = optional_non_empty_tag(tags, TAG_OA_SLUG)?;
        let channel_type = find_tag_value(tags, TAG_OA_CHANNEL_TYPE)
            .map(ManagedChannelType::from_str)
            .transpose()?;
        let category_id = optional_non_empty_tag(tags, TAG_OA_CATEGORY)?;
        let category_label = optional_non_empty_tag(tags, TAG_OA_CATEGORY_LABEL)?;
        let position =
            match find_tag_value(tags, TAG_OA_POSITION) {
                Some(value) => Some(value.parse::<u32>().map_err(|_| {
                    ManagedChatError::InvalidTag(format!("{TAG_OA_POSITION}:{value}"))
                })?),
                None => None,
            };

        let hints = Self {
            slug,
            channel_type,
            category_id,
            category_label,
            position,
            room_mode,
        };
        hints.validate()?;
        Ok(hints)
    }
}

/// One explicit `p`-tag mention on a managed chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChatMention {
    pub pubkey: String,
    pub relay_url: Option<String>,
}

impl ManagedChatMention {
    pub fn new(pubkey: impl Into<String>) -> Result<Self> {
        let pubkey = pubkey.into();
        validate_pubkey(&pubkey)?;
        Ok(Self {
            pubkey,
            relay_url: None,
        })
    }

    pub fn with_relay_url(mut self, relay_url: impl Into<String>) -> Result<Self> {
        let relay_url = relay_url.into();
        validate_relay_url(&relay_url)?;
        self.relay_url = Some(relay_url);
        Ok(self)
    }

    fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["p".to_string(), self.pubkey.clone()];
        if let Some(relay_url) = &self.relay_url {
            tag.push(relay_url.clone());
        }
        tag
    }
}

/// Reply reference for a managed channel message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedMessageReply {
    pub event_id: String,
    pub relay_url: String,
    pub author_pubkey: String,
}

impl ManagedMessageReply {
    pub fn new(
        event_id: impl Into<String>,
        relay_url: impl Into<String>,
        author_pubkey: impl Into<String>,
    ) -> Result<Self> {
        let event_id = event_id.into();
        let relay_url = relay_url.into();
        let author_pubkey = author_pubkey.into();
        validate_event_id(&event_id)?;
        validate_relay_url(&relay_url)?;
        validate_pubkey(&author_pubkey)?;
        Ok(Self {
            event_id,
            relay_url,
            author_pubkey,
        })
    }

    fn to_e_tag(&self) -> Vec<String> {
        vec![
            "e".to_string(),
            self.event_id.clone(),
            self.relay_url.clone(),
            REPLY_MARKER.to_string(),
            self.author_pubkey.clone(),
        ]
    }

    fn reply_author_mention(&self) -> ManagedChatMention {
        ManagedChatMention {
            pubkey: self.author_pubkey.clone(),
            relay_url: Some(self.relay_url.clone()),
        }
    }
}

/// Managed channel create event (`kind:40`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChannelCreateEvent {
    pub group_id: String,
    pub metadata: ChannelMetadata,
    pub hints: ManagedChannelHints,
    pub created_at: u64,
}

impl ManagedChannelCreateEvent {
    pub fn new(
        group_id: impl Into<String>,
        metadata: ChannelMetadata,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            metadata,
            hints: ManagedChannelHints::default(),
            created_at,
        })
    }

    pub fn with_hints(mut self, hints: ManagedChannelHints) -> Result<Self> {
        hints.validate()?;
        self.hints = hints;
        Ok(self)
    }

    pub fn content(&self) -> Result<String> {
        self.metadata.to_json().map_err(ManagedChatError::from)
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>> {
        let mut tags = vec![vec![TAG_GROUP_CONTEXT.to_string(), self.group_id.clone()]];
        tags.extend(self.hints.to_tags(true)?);
        Ok(tags)
    }

    pub fn to_unsigned_event(&self, pubkey: impl Into<String>) -> Result<UnsignedEvent> {
        Ok(UnsignedEvent {
            pubkey: pubkey.into(),
            created_at: self.created_at,
            kind: KIND_CHANNEL_CREATION,
            tags: self.to_tags()?,
            content: self.content()?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_CHANNEL_CREATION {
            return Err(ManagedChatError::InvalidKind {
                expected: KIND_CHANNEL_CREATION.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags)?;
        let metadata = ChannelMetadata::from_json(&event.content)?;
        let hints = ManagedChannelHints::from_tags(&event.tags, true)?;
        Ok(Self {
            group_id,
            metadata,
            hints,
            created_at: event.created_at,
        })
    }
}

/// Managed channel metadata update (`kind:41`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChannelMetadataEvent {
    pub group_id: String,
    pub channel_create_event_id: String,
    pub relay_url: String,
    pub metadata: ChannelMetadata,
    pub hints: ManagedChannelHints,
    pub created_at: u64,
}

impl ManagedChannelMetadataEvent {
    pub fn new(
        group_id: impl Into<String>,
        channel_create_event_id: impl Into<String>,
        relay_url: impl Into<String>,
        metadata: ChannelMetadata,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        let channel_create_event_id = channel_create_event_id.into();
        let relay_url = relay_url.into();
        validate_group_id_or_err(&group_id)?;
        validate_event_id(&channel_create_event_id)?;
        validate_relay_url(&relay_url)?;
        Ok(Self {
            group_id,
            channel_create_event_id,
            relay_url,
            metadata,
            hints: ManagedChannelHints::default(),
            created_at,
        })
    }

    pub fn with_hints(mut self, hints: ManagedChannelHints) -> Result<Self> {
        hints.validate()?;
        self.hints = hints;
        Ok(self)
    }

    pub fn content(&self) -> Result<String> {
        self.metadata.to_json().map_err(ManagedChatError::from)
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>> {
        let mut tags = vec![
            vec![TAG_GROUP_CONTEXT.to_string(), self.group_id.clone()],
            root_channel_tag(&self.channel_create_event_id, &self.relay_url),
        ];
        tags.extend(self.hints.to_tags(false)?);
        Ok(tags)
    }

    pub fn to_unsigned_event(&self, pubkey: impl Into<String>) -> Result<UnsignedEvent> {
        Ok(UnsignedEvent {
            pubkey: pubkey.into(),
            created_at: self.created_at,
            kind: KIND_CHANNEL_METADATA,
            tags: self.to_tags()?,
            content: self.content()?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_CHANNEL_METADATA {
            return Err(ManagedChatError::InvalidKind {
                expected: KIND_CHANNEL_METADATA.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags)?;
        let root = parse_root_reference(&event.tags)?;
        let metadata = ChannelMetadata::from_json(&event.content)?;
        let hints = ManagedChannelHints::from_tags(&event.tags, false)?;
        Ok(Self {
            group_id,
            channel_create_event_id: root.event_id,
            relay_url: root.relay_url,
            metadata,
            hints,
            created_at: event.created_at,
        })
    }
}

/// Managed channel message (`kind:42`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChannelMessageEvent {
    pub group_id: String,
    pub channel_create_event_id: String,
    pub relay_url: String,
    pub content: String,
    pub created_at: u64,
    pub reply_to: Option<ManagedMessageReply>,
    pub mentions: Vec<ManagedChatMention>,
}

impl ManagedChannelMessageEvent {
    pub fn new(
        group_id: impl Into<String>,
        channel_create_event_id: impl Into<String>,
        relay_url: impl Into<String>,
        content: impl Into<String>,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        let channel_create_event_id = channel_create_event_id.into();
        let relay_url = relay_url.into();
        validate_group_id_or_err(&group_id)?;
        validate_event_id(&channel_create_event_id)?;
        validate_relay_url(&relay_url)?;
        Ok(Self {
            group_id,
            channel_create_event_id,
            relay_url,
            content: content.into(),
            created_at,
            reply_to: None,
            mentions: Vec::new(),
        })
    }

    pub fn with_reply(mut self, reply_to: ManagedMessageReply) -> Self {
        self.reply_to = Some(reply_to);
        self
    }

    pub fn with_mentions(mut self, mentions: Vec<ManagedChatMention>) -> Self {
        self.mentions = mentions;
        self
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>> {
        validate_group_id_or_err(&self.group_id)?;
        validate_event_id(&self.channel_create_event_id)?;
        validate_relay_url(&self.relay_url)?;

        let mut tags = vec![
            vec![TAG_GROUP_CONTEXT.to_string(), self.group_id.clone()],
            root_channel_tag(&self.channel_create_event_id, &self.relay_url),
        ];

        let mut mentions = Vec::new();
        if let Some(reply_to) = &self.reply_to {
            tags.push(reply_to.to_e_tag());
            push_unique_mention(&mut mentions, reply_to.reply_author_mention());
        }

        for mention in &self.mentions {
            validate_pubkey(&mention.pubkey)?;
            if let Some(relay_url) = &mention.relay_url {
                validate_relay_url(relay_url)?;
            }
            push_unique_mention(&mut mentions, mention.clone());
        }

        tags.extend(mentions.into_iter().map(|mention| mention.to_tag()));
        Ok(tags)
    }

    pub fn to_unsigned_event(&self, pubkey: impl Into<String>) -> Result<UnsignedEvent> {
        Ok(UnsignedEvent {
            pubkey: pubkey.into(),
            created_at: self.created_at,
            kind: KIND_CHANNEL_MESSAGE,
            tags: self.to_tags()?,
            content: self.content.clone(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_CHANNEL_MESSAGE {
            return Err(ManagedChatError::InvalidKind {
                expected: KIND_CHANNEL_MESSAGE.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags)?;
        let root = parse_root_reference(&event.tags)?;
        let reply_to = parse_reply_reference(&event.tags)?;
        let mut mention_tags = parse_p_tags(&event.tags)?;

        if let Some(reply) = &reply_to {
            let reply_author_index = mention_tags
                .iter()
                .position(|mention| mention.pubkey == reply.author_pubkey)
                .ok_or_else(|| ManagedChatError::MissingTag("p(reply author)".to_string()))?;
            mention_tags.remove(reply_author_index);
        }

        Ok(Self {
            group_id,
            channel_create_event_id: root.event_id,
            relay_url: root.relay_url,
            content: event.content.clone(),
            created_at: event.created_at,
            reply_to,
            mentions: mention_tags,
        })
    }
}

/// Minimal managed channel descriptor used for deterministic channel ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChannelIndexEntry {
    pub channel_create_event_id: String,
    pub metadata: ChannelMetadata,
    pub hints: ManagedChannelHints,
}

impl ManagedChannelIndexEntry {
    pub fn new(
        channel_create_event_id: impl Into<String>,
        metadata: ChannelMetadata,
        hints: ManagedChannelHints,
    ) -> Result<Self> {
        let channel_create_event_id = channel_create_event_id.into();
        validate_event_id(&channel_create_event_id)?;
        hints.validate()?;
        Ok(Self {
            channel_create_event_id,
            metadata,
            hints,
        })
    }
}

/// Compare two channel entries using the managed-chat ordering contract.
pub fn compare_managed_channel_index(
    left: &ManagedChannelIndexEntry,
    right: &ManagedChannelIndexEntry,
) -> Ordering {
    left.hints
        .category_id
        .as_deref()
        .unwrap_or(UNCATEGORIZED_BUCKET)
        .cmp(
            right
                .hints
                .category_id
                .as_deref()
                .unwrap_or(UNCATEGORIZED_BUCKET),
        )
        .then_with(|| match (left.hints.position, right.hints.position) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| left.metadata.name.cmp(&right.metadata.name))
        .then_with(|| {
            left.channel_create_event_id
                .cmp(&right.channel_create_event_id)
        })
}

/// Sort a channel index slice in place using the managed-chat ordering contract.
pub fn sort_managed_channel_index(entries: &mut [ManagedChannelIndexEntry]) {
    entries.sort_by(compare_managed_channel_index);
}

/// Compare two timeline events using the managed-chat ordering contract.
pub fn compare_managed_timeline_events(left: &Event, right: &Event) -> Ordering {
    left.created_at
        .cmp(&right.created_at)
        .then_with(|| left.id.cmp(&right.id))
}

/// Deduplicate channel messages by event id and return them in canonical timeline order.
pub fn project_managed_channel_timeline<I>(events: I) -> Vec<Event>
where
    I: IntoIterator<Item = Event>,
{
    let mut by_id = BTreeMap::new();
    for event in events {
        by_id.entry(event.id.clone()).or_insert(event);
    }

    let mut projected: Vec<_> = by_id.into_values().collect();
    projected.sort_by(compare_managed_timeline_events);
    projected
}

#[derive(Debug, Clone)]
struct RootReference {
    event_id: String,
    relay_url: String,
}

fn root_channel_tag(channel_create_event_id: &str, relay_url: &str) -> Vec<String> {
    vec![
        "e".to_string(),
        channel_create_event_id.to_string(),
        relay_url.to_string(),
        ROOT_MARKER.to_string(),
    ]
}

fn validate_hex_64(value: &str) -> bool {
    value.len() == 64
        && value == value.to_ascii_lowercase()
        && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn validate_group_id_or_err(group_id: &str) -> Result<()> {
    if validate_group_id(group_id) {
        return Ok(());
    }
    Err(ManagedChatError::InvalidGroupId(group_id.to_string()))
}

fn validate_event_id(event_id: &str) -> Result<()> {
    if validate_hex_64(event_id) {
        return Ok(());
    }
    Err(ManagedChatError::InvalidEventId(event_id.to_string()))
}

fn validate_pubkey(pubkey: &str) -> Result<()> {
    if validate_hex_64(pubkey) {
        return Ok(());
    }
    Err(ManagedChatError::InvalidPubkey(pubkey.to_string()))
}

fn validate_relay_url(relay_url: &str) -> Result<()> {
    if relay_url.trim().is_empty() {
        return Err(ManagedChatError::InvalidRelayUrl(relay_url.to_string()));
    }
    Ok(())
}

fn validate_managed_room_mode(room_mode: ManagedRoomMode) -> Result<()> {
    if room_mode == ManagedRoomMode::ManagedChannel {
        return Ok(());
    }
    Err(ManagedChatError::UnsupportedRoomMode(room_mode.to_string()))
}

fn validate_optional_non_empty(value: Option<&str>, tag_name: &str) -> Result<()> {
    if matches!(value, Some(value) if value.trim().is_empty()) {
        return Err(ManagedChatError::InvalidTag(tag_name.to_string()));
    }
    Ok(())
}

fn optional_non_empty_tag(tags: &[Vec<String>], name: &str) -> Result<Option<String>> {
    let value = find_tag_value(tags, name).map(str::to_owned);
    validate_optional_non_empty(value.as_deref(), name)?;
    Ok(value)
}

fn parse_group_id(tags: &[Vec<String>]) -> Result<String> {
    let group_id = find_tag_value(tags, TAG_GROUP_CONTEXT)
        .ok_or_else(|| ManagedChatError::MissingTag(TAG_GROUP_CONTEXT.to_string()))?;
    validate_group_id_or_err(group_id)?;
    Ok(group_id.to_string())
}

fn parse_root_reference(tags: &[Vec<String>]) -> Result<RootReference> {
    let root_tag = tags
        .iter()
        .find(|tag| is_tag(tag, "e") && tag_field(tag, 3) == Some(ROOT_MARKER))
        .ok_or_else(|| ManagedChatError::MissingTag("e(root)".to_string()))?;

    let event_id = tag_field(root_tag, 1)
        .ok_or_else(|| ManagedChatError::MissingTag("e(root):id".to_string()))?;
    let relay_url = tag_field(root_tag, 2)
        .ok_or_else(|| ManagedChatError::MissingTag("e(root):relay".to_string()))?;

    validate_event_id(event_id)?;
    validate_relay_url(relay_url)?;
    Ok(RootReference {
        event_id: event_id.to_string(),
        relay_url: relay_url.to_string(),
    })
}

fn parse_reply_reference(tags: &[Vec<String>]) -> Result<Option<ManagedMessageReply>> {
    let Some(reply_tag) = tags
        .iter()
        .find(|tag| is_tag(tag, "e") && tag_field(tag, 3) == Some(REPLY_MARKER))
    else {
        return Ok(None);
    };

    let event_id = tag_field(reply_tag, 1)
        .ok_or_else(|| ManagedChatError::MissingTag("e(reply):id".to_string()))?;
    let relay_url = tag_field(reply_tag, 2)
        .ok_or_else(|| ManagedChatError::MissingTag("e(reply):relay".to_string()))?;
    let author_pubkey = tag_field(reply_tag, 4)
        .ok_or_else(|| ManagedChatError::MissingTag("e(reply):author".to_string()))?;

    ManagedMessageReply::new(event_id, relay_url, author_pubkey).map(Some)
}

fn parse_p_tags(tags: &[Vec<String>]) -> Result<Vec<ManagedChatMention>> {
    let mut mentions = Vec::new();
    for tag in tags.iter().filter(|tag| is_tag(tag, "p")) {
        let pubkey = tag_field(tag, 1)
            .ok_or_else(|| ManagedChatError::MissingTag("p:pubkey".to_string()))?;
        let mut mention = ManagedChatMention::new(pubkey)?;
        if let Some(relay_url) = tag_field(tag, 2) {
            mention = mention.with_relay_url(relay_url)?;
        }
        push_unique_mention(&mut mentions, mention);
    }
    Ok(mentions)
}

fn push_unique_mention(mentions: &mut Vec<ManagedChatMention>, candidate: ManagedChatMention) {
    if mentions
        .iter()
        .any(|mention| mention.pubkey == candidate.pubkey)
    {
        return;
    }
    mentions.push(candidate);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex64(ch: char) -> String {
        std::iter::repeat_n(ch, 64).collect()
    }

    fn sample_metadata(name: &str) -> ChannelMetadata {
        ChannelMetadata::new(name, "about", "")
            .with_relays(vec!["wss://chat.openagents.example".to_string()])
    }

    fn signed_event(
        id: &str,
        pubkey: &str,
        kind: u16,
        created_at: u64,
        content: &str,
        tags: Vec<Vec<String>>,
    ) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at,
            kind,
            tags,
            content: content.to_string(),
            sig: hex64('f'),
        }
    }

    #[test]
    fn managed_channel_create_round_trip() {
        let event = ManagedChannelCreateEvent::new("oa-main", sample_metadata("provider-ops"), 42)
            .unwrap()
            .with_hints(
                ManagedChannelHints::new()
                    .with_slug("provider-ops")
                    .with_channel_type(ManagedChannelType::Ops)
                    .with_category_id("operations")
                    .with_category_label("Operations")
                    .with_position(120),
            )
            .unwrap();

        let unsigned = event.to_unsigned_event(hex64('1')).unwrap();
        let parsed = ManagedChannelCreateEvent::from_event(&signed_event(
            &hex64('a'),
            &hex64('1'),
            unsigned.kind,
            unsigned.created_at,
            &unsigned.content,
            unsigned.tags.clone(),
        ))
        .unwrap();

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.metadata, event.metadata);
        assert_eq!(parsed.hints, event.hints);
    }

    #[test]
    fn managed_channel_metadata_round_trip() {
        let event = ManagedChannelMetadataEvent::new(
            "oa-main",
            hex64('2'),
            "wss://chat.openagents.example",
            sample_metadata("provider-ops-renamed"),
            43,
        )
        .unwrap()
        .with_hints(
            ManagedChannelHints::new()
                .with_slug("provider-ops")
                .with_category_id("operations")
                .with_position(121),
        )
        .unwrap();

        let unsigned = event.to_unsigned_event(hex64('3')).unwrap();
        let parsed = ManagedChannelMetadataEvent::from_event(&signed_event(
            &hex64('b'),
            &hex64('3'),
            unsigned.kind,
            unsigned.created_at,
            &unsigned.content,
            unsigned.tags.clone(),
        ))
        .unwrap();

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.channel_create_event_id, hex64('2'));
        assert_eq!(parsed.relay_url, "wss://chat.openagents.example");
        assert_eq!(parsed.metadata, event.metadata);
        assert_eq!(parsed.hints, event.hints);
    }

    #[test]
    fn managed_channel_message_round_trip_with_reply_and_mentions() {
        let reply =
            ManagedMessageReply::new(hex64('4'), "wss://chat.openagents.example", hex64('5'))
                .unwrap();
        let repeated_pubkey = hex64('6');
        let mentions = vec![
            ManagedChatMention::new(repeated_pubkey.clone()).unwrap(),
            ManagedChatMention::new(repeated_pubkey).unwrap(),
            ManagedChatMention::new(hex64('7'))
                .unwrap()
                .with_relay_url("wss://relay.example")
                .unwrap(),
        ];
        let event = ManagedChannelMessageEvent::new(
            "oa-main",
            hex64('8'),
            "wss://chat.openagents.example",
            "Acknowledged.",
            44,
        )
        .unwrap()
        .with_reply(reply)
        .with_mentions(mentions);

        let unsigned = event.to_unsigned_event(hex64('9')).unwrap();
        let p_tags = unsigned
            .tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some("p"))
            .count();
        assert_eq!(p_tags, 3);

        let parsed = ManagedChannelMessageEvent::from_event(&signed_event(
            &hex64('c'),
            &hex64('9'),
            unsigned.kind,
            unsigned.created_at,
            &unsigned.content,
            unsigned.tags.clone(),
        ))
        .unwrap();

        assert_eq!(parsed.reply_to, event.reply_to);
        assert_eq!(parsed.mentions.len(), 2);
        assert_eq!(parsed.mentions[0].pubkey, hex64('6'));
        assert_eq!(
            parsed.mentions[1].relay_url.as_deref(),
            Some("wss://relay.example")
        );
    }

    #[test]
    fn managed_channel_parser_rejects_invalid_contract_tags() {
        let create_missing_group = signed_event(
            &hex64('d'),
            &hex64('1'),
            KIND_CHANNEL_CREATION,
            45,
            &sample_metadata("provider-ops").to_json().unwrap(),
            vec![vec![
                TAG_OA_ROOM_MODE.to_string(),
                ROOM_MODE_MANAGED_CHANNEL.to_string(),
            ]],
        );
        let create_invalid_room_mode = signed_event(
            &hex64('e'),
            &hex64('1'),
            KIND_CHANNEL_CREATION,
            45,
            &sample_metadata("provider-ops").to_json().unwrap(),
            vec![
                vec![TAG_GROUP_CONTEXT.to_string(), "oa-main".to_string()],
                vec![TAG_OA_ROOM_MODE.to_string(), "dm".to_string()],
            ],
        );
        let metadata_missing_root = signed_event(
            &hex64('a'),
            &hex64('1'),
            KIND_CHANNEL_METADATA,
            46,
            &sample_metadata("provider-ops").to_json().unwrap(),
            vec![vec![TAG_GROUP_CONTEXT.to_string(), "oa-main".to_string()]],
        );
        let metadata_invalid_position = signed_event(
            &hex64('b'),
            &hex64('1'),
            KIND_CHANNEL_METADATA,
            46,
            &sample_metadata("provider-ops").to_json().unwrap(),
            vec![
                vec![TAG_GROUP_CONTEXT.to_string(), "oa-main".to_string()],
                root_channel_tag(&hex64('2'), "wss://chat.openagents.example"),
                vec![TAG_OA_POSITION.to_string(), "abc".to_string()],
            ],
        );
        let reply_missing_author_p = signed_event(
            &hex64('c'),
            &hex64('1'),
            KIND_CHANNEL_MESSAGE,
            47,
            "reply",
            vec![
                vec![TAG_GROUP_CONTEXT.to_string(), "oa-main".to_string()],
                root_channel_tag(&hex64('2'), "wss://chat.openagents.example"),
                vec![
                    "e".to_string(),
                    hex64('3'),
                    "wss://chat.openagents.example".to_string(),
                    REPLY_MARKER.to_string(),
                    hex64('4'),
                ],
            ],
        );

        assert!(matches!(
            ManagedChannelCreateEvent::from_event(&create_missing_group),
            Err(ManagedChatError::MissingTag(tag)) if tag == TAG_GROUP_CONTEXT
        ));
        assert!(matches!(
            ManagedChannelCreateEvent::from_event(&create_invalid_room_mode),
            Err(ManagedChatError::UnsupportedRoomMode(mode)) if mode == "dm"
        ));
        assert!(matches!(
            ManagedChannelMetadataEvent::from_event(&metadata_missing_root),
            Err(ManagedChatError::MissingTag(tag)) if tag == "e(root)"
        ));
        assert!(matches!(
            ManagedChannelMetadataEvent::from_event(&metadata_invalid_position),
            Err(ManagedChatError::InvalidTag(tag)) if tag == "oa-position:abc"
        ));
        assert!(matches!(
            ManagedChannelMessageEvent::from_event(&reply_missing_author_p),
            Err(ManagedChatError::MissingTag(tag)) if tag == "p(reply author)"
        ));
    }

    #[test]
    fn managed_channel_projection_helpers_are_deterministic() {
        let mut channels = vec![
            ManagedChannelIndexEntry::new(
                hex64('4'),
                sample_metadata("zeta"),
                ManagedChannelHints::new(),
            )
            .unwrap(),
            ManagedChannelIndexEntry::new(
                hex64('3'),
                sample_metadata("alpha"),
                ManagedChannelHints::new()
                    .with_category_id("operations")
                    .with_position(20),
            )
            .unwrap(),
            ManagedChannelIndexEntry::new(
                hex64('2'),
                sample_metadata("beta"),
                ManagedChannelHints::new()
                    .with_category_id("operations")
                    .with_position(10),
            )
            .unwrap(),
        ];
        sort_managed_channel_index(&mut channels);

        assert_eq!(channels[0].channel_create_event_id, hex64('4'));
        assert_eq!(channels[1].channel_create_event_id, hex64('2'));
        assert_eq!(channels[2].channel_create_event_id, hex64('3'));

        let projected = project_managed_channel_timeline(vec![
            signed_event(
                &hex64('c'),
                &hex64('1'),
                KIND_CHANNEL_MESSAGE,
                9,
                "later",
                vec![],
            ),
            signed_event(
                &hex64('a'),
                &hex64('1'),
                KIND_CHANNEL_MESSAGE,
                7,
                "first",
                vec![],
            ),
            signed_event(
                &hex64('b'),
                &hex64('1'),
                KIND_CHANNEL_MESSAGE,
                7,
                "same-time",
                vec![],
            ),
            signed_event(
                &hex64('a'),
                &hex64('1'),
                KIND_CHANNEL_MESSAGE,
                7,
                "duplicate",
                vec![],
            ),
        ]);

        assert_eq!(
            projected
                .iter()
                .map(|event| event.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                hex64('a').as_str(),
                hex64('b').as_str(),
                hex64('c').as_str()
            ]
        );
    }
}
