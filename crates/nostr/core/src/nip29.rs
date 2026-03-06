//! NIP-29: Relay-based Groups
//!
//! This module provides reusable builders, parsers, and validators for relay-based
//! group events. It intentionally stays protocol-level and avoids OpenAgents app
//! behavior or UI-specific policy.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::nip01::{Event, UnsignedEvent};
use crate::tag_parsing::{collect_tag_values, find_tag_value, tag_field, tag_name};

/// Group metadata event kind.
pub const KIND_GROUP_METADATA: u16 = 39000;
/// Group admins event kind.
pub const KIND_GROUP_ADMINS: u16 = 39001;
/// Group members event kind.
pub const KIND_GROUP_MEMBERS: u16 = 39002;
/// Group roles event kind.
pub const KIND_GROUP_ROLES: u16 = 39003;

/// Put user moderation event kind.
pub const KIND_PUT_USER: u16 = 9000;
/// Remove user moderation event kind.
pub const KIND_REMOVE_USER: u16 = 9001;
/// Edit metadata moderation event kind.
pub const KIND_EDIT_METADATA: u16 = 9002;
/// Delete event moderation event kind.
pub const KIND_DELETE_EVENT: u16 = 9005;
/// Create group moderation event kind.
pub const KIND_CREATE_GROUP: u16 = 9007;
/// Delete group moderation event kind.
pub const KIND_DELETE_GROUP: u16 = 9008;
/// Create invite moderation event kind.
pub const KIND_CREATE_INVITE: u16 = 9009;

/// Join request event kind.
pub const KIND_JOIN_REQUEST: u16 = 9021;
/// Leave request event kind.
pub const KIND_LEAVE_REQUEST: u16 = 9022;

/// Errors that can occur during NIP-29 operations.
#[derive(Debug, Error)]
pub enum Nip29Error {
    #[error("invalid kind: expected {expected}, got {got}")]
    InvalidKind { expected: String, got: u16 },

    #[error("invalid group id: {0}")]
    InvalidGroupId(String),

    #[error("invalid group identifier: {0}")]
    InvalidGroupIdentifier(String),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid tag: {0}")]
    InvalidTag(String),

    #[error("invalid pubkey: {0}")]
    InvalidPubkey(String),

    #[error("invalid event id: {0}")]
    InvalidEventId(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Result type for NIP-29 operations.
pub type Result<T> = std::result::Result<T, Nip29Error>;

/// Validate a NIP-29 group id.
pub fn validate_group_id(group_id: &str) -> bool {
    !group_id.is_empty()
        && group_id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

/// Validate a relay-qualified group identifier (`<host>'<group-id>`).
pub fn validate_group_identifier(identifier: &str) -> bool {
    let Some((host, group_id)) = identifier.split_once('\'') else {
        return false;
    };
    !host.is_empty() && validate_group_id(group_id)
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
    Err(Nip29Error::InvalidGroupId(group_id.to_string()))
}

fn validate_pubkey_or_err(pubkey: &str) -> Result<()> {
    if validate_hex_64(pubkey) {
        return Ok(());
    }
    Err(Nip29Error::InvalidPubkey(pubkey.to_string()))
}

fn validate_event_id_or_err(event_id: &str) -> Result<()> {
    if validate_hex_64(event_id) {
        return Ok(());
    }
    Err(Nip29Error::InvalidEventId(event_id.to_string()))
}

fn parse_group_id(tags: &[Vec<String>], tag_name_key: &str) -> Result<String> {
    let group_id = find_tag_value(tags, tag_name_key)
        .ok_or_else(|| Nip29Error::MissingTag(tag_name_key.to_string()))?;
    validate_group_id_or_err(group_id)?;
    Ok(group_id.to_string())
}

fn previous_tags(previous_refs: &[String]) -> Result<Vec<Vec<String>>> {
    let mut tags = Vec::with_capacity(previous_refs.len());
    for reference in previous_refs {
        if reference.len() != 8
            || !reference
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() && ch.is_ascii_lowercase())
        {
            return Err(Nip29Error::InvalidTag(format!(
                "invalid previous reference: {reference}"
            )));
        }
        tags.push(vec!["previous".to_string(), reference.clone()]);
    }
    Ok(tags)
}

fn optional_reason(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Group metadata flags from `kind:39000`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMetadata {
    pub name: Option<String>,
    pub picture: Option<String>,
    pub about: Option<String>,
    pub private: bool,
    pub restricted: bool,
    pub hidden: bool,
    pub closed: bool,
}

impl GroupMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_picture(mut self, picture: impl Into<String>) -> Self {
        self.picture = Some(picture.into());
        self
    }

    pub fn with_about(mut self, about: impl Into<String>) -> Self {
        self.about = Some(about.into());
        self
    }

    pub fn with_private(mut self, value: bool) -> Self {
        self.private = value;
        self
    }

    pub fn with_restricted(mut self, value: bool) -> Self {
        self.restricted = value;
        self
    }

    pub fn with_hidden(mut self, value: bool) -> Self {
        self.hidden = value;
        self
    }

    pub fn with_closed(mut self, value: bool) -> Self {
        self.closed = value;
        self
    }
}

/// Relay-generated group metadata event (`kind:39000`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMetadataEvent {
    pub group_id: String,
    pub metadata: GroupMetadata,
    pub content: String,
    pub created_at: u64,
}

impl GroupMetadataEvent {
    pub fn new(
        group_id: impl Into<String>,
        metadata: GroupMetadata,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            metadata,
            content: String::new(),
            created_at,
        })
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.group_id.clone()]];
        if let Some(name) = &self.metadata.name {
            tags.push(vec!["name".to_string(), name.clone()]);
        }
        if let Some(picture) = &self.metadata.picture {
            tags.push(vec!["picture".to_string(), picture.clone()]);
        }
        if let Some(about) = &self.metadata.about {
            tags.push(vec!["about".to_string(), about.clone()]);
        }
        if self.metadata.private {
            tags.push(vec!["private".to_string()]);
        }
        if self.metadata.restricted {
            tags.push(vec!["restricted".to_string()]);
        }
        if self.metadata.hidden {
            tags.push(vec!["hidden".to_string()]);
        }
        if self.metadata.closed {
            tags.push(vec!["closed".to_string()]);
        }
        tags
    }

    pub fn to_unsigned_event(&self, relay_pubkey: impl Into<String>) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: relay_pubkey.into(),
            created_at: self.created_at,
            kind: KIND_GROUP_METADATA,
            tags: self.to_tags(),
            content: self.content.clone(),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_GROUP_METADATA {
            return Err(Nip29Error::InvalidKind {
                expected: KIND_GROUP_METADATA.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags, "d")?;
        let metadata = GroupMetadata {
            name: find_tag_value(&event.tags, "name").map(str::to_owned),
            picture: find_tag_value(&event.tags, "picture").map(str::to_owned),
            about: find_tag_value(&event.tags, "about").map(str::to_owned),
            private: event
                .tags
                .iter()
                .any(|tag| matches!(tag_name(tag), Some("private"))),
            restricted: event
                .tags
                .iter()
                .any(|tag| matches!(tag_name(tag), Some("restricted"))),
            hidden: event
                .tags
                .iter()
                .any(|tag| matches!(tag_name(tag), Some("hidden"))),
            closed: event
                .tags
                .iter()
                .any(|tag| matches!(tag_name(tag), Some("closed"))),
        };

        Ok(Self {
            group_id,
            metadata,
            content: event.content.clone(),
            created_at: event.created_at,
        })
    }
}

/// A pubkey tagged in a relay-generated group event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaggedPubkey {
    pub pubkey: String,
    pub labels: Vec<String>,
}

impl TaggedPubkey {
    pub fn new(pubkey: impl Into<String>) -> Result<Self> {
        let pubkey = pubkey.into();
        validate_pubkey_or_err(&pubkey)?;
        Ok(Self {
            pubkey,
            labels: Vec::new(),
        })
    }

    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    fn to_p_tag(&self) -> Vec<String> {
        let mut tag = vec!["p".to_string(), self.pubkey.clone()];
        tag.extend(self.labels.clone());
        tag
    }
}

/// Relay-generated group admins event (`kind:39001`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupAdminsEvent {
    pub group_id: String,
    pub admins: Vec<TaggedPubkey>,
    pub content: String,
    pub created_at: u64,
}

impl GroupAdminsEvent {
    pub fn new(
        group_id: impl Into<String>,
        admins: Vec<TaggedPubkey>,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            admins,
            content: String::new(),
            created_at,
        })
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.group_id.clone()]];
        tags.extend(self.admins.iter().map(TaggedPubkey::to_p_tag));
        tags
    }

    pub fn to_unsigned_event(&self, relay_pubkey: impl Into<String>) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: relay_pubkey.into(),
            created_at: self.created_at,
            kind: KIND_GROUP_ADMINS,
            tags: self.to_tags(),
            content: self.content.clone(),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_GROUP_ADMINS {
            return Err(Nip29Error::InvalidKind {
                expected: KIND_GROUP_ADMINS.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags, "d")?;
        let mut admins = Vec::new();
        for tag in event
            .tags
            .iter()
            .filter(|tag| matches!(tag_name(tag), Some("p")))
        {
            let pubkey =
                tag_field(tag, 1).ok_or_else(|| Nip29Error::MissingTag("p".to_string()))?;
            validate_pubkey_or_err(pubkey)?;
            let labels = tag.iter().skip(2).cloned().collect();
            admins.push(TaggedPubkey {
                pubkey: pubkey.to_string(),
                labels,
            });
        }

        Ok(Self {
            group_id,
            admins,
            content: event.content.clone(),
            created_at: event.created_at,
        })
    }
}

/// Relay-generated group members event (`kind:39002`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMembersEvent {
    pub group_id: String,
    pub members: Vec<TaggedPubkey>,
    pub content: String,
    pub created_at: u64,
}

impl GroupMembersEvent {
    pub fn new(
        group_id: impl Into<String>,
        members: Vec<TaggedPubkey>,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            members,
            content: String::new(),
            created_at,
        })
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.group_id.clone()]];
        tags.extend(self.members.iter().map(TaggedPubkey::to_p_tag));
        tags
    }

    pub fn to_unsigned_event(&self, relay_pubkey: impl Into<String>) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: relay_pubkey.into(),
            created_at: self.created_at,
            kind: KIND_GROUP_MEMBERS,
            tags: self.to_tags(),
            content: self.content.clone(),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_GROUP_MEMBERS {
            return Err(Nip29Error::InvalidKind {
                expected: KIND_GROUP_MEMBERS.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags, "d")?;
        let mut members = Vec::new();
        for tag in event
            .tags
            .iter()
            .filter(|tag| matches!(tag_name(tag), Some("p")))
        {
            let pubkey =
                tag_field(tag, 1).ok_or_else(|| Nip29Error::MissingTag("p".to_string()))?;
            validate_pubkey_or_err(pubkey)?;
            let labels = tag.iter().skip(2).cloned().collect();
            members.push(TaggedPubkey {
                pubkey: pubkey.to_string(),
                labels,
            });
        }

        Ok(Self {
            group_id,
            members,
            content: event.content.clone(),
            created_at: event.created_at,
        })
    }
}

/// A role declared in a relay-generated roles event (`kind:39003`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupRole {
    pub name: String,
    pub description: Option<String>,
}

impl GroupRole {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    fn to_role_tag(&self) -> Vec<String> {
        let mut tag = vec!["role".to_string(), self.name.clone()];
        if let Some(description) = &self.description {
            tag.push(description.clone());
        }
        tag
    }
}

/// Relay-generated roles event (`kind:39003`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRolesEvent {
    pub group_id: String,
    pub roles: Vec<GroupRole>,
    pub content: String,
    pub created_at: u64,
}

impl GroupRolesEvent {
    pub fn new(
        group_id: impl Into<String>,
        roles: Vec<GroupRole>,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            roles,
            content: String::new(),
            created_at,
        })
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["d".to_string(), self.group_id.clone()]];
        tags.extend(self.roles.iter().map(GroupRole::to_role_tag));
        tags
    }

    pub fn to_unsigned_event(&self, relay_pubkey: impl Into<String>) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: relay_pubkey.into(),
            created_at: self.created_at,
            kind: KIND_GROUP_ROLES,
            tags: self.to_tags(),
            content: self.content.clone(),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_GROUP_ROLES {
            return Err(Nip29Error::InvalidKind {
                expected: KIND_GROUP_ROLES.to_string(),
                got: event.kind,
            });
        }

        let group_id = parse_group_id(&event.tags, "d")?;
        let mut roles = Vec::new();
        for tag in event
            .tags
            .iter()
            .filter(|tag| matches!(tag_name(tag), Some("role")))
        {
            let name = tag_field(tag, 1)
                .ok_or_else(|| Nip29Error::MissingTag("role".to_string()))?
                .to_string();
            let description = tag_field(tag, 2).map(str::to_owned);
            roles.push(GroupRole { name, description });
        }

        Ok(Self {
            group_id,
            roles,
            content: event.content.clone(),
            created_at: event.created_at,
        })
    }
}

/// User join request (`kind:9021`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequestEvent {
    pub group_id: String,
    pub reason: Option<String>,
    pub invite_code: Option<String>,
    pub created_at: u64,
}

impl JoinRequestEvent {
    pub fn new(group_id: impl Into<String>, created_at: u64) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            reason: None,
            invite_code: None,
            created_at,
        })
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_invite_code(mut self, invite_code: impl Into<String>) -> Self {
        self.invite_code = Some(invite_code.into());
        self
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec!["h".to_string(), self.group_id.clone()]];
        if let Some(invite_code) = &self.invite_code {
            tags.push(vec!["code".to_string(), invite_code.clone()]);
        }
        tags
    }

    pub fn to_unsigned_event(&self, pubkey: impl Into<String>) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: pubkey.into(),
            created_at: self.created_at,
            kind: KIND_JOIN_REQUEST,
            tags: self.to_tags(),
            content: self.reason.clone().unwrap_or_default(),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_JOIN_REQUEST {
            return Err(Nip29Error::InvalidKind {
                expected: KIND_JOIN_REQUEST.to_string(),
                got: event.kind,
            });
        }

        Ok(Self {
            group_id: parse_group_id(&event.tags, "h")?,
            reason: optional_reason(&event.content),
            invite_code: find_tag_value(&event.tags, "code").map(str::to_owned),
            created_at: event.created_at,
        })
    }
}

/// User leave request (`kind:9022`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaveRequestEvent {
    pub group_id: String,
    pub reason: Option<String>,
    pub created_at: u64,
}

impl LeaveRequestEvent {
    pub fn new(group_id: impl Into<String>, created_at: u64) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            reason: None,
            created_at,
        })
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        vec![vec!["h".to_string(), self.group_id.clone()]]
    }

    pub fn to_unsigned_event(&self, pubkey: impl Into<String>) -> UnsignedEvent {
        UnsignedEvent {
            pubkey: pubkey.into(),
            created_at: self.created_at,
            kind: KIND_LEAVE_REQUEST,
            tags: self.to_tags(),
            content: self.reason.clone().unwrap_or_default(),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        if event.kind != KIND_LEAVE_REQUEST {
            return Err(Nip29Error::InvalidKind {
                expected: KIND_LEAVE_REQUEST.to_string(),
                got: event.kind,
            });
        }

        Ok(Self {
            group_id: parse_group_id(&event.tags, "h")?,
            reason: optional_reason(&event.content),
            created_at: event.created_at,
        })
    }
}

/// Parsed moderation action for `kind:9000-9020`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationAction {
    PutUser { pubkey: String, roles: Vec<String> },
    RemoveUser { pubkey: String },
    EditMetadata { changes: Vec<Vec<String>> },
    DeleteEvent { event_id: String },
    CreateGroup,
    DeleteGroup,
    CreateInvite { code: String },
}

/// Generic user or relay moderation event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationEvent {
    pub group_id: String,
    pub action: ModerationAction,
    pub reason: Option<String>,
    pub previous_refs: Vec<String>,
    pub created_at: u64,
}

impl ModerationEvent {
    pub fn new(
        group_id: impl Into<String>,
        action: ModerationAction,
        created_at: u64,
    ) -> Result<Self> {
        let group_id = group_id.into();
        validate_group_id_or_err(&group_id)?;
        Ok(Self {
            group_id,
            action,
            reason: None,
            previous_refs: Vec::new(),
            created_at,
        })
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_previous_refs(mut self, previous_refs: Vec<String>) -> Self {
        self.previous_refs = previous_refs;
        self
    }

    pub fn kind(&self) -> u16 {
        match self.action {
            ModerationAction::PutUser { .. } => KIND_PUT_USER,
            ModerationAction::RemoveUser { .. } => KIND_REMOVE_USER,
            ModerationAction::EditMetadata { .. } => KIND_EDIT_METADATA,
            ModerationAction::DeleteEvent { .. } => KIND_DELETE_EVENT,
            ModerationAction::CreateGroup => KIND_CREATE_GROUP,
            ModerationAction::DeleteGroup => KIND_DELETE_GROUP,
            ModerationAction::CreateInvite { .. } => KIND_CREATE_INVITE,
        }
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>> {
        let mut tags = vec![vec!["h".to_string(), self.group_id.clone()]];
        tags.extend(previous_tags(&self.previous_refs)?);

        match &self.action {
            ModerationAction::PutUser { pubkey, roles } => {
                validate_pubkey_or_err(pubkey)?;
                let mut p_tag = vec!["p".to_string(), pubkey.clone()];
                p_tag.extend(roles.clone());
                tags.push(p_tag);
            }
            ModerationAction::RemoveUser { pubkey } => {
                validate_pubkey_or_err(pubkey)?;
                tags.push(vec!["p".to_string(), pubkey.clone()]);
            }
            ModerationAction::EditMetadata { changes } => {
                tags.extend(changes.clone());
            }
            ModerationAction::DeleteEvent { event_id } => {
                validate_event_id_or_err(event_id)?;
                tags.push(vec!["e".to_string(), event_id.clone()]);
            }
            ModerationAction::CreateGroup => {}
            ModerationAction::DeleteGroup => {}
            ModerationAction::CreateInvite { code } => {
                if code.trim().is_empty() {
                    return Err(Nip29Error::InvalidTag(
                        "invite code must not be empty".to_string(),
                    ));
                }
                tags.push(vec!["code".to_string(), code.clone()]);
            }
        }

        Ok(tags)
    }

    pub fn to_unsigned_event(&self, pubkey: impl Into<String>) -> Result<UnsignedEvent> {
        Ok(UnsignedEvent {
            pubkey: pubkey.into(),
            created_at: self.created_at,
            kind: self.kind(),
            tags: self.to_tags()?,
            content: self.reason.clone().unwrap_or_default(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self> {
        let action = match event.kind {
            KIND_PUT_USER => {
                let put_tag = event
                    .tags
                    .iter()
                    .find(|tag| matches!(tag_name(tag), Some("p")))
                    .ok_or_else(|| Nip29Error::MissingTag("p".to_string()))?;
                let pubkey =
                    tag_field(put_tag, 1).ok_or_else(|| Nip29Error::MissingTag("p".to_string()))?;
                validate_pubkey_or_err(pubkey)?;
                ModerationAction::PutUser {
                    pubkey: pubkey.to_string(),
                    roles: put_tag.iter().skip(2).cloned().collect(),
                }
            }
            KIND_REMOVE_USER => {
                let pubkey = find_tag_value(&event.tags, "p")
                    .ok_or_else(|| Nip29Error::MissingTag("p".to_string()))?;
                validate_pubkey_or_err(pubkey)?;
                ModerationAction::RemoveUser {
                    pubkey: pubkey.to_string(),
                }
            }
            KIND_EDIT_METADATA => {
                let changes = event
                    .tags
                    .iter()
                    .filter(|tag| !matches!(tag_name(tag), Some("h") | Some("previous")))
                    .cloned()
                    .collect();
                ModerationAction::EditMetadata { changes }
            }
            KIND_DELETE_EVENT => {
                let event_id = find_tag_value(&event.tags, "e")
                    .ok_or_else(|| Nip29Error::MissingTag("e".to_string()))?;
                validate_event_id_or_err(event_id)?;
                ModerationAction::DeleteEvent {
                    event_id: event_id.to_string(),
                }
            }
            KIND_CREATE_GROUP => ModerationAction::CreateGroup,
            KIND_DELETE_GROUP => ModerationAction::DeleteGroup,
            KIND_CREATE_INVITE => {
                let code = find_tag_value(&event.tags, "code")
                    .ok_or_else(|| Nip29Error::MissingTag("code".to_string()))?;
                if code.trim().is_empty() {
                    return Err(Nip29Error::InvalidTag(
                        "invite code must not be empty".to_string(),
                    ));
                }
                ModerationAction::CreateInvite {
                    code: code.to_string(),
                }
            }
            got => {
                return Err(Nip29Error::InvalidKind {
                    expected: "9000-9020".to_string(),
                    got,
                });
            }
        };

        Ok(Self {
            group_id: parse_group_id(&event.tags, "h")?,
            action,
            reason: optional_reason(&event.content),
            previous_refs: collect_tag_values(&event.tags, "previous"),
            created_at: event.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_event(kind: u16, content: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "a".repeat(64),
            pubkey: "b".repeat(64),
            created_at: 1_700_000_000,
            kind,
            tags,
            content: content.to_string(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn validate_group_id_accepts_allowed_characters() {
        assert!(validate_group_id("oa-main"));
        assert!(validate_group_id("ops_01"));
        assert!(!validate_group_id("Ops"));
        assert!(!validate_group_id("group space"));
        assert!(!validate_group_id(""));
    }

    #[test]
    fn validate_group_identifier_requires_host_and_group() {
        assert!(validate_group_identifier("groups.openagents.com'oa-main"));
        assert!(!validate_group_identifier("oa-main"));
        assert!(!validate_group_identifier("'oa-main"));
    }

    #[test]
    fn group_metadata_roundtrip_preserves_flags_and_fields() {
        let metadata = GroupMetadata::new()
            .with_name("Ops")
            .with_about("Operations")
            .with_private(true)
            .with_restricted(true);
        let event =
            GroupMetadataEvent::new("oa-main", metadata.clone(), 42).expect("build metadata event");
        let parsed =
            GroupMetadataEvent::from_event(&signed_event(KIND_GROUP_METADATA, "", event.to_tags()))
                .expect("parse metadata event");

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.metadata, metadata);
    }

    #[test]
    fn group_admins_roundtrip_preserves_roles() {
        let admin = TaggedPubkey::new("d".repeat(64))
            .expect("build pubkey")
            .with_labels(vec!["admin".to_string(), "moderator".to_string()]);
        let event = GroupAdminsEvent::new("oa-main", vec![admin.clone()], 42).expect("admins");
        let parsed = GroupAdminsEvent::from_event(&signed_event(
            KIND_GROUP_ADMINS,
            "admins",
            event.to_tags(),
        ))
        .expect("parse admins");

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.admins, vec![admin]);
    }

    #[test]
    fn join_request_roundtrip_preserves_code_and_reason() {
        let request = JoinRequestEvent::new("oa-main", 42)
            .expect("join request")
            .with_reason("let me in")
            .with_invite_code("alpha");
        let parsed = JoinRequestEvent::from_event(&signed_event(
            KIND_JOIN_REQUEST,
            "let me in",
            request.to_tags(),
        ))
        .expect("parse join request");

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.reason.as_deref(), Some("let me in"));
        assert_eq!(parsed.invite_code.as_deref(), Some("alpha"));
    }

    #[test]
    fn leave_request_roundtrip_preserves_reason() {
        let request = LeaveRequestEvent::new("oa-main", 42)
            .expect("leave request")
            .with_reason("done");
        let parsed = LeaveRequestEvent::from_event(&signed_event(
            KIND_LEAVE_REQUEST,
            "done",
            request.to_tags(),
        ))
        .expect("parse leave request");

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.reason.as_deref(), Some("done"));
    }

    #[test]
    fn moderation_put_user_roundtrip_preserves_roles_and_previous_refs() {
        let moderation = ModerationEvent::new(
            "oa-main",
            ModerationAction::PutUser {
                pubkey: "d".repeat(64),
                roles: vec!["moderator".to_string()],
            },
            42,
        )
        .expect("moderation")
        .with_reason("approved")
        .with_previous_refs(vec!["deadbeef".to_string()]);

        let parsed = ModerationEvent::from_event(&signed_event(
            KIND_PUT_USER,
            "approved",
            moderation.to_tags().expect("serialize tags"),
        ))
        .expect("parse moderation");

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.previous_refs, vec!["deadbeef".to_string()]);
        assert_eq!(parsed.reason.as_deref(), Some("approved"));
        assert_eq!(parsed.action, moderation.action);
    }

    #[test]
    fn moderation_delete_event_requires_hex_event_id() {
        let result = ModerationEvent::new(
            "oa-main",
            ModerationAction::DeleteEvent {
                event_id: "not-hex".to_string(),
            },
            42,
        )
        .expect("build moderation")
        .to_tags();

        assert!(matches!(result, Err(Nip29Error::InvalidEventId(_))));
    }

    #[test]
    fn group_roles_roundtrip_preserves_descriptions() {
        let roles = vec![
            GroupRole::new("admin").with_description("full access"),
            GroupRole::new("moderator"),
        ];
        let event = GroupRolesEvent::new("oa-main", roles.clone(), 42).expect("roles");
        let parsed =
            GroupRolesEvent::from_event(&signed_event(KIND_GROUP_ROLES, "roles", event.to_tags()))
                .expect("parse roles");

        assert_eq!(parsed.group_id, "oa-main");
        assert_eq!(parsed.roles, roles);
    }
}
