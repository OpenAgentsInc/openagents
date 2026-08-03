# Feasibility Assessment: SpacetimeDB for a Rust/Zed Desktop, TanStack Web, and React Native Mobile Stack

Date: 2026-08-03
Status: **external feasibility assessment — reference only, not implementation
authority.** Not an adoption decision, not a plan of record, and not admitted
work. Nothing here dispatches implementation. It targets a prospective
Rust/Zed desktop + TanStack web + React Native mobile stack, which is not the
current supported shape (Effect Native on Electron, Expo, and the
`openagents.com` app).

Read [`README.md`](README.md) and
[`2026-08-03-spacetimedb-historical-usage-audit.md`](2026-08-03-spacetimedb-historical-usage-audit.md)
first: SpacetimeDB has been adopted and removed three times here, and reviving
its world-service authority is currently an explicit non-goal.

Two recommendations below name infrastructure this repository has since
retired — SHC (§4, sandboxed execution) and Cloudflare R2 (§4 and §11, object
storage). Google Cloud is the sole production infrastructure authority; read
those cells as "an object store" and "a container execution plane", not as
platform selections.


## Verdict

**Yes—SpacetimeDB is feasible, and your stack is unusually well aligned with it.** I would seriously consider it as the **authoritative online operational database and realtime synchronization layer**.

I would **not** use it as the entire backend or assume it provides a complete Linear-style local-first system. The recommended architecture is:

> **SpacetimeDB for hot collaborative state, transactional domain logic, and live subscriptions; local databases for durable offline behavior; separate services for agent execution, large files, search, analytics, billing, and long-running workflows.**

My assessment:

| Area                            |                                                   Fit |
| ------------------------------- | ----------------------------------------------------: |
| Rust desktop integration        |                                              **9/10** |
| TanStack/React web integration  |                                              **9/10** |
| React Native integration        |                                            **6.5/10** |
| Online realtime synchronization |                                              **9/10** |
| Durable offline-first behavior  |           **3/10 natively; 7/10 with your own layer** |
| Multi-tenant authorization      |                                            **6.5/10** |
| Agent/workflow backend          |                      **5/10 alone; 8/10 in a hybrid** |
| Self-hosted high availability   | **4/10 from public tooling; higher under Enterprise** |
| Complete backend replacement    |                                              **5/10** |
| Recommended hybrid architecture |                                              **8/10** |

I would **green-light a production architecture spike**, but I would place hard gates around offline behavior, authorization, React Native lifecycle handling, schema migration, and deployment licensing.

SpacetimeDB is also developing quickly. Version 2.7.1 was released on July 30, 2026, and included reconnect, migration, authentication, networking, and operational fixes. The preceding 2.7 release included a TypeScript-generated API casing change, which illustrates that the platform is currently production-capable but still evolving rapidly enough that you should pin versions and maintain an SDK compatibility layer. ([GitHub][1])

---

# 1. Why it matches your architecture

SpacetimeDB combines several layers you would otherwise need to build separately:

```text
Relational database
+ server-side domain logic
+ transactional mutations
+ WebSocket synchronization
+ filtered subscriptions
+ in-memory client replica
+ generated Rust and TypeScript bindings
```

Clients subscribe to typed table or view queries. SpacetimeDB sends a consistent initial snapshot and then incremental updates for committed transactions. The SDK maintains a local cache representing the subscribed subset of the database; application code reads that cache without network round trips. Each committed transaction produces at most one atomic update, and updates are delivered in exact commit order. ([SpacetimeDB][2])

That gives you a model resembling:

```text
                    SpacetimeDB
          authoritative transactional state
                           │
             incremental transaction updates
          ┌────────────────┼─────────────────┐
          │                │                 │
          ▼                ▼                 ▼
    Rust client       TypeScript client   TypeScript client
    desktop cache      browser cache      mobile cache
          │                │                 │
          ▼                ▼                 ▼
      GPUI/Zed           React UI         React Native UI
```

This gets you much closer to Linear’s synchronized-object architecture than a conventional PostgreSQL-plus-REST backend would. You avoid implementing:

* a custom WebSocket fanout service;
* cache invalidation messages;
* GraphQL subscription machinery;
* entity-change serialization;
* per-client live query tracking;
* generated mutation clients;
* and much of the connection-to-database consistency logic.

The key difference from Linear is that the SpacetimeDB client cache is an **online synchronized runtime cache**, not presently a fully durable offline database with a persisted operation queue and rollback/rebase mechanism.

---

# 2. Client-by-client assessment

## Rust desktop application forked from Zed: excellent fit

This is the strongest part of the architecture.

SpacetimeDB officially supports Rust server modules and Rust clients. Its tooling can compile and publish the Rust module and generate Rust client bindings from the module schema. ([SpacetimeDB][3])

You can therefore have:

```text
Rust domain/module code
          │
          ├── compiled into the SpacetimeDB server module
          │
          └── used to generate Rust client bindings
                         │
                         ▼
                  Zed-derived desktop
```

I would not, however, let generated SpacetimeDB row types spread throughout the Zed/GPUI UI code. Use a boundary:

```text
┌─────────────────────────────────────────────┐
│ Zed/GPUI presentation                      │
│ stable application models and commands     │
└──────────────────────┬──────────────────────┘
                       │
┌──────────────────────▼──────────────────────┐
│ SyncRepository / WorkspaceStore            │
│                                            │
│ • maps generated rows to domain models     │
│ • owns subscriptions                       │
│ • projects changes onto GPUI entities      │
│ • manages pending local commands           │
│ • exposes connection/sync status           │
└──────────────────────┬──────────────────────┘
                       │
┌──────────────────────▼──────────────────────┐
│ Generated SpacetimeDB Rust bindings         │
│ DbConnection + subscribed cache             │
└─────────────────────────────────────────────┘
```

### Recommended desktop implementation

Run the SpacetimeDB connection and message-processing loop on a dedicated async executor or thread. Convert table callbacks into typed domain changes, then schedule those changes onto the GPUI model/update context rather than mutating the interface directly from networking callbacks.

Keep a persistent local database alongside it. Reasonable Rust choices include SQLite, `redb`, or another embedded store, depending on whether you need relational queries or mostly key/value persistence. That database should hold:

* pending commands;
* drafts;
* local preferences;
* recently used workspace projections;
* cached file metadata;
* uncommitted editor state;
* and perhaps a durable last-known display cache.

### Preserve Zed’s specialized collaboration machinery

Do not automatically replace Zed’s text-buffer collaboration layer with normal SpacetimeDB table updates.

SpacetimeDB is excellent for:

* file metadata;
* worktree state;
* agent sessions;
* task status;
* participants;
* comments;
* permissions;
* checkpoints;
* accepted outcomes;
* run activities;
* and presence.

Character-level collaborative editing is a different workload. It needs batching, cursor anchoring, undo semantics, concurrent insertion/deletion handling, and preferably offline merge behavior. Keep Zed’s specialized buffer protocol or use a dedicated CRDT/OT subsystem, while storing document identity, access control, snapshots, and checkpoint metadata in SpacetimeDB.

---

## TypeScript TanStack Router web application: excellent fit

The TypeScript SDK includes official React bindings:

* `SpacetimeDBProvider`;
* `useTable`;
* `useReducer`;
* connection-state hooks;
* automatic resubscription;
* and reconnect behavior with exponential backoff.

The provider is explicitly designed to work safely with React Strict Mode. The current SDK also reconstructs dead browser connections after focus, visibility, network-return, and page-show events. ([SpacetimeDB][4])

SpacetimeDB has an official TanStack Start template with generated TypeScript bindings and TanStack Query integration. A plain TanStack Router SPA is simpler still because you do not need to solve as much server/client query handoff. ([SpacetimeDB][5])

### Recommended web structure

```text
Root route
└── AuthenticationProvider
    └── SpacetimeConnectionProvider
        └── WorkspaceSubscriptionManager
            └── TanStack Router routes
```

Have routes declare the scopes they require:

```text
/workspace/:workspaceId
    workspace summary
    user membership
    unread counters

/workspace/:workspaceId/thread/:threadId
    thread
    messages
    activities
    agent runs
    participants
```

A central subscription manager should reference-count subscriptions so that navigation does not accidentally create several overlapping subscriptions returning the same rows. SpacetimeDB explicitly warns that overlapping queries can cause the same rows to be processed and serialized repeatedly; unsubscription is also asynchronous. ([SpacetimeDB][6])

### Do not create two competing frontend caches

I would not copy all SpacetimeDB rows into TanStack Query.

Use:

* **SpacetimeDB’s client cache** for live domain state;
* **TanStack Query** for ordinary HTTP services, cold search results, signed upload URLs, billing, analytics, and external integrations;
* **TanStack Router** for navigation, scope, loaders, and pending-route behavior.

Conceptually:

```text
SpacetimeDB cache
    live workspace state
    issues, threads, runs, activities, presence

TanStack Query
    search API
    object-storage metadata
    billing
    analytics
    integration APIs
```

### Server-side rendering caveat

SpacetimeDB can be used from server runtimes, and the official TanStack Start integration exists. However, first-class prefetch-to-live-subscription handoff is still being refined; there is an open request specifically for better prefetching and handoff around `useSpacetimeDBQuery`. ([GitHub][7])

For the authenticated application, I would initially treat the shell as client-connected after hydration. Use SSR primarily for public pages and marketing surfaces rather than making every private live view depend on perfect server-to-WebSocket cache handoff.

---

## React Native mobile application: viable, but requires an adapter layer

The TypeScript SDK received explicit React Native compatibility work in 2025. That work fixed URL handling and supplied `TextEncoder`/`TextDecoder` behavior missing from React Native. Older React Native versions still required a URL polyfill. ([GitHub][8])

The problem is not basic wire compatibility. The problem is that React Native has a different lifecycle from a browser:

* iOS can suspend the process and kill the socket;
* Android can background or terminate it;
* Wi-Fi-to-cellular transitions may leave a connection apparently open but unusable;
* browser events such as `visibilitychange`, `focus`, and `pageshow` do not map directly to React Native;
* tokens must be persisted securely and restored;
* and an app may resume after hours with a completely dead runtime cache.

The official TypeScript documentation contains React and browser framework integrations, but not a comprehensive, dedicated React Native integration architecture. That means I would treat React Native as supported at the transport/SDK level, not as turnkey. ([SpacetimeDB][4])

### Build a mobile connection controller

Do not mount the browser-oriented provider and assume the job is finished. Wrap the lower-level client in something like:

```text
MobileSpacetimeController
├── OIDC token restoration
├── AppState listener
├── network-state listener
├── foreground reconnect
├── dead-socket detection
├── subscription restoration
├── local outbox
├── durable display cache
└── sync-health state
```

Use:

* React Native `AppState`;
* a network-state facility such as NetInfo;
* Keychain/Keystore-backed token persistence;
* SQLite or another durable mobile database;
* an explicit reconnect when the application returns to the foreground;
* and subscription reconstruction from the current navigation state.

Test at least:

* Hermes;
* iOS process suspension;
* Android process death;
* Wi-Fi-to-cellular handoff;
* airplane-mode writes;
* expired token on resume;
* application upgrade while commands are pending;
* and reconnect after a server module migration.

React Native is not a reason to reject SpacetimeDB, but it is the client on which you will own the largest amount of sync glue.

---

# 3. The decisive gap: this is not yet durable offline-first sync

SpacetimeDB’s local cache is strong while the client is connected. It contains a consistent subset of committed server state, and callbacks see the cache after the complete transaction update has been applied. ([SpacetimeDB][2])

What it does not currently give you as a documented built-in product is:

* a durable client-side database;
* continued mutation while disconnected;
* a persisted operation outbox;
* immediate speculative table updates;
* rollback when a server rejects a mutation;
* conflict merging after extended offline work;
* or a Replicache-style reconnect protocol.

An open feature request from July 4, 2026 asks for exactly these capabilities: robust disconnected operation and catch-up, immediate local propagation, and rollback or correction after server processing. ([GitHub][9])

This matters much more for your desktop and mobile applications than for a browser tab.

## Recommended client-state model

Build three layers:

```text
Displayed state
      =
confirmed SpacetimeDB replica
      +
persisted speculative operations
      +
local-only drafts and device state
```

For example:

```rust
struct CommandEnvelope {
    operation_id: Uuid,
    device_id: Uuid,
    workspace_id: Uuid,
    issued_at: Timestamp,
    expected_entity_version: Option<u64>,
    payload: DomainCommand,
}
```

Every command should have a stable `operation_id`. The reducer should either:

1. find an existing receipt and treat the call as a retry; or
2. execute the mutation and insert an `operation_receipt` in the same transaction.

```text
operation_receipt
├── operation_id
├── actor_id
├── device_id
├── status
├── committed_at
├── resulting_entity_id
├── resulting_version
└── error_code / rejection reason
```

That gives you idempotency across:

* reconnects;
* app restarts;
* uncertain reducer acknowledgments;
* duplicate mobile submissions;
* and service retries.

## Different data needs different conflict behavior

Do not attempt one universal last-write-wins policy.

| Data type               | Recommended behavior                                |
| ----------------------- | --------------------------------------------------- |
| Messages and activities | Append-only with idempotency IDs                    |
| Issue or task status    | Expected-version check or explicit transition rules |
| Assignee/ownership      | Server-authoritative transition with rejection      |
| Reordering              | Fractional position or explicit move operation      |
| Draft prompts           | Device-local until explicitly submitted             |
| Rich text               | CRDT/OT or document-specific operations             |
| Agent run events        | Append-only ordered sequence                        |
| Presence                | Ephemeral, no offline replay                        |
| Settings                | Usually last-write-wins with timestamps             |

The online SpacetimeDB cache can remain your confirmed-state source. Your own local store only needs to overlay pending or locally owned state rather than duplicating the entire database semantics.

---

# 4. Where SpacetimeDB should sit in your backend

For an OpenAgents-style product, I would use SpacetimeDB as a **collaborative operational state plane**.

## Good data to place in SpacetimeDB

```text
Workspace
Membership
Role/capability grants
Project
Thread/workroom
Message metadata and durable messages
Agent identity
Agent run
Run plan
Run activity
Task
Approval request
Decision
Artifact metadata
Run receipt
Notification state
Presence
Device/session state
Worker lease
Queue admission state
```

This data benefits from:

* transactional writes;
* immediate synchronization;
* typed clients;
* multiple clients observing the same changes;
* and low-latency local reads.

## Systems I would keep outside SpacetimeDB

| System                               | Recommended home                                   |
| ------------------------------------ | -------------------------------------------------- |
| LLM calls and agent execution        | Rust worker services                               |
| Multi-day durable workflows          | Temporal or another durable workflow engine        |
| Sandboxed code execution             | GCP/SHC/container execution plane                  |
| Large artifacts and files            | R2, GCS, S3, or equivalent                         |
| Full-text search                     | OpenSearch/Meilisearch/PostgreSQL search service   |
| Vector retrieval                     | pgvector or dedicated retrieval service            |
| Product analytics                    | ClickHouse/BigQuery/warehouse                      |
| Accounting and BTC settlement ledger | Dedicated strongly audited ledger service/database |
| Email, push, webhooks                | Integration workers                                |
| Secrets                              | Cloud KMS/secret manager                           |
| Large immutable traces               | Object storage plus indexed metadata               |
| Character-level document/code sync   | Specialized CRDT/OT/editor protocol                |

SpacetimeDB itself recommends external storage references for large objects and keeping large-file bytes outside the core table model. ([SpacetimeDB][10])

---

# 5. SpacetimeDB should not become your agent runtime

SpacetimeDB reducers are deterministic transactional functions designed to modify tables; they should not perform ordinary external side effects. Procedures can perform outbound HTTP and return values, but they do not automatically run inside transactions, and Rust procedures remain marked unstable. Schedule tables can invoke reducers or procedures at future times. ([SpacetimeDB][11])

That is enough for:

* expiring a lease;
* updating a timeout;
* periodic cleanup;
* sending a narrow webhook;
* refreshing external metadata;
* or scheduling a reminder.

It is not where I would run:

* a 45-minute coding agent;
* a retrying LLM workflow;
* cross-provider failover;
* a multi-day Full Auto run;
* sandbox lifecycle management;
* model/tool streaming;
* or financially consequential settlement.

Use this pattern:

```text
SpacetimeDB
    │
    │ new runnable job / run state
    ▼
Durable workflow or queue
    │
    ▼
Rust agent worker
    ├── model gateway
    ├── tools/MCP
    ├── code sandbox
    ├── browser
    ├── local/cloud execution
    └── provider failover
    │
    ▼
SpacetimeDB reducers
    ├── append durable activity
    ├── update run state
    ├── attach artifact metadata
    └── publish completion receipt
```

The worker should claim work transactionally through a reducer:

```text
queued
  → leased(worker_id, lease_expires_at)
  → running
  → awaiting_approval
  → completed / failed / canceled
```

Expired leases can be recovered through scheduled logic, but the actual workflow should remain in the durable execution layer.

## Events versus durable activities

SpacetimeDB event tables broadcast inserted rows to connected subscribers and then immediately remove them from table state. They are useful for transient signals but are not retained in the client cache. ([SpacetimeDB][12])

Use event tables for:

* typing indicators;
* transient animation triggers;
* “worker is currently thinking” pulses;
* cursor movement;
* low-value live progress;
* and presence changes.

Use ordinary durable tables for:

* run steps;
* agent conclusions;
* approval requests;
* tool results;
* errors;
* handoffs;
* accepted artifacts;
* and anything that must survive reconnect or support replay.

For streamed model output, do not write one transaction per token. Batch into textual chunks or update a bounded current-output row at a controlled cadence, then commit the completed message as one durable object.

---

# 6. Multi-tenant authorization is the largest server-side risk

SpacetimeDB clients connect directly to the database. That means your Rust module—not a hidden REST API—is the application’s main security boundary.

Tables are private by default. Public tables can be subscribed to by clients but can only be written through reducers. Views can expose filtered rows or selected columns from private data. ([SpacetimeDB][13])

Anonymous clients can receive identities and can call exported reducers or query public tables. Therefore every sensitive reducer must explicitly require the correct issuer, authenticated identity, membership, and capability. SpacetimeDB recommends checking the JWT issuer because any accepted OIDC token could otherwise establish an identity. ([SpacetimeDB][14])

## Required security rules

I would make these non-negotiable:

1. **Canonical multi-tenant tables remain private.**
2. **Every public reducer checks authenticated workspace membership.**
3. **Every reducer checks the exact capability required, not merely “is a member.”**
4. **Every public view returns only rows the caller is allowed to know exist.**
5. **No client-side `workspace_id` filter is treated as authorization.**
6. **Anonymous connections are rejected for the authenticated product database.**
7. **Every deny path has an integration test using the real generated binding.**
8. **Service identities receive narrow capabilities rather than database-owner credentials.**

## Views are useful, but there is a scaling caveat

SpacetimeDB currently recommends views rather than row-level security; RLS remains experimental and unstable. ([SpacetimeDB][15])

A view using caller identity is computed and tracked independently for every subscriber. The documentation explicitly gives the example that 1,000 connected users can mean 1,000 separate computations and change-tracking sets. ([SpacetimeDB][16])

This is relevant because your natural authorization query is something like:

```text
All threads, messages, projects, runs, and notifications
visible to this user through their workspace memberships
```

That is exactly the kind of per-caller materialized view that can become expensive. An open proposal for `ScopedViewContext` identifies the gap between one shared global view and independently materialized per-user views; it proposes sharing view computation by a key such as team or workspace, but that capability is not currently available. ([GitHub][17])

This does not make SpacetimeDB infeasible. It means authorization-view performance must be benchmarked early with your actual membership graph and concurrency.

---

# 7. Recommended tenancy and database topology

I see three possible layouts.

## Option A: one global product database

```text
One SpacetimeDB database
└── all users, workspaces, threads, messages, runs
```

### Advantages

* simplest deployment;
* easiest cross-workspace queries;
* one client connection;
* atomic operations across all workspaces;
* easiest generated-binding management.

### Disadvantages

* all hot data must fit the database’s memory envelope;
* authorization views become critical;
* one database is a large failure and scaling unit;
* cold history accumulates indefinitely;
* tenant isolation is logical rather than physical.

I would use this for an initial prototype, but not assume it is the forever topology.

## Option B: one database per workspace

```text
Control database
├── user account
├── workspace directory
└── workspace → database mapping

Workspace database A
Workspace database B
Workspace database C
```

SpacetimeDB says databases are lightweight and explicitly recommends external orchestration when creating independent databases for rooms or matches. ([SpacetimeDB][18])

### Advantages

* strong tenant isolation;
* simpler workspace authorization;
* natural RAM and failure boundaries;
* enterprise customers can be moved independently;
* easier tenant export/deletion.

### Disadvantages

* users in several workspaces need several connections or connection switching;
* cross-workspace notifications need a global aggregation layer;
* module upgrades must be rolled out across a database fleet;
* database provisioning and routing become a product of their own;
* no single reducer transaction can span separate workspace databases.

This could work, but I would not begin here unless physical tenant isolation is a hard requirement.

## Option C: cell-based databases — recommended

```text
Global control plane
├── accounts
├── identity mapping
├── workspace directory
└── workspace → cell mapping

SpacetimeDB cell US-1
├── workspace A
├── workspace B
└── workspace C

SpacetimeDB cell US-2
├── workspace D
└── workspace E

Dedicated enterprise database
└── workspace F
```

This gives you a Linear-like tenant-home concept without requiring one deployment per tenant.

The control plane could be a small conventional Rust/PostgreSQL service or a separate SpacetimeDB database. Each client first resolves:

```text
user + workspace → SpacetimeDB host/database
```

Then it connects directly to that cell.

Start with one cell, but include `workspace_id` in every relevant key and index so that you retain the ability to move a workspace later.

---

# 8. Memory and data-volume implications

SpacetimeDB holds database state in memory; the practical database-size limit is therefore host RAM. Maincloud documents machines with up to hundreds of gigabytes of memory, but the architectural constraint remains: hot table state must fit on the assigned machine. ([SpacetimeDB][18])

This is not necessarily a problem for a collaboration product. Structured issue, thread, run, activity, and metadata rows are compact. It becomes a problem when you retain:

* every raw model token;
* full terminal transcripts;
* full code patches repeatedly;
* browser recordings;
* images;
* build artifacts;
* large logs;
* embeddings;
* or unbounded historical traces.

I would divide data into three temperatures:

| Temperature               | Storage                          |
| ------------------------- | -------------------------------- |
| Hot operational state     | SpacetimeDB                      |
| Warm searchable history   | PostgreSQL/search/vector service |
| Cold artifacts and traces | Object storage                   |

For example:

```text
SpacetimeDB agent_activity
├── activity_id
├── run_id
├── kind
├── summary
├── actor
├── timestamp
├── sequence
└── artifact_ref

Object storage
└── complete raw tool output / model trace / terminal log
```

The UI synchronizes the compact activity immediately, then fetches the full artifact only when someone expands it.

SpacetimeDB’s PGWire support should not be mistaken for PostgreSQL compatibility. It currently supports only the Simple Query Protocol, no parameterized queries, a subset of SQL, and no live subscription behavior through PGWire. Use it for limited inspection or extraction, not as the foundation of your normal analytics/ORM ecosystem. ([SpacetimeDB][19])

---

# 9. Schema evolution across three independently shipped clients

This is another major concern, especially because mobile releases lag behind server and web deployments.

SpacetimeDB can hot-swap a module without disconnecting current clients, but automatic migrations still restrict many schema changes. Removing or changing columns, renaming or reordering them, adding columns without defaults, and several constraint changes require an incremental migration strategy rather than an automatic update. ([SpacetimeDB][20])

Treat the module schema as a public wire contract.

## Recommended migration discipline

```text
Version N
    old table/field remains readable

Version N+1
    add new table/field with default
    server dual-writes old and new forms
    all clients understand at least the old form

Version N+2
    clients migrate to new form
    backfill old rows

Version N+3
    stop old writes

Later maintenance migration
    remove obsolete representation
```

Also:

* Pin the SpacetimeDB host, Rust SDK, TypeScript SDK, and CLI versions together.
* Generate Rust and TypeScript bindings in CI.
* Compile the desktop, web, and mobile applications after every module-schema change.
* Test the current server against both N and N−1 clients.
* Do not expose generated types directly through your whole application.
* Version important reducer command envelopes.
* Prefer opaque UUID identifiers and stable names.
* Use explicit secondary indexes for workspace and parent relationships.

SpacetimeDB allows only one primary-key column per table and does not yet support composite primary keys. Multi-column B-tree indexes are the documented alternative. ([SpacetimeDB][21])

---

# 10. Deployment and licensing may decide this before the technology does

## Managed Maincloud

Maincloud advertises managed scaling, replication, and backups. Paid tiers include point-in-time backup retention, automatic replication and backups, DDoS protection, dedicated-node options, and enterprise arrangements for custom scalability and deployment. ([SpacetimeDB][18])

For an initial production deployment, Maincloud or an Enterprise deployment is the path of least risk.

## Public self-hosting story

The public self-hosting guide describes a single SpacetimeDB process on an Ubuntu machine managed by systemd, with Nginx and Let’s Encrypt. It does not document a general self-hosted replicated cluster, automatic failover procedure, or multi-region topology. ([SpacetimeDB][22])

A single powerful GCP VM could be acceptable for:

* development;
* internal use;
* an alpha;
* a regional pilot;
* or a product whose recovery objective tolerates restart and replay.

It would not be enough for a high-availability control plane without additional vendor-supported replication and operations.

## License warning

The current repository is under Business Source License 1.1. Its Additional Use Grant permits production use with **no more than one SpacetimeDB instance** and prohibits using it to provide a “Database Service” as defined in the license. The license changes to AGPLv3 with a linking exception on July 26, 2031. ([GitHub][23])

That means a self-hosted topology involving:

* an active and standby instance;
* several regional instances;
* horizontally sharded hosts;
* or other multi-node production deployment

may require alternative commercial licensing. This needs confirmation from Clockwork Labs and your counsel rather than interpretation from an architecture document. Enterprise pricing explicitly includes customized licensing, on-prem/BYO-cloud options, and custom replication. ([SpacetimeDB][24])

Given your preference for Google Cloud/self-hosting, I would treat an Enterprise/BYO-cloud conversation as a prerequisite before locking in SpacetimeDB as strategic infrastructure.

---

# 11. Recommended target architecture

```text
                              OIDC provider
                          Auth0 / Clerk / Keycloak
                                     │
         ┌───────────────────────────┼───────────────────────────┐
         │                           │                           │
         ▼                           ▼                           ▼
┌─────────────────┐       ┌────────────────────┐       ┌─────────────────┐
│ Rust desktop    │       │ TanStack web       │       │ React Native    │
│ Zed/GPUI fork   │       │ React              │       │ iOS + Android   │
│                 │       │                    │       │                 │
│ Rust STDB SDK   │       │ TS/React STDB SDK  │       │ TS STDB SDK     │
│ local DB/outbox │       │ optional IndexedDB │       │ SQLite/outbox   │
└────────┬────────┘       └──────────┬─────────┘       └────────┬────────┘
         │                           │                           │
         └──────────── WebSocket subscriptions/reducers ────────┘
                                     │
                        ┌────────────▼─────────────┐
                        │ SpacetimeDB cell         │
                        │ Rust module              │
                        │                          │
                        │ • canonical hot state    │
                        │ • transactional reducers │
                        │ • private tables         │
                        │ • authorized views       │
                        │ • schedules              │
                        │ • transient events       │
                        │ • worker leases          │
                        └─────────┬─────────┬──────┘
                                  │         │
                        service SDK         │ procedures/HTTP
                                  │         │ narrow side effects
                    ┌─────────────▼──┐      │
                    │ Rust workflow  │      │
                    │ and agent plane│      │
                    │                │      │
                    │ • Temporal     │      │
                    │ • model gateway│      │
                    │ • MCP/tools    │      │
                    │ • sandboxes    │      │
                    │ • failover     │      │
                    └───────┬────────┘      │
                            │               │
          ┌─────────────────┼───────────────┼─────────────────┐
          │                 │               │                 │
          ▼                 ▼               ▼                 ▼
     Object store      Search/vector    Postgres ledger    Notifications
     R2/GCS/S3         retrieval        billing/BTC        email/push/webhooks
```

This preserves the primary benefit of SpacetimeDB—one synchronized state model across every interface—without making it responsible for workloads poorly matched to its execution model.

---

# 12. Suggested table and reducer model

A minimal operational model might look like:

```text
workspace
workspace_member
capability_grant

thread
thread_participant
message
message_revision

agent
agent_run
agent_run_plan_step
agent_activity
agent_question
agent_handoff

task
approval
decision

artifact
artifact_version
accepted_outcome
execution_receipt

worker
worker_lease
pending_job

device
operation_receipt
notification
```

Every tenant-owned table should have an indexed `workspace_id`.

Every ordered stream should have a monotonic sequence or sortable identifier:

```text
agent_activity
├── id
├── workspace_id
├── run_id
├── sequence
├── kind
├── summary
├── payload_ref
├── actor_id
└── created_at
```

Reducers should express domain commands rather than generic CRUD:

```text
create_thread
post_message
delegate_run
claim_run
append_run_activity
request_approval
answer_approval
handoff_run
complete_run
accept_outcome
cancel_run
```

That gives you one place to enforce:

* valid state transitions;
* permissions;
* idempotency;
* budgets;
* ownership;
* termination;
* and receipt creation.

SpacetimeDB’s own migration guidance emphasizes that reducers commit state changes while clients observe resulting rows through subscriptions; reducers are not designed as request-response endpoints returning arbitrary application data. ([SpacetimeDB][10])

---

# 13. Production spike and go/no-go tests

Before selecting it as the permanent backend, build one vertical slice across all three clients:

```text
Login
  → workspace subscription
  → create thread
  → start agent run
  → stream activities
  → request approval
  → approve from mobile
  → finish run
  → reconnect every client
  → verify identical state
```

The spike should prove:

### Connectivity and lifecycle

* Desktop restart with pending local commands.
* Browser sleep, network interruption, and tab restore.
* React Native background, process death, and Wi-Fi/cellular switching.
* Token expiration and refresh.
* Subscription reconstruction after reconnect.

### Correctness

* No duplicated commands after uncertain acknowledgments.
* Exact ordered activity streams.
* Expected-version conflict rejection.
* Resubscription without missing or duplicating rows.
* Durable receipt handling.

### Authorization

* User outside a workspace cannot infer that its rows exist.
* Removed member loses read access on every active client.
* A compromised client cannot subscribe more broadly than its authorization permits.
* Anonymous identities cannot enter the authenticated database.
* Worker identities cannot perform user/admin operations.

### Scale

* Realistic workspace row counts.
* Large message and activity histories.
* Several route-scoped subscriptions.
* Per-user authorization views under expected concurrency.
* Initial snapshot time and bytes.
* Reconnect storm behavior.
* Hot database RAM growth.

### Deployment

* Module upgrade while N−1 desktop/mobile clients remain connected.
* Incremental schema migration.
* Backup restore.
* Maincloud or Enterprise replica failure.
* Cell routing and workspace migration.
* Observability for queue depth, outgoing connection pressure, reducer failures, and view computation.

### Business and platform risk

* Enterprise license terms.
* BYO-GCP support.
* Replication and recovery guarantees.
* Export path if you later migrate away.
* Cost per active connection, database memory, and transaction workload.
* SDK support commitments for React Native.

---

# Final recommendation

**Adopt SpacetimeDB as the candidate primary realtime state layer, not as the entire platform.**

It is particularly compelling for you because:

* the server module can be Rust;
* the desktop client has a first-class Rust SDK;
* the web client has excellent TypeScript/React support;
* the mobile client can use the same generated TypeScript contract;
* and its transactional subscription model closely matches the shared-workroom, agent-run, activity, and fleet-state architecture you are building.

The three conditions are:

1. **You build your own durable local cache and mutation outbox for desktop and mobile.**
2. **You retain a separate Rust workflow/agent execution plane.**
3. **You settle deployment and licensing with Clockwork Labs before depending on self-hosted high availability.**

Under that hybrid, SpacetimeDB could plausibly save you from building **roughly two-thirds of the custom Linear-like online synchronization backend**: transaction ordering, live query tracking, WebSocket delivery, client replicas, generated cross-language APIs, and much of the state server.

The remaining third—offline reconciliation, enterprise authorization scaling, agent orchestration, cold storage, search, artifacts, analytics, regional cells, and operational recovery—is still substantial and should be treated as first-class architecture rather than deferred glue.

**Overall decision: proceed with a bounded production spike. The architecture is promising enough to justify commitment testing, but not mature enough to make an unconditional all-in backend decision before the offline, React Native, authorization-view, HA, and licensing gates pass.**

[1]: https://github.com/clockworklabs/SpacetimeDB/releases "https://github.com/clockworklabs/SpacetimeDB/releases"
[2]: https://spacetimedb.com/docs/clients/subscriptions/semantics/ "https://spacetimedb.com/docs/clients/subscriptions/semantics/"
[3]: https://spacetimedb.com/docs/quickstarts/rust/ "Rust Quickstart | SpacetimeDB docs"
[4]: https://spacetimedb.com/docs/clients/typescript "https://spacetimedb.com/docs/clients/typescript"
[5]: https://spacetimedb.com/docs/quickstarts/tanstack "https://spacetimedb.com/docs/quickstarts/tanstack"
[6]: https://spacetimedb.com/docs/clients/subscriptions "Subscriptions | SpacetimeDB docs"
[7]: https://github.com/clockworklabs/SpacetimeDB/issues/4438 "Tanstack Start: Allow prefetching data for useSpacetimeDBQuery · Issue #4438 · clockworklabs/SpacetimeDB · GitHub"
[8]: https://github.com/clockworklabs/SpacetimeDB/pull/2955 "https://github.com/clockworklabs/SpacetimeDB/pull/2955"
[9]: https://github.com/clockworklabs/SpacetimeDB/issues/5481 "https://github.com/clockworklabs/SpacetimeDB/issues/5481"
[10]: https://spacetimedb.com/docs/migrating-from-convex "Migrating from Convex | SpacetimeDB docs"
[11]: https://spacetimedb.com/docs/functions/reducers/ "Overview | SpacetimeDB docs"
[12]: https://spacetimedb.com/docs/tables/event-tables/ "Event Tables | SpacetimeDB docs"
[13]: https://spacetimedb.com/docs/tables/ "Tables | SpacetimeDB docs"
[14]: https://spacetimedb.com/docs/http/authorization/ "Authorization | SpacetimeDB docs"
[15]: https://spacetimedb.com/docs/how-to/rls/ "https://spacetimedb.com/docs/how-to/rls/"
[16]: https://spacetimedb.com/docs/functions/views "https://spacetimedb.com/docs/functions/views"
[17]: https://github.com/clockworklabs/SpacetimeDB/issues/4629 "Feature Request: `ScopedViewContext` · Issue #4629 · clockworklabs/SpacetimeDB · GitHub"
[18]: https://spacetimedb.com/docs/intro/faq/ "https://spacetimedb.com/docs/intro/faq/"
[19]: https://spacetimedb.com/docs/how-to/pg-wire/ "https://spacetimedb.com/docs/how-to/pg-wire/"
[20]: https://spacetimedb.com/docs/databases/automatic-migrations/ "https://spacetimedb.com/docs/databases/automatic-migrations/"
[21]: https://spacetimedb.com/docs/tables/constraints/ "https://spacetimedb.com/docs/tables/constraints/"
[22]: https://spacetimedb.com/docs/how-to/deploy/self-hosting/ "https://spacetimedb.com/docs/how-to/deploy/self-hosting/"
[23]: https://github.com/clockworklabs/spacetimedb/blob/master/LICENSE.txt "https://github.com/clockworklabs/spacetimedb/blob/master/LICENSE.txt"
[24]: https://spacetimedb.com/pricing "https://spacetimedb.com/pricing"


