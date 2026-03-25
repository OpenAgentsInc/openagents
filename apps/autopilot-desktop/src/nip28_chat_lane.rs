use crate::app_state::DefaultNip28ChannelConfig;
use nostr::Event;
use nostr_client::{
    ConnectionState, PoolConfig, RelayAuthIdentity, RelayConfig, RelayConnection, RelayMessage,
    RelayPool,
};
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

const LANE_POLL: Duration = Duration::from_millis(15);
const RELAY_RECV_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_MESSAGES_PER_RELAY_POLL: usize = 64;
const SUBSCRIPTION_ID_PREFIX: &str = "autopilot-nip28-chat";
const NIP28_CHAT_HISTORY_WINDOW_SECS: u64 = 86400;
pub const NIP28_CHAT_BACKFILL_OVERLAP_SECS: u64 = 120;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Nip28ChatLaneSnapshot {
    pub configured_relays: Vec<String>,
    pub subscribed_channel_ids: Vec<String>,
    pub connected_relay_count: usize,
    pub last_inbound_event_at_epoch_secs: Option<u64>,
    pub last_eose_at_epoch_secs: Option<u64>,
    pub reconnecting: bool,
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub enum Nip28ChatLaneCommand {
    SyncManagedChatSubscriptions {
        relay_urls: Vec<String>,
        channel_ids: Vec<String>,
        since_created_at: u64,
    },
    Publish {
        event: Event,
    },
    FetchKind0Metadata {
        pubkeys: Vec<String>,
    },
}

#[derive(Debug)]
pub enum Nip28ChatLaneUpdate {
    RelayEvent(Event),
    Eose { relay_url: String },
    ConnectionError { relay_url: String, message: String },
    PublishAck { event_id: String },
    PublishError { event_id: String, message: String },
    AuthChallengeReceived { relay_url: String },
    Snapshot(Nip28ChatLaneSnapshot),
}

pub struct Nip28ChatLaneWorker {
    update_rx: Receiver<Nip28ChatLaneUpdate>,
    command_tx: Sender<Nip28ChatLaneCommand>,
    dispatched_ids: HashSet<String>,
}

impl Nip28ChatLaneWorker {
    pub fn spawn() -> Self {
        Self::spawn_with_config(DefaultNip28ChannelConfig::from_env_or_default())
    }

    pub fn spawn_with_config(config: DefaultNip28ChannelConfig) -> Self {
        let (update_tx, update_rx) = mpsc::channel::<Nip28ChatLaneUpdate>();
        let (command_tx, command_rx) = mpsc::channel::<Nip28ChatLaneCommand>();
        std::thread::spawn(move || run_nip28_chat_lane_loop(update_tx, command_rx, config));
        Self {
            update_rx,
            command_tx,
            dispatched_ids: HashSet::new(),
        }
    }

    pub fn drain_updates(&mut self) -> Vec<Nip28ChatLaneUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn publish(&mut self, event: Event) {
        if self.dispatched_ids.contains(&event.id) {
            return;
        }
        self.dispatched_ids.insert(event.id.clone());
        let _ = self
            .command_tx
            .send(Nip28ChatLaneCommand::Publish { event });
    }

    pub fn sync_managed_chat_subscriptions(
        &self,
        relay_urls: Vec<String>,
        channel_ids: Vec<String>,
        since_created_at: u64,
    ) {
        let _ = self
            .command_tx
            .send(Nip28ChatLaneCommand::SyncManagedChatSubscriptions {
                relay_urls,
                channel_ids,
                since_created_at,
            });
    }

    pub fn clear_dispatched(&mut self, event_id: &str) {
        self.dispatched_ids.remove(event_id);
    }

    pub fn fetch_kind0_if_needed(&self, pubkeys: Vec<String>) {
        if !pubkeys.is_empty() {
            let _ = self
                .command_tx
                .send(Nip28ChatLaneCommand::FetchKind0Metadata { pubkeys });
        }
    }
}

struct Nip28ChatLaneState {
    pool: Option<Arc<RelayPool>>,
    desired_relays: Vec<String>,
    desired_channel_ids: Vec<String>,
    since_created_at: u64,
    relay_subscription_ids: HashMap<String, String>,
    next_subscription_seq: u64,
    fetched_kind0_pubkeys: HashSet<String>,
    snapshot: Nip28ChatLaneSnapshot,
    last_emitted_snapshot: Option<Nip28ChatLaneSnapshot>,
    subscriptions_dirty: bool,
}

impl Nip28ChatLaneState {
    fn new(config: DefaultNip28ChannelConfig) -> Self {
        let mut bootstrap_channels = vec![config.channel_id];
        if let Some(team_channel_id) = config.team_channel_id {
            bootstrap_channels.push(team_channel_id);
        }
        let desired_relays = normalize_relay_urls(vec![config.relay_url]);
        let desired_channel_ids = normalize_channel_ids(bootstrap_channels);
        let since_created_at =
            current_epoch_seconds().saturating_sub(NIP28_CHAT_HISTORY_WINDOW_SECS);
        Self {
            pool: None,
            desired_relays: desired_relays.clone(),
            desired_channel_ids: desired_channel_ids.clone(),
            since_created_at,
            relay_subscription_ids: HashMap::new(),
            next_subscription_seq: 0,
            fetched_kind0_pubkeys: HashSet::new(),
            snapshot: Nip28ChatLaneSnapshot {
                configured_relays: desired_relays,
                subscribed_channel_ids: desired_channel_ids,
                connected_relay_count: 0,
                last_inbound_event_at_epoch_secs: None,
                last_eose_at_epoch_secs: None,
                reconnecting: false,
                last_error: None,
            },
            last_emitted_snapshot: None,
            subscriptions_dirty: true,
        }
    }

    fn next_subscription_id(&mut self) -> String {
        self.next_subscription_seq = self.next_subscription_seq.saturating_add(1);
        format!("{SUBSCRIPTION_ID_PREFIX}-{}", self.next_subscription_seq)
    }

    fn emit_snapshot_if_changed(&mut self, update_tx: &Sender<Nip28ChatLaneUpdate>) {
        if self.last_emitted_snapshot.as_ref() == Some(&self.snapshot) {
            return;
        }
        let snapshot = self.snapshot.clone();
        let _ = update_tx.send(Nip28ChatLaneUpdate::Snapshot(snapshot.clone()));
        self.last_emitted_snapshot = Some(snapshot);
    }
}

fn normalize_relay_urls(relays: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    relays
        .into_iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .filter(|relay| seen.insert(relay.clone()))
        .collect()
}

fn normalize_channel_ids(channel_ids: Vec<String>) -> Vec<String> {
    channel_ids
        .into_iter()
        .map(|channel_id| channel_id.trim().to_ascii_lowercase())
        .filter(|channel_id| {
            channel_id.len() == 64
                && channel_id
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn build_filters(channel_ids: &[String], since_epoch_secs: u64) -> Vec<serde_json::Value> {
    if channel_ids.is_empty() {
        return Vec::new();
    }
    vec![
        json!({"kinds": [40], "ids": channel_ids}),
        json!({"kinds": [41], "#e": channel_ids}),
        json!({"kinds": [42], "#e": channel_ids, "since": since_epoch_secs, "limit": 512}),
    ]
}

fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn run_nip28_chat_lane_loop(
    update_tx: Sender<Nip28ChatLaneUpdate>,
    command_rx: Receiver<Nip28ChatLaneCommand>,
    config: DefaultNip28ChannelConfig,
) {
    tracing::info!("nip28: lane starting");

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            tracing::warn!(message = %error, "nip28: runtime init failed");
            return;
        }
    };

    let private_key_hex = config.private_key_hex.clone();
    let mut state = Nip28ChatLaneState::new(config);
    state.emit_snapshot_if_changed(&update_tx);

    loop {
        while let Ok(command) = command_rx.try_recv() {
            handle_command(&runtime, &mut state, &update_tx, command);
        }
        state.emit_snapshot_if_changed(&update_tx);

        reconcile_connections(&runtime, &mut state, &update_tx, private_key_hex.as_deref());
        state.emit_snapshot_if_changed(&update_tx);

        if state.pool.is_some() {
            runtime.block_on(poll_events(&mut state, &update_tx));
            state.emit_snapshot_if_changed(&update_tx);
        }

        std::thread::sleep(LANE_POLL);
    }
}

fn handle_command(
    runtime: &tokio::runtime::Runtime,
    state: &mut Nip28ChatLaneState,
    update_tx: &Sender<Nip28ChatLaneUpdate>,
    command: Nip28ChatLaneCommand,
) {
    match command {
        Nip28ChatLaneCommand::SyncManagedChatSubscriptions {
            relay_urls,
            channel_ids,
            since_created_at,
        } => {
            let next_relays = normalize_relay_urls(relay_urls);
            let next_channel_ids = normalize_channel_ids(channel_ids);
            let relay_changed = next_relays != state.desired_relays;
            let channel_changed = next_channel_ids != state.desired_channel_ids;
            let since_changed = since_created_at != state.since_created_at;
            if !relay_changed && !channel_changed && !since_changed {
                return;
            }

            state.desired_relays = next_relays.clone();
            state.desired_channel_ids = next_channel_ids.clone();
            state.since_created_at = since_created_at;
            state.snapshot.configured_relays = next_relays;
            state.snapshot.subscribed_channel_ids = next_channel_ids;
            state.snapshot.last_error = None;
            state.snapshot.reconnecting =
                !state.desired_relays.is_empty() && !state.desired_channel_ids.is_empty();
            if relay_changed {
                disconnect_pool(runtime, state);
            }
            state.subscriptions_dirty = true;
        }
        Nip28ChatLaneCommand::FetchKind0Metadata { pubkeys } => {
            let new_pubkeys: Vec<String> = pubkeys
                .into_iter()
                .filter(|pubkey| state.fetched_kind0_pubkeys.insert(pubkey.clone()))
                .collect();
            if new_pubkeys.is_empty() {
                return;
            }
            let Some(pool) = state.pool.as_ref().cloned() else {
                return;
            };
            let Some(relay_url) = state.desired_relays.first().cloned() else {
                return;
            };
            runtime.block_on(async {
                if let Some(relay) = pool.relay(relay_url.as_str()).await {
                    let filter = json!({"kinds": [0], "authors": new_pubkeys});
                    let _ = relay
                        .subscribe_filters("autopilot-nip28-kind0", vec![filter])
                        .await;
                }
            });
        }
        Nip28ChatLaneCommand::Publish { event } => {
            let event_id = event.id.clone();
            let Some(pool) = state.pool.as_ref().cloned() else {
                let _ = update_tx.send(Nip28ChatLaneUpdate::PublishError {
                    event_id,
                    message: "not connected, publish skipped".to_string(),
                });
                return;
            };
            match runtime.block_on(pool.publish(&event)) {
                Ok(confirmations) => {
                    let accepted = confirmations
                        .iter()
                        .any(|confirmation| confirmation.accepted);
                    if accepted {
                        let _ = update_tx.send(Nip28ChatLaneUpdate::PublishAck { event_id });
                    } else {
                        let message = confirmations
                            .first()
                            .map(|confirmation| confirmation.message.clone())
                            .unwrap_or_else(|| "relay rejected event".to_string());
                        let _ =
                            update_tx.send(Nip28ChatLaneUpdate::PublishError { event_id, message });
                    }
                }
                Err(error) => {
                    let _ = update_tx.send(Nip28ChatLaneUpdate::PublishError {
                        event_id,
                        message: format!("publish failed: {error}"),
                    });
                }
            }
        }
    }
}

fn disconnect_pool(runtime: &tokio::runtime::Runtime, state: &mut Nip28ChatLaneState) {
    if let Some(pool) = state.pool.take() {
        let _ = runtime.block_on(pool.disconnect_all());
    }
    state.relay_subscription_ids.clear();
    state.snapshot.connected_relay_count = 0;
}

fn reconcile_connections(
    runtime: &tokio::runtime::Runtime,
    state: &mut Nip28ChatLaneState,
    update_tx: &Sender<Nip28ChatLaneUpdate>,
    private_key_hex: Option<&str>,
) {
    if state.desired_relays.is_empty() {
        disconnect_pool(runtime, state);
        state.snapshot.reconnecting = false;
        return;
    }

    if state.pool.is_none() {
        let pool_config = if let Some(private_key_hex) = private_key_hex {
            PoolConfig {
                relay_config: RelayConfig {
                    nip42_identity: Some(RelayAuthIdentity {
                        private_key_hex: private_key_hex.to_string(),
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }
        } else {
            PoolConfig::default()
        };
        state.pool = Some(Arc::new(RelayPool::new(pool_config)));
        state.subscriptions_dirty = true;
    }

    let Some(pool) = state.pool.as_ref().cloned() else {
        return;
    };

    runtime.block_on(async {
        for relay_url in &state.desired_relays {
            if pool.relay(relay_url.as_str()).await.is_none()
                && let Err(error) = pool.add_relay(relay_url.as_str()).await
            {
                let message = format!("add relay failed: {error}");
                state.snapshot.last_error = Some(message.clone());
                state.snapshot.reconnecting = true;
                let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                    relay_url: relay_url.clone(),
                    message,
                });
            }
        }

        let mut connected_relays = 0usize;
        for relay_url in state.desired_relays.clone() {
            let Some(connection) = pool.relay(relay_url.as_str()).await else {
                continue;
            };

            if connection.state().await != ConnectionState::Connected {
                match pool.connect_relay(relay_url.as_str()).await {
                    Ok(()) => state.subscriptions_dirty = true,
                    Err(error) => {
                        let message = format!("connect failed: {error}");
                        state.snapshot.last_error = Some(message.clone());
                        state.snapshot.reconnecting = true;
                        let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                            relay_url: relay_url.clone(),
                            message,
                        });
                        continue;
                    }
                }
            }

            if connection.state().await == ConnectionState::Connected {
                connected_relays = connected_relays.saturating_add(1);
                let missing_subscription = !state
                    .relay_subscription_ids
                    .contains_key(relay_url.as_str());
                if state.subscriptions_dirty || missing_subscription {
                    if replace_subscription(connection.as_ref(), state)
                        .await
                        .map_err(|error| {
                            let message = format!("subscribe failed: {error}");
                            state.snapshot.last_error = Some(message.clone());
                            state.snapshot.reconnecting = true;
                            let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                                relay_url: relay_url.clone(),
                                message,
                            });
                        })
                        .is_ok()
                    {
                        state.snapshot.last_error = None;
                    }
                }
            }
        }

        state.snapshot.connected_relay_count = connected_relays;
        state.snapshot.reconnecting =
            !state.desired_channel_ids.is_empty() && connected_relays < state.desired_relays.len();
        if connected_relays > 0 && !state.subscriptions_dirty {
            state.snapshot.last_error = None;
        }
        state.subscriptions_dirty = false;
    });
}

async fn replace_subscription(
    connection: &RelayConnection,
    state: &mut Nip28ChatLaneState,
) -> Result<(), nostr_client::ClientError> {
    let relay_url = connection.url().to_string();
    let filters = build_filters(&state.desired_channel_ids, state.since_created_at);
    if let Some(previous_subscription_id) = state.relay_subscription_ids.remove(relay_url.as_str())
    {
        let _ = connection
            .unsubscribe(previous_subscription_id.as_str())
            .await;
    }
    if filters.is_empty() {
        return Ok(());
    }

    let next_subscription_id = state.next_subscription_id();
    connection
        .subscribe_filters(next_subscription_id.as_str(), filters)
        .await?;
    state
        .relay_subscription_ids
        .insert(relay_url, next_subscription_id);
    Ok(())
}

async fn poll_events(state: &mut Nip28ChatLaneState, update_tx: &Sender<Nip28ChatLaneUpdate>) {
    let Some(pool) = state.pool.as_ref().cloned() else {
        return;
    };
    let relays = pool.relays().await;
    for relay in relays {
        let relay_url = relay.url().to_string();
        for _ in 0..MAX_MESSAGES_PER_RELAY_POLL {
            match tokio::time::timeout(RELAY_RECV_TIMEOUT, relay.recv()).await {
                Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                    state.snapshot.last_inbound_event_at_epoch_secs = Some(current_epoch_seconds());
                    state.snapshot.last_error = None;
                    let _ = update_tx.send(Nip28ChatLaneUpdate::RelayEvent(event));
                }
                Ok(Ok(Some(RelayMessage::Eose(_)))) => {
                    state.snapshot.last_eose_at_epoch_secs = Some(current_epoch_seconds());
                    let _ = update_tx.send(Nip28ChatLaneUpdate::Eose {
                        relay_url: relay_url.clone(),
                    });
                }
                Ok(Ok(Some(RelayMessage::Auth(_)))) => {
                    let _ = update_tx.send(Nip28ChatLaneUpdate::AuthChallengeReceived {
                        relay_url: relay_url.clone(),
                    });
                }
                Ok(Ok(Some(_))) => {}
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    let message = format!("recv error: {error}");
                    state.snapshot.last_error = Some(message.clone());
                    state.snapshot.reconnecting = true;
                    state.relay_subscription_ids.remove(relay_url.as_str());
                    state.subscriptions_dirty = true;
                    let _ = relay.disconnect().await;
                    let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                        relay_url: relay_url.clone(),
                        message,
                    });
                    break;
                }
                Err(_) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NIP28_CHAT_BACKFILL_OVERLAP_SECS, build_filters, normalize_channel_ids,
        normalize_relay_urls,
    };

    #[test]
    fn normalize_relay_urls_dedupes_and_trims() {
        assert_eq!(
            normalize_relay_urls(vec![
                " wss://relay.one ".to_string(),
                "wss://relay.one".to_string(),
                "".to_string(),
                "wss://relay.two".to_string(),
            ]),
            vec!["wss://relay.one".to_string(), "wss://relay.two".to_string(),]
        );
    }

    #[test]
    fn normalize_channel_ids_dedupes_and_filters_invalid_values() {
        let valid = "ab".repeat(32);
        assert_eq!(
            normalize_channel_ids(vec![
                valid.clone(),
                valid.to_uppercase(),
                "short".to_string(),
            ]),
            vec![valid]
        );
    }

    #[test]
    fn build_filters_uses_requested_since_value() {
        let channel_id = "ab".repeat(32);
        let filters = build_filters(std::slice::from_ref(&channel_id), 42);
        assert_eq!(filters.len(), 3);
        assert_eq!(filters[2]["since"].as_u64(), Some(42));
        assert_eq!(NIP28_CHAT_BACKFILL_OVERLAP_SECS, 120);
    }
}
