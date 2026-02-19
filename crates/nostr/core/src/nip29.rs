//! NIP-29: Relay-based Groups
//!
//! This NIP defines a standard for groups that are only writable by a closed set
//! of users. Groups are identified by random strings and managed through relay
//! moderation events.
//!
//! ## Purpose
//!
//! Enable relay-based group chat and community management with:
//! - Member and admin management
//! - Group metadata and configuration
//! - Moderation actions
//! - Public or private groups
//!
//! ## Event Kinds
//!
//! User events:
//! - Kind 9021: Join request
//! - Kind 9022: Leave request
//!
//! Moderation events:
//! - Kind 9000: Add user with roles
//! - Kind 9001: Remove user
//! - Kind 9002: Edit metadata
//! - Kind 9005: Delete event
//! - Kind 9007: Create group
//! - Kind 9008: Delete group
//! - Kind 9009: Create invite
//!
//! Group metadata (relay-generated):
//! - Kind 39000: Group metadata
//! - Kind 39001: Group admins
//! - Kind 39002: Group members
//! - Kind 39003: Group roles
//!
//! ## Examples
//!
//! ```
//! use nostr::nip29::{GroupMetadata, ModerationAction};
//!
//! // Create group metadata
//! let metadata = GroupMetadata::new("pizza-lovers")
//!     .with_name("Pizza Lovers")
//!     .with_about("A group for people who love pizza")
//!     .with_private(true);
//!
//! // Create a moderation action to add a user
//! let action = ModerationAction::put_user("group-id", "pubkey-hex")
//!     .with_roles(vec!["admin", "moderator"]);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for join requests
pub const KIND_JOIN_REQUEST: u64 = 9021;

/// Event kind for leave requests
pub const KIND_LEAVE_REQUEST: u64 = 9022;

/// Event kind for put-user moderation
pub const KIND_MOD_PUT_USER: u64 = 9000;

/// Event kind for remove-user moderation
pub const KIND_MOD_REMOVE_USER: u64 = 9001;

/// Event kind for edit-metadata moderation
pub const KIND_MOD_EDIT_METADATA: u64 = 9002;

/// Event kind for delete-event moderation
pub const KIND_MOD_DELETE_EVENT: u64 = 9005;

/// Event kind for create-group moderation
pub const KIND_MOD_CREATE_GROUP: u64 = 9007;

/// Event kind for delete-group moderation
pub const KIND_MOD_DELETE_GROUP: u64 = 9008;

/// Event kind for create-invite moderation
pub const KIND_MOD_CREATE_INVITE: u64 = 9009;

/// Event kind for group metadata
pub const KIND_GROUP_METADATA: u64 = 39000;

/// Event kind for group admins
pub const KIND_GROUP_ADMINS: u64 = 39001;

/// Event kind for group members
pub const KIND_GROUP_MEMBERS: u64 = 39002;

/// Event kind for group roles
pub const KIND_GROUP_ROLES: u64 = 39003;

/// Tag name for group ID
pub const GROUP_TAG: &str = "h";

/// Tag name for addressable group events
pub const GROUP_D_TAG: &str = "d";

/// Tag name for previous event references
pub const PREVIOUS_TAG: &str = "previous";

/// Errors that can occur during NIP-29 operations
#[derive(Debug, Error)]
pub enum Nip29Error {
    #[error("group ID cannot be empty")]
    EmptyGroupId,

    #[error("invalid group ID: {0} (must contain only a-z0-9-_)")]
    InvalidGroupId(String),

    #[error("pubkey cannot be empty")]
    EmptyPubkey,

    #[error("event ID cannot be empty")]
    EmptyEventId,

    #[error("group identifier must contain ' separator")]
    InvalidGroupIdentifier,
}

/// Check if an event kind is a NIP-29 group kind
pub fn is_group_kind(kind: u64) -> bool {
    matches!(
        kind,
        KIND_JOIN_REQUEST
            | KIND_LEAVE_REQUEST
            | KIND_MOD_PUT_USER
            | KIND_MOD_REMOVE_USER
            | KIND_MOD_EDIT_METADATA
            | KIND_MOD_DELETE_EVENT
            | KIND_MOD_CREATE_GROUP
            | KIND_MOD_DELETE_GROUP
            | KIND_MOD_CREATE_INVITE
            | KIND_GROUP_METADATA
            | KIND_GROUP_ADMINS
            | KIND_GROUP_MEMBERS
            | KIND_GROUP_ROLES
    )
}

/// Check if an event kind is a group moderation kind
pub fn is_group_moderation_kind(kind: u64) -> bool {
    (9000..=9020).contains(&kind)
}

/// Check if an event kind is a group metadata kind
pub fn is_group_metadata_kind(kind: u64) -> bool {
    (39000..=39009).contains(&kind)
}

/// Validate a group ID (must be a-z0-9-_)
pub fn validate_group_id(id: &str) -> Result<(), Nip29Error> {
    if id.is_empty() {
        return Err(Nip29Error::EmptyGroupId);
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(Nip29Error::InvalidGroupId(id.to_string()));
    }
    Ok(())
}

/// Parse a group identifier into (host, group_id)
pub fn parse_group_identifier(identifier: &str) -> Result<(String, String), Nip29Error> {
    let parts: Vec<&str> = identifier.split('\'').collect();
    if parts.len() != 2 {
        return Err(Nip29Error::InvalidGroupIdentifier);
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Format a group identifier from host and group_id
pub fn format_group_identifier(host: &str, group_id: &str) -> String {
    format!("{}'{}", host, group_id)
}

/// Group metadata (kind 39000)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupMetadata {
    /// Group ID (d tag)
    pub id: String,
    /// Group name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Group picture URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// Group description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    /// Private group (members only can read)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    /// Restricted group (members only can write)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted: Option<bool>,
    /// Hidden group (metadata hidden from non-members)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Closed group (join requests ignored)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
}

impl GroupMetadata {
    /// Create new group metadata
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            picture: None,
            about: None,
            private: None,
            restricted: None,
            hidden: None,
            closed: None,
        }
    }

    /// Set the group name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the group picture
    pub fn with_picture(mut self, url: impl Into<String>) -> Self {
        self.picture = Some(url.into());
        self
    }

    /// Set the group description
    pub fn with_about(mut self, about: impl Into<String>) -> Self {
        self.about = Some(about.into());
        self
    }

    /// Set if the group is private
    pub fn with_private(mut self, private: bool) -> Self {
        self.private = Some(private);
        self
    }

    /// Set if the group is restricted
    pub fn with_restricted(mut self, restricted: bool) -> Self {
        self.restricted = Some(restricted);
        self
    }

    /// Set if the group is hidden
    pub fn with_hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// Set if the group is closed
    pub fn with_closed(mut self, closed: bool) -> Self {
        self.closed = Some(closed);
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec![GROUP_D_TAG.to_string(), self.id.clone()]];

        if let Some(name) = &self.name {
            tags.push(vec!["name".to_string(), name.clone()]);
        }
        if let Some(picture) = &self.picture {
            tags.push(vec!["picture".to_string(), picture.clone()]);
        }
        if let Some(about) = &self.about {
            tags.push(vec!["about".to_string(), about.clone()]);
        }
        if self.private == Some(true) {
            tags.push(vec!["private".to_string()]);
        }
        if self.restricted == Some(true) {
            tags.push(vec!["restricted".to_string()]);
        }
        if self.hidden == Some(true) {
            tags.push(vec!["hidden".to_string()]);
        }
        if self.closed == Some(true) {
            tags.push(vec!["closed".to_string()]);
        }

        tags
    }

    /// Validate the group metadata
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(&self.id)
    }
}

/// Admin with roles
#[derive(Debug, Clone, PartialEq)]
pub struct Admin {
    /// Admin pubkey
    pub pubkey: String,
    /// Admin roles
    pub roles: Vec<String>,
}

impl Admin {
    /// Create a new admin
    pub fn new(pubkey: impl Into<String>, roles: Vec<impl Into<String>>) -> Self {
        Self {
            pubkey: pubkey.into(),
            roles: roles.into_iter().map(|r| r.into()).collect(),
        }
    }
}

/// Group admins (kind 39001)
#[derive(Debug, Clone, PartialEq)]
pub struct GroupAdmins {
    /// Group ID (d tag)
    pub id: String,
    /// List of admins with their roles
    pub admins: Vec<Admin>,
}

impl GroupAdmins {
    /// Create new group admins list
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            admins: Vec::new(),
        }
    }

    /// Add an admin
    pub fn with_admin(mut self, pubkey: impl Into<String>, roles: Vec<impl Into<String>>) -> Self {
        self.admins.push(Admin::new(pubkey, roles));
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec![GROUP_D_TAG.to_string(), self.id.clone()]];

        for admin in &self.admins {
            let mut tag = vec!["p".to_string(), admin.pubkey.clone()];
            tag.extend(admin.roles.iter().cloned());
            tags.push(tag);
        }

        tags
    }

    /// Validate the group admins
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(&self.id)?;
        for admin in &self.admins {
            if admin.pubkey.trim().is_empty() {
                return Err(Nip29Error::EmptyPubkey);
            }
        }
        Ok(())
    }
}

/// Group members (kind 39002)
#[derive(Debug, Clone, PartialEq)]
pub struct GroupMembers {
    /// Group ID (d tag)
    pub id: String,
    /// List of member pubkeys
    pub members: Vec<String>,
}

impl GroupMembers {
    /// Create new group members list
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            members: Vec::new(),
        }
    }

    /// Add a member
    pub fn with_member(mut self, pubkey: impl Into<String>) -> Self {
        self.members.push(pubkey.into());
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec![GROUP_D_TAG.to_string(), self.id.clone()]];

        for member in &self.members {
            tags.push(vec!["p".to_string(), member.clone()]);
        }

        tags
    }

    /// Validate the group members
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(&self.id)?;
        for member in &self.members {
            if member.trim().is_empty() {
                return Err(Nip29Error::EmptyPubkey);
            }
        }
        Ok(())
    }
}

/// Role definition
#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    /// Role name
    pub name: String,
    /// Optional role description
    pub description: Option<String>,
}

impl Role {
    /// Create a new role
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    /// Create a role with description
    pub fn with_description(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
        }
    }
}

/// Group roles (kind 39003)
#[derive(Debug, Clone, PartialEq)]
pub struct GroupRoles {
    /// Group ID (d tag)
    pub id: String,
    /// List of supported roles
    pub roles: Vec<Role>,
}

impl GroupRoles {
    /// Create new group roles
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            roles: Vec::new(),
        }
    }

    /// Add a role
    pub fn with_role(mut self, name: impl Into<String>) -> Self {
        self.roles.push(Role::new(name));
        self
    }

    /// Add a role with description
    pub fn with_role_description(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.roles.push(Role::with_description(name, description));
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec![GROUP_D_TAG.to_string(), self.id.clone()]];

        for role in &self.roles {
            let mut tag = vec!["role".to_string(), role.name.clone()];
            if let Some(desc) = &role.description {
                tag.push(desc.clone());
            }
            tags.push(tag);
        }

        tags
    }

    /// Validate the group roles
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(&self.id)
    }
}

/// Join request (kind 9021)
#[derive(Debug, Clone, PartialEq)]
pub struct JoinRequest {
    /// Group ID
    pub group_id: String,
    /// Optional reason for joining
    pub reason: Option<String>,
    /// Optional invite code
    pub invite_code: Option<String>,
}

impl JoinRequest {
    /// Create a new join request
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            reason: None,
            invite_code: None,
        }
    }

    /// Set the join reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the invite code
    pub fn with_invite_code(mut self, code: impl Into<String>) -> Self {
        self.invite_code = Some(code.into());
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec![GROUP_TAG.to_string(), self.group_id.clone()]];

        if let Some(code) = &self.invite_code {
            tags.push(vec!["code".to_string(), code.clone()]);
        }

        tags
    }

    /// Get the content field
    pub fn content(&self) -> String {
        self.reason.clone().unwrap_or_default()
    }

    /// Validate the join request
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(&self.group_id)
    }
}

/// Leave request (kind 9022)
#[derive(Debug, Clone, PartialEq)]
pub struct LeaveRequest {
    /// Group ID
    pub group_id: String,
    /// Optional reason for leaving
    pub reason: Option<String>,
}

impl LeaveRequest {
    /// Create a new leave request
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            reason: None,
        }
    }

    /// Set the leave reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        vec![vec![GROUP_TAG.to_string(), self.group_id.clone()]]
    }

    /// Get the content field
    pub fn content(&self) -> String {
        self.reason.clone().unwrap_or_default()
    }

    /// Validate the leave request
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(&self.group_id)
    }
}

/// Moderation action
#[derive(Debug, Clone, PartialEq)]
pub enum ModerationAction {
    /// Put user (kind 9000)
    PutUser {
        group_id: String,
        pubkey: String,
        roles: Vec<String>,
        reason: Option<String>,
    },
    /// Remove user (kind 9001)
    RemoveUser {
        group_id: String,
        pubkey: String,
        reason: Option<String>,
    },
    /// Edit metadata (kind 9002)
    EditMetadata {
        group_id: String,
        metadata: GroupMetadata,
        reason: Option<String>,
    },
    /// Delete event (kind 9005)
    DeleteEvent {
        group_id: String,
        event_id: String,
        reason: Option<String>,
    },
    /// Create group (kind 9007)
    CreateGroup {
        group_id: String,
        reason: Option<String>,
    },
    /// Delete group (kind 9008)
    DeleteGroup {
        group_id: String,
        reason: Option<String>,
    },
    /// Create invite (kind 9009)
    CreateInvite {
        group_id: String,
        reason: Option<String>,
    },
}

impl ModerationAction {
    /// Create a put-user action
    pub fn put_user(group_id: impl Into<String>, pubkey: impl Into<String>) -> Self {
        Self::PutUser {
            group_id: group_id.into(),
            pubkey: pubkey.into(),
            roles: Vec::new(),
            reason: None,
        }
    }

    /// Add roles to a put-user action
    pub fn with_roles(mut self, roles: Vec<impl Into<String>>) -> Self {
        if let Self::PutUser { roles: r, .. } = &mut self {
            *r = roles.into_iter().map(|s| s.into()).collect();
        }
        self
    }

    /// Create a remove-user action
    pub fn remove_user(group_id: impl Into<String>, pubkey: impl Into<String>) -> Self {
        Self::RemoveUser {
            group_id: group_id.into(),
            pubkey: pubkey.into(),
            reason: None,
        }
    }

    /// Create an edit-metadata action
    pub fn edit_metadata(group_id: impl Into<String>, metadata: GroupMetadata) -> Self {
        Self::EditMetadata {
            group_id: group_id.into(),
            metadata,
            reason: None,
        }
    }

    /// Create a delete-event action
    pub fn delete_event(group_id: impl Into<String>, event_id: impl Into<String>) -> Self {
        Self::DeleteEvent {
            group_id: group_id.into(),
            event_id: event_id.into(),
            reason: None,
        }
    }

    /// Create a create-group action
    pub fn create_group(group_id: impl Into<String>) -> Self {
        Self::CreateGroup {
            group_id: group_id.into(),
            reason: None,
        }
    }

    /// Create a delete-group action
    pub fn delete_group(group_id: impl Into<String>) -> Self {
        Self::DeleteGroup {
            group_id: group_id.into(),
            reason: None,
        }
    }

    /// Create a create-invite action
    pub fn create_invite(group_id: impl Into<String>) -> Self {
        Self::CreateInvite {
            group_id: group_id.into(),
            reason: None,
        }
    }

    /// Set the reason for the action
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        let r = Some(reason.into());
        match &mut self {
            Self::PutUser { reason: re, .. }
            | Self::RemoveUser { reason: re, .. }
            | Self::EditMetadata { reason: re, .. }
            | Self::DeleteEvent { reason: re, .. }
            | Self::CreateGroup { reason: re, .. }
            | Self::DeleteGroup { reason: re, .. }
            | Self::CreateInvite { reason: re, .. } => *re = r,
        }
        self
    }

    /// Get the event kind for this action
    pub fn kind(&self) -> u64 {
        match self {
            Self::PutUser { .. } => KIND_MOD_PUT_USER,
            Self::RemoveUser { .. } => KIND_MOD_REMOVE_USER,
            Self::EditMetadata { .. } => KIND_MOD_EDIT_METADATA,
            Self::DeleteEvent { .. } => KIND_MOD_DELETE_EVENT,
            Self::CreateGroup { .. } => KIND_MOD_CREATE_GROUP,
            Self::DeleteGroup { .. } => KIND_MOD_DELETE_GROUP,
            Self::CreateInvite { .. } => KIND_MOD_CREATE_INVITE,
        }
    }

    /// Get the group ID
    pub fn group_id(&self) -> &str {
        match self {
            Self::PutUser { group_id, .. }
            | Self::RemoveUser { group_id, .. }
            | Self::EditMetadata { group_id, .. }
            | Self::DeleteEvent { group_id, .. }
            | Self::CreateGroup { group_id, .. }
            | Self::DeleteGroup { group_id, .. }
            | Self::CreateInvite { group_id, .. } => group_id,
        }
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = vec![vec![GROUP_TAG.to_string(), self.group_id().to_string()]];

        match self {
            Self::PutUser { pubkey, roles, .. } => {
                let mut tag = vec!["p".to_string(), pubkey.clone()];
                tag.extend(roles.iter().cloned());
                tags.push(tag);
            }
            Self::RemoveUser { pubkey, .. } => {
                tags.push(vec!["p".to_string(), pubkey.clone()]);
            }
            Self::EditMetadata { metadata, .. } => {
                // Add metadata fields as tags
                if let Some(name) = &metadata.name {
                    tags.push(vec!["name".to_string(), name.clone()]);
                }
                if let Some(picture) = &metadata.picture {
                    tags.push(vec!["picture".to_string(), picture.clone()]);
                }
                if let Some(about) = &metadata.about {
                    tags.push(vec!["about".to_string(), about.clone()]);
                }
            }
            Self::DeleteEvent { event_id, .. } => {
                tags.push(vec!["e".to_string(), event_id.clone()]);
            }
            Self::CreateGroup { .. } | Self::DeleteGroup { .. } | Self::CreateInvite { .. } => {
                // No additional tags
            }
        }

        tags
    }

    /// Get the content field
    pub fn content(&self) -> String {
        match self {
            Self::PutUser { reason, .. }
            | Self::RemoveUser { reason, .. }
            | Self::EditMetadata { reason, .. }
            | Self::DeleteEvent { reason, .. }
            | Self::CreateGroup { reason, .. }
            | Self::DeleteGroup { reason, .. }
            | Self::CreateInvite { reason, .. } => reason.clone().unwrap_or_default(),
        }
    }

    /// Validate the moderation action
    pub fn validate(&self) -> Result<(), Nip29Error> {
        validate_group_id(self.group_id())?;

        match self {
            Self::PutUser { pubkey, .. } | Self::RemoveUser { pubkey, .. } => {
                if pubkey.trim().is_empty() {
                    return Err(Nip29Error::EmptyPubkey);
                }
            }
            Self::EditMetadata { metadata, .. } => {
                metadata.validate()?;
            }
            Self::DeleteEvent { event_id, .. } => {
                if event_id.trim().is_empty() {
                    return Err(Nip29Error::EmptyEventId);
                }
            }
            Self::CreateGroup { .. } | Self::DeleteGroup { .. } | Self::CreateInvite { .. } => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_group_kind() {
        assert!(is_group_kind(KIND_JOIN_REQUEST));
        assert!(is_group_kind(KIND_LEAVE_REQUEST));
        assert!(is_group_kind(KIND_MOD_PUT_USER));
        assert!(is_group_kind(KIND_GROUP_METADATA));
        assert!(!is_group_kind(1));
    }

    #[test]
    fn test_is_group_moderation_kind() {
        assert!(is_group_moderation_kind(9000));
        assert!(is_group_moderation_kind(9009));
        assert!(!is_group_moderation_kind(9021));
        assert!(!is_group_moderation_kind(39000));
    }

    #[test]
    fn test_is_group_metadata_kind() {
        assert!(is_group_metadata_kind(39000));
        assert!(is_group_metadata_kind(39003));
        assert!(!is_group_metadata_kind(9000));
    }

    #[test]
    fn test_validate_group_id() {
        assert!(validate_group_id("pizza-lovers").is_ok());
        assert!(validate_group_id("group_123").is_ok());
        assert!(validate_group_id("abc-def-123").is_ok());
        assert!(validate_group_id("").is_err());
        assert!(validate_group_id("Pizza Lovers").is_err());
        assert!(validate_group_id("group@123").is_err());
    }

    #[test]
    fn test_parse_group_identifier() {
        let (host, id) = parse_group_identifier("groups.nostr.com'abcdef").unwrap();
        assert_eq!(host, "groups.nostr.com");
        assert_eq!(id, "abcdef");

        assert!(parse_group_identifier("invalid").is_err());
    }

    #[test]
    fn test_format_group_identifier() {
        let identifier = format_group_identifier("groups.nostr.com", "abcdef");
        assert_eq!(identifier, "groups.nostr.com'abcdef");
    }

    #[test]
    fn test_group_metadata() {
        let metadata = GroupMetadata::new("pizza-lovers")
            .with_name("Pizza Lovers")
            .with_about("A group for pizza enthusiasts")
            .with_picture("https://pizza.com/logo.png")
            .with_private(true)
            .with_closed(false);

        assert_eq!(metadata.id, "pizza-lovers");
        assert_eq!(metadata.name, Some("Pizza Lovers".to_string()));
        assert_eq!(metadata.private, Some(true));
        assert_eq!(metadata.closed, Some(false));

        let tags = metadata.to_tags();
        assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "pizza-lovers"));
        assert!(tags.iter().any(|t| t[0] == "name"));
        assert!(tags.iter().any(|t| t[0] == "private"));
    }

    #[test]
    fn test_group_admins() {
        let admins = GroupAdmins::new("pizza-lovers")
            .with_admin("pubkey1", vec!["admin", "moderator"])
            .with_admin("pubkey2", vec!["moderator"]);

        assert_eq!(admins.id, "pizza-lovers");
        assert_eq!(admins.admins.len(), 2);
        assert_eq!(admins.admins[0].roles.len(), 2);

        let tags = admins.to_tags();
        assert_eq!(tags[0], vec!["d", "pizza-lovers"]);
        assert_eq!(tags[1][0], "p");
        assert_eq!(tags[1][1], "pubkey1");
    }

    #[test]
    fn test_group_members() {
        let members = GroupMembers::new("pizza-lovers")
            .with_member("pubkey1")
            .with_member("pubkey2")
            .with_member("pubkey3");

        assert_eq!(members.id, "pizza-lovers");
        assert_eq!(members.members.len(), 3);

        let tags = members.to_tags();
        assert_eq!(tags.len(), 4); // d tag + 3 p tags
    }

    #[test]
    fn test_group_roles() {
        let roles = GroupRoles::new("pizza-lovers")
            .with_role("admin")
            .with_role_description("moderator", "Can delete messages");

        assert_eq!(roles.id, "pizza-lovers");
        assert_eq!(roles.roles.len(), 2);
        assert_eq!(roles.roles[0].name, "admin");
        assert_eq!(
            roles.roles[1].description,
            Some("Can delete messages".to_string())
        );

        let tags = roles.to_tags();
        assert_eq!(tags[1][0], "role");
        assert_eq!(tags[1][1], "admin");
    }

    #[test]
    fn test_join_request() {
        let request = JoinRequest::new("pizza-lovers")
            .with_reason("I love pizza!")
            .with_invite_code("secret123");

        assert_eq!(request.group_id, "pizza-lovers");
        assert_eq!(request.content(), "I love pizza!");

        let tags = request.to_tags();
        assert_eq!(tags[0], vec!["h", "pizza-lovers"]);
        assert!(tags.iter().any(|t| t[0] == "code" && t[1] == "secret123"));
    }

    #[test]
    fn test_leave_request() {
        let request = LeaveRequest::new("pizza-lovers").with_reason("Too much pizza");

        assert_eq!(request.group_id, "pizza-lovers");
        assert_eq!(request.content(), "Too much pizza");

        let tags = request.to_tags();
        assert_eq!(tags[0], vec!["h", "pizza-lovers"]);
    }

    #[test]
    fn test_moderation_put_user() {
        let action = ModerationAction::put_user("pizza-lovers", "pubkey123")
            .with_roles(vec!["admin", "moderator"])
            .with_reason("Trusted member");

        assert_eq!(action.kind(), KIND_MOD_PUT_USER);
        assert_eq!(action.group_id(), "pizza-lovers");
        assert_eq!(action.content(), "Trusted member");

        let tags = action.to_tags();
        assert_eq!(tags[0], vec!["h", "pizza-lovers"]);
        assert_eq!(tags[1][0], "p");
        assert_eq!(tags[1][1], "pubkey123");
    }

    #[test]
    fn test_moderation_remove_user() {
        let action = ModerationAction::remove_user("pizza-lovers", "pubkey123").with_reason("Spam");

        assert_eq!(action.kind(), KIND_MOD_REMOVE_USER);
        assert_eq!(action.content(), "Spam");
    }

    #[test]
    fn test_moderation_delete_event() {
        let action =
            ModerationAction::delete_event("pizza-lovers", "event123").with_reason("Offensive");

        assert_eq!(action.kind(), KIND_MOD_DELETE_EVENT);

        let tags = action.to_tags();
        assert!(tags.iter().any(|t| t[0] == "e" && t[1] == "event123"));
    }

    #[test]
    fn test_moderation_create_group() {
        let action = ModerationAction::create_group("new-group");
        assert_eq!(action.kind(), KIND_MOD_CREATE_GROUP);
        assert_eq!(action.group_id(), "new-group");
    }

    #[test]
    fn test_moderation_edit_metadata() {
        let metadata = GroupMetadata::new("pizza-lovers").with_name("New Name");
        let action = ModerationAction::edit_metadata("pizza-lovers", metadata);

        assert_eq!(action.kind(), KIND_MOD_EDIT_METADATA);

        let tags = action.to_tags();
        assert!(tags.iter().any(|t| t[0] == "name" && t[1] == "New Name"));
    }

    #[test]
    fn test_moderation_validate() {
        let action = ModerationAction::put_user("pizza-lovers", "pubkey123");
        assert!(action.validate().is_ok());

        let invalid = ModerationAction::put_user("INVALID ID", "pubkey123");
        assert!(invalid.validate().is_err());

        let empty_pubkey = ModerationAction::put_user("pizza-lovers", "");
        assert!(empty_pubkey.validate().is_err());
    }
}
