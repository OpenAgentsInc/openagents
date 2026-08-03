# SpacetimeDB as the Backend for a New Rust Nostr Relay

Date: 2026-08-03
Status: **external feasibility assessment — reference only, not implementation
authority.** Not an adoption decision, not a plan of record, and not admitted
work. Nothing here dispatches implementation.

Read [`README.md`](README.md) and
[`2026-08-03-spacetimedb-historical-usage-audit.md`](2026-08-03-spacetimedb-historical-usage-audit.md)
first. Companion assessment: [`analysis.md`](analysis.md).

Three standing repository constraints bear on this document, none of which it
resolves:

- **`apps/nostr-relay` is a retired, deleted path.** It is enforced as such by
  `scripts/google-cloud-authority-guard.mjs`, and current policy is not to
  recreate it or route work to it. Building the relay described here would
  require an explicit owner decision reopening that surface first.
- **Nostr primitives are owned by the workspace `nostr-effect` repository.**
  The Rust crate layout in §16 is a proposal against a different substrate, not
  a plan to fork the shared Effect Nostr implementation.
- **Google Cloud is the sole production infrastructure authority.** The
  transient-bus options in §9 (NATS, Redis Pub/Sub) and the Railway citation
  are named as mechanisms, not as platform selections.

Section 12 has the most immediate bearing on shipped code, and its central
claim checks out. NIP-90 is marked `unrecommended` upstream — `nips` at
2026-07-31 carries the header "this got totally out of control, prefer
use-case-specific microstandards" and the `draft` `unrecommended` `optional`
status line in `90.md`. This repository ships `packages/nip90` for the compute,
data, and labor market rails. Whether that warrants a move to narrower
OpenAgents microstandards is a protocol decision for the owner and the
`nostr-effect` lane, not something this assessment settles.


## Executive verdict

**Yes. SpacetimeDB would work well as the transactional backend and synchronization substrate for the OpenAgents Nostr relay.**

The correct architecture is **not**:

```text
Nostr client → SpacetimeDB directly
```

and it is not simply:

```text
nostr-rs-relay + replace SQLite calls with SpacetimeDB calls
```

The architecture should be:

```text
Nostr clients
      │
      │ Standard NIP WebSocket protocol
      ▼
Rust Nostr gateway
      │
      │ Private typed SDK/reducers
      ▼
SpacetimeDB
```

SpacetimeDB should own:

* canonical durable relay state;
* transactional event admission;
* deduplication;
* replaceable-event heads;
* deletion tombstones;
* relay policy and entitlements;
* typed OpenAgents projections;
* and committed-event distribution to relay gateway replicas.

The Rust gateway should own:

* the public Nostr WebSocket protocol;
* NIP-42 connection authentication;
* active `REQ`/`CLOSE` subscription state;
* rate limiting and backpressure;
* historical-query orchestration;
* per-connection event matching;
* ephemeral-event distribution;
* and protocol responses such as `OK`, `EOSE`, `CLOSED`, and `NOTICE`.

My scores are:

| Use                                                 |        Fit |
| --------------------------------------------------- | ---------: |
| OpenAgents private or authenticated agent relay     |   **9/10** |
| Paid, policy-controlled public agent relay          | **8.5/10** |
| General-purpose social relay with bounded retention | **6.5/10** |
| Unbounded public archive relay                      |   **4/10** |
| Direct SQLite-to-Spacetime backend swap             |   **5/10** |
| Purpose-built Rust relay designed around Spacetime  | **8.5/10** |

The major qualifications are:

1. Nostr’s arbitrary historical queries do not map cleanly to Spacetime subscriptions.
2. Nostr ephemeral traffic should not pass through Spacetime.
3. An unlimited archive is a poor fit for Spacetime’s in-memory state and never-compacted commit log.
4. Current self-hosted high availability and licensing need to be addressed before deploying more than one SpacetimeDB production instance.

---

# 1. What the current Rust relay actually does

`nostr-rs-relay` is a useful and mature reference implementation. The current code is version 0.10.0, uses SQLite as its primary storage engine, and contains experimental PostgreSQL support. Its networking stack is Tokio, Hyper, and Tungstenite; its storage layer is abstracted behind a `NostrRepo` trait.

Its architecture has two very different delivery paths.

## Historical path

When a client sends:

```json
["REQ", "subscription-id", { "authors": ["..."], "kinds": [1] }]
```

the relay:

1. Parses the Nostr filters.
2. Converts each filter into dynamic SQL.
3. Executes the query against SQLite or PostgreSQL.
4. Streams rows through a Tokio channel.
5. Sends each result as `EVENT`.
6. Sends `EOSE` after the historical query completes.

The repository interface is explicitly shaped around a cancellable, streaming SQL query.

The SQLite implementation contains extensive hand-written query planning. It chooses indexes for IDs, authors, kinds, timestamps, and tags; sheds expensive or slow queries; aborts queries for disconnected clients; and sends rows incrementally rather than materializing every result at once.

## Live path

New accepted events do not go back through the database query system. The relay publishes each accepted event into one process-local Tokio broadcast channel.

Each WebSocket connection then:

1. Receives the globally broadcast event.
2. Iterates over that connection’s active subscriptions.
3. Applies Nostr filter matching in memory.
4. Sends the event under every matching subscription ID.

That design is visible in the main connection loop: database query results and live broadcast events are separate branches of the `select!`, and each connection maintains its own subscription collection.

## Write path

The current SQLite transaction does considerably more than insert JSON:

* rejects duplicate event IDs;
* inserts the event and indexed tags;
* decides whether a replaceable or addressable event should supersede an older one;
* hides or removes older versions;
* applies deletion requests;
* detects deletion requests that arrived before the referenced event;
* and commits the whole state transition atomically.

The server adds policy checks around that transaction, including kind allowlists and denylists, pubkey admission, paid-relay balances, NIP-05 rules, and optional external authorization. Ephemeral events bypass storage and go directly to the live broadcast path.

That separation is fundamentally sensible:

```text
Historical results → indexed persistent query
Live results       → low-latency broadcast + local matching
```

We should preserve that conceptual split, even though we should replace most of its concrete implementation.

---

# 2. Why SpacetimeDB is a strong fit

SpacetimeDB gives us several properties that the current relay has to construct manually:

* transactional server-side mutations;
* an authoritative Rust domain module;
* ordered transaction updates;
* generated Rust and TypeScript clients;
* maintained client replicas;
* WebSocket distribution;
* and a common state layer for the relay, desktop application, web application, and mobile application.

Spacetime subscriptions provide a consistent initial snapshot, followed by transaction updates delivered atomically and in committed transaction order. That is excellent for propagating a newly accepted Nostr event, relay policy change, entitlement change, agent profile, job state, or receipt. ([SpacetimeDB][1])

A reducer can atomically perform:

```text
validate relay policy
        +
insert event
        +
insert indexed tags
        +
update replaceable head
        +
apply deletion tombstones
        +
update typed OpenAgents projection
        +
record entitlement charge
        +
emit committed-event notification
```

Reducers are isolated transactions and are the only normal mechanism for mutating Spacetime tables. They cannot perform ordinary filesystem, network, or arbitrary external side effects, which is exactly why their role should be deterministic relay-state admission rather than Lightning calls, HTTP fetching, or agent execution. ([SpacetimeDB][2])

This is materially better than inserting a Nostr event and then attempting to update five derived services asynchronously.

---

# 3. The recommended architecture

```text
                              Internet
                                 │
                  wss://relay.openagents.com
                                 │
                    ┌────────────▼────────────┐
                    │ Load balancer / proxy   │
                    │ TLS, DDoS, connection   │
                    │ and request controls    │
                    └────────────┬────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
     ┌────────▼────────┐ ┌───────▼─────────┐ ┌─────▼──────────┐
     │ Rust Gateway A  │ │ Rust Gateway B  │ │ Rust Gateway N │
     │                 │ │                 │ │                │
     │ NIP protocol    │ │ active REQs     │ │ rate limits    │
     │ NIP-42 auth     │ │ event matching  │ │ backpressure   │
     │ historical flow│ │ NIP responses   │ │ ephemeral bus  │
     └────────┬────────┘ └────────┬────────┘ └────────┬───────┘
              │                   │                   │
              └──────── private Spacetime SDK ────────┘
                                  │
                     ┌────────────▼─────────────┐
                     │ Self-hosted SpacetimeDB │
                     │ one production instance │
                     │                         │
                     │ Database: nostr-relay   │
                     │ • Nostr events          │
                     │ • indexed tags          │
                     │ • replacement heads     │
                     │ • deletion tombstones   │
                     │ • policies/entitlements │
                     │ • committed notices     │
                     │                         │
                     │ Database: OA core       │
                     │ • workrooms             │
                     │ • agent runs            │
                     │ • tasks/approvals       │
                     │ • fleet state           │
                     └──────┬───────────┬──────┘
                            │           │
                 ┌──────────▼───┐   ┌───▼────────────────┐
                 │ Query/index  │   │ Agent/workflow     │
                 │ projection   │   │ services           │
                 │ LMDB/RocksDB │   │ Temporal, tools,   │
                 │ or Postgres  │   │ sandboxes, models  │
                 └──────────────┘   └────────────────────┘
                            │
             ┌──────────────┼─────────────────────┐
             │              │                     │
       Object storage   Search/vector      BTC/Lightning ledger
```

## The public endpoint must be the relay, not SpacetimeDB

Spacetime’s WebSocket protocol is not NIP-01. It has different authentication, framing, query, subscription, and mutation semantics.

Public Nostr clients must communicate with the Rust gateway through standard messages such as:

```json
["EVENT", { ... }]
["REQ", "sub-id", { ... }]
["CLOSE", "sub-id"]
["AUTH", { ... }]
["COUNT", "query-id", { ... }]
```

The gateway translates those protocol operations into private Spacetime reducer calls and bounded queries.

The relay Spacetime database should be reachable only from the gateway and trusted internal services. The standard self-hosted Spacetime subscription endpoint can create identities, invoke accessible reducers, and subscribe to exposed data; it is not something to expose as an incidental public endpoint for a relay database. ([SpacetimeDB][3])

First-party OpenAgents clients may connect directly to a separate Spacetime database or module for application sync, but access to the `nostr-relay` database should remain private.

---

# 4. The event publication path

The publication flow should be:

```text
Client sends EVENT
        │
        ▼
Rust gateway parses message
        │
        ├─ enforce frame/event size
        ├─ canonicalize event
        ├─ recompute SHA-256 event ID
        ├─ verify Schnorr signature
        ├─ validate timestamp
        ├─ enforce NIP-42/NIP-70
        ├─ apply IP/pubkey rate limits
        └─ apply cheap relay policy checks
        │
        ▼
Call Spacetime `admit_event` reducer
        │
        ▼
Atomic Spacetime transaction
        │
        ├─ deduplicate ID
        ├─ classify event kind
        ├─ enforce durable relay policy
        ├─ check entitlement
        ├─ apply replacement rules
        ├─ apply deletion/tombstone rules
        ├─ insert event and indexed tags
        ├─ update typed projections
        ├─ assign ingest sequence
        └─ emit committed-event notice
        │
        ▼
Reducer commits successfully
        │
        ├─ origin gateway returns OK
        └─ all gateways receive committed notice
```

The gateway must not send:

```json
["OK", "<id>", true, ""]
```

until Spacetime has committed the event. If the internal database connection is unavailable, the safe response is an `error:` result or disconnection—not optimistic acceptance.

## Duplicate and stale replaceable outcomes

Spacetime reducer calls do not naturally behave like arbitrary request-response procedures returning a rich application object. We should use one of two designs:

### Simpler design

A reducer failure carries a machine-readable reason:

```text
duplicate: already have this event
invalid: older replaceable event
blocked: relay policy rejected event
restricted: authenticated pubkey lacks permission
```

The gateway maps reducer success or failure to the Nostr `OK` message.

### More expressive design

Pass an `ingest_request_id` to the reducer and create a small result row:

```text
relay_publish_result
├── request_id
├── gateway_id
├── event_id
├── result
├── accepted
└── committed_at
```

The gateway awaits the matching result and the rows are periodically removed.

I would begin with reducer success/errors. Add explicit result rows only if we need more detailed accounting or idempotency receipts.

---

# 5. Replacement and deletion semantics fit reducers extremely well

Nostr’s storage rules are transactional state-machine rules, not merely database inserts.

NIP-01 distinguishes:

* regular events;
* replaceable events;
* ephemeral events;
* addressable events.

For replaceable events, the retained head is selected by author and kind. Addressable events additionally include the `d` tag. When two replaceable events have the same timestamp, the lexicographically lower event ID wins.

A reducer can calculate:

```text
replaceable_address =
    hash(pubkey || kind || d_tag_or_empty)
```

and atomically update:

```text
ReplaceableHead[address] = {
    event_id,
    created_at
}
```

The transaction compares:

```text
candidate.created_at > existing.created_at

or

candidate.created_at == existing.created_at
and candidate.id < existing.id
```

That prevents races when two gateway processes receive competing events simultaneously.

## Deletion requests

NIP-09 deletion events can reference both event IDs and addressable events. The deletion request must itself remain available, even when referenced content is hidden or removed. An `a`-tag deletion applies to versions up to the deletion request’s timestamp.

We should maintain explicit tombstones:

```text
DeletionTombstone
├── target_key
├── deleting_pubkey
├── maximum_deleted_created_at
├── deletion_event_id
└── recorded_at
```

This handles the important reverse-order case:

```text
1. Deletion request arrives.
2. Original event is not present.
3. Original event is later submitted.
4. Relay must not resurrect it.
```

The existing relay already contains a form of this check for ID-based deletions. Spacetime lets us make it a first-class state transition and extend it correctly to current `a`-tag semantics.

---

# 6. The hardest part: historical `REQ` queries

This is the main architectural mismatch.

A Nostr `REQ` means:

1. Return historical events matching one or more arbitrary filters.
2. Respect each filter’s historical `limit`.
3. Order limited results newest-first, with the event-ID tie rule.
4. Deduplicate events that match several filters.
5. Send `EOSE`.
6. Continue delivering future matching events.
7. Ignore `limit` for the future-event phase.

Current NIP-01 filters include IDs, authors, kinds, indexed tags, `since`, `until`, and `limit`; conditions within a filter are ANDed, while multiple filters are ORed. Current NIP-01 also requires full 64-character IDs and authors for standard filters.

The present `nostr-rs-relay` still contains prefix matching in its in-memory filter implementation, so that should not be copied mechanically into the new relay.

## Why one Spacetime subscription per Nostr `REQ` is wrong

It is tempting to translate:

```json
["REQ", "sub", { "kinds": [1], "authors": ["abc..."] }]
```

into a Spacetime subscription.

That creates several problems:

* Spacetime tracks and maintains the matching row set for the connection.
* Nostr’s `limit` applies only to the initial historical response.
* Spacetime `LIMIT` does not define the required Nostr ordering.
* Current Spacetime subscription SQL is intentionally constrained and does not provide a general arbitrary-query planner with Nostr’s ordered union semantics.
* A client may create many concurrent Nostr subscriptions.
* Public clients should not have Spacetime identities or database subscriptions.
* Each Nostr subscription is scoped to one WebSocket connection and subscription ID, not to a durable application client cache.

Spacetime subscription SQL supports useful predicates and bounded joins, but it is not an arbitrary ordered-query language, and `LIMIT` is not an ordering guarantee. ([SpacetimeDB][4])

Therefore:

> **Spacetime should distribute the live committed stream to a small number of relay gateways—not maintain one Spacetime subscription for every public Nostr `REQ`.**

## Preserve a separate historical query engine

Define a relay-facing interface:

```rust
#[async_trait]
pub trait RelayQueryEngine: Send + Sync {
    async fn query_page(
        &self,
        filters: &[NostrFilter],
        cursor: Option<QueryCursor>,
        budget: QueryBudget,
    ) -> Result<QueryPage, QueryError>;

    async fn count(
        &self,
        filters: &[NostrFilter],
        budget: QueryBudget,
    ) -> Result<CountResult, QueryError>;
}
```

The `QueryBudget` should include:

```rust
pub struct QueryBudget {
    pub max_events: usize,
    pub max_candidate_rows: usize,
    pub max_response_bytes: usize,
    pub deadline: Instant,
}
```

This provides three implementation choices.

### Choice A: Spacetime-backed query procedure

Maintain purpose-built indexes inside Spacetime and expose a bounded server-side query procedure.

Advantages:

* one canonical data system;
* no secondary indexer;
* straightforward consistency;
* lower operational complexity initially.

Disadvantages:

* Rust procedures are currently marked unstable;
* query result size must be tightly bounded;
* deployment becomes coupled to the exact Spacetime version;
* general Nostr search remains awkward. ([SpacetimeDB][5])

This is feasible for an internal or strongly policy-controlled relay if we version-pin Spacetime and isolate the procedure behind `RelayQueryEngine`.

### Choice B: bounded SQL adapter

Use Spacetime’s SQL/PGWire facilities to retrieve a bounded candidate set, then sort, combine, and deduplicate in the Rust gateway.

This is acceptable only when relay policy guarantees small candidate sets. PGWire currently supports a limited SQL surface, the simple query protocol rather than full parameterized PostgreSQL behavior, and does not supply live subscriptions. ([SpacetimeDB][6])

I would not use this as the long-term public-relay query engine.

### Choice C: derived read projection — strongest at scale

A `relay-indexer` consumes the committed event sequence and maintains a purpose-built read store in:

* LMDB;
* RocksDB;
* `redb`;
* PostgreSQL;
* or another indexed query database.

Spacetime remains authoritative. The read store is disposable and rebuildable.

```text
Spacetime canonical event
          │
          ▼
Monotonic ingest sequence
          │
          ▼
Relay indexer
          │
          ├─ ID index
          ├─ author/time index
          ├─ kind/time index
          ├─ author/kind/time index
          ├─ tag/value/time index
          └─ Negentropy ordering index
```

For a serious public relay, **this is my preferred eventual architecture**. Nostr relay querying is specialized enough that a specialized read projection is not architectural failure; it is ordinary CQRS.

It also lets us use the strongest parts of the existing relay’s indexing and query tests without making SQLite the canonical system.

---

# 7. Race-free `EOSE` handling

Historical query and live delivery must not leave a gap.

Suppose:

1. The client sends `REQ`.
2. The gateway starts a historical query.
3. A matching event commits while the query is running.
4. The historical query finishes.
5. The gateway sends `EOSE`.

Without coordination, the event may be omitted or delivered twice.

The gateway should use this sequence:

```text
1. Register the Nostr subscription locally.
2. Begin buffering matching committed events.
3. Capture/query from a defined historical boundary.
4. Execute the historical query.
5. Send historical events, deduplicating by event ID.
6. Send EOSE.
7. Flush buffered live events not already sent.
8. Continue direct live delivery.
```

Every persisted event should receive a monotonically increasing `ingest_seq`. That sequence provides:

* gateway reconnect catch-up;
* query/live handoff boundaries;
* replay diagnostics;
* cross-gateway ordering;
* and deterministic indexer progress.

Spacetime transaction updates are delivered in commit order, so this sequence aligns naturally with its synchronization model. ([SpacetimeDB][1])

---

# 8. Live event fanout

The current Rust relay uses a process-local Tokio broadcast channel. That works for one relay process but does not naturally propagate an event accepted by Gateway A to clients connected to Gateway B.

Spacetime improves that architecture:

```text
Gateway A admits event
        │
        ▼
Spacetime transaction commits
        │
        ▼
CommittedEvent emitted
        │
        ├────────▶ Gateway A
        ├────────▶ Gateway B
        └────────▶ Gateway N
```

Every gateway maintains only its own socket connections and subscriptions. It receives the common committed stream and matches each event against its local subscription index.

## Use a Spacetime event table for low-latency notices

A Spacetime event table is useful for:

```rust
CommittedNostrEvent {
    ingest_seq,
    event_id,
    raw_json,
    pubkey,
    kind,
    created_at,
    indexed_tags,
}
```

Event-table rows are emitted after commit and are not retained in the ordinary client cache. That prevents every gateway from accumulating the complete relay archive merely to receive new inserts. ([SpacetimeDB][7])

## But also keep durable catch-up state

Event-table notifications alone are insufficient because a disconnected gateway can miss them.

Every durable event must also live in the ordinary `NostrEvent` table with `ingest_seq`. A recovering gateway does:

```text
last processed sequence = 8,512,301
current sequence        = 8,512,917

query events 8,512,302 through 8,512,917
then resume event-table feed
```

If a gateway loses the internal feed and cannot catch up safely, it should close its public Nostr sockets. Clients will reconnect and issue fresh `REQ`s, which is safer than silently losing live events.

## Improve matching beyond the reference relay

The current relay loops through each connection’s subscriptions for every event. A new relay should maintain an indexed `SubscriptionIndex` inside each gateway:

```text
by event ID
by author
by kind
by tag name/value
broad subscriptions
authenticated recipient subscriptions
```

For an incoming event, the gateway first computes a candidate set of subscription handles, then runs the complete filter predicate only against those candidates.

This matters more than the choice between Hyper and Axum once the relay has many sockets and subscriptions.

---

# 9. Nostr ephemeral events must bypass Spacetime

This is a non-negotiable architectural distinction.

NIP-01 defines kinds `20000–29999` as ephemeral: relays are not expected to persist them.

Spacetime event tables appear ephemeral from the table-state perspective, but their inserts are still recorded in the commit log. Spacetime’s commit log is its durability record and current documentation says it is never compacted. All normal table state is also maintained in memory and persisted through that log. ([SpacetimeDB][7])

Therefore this is wrong:

```text
Nostr ephemeral event
        ↓
Spacetime event table
        ↓
gateway fanout
```

It would keep supposedly ephemeral payloads in the Spacetime commit log and grow persistent disk usage for traffic that has no durable value.

Use:

```text
Nostr ephemeral event
        ↓
Rust gateway verification and policy
        ↓
Transient internal bus
        ↓
all gateway replicas
        ↓
matching Nostr clients
```

Suitable internal mechanisms include:

* NATS Core;
* a dedicated Tokio/QUIC relay mesh;
* Redis Pub/Sub;
* or another explicitly non-durable transport.

Do not use NATS JetStream or another durable stream unless the specific ephemeral kind actually needs recovery.

Also avoid passing the full ephemeral event through a “no-op” reducer merely to check policy. Cache the required policy in each gateway and update it through Spacetime subscriptions.

---

# 10. Recommended Spacetime schema

## Durable event table

```rust
#[table(name = nostr_event)]
pub struct NostrEventRow {
    #[primary_key]
    pub id: Hash,

    #[index(btree)]
    pub pubkey: Hash,

    pub created_at: u64,
    pub reverse_created_at: u64,
    pub kind: u32,

    pub received_at: u64,
    pub ingest_seq: u64,

    pub expires_at: u64,       // 0 means none
    pub address_key: Hash,     // zero for non-addressable events

    pub raw_json: String,
}
```

`reverse_created_at` can be:

```rust
u64::MAX - created_at
```

so indexes naturally traverse newest-first where supported.

## Indexed tags

Do not expect to query `Vec<Vec<String>>` efficiently. Spacetime indexes scalar columns rather than arbitrary nested vectors, so searchable Nostr tags should be normalized into their own table. ([SpacetimeDB][8])

```rust
#[table(name = nostr_indexed_tag)]
pub struct NostrIndexedTag {
    #[primary_key]
    #[auto_inc]
    pub row_id: u64,

    pub event_id: Hash,
    pub name: String,
    pub value: String,

    // Denormalized for useful compound indexes:
    pub pubkey: Hash,
    pub kind: u32,
    pub reverse_created_at: u64,
}
```

Indexes should cover the principal Nostr access patterns:

```text
(event_id)
(pubkey, reverse_created_at)
(kind, reverse_created_at)
(pubkey, kind, reverse_created_at)
(name, value, reverse_created_at)
(name, value, kind, reverse_created_at)
(ingest_seq)
```

Only the first value of a single-letter tag is conventionally indexed under NIP-01. The exact complete tags remain in the stored event JSON.

## Replacement head

```rust
#[table(name = replaceable_head)]
pub struct ReplaceableHead {
    #[primary_key]
    pub address_key: Hash,

    pub event_id: Hash,
    pub pubkey: Hash,
    pub kind: u32,
    pub d_tag: String,
    pub created_at: u64,
}
```

## Deletion tombstone

```rust
#[table(name = deletion_tombstone)]
pub struct DeletionTombstone {
    #[primary_key]
    pub target_key: Hash,

    pub author: Hash,
    pub deletion_event_id: Hash,
    pub maximum_deleted_created_at: u64,
}
```

## Relay policy and entitlements

```text
RelayPolicy
RelayKindPolicy
RelayPubkeyPolicy
RelayPubkeyEntitlement
RelayRateClass
RelayPaymentProjection
```

Actual Bitcoin and Lightning accounting should remain in the dedicated audited money system. Spacetime can hold an entitlement or balance projection used for relay admission, but it should not become the only authoritative financial ledger.

## Typed OpenAgents projections

For OpenAgents-specific events, reducers should update typed tables in the same transaction:

```text
Nostr event                    Typed projection
────────────────────────────────────────────────────
Agent profile             →    AgentProfile
Provider capability       →    ProviderCapability
Service offer             →    ProviderOffer
Job request               →    AgentJob
Job result                →    AgentJobResult
Acceptance/attestation    →    OutcomeReceipt
Reputation assertion      →    ReputationClaim
```

That gives external Nostr clients signed interoperable events while first-party applications query efficient typed rows through generated Spacetime bindings.

---

# 11. NIP-by-NIP fit

| NIP/function              |         Spacetime fit | Recommended implementation                                 |
| ------------------------- | --------------------: | ---------------------------------------------------------- |
| NIP-01 durable events     |             Excellent | Transactional `admit_event` reducer                        |
| NIP-01 historical `REQ`   |           Conditional | Dedicated query engine, not one STDB subscription per REQ  |
| NIP-01 live subscriptions |      Excellent hybrid | Committed-event feed to gateways; local matching           |
| NIP-09 deletion           |             Excellent | Reducer plus durable tombstones                            |
| NIP-11 relay info         |       Gateway concern | Ordinary HTTP response from Rust gateway                   |
| NIP-15 `EOSE`             |       Gateway concern | Historical/live handoff logic                              |
| NIP-20 `OK`               |       Gateway concern | Map reducer outcome to protocol result                     |
| NIP-40 expiration         |                  Good | Indexed periodic sweep plus query-time exclusion           |
| NIP-42 authentication     |       Gateway concern | Per-WebSocket challenge and authenticated-pubkey set       |
| NIP-45 count              | Good with projections | Materialized counters or bounded indexed query             |
| NIP-50 search             |            Poor alone | External full-text/semantic search service                 |
| NIP-70 protected events   |           Good hybrid | Gateway NIP-42 state plus policy check                     |
| NIP-77 Negentropy         |           Conditional | Dedicated sorted read index and gateway session            |
| Nostr ephemeral kinds     |     Poor through STDB | Transient bus bypass                                       |
| Paid relay access         |           Good hybrid | External Lightning system plus STDB entitlement projection |

## NIP-42 belongs in the gateway

NIP-42 authentication is scoped to the current WebSocket connection. The relay issues a challenge, and clients may authenticate one or more pubkeys for that connection. The event must be kind `22242`, reference the challenge and relay, and be timely and correctly signed.

That state should look like:

```rust
struct NostrConnection {
    challenge: String,
    authenticated_pubkeys: HashSet<PublicKey>,
    subscriptions: HashMap<SubscriptionId, Subscription>,
}
```

Spacetime may store durable ACLs and entitlements for those pubkeys, but it should not store each transient authentication challenge.

## NIP-45 counts

Counts can be expensive, which is why NIP-45 permits exact, approximate, and HyperLogLog-backed results and allows a relay to refuse queries.

For common OpenAgents queries, maintain projections:

```text
responses per job
offers per capability
receipts per provider
reactions per event
completed jobs per agent
```

Do not scan the entire event archive for every `COUNT`.

## NIP-50 search

NIP-50 search has relevance-ranked semantics rather than normal event-time ordering. That belongs in Meilisearch, OpenSearch, Tantivy, or a custom semantic-search service, not in the main Spacetime event table.

## NIP-77 Negentropy

Negentropy requires a set ordered by timestamp and event ID and maintains a stateful reconciliation exchange. That fits a specialized relay query index better than ordinary Spacetime client subscriptions.

The gateway should own `NEG-OPEN`, `NEG-MSG`, and `NEG-CLOSE` session state, while the read projection provides the sorted event-ID set.

---

# 12. Do not make NIP-90 the canonical OpenAgents protocol

This is important for your “backend speaks Nostr” strategy.

NIP-90, the Data Vending Machine proposal, is now explicitly marked **unrecommended**, with a warning to prefer use-case-specific microstandards. It defines the familiar `5000–7000` request, result, and feedback ranges, but the upstream specification no longer recommends treating it as a universal agent-compute protocol.

OpenAgents should:

* support NIP-90 through a compatibility adapter where useful;
* not make NIP-90 the canonical accepted-outcome protocol;
* define narrowly scoped OpenAgents event standards for capabilities, offers, requests, results, acceptance, and receipts;
* use addressable events for mutable profiles, offers, and policy documents;
* use immutable regular events for jobs, attestations, receipts, and settlement references;
* and publish those microstandards independently enough that other relays and clients can implement them.

Spacetime then acts as a typed materializer for those signed events.

---

# 13. “Backend speaks Nostr” should mean a dual representation

The most effective division is:

> **Nostr is the signed, portable interoperability envelope. SpacetimeDB is the authoritative operational projection and synchronization system.**

## Nostr-canonical data

Use Nostr for objects that should be independently signed, exported, mirrored, or consumed by outside clients:

* identities;
* agent and provider profiles;
* capability declarations;
* public service offers;
* job requests;
* job results;
* outcome acceptance attestations;
* public execution receipts;
* reputation claims;
* relay-discoverable policies.

## Spacetime-canonical data

Use Spacetime for operational state that does not benefit from global signed replication:

* device presence;
* typing and cursor state;
* unread counts;
* private drafts;
* transient run progress;
* scheduler leases;
* worker heartbeats;
* internal routing;
* rate-limit counters;
* UI projections;
* local approvals;
* encrypted internal control state;
* retry state;
* and provider health.

## Derived typed state

For each accepted OpenAgents Nostr event:

```text
Signed Nostr event
        │
        ▼
Spacetime reducer
        │
        ├─ stores signed event
        ├─ updates typed projection
        ├─ evaluates policy
        └─ emits live change
```

This avoids forcing every React component to parse and scan generic Nostr JSON while preserving protocol portability.

It also avoids the opposite mistake: putting every heartbeat, token chunk, local draft, and scheduling transition onto the public Nostr event fabric.

---

# 14. Spacetime’s storage model is the largest scale constraint

Current Spacetime documentation describes table state as held in memory and persisted to disk. Its commit log is the durability record and is not currently compacted. ([SpacetimeDB][9])

That has several consequences for a relay.

## Every retained event consumes live memory

A Nostr archive with:

* millions of events;
* several indexed tags per event;
* replacement metadata;
* deletion tombstones;
* and multiple secondary indexes

will consume considerably more memory than the raw JSON byte total.

Spacetime may still perform extremely well because it is operating on in-memory indexed state, but capacity planning is a hard bound rather than an afterthought.

## Deleting rows does not erase commit-log history

Removing expired, replaced, or deleted events reduces current table state and memory, but the historical transactions remain in the commit log.

That is acceptable for a bounded OpenAgents relay. It is much less attractive for an unfiltered public firehose receiving arbitrary social traffic indefinitely.

## Recommended data temperatures

| Data                                        | Storage                          |
| ------------------------------------------- | -------------------------------- |
| Current relay operational set               | SpacetimeDB                      |
| Current replacement heads                   | SpacetimeDB                      |
| Current deletion tombstones                 | SpacetimeDB                      |
| Current agent profiles/offers/jobs/receipts | SpacetimeDB                      |
| Old superseded events                       | Cold archive or query projection |
| Full raw traces and large outputs           | Object storage                   |
| Search corpus                               | Search service                   |
| Historical analytics                        | Warehouse/ClickHouse/PostgreSQL  |
| Ephemeral events                            | Transient bus only               |

For durable Nostr interoperability, an old event can remain in cold storage and still be retrievable through the relay query service without remaining in every hot Spacetime index.

---

# 15. Self-hosting assessment

Self-hosting Spacetime is compatible with this architecture, but it changes the risk profile.

## Publicly documented self-hosting is essentially single-node

The standard self-host guide describes running a Spacetime process on an Ubuntu host under systemd, commonly behind Nginx and TLS. Managed replication, automated backups, custom replication, and BYO-cloud deployment are presented under managed or enterprise offerings rather than as a turnkey public self-hosted cluster workflow. ([SpacetimeDB][3])

Therefore, for the community-licensed single-instance deployment, assume:

* one Spacetime failure domain;
* your own persistent-disk design;
* your own snapshots;
* your own commit-log archival;
* your own restore automation;
* your own monitoring;
* and your own tested recovery procedure.

Multiple Rust Nostr gateway replicas can still sit in front of that one instance. Those gateway processes are stateless enough to replace freely.

## Current license limitation

SpacetimeDB 2.7.1 is currently under BSL 1.1. Its Additional Use Grant allows production use with **no more than one SpacetimeDB instance**, provided the product is not a prohibited “Database Service.” The scheduled change license date is July 26, 2031.

This means that, absent a separate commercial agreement, the following likely require licensing clarification:

* an active Spacetime standby;
* two production regions;
* horizontal Spacetime shards;
* independent relay and application Spacetime hosts;
* or a blue/green arrangement where both instances are production-active.

That is not a legal conclusion; the exact meaning of “instance” and a proposed topology should be confirmed with Clockwork Labs and counsel.

A commercial BYO-cloud or on-premises license can still be fully self-hosted. “Self-hosted” does not require using only the public Additional Use Grant.

## One instance, multiple databases

A practical initial topology is one self-hosted Spacetime instance containing separate databases or modules:

```text
Spacetime instance
├── openagents-core
├── nostr-relay
├── provider-fleet
└── selected internal projections
```

Spacetime supports hosting more than one database on a host, but the license’s one-instance boundary should still be confirmed for the exact deployment. ([SpacetimeDB][10])

The disadvantage is failure coupling: the relay and first-party application share one database-host failure domain.

## My minimum self-hosted operating standard

Before treating Spacetime as canonical relay storage, prove:

1. Filesystem-consistent database snapshots.
2. Commit-log segment upload to object storage.
3. Automated restore onto a clean machine.
4. Verification of event counts, replacement heads, and ingest sequence after restore.
5. Recovery with old gateway clients reconnecting.
6. Module-version pinning and rollback.
7. Alerts for RAM, disk, commit-log growth, reducer latency, and subscriber lag.
8. A tested procedure for failing closed when Spacetime is unavailable.

If automatic failover and near-zero RPO are required immediately, obtain a topology that Clockwork officially supports under a commercial self-hosted license—or do not use Spacetime as the sole authoritative relay store.

---

# 16. Fork or rewrite `nostr-rs-relay`?

## Recommendation: fork it as a reference and conformance base, but rewrite the core architecture

The repository is MIT-licensed, so its useful portions can be reused.

### Reuse or adapt

* Nostr message parsing;
* canonical event validation tests;
* signature verification;
* filter parsing and matching tests;
* NIP-42 connection behavior;
* NIP-11 relay information;
* rate-limit concepts;
* Prometheus metric names;
* configuration patterns;
* WebSocket edge-case tests;
* replaceable/deletion test fixtures.

### Replace

* `NostrRepo`;
* SQLite/PostgreSQL query generators;
* the database writer channel;
* process-local global broadcast;
* SQL-specific migration machinery;
* payment logic coupled directly to relay storage;
* monolithic server ownership of policy, DB, WebSocket, and payments.

The current `NostrRepo` abstraction is:

```text
write one event
query one subscription by streaming SQL rows
perform SQL maintenance
manage account/invoice records
```

A Spacetime-native backend needs something closer to:

```rust
pub trait RelayState {
    async fn admit_event(
        &self,
        event: VerifiedEvent,
        context: AdmissionContext,
    ) -> Result<AdmissionOutcome>;

    async fn current_sequence(&self) -> Result<u64>;

    async fn events_after(
        &self,
        sequence: u64,
        limit: usize,
    ) -> Result<Vec<CommittedEvent>>;
}

pub trait RelayQueryEngine {
    async fn query_page(
        &self,
        filters: &[NostrFilter],
        cursor: Option<QueryCursor>,
        budget: QueryBudget,
    ) -> Result<QueryPage>;

    async fn count(
        &self,
        filters: &[NostrFilter],
        budget: QueryBudget,
    ) -> Result<CountResult>;
}

pub trait RelayLiveBus {
    async fn durable_events(&self) -> Result<CommittedEventStream>;
    async fn publish_ephemeral(&self, event: VerifiedEvent) -> Result<()>;
    async fn ephemeral_events(&self) -> Result<EphemeralEventStream>;
}
```

## Suggested crate organization

```text
crates/
├── nostr-domain
│   ├── event classification
│   ├── replacement address
│   ├── filter matching
│   ├── deletion semantics
│   └── policy vocabulary
│
├── nostr-gateway
│   ├── WebSocket protocol
│   ├── connection state
│   ├── NIP-42
│   ├── subscription index
│   ├── historical/live handoff
│   └── backpressure
│
├── nostr-spacetime-client
│   ├── generated bindings
│   ├── reducer adapter
│   ├── event feed
│   └── catch-up
│
├── nostr-query
│   ├── query planner
│   ├── limits/budgets
│   ├── dedup/order
│   └── COUNT
│
├── nostr-indexer
│   ├── LMDB/RocksDB projection
│   ├── search projection
│   └── Negentropy index
│
└── nostr-conformance
    ├── NIP fixtures
    ├── replacement races
    ├── deletion-before-event
    └── multi-gateway tests

modules/
└── nostr-relay-spacetimedb
    ├── tables
    ├── reducers
    ├── scheduled cleanup
    └── typed OA projections
```

`nostr-domain` should be pure Rust and shared where practical between the native gateway and the Spacetime module. That keeps replacement, deletion, and address derivation from drifting between the two implementations.

---

# 17. Production spike

The spike should exercise the actual architecture, not merely prove that a reducer can insert an event.

## Vertical slice

```text
Connect standard Nostr client
    → receive NIP-42 challenge
    → authenticate
    → publish durable event
    → commit through Spacetime
    → receive OK
    → query from second gateway
    → receive historical EVENT + EOSE
    → publish another event through Gateway A
    → receive it live through Gateway B
    → close subscription
```

## Required correctness tests

* Duplicate event sent concurrently to two gateways.
* Two replaceable events with different timestamps.
* Two replaceable events with the same timestamp and different IDs.
* Parameterized replaceable events with different `d` tags.
* ID deletion before the target event arrives.
* Address deletion before an older version arrives.
* Deletion event itself remains queryable.
* Expired event omitted and eventually removed.
* NIP-70 event rejected without matching NIP-42 authentication.
* Multiple filters OR correctly.
* Filter fields AND correctly.
* Per-filter historical limits.
* Correct newest-first order and ID tie ordering.
* No missing event during historical/`EOSE`/live handoff.
* No duplicate event after gateway catch-up.
* Gateway disconnects clients when internal state becomes unsafe.

## Required scale measurements

Measure:

```text
RAM per stored event
RAM per indexed tag
commit-log bytes per accepted event
commit-log bytes per rejected/duplicate operation
publish p50/p95/p99
gateway-to-gateway live propagation
historical first-result latency
historical completion latency
gateway matching CPU
Spacetime restart replay time
query-index rebuild time
restore time from clean host
```

Test at least:

* expected first-year retained event count;
* 10× expected concurrent sockets;
* 10× expected subscriptions;
* 10× expected publish rate;
* pathological broad filters;
* high-tag-count events;
* reconnect storms;
* and prolonged database unavailability.

## Hard go/no-go gates

Proceed only if:

1. Memory per retained event permits the intended retention window.
2. Commit-log growth has an affordable archival plan.
3. Backup restoration is independently proven.
4. The query engine produces exact NIP semantics.
5. Two gateway replicas do not miss cross-gateway events.
6. Nostr ephemeral payloads never enter the persistent Spacetime path.
7. The production-instance licensing topology is resolved.
8. The relay fails closed rather than acknowledging uncommitted events.

---

# Final recommendation

**Proceed with SpacetimeDB for the new OpenAgents Nostr relay.**

But define it precisely:

> **SpacetimeDB is the relay’s canonical transactional state and first-party sync engine. A separate Rust gateway is the Nostr protocol server. A dedicated query projection handles large historical and search workloads. A transient bus handles ephemeral Nostr traffic.**

That architecture gives you something the current relay does not:

* several stateless relay gateways sharing one authoritative event state;
* atomic replacement, deletion, policy, entitlement, and OpenAgents projection updates;
* the same signed events available through both standard Nostr and typed first-party clients;
* Rust domain logic on both sides of the database boundary;
* and a natural bridge from public Nostr events into desktop, web, and mobile application state.

The decisive distinction is relay scope:

* For a **bounded, authenticated, paid, agent-centric OpenAgents relay**, Spacetime is a very strong choice.
* For a **global, unlimited, archival social firehose**, Spacetime should not be the sole hot event store.
* For your broader product, where the relay is part of a Rust/TypeScript synchronized operating system rather than an isolated Nostr server, the benefits outweigh the additional architecture.

**Recommended decision: build the new relay around SpacetimeDB, but do not treat SpacetimeDB itself as the Nostr relay and do not preserve the existing relay’s SQL-shaped repository boundary.**

[1]: https://spacetimedb.com/docs/clients/subscriptions/semantics/ "https://spacetimedb.com/docs/clients/subscriptions/semantics/"
[2]: https://spacetimedb.com/docs/functions/reducers/ "https://spacetimedb.com/docs/functions/reducers/"
[3]: https://spacetimedb.com/docs/how-to/deploy/self-hosting/ "https://spacetimedb.com/docs/how-to/deploy/self-hosting/"
[4]: https://spacetimedb.com/docs/reference/sql/ "https://spacetimedb.com/docs/reference/sql/"
[5]: https://spacetimedb.com/docs/functions/procedures/ "https://spacetimedb.com/docs/functions/procedures/"
[6]: https://spacetimedb.com/docs/how-to/pg-wire/ "https://spacetimedb.com/docs/how-to/pg-wire/"
[7]: https://spacetimedb.com/docs/tables/event-tables/ "https://spacetimedb.com/docs/tables/event-tables/"
[8]: https://spacetimedb.com/docs/tables/indexes/ "https://spacetimedb.com/docs/tables/indexes/"
[9]: https://spacetimedb.com/docs/tables/ "https://spacetimedb.com/docs/tables/"
[10]: https://spacetimedb.com/docs/how-to/deploy/railway/ "https://spacetimedb.com/docs/how-to/deploy/railway/"


