use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app_state::{
    DefaultNip28ChannelConfig, ManagedChatDeliveryState, ManagedChatLocalState,
    ManagedChatProjectionSnapshot,
};
use crate::nip90_compute_semantics::normalize_pubkey;

pub(crate) const AUTOPILOT_COMPUTE_PRESENCE_TYPE: &str = "oa.autopilot.presence.v1";
pub(crate) const AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE: &str = "provider-online";
pub(crate) const AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE: &str = "provider-offline";
pub(crate) const AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS: u64 = 90;
pub(crate) const AUTOPILOT_MAIN_CHANNEL_CHAT_FRESHNESS_SECONDS: u64 = 60 * 60 * 24;
pub(crate) const AUTOPILOT_BUY_MODE_REQUEST_KIND_CAPABILITY: &str = "5050";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_ELIGIBLE: &str = "eligible";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_WAITING_FOR_PRESENCE: &str = "waiting-for-presence";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_PROVIDER_OFFLINE: &str = "provider-offline";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_PRESENCE_EXPIRED: &str = "presence-expired";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_MISSING_CAPABILITY: &str = "missing-capability-5050";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_LOCALLY_MUTED: &str = "locally-muted";
pub(crate) const AUTOPILOT_PEER_ELIGIBILITY_INVALID_PUBKEY: &str = "invalid-pubkey";
pub(crate) const AUTOPILOT_BUY_MODE_TARGET_BLOCK_INVALID_MAIN_CHANNEL_CONFIG: &str =
    "invalid-main-channel-config";
pub(crate) const AUTOPILOT_BUY_MODE_TARGET_BLOCK_WAITING_FOR_MAIN_CHANNEL: &str =
    "waiting-for-main-channel";
pub(crate) const AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_PEERS_OBSERVED: &str = "no-peers-observed";
pub(crate) const AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_ELIGIBLE_PEERS: &str = "no-eligible-peers";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct AutopilotPeerRosterRow {
    pub pubkey: String,
    pub last_chat_message_event_id: Option<String>,
    pub last_chat_message_at: Option<u64>,
    pub last_presence_event_id: Option<String>,
    pub last_presence_at: Option<u64>,
    pub presence_expires_at: Option<u64>,
    pub chat_activity_fresh: bool,
    pub presence_fresh: bool,
    pub online_for_compute: bool,
    pub online_reason: Option<String>,
    pub stale_reason: Option<String>,
    pub supported_request_kinds: Vec<String>,
    pub ready_model: Option<String>,
    pub source_channel_id: String,
    pub source_relay_url: String,
    pub eligible_for_buy_mode: bool,
    pub eligibility_reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct AutopilotBuyModeTargetSelection {
    pub selected_peer_pubkey: Option<String>,
    pub selected_relay_url: Option<String>,
    pub selected_ready_model: Option<String>,
    pub observed_peer_count: usize,
    pub eligible_peer_count: usize,
    pub blocked_reason_code: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct AutopilotComputePresence {
    pub mode: String,
    pub capabilities: Vec<String>,
    pub ready_model: Option<String>,
    pub started_at: Option<u64>,
    pub expires_at: Option<u64>,
}

#[derive(Clone, Debug, Default)]
struct MutableAutopilotPeerRosterRow {
    pubkey: String,
    last_chat_message_event_id: Option<String>,
    last_chat_message_at: Option<u64>,
    last_presence_event_id: Option<String>,
    last_presence_at: Option<u64>,
    presence_expires_at: Option<u64>,
    last_presence_mode: Option<String>,
    supported_request_kinds: Vec<String>,
    ready_model: Option<String>,
}

pub(crate) fn build_autopilot_peer_roster(
    snapshot: &ManagedChatProjectionSnapshot,
    local_state: &ManagedChatLocalState,
    local_pubkey: Option<&str>,
    config: &DefaultNip28ChannelConfig,
    now_epoch_seconds: u64,
) -> Vec<AutopilotPeerRosterRow> {
    if !config.is_valid() {
        return Vec::new();
    }

    let Some(channel) = snapshot
        .channels
        .iter()
        .find(|candidate| candidate.channel_id == config.channel_id)
    else {
        return Vec::new();
    };

    let local_pubkey = local_pubkey.map(normalize_pubkey);
    let source_relay_url = channel
        .relay_url
        .clone()
        .unwrap_or_else(|| config.relay_url.clone());
    let mut rows = BTreeMap::<String, MutableAutopilotPeerRosterRow>::new();

    for message_id in &channel.message_ids {
        let Some(message) = snapshot.messages.get(message_id) else {
            continue;
        };
        if message.delivery_state != ManagedChatDeliveryState::Confirmed {
            continue;
        }

        let normalized_pubkey = normalize_pubkey(message.author_pubkey.as_str());
        if normalized_pubkey.is_empty()
            || local_pubkey
                .as_deref()
                .is_some_and(|local| local == normalized_pubkey)
        {
            continue;
        }

        let entry = rows.entry(normalized_pubkey.clone()).or_insert_with(|| {
            MutableAutopilotPeerRosterRow {
                pubkey: normalized_pubkey.clone(),
                ..MutableAutopilotPeerRosterRow::default()
            }
        });

        if let Some(presence) = parse_autopilot_compute_presence_message(
            message.content.as_str(),
            normalized_pubkey.as_str(),
        ) {
            if is_newer_observation(
                entry.last_presence_at,
                entry.last_presence_event_id.as_deref(),
                message,
            ) {
                entry.last_presence_event_id = Some(message.event_id.clone());
                entry.last_presence_at = Some(message.created_at);
                entry.presence_expires_at = Some(presence.expires_at.unwrap_or_else(|| {
                    message
                        .created_at
                        .saturating_add(AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS)
                }));
                entry.last_presence_mode = Some(presence.mode);
                entry.supported_request_kinds = normalize_capabilities(presence.capabilities);
                entry.ready_model = presence.ready_model;
            }
            continue;
        }

        if is_newer_observation(
            entry.last_chat_message_at,
            entry.last_chat_message_event_id.as_deref(),
            message,
        ) {
            entry.last_chat_message_event_id = Some(message.event_id.clone());
            entry.last_chat_message_at = Some(message.created_at);
        }
    }

    let mut rows = rows
        .into_values()
        .map(|row| {
            finalize_roster_row(
                row,
                local_state,
                config.channel_id.as_str(),
                source_relay_url.as_str(),
                now_epoch_seconds,
            )
        })
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| {
        right
            .last_presence_at
            .cmp(&left.last_presence_at)
            .then_with(|| right.last_chat_message_at.cmp(&left.last_chat_message_at))
            .then_with(|| left.pubkey.cmp(&right.pubkey))
    });
    rows
}

pub(crate) fn select_autopilot_buy_mode_target(
    snapshot: &ManagedChatProjectionSnapshot,
    local_state: &ManagedChatLocalState,
    local_pubkey: Option<&str>,
    config: &DefaultNip28ChannelConfig,
    now_epoch_seconds: u64,
) -> AutopilotBuyModeTargetSelection {
    select_autopilot_buy_mode_target_with_policy(
        snapshot,
        local_state,
        local_pubkey,
        config,
        now_epoch_seconds,
        None,
    )
}

pub(crate) fn select_autopilot_buy_mode_target_with_policy(
    snapshot: &ManagedChatProjectionSnapshot,
    local_state: &ManagedChatLocalState,
    local_pubkey: Option<&str>,
    config: &DefaultNip28ChannelConfig,
    now_epoch_seconds: u64,
    last_targeted_peer_pubkey: Option<&str>,
) -> AutopilotBuyModeTargetSelection {
    let rows = build_autopilot_peer_roster(
        snapshot,
        local_state,
        local_pubkey,
        config,
        now_epoch_seconds,
    );
    select_autopilot_buy_mode_target_from_rows(
        snapshot,
        local_pubkey,
        config,
        last_targeted_peer_pubkey,
        rows.as_slice(),
    )
}

pub(crate) fn select_autopilot_buy_mode_target_from_rows(
    snapshot: &ManagedChatProjectionSnapshot,
    local_pubkey: Option<&str>,
    config: &DefaultNip28ChannelConfig,
    last_targeted_peer_pubkey: Option<&str>,
    rows: &[AutopilotPeerRosterRow],
) -> AutopilotBuyModeTargetSelection {
    if !config.is_valid() {
        return blocked_buy_mode_target_selection(
            AUTOPILOT_BUY_MODE_TARGET_BLOCK_INVALID_MAIN_CHANNEL_CONFIG,
            "Buy Mode blocked: invalid main NIP-28 channel configuration",
            0,
            0,
        );
    }

    if snapshot
        .channels
        .iter()
        .all(|channel| channel.channel_id != config.channel_id)
    {
        return blocked_buy_mode_target_selection(
            AUTOPILOT_BUY_MODE_TARGET_BLOCK_WAITING_FOR_MAIN_CHANNEL,
            "Buy Mode blocked: waiting for the configured main NIP-28 channel",
            0,
            0,
        );
    }

    let observed_peer_count = rows.len();
    let eligible_rows = rows
        .iter()
        .filter(|row| row.eligible_for_buy_mode)
        .collect::<Vec<_>>();
    let eligible_peer_count = eligible_rows.len();

    if let Some(selected) = select_eligible_target(
        eligible_rows.as_slice(),
        local_pubkey,
        last_targeted_peer_pubkey,
    ) {
        return AutopilotBuyModeTargetSelection {
            selected_peer_pubkey: Some(selected.pubkey.clone()),
            selected_relay_url: Some(selected.source_relay_url.clone()),
            selected_ready_model: selected.ready_model.clone(),
            observed_peer_count,
            eligible_peer_count,
            blocked_reason_code: None,
            blocked_reason: None,
        };
    }

    if rows.is_empty() {
        return blocked_buy_mode_target_selection(
            AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_PEERS_OBSERVED,
            "Buy Mode blocked: no Autopilot peers observed in the configured main NIP-28 channel",
            observed_peer_count,
            eligible_peer_count,
        );
    }

    let eligibility_summary = summarize_roster_eligibility_counts(rows);
    let blocked_reason = if eligibility_summary.is_empty() {
        "Buy Mode blocked: no eligible Autopilot peers are online for compute".to_string()
    } else {
        format!(
            "Buy Mode blocked: no eligible Autopilot peers are online for compute ({eligibility_summary})"
        )
    };
    blocked_buy_mode_target_selection(
        AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_ELIGIBLE_PEERS,
        blocked_reason.as_str(),
        observed_peer_count,
        eligible_peer_count,
    )
}

fn select_eligible_target<'a>(
    eligible_rows: &'a [&'a AutopilotPeerRosterRow],
    local_pubkey: Option<&str>,
    last_targeted_peer_pubkey: Option<&str>,
) -> Option<&'a AutopilotPeerRosterRow> {
    if eligible_rows.is_empty() {
        return None;
    }

    let normalized_last = last_targeted_peer_pubkey
        .map(normalize_pubkey)
        .filter(|value| !value.is_empty());
    if let Some(last) = normalized_last.as_deref()
        && let Some(last_index) = eligible_rows.iter().position(|row| row.pubkey == last)
    {
        let next_index = (last_index + 1) % eligible_rows.len();
        return eligible_rows.get(next_index).copied();
    }

    let normalized_local = local_pubkey.map(normalize_pubkey).unwrap_or_default();
    let offset = stable_peer_offset(normalized_local.as_str(), eligible_rows.len());
    eligible_rows.get(offset).copied()
}

fn stable_peer_offset(seed: &str, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    let digest = seed.as_bytes().iter().fold(0usize, |acc, byte| {
        acc.wrapping_mul(131).wrapping_add(*byte as usize)
    });
    digest % len
}

pub(crate) fn parse_autopilot_compute_presence_message(
    content: &str,
    author_pubkey: &str,
) -> Option<AutopilotComputePresence> {
    let payload = parse_presence_payload(content)?;
    let payload_pubkey = payload
        .get("pubkey")
        .and_then(Value::as_str)
        .map(normalize_pubkey);
    if payload_pubkey
        .as_deref()
        .is_some_and(|value| value != normalize_pubkey(author_pubkey))
    {
        return None;
    }

    let mode = payload.get("mode").and_then(Value::as_str)?.trim();
    if !matches!(
        mode,
        AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE | AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE
    ) {
        return None;
    }

    Some(AutopilotComputePresence {
        mode: mode.to_string(),
        capabilities: payload
            .get("capabilities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect(),
        ready_model: payload
            .get("ready_model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        started_at: payload.get("started_at").and_then(Value::as_u64),
        expires_at: payload.get("expires_at").and_then(Value::as_u64),
    })
}

pub(crate) fn build_autopilot_compute_presence_content(
    pubkey: &str,
    mode: &str,
    capabilities: &[String],
    ready_model: Option<&str>,
    started_at: Option<u64>,
    expires_at: u64,
) -> String {
    let mut capabilities = normalize_capabilities(capabilities.to_vec());
    if mode == AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE {
        capabilities.clear();
    }
    let mut payload = json!({
        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
        "pubkey": normalize_pubkey(pubkey),
        "mode": mode,
        "capabilities": capabilities,
        "expires_at": expires_at,
    });
    if let Some(started_at) = started_at {
        payload["started_at"] = Value::from(started_at);
    }
    if let Some(ready_model) = ready_model.map(str::trim).filter(|value| !value.is_empty()) {
        payload["ready_model"] = Value::from(ready_model.to_string());
    }
    serde_json::to_string(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"type":"{}","pubkey":"{}","mode":"{}","capabilities":[],"expires_at":{}}}"#,
            AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            normalize_pubkey(pubkey),
            mode,
            expires_at
        )
    })
}

fn parse_presence_payload(content: &str) -> Option<Value> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    let payload = if let Some(json_payload) = trimmed.strip_prefix(AUTOPILOT_COMPUTE_PRESENCE_TYPE)
    {
        serde_json::from_str::<Value>(json_payload.trim()).ok()?
    } else {
        serde_json::from_str::<Value>(trimmed).ok()?
    };

    match payload {
        Value::Object(map)
            if map
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|value| value.trim() == AUTOPILOT_COMPUTE_PRESENCE_TYPE) =>
        {
            Some(Value::Object(map))
        }
        _ => None,
    }
}

fn normalize_capabilities(capabilities: Vec<String>) -> Vec<String> {
    let mut capabilities = capabilities
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn blocked_buy_mode_target_selection(
    blocked_reason_code: &str,
    blocked_reason: &str,
    observed_peer_count: usize,
    eligible_peer_count: usize,
) -> AutopilotBuyModeTargetSelection {
    AutopilotBuyModeTargetSelection {
        selected_peer_pubkey: None,
        selected_relay_url: None,
        selected_ready_model: None,
        observed_peer_count,
        eligible_peer_count,
        blocked_reason_code: Some(blocked_reason_code.to_string()),
        blocked_reason: Some(blocked_reason.to_string()),
    }
}

fn summarize_roster_eligibility_counts(rows: &[AutopilotPeerRosterRow]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows.iter().filter(|row| !row.eligible_for_buy_mode) {
        *counts.entry(row.eligibility_reason.clone()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(reason, count)| format!("{reason}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn finalize_roster_row(
    row: MutableAutopilotPeerRosterRow,
    local_state: &ManagedChatLocalState,
    source_channel_id: &str,
    source_relay_url: &str,
    now_epoch_seconds: u64,
) -> AutopilotPeerRosterRow {
    let presence_fresh = row
        .presence_expires_at
        .zip(row.last_presence_at)
        .is_some_and(|(expires_at, last_presence_at)| {
            expires_at >= now_epoch_seconds && last_presence_at <= expires_at
        });
    let chat_activity_fresh = row.last_chat_message_at.is_some_and(|value| {
        value.saturating_add(AUTOPILOT_MAIN_CHANNEL_CHAT_FRESHNESS_SECONDS) >= now_epoch_seconds
    });
    let locally_muted = local_state.muted_pubkeys.contains(&row.pubkey);
    let valid_pubkey = is_valid_peer_pubkey(row.pubkey.as_str());
    let explicit_online =
        row.last_presence_mode.as_deref() == Some(AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE);
    let explicit_offline =
        row.last_presence_mode.as_deref() == Some(AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE);
    let supports_buy_mode = row
        .supported_request_kinds
        .iter()
        .any(|value| value == AUTOPILOT_BUY_MODE_REQUEST_KIND_CAPABILITY);
    let online_for_compute =
        valid_pubkey && !locally_muted && explicit_online && presence_fresh && supports_buy_mode;

    let eligibility_reason = if !valid_pubkey {
        AUTOPILOT_PEER_ELIGIBILITY_INVALID_PUBKEY
    } else if locally_muted {
        AUTOPILOT_PEER_ELIGIBILITY_LOCALLY_MUTED
    } else if row.last_presence_at.is_none() {
        AUTOPILOT_PEER_ELIGIBILITY_WAITING_FOR_PRESENCE
    } else if explicit_offline {
        AUTOPILOT_PEER_ELIGIBILITY_PROVIDER_OFFLINE
    } else if !presence_fresh {
        AUTOPILOT_PEER_ELIGIBILITY_PRESENCE_EXPIRED
    } else if !supports_buy_mode {
        AUTOPILOT_PEER_ELIGIBILITY_MISSING_CAPABILITY
    } else {
        AUTOPILOT_PEER_ELIGIBILITY_ELIGIBLE
    };

    let stale_reason = if row.last_presence_at.is_none() {
        Some("no-presence".to_string())
    } else if !presence_fresh {
        Some("presence-expired".to_string())
    } else {
        None
    };
    let online_reason = row.last_presence_mode.clone().or_else(|| {
        if row.last_presence_at.is_some() {
            Some("presence-observed".to_string())
        } else {
            None
        }
    });

    AutopilotPeerRosterRow {
        pubkey: row.pubkey,
        last_chat_message_event_id: row.last_chat_message_event_id,
        last_chat_message_at: row.last_chat_message_at,
        last_presence_event_id: row.last_presence_event_id,
        last_presence_at: row.last_presence_at,
        presence_expires_at: row.presence_expires_at,
        chat_activity_fresh,
        presence_fresh,
        online_for_compute,
        online_reason,
        stale_reason,
        supported_request_kinds: row.supported_request_kinds,
        ready_model: row.ready_model,
        source_channel_id: source_channel_id.to_string(),
        source_relay_url: source_relay_url.to_string(),
        eligible_for_buy_mode: online_for_compute,
        eligibility_reason: eligibility_reason.to_string(),
    }
}

fn is_newer_observation(
    current_at: Option<u64>,
    current_event_id: Option<&str>,
    message: &crate::app_state::ManagedChatMessageProjection,
) -> bool {
    match current_at {
        None => true,
        Some(current_at) => {
            message.created_at > current_at
                || (message.created_at == current_at
                    && current_event_id.is_none_or(|event_id| message.event_id.as_str() > event_id))
        }
    }
}

fn is_valid_peer_pubkey(value: &str) -> bool {
    let normalized = normalize_pubkey(value);
    (normalized.len() == 64
        && normalized
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')))
        || normalized.starts_with("npub1")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nostr::{ChannelMetadata, GroupMetadata, ManagedChannelHints, ManagedRoomMode};

    use super::*;
    use crate::app_state::{
        ManagedChatChannelProjection, ManagedChatDeliveryState, ManagedChatGroupProjection,
        ManagedChatMessageProjection, ManagedChatProjectionSnapshot,
    };

    fn fixture_config(channel_id: &str) -> DefaultNip28ChannelConfig {
        DefaultNip28ChannelConfig {
            relay_url: "wss://relay.openagents.test".to_string(),
            channel_id: channel_id.to_string(),
        }
    }

    fn fixture_snapshot(
        channel_id: &str,
        extra_channels: Vec<ManagedChatChannelProjection>,
        messages: Vec<ManagedChatMessageProjection>,
    ) -> ManagedChatProjectionSnapshot {
        let mut snapshot = ManagedChatProjectionSnapshot::default();
        let main_group_id = "oa-main".to_string();
        let main_channel = ManagedChatChannelProjection {
            channel_id: channel_id.to_string(),
            group_id: main_group_id.clone(),
            room_mode: ManagedRoomMode::ManagedChannel,
            metadata: ChannelMetadata::new("main", "", ""),
            hints: ManagedChannelHints::default(),
            relay_url: Some("wss://relay.openagents.test".to_string()),
            message_ids: messages
                .iter()
                .filter(|message| message.channel_id == channel_id)
                .map(|message| message.event_id.clone())
                .collect(),
            root_message_ids: messages
                .iter()
                .filter(|message| {
                    message.channel_id == channel_id && message.reply_to_event_id.is_none()
                })
                .map(|message| message.event_id.clone())
                .collect(),
            unread_count: 0,
            mention_count: 0,
            latest_message_id: messages
                .iter()
                .filter(|message| message.channel_id == channel_id)
                .map(|message| message.event_id.clone())
                .last(),
        };
        let group = ManagedChatGroupProjection {
            group_id: main_group_id,
            metadata: GroupMetadata::new().with_name("OpenAgents Main"),
            roles: Vec::new(),
            members: Vec::new(),
            channel_ids: std::iter::once(channel_id.to_string())
                .chain(
                    extra_channels
                        .iter()
                        .map(|channel| channel.channel_id.clone()),
                )
                .collect(),
            unread_count: 0,
            mention_count: 0,
        };

        snapshot.groups.push(group);
        snapshot.channels.push(main_channel);
        snapshot.channels.extend(extra_channels);
        snapshot.messages = messages
            .into_iter()
            .map(|message| (message.event_id.clone(), message))
            .collect::<BTreeMap<_, _>>();
        snapshot
    }

    fn fixture_message(
        event_id: &str,
        channel_id: &str,
        author_pubkey: &str,
        content: &str,
        created_at: u64,
    ) -> ManagedChatMessageProjection {
        ManagedChatMessageProjection {
            event_id: event_id.to_string(),
            group_id: "oa-main".to_string(),
            channel_id: channel_id.to_string(),
            author_pubkey: author_pubkey.to_string(),
            content: content.to_string(),
            created_at,
            reply_to_event_id: None,
            mention_pubkeys: Vec::new(),
            reaction_summaries: Vec::new(),
            reply_child_ids: Vec::new(),
            delivery_state: ManagedChatDeliveryState::Confirmed,
            delivery_error: None,
            attempt_count: 0,
        }
    }

    #[test]
    fn roster_derives_peer_rows_from_configured_main_channel_only() {
        let main_channel_id = &"aa".repeat(32);
        let other_channel_id = &"bb".repeat(32);
        let snapshot = fixture_snapshot(
            main_channel_id,
            vec![ManagedChatChannelProjection {
                channel_id: other_channel_id.to_string(),
                group_id: "oa-main".to_string(),
                room_mode: ManagedRoomMode::ManagedChannel,
                metadata: ChannelMetadata::new("other", "", ""),
                hints: ManagedChannelHints::default(),
                relay_url: Some("wss://relay.openagents.test".to_string()),
                message_ids: vec!["m2".to_string()],
                root_message_ids: vec!["m2".to_string()],
                unread_count: 0,
                mention_count: 0,
                latest_message_id: Some("m2".to_string()),
            }],
            vec![
                fixture_message("m1", main_channel_id, &"11".repeat(32), "hello", 10),
                fixture_message("m2", other_channel_id, &"22".repeat(32), "ignore", 12),
            ],
        );

        let rows = build_autopilot_peer_roster(
            &snapshot,
            &ManagedChatLocalState::default(),
            None,
            &fixture_config(main_channel_id),
            20,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pubkey, "11".repeat(32));
        assert_eq!(rows[0].last_chat_message_event_id.as_deref(), Some("m1"));
    }

    #[test]
    fn roster_excludes_local_identity_even_when_local_posts_in_main_channel() {
        let main_channel_id = &"aa".repeat(32);
        let local_pubkey = "11".repeat(32);
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![
                fixture_message("m1", main_channel_id, &local_pubkey, "hello", 10),
                fixture_message("m2", main_channel_id, &"22".repeat(32), "remote", 11),
            ],
        );

        let rows = build_autopilot_peer_roster(
            &snapshot,
            &ManagedChatLocalState::default(),
            Some(local_pubkey.as_str()),
            &fixture_config(main_channel_id),
            20,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pubkey, "22".repeat(32));
    }

    #[test]
    fn roster_tracks_presence_and_marks_fresh_online_buy_mode_peers_as_eligible() {
        let main_channel_id = &"aa".repeat(32);
        let author = "11".repeat(32);
        let presence = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
            "pubkey": author,
            "capabilities": ["5050", "5050"],
            "ready_model": "apple-foundation-model",
            "started_at": 25,
            "expires_at": 90
        });
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![
                fixture_message("chat", main_channel_id, &author, "human hello", 20),
                fixture_message(
                    "presence",
                    main_channel_id,
                    &author,
                    &presence.to_string(),
                    30,
                ),
            ],
        );

        let rows = build_autopilot_peer_roster(
            &snapshot,
            &ManagedChatLocalState::default(),
            None,
            &fixture_config(main_channel_id),
            40,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].last_chat_message_event_id.as_deref(), Some("chat"));
        assert_eq!(rows[0].last_presence_event_id.as_deref(), Some("presence"));
        assert!(rows[0].presence_fresh);
        assert!(rows[0].online_for_compute);
        assert!(rows[0].eligible_for_buy_mode);
        assert_eq!(
            rows[0].eligibility_reason,
            AUTOPILOT_PEER_ELIGIBILITY_ELIGIBLE
        );
        assert_eq!(rows[0].supported_request_kinds, vec!["5050".to_string()]);
        assert_eq!(
            rows[0].ready_model.as_deref(),
            Some("apple-foundation-model")
        );
    }

    #[test]
    fn roster_expires_stale_presence_even_without_explicit_offline_transition() {
        let main_channel_id = &"aa".repeat(32);
        let author = "11".repeat(32);
        let presence = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
            "capabilities": ["5050"]
        });
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![fixture_message(
                "presence",
                main_channel_id,
                &author,
                &presence.to_string(),
                10,
            )],
        );

        let rows = build_autopilot_peer_roster(
            &snapshot,
            &ManagedChatLocalState::default(),
            None,
            &fixture_config(main_channel_id),
            10 + AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS + 1,
        );

        assert_eq!(rows.len(), 1);
        assert!(!rows[0].presence_fresh);
        assert!(!rows[0].eligible_for_buy_mode);
        assert_eq!(
            rows[0].eligibility_reason,
            AUTOPILOT_PEER_ELIGIBILITY_PRESENCE_EXPIRED
        );
        assert_eq!(rows[0].stale_reason.as_deref(), Some("presence-expired"));
    }

    #[test]
    fn roster_marks_locally_muted_peer_ineligible_even_with_fresh_presence() {
        let main_channel_id = &"aa".repeat(32);
        let author = "11".repeat(32);
        let presence = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
            "capabilities": ["5050"],
            "expires_at": 90
        });
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![fixture_message(
                "presence",
                main_channel_id,
                &author,
                &presence.to_string(),
                30,
            )],
        );
        let mut local_state = ManagedChatLocalState::default();
        local_state.muted_pubkeys.insert(author.clone());

        let rows = build_autopilot_peer_roster(
            &snapshot,
            &local_state,
            None,
            &fixture_config(main_channel_id),
            40,
        );

        assert_eq!(rows.len(), 1);
        assert!(!rows[0].eligible_for_buy_mode);
        assert_eq!(
            rows[0].eligibility_reason,
            AUTOPILOT_PEER_ELIGIBILITY_LOCALLY_MUTED
        );
    }

    #[test]
    fn roster_parser_accepts_prefixed_presence_payloads_and_rejects_pubkey_mismatch() {
        let author = "11".repeat(32);
        let payload = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE,
            "pubkey": author,
            "capabilities": ["5050"]
        });
        let parsed = parse_autopilot_compute_presence_message(
            &format!("{AUTOPILOT_COMPUTE_PRESENCE_TYPE} {}", payload),
            &author,
        )
        .unwrap();
        assert_eq!(parsed.mode, AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE);

        let mismatched = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
            "pubkey": "22".repeat(32),
            "capabilities": ["5050"]
        });
        assert!(
            parse_autopilot_compute_presence_message(&mismatched.to_string(), &author).is_none()
        );
    }

    #[test]
    fn buy_mode_target_selection_chooses_first_eligible_peer_deterministically() {
        let main_channel_id = &"aa".repeat(32);
        let older = "11".repeat(32);
        let newer = "22".repeat(32);
        let older_presence = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
            "pubkey": older,
            "capabilities": ["5050"],
            "expires_at": 100
        });
        let newer_presence = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
            "pubkey": newer,
            "capabilities": ["5050"],
            "ready_model": "apple-foundation-model",
            "expires_at": 100
        });
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![
                fixture_message(
                    "presence-older",
                    main_channel_id,
                    &older,
                    &older_presence.to_string(),
                    30,
                ),
                fixture_message(
                    "presence-newer",
                    main_channel_id,
                    &newer,
                    &newer_presence.to_string(),
                    40,
                ),
            ],
        );

        let selection = select_autopilot_buy_mode_target(
            &snapshot,
            &ManagedChatLocalState::default(),
            None,
            &fixture_config(main_channel_id),
            50,
        );

        assert_eq!(selection.observed_peer_count, 2);
        assert_eq!(selection.eligible_peer_count, 2);
        assert_eq!(
            selection.selected_peer_pubkey.as_deref(),
            Some(newer.as_str())
        );
        assert_eq!(
            selection.selected_relay_url.as_deref(),
            Some("wss://relay.openagents.test")
        );
        assert_eq!(
            selection.selected_ready_model.as_deref(),
            Some("apple-foundation-model")
        );
        assert!(selection.blocked_reason_code.is_none());
    }

    #[test]
    fn buy_mode_target_selection_blocks_when_main_channel_is_missing() {
        let main_channel_id = &"aa".repeat(32);
        let selection = select_autopilot_buy_mode_target(
            &ManagedChatProjectionSnapshot::default(),
            &ManagedChatLocalState::default(),
            None,
            &fixture_config(main_channel_id),
            50,
        );

        assert_eq!(
            selection.blocked_reason_code.as_deref(),
            Some(AUTOPILOT_BUY_MODE_TARGET_BLOCK_WAITING_FOR_MAIN_CHANNEL)
        );
        assert!(selection.selected_peer_pubkey.is_none());
    }

    #[test]
    fn buy_mode_target_selection_blocks_when_no_peer_is_eligible() {
        let main_channel_id = &"aa".repeat(32);
        let author = "11".repeat(32);
        let presence = serde_json::json!({
            "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
            "mode": AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE,
            "pubkey": author,
            "capabilities": ["5050"],
            "expires_at": 90
        });
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![fixture_message(
                "presence",
                main_channel_id,
                &author,
                &presence.to_string(),
                30,
            )],
        );

        let selection = select_autopilot_buy_mode_target(
            &snapshot,
            &ManagedChatLocalState::default(),
            None,
            &fixture_config(main_channel_id),
            40,
        );

        assert_eq!(selection.observed_peer_count, 1);
        assert_eq!(selection.eligible_peer_count, 0);
        assert_eq!(
            selection.blocked_reason_code.as_deref(),
            Some(AUTOPILOT_BUY_MODE_TARGET_BLOCK_NO_ELIGIBLE_PEERS)
        );
        assert!(
            selection
                .blocked_reason
                .as_deref()
                .is_some_and(|reason| reason.contains(AUTOPILOT_PEER_ELIGIBILITY_PROVIDER_OFFLINE))
        );
    }

    #[test]
    fn buy_mode_target_selection_rotates_after_last_targeted_peer() {
        let main_channel_id = &"aa".repeat(32);
        let first = "11".repeat(32);
        let second = "22".repeat(32);
        let third = "33".repeat(32);
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![
                fixture_message(
                    "presence-first",
                    main_channel_id,
                    &first,
                    &serde_json::json!({
                        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
                        "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
                        "pubkey": first,
                        "capabilities": ["5050"],
                        "expires_at": 100
                    })
                    .to_string(),
                    10,
                ),
                fixture_message(
                    "presence-second",
                    main_channel_id,
                    &second,
                    &serde_json::json!({
                        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
                        "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
                        "pubkey": second,
                        "capabilities": ["5050"],
                        "expires_at": 100
                    })
                    .to_string(),
                    20,
                ),
                fixture_message(
                    "presence-third",
                    main_channel_id,
                    &third,
                    &serde_json::json!({
                        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
                        "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
                        "pubkey": third,
                        "capabilities": ["5050"],
                        "expires_at": 100
                    })
                    .to_string(),
                    30,
                ),
            ],
        );

        let selection = select_autopilot_buy_mode_target_with_policy(
            &snapshot,
            &ManagedChatLocalState::default(),
            Some(&"44".repeat(32)),
            &fixture_config(main_channel_id),
            40,
            Some(&third),
        );

        assert_eq!(selection.eligible_peer_count, 3);
        assert_eq!(
            selection.selected_peer_pubkey.as_deref(),
            Some(second.as_str())
        );
    }

    #[test]
    fn buy_mode_target_selection_uses_stable_local_offset_when_no_last_target_exists() {
        let main_channel_id = &"aa".repeat(32);
        let first = "11".repeat(32);
        let second = "22".repeat(32);
        let third = "33".repeat(32);
        let local = "44".repeat(32);
        let snapshot = fixture_snapshot(
            main_channel_id,
            Vec::new(),
            vec![
                fixture_message(
                    "presence-first",
                    main_channel_id,
                    &first,
                    &serde_json::json!({
                        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
                        "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
                        "pubkey": first,
                        "capabilities": ["5050"],
                        "expires_at": 100
                    })
                    .to_string(),
                    10,
                ),
                fixture_message(
                    "presence-second",
                    main_channel_id,
                    &second,
                    &serde_json::json!({
                        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
                        "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
                        "pubkey": second,
                        "capabilities": ["5050"],
                        "expires_at": 100
                    })
                    .to_string(),
                    20,
                ),
                fixture_message(
                    "presence-third",
                    main_channel_id,
                    &third,
                    &serde_json::json!({
                        "type": AUTOPILOT_COMPUTE_PRESENCE_TYPE,
                        "mode": AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE,
                        "pubkey": third,
                        "capabilities": ["5050"],
                        "expires_at": 100
                    })
                    .to_string(),
                    30,
                ),
            ],
        );

        let selection = select_autopilot_buy_mode_target_with_policy(
            &snapshot,
            &ManagedChatLocalState::default(),
            Some(local.as_str()),
            &fixture_config(main_channel_id),
            40,
            None,
        );

        let expected_order = [third.as_str(), second.as_str(), first.as_str()];
        let expected = expected_order[stable_peer_offset(local.as_str(), expected_order.len())];
        assert_eq!(selection.selected_peer_pubkey.as_deref(), Some(expected));
    }
}
