use std::collections::{BTreeMap, BTreeSet};

use nostr::{
    Event, EventTemplate, GroupAdminsEvent, GroupMembersEvent, GroupMetadata, GroupMetadataEvent,
    GroupRole, GroupRolesEvent, JoinRequestEvent, KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE,
    KIND_CHANNEL_METADATA, KIND_CREATE_GROUP, KIND_CREATE_INVITE, KIND_DELETE_EVENT,
    KIND_DELETE_GROUP, KIND_EDIT_METADATA, KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS,
    KIND_GROUP_METADATA, KIND_GROUP_ROLES, KIND_JOIN_REQUEST, KIND_LEAVE_REQUEST, KIND_PUT_USER,
    KIND_REMOVE_USER, LeaveRequestEvent, ManagedChannelCreateEvent, ManagedChannelMessageEvent,
    ManagedChannelMetadataEvent, ModerationAction, ModerationEvent, TaggedPubkey, finalize_event,
};
use serde_json::Value;

use crate::RelayIdentity;

#[derive(Debug, Clone, Default)]
pub(crate) struct ManagedGroupsState {
    groups: BTreeMap<String, ManagedGroup>,
}

#[derive(Debug, Clone)]
pub(crate) struct ManagedGroupOutcome {
    pub accepted_events: Vec<Event>,
    pub removed_event_ids: Vec<String>,
    pub pruned_group_id: Option<String>,
}

impl ManagedGroupOutcome {
    fn accepted(mut accepted_events: Vec<Event>) -> Self {
        accepted_events.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Self {
            accepted_events,
            removed_event_ids: Vec::new(),
            pruned_group_id: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ManagedGroup {
    metadata: GroupMetadata,
    members: BTreeMap<String, GroupMemberRecord>,
    invites: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct GroupMemberRecord {
    roles: BTreeSet<String>,
    is_admin: bool,
}

impl GroupMemberRecord {
    fn labels(&self) -> Vec<String> {
        let mut labels = self.roles.iter().cloned().collect::<Vec<_>>();
        if self.is_admin && !labels.iter().any(|label| label == "admin") {
            labels.push("admin".to_string());
        }
        labels.sort();
        labels.dedup();
        labels
    }
}

impl ManagedGroup {
    fn new(owner_pubkey: &str) -> Self {
        let mut members = BTreeMap::new();
        members.insert(
            owner_pubkey.to_string(),
            GroupMemberRecord {
                roles: BTreeSet::from(["admin".to_string()]),
                is_admin: true,
            },
        );

        Self {
            metadata: GroupMetadata::default(),
            members,
            invites: BTreeSet::new(),
        }
    }

    fn is_member(&self, pubkey: &str) -> bool {
        self.members.contains_key(pubkey)
    }

    fn is_admin(&self, pubkey: &str) -> bool {
        self.members
            .get(pubkey)
            .is_some_and(|member| member.is_admin)
    }

    fn add_member(&mut self, pubkey: String, roles: impl IntoIterator<Item = String>) {
        let role_set = roles
            .into_iter()
            .filter(|role| !role.trim().is_empty())
            .collect::<BTreeSet<_>>();
        let is_admin = role_set
            .iter()
            .any(|role| matches!(role.as_str(), "admin" | "owner"));

        self.members.insert(
            pubkey,
            GroupMemberRecord {
                roles: role_set,
                is_admin,
            },
        );
    }

    fn remove_member(&mut self, pubkey: &str) {
        self.members.remove(pubkey);
    }

    fn should_auto_join(&self, invite_code: Option<&str>) -> bool {
        invite_code.is_some_and(|code| self.invites.contains(code))
            || !(self.metadata.restricted
                || self.metadata.private
                || self.metadata.hidden
                || self.metadata.closed)
    }

    fn require_member(&self, pubkey: &str) -> Result<(), String> {
        if self.is_member(pubkey) {
            return Ok(());
        }
        Err("managed_group:membership_required".to_string())
    }

    fn require_admin(&self, pubkey: &str) -> Result<(), String> {
        if self.is_admin(pubkey) {
            return Ok(());
        }
        Err("managed_group:admin_required".to_string())
    }

    fn apply_metadata_changes(&mut self, changes: &[Vec<String>]) {
        for change in changes {
            let Some(tag_name) = change.first().map(String::as_str) else {
                continue;
            };
            match tag_name {
                "name" => {
                    self.metadata.name = change.get(1).cloned().filter(|value| !value.is_empty());
                }
                "picture" => {
                    self.metadata.picture =
                        change.get(1).cloned().filter(|value| !value.is_empty());
                }
                "about" => {
                    self.metadata.about = change.get(1).cloned().filter(|value| !value.is_empty());
                }
                "private" => self.metadata.private = true,
                "restricted" => self.metadata.restricted = true,
                "hidden" => self.metadata.hidden = true,
                "closed" => self.metadata.closed = true,
                _ => {}
            }
        }
    }

    fn snapshot_events(
        &self,
        relay_identity: &RelayIdentity,
        group_id: &str,
        created_at: u64,
    ) -> Result<Vec<Event>, String> {
        let metadata = GroupMetadataEvent::new(group_id, self.metadata.clone(), created_at)
            .map_err(|error| format!("managed_group:{error}"))?;

        let admins = GroupAdminsEvent::new(
            group_id,
            self.members
                .iter()
                .filter(|(_, member)| member.is_admin)
                .map(|(pubkey, member)| {
                    TaggedPubkey::new(pubkey.clone())
                        .map(|tagged| tagged.with_labels(member.labels()))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("managed_group:{error}"))?,
            created_at,
        )
        .map_err(|error| format!("managed_group:{error}"))?;

        let members = GroupMembersEvent::new(
            group_id,
            self.members
                .iter()
                .map(|(pubkey, member)| {
                    TaggedPubkey::new(pubkey.clone())
                        .map(|tagged| tagged.with_labels(member.labels()))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("managed_group:{error}"))?,
            created_at,
        )
        .map_err(|error| format!("managed_group:{error}"))?;

        let mut role_names = BTreeSet::new();
        for member in self.members.values() {
            for role in &member.roles {
                role_names.insert(role.clone());
            }
            if member.is_admin {
                role_names.insert("admin".to_string());
            }
        }
        let roles = GroupRolesEvent::new(
            group_id,
            role_names
                .into_iter()
                .map(GroupRole::new)
                .collect::<Vec<_>>(),
            created_at,
        )
        .map_err(|error| format!("managed_group:{error}"))?;

        Ok(vec![
            sign_unsigned_event(
                relay_identity,
                metadata.to_unsigned_event(relay_identity.public_key_hex.clone()),
            )?,
            sign_unsigned_event(
                relay_identity,
                admins.to_unsigned_event(relay_identity.public_key_hex.clone()),
            )?,
            sign_unsigned_event(
                relay_identity,
                members.to_unsigned_event(relay_identity.public_key_hex.clone()),
            )?,
            sign_unsigned_event(
                relay_identity,
                roles.to_unsigned_event(relay_identity.public_key_hex.clone()),
            )?,
        ])
    }
}

impl ManagedGroupsState {
    pub(crate) fn can_read_event(&self, event: &Event, authenticated_pubkey: Option<&str>) -> bool {
        let Some(group_id) = event_group_id(event) else {
            return true;
        };
        let Some(group) = self.groups.get(group_id) else {
            return true;
        };
        if !group_requires_auth(group) {
            return true;
        }
        authenticated_pubkey.is_some_and(|pubkey| group.is_member(pubkey))
    }

    pub(crate) fn subscription_auth_message(
        &self,
        filters: &[Value],
        authenticated_pubkey: Option<&str>,
    ) -> Option<String> {
        let group_ids = filters
            .iter()
            .flat_map(filter_group_ids)
            .collect::<BTreeSet<_>>();

        for group_id in group_ids {
            let Some(group) = self.groups.get(group_id.as_str()) else {
                continue;
            };
            if !group_requires_auth(group) {
                continue;
            }
            let Some(pubkey) = authenticated_pubkey else {
                return Some(nostr::nip42::create_auth_required_message(
                    "managed group subscription requires relay authentication",
                ));
            };
            if !group.is_member(pubkey) {
                return Some(nostr::nip42::create_restricted_message(
                    "managed group subscription is not permitted",
                ));
            }
        }

        None
    }

    pub(crate) fn publish_auth_message(
        &self,
        event: &Event,
        authenticated_pubkey: Option<&str>,
    ) -> Option<String> {
        let group_id = event_group_id(event)?;
        let group = self.groups.get(group_id)?;
        if !group_requires_auth(group) {
            return None;
        }
        let Some(pubkey) = authenticated_pubkey else {
            return Some(nostr::nip42::create_auth_required_message(
                "managed group write requires relay authentication",
            ));
        };
        if pubkey != event.pubkey {
            return Some(nostr::nip42::create_restricted_message(
                "authenticated pubkey does not match event pubkey",
            ));
        }
        match nostr::verify_event(event) {
            Ok(true) => None,
            Ok(false) | Err(_) => Some(nostr::nip42::create_restricted_message(
                "managed group event signature is invalid",
            )),
        }
    }

    pub(crate) fn apply_event(
        &mut self,
        relay_identity: &RelayIdentity,
        event: &Event,
    ) -> Result<Option<ManagedGroupOutcome>, String> {
        match event.kind {
            KIND_GROUP_METADATA | KIND_GROUP_ADMINS | KIND_GROUP_MEMBERS | KIND_GROUP_ROLES => {
                Err("managed_group:relay_owned_snapshot_kind".to_string())
            }
            KIND_CREATE_GROUP | KIND_PUT_USER | KIND_REMOVE_USER | KIND_EDIT_METADATA
            | KIND_DELETE_EVENT | KIND_DELETE_GROUP | KIND_CREATE_INVITE => {
                let moderation = ModerationEvent::from_event(event)
                    .map_err(|error| format!("managed_group:{error}"))?;
                Ok(Some(self.apply_moderation(
                    relay_identity,
                    event,
                    moderation,
                )?))
            }
            KIND_JOIN_REQUEST => {
                let join = JoinRequestEvent::from_event(event)
                    .map_err(|error| format!("managed_group:{error}"))?;
                Ok(Some(self.apply_join(relay_identity, event, join)?))
            }
            KIND_LEAVE_REQUEST => {
                let leave = LeaveRequestEvent::from_event(event)
                    .map_err(|error| format!("managed_group:{error}"))?;
                Ok(Some(self.apply_leave(relay_identity, event, leave)?))
            }
            KIND_CHANNEL_CREATION => {
                let channel = ManagedChannelCreateEvent::from_event(event)
                    .map_err(|error| format!("managed_group:{error}"))?;
                let group = self
                    .groups
                    .get(&channel.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                Ok(Some(ManagedGroupOutcome::accepted(vec![event.clone()])))
            }
            KIND_CHANNEL_METADATA => {
                let channel = ManagedChannelMetadataEvent::from_event(event)
                    .map_err(|error| format!("managed_group:{error}"))?;
                let group = self
                    .groups
                    .get(&channel.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                Ok(Some(ManagedGroupOutcome::accepted(vec![event.clone()])))
            }
            KIND_CHANNEL_MESSAGE => {
                let message = ManagedChannelMessageEvent::from_event(event)
                    .map_err(|error| format!("managed_group:{error}"))?;
                let group = self
                    .groups
                    .get(&message.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_member(event.pubkey.as_str())?;
                Ok(Some(ManagedGroupOutcome::accepted(vec![event.clone()])))
            }
            _ => Ok(None),
        }
    }

    fn apply_moderation(
        &mut self,
        relay_identity: &RelayIdentity,
        event: &Event,
        moderation: ModerationEvent,
    ) -> Result<ManagedGroupOutcome, String> {
        match moderation.action {
            ModerationAction::CreateGroup => {
                if self.groups.contains_key(&moderation.group_id) {
                    return Err("managed_group:group_exists".to_string());
                }
                self.groups.insert(
                    moderation.group_id.clone(),
                    ManagedGroup::new(event.pubkey.as_str()),
                );

                let snapshots = self
                    .groups
                    .get(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?
                    .snapshot_events(
                        relay_identity,
                        moderation.group_id.as_str(),
                        event.created_at,
                    )?;
                let mut accepted_events = vec![event.clone()];
                accepted_events.extend(snapshots);
                Ok(ManagedGroupOutcome::accepted(accepted_events))
            }
            ModerationAction::PutUser { pubkey, roles } => {
                let group = self
                    .groups
                    .get_mut(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                group.add_member(pubkey, roles);
                let snapshots = group.snapshot_events(
                    relay_identity,
                    moderation.group_id.as_str(),
                    event.created_at,
                )?;
                let mut accepted_events = vec![event.clone()];
                accepted_events.extend(snapshots);
                Ok(ManagedGroupOutcome::accepted(accepted_events))
            }
            ModerationAction::RemoveUser { pubkey } => {
                let group = self
                    .groups
                    .get_mut(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                group.remove_member(pubkey.as_str());
                let snapshots = group.snapshot_events(
                    relay_identity,
                    moderation.group_id.as_str(),
                    event.created_at,
                )?;
                let mut accepted_events = vec![event.clone()];
                accepted_events.extend(snapshots);
                Ok(ManagedGroupOutcome::accepted(accepted_events))
            }
            ModerationAction::EditMetadata { changes } => {
                let group = self
                    .groups
                    .get_mut(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                group.apply_metadata_changes(changes.as_slice());
                let snapshots = group.snapshot_events(
                    relay_identity,
                    moderation.group_id.as_str(),
                    event.created_at,
                )?;
                let mut accepted_events = vec![event.clone()];
                accepted_events.extend(snapshots);
                Ok(ManagedGroupOutcome::accepted(accepted_events))
            }
            ModerationAction::DeleteEvent { event_id } => {
                let group = self
                    .groups
                    .get(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                Ok(ManagedGroupOutcome {
                    accepted_events: vec![event.clone()],
                    removed_event_ids: vec![event_id],
                    pruned_group_id: None,
                })
            }
            ModerationAction::DeleteGroup => {
                let group = self
                    .groups
                    .get(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                self.groups.remove(&moderation.group_id);
                Ok(ManagedGroupOutcome {
                    accepted_events: vec![event.clone()],
                    removed_event_ids: Vec::new(),
                    pruned_group_id: Some(moderation.group_id),
                })
            }
            ModerationAction::CreateInvite { code } => {
                let group = self
                    .groups
                    .get_mut(&moderation.group_id)
                    .ok_or_else(|| "managed_group:group_not_found".to_string())?;
                group.require_admin(event.pubkey.as_str())?;
                group.invites.insert(code);
                Ok(ManagedGroupOutcome::accepted(vec![event.clone()]))
            }
        }
    }

    fn apply_join(
        &mut self,
        relay_identity: &RelayIdentity,
        event: &Event,
        join: JoinRequestEvent,
    ) -> Result<ManagedGroupOutcome, String> {
        let group = self
            .groups
            .get_mut(&join.group_id)
            .ok_or_else(|| "managed_group:group_not_found".to_string())?;

        if join.invite_code.is_none() && group.metadata.closed {
            return Err("managed_group:invite_required".to_string());
        }

        let mut accepted_events = vec![event.clone()];
        if group.should_auto_join(join.invite_code.as_deref()) {
            group.add_member(event.pubkey.clone(), Vec::<String>::new());
            accepted_events.extend(group.snapshot_events(
                relay_identity,
                join.group_id.as_str(),
                event.created_at,
            )?);
        }

        Ok(ManagedGroupOutcome::accepted(accepted_events))
    }

    fn apply_leave(
        &mut self,
        relay_identity: &RelayIdentity,
        event: &Event,
        leave: LeaveRequestEvent,
    ) -> Result<ManagedGroupOutcome, String> {
        let group = self
            .groups
            .get_mut(&leave.group_id)
            .ok_or_else(|| "managed_group:group_not_found".to_string())?;
        group.require_member(event.pubkey.as_str())?;
        group.remove_member(event.pubkey.as_str());

        let mut accepted_events = vec![event.clone()];
        accepted_events.extend(group.snapshot_events(
            relay_identity,
            leave.group_id.as_str(),
            event.created_at,
        )?);
        Ok(ManagedGroupOutcome::accepted(accepted_events))
    }
}

pub(crate) fn event_group_id(event: &Event) -> Option<&str> {
    if let Some(group_id) = event
        .tags
        .iter()
        .find(|tag| tag.first().is_some_and(|name| name == "h"))
        .and_then(|tag| tag.get(1))
    {
        return Some(group_id.as_str());
    }

    if matches!(
        event.kind,
        KIND_GROUP_METADATA | KIND_GROUP_ADMINS | KIND_GROUP_MEMBERS | KIND_GROUP_ROLES
    ) {
        return event
            .tags
            .iter()
            .find(|tag| tag.first().is_some_and(|name| name == "d"))
            .and_then(|tag| tag.get(1))
            .map(String::as_str);
    }

    None
}

fn filter_group_ids(filter: &Value) -> Vec<String> {
    let Some(object) = filter.as_object() else {
        return Vec::new();
    };

    let mut group_ids = BTreeSet::new();
    if let Some(tag_values) = object.get("#h").and_then(Value::as_array) {
        for group_id in tag_values.iter().filter_map(Value::as_str) {
            group_ids.insert(group_id.to_string());
        }
    }
    if let Some(tag_values) = object.get("#d").and_then(Value::as_array) {
        for group_id in tag_values.iter().filter_map(Value::as_str) {
            group_ids.insert(group_id.to_string());
        }
    }
    group_ids.into_iter().collect()
}

fn group_requires_auth(group: &ManagedGroup) -> bool {
    group.metadata.private
        || group.metadata.restricted
        || group.metadata.hidden
        || group.metadata.closed
}

fn sign_unsigned_event(
    relay_identity: &RelayIdentity,
    unsigned: nostr::UnsignedEvent,
) -> Result<Event, String> {
    let template = EventTemplate {
        created_at: unsigned.created_at,
        kind: unsigned.kind,
        tags: unsigned.tags,
        content: unsigned.content,
    };
    finalize_event(&template, &relay_identity.secret_key)
        .map_err(|error| format!("managed_group:failed_to_sign_snapshot:{error}"))
}

#[cfg(test)]
mod tests {
    use nostr::{
        EventTemplate, KIND_CREATE_GROUP, KIND_GROUP_METADATA, KIND_JOIN_REQUEST,
        KIND_LEAVE_REQUEST, ModerationAction, ModerationEvent, finalize_event, get_public_key_hex,
    };
    use serde_json::json;

    use super::{ManagedGroupsState, RelayIdentity, event_group_id};

    fn secret_key(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn relay_identity() -> RelayIdentity {
        let secret_key = secret_key(11);
        RelayIdentity {
            public_key_hex: get_public_key_hex(&secret_key).unwrap(),
            secret_key,
        }
    }

    fn sign_template(template: EventTemplate, secret_key: &[u8; 32]) -> nostr::Event {
        finalize_event(&template, secret_key).unwrap()
    }

    fn sign_moderation(
        event: ModerationEvent,
        secret_key: &[u8; 32],
        pubkey: &str,
    ) -> nostr::Event {
        let unsigned = event.to_unsigned_event(pubkey.to_string()).unwrap();
        sign_template(
            EventTemplate {
                created_at: unsigned.created_at,
                kind: unsigned.kind,
                tags: unsigned.tags,
                content: unsigned.content,
            },
            secret_key,
        )
    }

    #[test]
    fn managed_group_state_emits_snapshots_and_enforces_membership() {
        let relay = relay_identity();
        let admin_secret = secret_key(1);
        let admin_pubkey = get_public_key_hex(&admin_secret).unwrap();
        let member_secret = secret_key(2);
        let member_pubkey = get_public_key_hex(&member_secret).unwrap();
        let mut groups = ManagedGroupsState::default();

        let create = sign_moderation(
            ModerationEvent::new("oa-main", ModerationAction::CreateGroup, 10).unwrap(),
            &admin_secret,
            admin_pubkey.as_str(),
        );
        let create_outcome = groups.apply_event(&relay, &create).unwrap().unwrap();
        assert!(
            create_outcome
                .accepted_events
                .iter()
                .any(|event| event.kind == KIND_CREATE_GROUP)
        );
        assert!(
            create_outcome
                .accepted_events
                .iter()
                .any(|event| event.kind == KIND_GROUP_METADATA)
        );

        let outsider_message = sign_template(
            EventTemplate {
                created_at: 11,
                kind: nostr::KIND_CHANNEL_MESSAGE,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec![
                        "e".to_string(),
                        "a".repeat(64),
                        "wss://relay.example".to_string(),
                        "root".to_string(),
                    ],
                ],
                content: "hello".to_string(),
            },
            &member_secret,
        );
        let outsider_error = groups.apply_event(&relay, &outsider_message).unwrap_err();
        assert_eq!(outsider_error, "managed_group:membership_required");

        let add_member = sign_moderation(
            ModerationEvent::new(
                "oa-main",
                ModerationAction::PutUser {
                    pubkey: member_pubkey.clone(),
                    roles: vec!["support".to_string()],
                },
                12,
            )
            .unwrap(),
            &admin_secret,
            admin_pubkey.as_str(),
        );
        let add_outcome = groups.apply_event(&relay, &add_member).unwrap().unwrap();
        assert!(
            add_outcome
                .accepted_events
                .iter()
                .any(|event| event.kind == nostr::KIND_GROUP_MEMBERS)
        );

        let member_message = sign_template(
            EventTemplate {
                created_at: 13,
                kind: nostr::KIND_CHANNEL_MESSAGE,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec![
                        "e".to_string(),
                        "a".repeat(64),
                        "wss://relay.example".to_string(),
                        "root".to_string(),
                    ],
                    vec!["p".to_string(), admin_pubkey.clone()],
                ],
                content: "thanks".to_string(),
            },
            &member_secret,
        );
        let message_outcome = groups
            .apply_event(&relay, &member_message)
            .unwrap()
            .unwrap();
        assert_eq!(message_outcome.accepted_events, vec![member_message]);
    }

    #[test]
    fn managed_group_state_supports_join_leave_and_deletes() {
        let relay = relay_identity();
        let admin_secret = secret_key(3);
        let admin_pubkey = get_public_key_hex(&admin_secret).unwrap();
        let joiner_secret = secret_key(4);
        let joiner_pubkey = get_public_key_hex(&joiner_secret).unwrap();
        let mut groups = ManagedGroupsState::default();

        let create = sign_moderation(
            ModerationEvent::new("oa-main", ModerationAction::CreateGroup, 20).unwrap(),
            &admin_secret,
            admin_pubkey.as_str(),
        );
        groups.apply_event(&relay, &create).unwrap();

        let join = sign_template(
            EventTemplate {
                created_at: 21,
                kind: KIND_JOIN_REQUEST,
                tags: vec![vec!["h".to_string(), "oa-main".to_string()]],
                content: "let me in".to_string(),
            },
            &joiner_secret,
        );
        let join_outcome = groups.apply_event(&relay, &join).unwrap().unwrap();
        assert!(
            join_outcome
                .accepted_events
                .iter()
                .any(|event| event.kind == nostr::KIND_GROUP_MEMBERS)
        );

        let message = sign_template(
            EventTemplate {
                created_at: 22,
                kind: nostr::KIND_CHANNEL_MESSAGE,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec![
                        "e".to_string(),
                        "b".repeat(64),
                        "wss://relay.example".to_string(),
                        "root".to_string(),
                    ],
                ],
                content: "hi".to_string(),
            },
            &joiner_secret,
        );
        groups.apply_event(&relay, &message).unwrap();

        let delete = sign_moderation(
            ModerationEvent::new(
                "oa-main",
                ModerationAction::DeleteEvent {
                    event_id: message.id.clone(),
                },
                23,
            )
            .unwrap(),
            &admin_secret,
            admin_pubkey.as_str(),
        );
        let delete_outcome = groups.apply_event(&relay, &delete).unwrap().unwrap();
        assert_eq!(delete_outcome.removed_event_ids, vec![message.id.clone()]);
        assert_eq!(event_group_id(&delete), Some("oa-main"));

        let leave = sign_template(
            EventTemplate {
                created_at: 24,
                kind: KIND_LEAVE_REQUEST,
                tags: vec![vec!["h".to_string(), "oa-main".to_string()]],
                content: String::new(),
            },
            &joiner_secret,
        );
        let leave_outcome = groups.apply_event(&relay, &leave).unwrap().unwrap();
        assert!(
            leave_outcome
                .accepted_events
                .iter()
                .any(|event| event.kind == nostr::KIND_GROUP_MEMBERS)
        );

        let message_after_leave = sign_template(
            EventTemplate {
                created_at: 25,
                kind: nostr::KIND_CHANNEL_MESSAGE,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec![
                        "e".to_string(),
                        "c".repeat(64),
                        "wss://relay.example".to_string(),
                        "root".to_string(),
                    ],
                ],
                content: "still here?".to_string(),
            },
            &joiner_secret,
        );
        assert_eq!(
            groups
                .apply_event(&relay, &message_after_leave)
                .unwrap_err(),
            "managed_group:membership_required"
        );
        assert_eq!(joiner_pubkey.len(), 64);
    }

    #[test]
    fn managed_group_auth_helpers_require_membership_for_restricted_reads_and_writes() {
        let relay = relay_identity();
        let admin_secret = secret_key(5);
        let admin_pubkey = get_public_key_hex(&admin_secret).unwrap();
        let mut groups = ManagedGroupsState::default();

        let create = sign_moderation(
            ModerationEvent::new("oa-main", ModerationAction::CreateGroup, 30).unwrap(),
            &admin_secret,
            admin_pubkey.as_str(),
        );
        groups.apply_event(&relay, &create).unwrap();

        let restrict = sign_moderation(
            ModerationEvent::new(
                "oa-main",
                ModerationAction::EditMetadata {
                    changes: vec![vec!["restricted".to_string()]],
                },
                31,
            )
            .unwrap(),
            &admin_secret,
            admin_pubkey.as_str(),
        );
        groups.apply_event(&relay, &restrict).unwrap();

        let read_message =
            groups.subscription_auth_message(&[json!({"kinds":[39000], "#d":["oa-main"]})], None);
        assert!(
            read_message
                .expect("auth-required message")
                .starts_with(nostr::nip42::AUTH_REQUIRED_PREFIX)
        );

        let write_event = sign_template(
            EventTemplate {
                created_at: 32,
                kind: nostr::KIND_CHANNEL_CREATION,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec!["oa-room-mode".to_string(), "managed-channel".to_string()],
                ],
                content: "{\"name\":\"ops\",\"about\":\"\",\"picture\":\"\"}".to_string(),
            },
            &admin_secret,
        );
        let write_message = groups.publish_auth_message(&write_event, None);
        assert!(
            write_message
                .expect("auth-required write message")
                .starts_with(nostr::nip42::AUTH_REQUIRED_PREFIX)
        );
        assert!(
            groups
                .publish_auth_message(&write_event, Some(admin_pubkey.as_str()))
                .is_none()
        );
    }
}
