use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use tokio::{
    sync::{Notify, mpsc, oneshot, watch},
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
    pub outbound: Arc<OutboundQueue>,
    pub close: watch::Receiver<bool>,
}

/// One message the hub hands a connection to write.
///
/// A live event is kept as its two serialized halves, shared with every
/// other subscriber it goes to, so the hub does no per-delivery
/// serialization; the connection's own task joins them when it writes.
#[derive(Debug, Clone)]
pub enum Outbound {
    Text(String),
    Event {
        /// The subscription ID as a JSON string, quotes included.
        subscription: Arc<str>,
        /// The event as JSON.
        event: Arc<str>,
        /// An ephemeral event may be dropped for a newer one when the
        /// connection falls behind. A stored event never is.
        ephemeral: bool,
    },
}

impl Outbound {
    fn is_ephemeral(&self) -> bool {
        matches!(
            self,
            Self::Event {
                ephemeral: true,
                ..
            }
        )
    }

    fn len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Event {
                subscription,
                event,
                ..
            } => wire::EVENT_MESSAGE_OVERHEAD + subscription.len() + event.len(),
        }
    }

    /// The text frame for this message.
    pub fn render(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Event {
                subscription,
                event,
                ..
            } => wire::event_message_from_json(&subscription, &event),
        }
    }
}

/// What happened to a message offered to a full or open queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    Queued,
    /// The queue was full; an older ephemeral event, or this ephemeral
    /// event, was dropped to make room.
    DroppedEphemeral,
    /// The queue is full of messages that may not be dropped.
    Full,
}

#[derive(Default)]
struct QueueState {
    items: VecDeque<Outbound>,
    ephemeral: usize,
}

/// A connection's bounded send queue.
///
/// The hub pushes and the connection's task pops, each holding the lock only
/// to move one message. When the queue is full, the oldest queued ephemeral
/// event makes room for the new message: a crowded room's pose frames
/// replace each other instead of closing a reader that fell behind. Stored
/// events and control messages are never dropped; when they alone fill the
/// queue, the offer fails and the hub closes the connection, as before.
pub struct OutboundQueue {
    state: Mutex<QueueState>,
    ready: Notify,
    capacity: usize,
}

impl OutboundQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(QueueState::default()),
            ready: Notify::new(),
            capacity: capacity.max(1),
        }
    }

    fn lock(&self) -> MutexGuard<'_, QueueState> {
        // The critical sections cannot panic halfway, so a poisoned lock
        // still holds a consistent queue.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn offer(&self, message: Outbound) -> Offer {
        let mut state = self.lock();
        let mut outcome = Offer::Queued;
        if state.items.len() >= self.capacity {
            if state.ephemeral > 0 {
                if let Some(oldest) = state.items.iter().position(Outbound::is_ephemeral) {
                    state.items.remove(oldest);
                    state.ephemeral -= 1;
                }
            } else if message.is_ephemeral() {
                return Offer::DroppedEphemeral;
            } else {
                return Offer::Full;
            }
            outcome = Offer::DroppedEphemeral;
        }
        let was_empty = state.items.is_empty();
        if message.is_ephemeral() {
            state.ephemeral += 1;
        }
        state.items.push_back(message);
        drop(state);
        if was_empty {
            // A waiting writer wakes; a busy one finds the permit later.
            self.ready.notify_one();
        }
        outcome
    }

    pub fn pop(&self) -> Option<Outbound> {
        let mut state = self.lock();
        let message = state.items.pop_front()?;
        if message.is_ephemeral() {
            state.ephemeral -= 1;
        }
        Some(message)
    }

    /// True when the queue holds messages and every one is an ephemeral
    /// event, which may wait a moment to share a write.
    pub fn holds_only_ephemeral(&self) -> bool {
        let state = self.lock();
        !state.items.is_empty() && state.ephemeral == state.items.len()
    }

    /// Wait until a message may be queued. Call it only after [`pop`]
    /// returned `None`.
    ///
    /// [`pop`]: Self::pop
    pub async fn ready(&self) {
        self.ready.notified().await;
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lock().items.len()
    }
}

enum HubCommand {
    AddConnection {
        connection_id: ConnectionId,
        outbound: Arc<OutboundQueue>,
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
        complete: bool,
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

/// The hub's own handle for a subscription, so the index and fan-out move
/// integers instead of cloning subscription IDs.
type Slot = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum IndexKey {
    Id(String),
    Author(String),
    Kind(u16),
    Tag(String, String),
    Broad,
}

struct Subscription {
    key: SubscriptionKey,
    /// The subscription ID as a JSON string, shared by every delivery.
    subscription_json: Arc<str>,
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
    outbound: Arc<OutboundQueue>,
    close: watch::Sender<bool>,
    subscriptions: HashSet<String>,
}

struct Hub {
    connections: HashMap<ConnectionId, ConnectionSink>,
    subscriptions: HashMap<Slot, Subscription>,
    slots: HashMap<SubscriptionKey, Slot>,
    next_slot: Slot,
    index: HashMap<IndexKey, HashSet<Slot>>,
    max_buffered_live: usize,
    max_outbound_bytes: usize,
    recent_ephemeral_ids: HashSet<String>,
    recent_ephemeral_order: VecDeque<(String, Instant)>,
    /// Reused across publishes: the candidate subscriptions of one event.
    candidates: Vec<Slot>,
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
                slots: HashMap::new(),
                next_slot: 0,
                index: HashMap::new(),
                max_buffered_live: max_buffered_live.max(1),
                max_outbound_bytes: max_outbound_bytes.max(1),
                recent_ephemeral_ids: HashSet::new(),
                recent_ephemeral_order: VecDeque::new(),
                candidates: Vec::new(),
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
        let outbound = Arc::new(OutboundQueue::new(queue_capacity));
        let (close, close_receiver) = watch::channel(false);
        let (response, acknowledged) = oneshot::channel();
        self.sender
            .send(HubCommand::AddConnection {
                connection_id,
                outbound: Arc::clone(&outbound),
                close,
                response,
            })
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))?;
        acknowledged
            .await
            .map_err(|_| GatewayError::Internal("subscription hub stopped".to_owned()))?;
        Ok(ConnectionChannels {
            outbound,
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
        complete: bool,
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
                complete,
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
                complete,
            } => self.history_ready(&key, generation, high_water, events, complete),
            HubCommand::Remove { key } => self.remove_subscription(&key),
            HubCommand::CloseSubscription { key, message } => {
                self.send_one(
                    key.connection_id,
                    Outbound::Text(wire::closed_message(&key.subscription_id, &message)),
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
        let slot = self.next_slot;
        self.next_slot = self.next_slot.wrapping_add(1);
        let index_keys = subscription_index_keys(&filters);
        for index_key in &index_keys {
            self.index
                .entry(index_key.clone())
                .or_default()
                .insert(slot);
        }
        let subscription_json = serde_json::to_string(&key.subscription_id)
            .expect("serializing a string cannot fail")
            .into();
        if let Some(connection) = self.connections.get_mut(&key.connection_id) {
            connection.subscriptions.insert(key.subscription_id.clone());
        }
        self.slots.insert(key.clone(), slot);
        self.subscriptions.insert(
            slot,
            Subscription {
                key,
                subscription_json,
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
        true
    }

    fn history_ready(
        &mut self,
        key: &SubscriptionKey,
        generation: u64,
        high_water: i64,
        events: Vec<StoredEvent>,
        complete: bool,
    ) {
        let Some(subscription) = self
            .slots
            .get(key)
            .and_then(|slot| self.subscriptions.get_mut(slot))
        else {
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
        let subscription_json = Arc::clone(&subscription.subscription_json);

        for stored in &events {
            if !self.send_one(
                key.connection_id,
                Outbound::Text(wire::event_message(&key.subscription_id, &stored.event)),
            ) {
                break;
            }
        }
        if !self.send_one(
            key.connection_id,
            Outbound::Text(wire::eose_message(&key.subscription_id, complete)),
        ) {
            return;
        }
        for published in &live {
            if !self.send_one(
                key.connection_id,
                Outbound::Event {
                    subscription: Arc::clone(&subscription_json),
                    event: wire::event_json(&published.event).into(),
                    ephemeral: published.ingest_seq.is_none(),
                },
            ) {
                break;
            }
        }
    }

    fn publish(&mut self, published: PublishedEvent, now: u64) {
        if published.event.kind == 22_242 || published.event.is_expired(now) {
            return;
        }
        let ephemeral = published.ingest_seq.is_none();
        if ephemeral && !self.accept_ephemeral_id(&published.event.id) {
            return;
        }
        let mut candidates = std::mem::take(&mut self.candidates);
        candidates.clear();
        for index_key in event_index_keys(&published.event) {
            if let Some(bucket) = self.index.get(&index_key) {
                candidates.extend(bucket.iter().copied());
            }
        }
        candidates.sort_unstable();
        candidates.dedup();

        // Serialized once, on the first live match, and shared by every
        // delivery of this event.
        let mut event_json: Option<Arc<str>> = None;
        let mut overflowed_connections = HashSet::new();
        for slot in &candidates {
            let Some(subscription) = self.subscriptions.get_mut(slot) else {
                continue;
            };
            if !matches_any(&subscription.filters, &published.event) {
                continue;
            }
            if !event_visible_to_reader(&published.event, &subscription.read_pubkeys) {
                continue;
            }
            let connection_id = subscription.key.connection_id;
            match &mut subscription.state {
                SubscriptionState::Buffering { events, ids } => {
                    if ids.insert(published.event.id.clone()) {
                        if events.len() < self.max_buffered_live {
                            events.push(published.clone());
                        } else if !ephemeral {
                            // A stored event may not be skipped: the reader
                            // reconnects and reads it from history.
                            overflowed_connections.insert(connection_id);
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
                    let event =
                        event_json.get_or_insert_with(|| wire::event_json(&published.event).into());
                    let message = Outbound::Event {
                        subscription: Arc::clone(&subscription.subscription_json),
                        event: Arc::clone(event),
                        ephemeral,
                    };
                    if !overflowed_connections.contains(&connection_id) {
                        Self::deliver(
                            &self.connections,
                            self.max_outbound_bytes,
                            connection_id,
                            message,
                            &mut overflowed_connections,
                        );
                    }
                }
            }
        }
        self.candidates = candidates;
        for connection_id in overflowed_connections {
            self.close_connection(connection_id);
        }
    }

    /// Queue `message` for a connection, or mark the connection to close
    /// when the message is too large or may not be dropped from a full
    /// queue.
    fn deliver(
        connections: &HashMap<ConnectionId, ConnectionSink>,
        max_outbound_bytes: usize,
        connection_id: ConnectionId,
        message: Outbound,
        overflowed: &mut HashSet<ConnectionId>,
    ) {
        let Some(connection) = connections.get(&connection_id) else {
            return;
        };
        if message.len() > max_outbound_bytes || connection.outbound.offer(message) == Offer::Full {
            overflowed.insert(connection_id);
        }
    }

    fn send_one(&mut self, connection_id: ConnectionId, message: Outbound) -> bool {
        if message.len() > self.max_outbound_bytes {
            self.close_connection(connection_id);
            return false;
        }
        let sent = self
            .connections
            .get(&connection_id)
            .is_some_and(|connection| connection.outbound.offer(message) != Offer::Full);
        if !sent {
            self.close_connection(connection_id);
        }
        sent
    }

    fn remove_subscription(&mut self, key: &SubscriptionKey) {
        let Some(slot) = self.slots.remove(key) else {
            return;
        };
        let Some(subscription) = self.subscriptions.remove(&slot) else {
            return;
        };
        for index_key in subscription.index_keys {
            if let Some(bucket) = self.index.get_mut(&index_key) {
                bucket.remove(&slot);
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

/// True when every filter names only ephemeral kinds (NIP-01: 20000 to
/// 29999). The relay never stores such events, so a subscription like this
/// has no history to read.
pub(crate) fn ephemeral_only(filters: &[Filter]) -> bool {
    !filters.is_empty()
        && filters.iter().all(|filter| {
            filter.kinds.as_ref().is_some_and(|kinds| {
                !kinds.is_empty()
                    && kinds.iter().all(|kind| {
                        crate::domain::EventClass::from_kind(*kind)
                            == crate::domain::EventClass::Ephemeral
                    })
            })
        })
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
        78 | 30_078 | 30_300 | 30_350 => readers.contains(&event.pubkey),
        30_179 => false,
        3_188 => nostr::private_artifact::visible(event, readers),
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
        HubHandle, IndexKey, Offer, Outbound, OutboundQueue, PublishedEvent, ephemeral_only,
        event_index_keys, event_visible_to_reader, subscription_index_keys,
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
        let channels = hub.add_connection(1, 8).await.unwrap();
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
            true,
        )
        .await;

        let first = receive_json(&channels.outbound).await;
        let second = receive_json(&channels.outbound).await;
        let third = receive_json(&channels.outbound).await;
        assert_eq!(first[0], "EVENT");
        assert_eq!(first[2]["id"], "a".repeat(64));
        assert_eq!(second, serde_json::json!(["EOSE", "sub", ["finish"]]));
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
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(channels.outbound.pop().is_none());

        shutdown.send(true).unwrap();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn a_truncated_history_announces_more_at_eose() {
        let (shutdown, receiver) = watch::channel(false);
        let (hub, task) = HubHandle::start(32, 8, 128 * 1024, receiver);
        let channels = hub.add_connection(1, 8).await.unwrap();
        assert!(
            hub.register(1, "sub".into(), 1, vec![Filter::default()])
                .await
                .unwrap()
        );
        hub.history_ready(1, "sub".into(), 1, 0, Vec::new(), false)
            .await;
        assert_eq!(
            receive_json(&channels.outbound).await,
            serde_json::json!(["EOSE", "sub", ["more"]])
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
        hub.history_ready(1, "sub".into(), 1, 0, Vec::new(), true)
            .await;
        assert_eq!(
            receive_json(&channels.outbound).await,
            serde_json::json!(["EOSE", "sub", ["finish"]])
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

    #[tokio::test]
    async fn a_crowded_room_drops_the_oldest_frames_and_keeps_the_reader() {
        let (shutdown, receiver) = watch::channel(false);
        let (hub, task) = HubHandle::start(64, 8, 128 * 1024, receiver);
        let channels = hub.add_connection(1, 4).await.unwrap();
        let filter = Filter {
            kinds: Some(vec![23_300]),
            ..Filter::default()
        };
        assert!(
            hub.register(1, "room".into(), 1, vec![filter])
                .await
                .unwrap()
        );
        hub.history_ready(1, "room".into(), 1, 0, Vec::new(), true)
            .await;
        assert_eq!(
            receive_json(&channels.outbound).await,
            serde_json::json!(["EOSE", "room", ["finish"]])
        );
        let ids = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
        for id in ids {
            hub.publish(
                PublishedEvent {
                    event: Arc::new(event(id, 10, 23_300)),
                    ingest_seq: None,
                },
                100,
            )
            .await
            .unwrap();
        }
        // The hub handles commands in order; a refused registration on an
        // unknown connection returns once every publish above is handled.
        assert!(
            !hub.register(99, "sync".into(), 1, Vec::new())
                .await
                .unwrap()
        );
        assert!(!*channels.close.borrow());
        assert_eq!(channels.outbound.len(), 4);
        for id in ['g', 'h', 'i', 'j'] {
            let message = receive_json(&channels.outbound).await;
            assert_eq!(message[0], "EVENT");
            assert_eq!(message[1], "room");
            assert_eq!(message[2]["id"], id.to_string().repeat(64));
        }
        shutdown.send(true).unwrap();
        task.await.unwrap();
    }

    #[test]
    fn stored_events_and_control_messages_are_never_dropped() {
        let message = |id: char, ephemeral: bool| Outbound::Event {
            subscription: "\"sub\"".into(),
            event: format!("{{\"id\":\"{id}\"}}").into(),
            ephemeral,
        };
        let queue = OutboundQueue::new(2);
        assert_eq!(queue.offer(message('a', true)), Offer::Queued);
        assert_eq!(queue.offer(message('b', true)), Offer::Queued);
        // A stored event takes the oldest ephemeral event's place.
        assert_eq!(queue.offer(message('c', false)), Offer::DroppedEphemeral);
        assert_eq!(
            queue.offer(Outbound::Text("[\"EOSE\",\"sub\"]".into())),
            Offer::DroppedEphemeral
        );
        // Full of what may not be dropped: a new ephemeral event is the one
        // dropped, and anything else is refused so the hub closes the
        // connection instead of skipping it.
        assert_eq!(queue.offer(message('d', true)), Offer::DroppedEphemeral);
        assert_eq!(queue.offer(message('e', false)), Offer::Full);
        assert_eq!(
            queue.pop().unwrap().render(),
            r#"["EVENT","sub",{"id":"c"}]"#
        );
        assert_eq!(queue.pop().unwrap().render(), r#"["EOSE","sub"]"#);
        assert!(queue.pop().is_none());
    }

    #[test]
    fn an_event_message_joins_its_shared_halves_into_valid_json() {
        let mut event = event('a', 10, 23_300);
        event.content = "quote \" and \\ backslash".into();
        event.tags = vec![crate::domain::Tag::new(vec!["w".into(), "room".into()])];
        let subscription = "odd \"sub\" id";
        let message = Outbound::Event {
            subscription: serde_json::to_string(subscription).unwrap().into(),
            event: super::wire::event_json(&event).into(),
            ephemeral: true,
        };
        let expected = serde_json::json!(["EVENT", subscription, event]);
        let rendered: Value = serde_json::from_str(&message.clone().render()).unwrap();
        assert_eq!(rendered, expected);
        let direct: Value =
            serde_json::from_str(&super::wire::event_message(subscription, &event)).unwrap();
        assert_eq!(direct, expected);
        assert_eq!(message.len(), message.render().len());
    }

    #[test]
    fn only_filters_that_name_only_ephemeral_kinds_skip_history() {
        let kinds = |kinds: Option<Vec<u16>>| Filter {
            kinds,
            ..Filter::default()
        };
        assert!(ephemeral_only(&[kinds(Some(vec![23_300]))]));
        assert!(ephemeral_only(&[
            kinds(Some(vec![20_000, 29_999])),
            kinds(Some(vec![23_301]))
        ]));
        assert!(!ephemeral_only(&[]));
        assert!(!ephemeral_only(&[kinds(None)]));
        assert!(!ephemeral_only(&[kinds(Some(Vec::new()))]));
        assert!(!ephemeral_only(&[kinds(Some(vec![23_300, 33_301]))]));
        assert!(!ephemeral_only(&[kinds(Some(vec![19_999]))]));
        assert!(!ephemeral_only(&[kinds(Some(vec![30_000]))]));
        assert!(!ephemeral_only(&[
            kinds(Some(vec![23_300])),
            kinds(Some(vec![1]))
        ]));
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

    async fn receive_json(queue: &OutboundQueue) -> Value {
        let message = timeout(Duration::from_secs(1), async {
            loop {
                if let Some(message) = queue.pop() {
                    return message.render();
                }
                queue.ready().await;
            }
        })
        .await
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
