use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use crate::app_state::{AutopilotChatState, ChatBrowseMode};
use crate::spacetime_presence::SpacetimePresenceSnapshot;

const CHAT_SEARCH_RESULT_LIMIT: usize = 5;
const CHAT_SEARCH_PREVIEW_LIMIT: usize = 56;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatSpacetimeSearchHit {
    pub reference_label: String,
    pub message_id: String,
    pub preview: String,
    pub unread: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatSpacetimeSearchResult {
    pub source_tag: String,
    pub hit_count: usize,
    pub hits: Vec<ChatSpacetimeSearchHit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChatSearchEntry {
    position: usize,
    reference_label: String,
    message_id: String,
    preview: String,
    lowered_content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChatSearchIndex {
    fingerprint: String,
    entries: Vec<ChatSearchEntry>,
}

static CHAT_SEARCH_CACHE: OnceLock<Mutex<BTreeMap<String, ChatSearchIndex>>> = OnceLock::new();

pub fn active_chat_presence_summary(
    autopilot_chat: &AutopilotChatState,
    spacetime_presence: &SpacetimePresenceSnapshot,
) -> Option<String> {
    if autopilot_chat.chat_browse_mode() == ChatBrowseMode::Autopilot {
        return None;
    }
    if spacetime_presence.providers_online == 0 && spacetime_presence.node_status == "unregistered"
    {
        return None;
    }

    let source = if chat_spacetime_enabled(spacetime_presence) {
        "Spacetime accel"
    } else {
        "Presence fallback"
    };
    Some(format!(
        "{source}: {} provider(s) online  •  node {}",
        spacetime_presence.providers_online, spacetime_presence.node_status
    ))
}

pub fn active_chat_typing_summary(
    autopilot_chat: &AutopilotChatState,
    composer_value: &str,
    spacetime_presence: &SpacetimePresenceSnapshot,
) -> Option<String> {
    if autopilot_chat.chat_browse_mode() == ChatBrowseMode::Autopilot {
        return None;
    }
    if composer_value.trim().is_empty() || !chat_spacetime_enabled(spacetime_presence) {
        return None;
    }
    Some("Spacetime typing accel: you".to_string())
}

pub fn search_active_chat_messages(
    autopilot_chat: &AutopilotChatState,
    query: &str,
    spacetime_presence: &SpacetimePresenceSnapshot,
) -> ChatSpacetimeSearchResult {
    let query = query.trim().to_ascii_lowercase();
    let source_tag = if chat_spacetime_enabled(spacetime_presence) {
        "spacetime.chat.search".to_string()
    } else {
        "nostr.chat.search.fallback".to_string()
    };
    if query.is_empty() {
        return ChatSpacetimeSearchResult {
            source_tag,
            hit_count: 0,
            hits: Vec::new(),
        };
    }

    let Some(cache_key) = active_chat_cache_key(autopilot_chat) else {
        return ChatSpacetimeSearchResult {
            source_tag,
            hit_count: 0,
            hits: Vec::new(),
        };
    };
    let fingerprint = active_chat_fingerprint(autopilot_chat);
    let index = cached_or_rebuild_index(autopilot_chat, &cache_key, &fingerprint);
    let unread_cutoff = active_chat_read_cursor_position(autopilot_chat);

    let mut hits = index
        .entries
        .iter()
        .filter(|entry| entry.lowered_content.contains(&query))
        .map(|entry| {
            (
                entry.position,
                ChatSpacetimeSearchHit {
                    reference_label: entry.reference_label.clone(),
                    message_id: entry.message_id.clone(),
                    preview: entry.preview.clone(),
                    unread: unread_cutoff.is_none_or(|position| entry.position > position),
                },
            )
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .1
            .unread
            .cmp(&left.1.unread)
            .then_with(|| right.0.cmp(&left.0))
    });
    let hit_count = hits.len();
    hits.truncate(CHAT_SEARCH_RESULT_LIMIT);
    let hits = hits.into_iter().map(|(_, hit)| hit).collect();

    ChatSpacetimeSearchResult {
        source_tag,
        hit_count,
        hits,
    }
}

pub fn clear_cached_chat_search_state() {
    let cache = CHAT_SEARCH_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut cache) = cache.lock() {
        cache.clear();
    }
}

fn chat_spacetime_enabled(spacetime_presence: &SpacetimePresenceSnapshot) -> bool {
    std::env::var("OPENAGENTS_ENABLE_SPACETIME_SYNC")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
        || spacetime_presence.counter_source.contains(".live")
}

fn cached_or_rebuild_index(
    autopilot_chat: &AutopilotChatState,
    cache_key: &str,
    fingerprint: &str,
) -> ChatSearchIndex {
    let cache = CHAT_SEARCH_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut cache) = cache.lock() {
        if let Some(index) = cache.get(cache_key)
            && index.fingerprint == fingerprint
        {
            return index.clone();
        }
        let rebuilt = rebuild_search_index(autopilot_chat, fingerprint);
        cache.insert(cache_key.to_string(), rebuilt.clone());
        rebuilt
    } else {
        rebuild_search_index(autopilot_chat, fingerprint)
    }
}

fn rebuild_search_index(autopilot_chat: &AutopilotChatState, fingerprint: &str) -> ChatSearchIndex {
    let entries = match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => autopilot_chat
            .active_managed_chat_messages()
            .into_iter()
            .enumerate()
            .map(|(index, message)| ChatSearchEntry {
                position: index,
                reference_label: format!("#{}", index + 1),
                message_id: message.event_id.clone(),
                preview: compact_search_preview(&message.content),
                lowered_content: message.content.to_ascii_lowercase(),
            })
            .collect(),
        ChatBrowseMode::ManagedSystem => autopilot_chat
            .active_managed_system_messages()
            .into_iter()
            .enumerate()
            .map(|(index, message)| ChatSearchEntry {
                position: index,
                reference_label: format!("#{}", index + 1),
                message_id: message.event_id.clone(),
                preview: compact_search_preview(&message.content),
                lowered_content: message.content.to_ascii_lowercase(),
            })
            .collect(),
        ChatBrowseMode::DirectMessages => autopilot_chat
            .active_direct_message_messages()
            .into_iter()
            .enumerate()
            .map(|(index, message)| ChatSearchEntry {
                position: index,
                reference_label: format!("#{}", index + 1),
                message_id: message.message_id.clone(),
                preview: compact_search_preview(&message.content),
                lowered_content: message.content.to_ascii_lowercase(),
            })
            .collect(),
        ChatBrowseMode::Autopilot => Vec::new(),
    };

    ChatSearchIndex {
        fingerprint: fingerprint.to_string(),
        entries,
    }
}

fn active_chat_cache_key(autopilot_chat: &AutopilotChatState) -> Option<String> {
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            let group = autopilot_chat.active_managed_chat_group()?;
            let channel = autopilot_chat.active_managed_chat_channel()?;
            Some(format!("managed:{}:{}", group.group_id, channel.channel_id))
        }
        ChatBrowseMode::ManagedSystem => Some("managed-system".to_string()),
        ChatBrowseMode::DirectMessages => {
            let room = autopilot_chat.active_direct_message_room()?;
            Some(format!("dm:{}", room.room_id))
        }
        ChatBrowseMode::Autopilot => None,
    }
}

fn active_chat_fingerprint(autopilot_chat: &AutopilotChatState) -> String {
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            let messages = autopilot_chat.active_managed_chat_messages();
            format!(
                "managed:{}:{}",
                messages.len(),
                messages
                    .last()
                    .map(|message| message.event_id.as_str())
                    .unwrap_or("none")
            )
        }
        ChatBrowseMode::ManagedSystem => {
            let messages = autopilot_chat.active_managed_system_messages();
            format!(
                "managed-system:{}:{}",
                messages.len(),
                messages
                    .last()
                    .map(|message| message.event_id.as_str())
                    .unwrap_or("none")
            )
        }
        ChatBrowseMode::DirectMessages => {
            let messages = autopilot_chat.active_direct_message_messages();
            format!(
                "dm:{}:{}",
                messages.len(),
                messages
                    .last()
                    .map(|message| message.message_id.as_str())
                    .unwrap_or("none")
            )
        }
        ChatBrowseMode::Autopilot => "autopilot:empty".to_string(),
    }
}

fn active_chat_read_cursor_position(autopilot_chat: &AutopilotChatState) -> Option<usize> {
    match autopilot_chat.chat_browse_mode() {
        ChatBrowseMode::Managed => {
            let channel = autopilot_chat.active_managed_chat_channel()?;
            let cursor_id = autopilot_chat
                .managed_chat_projection
                .local_state
                .read_cursors
                .get(channel.channel_id.as_str())
                .and_then(|cursor| cursor.last_read_event_id.as_deref())?;
            autopilot_chat
                .active_managed_chat_messages()
                .into_iter()
                .position(|message| message.event_id == cursor_id)
        }
        ChatBrowseMode::ManagedSystem => None,
        ChatBrowseMode::DirectMessages => {
            let room = autopilot_chat.active_direct_message_room()?;
            let cursor_id = autopilot_chat
                .direct_message_projection
                .local_state
                .read_cursors
                .get(room.room_id.as_str())
                .and_then(|cursor| cursor.last_read_message_id.as_deref())?;
            autopilot_chat
                .active_direct_message_messages()
                .into_iter()
                .position(|message| message.message_id == cursor_id)
        }
        ChatBrowseMode::Autopilot => None,
    }
}

fn compact_search_preview(content: &str) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= CHAT_SEARCH_PREVIEW_LIMIT {
        compact
    } else {
        format!(
            "{}...",
            compact
                .chars()
                .take(CHAT_SEARCH_PREVIEW_LIMIT.saturating_sub(3))
                .collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::app_state::{AutopilotChatState, ManagedChatProjectionState};
    use crate::spacetime_presence::SpacetimePresenceSnapshot;

    use super::{
        ChatSpacetimeSearchResult, active_chat_presence_summary, clear_cached_chat_search_state,
        search_active_chat_messages,
    };

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
    ) -> nostr::Event {
        nostr::Event {
            id: repeated_hex(id_ch, 64),
            pubkey: repeated_hex(pubkey_ch, 64),
            created_at,
            kind,
            tags,
            content,
            sig: repeated_hex('f', 128),
        }
    }

    fn build_managed_chat_fixture() -> AutopilotChatState {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(path);
        let channel_id = repeated_hex('b', 64);

        let group_metadata = nostr::GroupMetadataEvent::new(
            "oa-main",
            nostr::GroupMetadata::new().with_name("Ops"),
            10,
        )
        .expect("group metadata");
        let channel = nostr::ManagedChannelCreateEvent::new(
            "oa-main",
            nostr::ChannelMetadata::new("ops", "", ""),
            20,
        )
        .expect("channel")
        .with_hints(
            nostr::ManagedChannelHints::new()
                .with_channel_type(nostr::ManagedChannelType::Ops)
                .with_position(1),
        )
        .expect("channel hints");
        let msg_one = nostr::ManagedChannelMessageEvent::new(
            "oa-main",
            &channel_id,
            "wss://relay.openagents.test",
            "Deploy started",
            30,
        )
        .expect("message one");
        let msg_two = nostr::ManagedChannelMessageEvent::new(
            "oa-main",
            &channel_id,
            "wss://relay.openagents.test",
            "Deploy completed cleanly",
            31,
        )
        .expect("message two");
        let msg_three = nostr::ManagedChannelMessageEvent::new(
            "oa-main",
            &channel_id,
            "wss://relay.openagents.test",
            "Rollback deploy if staging fails",
            32,
        )
        .expect("message three");

        chat.managed_chat_projection.record_relay_events(vec![
            signed_event('a', '1', 10, 39000, group_metadata.to_tags(), String::new()),
            signed_event(
                'b',
                '2',
                20,
                40,
                channel.to_tags().expect("channel tags"),
                channel.content().expect("channel content"),
            ),
            signed_event(
                'c',
                '3',
                30,
                42,
                msg_one.to_tags().expect("message tags"),
                "Deploy started".to_string(),
            ),
            signed_event(
                'd',
                '4',
                31,
                42,
                msg_two.to_tags().expect("message tags"),
                "Deploy completed cleanly".to_string(),
            ),
            signed_event(
                'e',
                '5',
                32,
                42,
                msg_three.to_tags().expect("message tags"),
                "Rollback deploy if staging fails".to_string(),
            ),
        ]);
        let _ = chat
            .managed_chat_projection
            .mark_channel_read(&channel_id, Some(&repeated_hex('c', 64)));
        chat
    }

    fn search_hit_labels(result: &ChatSpacetimeSearchResult) -> Vec<String> {
        result
            .hits
            .iter()
            .map(|hit| hit.reference_label.clone())
            .collect()
    }

    #[test]
    fn search_rebuilds_after_cache_clear_without_losing_canonical_hits() {
        clear_cached_chat_search_state();
        let chat = build_managed_chat_fixture();
        let snapshot = SpacetimePresenceSnapshot::default();

        let before = search_active_chat_messages(&chat, "deploy", &snapshot);
        clear_cached_chat_search_state();
        let after = search_active_chat_messages(&chat, "deploy", &snapshot);

        assert_eq!(before.hit_count, 3);
        assert_eq!(before, after);
    }

    #[test]
    fn search_prioritizes_messages_after_the_read_cursor() {
        clear_cached_chat_search_state();
        let chat = build_managed_chat_fixture();
        let result =
            search_active_chat_messages(&chat, "deploy", &SpacetimePresenceSnapshot::default());

        assert_eq!(search_hit_labels(&result), vec!["#3", "#2", "#1"]);
        assert!(result.hits[0].unread);
        assert!(result.hits[1].unread);
        assert!(!result.hits[2].unread);
    }

    #[test]
    fn presence_summary_stays_derived_and_non_authoritative() {
        let chat = build_managed_chat_fixture();
        let snapshot = SpacetimePresenceSnapshot {
            providers_online: 7,
            counter_source: "spacetime.presence.live".to_string(),
            node_status: "online".to_string(),
            ..SpacetimePresenceSnapshot::default()
        };

        let summary = active_chat_presence_summary(&chat, &snapshot).expect("presence summary");
        assert!(summary.contains("Spacetime accel"));
        assert!(summary.contains("7 provider(s) online"));
        assert!(summary.contains("node online"));
    }
}
