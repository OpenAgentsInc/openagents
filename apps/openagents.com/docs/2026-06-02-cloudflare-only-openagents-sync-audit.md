# OpenAgents Autopilot Cloudflare-Only OpenAgents Sync Audit

Date: 2026-06-02

Status: canonical replacement plan for `openagents`. It records the
user-directed target: replace Convex with Cloudflare-only infrastructure,
remove TanStack DB/Electric/Postgres from the target architecture, use Effect
as the server authority model, use Foldkit for the webapp, and build a custom
OpenAgents Sync protocol.

## Executive Summary

The clean correction is:

```text
Do not use TanStack DB.
Do not use Electric.
Do not use Postgres.
Build OpenAgents Sync on Cloudflare primitives.
```

TanStack DB's Electric collection is explicitly a Postgres/ElectricSQL sync
path. That does not match the stated infrastructure constraint: Cloudflare
only. For `openagents`, the replacement stack should be:

```text
Foldkit webapp
  -> Effect-first Model / Message / update / Command architecture
  -> OpenAgents Sync client
  -> optional IndexedDB cache

Cloudflare Worker API
  -> OpenAuth
  -> Effect command handlers
  -> D1 repositories
  -> R2 artifact refs
  -> Queue producers
  -> Workflow starters

Durable Objects / Agents
  -> active realtime sync
  -> WebSocket hibernation
  -> cursor replay
  -> presence
  -> per-thread / per-run / deploy coordination

D1
  -> source of truth
  -> sync outbox
  -> idempotent mutations
  -> cold snapshots

R2
  -> artifacts, logs, screenshots, transcripts, generated bundles

Queues / Workflows
  -> async durable background work
  -> retries
  -> long-running transitions

SHC OpenCode
  -> primary execution lane

GCloud
  -> backup execution lane
```

The new product-specific sync layer is **OpenAgents Sync**. It should be a
small owned protocol and implementation, not generic SQL replication.

## Repository Layout Decision

`openagents` should be a Bun workspace, not a single-package app. Bun is
already the package manager, and the target needs clean runtime boundaries
between browser code, Cloudflare Worker code, and shared Effect protocol code.

Canonical layout:

```text
apps/web/
  Foldkit/Vite browser app

workers/api/
  Cloudflare Worker API
  SyncRoom Durable Object
  D1 migrations
  Wrangler configuration

packages/sync-schema/
  Effect Schema definitions for SyncPatch, SyncCommand, snapshots, cursors,
  mutation results, and server messages

packages/sync-client/
  browser-side OpenAgents Sync helpers that the Foldkit Model/update layer can
  call from Commands and Messages

packages/sync-worker/
  Worker-side sync helpers, response helpers, binding types, and D1/DO routing
  primitives
```

Rules:

- Cloudflare runtime code belongs in `workers/api/` or `packages/sync-worker/`.
- Foldkit/browser code belongs in `apps/web/` or `packages/sync-client/`.
- External protocol models belong in `packages/sync-schema/` and must be Effect
  Schema-first.
- Foldkit reference source must not be vendored into this repo. Use
  `../projects/repos/foldkit/` or `node_modules/foldkit`; do not commit
  `repos/foldkit`.

## Current Vortex Dependency On Convex

Vortex is not using Convex as a small cache. It is using Convex as the product
ledger.

Current local facts:

- `package.json` depends on `convex` and the dev script starts `convex dev`
  alongside Next.
- `components/ConvexClientProvider.tsx` wires `ConvexReactClient`,
  `ConvexProviderWithAuth`, WorkOS/AuthKit access tokens, and authenticated
  user sync.
- `server/services/ConvexService.ts` is the main Effect boundary around Convex.
  It exposes thread, message, workroom event, generated UI, signature,
  approval, receipt, benchmark, Codex run, and training run operations.
- `server/AppLayer.ts` composes many services through `ConvexService`.
- `convex/schema.ts` currently defines 107 tables.

Replacing Convex means replacing:

- database authority;
- realtime subscriptions;
- server mutations;
- auth identity integration;
- idempotent event ingestion;
- scheduler/action behavior;
- generated API discipline;
- product authorization rules;
- migration and operator repair workflows.

OpenAgents Autopilot implication: do not hollow Convex out in Vortex. Use Vortex as
source material and parity reference. In `openagents`, build Cloudflare
authority from the beginning.

## Removed From The Target

### TanStack DB

TanStack DB is a client collection/live-query library for an API. It is not an
authoritative backend and it is not needed if OpenAgents owns its sync
protocol. Keeping it in the target would add an extra client database
abstraction before the actual Convex replacement exists.

Drop it from the first `openagents` plan.

### Electric

Electric is the wrong path for Cloudflare-only infrastructure. The official
TanStack DB Electric collection docs describe Electric collections as syncing
with Postgres through ElectricSQL's sync engine. If Postgres is out, Electric
is out.

### Postgres

Postgres is a viable future escape hatch through Hyperdrive if D1 becomes the
wrong store for a specific subsystem. It should not be in the first
`openagents` architecture. The stated target is Cloudflare-only.

### TanStack Start

TanStack Start can run on Cloudflare Workers, but Foldkit is a better fit for
the corrected direction because the user wants the webapp to be as
Effect-native as possible. TanStack Start is React-centered. Foldkit is
Effect-centered.

## Foldkit Assessment

Local and workspace references inspected:

```text
README.md
AGENTS.md
package.json
node_modules/foldkit/README.md
../projects/repos/foldkit/
../projects/foldkit-examples.md
../projects/repos/foldkit/examples/websocket-chat/src/main.ts
../projects/repos/foldkit/examples/auth/src/main.ts
../projects/repos/foldkit/examples/query-sync/src/main.ts
../projects/repos/foldkit/packages/foldkit/package.json
../projects/repos/foldkit/packages/vite-plugin-foldkit/README.md
```

The `openagents` README already defines the stack as Foldkit, Effect,
Cloudflare, OpenAuth, OpenCode, and SHC. Foldkit's README describes it as an
Effect-built, Elm-architecture frontend framework: one Model, one update
function, Messages for events, Commands for effects, no hooks, no local React
state, and no incremental React interop. The local package metadata describes
`foldkit` as a TypeScript frontend framework built on Effect and architected
like Elm. The examples use Vite, Tailwind, Effect, and
`@effect/platform-browser`.

Important local findings:

- Foldkit is Effect-first. Model is Effect Schema, every Message is typed, and
  Commands are Effects that return Messages.
- Foldkit has a Vite plugin that preserves model state across hot reloads.
- Foldkit has typed routing and navigation helpers.
- Foldkit has Managed Resources for lifecycle-bound browser resources such as
  WebSockets.
- Foldkit has Subscriptions for streams that depend on the current Model.
- The local `websocket-chat` example uses `ManagedResource` for a `WebSocket`
  and a `Subscription` stream for incoming messages.
- The local `auth` example uses `@effect/platform-browser` key-value storage
  and Effect Schema to load session flags.
- The local `query-sync` example demonstrates typed URL/query parsing and
  model-driven navigation.
- Foldkit includes Story and Scene testing helpers for update-level and
  user-level tests.
- Foldkit is pre-1.0. This is acceptable for `openagents` only if we accept
  that we may need to pin versions, vendor patches, or contribute fixes.

Foldkit is a good fit for the `openagents` webapp because OpenAgents Sync maps
cleanly onto its architecture:

```text
OpenAgents SyncPatch
  -> Foldkit Message
  -> update(model, message)
  -> deterministic Model transition
  -> view(model)

OpenAgents mutation command
  -> Foldkit Command
  -> Effect HTTP/WebSocket operation
  -> Accepted / Rejected / PatchReceived Message
```

The webapp should not use React. It should start as a Foldkit/Vite app served
from Cloudflare Workers Static Assets or an equivalent Cloudflare Worker asset
pipeline.

## Cloudflare-Only Sync Model

Build **scope sync**, not generic SQL replication.

OpenAgents scopes:

```text
workspace:{workspaceId}
thread:{threadId}
agent-run:{runId}
deploy:{deployId}
public-agent:{agentId}
repo:{repoId}
```

Implemented OpenAgents product surface scope names for the first Autopilot slice:

```text
workspace:{userId}
  Personal workroom/sidebar scope. Currently carries `missions` and
  `agent_runs` projections for the authenticated user's runs.

team:{teamId}
  Team room projection state. Currently carries durable `team_chat_messages`
  rows and team-scoped `thread_files` rows for shared team room/file views.

thread:{threadId}
  Thread route scope. For Autopilot missions, `threadId` is the public run route
  UUID and carries the mission's `agent_runs` and `agent_run_events`
  projections.

agent-run:{runId}
  Low-level run event scope. Carries the same public `agent_runs` projection plus
  append-only `agent_run_events`.
```

The initial D1 outbox implementation lives in `packages/sync-worker` as
`makeD1SyncOutboxRepository(...)`. It writes ordered `sync_changes` rows using
the existing `sync_scopes` sequence table and materializes snapshots by reducing
those patches. `workers/api/src/omni-runs.ts` now appends public mission,
agent-run, and agent-run-event projections whenever an Autopilot run is saved or
runner events are appended. The projection intentionally excludes raw callback
tokens, provider grant bodies, and runner payload blobs; detailed product views
can keep using redacted API detail surfaces until the UI sync conversion is
complete.

The Worker sync routes now authorize and serve the first generic scope set:

```text
GET  /api/sync/workspace/:userId/snapshot
GET  /api/sync/workspace/:userId/stream?cursor=:seq
POST /api/sync/workspace/:userId/mutate

GET  /api/sync/team/:teamId/snapshot
GET  /api/sync/thread/:threadId/snapshot
GET  /api/sync/agent-run/:runId/snapshot
```

`SyncRoomDurableObject` now accepts WebSocket upgrades, replays committed
`sync_changes` after the requested cursor, stores each socket cursor in the
hibernatable attachment, and can be poked through an internal `POST
/__sync/notify` request. Autopilot run launch and runner event ingest paths
poke the workspace/thread/agent-run scopes after appending outbox rows. Team
chat message writes and team-scoped thread-file uploads append `team:<teamId>`
changes and poke that team scope. The mutation route records `sync_mutations`
idempotently and currently accepts commands for future product reducers;
product-specific command execution still belongs in the later conversion phase.

A browser subscribes to a scope:

```text
GET /api/sync/thread/thr_123/snapshot
WebSocket /api/sync/thread/thr_123/stream?cursor=892
POST /api/sync/thread/thr_123/mutate
```

Cold read:

```text
Browser
  -> GET snapshot
  -> receives cursor + collection maps
  -> initializes Foldkit Model
```

Live read:

```text
Browser
  -> opens WebSocket with last cursor
  -> Durable Object replays missed sync_changes
  -> Durable Object streams future SyncPatch messages
  -> Foldkit Subscription emits PatchReceived Message
  -> update applies patch to Model
```

Write:

```text
Foldkit Command
  -> POST command with mutationId
  -> Worker verifies OpenAuth and validates command with Effect Schema
  -> Worker writes authoritative D1 rows
  -> Worker appends sync_changes rows
  -> Worker records sync_mutations idempotency row
  -> Worker pokes scope Durable Object / Agent
  -> Durable Object broadcasts patch
  -> client confirms optimistic mutation when matching mutationId arrives
```

## Minimal Sync Protocol

Every synced update is a patch envelope:

```ts
type SyncPatch = {
  scope: string
  seq: number
  collection: string
  op: 'put' | 'patch' | 'delete' | 'invalidate'
  id: string
  value?: unknown
  patch?: unknown
  serverTime: string
  mutationId?: string
}
```

Every client command carries a mutation id:

```ts
type SyncCommand = {
  mutationId: string
  scope: string
  command: string
  payload: unknown
  expectedVersion?: number
}
```

The authoritative commit signal is not the HTTP `accepted` response. The
commit signal is the streamed patch with the same `mutationId`.

## Browser Store

With Foldkit, the "store" is the Foldkit Model. Do not add TanStack DB.

Model shape:

```ts
type OpenAgentsSyncModel = {
  cursors: Record<string, number>
  collections: Record<string, Record<string, unknown>>
  pendingMutations: Record<string, PendingMutation>
  connectionByScope: Record<string, SyncConnectionState>
}
```

The patch application path is a pure update branch:

```text
PatchReceived(patch)
  -> verify seq is expected or mark gap
  -> apply put/patch/delete/invalidate to collection map
  -> update cursor
  -> reconcile pending mutation by mutationId
```

IndexedDB is optional. Use it after the online path works.

Implemented browser slice:

- `apps/web/src/page/loggedIn/model.ts` now carries a schema-backed sync model
  with cursors, collection maps, pending mutation slots, connection state, and a
  canonical `workspace:{userId}` scope.
- `apps/web/src/page/loggedIn/update.ts` loads the workspace snapshot on
  logged-in init, applies workspace `missions` patches to the sidebar, records
  cursors, reconciles matching pending mutation IDs, and reloads a snapshot on
  cursor gaps.
- `apps/web/src/subscriptions.ts` opens a workspace WebSocket stream from the
  last cursor and emits Foldkit messages for connection state, patches, and
  cursor gaps.
- `SucceededLaunchAutopilotRun` still bridges the launch response into a
  sidebar mission row immediately so new chats appear without waiting for a
  full-page refresh; the committed sync patch remains the authoritative
  reconciliation path.
- Focused reducer and subscription tests cover launch insertion, snapshot
  projection, live mission patches, cursor-gap reloads, and workspace stream
  dependency calculation.

Implemented product-surface conversion:

- Mission route entry now requests `thread:{threadId}` snapshots instead of
  immediately fetching `/api/omni/agent-runs/:id`.
- Active runs subscribe to `agent-run:{runId}` streams, while thread routes
  subscribe to `thread:{threadId}` streams. The browser still keeps the
  workspace stream open for sidebar mission changes.
- Sync `agent_runs` and `agent_run_events` collections are converted into the
  same run detail shape used by the existing OpenCode-style timeline renderer.
  This keeps event grouping and the run metadata dialog consistent across sync
  and compatibility API paths.
- New launch responses request the corresponding `agent-run:{runId}` snapshot
  immediately after the accepted response so replay and refresh parity begin
  without waiting for a full browser refresh.
- Active-run polling is demoted to fallback behavior. The subscription emits
  poll requests only after the relevant `agent-run:{runId}` sync connection is
  failed or closed.
- Empty or failed thread sync snapshots fall back to the legacy detail API while
  the migration is in progress.
- Runner event sync projections now include sanitized `payloadJson` and
  `externalEventId` so debug metadata remains inspectable through the info
  dialog without dumping it into the primary chat timeline.

IndexedDB tables:

```text
collections
  scope
  collection
  id
  value_json
  updated_at

cursors
  scope
  seq

pending_mutations
  mutation_id
  scope
  command
  payload_json
  status
  created_at
```

On reconnect:

```text
1. Load cached state.
2. Open WebSocket with last cursor.
3. Replay server changes.
4. Re-send pending mutations.
5. Server dedupes by mutation_id.
```

## D1 Schema

Use real product tables plus a durable sync outbox.

```sql
CREATE TABLE sync_scopes (
  scope TEXT PRIMARY KEY,
  last_seq INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE sync_changes (
  scope TEXT NOT NULL,
  seq INTEGER NOT NULL,
  collection TEXT NOT NULL,
  op TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  value_json TEXT,
  patch_json TEXT,
  mutation_id TEXT,
  actor_id TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (scope, seq)
);

CREATE INDEX sync_changes_scope_seq_idx
  ON sync_changes(scope, seq);

CREATE TABLE sync_mutations (
  mutation_id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  actor_id TEXT NOT NULL,
  status TEXT NOT NULL,
  result_json TEXT,
  created_at TEXT NOT NULL
);
```

Example product table:

```sql
CREATE TABLE thread_messages (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL,
  org_id TEXT NOT NULL,
  author_id TEXT,
  body_json TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  deleted_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX thread_messages_thread_idx
  ON thread_messages(thread_id, created_at);
```

Sequence assignment must be serialized per scope. The cleanest first version
is to route scope mutations through the scope Durable Object, let that object
assign `seq`, and have it call Effect/D1 repositories. If writes go directly to
the Worker first, the Worker must use a transactionally safe `sync_scopes`
update pattern and then poke the Durable Object.

## Durable Object Versus Agent

Use raw Durable Objects for the generic sync substrate first:

```text
SyncRoomDurableObject
  -> WebSocket lifecycle
  -> hibernation-aware connection handling
  -> replay from D1 sync_changes
  -> broadcast patches
  -> presence
  -> cursor gap handling
```

Use Agents SDK when the thing is actually agent-shaped:

```text
ThreadAgent
  -> thread-specific live state, human approval prompts, agent-facing methods

AgentRunAgent
  -> runner heartbeats, status fanout, closeout coordination

DeployAgent
  -> SHC/GCloud deploy state, receipts, rollback coordination

PublicAgent
  -> published agent state, counters, public-facing live behavior
```

Cloudflare Durable Objects are the lower-level coordination primitive.
Cloudflare Agents are built on Durable Objects and add Agent-oriented state,
SQL, WebSockets, scheduling, workflows, queue/retry helpers, chat, MCP, and
React hooks. That is useful for agent-shaped objects, but the generic database
sync engine should not be hidden inside magical agent abstractions.

## Cloudflare Resource Map

```text
Worker API
  -> OpenAuth
  -> Effect command/query handlers
  -> snapshot endpoints
  -> mutate endpoints
  -> runner callback endpoints
  -> artifact signed URL endpoints

D1
  -> product tables
  -> sync_scopes
  -> sync_changes
  -> sync_mutations
  -> import/parity state

Durable Objects
  -> SyncRoomDurableObject per scope
  -> hibernating WebSocket fanout
  -> replay cursors and presence

Agents SDK
  -> ThreadAgent / AgentRunAgent / DeployAgent / PublicAgent
  -> agent-specific RPC, schedules, workflows, durable execution

R2
  -> artifacts
  -> logs
  -> screenshots
  -> transcripts
  -> generated bundles

Queues
  -> runner event ingest
  -> artifact processing
  -> redaction jobs
  -> fanout repair
  -> dead-letter queues

Workflows
  -> run closeout
  -> approval waits
  -> deploy promotion / rollback
  -> migration backfills
```

## First Implementation Slice

Build one thing end to end:

```text
Live thread workroom
```

Collections:

```text
thread_messages
workroom_events
agent_runs
agent_run_events
approvals
receipts
artifact_refs
```

Cloudflare resources:

```text
D1:
  product tables
  sync_scopes
  sync_changes
  sync_mutations

Durable Object:
  ThreadSyncRoom

Worker routes:
  GET  /api/sync/thread/:id/snapshot
  GET  /api/sync/thread/:id/stream
  POST /api/sync/thread/:id/mutate

R2:
  artifact blobs

Queue:
  runner event ingest

Workflow:
  run closeout / approval wait
```

Foldkit app behavior:

```text
init
  -> load snapshot command
  -> connect stream managed resource

PatchReceived
  -> apply sync patch
  -> reconcile optimistic mutation

MessageSubmitted
  -> add pending local message
  -> send mutation command

MutationRejected
  -> remove pending row
  -> show typed error
```

## Conflict Handling

Start with server authority, not CRDTs.

Rules:

- every mutation has a globally unique `mutationId`;
- every scope patch has monotonic `seq`;
- every update can include `expectedVersion`;
- duplicate mutation ids return the original result;
- optimistic state is pending until a matching patch arrives;
- server rejection removes or marks the pending item;
- cursor gaps trigger snapshot refresh or replay from the last known cursor.

Do not start with full offline CRDT behavior. The first offline-capable target
is:

```text
online realtime sync
reconnect replay
optional IndexedDB cache
pending mutation queue
server idempotency
expectedVersion conflict checks
```

## Migration Model

### Phase 0: Establish openagents

- Use `openagents` as the standalone repo on `main`.
- Use Bun workspaces for `apps/*`, `workers/*`, and `packages/*`.
- Use Foldkit + Vite for the webapp in `apps/web`.
- Put the Cloudflare Worker API, Durable Object surface, Wrangler config, and
  D1 migrations in `workers/api`.
- Put reusable OpenAgents Sync protocol and runtime helpers in `packages/*`.
- Use Workers Static Assets or Cloudflare Worker asset handling for the built
  webapp.
- Add Effect, Effect Schema, OpenAuth, Wrangler, D1, R2, Queues, Workflows,
  Durable Objects, and Agents SDK.
- Do not add TanStack DB, Electric, or Postgres.

Success criteria:

- no Convex runtime dependency;
- no TanStack DB runtime dependency;
- no Electric runtime dependency;
- no Postgres runtime dependency;
- local dev can run the Worker API and Foldkit app;
- CI can typecheck, test, and validate D1 migrations.

### Phase 1: Effect domain and sync schema

- Define Effect Schema for `SyncPatch`, `SyncCommand`, snapshots, cursors,
  mutation results, auth context, runner events, artifact refs, approvals, and
  receipts.
- Define Effect services for D1 repositories, R2 artifacts, Queue publishing,
  Workflow starting, SyncRoom routing, and OpenAuth identity.
- Define fake services for tests.

Success criteria:

- domain code compiles without Cloudflare imports;
- every external boundary validates with Effect Schema;
- command handlers are idempotent by design;
- authz errors are typed and fail closed.

### Phase 2: D1 authority and sync outbox

- Create product tables for the first live-thread slice.
- Create `sync_scopes`, `sync_changes`, and `sync_mutations`.
- Implement snapshot queries.
- Implement append mutation commands.
- Add import/parity scaffolding for Vortex/Convex later.

Success criteria:

- migrations apply locally and remotely;
- D1 tests cover idempotency, sequence assignment, authz filtering, and
  replay;
- no secrets or large blobs are stored in D1.

### Phase 3: Durable Object sync room

- Implement `ThreadSyncRoom`.
- Add WebSocket hibernation support.
- Authenticate sockets.
- Replay missed `sync_changes` from D1.
- Broadcast future patches.
- Track presence separately from durable product state.

Success criteria:

- reconnect from cursor works;
- monotonic sequence order is preserved;
- hibernation does not lose connection metadata needed for replay;
- cursor gaps force safe recovery.

### Phase 4: Foldkit workroom app

- Model snapshot state, collections, cursors, pending mutations, and connection
  state in the Foldkit Model.
- Use Managed Resources for scope WebSockets.
- Use Subscriptions for incoming patch streams.
- Use Commands for snapshot fetches and mutations.
- Use Story tests for update logic and Scene tests for user flows.

Success criteria:

- no React state or hooks;
- no TanStack DB;
- every patch enters through a typed Message;
- optimistic messages reconcile through matching `mutationId`.

### Phase 5: Runner and artifacts

- Wire SHC OpenCode event ingest through Worker/Queue.
- Keep GCloud as a backup runner using the same event envelope.
- Store artifact blobs in R2.
- Store artifact refs in D1 and stream them through OpenAgents Sync.
- Use Workflows for run closeout and approval waits.

Success criteria:

- SHC and GCloud emit equivalent runner envelopes;
- failed SHC dispatch can fail over without duplicate receipts;
- artifact refs are replayable and redaction-aware.

### Phase 6: Vortex/Convex migration

- Export Vortex Convex data by table family.
- Transform into Effect Schema/D1/R2 records.
- Import into `openagents` staging.
- Run parity checks against Vortex projections.
- Document intentional model-boundary changes.

Success criteria:

- import can resume from checkpoints;
- parity reports identify missing/changed rows by stable ids;
- no private secrets are exported into tracked files or logs;
- rollback means returning traffic to Vortex, not enabling Convex in
  `openagents`.

## Tests And Verification

### Sync protocol tests

- `SyncPatch` and `SyncCommand` schemas reject invalid payloads.
- Patch application is deterministic.
- Replaying the same patch sequence produces the same Model.
- Duplicate `mutationId` returns the original mutation result.
- Cursor gaps trigger replay or snapshot refresh.

### D1 tests

- migrations apply locally and remotely;
- `sync_changes` preserves `(scope, seq)` uniqueness;
- mutation idempotency works under retry;
- authz filters prevent cross-tenant reads;
- snapshots return a cursor matching the latest included change.

### Durable Object tests

- `ThreadSyncRoom` authenticates sockets;
- reconnect with cursor replays missed changes;
- WebSocket hibernation does not break replay;
- presence is not treated as durable product state;
- fanout preserves sequence order.

### Foldkit tests

- Story tests cover `PatchReceived`, optimistic mutation, rejection, reconnect,
  and cursor gap messages.
- Scene tests cover loading a workroom, sending a message, receiving a runner
  event, and reconnect replay.
- crash reporting includes Model, Message, and error.
- WebSocket Managed Resource acquisition and cleanup are tested.

### Runner tests

- SHC OpenCode and GCloud backup emit the same event envelope shapes.
- runner callbacks enqueue and replay through the sync log.
- artifact upload writes R2 object and D1 metadata.
- Workflow closeout does not duplicate receipts.

## Recommendation

Proceed with a full Convex replacement in `openagents` using a
Cloudflare-only data plane and Foldkit webapp.

The first implementation issue should be:

```text
Scaffold the Cloudflare-only foundation in openagents with Foldkit/Vite
webapp, Effect domain packages, OpenAuth, Wrangler, D1/R2/Queues/Workflows,
Durable Object sync room, and Agents SDK bindings. Do not add TanStack DB,
Electric, or Postgres.
```

The second implementation issue should be:

```text
Define OpenAgents Sync schemas and Effect services:
SyncPatch, SyncCommand, Snapshot, Cursor, mutation idempotency,
D1 repositories, R2 artifact refs, Queue publishing, Workflow starts,
and SyncRoom routing.
```

The third implementation issue should be:

```text
Implement the first Cloudflare-native live thread workroom:
D1 product tables + sync outbox, ThreadSyncRoom WebSocket replay/fanout,
Foldkit Model/Message/update/Command client, OpenAuth authz,
and SHC OpenCode runner event ingest.
```

The final target is:

```text
Foldkit = Effect-native webapp shell
OpenAgents Sync = product-specific realtime protocol
Effect = domain authority, policy, command handling, schemas
Cloudflare = authoritative data/control/auth/realtime substrate
D1 = source of truth and sync outbox
Durable Objects = generic scope sync and WebSocket hibernation
Agents SDK = agent-shaped coordination and RPC
R2 = blobs and artifacts
Queues / Workflows = async durable processing
SHC OpenCode = primary execution substrate
GCloud = backup execution substrate using the same contract
```

## Reference Links

### Vortex Source Material

- Convex service: `../vortex/server/services/ConvexService.ts`
- Convex schema: `../vortex/convex/schema.ts`
- Convex client provider: `../vortex/components/ConvexClientProvider.tsx`
- App service graph: `../vortex/server/AppLayer.ts`
- Prior SHC/OpenCode/Bun audit:
  `../vortex/docs/omni/2026-06-02-effect-first-openauth-opencode-codex-cloudflare-audit.md`

### OpenAgents Autopilot

- Repo README: `README.md`
- Repo agent instructions: `AGENTS.md`
- Root workspace manifest: `package.json`
- Web app package: `apps/web/package.json`
- Worker package: `workers/api/package.json`
- Sync protocol package: `packages/sync-schema/package.json`

### Foldkit Local Reference

- Installed Foldkit package README: `node_modules/foldkit/README.md`
- Workspace Foldkit repo: `../projects/repos/foldkit/`
- Foldkit examples index: `../projects/foldkit-examples.md`
- Foldkit README: `../projects/repos/foldkit/README.md`
- Foldkit WebSocket example:
  `../projects/repos/foldkit/examples/websocket-chat/src/main.ts`
- Foldkit auth example: `../projects/repos/foldkit/examples/auth/src/main.ts`
- Foldkit query-sync example:
  `../projects/repos/foldkit/examples/query-sync/src/main.ts`
- Foldkit Vite plugin:
  `../projects/repos/foldkit/packages/vite-plugin-foldkit/README.md`

### Cloudflare

- Durable Objects: `https://developers.cloudflare.com/durable-objects/`
- Durable Objects WebSockets:
  `https://developers.cloudflare.com/durable-objects/best-practices/websockets/`
- D1: `https://developers.cloudflare.com/d1/`
- R2: `https://developers.cloudflare.com/r2/how-r2-works/`
- Queues: `https://developers.cloudflare.com/queues/`
- Workflows: `https://developers.cloudflare.com/workflows/`
- Workers Static Assets:
  `https://developers.cloudflare.com/workers/static-assets/`
- Agents SDK overview: `https://developers.cloudflare.com/agents/`
- Agents WebSockets:
  `https://developers.cloudflare.com/agents/api-reference/websockets/`
- Agents API:
  `https://developers.cloudflare.com/agents/api-reference/agents-api/`

### Removed Options

- TanStack DB Electric Collection:
  `https://tanstack.com/db/latest/docs/collections/electric-collection`
- TanStack DB overview: `https://tanstack.com/db/latest/docs/overview`
