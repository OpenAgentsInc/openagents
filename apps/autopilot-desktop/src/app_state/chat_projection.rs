use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use nostr::{
    ChannelMetadata, Event, GroupAdminsEvent, GroupMembersEvent, GroupMetadata, GroupMetadataEvent,
    GroupRole, GroupRolesEvent, ManagedChannelCreateEvent, ManagedChannelHints,
    ManagedChannelIndexEntry, ManagedChannelMessageEvent, ManagedChannelMetadataEvent,
    ManagedRoomMode, ModerationAction, ModerationEvent, compare_managed_timeline_events,
    sort_managed_channel_index,
};
use serde::{Deserialize, Serialize};

use super::PaneLoadState;

const MANAGED_CHAT_PROJECTION_SCHEMA_VERSION: u16 = 1;
const MANAGED_CHAT_PROJECTION_STREAM_ID: &str = "stream.managed_chat_projection.v1";
const MANAGED_CHAT_EVENT_LIMIT: usize = 8_192;
pub const MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID: &str = "oa:uncategorized";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedChatDeliveryState {
    Confirmed,
    #[default]
    Publishing,
    Acked,
    Failed,
}

impl ManagedChatDeliveryState {
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Failed)
    }
}

fn managed_chat_outbound_attempt_count_default() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedChatReadCursor {
    pub last_read_event_id: Option<String>,
    pub last_read_created_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedChatLocalState {
    pub selected_group_id: Option<String>,
    pub selected_channel_id: Option<String>,
    #[serde(default)]
    pub read_cursors: BTreeMap<String, ManagedChatReadCursor>,
    #[serde(default)]
    pub collapsed_category_keys: BTreeSet<String>,
    #[serde(default)]
    pub muted_pubkeys: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedChatOutboundMessage {
    pub event: Event,
    pub group_id: String,
    pub channel_id: String,
    pub relay_url: String,
    #[serde(default)]
    pub delivery_state: ManagedChatDeliveryState,
    #[serde(default = "managed_chat_outbound_attempt_count_default")]
    pub attempt_count: u32,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChatReactionSummary {
    pub content: String,
    pub author_pubkeys: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChatMessageProjection {
    pub event_id: String,
    pub group_id: String,
    pub channel_id: String,
    pub author_pubkey: String,
    pub content: String,
    pub created_at: u64,
    pub reply_to_event_id: Option<String>,
    pub mention_pubkeys: Vec<String>,
    pub reaction_summaries: Vec<ManagedChatReactionSummary>,
    pub reply_child_ids: Vec<String>,
    pub delivery_state: ManagedChatDeliveryState,
    pub delivery_error: Option<String>,
    pub attempt_count: u32,
    pub message_class: crate::chat_message_classifier::ChatMessageClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChatChannelProjection {
    pub channel_id: String,
    pub group_id: String,
    pub room_mode: ManagedRoomMode,
    pub metadata: ChannelMetadata,
    pub hints: ManagedChannelHints,
    pub relay_url: Option<String>,
    pub message_ids: Vec<String>,
    pub root_message_ids: Vec<String>,
    pub unread_count: usize,
    pub mention_count: usize,
    pub latest_message_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChatMemberProjection {
    pub pubkey: String,
    pub labels: Vec<String>,
    pub is_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedChatGroupProjection {
    pub group_id: String,
    pub metadata: GroupMetadata,
    pub roles: Vec<GroupRole>,
    pub members: Vec<ManagedChatMemberProjection>,
    pub channel_ids: Vec<String>,
    pub unread_count: usize,
    pub mention_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedChatProjectionSnapshot {
    pub groups: Vec<ManagedChatGroupProjection>,
    pub channels: Vec<ManagedChatChannelProjection>,
    pub messages: BTreeMap<String, ManagedChatMessageProjection>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManagedChatProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    #[serde(default)]
    relay_events: Vec<Event>,
    #[serde(default)]
    outbound_messages: Vec<ManagedChatOutboundMessage>,
    #[serde(default)]
    local_state: ManagedChatLocalState,
}

pub struct ManagedChatProjectionState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub stream_id: String,
    pub projection_revision: u64,
    pub relay_events: Vec<Event>,
    pub outbound_messages: Vec<ManagedChatOutboundMessage>,
    /// One-off admin events (kind-40 etc.) awaiting publish via the lane worker.
    pub pending_admin_publishes: Vec<Event>,
    pub local_state: ManagedChatLocalState,
    pub snapshot: ManagedChatProjectionSnapshot,
    local_pubkey: Option<String>,
    /// Team channel ID from config — injected as a synthetic placeholder in the projection.
    team_channel_id: Option<String>,
    projection_file_path: PathBuf,
}

impl Default for ManagedChatProjectionState {
    fn default() -> Self {
        Self::from_projection_file_path(managed_chat_projection_file_path())
    }
}

impl ManagedChatProjectionState {
    fn from_projection_file_path(projection_file_path: PathBuf) -> Self {
        match load_managed_chat_projection_document(projection_file_path.as_path()) {
            Ok((relay_events, mut outbound_messages, local_state)) => {
                reconcile_outbound_messages_against_relays(&mut outbound_messages, &relay_events);
                let team_channel_id =
                    crate::app_state::DefaultNip28ChannelConfig::from_env_or_default()
                        .team_channel_id;
                let snapshot = rebuild_managed_chat_projection(
                    &relay_events,
                    &outbound_messages,
                    &local_state,
                    None,
                    team_channel_id.as_deref(),
                );
                Self {
                    load_state: PaneLoadState::Ready,
                    last_error: None,
                    last_action: Some(format!(
                        "Loaded managed chat projection stream ({} relay events / {} outbound)",
                        relay_events.len(),
                        outbound_messages.len()
                    )),
                    stream_id: MANAGED_CHAT_PROJECTION_STREAM_ID.to_string(),
                    projection_revision: 1,
                    relay_events,
                    outbound_messages,
                    pending_admin_publishes: Vec::new(),
                    local_state,
                    snapshot,
                    local_pubkey: None,
                    team_channel_id,
                    projection_file_path,
                }
            }
            Err(error) => Self {
                load_state: PaneLoadState::Error,
                last_error: Some(error),
                last_action: Some("Managed chat projection stream load failed".to_string()),
                stream_id: MANAGED_CHAT_PROJECTION_STREAM_ID.to_string(),
                projection_revision: 1,
                relay_events: Vec::new(),
                outbound_messages: Vec::new(),
                pending_admin_publishes: Vec::new(),
                local_state: ManagedChatLocalState::default(),
                snapshot: ManagedChatProjectionSnapshot::default(),
                local_pubkey: None,
                team_channel_id: None,
                projection_file_path,
            },
        }
    }

    pub(crate) fn from_projection_path_for_tests(projection_file_path: PathBuf) -> Self {
        // Tests do not inject the synthetic team channel (team_channel_id: None).
        let mut state = Self::from_projection_file_path(projection_file_path);
        if state.team_channel_id.is_some() {
            state.team_channel_id = None;
            state.snapshot = rebuild_managed_chat_projection(
                &state.relay_events,
                &state.outbound_messages,
                &state.local_state,
                state.local_pubkey.as_deref(),
                None,
            );
        }
        state
    }

    pub fn set_local_pubkey(&mut self, local_pubkey: Option<&str>) {
        self.local_pubkey = local_pubkey
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);
        self.refresh_projection("Configured managed chat identity");
    }

    pub fn local_pubkey(&self) -> Option<&str> {
        self.local_pubkey.as_deref()
    }

    pub fn projection_revision(&self) -> u64 {
        self.projection_revision
    }

    pub fn is_pubkey_muted(&self, pubkey: &str) -> bool {
        self.local_state
            .muted_pubkeys
            .contains(&pubkey.trim().to_ascii_lowercase())
    }

    pub fn set_pubkey_muted(&mut self, pubkey: &str, muted: bool) -> Result<(), String> {
        let normalized = pubkey.trim().to_ascii_lowercase();
        if normalized.len() != 64 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(format!("Invalid Nostr public key hex: {pubkey}"));
        }
        if muted {
            self.local_state.muted_pubkeys.insert(normalized.clone());
        } else {
            self.local_state.muted_pubkeys.remove(&normalized);
        }
        self.persist_current_state(if muted {
            format!("Muted managed chat member {normalized}")
        } else {
            format!("Unmuted managed chat member {normalized}")
        })
    }

    pub fn record_relay_event(&mut self, event: Event) {
        self.relay_events.push(event);
        self.refresh_projection("Projected managed chat event");
    }

    pub fn record_relay_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = Event>,
    {
        self.relay_events.extend(events);
        self.refresh_projection("Projected managed chat relay sync");
    }

    pub fn replace_relay_events(&mut self, relay_events: Vec<Event>) {
        self.relay_events = relay_events;
        self.refresh_projection("Rebuilt managed chat projection");
    }

    pub fn queue_outbound_message(
        &mut self,
        mut outbound_message: ManagedChatOutboundMessage,
    ) -> Result<(), String> {
        let parsed = ManagedChannelMessageEvent::from_event(&outbound_message.event)
            .map_err(|error| format!("Invalid managed chat outbound event: {error}"))?;
        if parsed.group_id != outbound_message.group_id {
            return Err(format!(
                "Managed chat outbound group mismatch: {} != {}",
                parsed.group_id, outbound_message.group_id
            ));
        }
        if parsed.channel_create_event_id != outbound_message.channel_id {
            return Err(format!(
                "Managed chat outbound channel mismatch: {} != {}",
                parsed.channel_create_event_id, outbound_message.channel_id
            ));
        }
        if parsed.relay_url != outbound_message.relay_url {
            return Err(format!(
                "Managed chat outbound relay mismatch: {} != {}",
                parsed.relay_url, outbound_message.relay_url
            ));
        }
        let Some(channel) = self
            .snapshot
            .channels
            .iter()
            .find(|channel| channel.channel_id == outbound_message.channel_id)
        else {
            return Err(format!(
                "Unknown managed chat outbound channel: {}",
                outbound_message.channel_id
            ));
        };
        if channel.group_id != outbound_message.group_id {
            return Err(format!(
                "Managed chat outbound channel {} does not belong to group {}",
                outbound_message.channel_id, outbound_message.group_id
            ));
        }

        outbound_message.attempt_count = outbound_message.attempt_count.max(1);
        self.outbound_messages
            .retain(|candidate| candidate.event.id != outbound_message.event.id);
        self.outbound_messages.push(outbound_message);
        let event_id = self
            .outbound_messages
            .last()
            .map(|message| message.event.id.clone())
            .unwrap_or_default();
        self.refresh_projection(format!("Queued managed chat local echo {event_id}"));
        Ok(())
    }

    pub fn fail_outbound_message(
        &mut self,
        event_id: &str,
        error: impl Into<String>,
    ) -> Result<(), String> {
        let Some(index) = self
            .outbound_messages
            .iter()
            .position(|message| message.event.id == event_id)
        else {
            return Err(format!("Unknown managed chat outbound message: {event_id}"));
        };
        self.outbound_messages[index].delivery_state = ManagedChatDeliveryState::Failed;
        self.outbound_messages[index].attempt_count =
            self.outbound_messages[index].attempt_count.max(1);
        self.outbound_messages[index].last_error = Some(error.into());
        self.refresh_projection(format!("Marked managed chat outbound {event_id} failed"));
        Ok(())
    }

    pub fn ack_outbound_message(&mut self, event_id: &str) -> Result<(), String> {
        let Some(index) = self
            .outbound_messages
            .iter()
            .position(|message| message.event.id == event_id)
        else {
            return Err(format!("Unknown managed chat outbound message: {event_id}"));
        };
        self.outbound_messages[index].delivery_state = ManagedChatDeliveryState::Acked;
        self.outbound_messages[index].attempt_count =
            self.outbound_messages[index].attempt_count.max(1);
        self.outbound_messages[index].last_error = None;
        self.refresh_projection(format!("Acknowledged managed chat outbound {event_id}"));
        Ok(())
    }

    pub fn retry_outbound_message(&mut self, event_id: &str) -> Result<(), String> {
        let Some(index) = self
            .outbound_messages
            .iter()
            .position(|message| message.event.id == event_id)
        else {
            return Err(format!("Unknown managed chat outbound message: {event_id}"));
        };
        self.outbound_messages[index].delivery_state = ManagedChatDeliveryState::Publishing;
        self.outbound_messages[index].attempt_count = self.outbound_messages[index]
            .attempt_count
            .max(1)
            .saturating_add(1);
        self.outbound_messages[index].last_error = None;
        self.refresh_projection(format!("Retried managed chat outbound {event_id}"));
        Ok(())
    }

    pub fn latest_retryable_outbound_event_id(&self, channel_id: &str) -> Option<String> {
        self.outbound_messages
            .iter()
            .filter(|message| {
                message.channel_id == channel_id && message.delivery_state.is_retryable()
            })
            .max_by(|left, right| {
                left.event
                    .created_at
                    .cmp(&right.event.created_at)
                    .then_with(|| left.event.id.cmp(&right.event.id))
            })
            .map(|message| message.event.id.clone())
    }

    pub fn set_selected_channel(&mut self, group_id: &str, channel_id: &str) -> Result<(), String> {
        let Some(channel) = self
            .snapshot
            .channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
        else {
            return Err(format!("Unknown managed chat channel: {channel_id}"));
        };
        if channel.group_id != group_id {
            // Stale selection — clear it silently rather than surfacing an error.
            self.local_state.selected_group_id = None;
            self.local_state.selected_channel_id = None;
            return Ok(());
        }
        self.local_state.selected_group_id = Some(group_id.to_string());
        self.local_state.selected_channel_id = Some(channel_id.to_string());
        self.mark_channel_read(channel_id, None)?;
        self.last_action = Some(format!("Selected managed chat channel {channel_id}"));
        Ok(())
    }

    pub fn set_selected_group(&mut self, group_id: &str) -> Result<(), String> {
        if !self
            .snapshot
            .groups
            .iter()
            .any(|group| group.group_id == group_id)
        {
            return Err(format!("Unknown managed chat group: {group_id}"));
        }
        self.local_state.selected_group_id = Some(group_id.to_string());
        self.local_state.selected_channel_id = self
            .snapshot
            .channels
            .iter()
            .find(|channel| channel.group_id == group_id)
            .map(|channel| channel.channel_id.clone());
        if let Some(channel_id) = self.local_state.selected_channel_id.clone() {
            self.mark_channel_read(channel_id.as_str(), None)?;
            self.last_action = Some(format!("Selected managed chat group {group_id}"));
            Ok(())
        } else {
            self.persist_current_state(format!("Selected managed chat group {group_id}"))
        }
    }

    pub fn toggle_category_collapsed(
        &mut self,
        group_id: &str,
        category_id: &str,
    ) -> Result<(), String> {
        if !self
            .snapshot
            .groups
            .iter()
            .any(|group| group.group_id == group_id)
        {
            return Err(format!("Unknown managed chat group: {group_id}"));
        }
        if !self.snapshot.channels.iter().any(|channel| {
            channel.group_id == group_id && managed_chat_channel_category_id(channel) == category_id
        }) {
            return Err(format!(
                "Unknown managed chat category {category_id} in group {group_id}"
            ));
        }
        let key = managed_chat_collapsed_category_key(group_id, category_id);
        if !self.local_state.collapsed_category_keys.insert(key.clone()) {
            self.local_state.collapsed_category_keys.remove(&key);
        }
        self.persist_current_state(format!(
            "Toggled managed chat category {category_id} in group {group_id}"
        ))
    }

    pub fn category_is_collapsed(&self, group_id: &str, category_id: &str) -> bool {
        self.local_state
            .collapsed_category_keys
            .contains(&managed_chat_collapsed_category_key(group_id, category_id))
    }

    pub fn mark_channel_read(
        &mut self,
        channel_id: &str,
        last_read_event_id: Option<&str>,
    ) -> Result<(), String> {
        let Some(channel) = self
            .snapshot
            .channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
        else {
            return Err(format!("Unknown managed chat channel: {channel_id}"));
        };

        let cursor = match last_read_event_id {
            Some(event_id) => {
                let Some(message) = self.snapshot.messages.get(event_id) else {
                    return Err(format!(
                        "Unknown managed chat message for read cursor: {event_id}"
                    ));
                };
                if message.channel_id != channel_id {
                    return Err(format!(
                        "Managed chat message {event_id} does not belong to channel {channel_id}"
                    ));
                }
                ManagedChatReadCursor {
                    last_read_event_id: Some(message.event_id.clone()),
                    last_read_created_at: Some(message.created_at),
                }
            }
            None => channel
                .latest_message_id
                .as_ref()
                .and_then(|event_id| self.snapshot.messages.get(event_id))
                .map(|message| ManagedChatReadCursor {
                    last_read_event_id: Some(message.event_id.clone()),
                    last_read_created_at: Some(message.created_at),
                })
                .unwrap_or_default(),
        };

        if cursor.last_read_event_id.is_some() {
            self.local_state
                .read_cursors
                .insert(channel_id.to_string(), cursor);
        } else {
            self.local_state.read_cursors.remove(channel_id);
        }
        self.refresh_projection(format!(
            "Updated read cursor for managed chat channel {channel_id}"
        ));
        Ok(())
    }

    pub fn reload_projection(&mut self) -> Result<(), String> {
        let (relay_events, mut outbound_messages, local_state) =
            load_managed_chat_projection_document(self.projection_file_path.as_path())?;
        self.relay_events = relay_events;
        reconcile_outbound_messages_against_relays(&mut outbound_messages, &self.relay_events);
        self.outbound_messages = outbound_messages;
        self.local_state = local_state;
        self.snapshot = rebuild_managed_chat_projection(
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
            self.local_pubkey.as_deref(),
            self.team_channel_id.as_deref(),
        );
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.projection_revision = self.projection_revision.saturating_add(1);
        self.last_action = Some(format!(
            "Managed chat projection reloaded ({} relay events / {} outbound)",
            self.relay_events.len(),
            self.outbound_messages.len()
        ));
        Ok(())
    }

    fn refresh_projection(&mut self, action: impl Into<String>) {
        self.relay_events =
            normalize_managed_chat_relay_events(std::mem::take(&mut self.relay_events));
        self.outbound_messages =
            normalize_managed_chat_outbound_messages(std::mem::take(&mut self.outbound_messages));
        reconcile_outbound_messages_against_relays(&mut self.outbound_messages, &self.relay_events);
        self.snapshot = rebuild_managed_chat_projection(
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
            self.local_pubkey.as_deref(),
            self.team_channel_id.as_deref(),
        );
        self.projection_revision = self.projection_revision.saturating_add(1);
        let action = format!(
            "{} ({} relay events / {} outbound / {} channels)",
            action.into(),
            self.relay_events.len(),
            self.outbound_messages.len(),
            self.snapshot.channels.len()
        );
        if let Err(error) = persist_managed_chat_projection_document(
            self.projection_file_path.as_path(),
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
        ) {
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
            self.last_action = Some(action);
        } else {
            self.last_error = None;
            self.load_state = PaneLoadState::Ready;
            self.last_action = Some(action);
        }
    }

    fn persist_current_state(&mut self, action: String) -> Result<(), String> {
        persist_managed_chat_projection_document(
            self.projection_file_path.as_path(),
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
        )?;
        self.projection_revision = self.projection_revision.saturating_add(1);
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(action);
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct MutableGroupProjection {
    metadata: GroupMetadata,
    roles: Vec<GroupRole>,
    admins: BTreeMap<String, Vec<String>>,
    members: BTreeMap<String, Vec<String>>,
    deleted: bool,
}

#[derive(Debug, Clone)]
struct MutableChannelProjection {
    group_id: String,
    room_mode: ManagedRoomMode,
    metadata: ChannelMetadata,
    hints: ManagedChannelHints,
    relay_url: Option<String>,
}

impl Default for MutableChannelProjection {
    fn default() -> Self {
        Self {
            group_id: String::new(),
            room_mode: ManagedRoomMode::ManagedChannel,
            metadata: ChannelMetadata::new("", "", ""),
            hints: ManagedChannelHints::default(),
            relay_url: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ReactionCandidate {
    group_id: String,
    event_refs: Vec<String>,
    author_pubkey: String,
    content: String,
}

fn managed_chat_projection_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-managed-chat-projection-v1.json")
}

fn normalize_managed_chat_relay_events(mut relay_events: Vec<Event>) -> Vec<Event> {
    relay_events.sort_by(compare_managed_timeline_events);
    let mut seen_event_ids = BTreeSet::new();
    relay_events.retain(|event| seen_event_ids.insert(event.id.clone()));
    if relay_events.len() > MANAGED_CHAT_EVENT_LIMIT {
        relay_events = relay_events.split_off(relay_events.len() - MANAGED_CHAT_EVENT_LIMIT);
    }
    relay_events
}

fn managed_chat_channel_category_id(channel: &ManagedChatChannelProjection) -> &str {
    channel
        .hints
        .category_id
        .as_deref()
        .unwrap_or(MANAGED_CHAT_UNCATEGORIZED_CATEGORY_ID)
}

fn managed_chat_collapsed_category_key(group_id: &str, category_id: &str) -> String {
    format!("{group_id}:{category_id}")
}

fn normalize_managed_chat_outbound_messages(
    outbound_messages: Vec<ManagedChatOutboundMessage>,
) -> Vec<ManagedChatOutboundMessage> {
    let mut deduped = BTreeMap::<String, ManagedChatOutboundMessage>::new();
    for mut outbound_message in outbound_messages {
        outbound_message.attempt_count = outbound_message.attempt_count.max(1);
        deduped.insert(outbound_message.event.id.clone(), outbound_message);
    }
    let mut normalized = deduped.into_values().collect::<Vec<_>>();
    normalized.sort_by(|left, right| {
        left.event
            .created_at
            .cmp(&right.event.created_at)
            .then_with(|| left.event.id.cmp(&right.event.id))
    });
    normalized
}

fn reconcile_outbound_messages_against_relays(
    outbound_messages: &mut [ManagedChatOutboundMessage],
    relay_events: &[Event],
) {
    let relay_event_ids = relay_events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<BTreeSet<_>>();
    for outbound_message in outbound_messages {
        if relay_event_ids.contains(outbound_message.event.id.as_str()) {
            outbound_message.delivery_state = ManagedChatDeliveryState::Acked;
            outbound_message.last_error = None;
            outbound_message.attempt_count = outbound_message.attempt_count.max(1);
        }
    }
}

fn persist_managed_chat_projection_document(
    path: &Path,
    relay_events: &[Event],
    outbound_messages: &[ManagedChatOutboundMessage],
    local_state: &ManagedChatLocalState,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create managed chat projection dir: {error}"))?;
    }
    let document = ManagedChatProjectionDocumentV1 {
        schema_version: MANAGED_CHAT_PROJECTION_SCHEMA_VERSION,
        stream_id: MANAGED_CHAT_PROJECTION_STREAM_ID.to_string(),
        relay_events: normalize_managed_chat_relay_events(relay_events.to_vec()),
        outbound_messages: normalize_managed_chat_outbound_messages(outbound_messages.to_vec()),
        local_state: local_state.clone(),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode managed chat projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write managed chat projection temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist managed chat projection: {error}"))?;
    Ok(())
}

fn load_managed_chat_projection_document(
    path: &Path,
) -> Result<
    (
        Vec<Event>,
        Vec<ManagedChatOutboundMessage>,
        ManagedChatLocalState,
    ),
    String,
> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new(), ManagedChatLocalState::default()));
        }
        Err(error) => {
            return Err(format!("Failed to read managed chat projection: {error}"));
        }
    };

    let document = serde_json::from_str::<ManagedChatProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse managed chat projection: {error}"))?;
    if document.schema_version != MANAGED_CHAT_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported managed chat projection schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != MANAGED_CHAT_PROJECTION_STREAM_ID {
        return Err(format!(
            "Unsupported managed chat projection stream id: {}",
            document.stream_id
        ));
    }
    Ok((
        normalize_managed_chat_relay_events(document.relay_events),
        normalize_managed_chat_outbound_messages(document.outbound_messages),
        document.local_state,
    ))
}

fn rebuild_managed_chat_projection(
    relay_events: &[Event],
    outbound_messages: &[ManagedChatOutboundMessage],
    local_state: &ManagedChatLocalState,
    local_pubkey: Option<&str>,
    team_channel_id: Option<&str>,
) -> ManagedChatProjectionSnapshot {
    let deleted_event_ids = collect_deleted_event_ids(relay_events);
    let mut groups = BTreeMap::<String, MutableGroupProjection>::new();
    let mut channels = BTreeMap::<String, MutableChannelProjection>::new();
    let mut messages = BTreeMap::<String, ManagedChatMessageProjection>::new();
    let mut reaction_candidates = Vec::<ReactionCandidate>::new();

    for event in relay_events
        .iter()
        .filter(|event| !deleted_event_ids.contains(&event.id))
    {
        if let Some(candidate) = parse_reaction_candidate(event) {
            reaction_candidates.push(candidate);
            continue;
        }

        match event.kind {
            39000 => {
                if let Ok(parsed) = GroupMetadataEvent::from_event(event) {
                    let group = groups.entry(parsed.group_id).or_default();
                    group.metadata = parsed.metadata;
                    group.deleted = false;
                }
            }
            39001 => {
                if let Ok(parsed) = GroupAdminsEvent::from_event(event) {
                    let group = groups.entry(parsed.group_id).or_default();
                    group.admins = pubkey_label_map(
                        parsed
                            .admins
                            .into_iter()
                            .map(|admin| (admin.pubkey, admin.labels)),
                    );
                    group.deleted = false;
                }
            }
            39002 => {
                if let Ok(parsed) = GroupMembersEvent::from_event(event) {
                    let group = groups.entry(parsed.group_id).or_default();
                    group.members = pubkey_label_map(
                        parsed
                            .members
                            .into_iter()
                            .map(|member| (member.pubkey, member.labels)),
                    );
                    group.deleted = false;
                }
            }
            39003 => {
                if let Ok(parsed) = GroupRolesEvent::from_event(event) {
                    let group = groups.entry(parsed.group_id).or_default();
                    let mut roles = parsed.roles;
                    roles.sort_by(|left, right| left.name.cmp(&right.name));
                    group.roles = roles;
                    group.deleted = false;
                }
            }
            40 => {
                if let Ok(parsed) = ManagedChannelCreateEvent::from_event(event) {
                    groups.entry(parsed.group_id.clone()).or_default().deleted = false;
                    let channel = channels.entry(event.id.clone()).or_default();
                    channel.group_id = parsed.group_id;
                    channel.room_mode = parsed.hints.room_mode;
                    channel.metadata = parsed.metadata;
                    channel.hints = parsed.hints;
                } else if let Ok(metadata) = ChannelMetadata::from_json(&event.content) {
                    // Standard NIP-28 kind-40: no OA group tag — use event.id as group_id,
                    // except for the team channel which always belongs to oa-default.
                    let effective_group_id = if Some(event.id.as_str()) == team_channel_id {
                        "oa-default".to_string()
                    } else {
                        event.id.clone()
                    };
                    groups.entry(effective_group_id.clone()).or_default().deleted = false;
                    let channel = channels.entry(event.id.clone()).or_default();
                    channel.group_id = effective_group_id;
                    channel.metadata = metadata;
                } else {
                    tracing::warn!(event_id = %event.id, "nip28: kind-40 parse failed, content not valid ChannelMetadata JSON");
                }
            }
            41 => {
                if let Ok(parsed) = ManagedChannelMetadataEvent::from_event(event) {
                    groups.entry(parsed.group_id.clone()).or_default().deleted = false;
                    let channel = channels.entry(parsed.channel_create_event_id).or_default();
                    channel.group_id = parsed.group_id;
                    channel.room_mode = parsed.hints.room_mode;
                    channel.metadata = parsed.metadata;
                    channel.hints = parsed.hints;
                    channel.relay_url = Some(parsed.relay_url);
                } else if let Ok(metadata) = ChannelMetadata::from_json(&event.content) {
                    // Standard NIP-28 kind-41: apply metadata update via root "e" tag
                    if let Some(channel_create_id) = event
                        .tags
                        .iter()
                        .find(|t| t.first().map(|s| s == "e").unwrap_or(false))
                        .and_then(|t| t.get(1))
                    {
                        let channel = channels.entry(channel_create_id.clone()).or_default();
                        channel.metadata = metadata;
                    }
                } else {
                    tracing::warn!(event_id = %event.id, "nip28: kind-41 parse failed, content not valid ChannelMetadata JSON");
                }
            }
            42 => {
                if let Ok(parsed) = ManagedChannelMessageEvent::from_event(event) {
                    groups.entry(parsed.group_id.clone()).or_default().deleted = false;
                    let channel = channels
                        .entry(parsed.channel_create_event_id.clone())
                        .or_default();
                    channel.group_id = parsed.group_id.clone();
                    if channel.relay_url.is_none() {
                        channel.relay_url = Some(parsed.relay_url.clone());
                    }

                    let mut mention_pubkeys = parsed
                        .mentions
                        .into_iter()
                        .map(|mention| mention.pubkey)
                        .collect::<Vec<_>>();
                    mention_pubkeys.sort();
                    mention_pubkeys.dedup();

                    messages.insert(
                        event.id.clone(),
                        ManagedChatMessageProjection {
                            event_id: event.id.clone(),
                            group_id: parsed.group_id,
                            channel_id: parsed.channel_create_event_id,
                            author_pubkey: event.pubkey.clone(),
                            content: parsed.content,
                            created_at: event.created_at,
                            reply_to_event_id: parsed.reply_to.map(|reply| reply.event_id),
                            mention_pubkeys,
                            reaction_summaries: Vec::new(),
                            reply_child_ids: Vec::new(),
                            delivery_state: ManagedChatDeliveryState::Confirmed,
                            delivery_error: None,
                            attempt_count: 0,
                            message_class: crate::chat_message_classifier::classify(event),
                        },
                    );
                } else {
                    // Fallback: plain NIP-28 kind-42 from external clients (e.g. Amethyst)
                    // that omit the ['h', group_id] tag. Extract the root 'e' tag manually
                    // and assign to oa-default so messages are never silently dropped.
                    let root_tag = event.tags.iter().find(|tag| {
                        tag.len() >= 2
                            && tag[0] == "e"
                            && tag.get(3).map(|s| s == "root").unwrap_or(false)
                    });
                    if let Some(root_tag) = root_tag {
                        let channel_id = root_tag[1].clone();
                        let relay = root_tag
                            .get(2)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.clone())
                            .unwrap_or_else(|| {
                                crate::app_state::DEFAULT_NIP28_RELAY_URL.to_string()
                            });
                        let group_id = if Some(channel_id.as_str()) == team_channel_id {
                            "oa-default".to_string()
                        } else {
                            channels
                                .get(&channel_id)
                                .map(|ch| ch.group_id.clone())
                                .unwrap_or_else(|| "oa-default".to_string())
                        };
                        groups.entry(group_id.clone()).or_default().deleted = false;
                        let channel = channels.entry(channel_id.clone()).or_default();
                        channel.group_id = group_id.clone();
                        if channel.relay_url.is_none() {
                            channel.relay_url = Some(relay);
                        }
                        messages.insert(
                            event.id.clone(),
                            ManagedChatMessageProjection {
                                event_id: event.id.clone(),
                                group_id,
                                channel_id,
                                author_pubkey: event.pubkey.clone(),
                                content: event.content.clone(),
                                created_at: event.created_at,
                                reply_to_event_id: None,
                                mention_pubkeys: Vec::new(),
                                reaction_summaries: Vec::new(),
                                reply_child_ids: Vec::new(),
                                delivery_state: ManagedChatDeliveryState::Confirmed,
                                delivery_error: None,
                                attempt_count: 0,
                                message_class: crate::chat_message_classifier::classify(event),
                            },
                        );
                    }
                }
            }
            kind if (9000..=9020).contains(&kind) => {
                if let Ok(parsed) = ModerationEvent::from_event(event) {
                    let group = groups.entry(parsed.group_id).or_default();
                    match parsed.action {
                        ModerationAction::PutUser { pubkey, roles } => {
                            group.members.insert(pubkey, sorted_labels(roles));
                            group.deleted = false;
                        }
                        ModerationAction::RemoveUser { pubkey } => {
                            group.members.remove(&pubkey);
                            group.admins.remove(&pubkey);
                        }
                        ModerationAction::EditMetadata { changes } => {
                            apply_group_metadata_changes(&mut group.metadata, &changes);
                            group.deleted = false;
                        }
                        ModerationAction::DeleteEvent { .. } => {}
                        ModerationAction::CreateGroup => {
                            group.deleted = false;
                        }
                        ModerationAction::DeleteGroup => {
                            group.deleted = true;
                        }
                        ModerationAction::CreateInvite { .. } => {}
                    }
                }
            }
            _ => {}
        }
    }

    let live_group_ids = groups
        .iter()
        .filter(|(_, group)| !group.deleted)
        .map(|(group_id, _)| group_id.clone())
        .collect::<BTreeSet<_>>();

    let live_channels = channels
        .into_iter()
        .filter(|(channel_id, channel)| {
            !channel.group_id.is_empty()
                && live_group_ids.contains(&channel.group_id)
                && !deleted_event_ids.contains(channel_id)
        })
        .collect::<BTreeMap<_, _>>();

    messages.retain(|_, message| {
        live_channels
            .get(&message.channel_id)
            .is_some_and(|channel| channel.group_id == message.group_id)
    });

    let relay_message_ids = messages.keys().cloned().collect::<BTreeSet<_>>();
    for outbound_message in outbound_messages {
        if relay_message_ids.contains(&outbound_message.event.id) {
            continue;
        }
        if !live_channels
            .get(&outbound_message.channel_id)
            .is_some_and(|channel| channel.group_id == outbound_message.group_id)
        {
            continue;
        }
        let Ok(parsed) = ManagedChannelMessageEvent::from_event(&outbound_message.event) else {
            continue;
        };
        if parsed.group_id != outbound_message.group_id
            || parsed.channel_create_event_id != outbound_message.channel_id
        {
            continue;
        }

        let mut mention_pubkeys = parsed
            .mentions
            .into_iter()
            .map(|mention| mention.pubkey)
            .collect::<Vec<_>>();
        mention_pubkeys.sort();
        mention_pubkeys.dedup();

        messages.insert(
            outbound_message.event.id.clone(),
            ManagedChatMessageProjection {
                event_id: outbound_message.event.id.clone(),
                group_id: parsed.group_id,
                channel_id: parsed.channel_create_event_id,
                author_pubkey: outbound_message.event.pubkey.clone(),
                content: parsed.content,
                created_at: outbound_message.event.created_at,
                reply_to_event_id: parsed.reply_to.map(|reply| reply.event_id),
                mention_pubkeys,
                reaction_summaries: Vec::new(),
                reply_child_ids: Vec::new(),
                delivery_state: outbound_message.delivery_state,
                delivery_error: outbound_message.last_error.clone(),
                attempt_count: outbound_message.attempt_count.max(1),
                message_class: crate::chat_message_classifier::ChatMessageClass::HumanMessage,
            },
        );
    }

    let mut reaction_map = BTreeMap::<String, BTreeMap<String, BTreeSet<String>>>::new();
    for candidate in reaction_candidates {
        let Some(target_event_id) = candidate
            .event_refs
            .iter()
            .rev()
            .find(|event_id| {
                messages
                    .get(*event_id)
                    .is_some_and(|message| message.group_id == candidate.group_id)
            })
            .cloned()
        else {
            continue;
        };
        reaction_map
            .entry(target_event_id)
            .or_default()
            .entry(candidate.content.clone())
            .or_default()
            .insert(candidate.author_pubkey);
    }

    let mut reply_children = BTreeMap::<String, Vec<String>>::new();
    for message in messages.values() {
        let Some(parent_id) = message.reply_to_event_id.as_ref() else {
            continue;
        };
        if messages.contains_key(parent_id) {
            reply_children
                .entry(parent_id.clone())
                .or_default()
                .push(message.event_id.clone());
        }
    }
    for child_ids in reply_children.values_mut() {
        child_ids.sort_by(|left, right| compare_message_ids(left, right, &messages));
    }

    for message in messages.values_mut() {
        message.reply_child_ids = reply_children
            .get(&message.event_id)
            .cloned()
            .unwrap_or_default();
        message.reaction_summaries = reaction_map
            .get(&message.event_id)
            .map(|reactions| {
                reactions
                    .iter()
                    .map(|(content, authors)| ManagedChatReactionSummary {
                        content: content.clone(),
                        author_pubkeys: authors.iter().cloned().collect(),
                        count: authors.len(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
    }

    let mut channel_rows = live_channels
        .iter()
        .map(|(channel_id, channel)| {
            let mut message_ids = messages
                .values()
                .filter(|message| message.channel_id == *channel_id)
                .map(|message| message.event_id.clone())
                .collect::<Vec<_>>();
            message_ids.sort_by(|left, right| compare_message_ids(left, right, &messages));
            let root_message_ids = message_ids
                .iter()
                .filter(|event_id| {
                    messages
                        .get(event_id.as_str())
                        .and_then(|message| message.reply_to_event_id.as_ref())
                        .is_none()
                })
                .cloned()
                .collect::<Vec<_>>();
            let latest_message_id = message_ids.last().cloned();
            let unread_count = unread_count_for_channel(
                &message_ids,
                local_state.read_cursors.get(channel_id),
                &messages,
            );
            let mention_count = mention_count_for_channel(
                &message_ids,
                local_state.read_cursors.get(channel_id),
                &messages,
                local_pubkey,
            );
            ManagedChatChannelProjection {
                channel_id: channel_id.clone(),
                group_id: channel.group_id.clone(),
                room_mode: channel.room_mode,
                metadata: channel.metadata.clone(),
                hints: channel.hints.clone(),
                relay_url: channel.relay_url.clone(),
                message_ids,
                root_message_ids,
                unread_count,
                mention_count,
                latest_message_id,
            }
        })
        .collect::<Vec<_>>();

    // Inject a synthetic team channel if a team_channel_id is provided
    // and the channel hasn't already been loaded from the relay into oa-default.
    if let Some(team_id) = team_channel_id {
        let already_in_oa_default = channel_rows
            .iter()
            .any(|c| c.channel_id == team_id && c.group_id == "oa-default");
        if !already_in_oa_default {
            channel_rows.push(ManagedChatChannelProjection {
                channel_id: team_id.to_string(),
                group_id: "oa-default".to_string(),
                room_mode: ManagedRoomMode::default(),
                metadata: ChannelMetadata::new("Team", "", ""),
                hints: ManagedChannelHints::new(),
                relay_url: None,
                message_ids: Vec::new(),
                root_message_ids: Vec::new(),
                unread_count: 0,
                mention_count: 0,
                latest_message_id: None,
            });
        }
    }

    let mut group_channel_ids = BTreeMap::<String, Vec<String>>::new();
    for channel in &channel_rows {
        group_channel_ids
            .entry(channel.group_id.clone())
            .or_default()
            .push(channel.channel_id.clone());
    }

    let mut ordered_group_channel_ids = BTreeMap::<String, Vec<String>>::new();
    for (group_id, channel_ids) in group_channel_ids {
        let mut entries = channel_ids
            .iter()
            .filter_map(|channel_id| {
                let channel = live_channels.get(channel_id)?;
                ManagedChannelIndexEntry::new(
                    channel_id.clone(),
                    channel.metadata.clone(),
                    channel.hints.clone(),
                )
                .ok()
            })
            .collect::<Vec<_>>();
        sort_managed_channel_index(&mut entries);
        let mut ordered = entries
            .into_iter()
            .map(|entry| entry.channel_create_event_id)
            .collect::<Vec<_>>();
        let ordered_ids = ordered.iter().cloned().collect::<BTreeSet<_>>();
        let mut leftovers = channel_ids
            .into_iter()
            .filter(|channel_id| !ordered_ids.contains(channel_id))
            .collect::<Vec<_>>();
        leftovers.sort();
        ordered.extend(leftovers);
        ordered_group_channel_ids.insert(group_id, ordered);
    }

    channel_rows.sort_by(|left, right| {
        left.group_id
            .cmp(&right.group_id)
            .then_with(|| {
                channel_rank(
                    ordered_group_channel_ids.get(&left.group_id),
                    &left.channel_id,
                )
                .cmp(&channel_rank(
                    ordered_group_channel_ids.get(&right.group_id),
                    &right.channel_id,
                ))
            })
            .then_with(|| left.channel_id.cmp(&right.channel_id))
    });

    let channel_lookup = channel_rows
        .iter()
        .map(|channel| (channel.channel_id.clone(), channel))
        .collect::<BTreeMap<_, _>>();

    let mut group_rows = groups
        .into_iter()
        .filter(|(group_id, group)| !group.deleted && live_group_ids.contains(group_id))
        .map(|(group_id, group)| {
            let channel_ids = ordered_group_channel_ids
                .remove(&group_id)
                .unwrap_or_default();
            let unread_count = channel_ids
                .iter()
                .filter_map(|channel_id| channel_lookup.get(channel_id))
                .map(|channel| channel.unread_count)
                .sum();
            let mention_count = channel_ids
                .iter()
                .filter_map(|channel_id| channel_lookup.get(channel_id))
                .map(|channel| channel.mention_count)
                .sum();
            let mut roles = group.roles;
            roles.sort_by(|left, right| left.name.cmp(&right.name));
            ManagedChatGroupProjection {
                group_id,
                metadata: group.metadata,
                roles,
                members: finalize_group_members(group.members, group.admins),
                channel_ids,
                unread_count,
                mention_count,
            }
        })
        .collect::<Vec<_>>();
    group_rows.sort_by(|left, right| left.group_id.cmp(&right.group_id));

    ManagedChatProjectionSnapshot {
        groups: group_rows,
        channels: channel_rows,
        messages,
    }
}

fn collect_deleted_event_ids(relay_events: &[Event]) -> BTreeSet<String> {
    let mut deleted = BTreeSet::new();
    for event in relay_events {
        if !(9000..=9020).contains(&event.kind) {
            continue;
        }
        let Ok(parsed) = ModerationEvent::from_event(event) else {
            continue;
        };
        if let ModerationAction::DeleteEvent { event_id } = parsed.action {
            deleted.insert(event_id);
        }
    }
    deleted
}

fn parse_reaction_candidate(event: &Event) -> Option<ReactionCandidate> {
    if event.kind != 7 {
        return None;
    }
    let group_id = tag_value(&event.tags, "h")?.to_string();
    let content = event.content.trim().to_string();
    if content.is_empty() {
        return None;
    }
    let event_refs = event
        .tags
        .iter()
        .filter(|tag| tag_name(tag.as_slice()) == Some("e"))
        .filter_map(|tag| tag.get(1).cloned())
        .collect::<Vec<_>>();
    if event_refs.is_empty() {
        return None;
    }
    Some(ReactionCandidate {
        group_id,
        event_refs,
        author_pubkey: event.pubkey.clone(),
        content,
    })
}

fn tag_name(tag: &[String]) -> Option<&str> {
    tag.first().map(String::as_str)
}

fn tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag_name(tag.as_slice()) == Some(name))
        .and_then(|tag| tag.get(1))
        .map(String::as_str)
}

fn pubkey_label_map<I>(entries: I) -> BTreeMap<String, Vec<String>>
where
    I: IntoIterator<Item = (String, Vec<String>)>,
{
    entries
        .into_iter()
        .map(|(pubkey, labels)| (pubkey, sorted_labels(labels)))
        .collect()
}

fn sorted_labels(mut labels: Vec<String>) -> Vec<String> {
    labels.sort();
    labels.dedup();
    labels
}

fn apply_group_metadata_changes(metadata: &mut GroupMetadata, changes: &[Vec<String>]) {
    for change in changes {
        match tag_name(change.as_slice()) {
            Some("name") => {
                metadata.name = change.get(1).cloned();
            }
            Some("picture") => {
                metadata.picture = change.get(1).cloned();
            }
            Some("about") => {
                metadata.about = change.get(1).cloned();
            }
            Some("private") => metadata.private = true,
            Some("public") => metadata.private = false,
            Some("restricted") => metadata.restricted = true,
            Some("unrestricted") => metadata.restricted = false,
            Some("hidden") => metadata.hidden = true,
            Some("visible") => metadata.hidden = false,
            Some("closed") => metadata.closed = true,
            Some("open") => metadata.closed = false,
            _ => {}
        }
    }
}

fn finalize_group_members(
    members: BTreeMap<String, Vec<String>>,
    admins: BTreeMap<String, Vec<String>>,
) -> Vec<ManagedChatMemberProjection> {
    let mut pubkeys = members.keys().cloned().collect::<BTreeSet<_>>();
    pubkeys.extend(admins.keys().cloned());

    pubkeys
        .into_iter()
        .map(|pubkey| {
            let mut labels = members.get(&pubkey).cloned().unwrap_or_default();
            labels.extend(admins.get(&pubkey).cloned().unwrap_or_default());
            labels = sorted_labels(labels);
            ManagedChatMemberProjection {
                pubkey: pubkey.clone(),
                labels,
                is_admin: admins.contains_key(&pubkey),
            }
        })
        .collect()
}

fn channel_rank(ordered_ids: Option<&Vec<String>>, channel_id: &str) -> usize {
    ordered_ids
        .and_then(|ordered| ordered.iter().position(|value| value == channel_id))
        .unwrap_or(usize::MAX)
}

fn compare_message_ids(
    left: &str,
    right: &str,
    messages: &BTreeMap<String, ManagedChatMessageProjection>,
) -> Ordering {
    let Some(left_message) = messages.get(left) else {
        return left.cmp(right);
    };
    let Some(right_message) = messages.get(right) else {
        return left.cmp(right);
    };
    left_message
        .created_at
        .cmp(&right_message.created_at)
        .then_with(|| left_message.event_id.cmp(&right_message.event_id))
}

fn unread_count_for_channel(
    message_ids: &[String],
    cursor: Option<&ManagedChatReadCursor>,
    messages: &BTreeMap<String, ManagedChatMessageProjection>,
) -> usize {
    unread_messages_for_channel(message_ids, cursor, messages).len()
}

fn mention_count_for_channel(
    message_ids: &[String],
    cursor: Option<&ManagedChatReadCursor>,
    messages: &BTreeMap<String, ManagedChatMessageProjection>,
    local_pubkey: Option<&str>,
) -> usize {
    let Some(local_pubkey) = local_pubkey else {
        return 0;
    };
    unread_messages_for_channel(message_ids, cursor, messages)
        .into_iter()
        .filter(|message| {
            message.author_pubkey != local_pubkey
                && message
                    .mention_pubkeys
                    .iter()
                    .any(|candidate| candidate == local_pubkey)
        })
        .count()
}

fn unread_messages_for_channel<'a>(
    message_ids: &'a [String],
    cursor: Option<&ManagedChatReadCursor>,
    messages: &'a BTreeMap<String, ManagedChatMessageProjection>,
) -> Vec<&'a ManagedChatMessageProjection> {
    message_ids
        .iter()
        .filter_map(|event_id| messages.get(event_id.as_str()))
        .filter(|message| message.delivery_state == ManagedChatDeliveryState::Confirmed)
        .filter(|message| managed_chat_message_is_after_cursor(message, cursor))
        .collect()
}

fn managed_chat_message_is_after_cursor(
    message: &ManagedChatMessageProjection,
    cursor: Option<&ManagedChatReadCursor>,
) -> bool {
    let Some(cursor) = cursor else {
        return true;
    };
    let read_created_at = cursor.last_read_created_at.unwrap_or(0);
    let read_event_id = cursor.last_read_event_id.as_deref().unwrap_or("");
    message.created_at > read_created_at
        || (message.created_at == read_created_at && message.event_id.as_str() > read_event_id)
}

#[cfg(test)]
mod tests {
    use super::{
        ManagedChatDeliveryState, ManagedChatOutboundMessage, ManagedChatProjectionState,
        ManagedChatReadCursor, tag_value, unread_count_for_channel,
    };
    use nostr::{
        ChannelMetadata, Event, GroupAdminsEvent, GroupMembersEvent, GroupMetadata,
        GroupMetadataEvent, GroupRole, GroupRolesEvent, ManagedChannelCreateEvent,
        ManagedChannelHints, ManagedChannelMessageEvent, ManagedChannelMetadataEvent,
        ManagedChannelType, ManagedChatMention, ModerationAction, ModerationEvent, TaggedPubkey,
    };
    use tempfile::tempdir;

    fn repeated_hex(ch: char, len: usize) -> String {
        std::iter::repeat_n(ch, len).collect()
    }

    fn signed_event(
        id_ch: char,
        pubkey_ch: char,
        created_at: u64,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> Event {
        Event {
            id: repeated_hex(id_ch, 64),
            pubkey: repeated_hex(pubkey_ch, 64),
            created_at,
            kind,
            tags,
            content,
            sig: repeated_hex('f', 128),
        }
    }

    fn build_group_metadata_event(id_ch: char, created_at: u64, name: &str) -> Event {
        let metadata = GroupMetadata::new()
            .with_name(name)
            .with_about("ops coordination");
        let template = GroupMetadataEvent::new("oa-main", metadata, created_at).unwrap();
        signed_event(
            id_ch,
            'a',
            created_at,
            39000,
            template.to_tags(),
            String::new(),
        )
    }

    fn build_admins_event(id_ch: char, created_at: u64, admins: Vec<TaggedPubkey>) -> Event {
        let template = GroupAdminsEvent::new("oa-main", admins, created_at).unwrap();
        signed_event(
            id_ch,
            'b',
            created_at,
            39001,
            template.to_tags(),
            "admins".to_string(),
        )
    }

    fn build_members_event(id_ch: char, created_at: u64, members: Vec<TaggedPubkey>) -> Event {
        let template = GroupMembersEvent::new("oa-main", members, created_at).unwrap();
        signed_event(
            id_ch,
            'c',
            created_at,
            39002,
            template.to_tags(),
            "members".to_string(),
        )
    }

    fn build_roles_event(id_ch: char, created_at: u64) -> Event {
        let template = GroupRolesEvent::new(
            "oa-main",
            vec![
                GroupRole::new("admin").with_description("full access"),
                GroupRole::new("moderator").with_description("room ops"),
            ],
            created_at,
        )
        .unwrap();
        signed_event(
            id_ch,
            'd',
            created_at,
            39003,
            template.to_tags(),
            "roles".to_string(),
        )
    }

    fn build_channel_create_event(id_ch: char, created_at: u64, name: &str) -> Event {
        let metadata = ChannelMetadata::new(name, "coordination", "");
        let template = ManagedChannelCreateEvent::new("oa-main", metadata, created_at)
            .unwrap()
            .with_hints(
                ManagedChannelHints::new()
                    .with_slug(name)
                    .with_channel_type(ManagedChannelType::Ops)
                    .with_category_id("ops")
                    .with_category_label("Operations")
                    .with_position(1),
            )
            .unwrap();
        signed_event(
            id_ch,
            'e',
            created_at,
            40,
            template.to_tags().unwrap(),
            template.content().unwrap(),
        )
    }

    fn build_channel_metadata_event(
        id_ch: char,
        created_at: u64,
        channel_create_event_id: &str,
        name: &str,
        position: u32,
    ) -> Event {
        let metadata = ChannelMetadata::new(name, "renamed", "");
        let template = ManagedChannelMetadataEvent::new(
            "oa-main",
            channel_create_event_id,
            "wss://relay.openagents.test",
            metadata,
            created_at,
        )
        .unwrap()
        .with_hints(
            ManagedChannelHints::new()
                .with_slug(name)
                .with_channel_type(ManagedChannelType::Ops)
                .with_category_id("ops")
                .with_category_label("Operations")
                .with_position(position),
        )
        .unwrap();
        signed_event(
            id_ch,
            '1',
            created_at,
            41,
            template.to_tags().unwrap(),
            template.content().unwrap(),
        )
    }

    fn build_message_event(
        id_ch: char,
        pubkey_ch: char,
        created_at: u64,
        channel_create_event_id: &str,
        content: &str,
    ) -> Event {
        let template = ManagedChannelMessageEvent::new(
            "oa-main",
            channel_create_event_id,
            "wss://relay.openagents.test",
            content,
            created_at,
        )
        .unwrap();
        signed_event(
            id_ch,
            pubkey_ch,
            created_at,
            42,
            template.to_tags().unwrap(),
            content.to_string(),
        )
    }

    fn build_message_with_mentions_event(
        id_ch: char,
        pubkey_ch: char,
        created_at: u64,
        channel_create_event_id: &str,
        content: &str,
        mentions: Vec<ManagedChatMention>,
    ) -> Event {
        let template = ManagedChannelMessageEvent::new(
            "oa-main",
            channel_create_event_id,
            "wss://relay.openagents.test",
            content,
            created_at,
        )
        .unwrap()
        .with_mentions(mentions);
        signed_event(
            id_ch,
            pubkey_ch,
            created_at,
            42,
            template.to_tags().unwrap(),
            content.to_string(),
        )
    }

    fn build_reply_event(
        id_ch: char,
        pubkey_ch: char,
        created_at: u64,
        channel_create_event_id: &str,
        reply_to_event_id: &str,
        reply_author_pubkey: &str,
        content: &str,
    ) -> Event {
        let reply = nostr::ManagedMessageReply::new(
            reply_to_event_id,
            "wss://relay.openagents.test",
            reply_author_pubkey,
        )
        .unwrap();
        let template = ManagedChannelMessageEvent::new(
            "oa-main",
            channel_create_event_id,
            "wss://relay.openagents.test",
            content,
            created_at,
        )
        .unwrap()
        .with_reply(reply);
        signed_event(
            id_ch,
            pubkey_ch,
            created_at,
            42,
            template.to_tags().unwrap(),
            content.to_string(),
        )
    }

    fn build_reaction_event(
        id_ch: char,
        pubkey_ch: char,
        created_at: u64,
        message_event_id: &str,
    ) -> Event {
        signed_event(
            id_ch,
            pubkey_ch,
            created_at,
            7,
            vec![
                vec!["h".to_string(), "oa-main".to_string()],
                vec!["e".to_string(), message_event_id.to_string()],
            ],
            "+".to_string(),
        )
    }

    fn build_delete_event(id_ch: char, created_at: u64, event_id: &str) -> Event {
        let template = ModerationEvent::new(
            "oa-main",
            ModerationAction::DeleteEvent {
                event_id: event_id.to_string(),
            },
            created_at,
        )
        .unwrap();
        signed_event(
            id_ch,
            '2',
            created_at,
            template.kind(),
            template.to_tags().unwrap(),
            "delete".to_string(),
        )
    }

    fn build_remove_user_event(id_ch: char, created_at: u64, pubkey: &str) -> Event {
        let template = ModerationEvent::new(
            "oa-main",
            ModerationAction::RemoveUser {
                pubkey: pubkey.to_string(),
            },
            created_at,
        )
        .unwrap();
        signed_event(
            id_ch,
            '3',
            created_at,
            template.kind(),
            template.to_tags().unwrap(),
            "remove".to_string(),
        )
    }

    fn build_put_user_event(
        id_ch: char,
        created_at: u64,
        pubkey: &str,
        roles: Vec<String>,
    ) -> Event {
        let template = ModerationEvent::new(
            "oa-main",
            ModerationAction::PutUser {
                pubkey: pubkey.to_string(),
                roles,
            },
            created_at,
        )
        .unwrap();
        signed_event(
            id_ch,
            '4',
            created_at,
            template.kind(),
            template.to_tags().unwrap(),
            "put".to_string(),
        )
    }

    fn build_edit_metadata_event(id_ch: char, created_at: u64, changes: Vec<Vec<String>>) -> Event {
        let template = ModerationEvent::new(
            "oa-main",
            ModerationAction::EditMetadata { changes },
            created_at,
        )
        .unwrap();
        signed_event(
            id_ch,
            '5',
            created_at,
            template.kind(),
            template.to_tags().unwrap(),
            "edit metadata".to_string(),
        )
    }

    #[test]
    fn managed_chat_projection_rebuild_is_deterministic_across_arrival_order_and_reload() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection =
            ManagedChatProjectionState::from_projection_path_for_tests(path.clone());

        let admin = TaggedPubkey::new(repeated_hex('a', 64))
            .unwrap()
            .with_labels(vec!["admin".to_string()]);
        let member = TaggedPubkey::new(repeated_hex('b', 64)).unwrap();
        let channel_create = build_channel_create_event('c', 20, "ops");
        let channel_metadata =
            build_channel_metadata_event('d', 21, &channel_create.id, "ops-renamed", 2);
        let message_one = build_message_event('e', 'a', 30, &channel_create.id, "ready");
        let message_two = build_reply_event(
            '1',
            'b',
            31,
            &channel_create.id,
            &message_one.id,
            &message_one.pubkey,
            "copy",
        );
        let reaction = build_reaction_event('2', 'b', 32, &message_one.id);

        projection.record_relay_events(vec![
            reaction.clone(),
            message_two.clone(),
            build_group_metadata_event('5', 10, "Ops"),
            build_members_event('6', 12, vec![admin.clone(), member.clone()]),
            build_roles_event('7', 13),
            channel_metadata.clone(),
            message_one.clone(),
            build_admins_event('8', 11, vec![admin]),
            channel_create.clone(),
        ]);

        assert_eq!(projection.snapshot.groups.len(), 1);
        assert_eq!(projection.snapshot.channels.len(), 1);
        let group = &projection.snapshot.groups[0];
        assert_eq!(group.group_id, "oa-main");
        assert_eq!(group.metadata.name.as_deref(), Some("Ops"));
        assert_eq!(group.members.len(), 2);
        assert!(group.members.iter().any(|member| member.is_admin));
        assert_eq!(group.channel_ids, vec![channel_create.id.clone()]);

        let channel = &projection.snapshot.channels[0];
        assert_eq!(channel.metadata.name, "ops-renamed");
        assert_eq!(
            channel.message_ids,
            vec![message_one.id.clone(), message_two.id.clone()]
        );
        assert_eq!(channel.root_message_ids, vec![message_one.id.clone()]);
        assert_eq!(
            channel.latest_message_id.as_deref(),
            Some(message_two.id.as_str())
        );

        let root_message = projection.snapshot.messages.get(&message_one.id).unwrap();
        assert_eq!(root_message.reply_child_ids, vec![message_two.id.clone()]);
        assert_eq!(root_message.reaction_summaries.len(), 1);
        assert_eq!(root_message.reaction_summaries[0].content, "+");
        assert_eq!(root_message.reaction_summaries[0].count, 1);

        projection
            .set_selected_channel("oa-main", &channel_create.id)
            .unwrap();
        projection
            .mark_channel_read(&channel_create.id, Some(&message_one.id))
            .unwrap();
        assert_eq!(projection.snapshot.channels[0].unread_count, 1);

        let reloaded = ManagedChatProjectionState::from_projection_path_for_tests(path);
        assert_eq!(reloaded.snapshot, projection.snapshot);
        assert_eq!(
            reloaded.local_state.selected_channel_id.as_deref(),
            Some(channel_create.id.as_str())
        );
    }

    #[test]
    fn managed_chat_projection_gap_recovery_matches_full_sync_snapshot() {
        let full_dir = tempdir().unwrap();
        let req_dir = tempdir().unwrap();
        let neg_dir = tempdir().unwrap();
        let full_path = full_dir.path().join("managed-full.json");
        let req_path = req_dir.path().join("managed-req.json");
        let neg_path = neg_dir.path().join("managed-neg.json");
        let mut full = ManagedChatProjectionState::from_projection_path_for_tests(full_path);
        let mut req = ManagedChatProjectionState::from_projection_path_for_tests(req_path);
        let mut neg = ManagedChatProjectionState::from_projection_path_for_tests(neg_path);

        let channel_create = build_channel_create_event('a', 20, "ops");
        let channel_metadata = build_channel_metadata_event('b', 21, &channel_create.id, "ops", 1);
        let message_one = build_message_event('c', 'a', 30, &channel_create.id, "ready");
        let message_two = build_reply_event(
            'd',
            'b',
            31,
            &channel_create.id,
            &message_one.id,
            &message_one.pubkey,
            "copy",
        );
        let reaction = build_reaction_event('e', 'c', 32, &message_one.id);
        let group = build_group_metadata_event('f', 10, "Ops");
        let all_events = vec![
            group.clone(),
            channel_create.clone(),
            channel_metadata.clone(),
            message_one.clone(),
            message_two.clone(),
            reaction.clone(),
        ];

        full.replace_relay_events(all_events);
        req.record_relay_events(vec![group.clone(), channel_create.clone()]);
        req.record_relay_events(vec![
            channel_metadata.clone(),
            message_one.clone(),
            message_two.clone(),
            reaction.clone(),
        ]);
        neg.record_relay_events(vec![group, channel_create, channel_metadata, message_one]);
        neg.record_relay_events(vec![reaction, message_two]);

        assert_eq!(req.snapshot, full.snapshot);
        assert_eq!(neg.snapshot, full.snapshot);
    }

    #[test]
    fn managed_chat_projection_delete_event_falls_back_to_previous_metadata_and_applies_roster_moderation()
     {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection = ManagedChatProjectionState::from_projection_path_for_tests(path);

        let alice = TaggedPubkey::new(repeated_hex('a', 64)).unwrap();
        let bob = TaggedPubkey::new(repeated_hex('b', 64)).unwrap();
        let channel_create = build_channel_create_event('c', 20, "ops");
        let channel_metadata_one =
            build_channel_metadata_event('d', 21, &channel_create.id, "ops-v1", 1);
        let channel_metadata_two =
            build_channel_metadata_event('e', 22, &channel_create.id, "ops-v2", 2);
        let delete_latest_metadata = build_delete_event('1', 23, &channel_metadata_two.id);
        let remove_bob = build_remove_user_event('2', 24, &repeated_hex('b', 64));
        let add_charlie = build_put_user_event(
            '3',
            25,
            &repeated_hex('c', 64),
            vec!["moderator".to_string()],
        );

        projection.replace_relay_events(vec![
            build_group_metadata_event('4', 10, "Ops"),
            build_members_event('5', 11, vec![alice, bob]),
            channel_create.clone(),
            channel_metadata_one.clone(),
            channel_metadata_two,
            delete_latest_metadata,
            remove_bob,
            add_charlie,
        ]);

        assert_eq!(projection.snapshot.channels.len(), 1);
        assert_eq!(projection.snapshot.channels[0].metadata.name, "ops-v1");
        let roster = &projection.snapshot.groups[0].members;
        assert!(
            roster
                .iter()
                .any(|member| member.pubkey == repeated_hex('a', 64))
        );
        assert!(roster.iter().any(|member| {
            member.pubkey == repeated_hex('c', 64) && member.labels == vec!["moderator".to_string()]
        }));
        assert!(
            !roster
                .iter()
                .any(|member| member.pubkey == repeated_hex('b', 64))
        );
    }

    #[test]
    fn managed_chat_projection_applies_metadata_toggle_changes_from_moderation_events() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection = ManagedChatProjectionState::from_projection_path_for_tests(path);

        let metadata_event = signed_event(
            'a',
            '1',
            10,
            39000,
            GroupMetadataEvent::new(
                "oa-main",
                GroupMetadata::new()
                    .with_name("Ops")
                    .with_private(true)
                    .with_restricted(true)
                    .with_hidden(true)
                    .with_closed(true),
                10,
            )
            .unwrap()
            .to_tags(),
            String::new(),
        );
        let edit_metadata = build_edit_metadata_event(
            'b',
            11,
            vec![
                vec!["public".to_string()],
                vec!["unrestricted".to_string()],
                vec!["visible".to_string()],
                vec!["open".to_string()],
            ],
        );
        projection.replace_relay_events(vec![metadata_event, edit_metadata]);

        let metadata = &projection.snapshot.groups[0].metadata;
        assert!(!metadata.private);
        assert!(!metadata.restricted);
        assert!(!metadata.hidden);
        assert!(!metadata.closed);
    }

    #[test]
    fn managed_chat_projection_mark_channel_read_tracks_unread_counts() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection = ManagedChatProjectionState::from_projection_path_for_tests(path);

        let channel_create = build_channel_create_event('a', 20, "ops");
        let message_one = build_message_event('b', 'a', 30, &channel_create.id, "one");
        let message_two = build_message_event('c', 'b', 31, &channel_create.id, "two");
        let message_three = build_message_event('d', 'c', 32, &channel_create.id, "three");
        projection.replace_relay_events(vec![
            build_group_metadata_event('e', 10, "Ops"),
            channel_create.clone(),
            message_three.clone(),
            message_one.clone(),
            message_two.clone(),
        ]);

        assert_eq!(projection.snapshot.channels[0].unread_count, 3);

        projection
            .mark_channel_read(&channel_create.id, Some(&message_two.id))
            .unwrap();
        assert_eq!(projection.snapshot.channels[0].unread_count, 1);
        assert_eq!(
            projection
                .local_state
                .read_cursors
                .get(&channel_create.id)
                .and_then(|cursor| cursor.last_read_event_id.as_deref()),
            Some(message_two.id.as_str())
        );

        projection
            .mark_channel_read(&channel_create.id, None)
            .unwrap();
        assert_eq!(projection.snapshot.channels[0].unread_count, 0);
        assert_eq!(
            projection
                .local_state
                .read_cursors
                .get(&channel_create.id)
                .and_then(|cursor| cursor.last_read_event_id.as_deref()),
            Some(message_three.id.as_str())
        );
    }

    #[test]
    fn managed_chat_projection_tracks_mentions_for_local_identity() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection = ManagedChatProjectionState::from_projection_path_for_tests(path);

        let channel_create = build_channel_create_event('a', 20, "ops");
        let local_pubkey = repeated_hex('f', 64);
        let message_one = build_message_with_mentions_event(
            'b',
            'a',
            30,
            &channel_create.id,
            "heads up",
            vec![ManagedChatMention::new(local_pubkey.clone()).unwrap()],
        );
        let message_two = build_message_event('c', 'b', 31, &channel_create.id, "plain");
        projection.replace_relay_events(vec![
            build_group_metadata_event('d', 10, "Ops"),
            channel_create.clone(),
            message_one.clone(),
            message_two,
        ]);

        assert_eq!(projection.snapshot.channels[0].mention_count, 0);
        projection.set_local_pubkey(Some(local_pubkey.as_str()));
        assert_eq!(projection.snapshot.channels[0].unread_count, 2);
        assert_eq!(projection.snapshot.channels[0].mention_count, 1);
        assert_eq!(projection.snapshot.groups[0].mention_count, 1);

        projection
            .mark_channel_read(&channel_create.id, Some(&message_one.id))
            .unwrap();
        assert_eq!(projection.snapshot.channels[0].unread_count, 1);
        assert_eq!(projection.snapshot.channels[0].mention_count, 0);
        assert_eq!(projection.snapshot.groups[0].mention_count, 0);
    }

    #[test]
    fn managed_chat_projection_persists_outbound_failure_retry_and_ack_state() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection =
            ManagedChatProjectionState::from_projection_path_for_tests(path.clone());

        let channel_create = build_channel_create_event('a', 20, "ops");
        projection.replace_relay_events(vec![
            build_group_metadata_event('b', 10, "Ops"),
            build_channel_metadata_event('c', 21, &channel_create.id, "ops", 1),
            channel_create.clone(),
        ]);

        let outbound_event = build_message_event('d', 'a', 30, &channel_create.id, "queued");
        projection
            .queue_outbound_message(ManagedChatOutboundMessage {
                event: outbound_event.clone(),
                group_id: "oa-main".to_string(),
                channel_id: channel_create.id.clone(),
                relay_url: "wss://relay.openagents.test".to_string(),
                delivery_state: ManagedChatDeliveryState::Publishing,
                attempt_count: 1,
                last_error: None,
            })
            .unwrap();

        let queued = projection
            .snapshot
            .messages
            .get(&outbound_event.id)
            .unwrap();
        assert_eq!(queued.delivery_state, ManagedChatDeliveryState::Publishing);
        assert_eq!(queued.attempt_count, 1);
        assert_eq!(projection.snapshot.channels[0].unread_count, 0);

        projection
            .fail_outbound_message(&outbound_event.id, "transport offline")
            .unwrap();
        let failed = projection
            .snapshot
            .messages
            .get(&outbound_event.id)
            .unwrap();
        assert_eq!(failed.delivery_state, ManagedChatDeliveryState::Failed);
        assert_eq!(failed.attempt_count, 1);
        assert_eq!(failed.delivery_error.as_deref(), Some("transport offline"));

        projection
            .retry_outbound_message(&outbound_event.id)
            .unwrap();
        let retrying = projection
            .snapshot
            .messages
            .get(&outbound_event.id)
            .unwrap();
        assert_eq!(
            retrying.delivery_state,
            ManagedChatDeliveryState::Publishing
        );
        assert_eq!(retrying.attempt_count, 2);
        assert_eq!(retrying.delivery_error, None);

        projection.ack_outbound_message(&outbound_event.id).unwrap();
        let acked = projection
            .snapshot
            .messages
            .get(&outbound_event.id)
            .unwrap();
        assert_eq!(acked.delivery_state, ManagedChatDeliveryState::Acked);
        assert_eq!(acked.attempt_count, 2);
        assert_eq!(acked.delivery_error, None);

        let reloaded = ManagedChatProjectionState::from_projection_path_for_tests(path);
        let reloaded_message = reloaded.snapshot.messages.get(&outbound_event.id).unwrap();
        assert_eq!(
            reloaded_message.delivery_state,
            ManagedChatDeliveryState::Acked
        );
        assert_eq!(reloaded_message.attempt_count, 2);
    }

    #[test]
    fn managed_chat_projection_persists_collapsed_category_state() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection =
            ManagedChatProjectionState::from_projection_path_for_tests(path.clone());

        let channel_create = build_channel_create_event('a', 20, "ops");
        projection.replace_relay_events(vec![
            build_group_metadata_event('b', 10, "Ops"),
            build_channel_metadata_event('c', 21, &channel_create.id, "ops", 1),
            channel_create,
        ]);
        assert!(!projection.category_is_collapsed("oa-main", "ops"));

        projection
            .toggle_category_collapsed("oa-main", "ops")
            .unwrap();
        assert!(projection.category_is_collapsed("oa-main", "ops"));

        let reloaded = ManagedChatProjectionState::from_projection_path_for_tests(path);
        assert!(reloaded.category_is_collapsed("oa-main", "ops"));
    }

    #[test]
    fn managed_chat_projection_persists_local_muted_members() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection =
            ManagedChatProjectionState::from_projection_path_for_tests(path.clone());
        let muted_pubkey = repeated_hex('a', 64);

        projection
            .set_pubkey_muted(&muted_pubkey, true)
            .expect("mute member");
        assert!(projection.is_pubkey_muted(&muted_pubkey));

        let reloaded = ManagedChatProjectionState::from_projection_path_for_tests(path);
        assert!(reloaded.is_pubkey_muted(&muted_pubkey));
    }

    #[test]
    fn managed_chat_projection_persists_relay_events() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection =
            ManagedChatProjectionState::from_projection_path_for_tests(path.clone());

        let channel_create = build_channel_create_event('a', 20, "ops");
        projection.replace_relay_events(vec![
            build_group_metadata_event('b', 10, "Ops"),
            build_channel_metadata_event('c', 21, &channel_create.id, "ops", 1),
            channel_create.clone(),
        ]);

        let message = build_message_event('d', 'e', 30, &channel_create.id, "hello from relay");
        projection.record_relay_event(message.clone());

        let reloaded = ManagedChatProjectionState::from_projection_path_for_tests(path);
        assert!(
            reloaded
                .relay_events
                .iter()
                .any(|event| event.id == message.id),
            "relay event not found in reloaded projection"
        );
        assert!(
            reloaded.snapshot.messages.contains_key(&message.id),
            "relay event not reflected in reloaded snapshot"
        );
    }

    #[test]
    fn managed_chat_projection_reconciles_outbound_local_echo_when_relay_echo_arrives() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("managed-chat.json");
        let mut projection = ManagedChatProjectionState::from_projection_path_for_tests(path);

        let channel_create = build_channel_create_event('a', 20, "ops");
        let outbound_event = build_message_event('b', 'a', 30, &channel_create.id, "hello");
        projection.replace_relay_events(vec![
            build_group_metadata_event('c', 10, "Ops"),
            build_channel_metadata_event('d', 21, &channel_create.id, "ops", 1),
            channel_create.clone(),
        ]);
        projection
            .queue_outbound_message(ManagedChatOutboundMessage {
                event: outbound_event.clone(),
                group_id: "oa-main".to_string(),
                channel_id: channel_create.id.clone(),
                relay_url: "wss://relay.openagents.test".to_string(),
                delivery_state: ManagedChatDeliveryState::Publishing,
                attempt_count: 1,
                last_error: None,
            })
            .unwrap();
        projection
            .fail_outbound_message(&outbound_event.id, "transport offline")
            .unwrap();
        assert_eq!(
            projection
                .snapshot
                .messages
                .get(&outbound_event.id)
                .unwrap()
                .delivery_state,
            ManagedChatDeliveryState::Failed
        );

        projection.record_relay_event(outbound_event.clone());

        let confirmed = projection
            .snapshot
            .messages
            .get(&outbound_event.id)
            .unwrap();
        assert_eq!(
            confirmed.delivery_state,
            ManagedChatDeliveryState::Confirmed
        );
        assert_eq!(
            projection.snapshot.channels[0].message_ids,
            vec![outbound_event.id.clone()]
        );
        assert_eq!(projection.snapshot.channels[0].unread_count, 1);
        assert_eq!(
            projection
                .outbound_messages
                .iter()
                .find(|message| message.event.id == outbound_event.id)
                .map(|message| message.delivery_state),
            Some(ManagedChatDeliveryState::Acked)
        );
    }

    #[test]
    fn managed_chat_helpers_handle_read_cursors_with_unknown_event_ids() {
        let message_ids = vec!["a".to_string(), "b".to_string()];
        let mut messages = std::collections::BTreeMap::new();
        messages.insert(
            "a".to_string(),
            super::ManagedChatMessageProjection {
                event_id: "a".to_string(),
                group_id: "oa-main".to_string(),
                channel_id: "channel".to_string(),
                author_pubkey: "author".to_string(),
                content: "first".to_string(),
                created_at: 10,
                reply_to_event_id: None,
                mention_pubkeys: Vec::new(),
                reaction_summaries: Vec::new(),
                reply_child_ids: Vec::new(),
                delivery_state: ManagedChatDeliveryState::Confirmed,
                delivery_error: None,
                attempt_count: 0,
                message_class: crate::chat_message_classifier::ChatMessageClass::HumanMessage,
            },
        );
        messages.insert(
            "b".to_string(),
            super::ManagedChatMessageProjection {
                event_id: "b".to_string(),
                group_id: "oa-main".to_string(),
                channel_id: "channel".to_string(),
                author_pubkey: "author".to_string(),
                content: "second".to_string(),
                created_at: 11,
                reply_to_event_id: None,
                mention_pubkeys: Vec::new(),
                reaction_summaries: Vec::new(),
                reply_child_ids: Vec::new(),
                delivery_state: ManagedChatDeliveryState::Confirmed,
                delivery_error: None,
                attempt_count: 0,
                message_class: crate::chat_message_classifier::ChatMessageClass::HumanMessage,
            },
        );
        let cursor = ManagedChatReadCursor {
            last_read_event_id: Some("missing".to_string()),
            last_read_created_at: Some(10),
        };
        assert_eq!(
            unread_count_for_channel(&message_ids, Some(&cursor), &messages),
            1
        );
        assert_eq!(
            tag_value(&[vec!["h".to_string(), "oa-main".to_string()]], "h"),
            Some("oa-main")
        );
    }
}
