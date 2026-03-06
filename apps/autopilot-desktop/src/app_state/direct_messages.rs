use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nostr::nip17::{ChatMessage, DmRelayList};
use serde::{Deserialize, Serialize};

use super::{ManagedChatDeliveryState, PaneLoadState};

const DIRECT_MESSAGE_PROJECTION_SCHEMA_VERSION: u16 = 1;
const DIRECT_MESSAGE_PROJECTION_STREAM_ID: &str = "stream.direct_message_projection.v1";
const DIRECT_MESSAGE_EVENT_LIMIT: usize = 4_096;

fn direct_message_outbound_attempt_count_default() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectMessageLocalState {
    pub selected_room_id: Option<String>,
    #[serde(default)]
    pub read_cursors: BTreeMap<String, DirectMessageReadCursor>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectMessageReadCursor {
    pub last_read_message_id: Option<String>,
    pub last_read_created_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectMessageOutboundMessage {
    pub room_id: String,
    pub message_id: String,
    pub author_pubkey: String,
    pub participant_pubkeys: Vec<String>,
    pub recipient_pubkeys: Vec<String>,
    #[serde(default)]
    pub recipient_relay_hints: BTreeMap<String, Vec<String>>,
    pub content: String,
    pub created_at: u64,
    #[serde(default)]
    pub reply_to_event_id: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub wrapped_events: Vec<nostr::Event>,
    #[serde(default)]
    pub delivery_state: ManagedChatDeliveryState,
    #[serde(default = "direct_message_outbound_attempt_count_default")]
    pub attempt_count: u32,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectMessageMessageProjection {
    pub message_id: String,
    pub room_id: String,
    pub author_pubkey: String,
    pub participant_pubkeys: Vec<String>,
    pub recipient_pubkeys: Vec<String>,
    pub content: String,
    pub created_at: u64,
    pub reply_to_event_id: Option<String>,
    pub subject: Option<String>,
    pub wrapped_event_ids: Vec<String>,
    pub delivery_state: ManagedChatDeliveryState,
    pub delivery_error: Option<String>,
    pub attempt_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectMessageRoomProjection {
    pub room_id: String,
    pub participant_pubkeys: Vec<String>,
    pub other_pubkeys: Vec<String>,
    pub subject: Option<String>,
    pub message_ids: Vec<String>,
    pub latest_message_id: Option<String>,
    pub unread_count: usize,
    pub mention_count: usize,
    pub relay_hints: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectMessageProjectionSnapshot {
    pub rooms: Vec<DirectMessageRoomProjection>,
    pub messages: BTreeMap<String, DirectMessageMessageProjection>,
    pub relay_lists: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DirectMessageProjectionDocumentV1 {
    schema_version: u16,
    stream_id: String,
    #[serde(default)]
    relay_events: Vec<nostr::Event>,
    #[serde(default)]
    outbound_messages: Vec<DirectMessageOutboundMessage>,
    #[serde(default)]
    local_state: DirectMessageLocalState,
}

pub struct DirectMessageProjectionState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub stream_id: String,
    pub relay_events: Vec<nostr::Event>,
    pub outbound_messages: Vec<DirectMessageOutboundMessage>,
    pub local_state: DirectMessageLocalState,
    pub snapshot: DirectMessageProjectionSnapshot,
    local_pubkey: Option<String>,
    local_private_key_hex: Option<String>,
    projection_file_path: PathBuf,
}

impl Default for DirectMessageProjectionState {
    fn default() -> Self {
        Self::from_projection_file_path(direct_message_projection_file_path())
    }
}

impl DirectMessageProjectionState {
    fn from_projection_file_path(projection_file_path: PathBuf) -> Self {
        match load_direct_message_projection_document(projection_file_path.as_path()) {
            Ok((relay_events, mut outbound_messages, local_state)) => {
                reconcile_outbound_messages_against_relays(&mut outbound_messages, &relay_events);
                let snapshot = rebuild_direct_message_projection(
                    &relay_events,
                    &outbound_messages,
                    &local_state,
                    None,
                    None,
                );
                Self {
                    load_state: PaneLoadState::Ready,
                    last_error: None,
                    last_action: Some(format!(
                        "Loaded direct message projection stream ({} relay events / {} outbound)",
                        relay_events.len(),
                        outbound_messages.len()
                    )),
                    stream_id: DIRECT_MESSAGE_PROJECTION_STREAM_ID.to_string(),
                    relay_events,
                    outbound_messages,
                    local_state,
                    snapshot,
                    local_pubkey: None,
                    local_private_key_hex: None,
                    projection_file_path,
                }
            }
            Err(error) => Self {
                load_state: PaneLoadState::Error,
                last_error: Some(error),
                last_action: Some("Direct message projection stream load failed".to_string()),
                stream_id: DIRECT_MESSAGE_PROJECTION_STREAM_ID.to_string(),
                relay_events: Vec::new(),
                outbound_messages: Vec::new(),
                local_state: DirectMessageLocalState::default(),
                snapshot: DirectMessageProjectionSnapshot::default(),
                local_pubkey: None,
                local_private_key_hex: None,
                projection_file_path,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn from_projection_path_for_tests(projection_file_path: PathBuf) -> Self {
        Self::from_projection_file_path(projection_file_path)
    }

    pub fn set_identity(&mut self, identity: Option<&nostr::NostrIdentity>) {
        self.local_pubkey = identity.map(|identity| identity.public_key_hex.to_ascii_lowercase());
        self.local_private_key_hex =
            identity.map(|identity| identity.private_key_hex.trim().to_string());
        self.refresh_projection("Configured direct message identity");
    }

    pub fn local_pubkey(&self) -> Option<&str> {
        self.local_pubkey.as_deref()
    }

    pub fn record_relay_event(&mut self, event: nostr::Event) {
        self.relay_events.push(event);
        self.refresh_projection("Projected direct message event");
    }

    pub fn record_relay_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = nostr::Event>,
    {
        self.relay_events.extend(events);
        self.refresh_projection("Projected direct message relay sync");
    }

    pub fn replace_relay_events(&mut self, relay_events: Vec<nostr::Event>) {
        self.relay_events = relay_events;
        self.refresh_projection("Rebuilt direct message projection");
    }

    pub fn queue_outbound_message(
        &mut self,
        mut outbound_message: DirectMessageOutboundMessage,
    ) -> Result<(), String> {
        if outbound_message.room_id.trim().is_empty() {
            return Err("Direct message outbound room id is missing.".to_string());
        }
        if outbound_message.message_id.trim().is_empty() {
            return Err("Direct message outbound logical message id is missing.".to_string());
        }
        outbound_message.author_pubkey =
            normalize_direct_message_pubkey(outbound_message.author_pubkey.as_str())?;
        outbound_message.participant_pubkeys =
            normalize_direct_message_pubkeys(outbound_message.participant_pubkeys);
        outbound_message.recipient_pubkeys =
            normalize_direct_message_pubkeys(outbound_message.recipient_pubkeys);
        if outbound_message.participant_pubkeys.is_empty() {
            return Err("Direct message outbound participants are missing.".to_string());
        }
        if !outbound_message
            .participant_pubkeys
            .iter()
            .any(|pubkey| pubkey == &outbound_message.author_pubkey)
        {
            return Err(
                "Direct message outbound author is not in the participant list.".to_string(),
            );
        }
        if outbound_message.recipient_pubkeys.is_empty() {
            return Err("Direct message outbound recipients are missing.".to_string());
        }
        outbound_message.recipient_relay_hints = outbound_message
            .recipient_relay_hints
            .into_iter()
            .map(|(pubkey, relays)| {
                (
                    normalize_direct_message_pubkey(pubkey.as_str()).unwrap_or(pubkey),
                    normalize_relay_urls(relays),
                )
            })
            .collect();
        outbound_message.subject = normalize_optional_subject(outbound_message.subject.as_deref());
        outbound_message.attempt_count = outbound_message.attempt_count.max(1);
        self.outbound_messages
            .retain(|candidate| candidate.message_id != outbound_message.message_id);
        let message_id = outbound_message.message_id.clone();
        self.outbound_messages.push(outbound_message);
        self.refresh_projection(format!("Queued direct message local echo {message_id}"));
        Ok(())
    }

    pub fn fail_outbound_message(
        &mut self,
        message_id: &str,
        error: impl Into<String>,
    ) -> Result<(), String> {
        let Some(index) = self
            .outbound_messages
            .iter()
            .position(|message| message.message_id == message_id)
        else {
            return Err(format!(
                "Unknown direct message outbound message: {message_id}"
            ));
        };
        self.outbound_messages[index].delivery_state = ManagedChatDeliveryState::Failed;
        self.outbound_messages[index].attempt_count =
            self.outbound_messages[index].attempt_count.max(1);
        self.outbound_messages[index].last_error = Some(error.into());
        self.refresh_projection(format!(
            "Marked direct message outbound {message_id} failed"
        ));
        Ok(())
    }

    pub fn ack_outbound_message(&mut self, message_id: &str) -> Result<(), String> {
        let Some(index) = self
            .outbound_messages
            .iter()
            .position(|message| message.message_id == message_id)
        else {
            return Err(format!(
                "Unknown direct message outbound message: {message_id}"
            ));
        };
        self.outbound_messages[index].delivery_state = ManagedChatDeliveryState::Acked;
        self.outbound_messages[index].attempt_count =
            self.outbound_messages[index].attempt_count.max(1);
        self.outbound_messages[index].last_error = None;
        self.refresh_projection(format!("Acknowledged direct message outbound {message_id}"));
        Ok(())
    }

    pub fn retry_outbound_message(&mut self, message_id: &str) -> Result<(), String> {
        let Some(index) = self
            .outbound_messages
            .iter()
            .position(|message| message.message_id == message_id)
        else {
            return Err(format!(
                "Unknown direct message outbound message: {message_id}"
            ));
        };
        self.outbound_messages[index].delivery_state = ManagedChatDeliveryState::Publishing;
        self.outbound_messages[index].attempt_count = self.outbound_messages[index]
            .attempt_count
            .saturating_add(1)
            .max(1);
        self.outbound_messages[index].last_error = None;
        self.refresh_projection(format!("Retried direct message outbound {message_id}"));
        Ok(())
    }

    pub fn set_selected_room(&mut self, room_id: &str) -> Result<(), String> {
        if !self
            .snapshot
            .rooms
            .iter()
            .any(|room| room.room_id == room_id)
        {
            return Err(format!("Unknown direct message room: {room_id}"));
        }
        self.local_state.selected_room_id = Some(room_id.to_string());
        self.mark_room_read(room_id, None)?;
        self.last_action = Some(format!("Selected direct message room {room_id}"));
        Ok(())
    }

    pub fn mark_room_read(
        &mut self,
        room_id: &str,
        last_read_message_id: Option<&str>,
    ) -> Result<(), String> {
        let Some(room) = self
            .snapshot
            .rooms
            .iter()
            .find(|room| room.room_id == room_id)
        else {
            return Err(format!("Unknown direct message room: {room_id}"));
        };

        let cursor = match last_read_message_id {
            Some(message_id) => {
                let Some(message) = self.snapshot.messages.get(message_id) else {
                    return Err(format!(
                        "Unknown direct message for read cursor: {message_id}"
                    ));
                };
                if message.room_id != room_id {
                    return Err(format!(
                        "Direct message {message_id} does not belong to room {room_id}"
                    ));
                }
                DirectMessageReadCursor {
                    last_read_message_id: Some(message.message_id.clone()),
                    last_read_created_at: Some(message.created_at),
                }
            }
            None => room
                .latest_message_id
                .as_ref()
                .and_then(|message_id| self.snapshot.messages.get(message_id))
                .map(|message| DirectMessageReadCursor {
                    last_read_message_id: Some(message.message_id.clone()),
                    last_read_created_at: Some(message.created_at),
                })
                .unwrap_or_default(),
        };

        if cursor.last_read_message_id.is_some() {
            self.local_state
                .read_cursors
                .insert(room_id.to_string(), cursor);
        } else {
            self.local_state.read_cursors.remove(room_id);
        }
        self.refresh_projection(format!("Updated read cursor for direct message room {room_id}"));
        Ok(())
    }

    pub fn reload_projection(&mut self) -> Result<(), String> {
        let (relay_events, mut outbound_messages, local_state) =
            load_direct_message_projection_document(self.projection_file_path.as_path())?;
        self.relay_events = relay_events;
        reconcile_outbound_messages_against_relays(&mut outbound_messages, &self.relay_events);
        self.outbound_messages = outbound_messages;
        self.local_state = local_state;
        self.snapshot = rebuild_direct_message_projection(
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
            self.local_pubkey.as_deref(),
            self.local_private_key_hex.as_deref(),
        );
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!(
            "Direct message projection reloaded ({} relay events / {} outbound)",
            self.relay_events.len(),
            self.outbound_messages.len()
        ));
        Ok(())
    }

    fn refresh_projection(&mut self, action: impl Into<String>) {
        self.relay_events =
            normalize_direct_message_relay_events(std::mem::take(&mut self.relay_events));
        self.outbound_messages =
            normalize_direct_message_outbound_messages(std::mem::take(&mut self.outbound_messages));
        reconcile_outbound_messages_against_relays(&mut self.outbound_messages, &self.relay_events);
        self.snapshot = rebuild_direct_message_projection(
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
            self.local_pubkey.as_deref(),
            self.local_private_key_hex.as_deref(),
        );
        let action = format!(
            "{} ({} relay events / {} outbound / {} rooms)",
            action.into(),
            self.relay_events.len(),
            self.outbound_messages.len(),
            self.snapshot.rooms.len()
        );
        if let Err(error) = persist_direct_message_projection_document(
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
        persist_direct_message_projection_document(
            self.projection_file_path.as_path(),
            &self.relay_events,
            &self.outbound_messages,
            &self.local_state,
        )?;
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(action);
        Ok(())
    }
}

pub fn direct_message_room_id(subject: Option<&str>, participant_pubkeys: &[String]) -> String {
    let participants = normalize_direct_message_pubkeys(participant_pubkeys.to_vec());
    let subject = normalize_room_subject(subject);
    if subject.is_empty() {
        format!("dm:{}", participants.join(":"))
    } else {
        format!("room:{subject}:{}", participants.join(":"))
    }
}

fn direct_message_projection_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-direct-message-projection-v1.json")
}

fn compare_direct_message_timeline_events(left: &nostr::Event, right: &nostr::Event) -> Ordering {
    left.created_at
        .cmp(&right.created_at)
        .then_with(|| left.id.cmp(&right.id))
}

fn normalize_direct_message_relay_events(mut relay_events: Vec<nostr::Event>) -> Vec<nostr::Event> {
    relay_events.sort_by(compare_direct_message_timeline_events);
    let mut seen_event_ids = std::collections::BTreeSet::new();
    relay_events.retain(|event| seen_event_ids.insert(event.id.clone()));
    if relay_events.len() > DIRECT_MESSAGE_EVENT_LIMIT {
        relay_events = relay_events.split_off(relay_events.len() - DIRECT_MESSAGE_EVENT_LIMIT);
    }
    relay_events
}

fn normalize_direct_message_outbound_messages(
    outbound_messages: Vec<DirectMessageOutboundMessage>,
) -> Vec<DirectMessageOutboundMessage> {
    let mut deduped = BTreeMap::<String, DirectMessageOutboundMessage>::new();
    for mut outbound_message in outbound_messages {
        outbound_message.author_pubkey =
            normalize_direct_message_pubkey(outbound_message.author_pubkey.as_str())
                .unwrap_or(outbound_message.author_pubkey);
        outbound_message.participant_pubkeys =
            normalize_direct_message_pubkeys(outbound_message.participant_pubkeys);
        outbound_message.recipient_pubkeys =
            normalize_direct_message_pubkeys(outbound_message.recipient_pubkeys);
        outbound_message.recipient_relay_hints = outbound_message
            .recipient_relay_hints
            .into_iter()
            .map(|(pubkey, relays)| {
                (
                    normalize_direct_message_pubkey(pubkey.as_str()).unwrap_or(pubkey),
                    normalize_relay_urls(relays),
                )
            })
            .collect();
        outbound_message.subject = normalize_optional_subject(outbound_message.subject.as_deref());
        outbound_message.attempt_count = outbound_message.attempt_count.max(1);
        deduped.insert(outbound_message.message_id.clone(), outbound_message);
    }
    let mut normalized = deduped.into_values().collect::<Vec<_>>();
    normalized.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.message_id.cmp(&right.message_id))
    });
    normalized
}

fn reconcile_outbound_messages_against_relays(
    outbound_messages: &mut [DirectMessageOutboundMessage],
    relay_events: &[nostr::Event],
) {
    let relay_event_ids = relay_events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for outbound_message in outbound_messages {
        if outbound_message
            .wrapped_events
            .iter()
            .any(|event| relay_event_ids.contains(event.id.as_str()))
        {
            outbound_message.delivery_state = ManagedChatDeliveryState::Acked;
            outbound_message.last_error = None;
            outbound_message.attempt_count = outbound_message.attempt_count.max(1);
        }
    }
}

fn persist_direct_message_projection_document(
    path: &Path,
    relay_events: &[nostr::Event],
    outbound_messages: &[DirectMessageOutboundMessage],
    local_state: &DirectMessageLocalState,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create direct message projection dir: {error}"))?;
    }
    let document = DirectMessageProjectionDocumentV1 {
        schema_version: DIRECT_MESSAGE_PROJECTION_SCHEMA_VERSION,
        stream_id: DIRECT_MESSAGE_PROJECTION_STREAM_ID.to_string(),
        relay_events: normalize_direct_message_relay_events(relay_events.to_vec()),
        outbound_messages: normalize_direct_message_outbound_messages(outbound_messages.to_vec()),
        local_state: local_state.clone(),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode direct message projection: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write direct message projection temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist direct message projection: {error}"))?;
    Ok(())
}

fn load_direct_message_projection_document(
    path: &Path,
) -> Result<
    (
        Vec<nostr::Event>,
        Vec<DirectMessageOutboundMessage>,
        DirectMessageLocalState,
    ),
    String,
> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new(), DirectMessageLocalState::default()));
        }
        Err(error) => {
            return Err(format!("Failed to read direct message projection: {error}"));
        }
    };

    let document = serde_json::from_str::<DirectMessageProjectionDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse direct message projection: {error}"))?;
    if document.schema_version != DIRECT_MESSAGE_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported direct message projection schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != DIRECT_MESSAGE_PROJECTION_STREAM_ID {
        return Err(format!(
            "Unsupported direct message projection stream id: {}",
            document.stream_id
        ));
    }
    Ok((
        normalize_direct_message_relay_events(document.relay_events),
        normalize_direct_message_outbound_messages(document.outbound_messages),
        document.local_state,
    ))
}

fn rebuild_direct_message_projection(
    relay_events: &[nostr::Event],
    outbound_messages: &[DirectMessageOutboundMessage],
    local_state: &DirectMessageLocalState,
    local_pubkey: Option<&str>,
    local_private_key_hex: Option<&str>,
) -> DirectMessageProjectionSnapshot {
    let mut relay_lists = BTreeMap::<String, Vec<String>>::new();
    for event in relay_events {
        if event.kind != nostr::nip17::KIND_DM_RELAY_LIST {
            continue;
        }
        if let Ok(parsed) = DmRelayList::from_event(event) {
            relay_lists.insert(
                event.pubkey.to_ascii_lowercase(),
                normalize_relay_urls(parsed.relays),
            );
        }
    }

    let mut messages = BTreeMap::<String, DirectMessageMessageProjection>::new();
    if let Some(private_key_hex) = local_private_key_hex
        && let Ok(private_key) = parse_private_key_hex(private_key_hex)
    {
        for event in relay_events
            .iter()
            .filter(|event| event.kind == nostr::nip59::KIND_GIFT_WRAP)
        {
            let Ok(rumor) = nostr::nip59::unwrap_gift_wrap_full(event, &private_key) else {
                continue;
            };
            let Ok(chat_message) = ChatMessage::from_rumor(&rumor) else {
                continue;
            };
            let participant_pubkeys = direct_message_participant_pubkeys(
                &rumor.pubkey,
                &chat_message.recipients,
                local_pubkey,
            );
            let subject = normalize_optional_subject(chat_message.subject.as_deref());
            let room_id = direct_message_room_id(subject.as_deref(), &participant_pubkeys);
            let wrapped_event_ids = vec![event.id.clone()];
            messages
                .entry(rumor.id.clone())
                .and_modify(|message| {
                    merge_wrapped_event_ids(&mut message.wrapped_event_ids, &wrapped_event_ids)
                })
                .or_insert_with(|| DirectMessageMessageProjection {
                    message_id: rumor.id.clone(),
                    room_id,
                    author_pubkey: rumor.pubkey.to_ascii_lowercase(),
                    participant_pubkeys,
                    recipient_pubkeys: normalize_direct_message_pubkeys(chat_message.recipients),
                    content: chat_message.content,
                    created_at: rumor.created_at,
                    reply_to_event_id: chat_message
                        .reply_to
                        .map(|value| value.to_ascii_lowercase()),
                    subject,
                    wrapped_event_ids,
                    delivery_state: ManagedChatDeliveryState::Confirmed,
                    delivery_error: None,
                    attempt_count: 1,
                });
        }
    }

    for outbound_message in outbound_messages {
        let wrapped_event_ids = outbound_message
            .wrapped_events
            .iter()
            .map(|event| event.id.clone())
            .collect::<Vec<_>>();
        let message = DirectMessageMessageProjection {
            message_id: outbound_message.message_id.clone(),
            room_id: outbound_message.room_id.clone(),
            author_pubkey: outbound_message.author_pubkey.clone(),
            participant_pubkeys: outbound_message.participant_pubkeys.clone(),
            recipient_pubkeys: outbound_message.recipient_pubkeys.clone(),
            content: outbound_message.content.clone(),
            created_at: outbound_message.created_at,
            reply_to_event_id: outbound_message.reply_to_event_id.clone(),
            subject: outbound_message.subject.clone(),
            wrapped_event_ids,
            delivery_state: outbound_message.delivery_state,
            delivery_error: outbound_message.last_error.clone(),
            attempt_count: outbound_message.attempt_count.max(1),
        };
        match messages.get_mut(outbound_message.message_id.as_str()) {
            Some(existing) => {
                merge_wrapped_event_ids(
                    &mut existing.wrapped_event_ids,
                    &message.wrapped_event_ids,
                );
                if existing.delivery_state != ManagedChatDeliveryState::Confirmed {
                    existing.delivery_state = message.delivery_state;
                    existing.delivery_error = message.delivery_error;
                    existing.attempt_count = message.attempt_count;
                }
            }
            None => {
                messages.insert(outbound_message.message_id.clone(), message);
            }
        }
    }

    let mut sorted_messages = messages.values().cloned().collect::<Vec<_>>();
    sorted_messages.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.message_id.cmp(&right.message_id))
    });

    let local_pubkey = local_pubkey.map(str::to_ascii_lowercase);
    let mut rooms = BTreeMap::<String, DirectMessageRoomProjection>::new();
    for message in &sorted_messages {
        let room =
            rooms
                .entry(message.room_id.clone())
                .or_insert_with(|| DirectMessageRoomProjection {
                    room_id: message.room_id.clone(),
                    participant_pubkeys: message.participant_pubkeys.clone(),
                    other_pubkeys: message
                        .participant_pubkeys
                        .iter()
                        .filter(|pubkey| Some(pubkey.as_str()) != local_pubkey.as_deref())
                        .cloned()
                        .collect(),
                    subject: message.subject.clone(),
                    message_ids: Vec::new(),
                    latest_message_id: None,
                    unread_count: 0,
                    mention_count: 0,
                    relay_hints: BTreeMap::new(),
                });
        room.message_ids.push(message.message_id.clone());
        room.latest_message_id = Some(message.message_id.clone());
        if room.subject.is_none() {
            room.subject = message.subject.clone();
        }
        for participant_pubkey in &message.participant_pubkeys {
            if let Some(relays) = relay_lists.get(participant_pubkey.as_str()) {
                merge_relay_hints(&mut room.relay_hints, participant_pubkey, relays.clone());
            }
        }
        if let Some(outbound) = outbound_messages
            .iter()
            .find(|outbound| outbound.message_id == message.message_id)
        {
            for (pubkey, relays) in &outbound.recipient_relay_hints {
                merge_relay_hints(&mut room.relay_hints, pubkey, relays.clone());
            }
        }
    }

    for room in rooms.values_mut() {
        let cursor = local_state.read_cursors.get(room.room_id.as_str());
        room.unread_count = unread_count_for_room(&room.message_ids, cursor, &messages);
        room.mention_count =
            mention_count_for_room(room, cursor, &messages, local_pubkey.as_deref());
    }

    let mut rooms = rooms.into_values().collect::<Vec<_>>();
    rooms.sort_by(|left, right| {
        let left_key = left
            .latest_message_id
            .as_deref()
            .and_then(|message_id| messages.get(message_id))
            .map(|message| (message.created_at, message.message_id.as_str()));
        let right_key = right
            .latest_message_id
            .as_deref()
            .and_then(|message_id| messages.get(message_id))
            .map(|message| (message.created_at, message.message_id.as_str()));
        right_key
            .cmp(&left_key)
            .then_with(|| left.room_id.cmp(&right.room_id))
    });

    DirectMessageProjectionSnapshot {
        rooms,
        messages,
        relay_lists,
    }
}

fn unread_count_for_room(
    message_ids: &[String],
    cursor: Option<&DirectMessageReadCursor>,
    messages: &BTreeMap<String, DirectMessageMessageProjection>,
) -> usize {
    unread_messages_for_room(message_ids, cursor, messages).len()
}

fn mention_count_for_room(
    room: &DirectMessageRoomProjection,
    cursor: Option<&DirectMessageReadCursor>,
    messages: &BTreeMap<String, DirectMessageMessageProjection>,
    local_pubkey: Option<&str>,
) -> usize {
    let Some(local_pubkey) = local_pubkey else {
        return 0;
    };
    unread_messages_for_room(&room.message_ids, cursor, messages)
        .into_iter()
        .filter(|message| direct_message_is_priority_ping(message, room, messages, local_pubkey))
        .count()
}

fn unread_messages_for_room<'a>(
    message_ids: &'a [String],
    cursor: Option<&DirectMessageReadCursor>,
    messages: &'a BTreeMap<String, DirectMessageMessageProjection>,
) -> Vec<&'a DirectMessageMessageProjection> {
    message_ids
        .iter()
        .filter_map(|message_id| messages.get(message_id.as_str()))
        .filter(|message| message.delivery_state == ManagedChatDeliveryState::Confirmed)
        .filter(|message| direct_message_is_after_cursor(message, cursor))
        .collect()
}

fn direct_message_is_after_cursor(
    message: &DirectMessageMessageProjection,
    cursor: Option<&DirectMessageReadCursor>,
) -> bool {
    let Some(cursor) = cursor else {
        return true;
    };
    let read_created_at = cursor.last_read_created_at.unwrap_or(0);
    let read_message_id = cursor.last_read_message_id.as_deref().unwrap_or("");
    message.created_at > read_created_at
        || (message.created_at == read_created_at
            && message.message_id.as_str() > read_message_id)
}

fn direct_message_is_priority_ping(
    message: &DirectMessageMessageProjection,
    room: &DirectMessageRoomProjection,
    messages: &BTreeMap<String, DirectMessageMessageProjection>,
    local_pubkey: &str,
) -> bool {
    if message.author_pubkey == local_pubkey {
        return false;
    }
    if room.participant_pubkeys.len() <= 2 {
        return true;
    }
    message
        .reply_to_event_id
        .as_deref()
        .and_then(|message_id| messages.get(message_id))
        .is_some_and(|parent| parent.author_pubkey == local_pubkey)
}

fn merge_wrapped_event_ids(target: &mut Vec<String>, incoming: &[String]) {
    for wrapped_event_id in incoming {
        if !target.iter().any(|existing| existing == wrapped_event_id) {
            target.push(wrapped_event_id.clone());
        }
    }
}

fn direct_message_participant_pubkeys(
    author_pubkey: &str,
    recipients: &[String],
    local_pubkey: Option<&str>,
) -> Vec<String> {
    let mut participants = recipients.to_vec();
    participants.push(author_pubkey.to_string());
    if let Some(local_pubkey) = local_pubkey {
        participants.push(local_pubkey.to_string());
    }
    normalize_direct_message_pubkeys(participants)
}

fn merge_relay_hints(
    relay_hints: &mut BTreeMap<String, Vec<String>>,
    pubkey: &str,
    relays: Vec<String>,
) {
    let key = pubkey.to_ascii_lowercase();
    let mut merged = relay_hints.get(key.as_str()).cloned().unwrap_or_default();
    merged.extend(relays);
    relay_hints.insert(key, normalize_relay_urls(merged));
}

fn normalize_direct_message_pubkeys(pubkeys: Vec<String>) -> Vec<String> {
    let mut normalized = pubkeys
        .into_iter()
        .filter_map(|pubkey| normalize_direct_message_pubkey(pubkey.as_str()).ok())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_direct_message_pubkey(pubkey: &str) -> Result<String, String> {
    let normalized = pubkey.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("Invalid Nostr public key hex: {pubkey}"));
    }
    Ok(normalized)
}

fn normalize_room_subject(subject: Option<&str>) -> String {
    subject
        .map(|value| {
            value
                .trim()
                .chars()
                .filter_map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        Some(ch.to_ascii_lowercase())
                    } else if ch == '-' || ch == '_' {
                        Some(ch)
                    } else if ch.is_ascii_whitespace() {
                        Some('-')
                    } else {
                        None
                    }
                })
                .collect::<String>()
                .split('-')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        })
        .unwrap_or_default()
}

fn normalize_optional_subject(subject: Option<&str>) -> Option<String> {
    subject
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
        .map(ToString::to_string)
}

fn normalize_relay_urls(relays: Vec<String>) -> Vec<String> {
    let mut normalized = relays
        .into_iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid identity private_key_hex: {error}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "invalid identity private_key_hex length {}, expected 32 bytes",
            key_bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{DirectMessageProjectionState, ManagedChatDeliveryState, direct_message_room_id};
    use tempfile::tempdir;

    fn fixture_identity() -> nostr::NostrIdentity {
        let private_key = [7u8; 32];
        let public_key_hex = nostr::get_public_key_hex(&private_key).expect("fixture pubkey");
        nostr::NostrIdentity {
            identity_path: std::path::PathBuf::from("/tmp/test-identity.mnemonic"),
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            nsec: "nsec-test".to_string(),
            npub: "npub1directprojectionfixture".to_string(),
            public_key_hex,
            private_key_hex: hex::encode(private_key),
        }
    }

    #[test]
    fn direct_message_projection_decrypts_dm_room_and_side_room() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("direct-messages.json");
        let mut projection = DirectMessageProjectionState::from_projection_path_for_tests(path);
        let identity = fixture_identity();
        projection.set_identity(Some(&identity));

        let sender_one_secret = [3u8; 32];
        let sender_two_secret = [4u8; 32];
        let sender_one_pubkey =
            nostr::get_public_key_hex(&sender_one_secret).expect("sender one pubkey");
        let sender_two_pubkey =
            nostr::get_public_key_hex(&sender_two_secret).expect("sender two pubkey");

        let dm_message = nostr::nip17::ChatMessage::new("hello there").add_recipient(
            identity.public_key_hex.clone(),
            Some("wss://relay.dm".to_string()),
        );
        let side_room_message = nostr::nip17::ChatMessage::new("draft posted")
            .add_recipient(
                identity.public_key_hex.clone(),
                Some("wss://relay.side".to_string()),
            )
            .add_recipient(
                sender_one_pubkey.clone(),
                Some("wss://relay.side".to_string()),
            )
            .subject("Design Review");

        let dm_wrap = nostr::nip17::send_chat_message(
            &dm_message,
            &sender_one_secret,
            &identity.public_key_hex,
            101,
        )
        .expect("dm wrap");
        let side_room_wrap = nostr::nip17::send_chat_message(
            &side_room_message,
            &sender_two_secret,
            &identity.public_key_hex,
            202,
        )
        .expect("side room wrap");

        let relay_list = nostr::nip17::DmRelayList::new()
            .add_relay("wss://relay.dm")
            .add_relay("wss://relay.dm-backup");
        let relay_list_event = nostr::Event {
            id: "ab".repeat(32),
            pubkey: sender_one_pubkey.clone(),
            created_at: 90,
            kind: nostr::nip17::KIND_DM_RELAY_LIST,
            tags: relay_list.to_unsigned_event(&sender_one_pubkey, 90).tags,
            content: String::new(),
            sig: "cd".repeat(64),
        };

        projection.record_relay_events(vec![relay_list_event, dm_wrap, side_room_wrap]);

        assert_eq!(projection.snapshot.rooms.len(), 2);
        let dm_room_id = direct_message_room_id(
            None,
            &[identity.public_key_hex.clone(), sender_one_pubkey.clone()],
        );
        let side_room_id = direct_message_room_id(
            Some("Design Review"),
            &[
                identity.public_key_hex.clone(),
                sender_one_pubkey.clone(),
                sender_two_pubkey.clone(),
            ],
        );
        assert!(
            projection
                .snapshot
                .rooms
                .iter()
                .any(|room| room.room_id == dm_room_id && room.subject.is_none())
        );
        assert!(
            projection
                .snapshot
                .rooms
                .iter()
                .any(|room| room.room_id == side_room_id
                    && room.subject.as_deref() == Some("Design Review"))
        );
        assert_eq!(
            projection
                .snapshot
                .relay_lists
                .get(sender_one_pubkey.as_str())
                .cloned(),
            Some(vec![
                "wss://relay.dm".to_string(),
                "wss://relay.dm-backup".to_string()
            ])
        );
    }

    #[test]
    fn direct_message_projection_persists_outbound_retry_state() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("direct-messages.json");
        let mut projection =
            DirectMessageProjectionState::from_projection_path_for_tests(path.clone());
        let identity = fixture_identity();
        projection.set_identity(Some(&identity));

        let room_id =
            direct_message_room_id(None, &[identity.public_key_hex.clone(), "33".repeat(32)]);
        projection
            .queue_outbound_message(super::DirectMessageOutboundMessage {
                room_id: room_id.clone(),
                message_id: "44".repeat(32),
                author_pubkey: identity.public_key_hex.clone(),
                participant_pubkeys: vec![identity.public_key_hex.clone(), "33".repeat(32)],
                recipient_pubkeys: vec!["33".repeat(32)],
                recipient_relay_hints: BTreeMap::from([(
                    "33".repeat(32),
                    vec!["wss://relay.example".to_string()],
                )]),
                content: "hi".to_string(),
                created_at: 55,
                reply_to_event_id: None,
                subject: None,
                wrapped_events: Vec::new(),
                delivery_state: ManagedChatDeliveryState::Publishing,
                attempt_count: 1,
                last_error: None,
            })
            .expect("queue outbound");
        projection
            .fail_outbound_message("44".repeat(32).as_str(), "transport unwired")
            .expect("fail outbound");
        projection
            .set_selected_room(&room_id)
            .expect("select direct room");

        let reloaded = DirectMessageProjectionState::from_projection_path_for_tests(path);
        let outbound = reloaded
            .outbound_messages
            .iter()
            .find(|message| message.message_id == "44".repeat(32))
            .expect("reloaded outbound");
        assert_eq!(outbound.delivery_state, ManagedChatDeliveryState::Failed);
        assert_eq!(outbound.last_error.as_deref(), Some("transport unwired"));
        assert_eq!(
            reloaded.local_state.selected_room_id.as_deref(),
            Some(room_id.as_str())
        );
    }

    #[test]
    fn direct_message_projection_mark_room_read_tracks_unread_counts() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("direct-messages.json");
        let mut projection =
            DirectMessageProjectionState::from_projection_path_for_tests(path.clone());
        let identity = fixture_identity();
        projection.set_identity(Some(&identity));

        let sender_secret = [8u8; 32];
        let sender_pubkey = nostr::get_public_key_hex(&sender_secret).expect("sender pubkey");
        let message_one = nostr::nip17::ChatMessage::new("first ping")
            .add_recipient(identity.public_key_hex.clone(), None);
        let message_two = nostr::nip17::ChatMessage::new("second ping")
            .add_recipient(identity.public_key_hex.clone(), None);
        let wrap_one = nostr::nip17::send_chat_message(
            &message_one,
            &sender_secret,
            &identity.public_key_hex,
            101,
        )
        .expect("wrap one");
        let wrap_two = nostr::nip17::send_chat_message(
            &message_two,
            &sender_secret,
            &identity.public_key_hex,
            102,
        )
        .expect("wrap two");

        projection.record_relay_events(vec![wrap_one.clone(), wrap_two.clone()]);

        let room_id = direct_message_room_id(
            None,
            &[identity.public_key_hex.clone(), sender_pubkey.clone()],
        );
        let room = projection
            .snapshot
            .rooms
            .iter()
            .find(|room| room.room_id == room_id)
            .expect("room");
        assert_eq!(room.unread_count, 2);
        assert_eq!(room.mention_count, 2);

        let first_message_id = projection
            .snapshot
            .messages
            .values()
            .find(|message| message.content == "first ping")
            .map(|message| message.message_id.clone())
            .expect("first message id");
        projection
            .mark_room_read(&room_id, Some(first_message_id.as_str()))
            .expect("mark first read");

        let room = projection
            .snapshot
            .rooms
            .iter()
            .find(|room| room.room_id == room_id)
            .expect("room after partial read");
        assert_eq!(room.unread_count, 1);
        assert_eq!(room.mention_count, 1);

        let reloaded = {
            let mut state = DirectMessageProjectionState::from_projection_path_for_tests(path);
            state.set_identity(Some(&identity));
            state
        };
        let room = reloaded
            .snapshot
            .rooms
            .iter()
            .find(|room| room.room_id == room_id)
            .expect("reloaded room");
        assert_eq!(room.unread_count, 1);
        assert_eq!(room.mention_count, 1);
        assert_eq!(
            reloaded
                .local_state
                .read_cursors
                .get(&room_id)
                .and_then(|cursor| cursor.last_read_message_id.as_deref()),
            Some(first_message_id.as_str())
        );
    }

    #[test]
    fn direct_message_projection_counts_side_room_mentions_for_replies_to_local_messages() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("direct-messages.json");
        let mut projection = DirectMessageProjectionState::from_projection_path_for_tests(path);
        let identity = fixture_identity();
        projection.set_identity(Some(&identity));

        let sender_secret = [9u8; 32];
        let sender_pubkey = nostr::get_public_key_hex(&sender_secret).expect("sender pubkey");
        let room_id = direct_message_room_id(
            Some("War Room"),
            &[identity.public_key_hex.clone(), sender_pubkey.clone()],
        );
        let local_message_id = "55".repeat(32);
        projection
            .queue_outbound_message(super::DirectMessageOutboundMessage {
                room_id: room_id.clone(),
                message_id: local_message_id.clone(),
                author_pubkey: identity.public_key_hex.clone(),
                participant_pubkeys: vec![identity.public_key_hex.clone(), sender_pubkey.clone()],
                recipient_pubkeys: vec![sender_pubkey.clone()],
                recipient_relay_hints: BTreeMap::new(),
                content: "project status".to_string(),
                created_at: 201,
                reply_to_event_id: None,
                subject: Some("War Room".to_string()),
                wrapped_events: Vec::new(),
                delivery_state: ManagedChatDeliveryState::Publishing,
                attempt_count: 1,
                last_error: None,
            })
            .expect("queue local side room message");

        let reply_message = nostr::nip17::ChatMessage::new("ack")
            .add_recipient(identity.public_key_hex.clone(), None)
            .subject("War Room");
        let reply_message = reply_message.reply_to(local_message_id);
        let reply_wrap = nostr::nip17::send_chat_message(
            &reply_message,
            &sender_secret,
            &identity.public_key_hex,
            202,
        )
        .expect("reply wrap");

        projection.record_relay_events(vec![reply_wrap]);

        let room = projection
            .snapshot
            .rooms
            .iter()
            .find(|room| room.room_id == room_id)
            .expect("side room");
        assert_eq!(room.unread_count, 1);
        assert_eq!(room.mention_count, 1);
    }
}
