use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

use crate::{
    domain::{Event, Filter, matches_any},
    store::StoredEvent,
};

use super::{GatewayError, wire};

pub type ConnectionId = u64;

#[derive(Clone)]
pub struct HubHandle {
    sender: mpsc::Sender<HubCommand>,
}

pub struct ConnectionChannels {
    pub outbound: mpsc::Receiver<String>,
    pub close: watch::Receiver<bool>,
}

enum HubCommand {
    AddConnection {
        connection_id: ConnectionId,
        outbound: mpsc::Sender<String>,
        close: watch::Sender<bool>,
        response: oneshot::Sender<()>,
    },
    Register {
        key: SubscriptionKey,
        generation: u64,
        filters: Vec<Filter>,
        read_pubkeys: HashSet<String>,
        response: oneshot::Sender<bool>,
    },
    HistoryReady {
        key: SubscriptionKey,
        generation: u64,
        high_water: i64,
        events: Vec<StoredEvent>,
    },
    Remove {
        key: SubscriptionKey,
    },
    CloseSubscription {
        key: SubscriptionKey,
        message: String,
    },
    RemoveConnection {
        connection_id: ConnectionId,
    },
    Publish {
        event: PublishedEvent,
        now: u64,
    },
}

#[derive(Debug, Clone)]
pub struct PublishedEvent {
    pub event: Arc<Event>,
    pub ingest_seq: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubscriptionKey {
    connection_id: ConnectionId,
    subscription_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum IndexKey {
    Id(String),
    Author(String),
    Kind(u16),
    Tag(String, String),
    Broad,
}

struct Subscription {
    generation: u64,
    filters: Vec<Filter>,
    read_pubkeys: HashSet<String>,
    index_keys: HashSet<IndexKey>,
    state: SubscriptionState,
}

enum SubscriptionState {
    Buffering {
        events: Vec<PublishedEvent>,
        ids: HashSet<String>,
    },
    Live {
        high_water: i64,
    },
}

struct ConnectionSink {
    outbound: mpsc::Sender<String>,
    close: watch::Sender<bool>,
    subscriptions: HashSet<String>,
}

struct Hub {
    connections: HashMap<ConnectionId, ConnectionSink>,
    subscriptions: HashMap<SubscriptionKey, Subscription>,
    index: HashMap<IndexKey, HashSet<SubscriptionKey>>,
    max_buffered_live: usize,
    max_outbound_bytes: usize,
    recent_ephemeral_ids: HashSet<String>,
    recent_ephemeral_order: VecDeque<(String, Instant)>,
}

const EPHEMERAL_DEDUP_WINDOW: Duration = Duration::from_secs(60);
const MAX_RECENT_EPHEMERAL_IDS: usize = 4_096;

impl HubHandle {
    pub fn start(
        command_capacity: usize,
        max_buffered_live: usize,
        max_outbound_bytes: usize,
        mut shutdown: watch::Receiver<bool>,
    ) -> (Self, JoinHandle<()>) {
        let (sender, mut receiver) = mpsc::channel(command_capacity.max(1));
        let handle = Self { sender };
        let task = tokio::spawn(async move {
            let mut hub = Hub {
                connections: HashMap::new(),
                subscriptions: HashMap::new(),
                index: HashMap::new(),
                max_buffered_live: max_buffered_live.max(1),
                max_outbound_bytes: max_outbound_bytes.max(1),
                recent_ephemeral_ids: HashSet::new(),
                recent_ephemeral_order: VecDeque::new(),
            };
            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            hub.shutdown();
                            break;
                        }
                    }
                    command = receiver.recv() => {
                        let Some(command) = command else {
                            hub.shutdown();
                            break;
                        };
                        hub.handle(command);
                    }
                }
            }
        });
        (handle, task)
    }

    pub async fn add_connection(
        &self,
        connection_id: ConnectionId,
        queue_capacity: usize,
    ) -> Result<ConnectionChannels, GatewayError> {
        let (outbound, receiver) = mpsc::channel(queue_capacity.max(1));
        let (close, close_receiver) = watch::channel(false);
        let (response, acknowledged) = oneshot::channel();
        self.sender
            .send(HubCommand::AddConnection {
                connection_id,
                outbound,
                close,
                response,
            })
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))?;
        acknowledged
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))?;
        Ok(ConnectionChannels {
            outbound: receiver,
            close: close_receiver,
        })
    }

    #[cfg(test)]
    pub async fn register(
        &self,
        connection_id: ConnectionId,
        subscription_id: String,
        generation: u64,
        filters: Vec<Filter>,
    ) -> Result<bool, GatewayError> {
        self.register_for(
            connection_id,
            subscription_id,
            generation,
            filters,
            HashSet::new(),
        )
        .await
    }

    pub async fn register_for(
        &self,
        connection_id: ConnectionId,
        subscription_id: String,
        generation: u64,
        filters: Vec<Filter>,
        read_pubkeys: HashSet<String>,
    ) -> Result<bool, GatewayError> {
        let (response, acknowledged) = oneshot::channel();
        self.sender
            .send(HubCommand::Register {
                key: SubscriptionKey {
                    connection_id,
                    subscription_id,
                },
                generation,
                filters,
                read_pubkeys,
                response,
            })
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))?;
        acknowledged
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))
    }

    pub async fn history_ready(
        &self,
        connection_id: ConnectionId,
        subscription_id: String,
        generation: u64,
        high_water: i64,
        events: Vec<StoredEvent>,
    ) {
        let _ = self
            .sender
            .send(HubCommand::HistoryReady {
                key: SubscriptionKey {
                    connection_id,
                    subscription_id,
                },
                generation,
                high_water,
                events,
            })
            .await;
    }

    pub async fn remove(&self, connection_id: ConnectionId, subscription_id: String) {
        let _ = self
            .sender
            .send(HubCommand::Remove {
                key: SubscriptionKey {
                    connection_id,
                    subscription_id,
                },
            })
            .await;
    }

    pub async fn close_subscription(
        &self,
        connection_id: ConnectionId,
        subscription_id: String,
        message: String,
    ) {
        let _ = self
            .sender
            .send(HubCommand::CloseSubscription {
                key: SubscriptionKey {
                    connection_id,
                    subscription_id,
                },
                message,
            })
            .await;
    }

    pub async fn remove_connection(&self, connection_id: ConnectionId) {
        let _ = self
            .sender
            .send(HubCommand::RemoveConnection { connection_id })
            .await;
    }

    pub async fn publish(&self, event: PublishedEvent, now: u64) -> Result<(), GatewayError> {
        self.sender
            .send(HubCommand::Publish { event, now })
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))
    }
}

impl Hub {
    fn handle(&mut self, command: HubCommand) {
        match command {
            HubCommand::AddConnection {
                connection_id,
                outbound,
                close,
                response,
            } => {
                if self.connections.contains_key(&connection_id) {
                    self.close_connection(connection_id);
                }
                self.connections.insert(
                    connection_id,
                    ConnectionSink {
                        outbound,
                        close,
                        subscriptions: HashSet::new(),
                    },
                );
                let _ = response.send(());
            }
            HubCommand::Register {
                key,
                generation,
                filters,
                read_pubkeys,
                response,
            } => {
                let registered = self.register(key, generation, filters, read_pubkeys);
                let _ = response.send(registered);
            }
            HubCommand::HistoryReady {
                key,
                generation,
                high_water,
                events,
            } => self.history_ready(&key, generation, high_water, events),
            HubCommand::Remove { key } => self.remove_subscription(&key),
            HubCommand::CloseSubscription { key, message } => {
                self.send_one(
                    key.connection_id,
                    wire::closed_message(&key.subscription_id, &message),
                );
                self.remove_subscription(&key);
            }
            HubCommand::RemoveConnection { connection_id } => self.close_connection(connection_id),
            HubCommand::Publish { event, now } => self.publish(event, now),
        }
    }

    fn register(
        &mut self,
        key: SubscriptionKey,
        generation: u64,
        filters: Vec<Filter>,
        read_pubkeys: HashSet<String>,
    ) -> bool {
        if !self.connections.contains_key(&key.connection_id) {
            return false;
        }
        self.remove_subscription(&key);
        let index_keys = subscription_index_keys(&filters);
        for index_key in &index_keys {
            self.index
                .entry(index_key.clone())
                .or_default()
                .insert(key.clone());
        }
        self.subscriptions.insert(
            key.clone(),
            Subscription {
                generation,
                filters,
                read_pubkeys,
                index_keys,
                state: SubscriptionState::Buffering {
                    events: Vec::new(),
                    ids: HashSet::new(),
                },
            },
        );
        if let Some(connection) = self.connections.get_mut(&key.connection_id) {
            connection.subscriptions.insert(key.subscription_id.clone());
        }
        true
    }

    fn history_ready(
        &mut self,
        key: &SubscriptionKey,
        generation: u64,
        high_water: i64,
        events: Vec<StoredEvent>,
    ) {
        let Some(subscription) = self.subscriptions.get_mut(key) else {
            return;
        };
        if subscription.generation != generation {
            return;
        }
        let SubscriptionState::Buffering {
            events: buffered, ..
        } = &mut subscription.state
        else {
            return;
        };
        let historical_ids = events
            .iter()
            .map(|stored| stored.event.id.clone())
            .collect::<HashSet<_>>();
        let live = buffered
            .drain(..)
            .filter(|published| {
                !historical_ids.contains(&published.event.id)
                    && published
                        .ingest_seq
                        .is_none_or(|ingest_seq| ingest_seq > high_water)
            })
            .collect::<Vec<_>>();
        let live_high_water = live
            .iter()
            .filter_map(|published| published.ingest_seq)
            .fold(high_water, i64::max);
        subscription.state = SubscriptionState::Live {
            high_water: live_high_water,
        };

        for stored in &events {
            if !self.send_one(
                key.connection_id,
                wire::event_message(&key.subscription_id, &stored.event),
            ) {
                break;
            }
        }
        if !self.send_one(key.connection_id, wire::eose_message(&key.subscription_id)) {
            return;
        }
        for published in &live {
            if !self.send_one(
                key.connection_id,
                wire::event_message(&key.subscription_id, &published.event),
            ) {
                break;
            }
        }
    }

    fn publish(&mut self, published: PublishedEvent, now: u64) {
        if published.event.kind == 22_242 || published.event.is_expired(now) {
            return;
        }
        if published.ingest_seq.is_none() && !self.accept_ephemeral_id(&published.event.id) {
            return;
        }
        let candidates = event_index_keys(&published.event)
            .into_iter()
            .filter_map(|index_key| self.index.get(&index_key))
            .flatten()
            .cloned()
            .collect::<HashSet<_>>();
        let mut outbound = Vec::new();
        let mut overflowed_connections = HashSet::new();
        for key in candidates {
            let Some(subscription) = self.subscriptions.get_mut(&key) else {
                continue;
            };
            if !matches_any(&subscription.filters, &published.event) {
                continue;
            }
            if !event_visible_to_reader(&published.event, &subscription.read_pubkeys) {
                continue;
            }
            match &mut subscription.state {
                SubscriptionState::Buffering { events, ids } => {
                    if ids.insert(published.event.id.clone()) {
                        if events.len() >= self.max_buffered_live {
                            overflowed_connections.insert(key.connection_id);
                        } else {
                            events.push(published.clone());
                        }
                    }
                }
                SubscriptionState::Live { high_water } => {
                    if published
                        .ingest_seq
                        .is_some_and(|ingest_seq| ingest_seq <= *high_water)
                    {
                        continue;
                    }
                    if let Some(ingest_seq) = published.ingest_seq {
                        *high_water = ingest_seq;
                    }
                    outbound.push((
                        key.connection_id,
                        wire::event_message(&key.subscription_id, &published.event),
                    ));
                }
            }
        }
        for connection_id in overflowed_connections {
            self.close_connection(connection_id);
        }
        for (connection_id, message) in outbound {
            self.send_one(connection_id, message);
        }
    }

    fn send_one(&mut self, connection_id: ConnectionId, message: String) -> bool {
        if message.len() > self.max_outbound_bytes {
            self.close_connection(connection_id);
            return false;
        }
        let sent = self
            .connections
            .get(&connection_id)
            .is_some_and(|connection| connection.outbound.try_send(message).is_ok());
        if !sent {
            self.close_connection(connection_id);
        }
        sent
    }

    fn remove_subscription(&mut self, key: &SubscriptionKey) {
        let Some(subscription) = self.subscriptions.remove(key) else {
            return;
        };
        for index_key in subscription.index_keys {
            if let Some(bucket) = self.index.get_mut(&index_key) {
                bucket.remove(key);
                if bucket.is_empty() {
                    self.index.remove(&index_key);
                }
            }
        }
        if let Some(connection) = self.connections.get_mut(&key.connection_id) {
            connection.subscriptions.remove(&key.subscription_id);
        }
    }

    fn close_connection(&mut self, connection_id: ConnectionId) {
        let Some(connection) = self.connections.remove(&connection_id) else {
            return;
        };
        let _ = connection.close.send(true);
        for subscription_id in connection.subscriptions {
            self.remove_subscription(&SubscriptionKey {
                connection_id,
                subscription_id,
            });
        }
    }

    fn shutdown(&mut self) {
        let connection_ids = self.connections.keys().copied().collect::<Vec<_>>();
        for connection_id in connection_ids {
            self.close_connection(connection_id);
        }
    }

    fn accept_ephemeral_id(&mut self, event_id: &str) -> bool {
        let now = Instant::now();
        while self
            .recent_ephemeral_order
            .front()
            .is_some_and(|(_, seen)| {
                now.duration_since(*seen) >= EPHEMERAL_DEDUP_WINDOW
                    || self.recent_ephemeral_order.len() >= MAX_RECENT_EPHEMERAL_IDS
            })
        {
            if let Some((expired, _)) = self.recent_ephemeral_order.pop_front() {
                self.recent_ephemeral_ids.remove(&expired);
            }
        }
        if !self.recent_ephemeral_ids.insert(event_id.to_owned()) {
            return false;
        }
        self.recent_ephemeral_order
            .push_back((event_id.to_owned(), now));
        true
    }
}

pub(crate) fn event_visible_to_reader(event: &Event, readers: &HashSet<String>) -> bool {
    match event.kind {
        1_059 | 21_059 => {
            let recipients = event.tag_values("p").collect::<Vec<_>>();
            recipients.len() == 1 && readers.contains(recipients[0])
        }
        24_200 | 30_622 | 44_200 => event
            .tag_values("p")
            .next()
            .is_some_and(|recipient| readers.contains(recipient)),
        30_300 | 30_350 => readers.contains(&event.pubkey),
        3_187 | 30_186 => nostr::run::record_visible(event, readers),
        30_181 => {
            if event.tags.iter().any(|tag| {
                tag.name() == Some("t") && tag.value() == Some(nostr::cap::PRIVATE_POLICY_MARKER)
            }) {
                nostr::cap::private_policy_visible(event, readers)
            } else {
                true
            }
        }
        30_174 => {
            readers.contains(&event.pubkey)
                || event.tag_values("p").any(|owner| readers.contains(owner))
        }
        30_175 | 30_178 => {
            readers.contains(&event.pubkey)
                || event
                    .tags
                    .iter()
                    .any(|tag| tag.as_slice() == ["shared", "true"])
        }
        _ => true,
    }
}

fn subscription_index_keys(filters: &[Filter]) -> HashSet<IndexKey> {
    let mut keys = HashSet::new();
    for filter in filters {
        let mut indexed = false;
        if let Some(ids) = &filter.ids {
            indexed = true;
            keys.extend(ids.iter().cloned().map(IndexKey::Id));
        }
        if let Some(authors) = &filter.authors {
            indexed = true;
            keys.extend(authors.iter().cloned().map(IndexKey::Author));
        }
        if let Some(kinds) = &filter.kinds {
            indexed = true;
            keys.extend(kinds.iter().copied().map(IndexKey::Kind));
        }
        for (name, values) in &filter.tags {
            indexed = true;
            keys.extend(
                values
                    .iter()
                    .cloned()
                    .map(|value| IndexKey::Tag(name.clone(), value)),
            );
        }
        if !indexed {
            keys.insert(IndexKey::Broad);
        }
    }
    keys
}

fn event_index_keys(event: &Event) -> HashSet<IndexKey> {
    let mut keys = HashSet::from([
        IndexKey::Broad,
        IndexKey::Id(event.id.clone()),
        IndexKey::Author(event.pubkey.clone()),
        IndexKey::Kind(event.kind),
    ]);
    keys.extend(
        event
            .indexed_tags()
            .map(|(name, value)| IndexKey::Tag(name.to_owned(), value.to_owned())),
    );
    keys
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
        time::Duration,
    };

    use serde_json::Value;
    use tokio::{sync::watch, time::timeout};

    use crate::{
        domain::{Event, Filter, RelaySigner, Tag},
        store::StoredEvent,
    };

    #[test]
    fn a_run_record_is_visible_to_its_author_and_recipient_only() {
        let author = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let recipient = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let owner = RelaySigner::from_secret_hex(&"33".repeat(32)).unwrap();
        let mailbox = "ab".repeat(32);
        let event = author.sign(
            20,
            nostr::run::RECORD_KIND,
            vec![
                Tag::new(vec!["p".into(), recipient.pubkey().into()]),
                Tag::new(vec!["h".into(), mailbox]),
                Tag::new(vec!["t".into(), nostr::run::MARKER.into()]),
            ],
            "ciphertext".into(),
        );
        assert!(event_visible_to_reader(
            &event,
            &HashSet::from([author.pubkey().to_string()])
        ));
        assert!(event_visible_to_reader(
            &event,
            &HashSet::from([recipient.pubkey().to_string()])
        ));
        assert!(!event_visible_to_reader(
            &event,
            &HashSet::from([owner.pubkey().to_string()])
        ));
        assert!(!event_visible_to_reader(&event, &HashSet::new()));
    }

    use super::{
        HubHandle, IndexKey, PublishedEvent, event_index_keys, event_visible_to_reader,
        subscription_index_keys,
    };

    #[test]
    fn malformed_wraps_are_never_live_visible() {
        let recipient = "a".repeat(64);
        let readers = HashSet::from([recipient.clone()]);

        let mut valid_wrap = event('b', 10, 1_059);
        valid_wrap.tags = vec![crate::domain::Tag::new(vec!["p".into(), recipient.clone()])];
        assert!(event_visible_to_reader(&valid_wrap, &readers));
        valid_wrap
            .tags
            .push(crate::domain::Tag::new(vec!["p".into(), "b".repeat(64)]));
        assert!(!event_visible_to_reader(&valid_wrap, &readers));

        let mut ephemeral = event('c', 11, 21_059);
        ephemeral.tags = vec![crate::domain::Tag::new(vec!["p".into(), recipient])];
        assert!(event_visible_to_reader(&ephemeral, &readers));
        ephemeral.tags.clear();
        assert!(!event_visible_to_reader(&ephemeral, &readers));
    }

    #[tokio::test]
    async fn indexed_fanout_buffers_and_deduplicates_the_eose_handoff() {
        let (shutdown, receiver) = watch::channel(false);
        let (hub, task) = HubHandle::start(32, 8, 128 * 1024, receiver);
        let mut channels = hub.add_connection(1, 8).await.unwrap();
        let filter = Filter {
            kinds: Some(vec![1]),
            ..Filter::default()
        };
        assert!(
            hub.register(1, "sub".into(), 1, vec![filter])
                .await
                .unwrap()
        );

        let historical = event('a', 10, 1);
        let after_boundary = event('b', 11, 1);
        hub.publish(
            PublishedEvent {
                event: Arc::new(historical.clone()),
                ingest_seq: Some(5),
            },
            100,
        )
        .await
        .unwrap();
        hub.publish(
            PublishedEvent {
                event: Arc::new(after_boundary.clone()),
                ingest_seq: Some(6),
            },
            100,
        )
        .await
        .unwrap();
        hub.history_ready(
            1,
            "sub".into(),
            1,
            5,
            vec![StoredEvent {
                event: historical.clone(),
                ingest_seq: 5,
            }],
        )
        .await;

        let first = receive_json(&mut channels.outbound).await;
        let second = receive_json(&mut channels.outbound).await;
        let third = receive_json(&mut channels.outbound).await;
        assert_eq!(first[0], "EVENT");
        assert_eq!(first[2]["id"], "a".repeat(64));
        assert_eq!(second, serde_json::json!(["EOSE", "sub"]));
        assert_eq!(third[0], "EVENT");
        assert_eq!(third[2]["id"], "b".repeat(64));
        for (event, ingest_seq) in [(historical, 5), (after_boundary, 6)] {
            hub.publish(
                PublishedEvent {
                    event: Arc::new(event),
                    ingest_seq: Some(ingest_seq),
                },
                100,
            )
            .await
            .unwrap();
        }
        assert!(
            timeout(Duration::from_millis(20), channels.outbound.recv())
                .await
                .is_err()
        );

        shutdown.send(true).unwrap();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn full_send_queue_closes_only_its_connection() {
        let (shutdown, receiver) = watch::channel(false);
        let (hub, task) = HubHandle::start(32, 2, 128 * 1024, receiver);
        let mut channels = hub.add_connection(1, 1).await.unwrap();
        assert!(
            hub.register(1, "sub".into(), 1, vec![Filter::default()])
                .await
                .unwrap()
        );
        hub.history_ready(1, "sub".into(), 1, 0, Vec::new()).await;
        assert_eq!(
            receive_json(&mut channels.outbound).await,
            serde_json::json!(["EOSE", "sub"])
        );
        for id in ['a', 'b'] {
            hub.publish(
                PublishedEvent {
                    event: Arc::new(event(id, 10, 1)),
                    ingest_seq: Some(i64::from(id as u8)),
                },
                100,
            )
            .await
            .unwrap();
        }
        timeout(Duration::from_secs(1), channels.close.changed())
            .await
            .unwrap()
            .unwrap();
        assert!(*channels.close.borrow());
        shutdown.send(true).unwrap();
        task.await.unwrap();
    }

    #[test]
    fn subscription_index_covers_every_exact_selector_and_the_broad_lane() {
        let filter = Filter {
            ids: Some(vec!["a".repeat(64)]),
            authors: Some(vec!["c".repeat(64)]),
            kinds: Some(vec![1]),
            tags: BTreeMap::from([("p".to_owned(), vec!["d".repeat(64)])]),
            ..Filter::default()
        };
        let keys = subscription_index_keys(&[filter]);
        assert!(keys.contains(&IndexKey::Id("a".repeat(64))));
        assert!(keys.contains(&IndexKey::Author("c".repeat(64))));
        assert!(keys.contains(&IndexKey::Kind(1)));
        assert!(keys.contains(&IndexKey::Tag("p".to_owned(), "d".repeat(64))));
        assert!(!keys.contains(&IndexKey::Broad));
        assert_eq!(
            subscription_index_keys(&[Filter::default()]),
            std::collections::HashSet::from([IndexKey::Broad])
        );

        let mut indexed_event = event('a', 10, 1);
        indexed_event.tags = vec![crate::domain::Tag::new(vec!["p".into(), "d".repeat(64)])];
        let event_keys = event_index_keys(&indexed_event);
        assert!(event_keys.contains(&IndexKey::Broad));
        assert!(event_keys.contains(&IndexKey::Id("a".repeat(64))));
        assert!(event_keys.contains(&IndexKey::Author("c".repeat(64))));
        assert!(event_keys.contains(&IndexKey::Kind(1)));
        assert!(event_keys.contains(&IndexKey::Tag("p".to_owned(), "d".repeat(64))));
    }

    async fn receive_json(receiver: &mut tokio::sync::mpsc::Receiver<String>) -> Value {
        let message = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        serde_json::from_str(&message).unwrap()
    }

    fn event(id: char, created_at: u64, kind: u16) -> Event {
        Event {
            id: id.to_string().repeat(64),
            pubkey: "c".repeat(64),
            created_at,
            kind,
            tags: Vec::new(),
            content: String::new(),
            sig: "d".repeat(128),
        }
    }
}
