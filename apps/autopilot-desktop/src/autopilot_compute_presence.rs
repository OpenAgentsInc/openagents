use std::time::{Duration, Instant};

use crate::app_state::{AutopilotChatState, DefaultNip28ChannelConfig};
use crate::autopilot_peer_roster::{
    AUTOPILOT_BUY_MODE_REQUEST_KIND_CAPABILITY, AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE,
    AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE, AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS,
    build_autopilot_compute_presence_content, parse_autopilot_compute_presence_message,
};
use crate::state::provider_runtime::{LocalInferenceBackend, ProviderMode, ProviderRuntimeState};

const AUTOPILOT_COMPUTE_PRESENCE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const AUTOPILOT_COMPUTE_PRESENCE_RETRY_INTERVAL: Duration = Duration::from_secs(5);

pub(crate) fn pump_provider_chat_presence(
    provider_runtime: &mut ProviderRuntimeState,
    chat: &mut AutopilotChatState,
    identity: Option<&nostr::NostrIdentity>,
    now: Instant,
    now_epoch_seconds: u64,
) -> bool {
    let mut changed =
        reconcile_pending_presence_publish(provider_runtime, chat, now, now_epoch_seconds);
    let Some(desired_mode) = desired_presence_mode(provider_runtime) else {
        return changed;
    };
    if provider_runtime
        .autopilot_presence
        .pending_event_id
        .as_deref()
        .is_some()
    {
        return changed;
    }
    if provider_runtime
        .autopilot_presence
        .retry_after
        .is_some_and(|retry_after| retry_after > now)
    {
        return changed;
    }

    let heartbeat_due = desired_mode == AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE
        && provider_runtime
            .autopilot_presence
            .next_heartbeat_at
            .is_some_and(|deadline| deadline <= now);
    let mode_changed = provider_runtime
        .autopilot_presence
        .last_published_mode
        .as_deref()
        != Some(desired_mode);
    let should_queue = if desired_mode == AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE {
        mode_changed || heartbeat_due
    } else {
        provider_runtime
            .autopilot_presence
            .last_published_mode
            .as_deref()
            == Some(AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE)
    };
    if !should_queue {
        return changed;
    }

    let Some(identity) = identity else {
        changed |= record_presence_publish_error(
            provider_runtime,
            now,
            "Cannot publish Autopilot compute presence: Nostr identity unavailable".to_string(),
        );
        return changed;
    };

    let config = DefaultNip28ChannelConfig::from_env_or_default();
    if !config.is_valid() {
        changed |= record_presence_publish_error(
            provider_runtime,
            now,
            "Cannot publish Autopilot compute presence: configured main NIP-28 channel is invalid"
                .to_string(),
        );
        return changed;
    }

    let Some(channel) = chat.configured_main_managed_chat_channel(&config).cloned() else {
        changed |= record_presence_publish_error(
            provider_runtime,
            now,
            "Cannot publish Autopilot compute presence: configured main NIP-28 channel is not loaded yet"
                .to_string(),
        );
        return changed;
    };

    let ready_model = current_ready_model(provider_runtime);
    let started_at = provider_runtime
        .inventory_session_started_at_ms
        .and_then(|value| u64::try_from(value).ok())
        .map(|value| value / 1_000)
        .filter(|value| *value > 0);
    let expires_at =
        now_epoch_seconds.saturating_add(AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS);
    let capabilities = if desired_mode == AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE {
        vec![AUTOPILOT_BUY_MODE_REQUEST_KIND_CAPABILITY.to_string()]
    } else {
        Vec::new()
    };
    let content = build_autopilot_compute_presence_content(
        identity.public_key_hex.as_str(),
        desired_mode,
        capabilities.as_slice(),
        ready_model.as_deref(),
        started_at,
        expires_at,
    );

    match crate::input::queue_managed_chat_message_to_channel_with_relay(
        chat,
        identity,
        channel.group_id.as_str(),
        channel.channel_id.as_str(),
        Some(config.relay_url.as_str()),
        content.as_str(),
        None,
    ) {
        Ok(event_id) => {
            provider_runtime.autopilot_presence.pending_mode = Some(desired_mode.to_string());
            provider_runtime.autopilot_presence.pending_event_id = Some(event_id.clone());
            provider_runtime.autopilot_presence.pending_queued_at = Some(now);
            provider_runtime.autopilot_presence.retry_after = None;
            provider_runtime.autopilot_presence.last_error = None;
            provider_runtime.autopilot_presence.last_action = Some(format!(
                "Queued Autopilot compute presence {} in main channel {}",
                desired_mode, channel.channel_id
            ));
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Queued Autopilot compute presence mode={} event_id={} channel_id={} relay_url={} ready_model={:?}",
                desired_mode,
                event_id,
                channel.channel_id,
                channel.relay_url.as_deref().unwrap_or(config.relay_url.as_str()),
                ready_model
            );
            changed = true;
        }
        Err(error) => {
            changed |= record_presence_publish_error(
                provider_runtime,
                now,
                format!("Cannot publish Autopilot compute presence: {error}"),
            );
        }
    }

    changed
}

fn reconcile_pending_presence_publish(
    provider_runtime: &mut ProviderRuntimeState,
    chat: &AutopilotChatState,
    now: Instant,
    now_epoch_seconds: u64,
) -> bool {
    let Some(event_id) = provider_runtime.autopilot_presence.pending_event_id.clone() else {
        return false;
    };
    let Some(message) = chat
        .managed_chat_projection
        .snapshot
        .messages
        .get(event_id.as_str())
    else {
        return false;
    };

    match message.delivery_state {
        crate::app_state::ManagedChatDeliveryState::Publishing => false,
        crate::app_state::ManagedChatDeliveryState::Failed => {
            let error = message
                .delivery_error
                .clone()
                .or_else(|| chat.autopilot_chat_error())
                .unwrap_or_else(|| "managed chat publish failed".to_string());
            provider_runtime.autopilot_presence.pending_mode = None;
            provider_runtime.autopilot_presence.pending_event_id = None;
            provider_runtime.autopilot_presence.pending_queued_at = None;
            provider_runtime.autopilot_presence.retry_after =
                Some(now + AUTOPILOT_COMPUTE_PRESENCE_RETRY_INTERVAL);
            provider_runtime.autopilot_presence.last_error = Some(error.clone());
            provider_runtime.autopilot_presence.last_action = Some(format!(
                "Autopilot compute presence publish failed for {event_id}"
            ));
            tracing::warn!(
                target: "autopilot_desktop::provider",
                "Autopilot compute presence publish failed event_id={} error={}",
                event_id,
                error
            );
            true
        }
        crate::app_state::ManagedChatDeliveryState::Acked
        | crate::app_state::ManagedChatDeliveryState::Confirmed => {
            let published_mode = provider_runtime
                .autopilot_presence
                .pending_mode
                .clone()
                .or_else(|| {
                    parse_autopilot_compute_presence_message(
                        message.content.as_str(),
                        message.author_pubkey.as_str(),
                    )
                    .map(|presence| presence.mode)
                })
                .unwrap_or_else(|| AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE.to_string());
            let expires_at = parse_autopilot_compute_presence_message(
                message.content.as_str(),
                message.author_pubkey.as_str(),
            )
            .and_then(|presence| presence.expires_at)
            .or(Some(now_epoch_seconds.saturating_add(
                AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS,
            )));
            provider_runtime.autopilot_presence.last_published_mode = Some(published_mode.clone());
            provider_runtime.autopilot_presence.last_published_event_id = Some(event_id.clone());
            provider_runtime
                .autopilot_presence
                .last_published_at_epoch_seconds = Some(now_epoch_seconds);
            provider_runtime
                .autopilot_presence
                .last_expires_at_epoch_seconds = expires_at;
            provider_runtime.autopilot_presence.pending_mode = None;
            provider_runtime.autopilot_presence.pending_event_id = None;
            provider_runtime.autopilot_presence.pending_queued_at = None;
            provider_runtime.autopilot_presence.retry_after = None;
            provider_runtime.autopilot_presence.last_error = None;
            provider_runtime.autopilot_presence.last_action = Some(format!(
                "Published Autopilot compute presence {} via managed chat",
                published_mode
            ));
            provider_runtime.autopilot_presence.next_heartbeat_at = (published_mode
                == AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE)
                .then_some(now + AUTOPILOT_COMPUTE_PRESENCE_HEARTBEAT_INTERVAL);
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Published Autopilot compute presence mode={} event_id={} expires_at={:?}",
                published_mode,
                event_id,
                expires_at
            );
            true
        }
    }
}

fn desired_presence_mode(provider_runtime: &ProviderRuntimeState) -> Option<&'static str> {
    if provider_runtime.mode == ProviderMode::Online
        && provider_runtime.active_inference_backend().is_some()
    {
        return Some(AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE);
    }
    if provider_runtime
        .autopilot_presence
        .last_published_mode
        .as_deref()
        == Some(AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE)
        || provider_runtime.autopilot_presence.pending_mode.as_deref()
            == Some(AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE)
    {
        return Some(AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE);
    }
    None
}

fn current_ready_model(provider_runtime: &ProviderRuntimeState) -> Option<String> {
    match provider_runtime.active_inference_backend() {
        Some(LocalInferenceBackend::AppleFoundationModels) => {
            provider_runtime.apple_fm.ready_model.clone()
        }
        Some(LocalInferenceBackend::Ollama) => provider_runtime
            .ollama
            .ready_model
            .clone()
            .or_else(|| provider_runtime.ollama.configured_model.clone()),
        _ => None,
    }
}

fn record_presence_publish_error(
    provider_runtime: &mut ProviderRuntimeState,
    now: Instant,
    error: String,
) -> bool {
    let changed = provider_runtime.autopilot_presence.last_error.as_deref() != Some(error.as_str())
        || provider_runtime.autopilot_presence.retry_after.is_none()
        || provider_runtime.autopilot_presence.last_action.is_none();
    provider_runtime.autopilot_presence.retry_after =
        Some(now + AUTOPILOT_COMPUTE_PRESENCE_RETRY_INTERVAL);
    provider_runtime.autopilot_presence.last_error = Some(error.clone());
    provider_runtime.autopilot_presence.last_action =
        Some("Autopilot compute presence publish blocked".to_string());
    tracing::warn!(
        target: "autopilot_desktop::provider",
        "{}",
        error
    );
    changed
}

trait AutopilotChatPresenceError {
    fn autopilot_chat_error(&self) -> Option<String>;
}

impl AutopilotChatPresenceError for AutopilotChatState {
    fn autopilot_chat_error(&self) -> Option<String> {
        self.last_error
            .clone()
            .or_else(|| self.managed_chat_projection.last_error.clone())
    }
}

#[cfg(test)]
mod tests {
    use nostr::{
        ChannelMetadata, Event, GroupMetadata, GroupMetadataEvent, ManagedChannelCreateEvent,
        ManagedChannelHints, ManagedChannelType,
    };
    use tempfile::tempdir;

    use super::*;
    use crate::app_state::{ManagedChatProjectionState, ProviderMode};

    fn repeated_hex(ch: char, len: usize) -> String {
        std::iter::repeat_n(ch, len).collect()
    }

    fn signed_event(
        id: impl Into<String>,
        pubkey: impl Into<String>,
        created_at: u64,
        kind: u16,
        tags: Vec<Vec<String>>,
        content: impl Into<String>,
    ) -> Event {
        Event {
            id: id.into(),
            pubkey: pubkey.into(),
            created_at,
            kind,
            tags,
            content: content.into(),
            sig: repeated_hex('f', 128),
        }
    }

    fn build_test_group_metadata_event() -> Event {
        let template = GroupMetadataEvent::new(
            "oa-main",
            GroupMetadata::new().with_name("OpenAgents Main"),
            10,
        )
        .expect("group metadata");
        signed_event(
            repeated_hex('a', 64),
            repeated_hex('1', 64),
            10,
            39000,
            template.to_tags(),
            String::new(),
        )
    }

    fn build_test_channel_create_event(channel_id: &str) -> Event {
        let template = ManagedChannelCreateEvent::new(
            "oa-main",
            ChannelMetadata::new("main", "OpenAgents main channel", ""),
            20,
        )
        .expect("channel create")
        .with_hints(
            ManagedChannelHints::new()
                .with_slug("main")
                .with_channel_type(ManagedChannelType::Ops)
                .with_category_id("main")
                .with_category_label("Main")
                .with_position(1),
        )
        .expect("channel hints");
        signed_event(
            channel_id.to_string(),
            repeated_hex('2', 64),
            20,
            40,
            template.to_tags().expect("channel tags"),
            template.content().expect("channel content"),
        )
    }

    fn ready_provider_runtime(now: Instant) -> ProviderRuntimeState {
        let mut runtime = ProviderRuntimeState::default();
        runtime.mode = ProviderMode::Online;
        runtime.mode_changed_at = now;
        runtime.inventory_session_started_at_ms = Some(25_000);
        runtime.apple_fm.reachable = true;
        runtime.apple_fm.model_available = true;
        runtime.apple_fm.ready_model = Some("apple-foundation-model".to_string());
        runtime
    }

    fn fixture_chat(identity: &nostr::NostrIdentity) -> AutopilotChatState {
        let temp = tempdir().expect("tempdir");
        let projection_path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(projection_path);
        chat.managed_chat_projection
            .set_local_pubkey(Some(identity.public_key_hex.as_str()));
        let main_channel_id = DefaultNip28ChannelConfig::from_env_or_default().channel_id;
        chat.managed_chat_projection.record_relay_events(vec![
            build_test_group_metadata_event(),
            build_test_channel_create_event(main_channel_id.as_str()),
        ]);
        chat
    }

    #[test]
    fn provider_presence_pump_queues_online_heartbeat_and_offline_messages_without_ui_selection() {
        let identity = nostr::regenerate_identity().expect("identity");
        let mut chat = fixture_chat(&identity);
        let now = Instant::now();
        let mut provider_runtime = ready_provider_runtime(now);
        let selected_channel_before = chat
            .managed_chat_projection
            .local_state
            .selected_channel_id
            .clone();

        assert!(pump_provider_chat_presence(
            &mut provider_runtime,
            &mut chat,
            Some(&identity),
            now,
            100
        ));
        assert_eq!(
            chat.managed_chat_projection.local_state.selected_channel_id,
            selected_channel_before
        );
        let first_event_id = provider_runtime
            .autopilot_presence
            .pending_event_id
            .clone()
            .unwrap_or_else(|| {
                panic!(
                    "pending presence event missing: last_action={:?} last_error={:?}",
                    provider_runtime.autopilot_presence.last_action,
                    provider_runtime.autopilot_presence.last_error
                )
            });
        let first_message = chat
            .managed_chat_projection
            .snapshot
            .messages
            .get(first_event_id.as_str())
            .expect("first local echo");
        let first_presence = parse_autopilot_compute_presence_message(
            first_message.content.as_str(),
            first_message.author_pubkey.as_str(),
        )
        .expect("first presence");
        assert_eq!(
            first_presence.mode,
            AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE.to_string()
        );
        assert_eq!(
            first_presence.capabilities,
            vec![AUTOPILOT_BUY_MODE_REQUEST_KIND_CAPABILITY.to_string()]
        );
        assert_eq!(
            first_presence.ready_model.as_deref(),
            Some("apple-foundation-model")
        );

        chat.managed_chat_projection
            .ack_outbound_message(first_event_id.as_str())
            .expect("ack first presence");
        assert!(pump_provider_chat_presence(
            &mut provider_runtime,
            &mut chat,
            Some(&identity),
            now,
            100
        ));
        assert_eq!(
            provider_runtime
                .autopilot_presence
                .last_published_mode
                .as_deref(),
            Some(AUTOPILOT_COMPUTE_PRESENCE_ONLINE_MODE)
        );
        assert!(
            provider_runtime
                .autopilot_presence
                .next_heartbeat_at
                .is_some()
        );

        let heartbeat_now =
            now + AUTOPILOT_COMPUTE_PRESENCE_HEARTBEAT_INTERVAL + Duration::from_secs(1);
        assert!(pump_provider_chat_presence(
            &mut provider_runtime,
            &mut chat,
            Some(&identity),
            heartbeat_now,
            131
        ));
        let heartbeat_event_id = provider_runtime
            .autopilot_presence
            .pending_event_id
            .clone()
            .expect("heartbeat event");
        assert_ne!(heartbeat_event_id, first_event_id);
        chat.managed_chat_projection
            .ack_outbound_message(heartbeat_event_id.as_str())
            .expect("ack heartbeat");
        assert!(pump_provider_chat_presence(
            &mut provider_runtime,
            &mut chat,
            Some(&identity),
            heartbeat_now,
            131
        ));

        provider_runtime.mode = ProviderMode::Offline;
        provider_runtime.mode_changed_at = heartbeat_now + Duration::from_secs(1);
        assert!(pump_provider_chat_presence(
            &mut provider_runtime,
            &mut chat,
            Some(&identity),
            heartbeat_now + Duration::from_secs(1),
            132
        ));
        let offline_event_id = provider_runtime
            .autopilot_presence
            .pending_event_id
            .clone()
            .expect("offline event");
        let offline_message = chat
            .managed_chat_projection
            .snapshot
            .messages
            .get(offline_event_id.as_str())
            .expect("offline message");
        let offline_presence = parse_autopilot_compute_presence_message(
            offline_message.content.as_str(),
            offline_message.author_pubkey.as_str(),
        )
        .expect("offline presence");
        assert_eq!(
            offline_presence.mode,
            AUTOPILOT_COMPUTE_PRESENCE_OFFLINE_MODE.to_string()
        );
        assert!(offline_presence.capabilities.is_empty());
    }

    #[test]
    fn provider_presence_pump_retries_when_main_channel_is_not_loaded() {
        let identity = nostr::regenerate_identity().expect("identity");
        let temp = tempdir().expect("tempdir");
        let projection_path = temp.path().join("managed-chat.json");
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(projection_path);
        chat.managed_chat_projection
            .set_local_pubkey(Some(identity.public_key_hex.as_str()));
        let now = Instant::now();
        let mut provider_runtime = ready_provider_runtime(now);

        assert!(pump_provider_chat_presence(
            &mut provider_runtime,
            &mut chat,
            Some(&identity),
            now,
            100
        ));
        assert!(
            provider_runtime
                .autopilot_presence
                .pending_event_id
                .is_none()
        );
        assert!(provider_runtime.autopilot_presence.retry_after.is_some());
        assert!(
            provider_runtime
                .autopilot_presence
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("configured main NIP-28 channel is not loaded"))
        );
    }
}
