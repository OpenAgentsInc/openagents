# @openagentsinc/khala-sync-client

Client engine for **Khala Sync** (contracts in `packages/khala-sync`, spec
in `docs/khala-sync/SPEC.md` §6, server in `packages/khala-sync-server`).

Components (KS-5 workstream):

- **Local store** — ✅ shipped on `bun:sqlite` (KS-5.1, Khala Code
  desktop; SQLite-WASM/`opfs-sahpool` + SharedWorker single-writer on web,
  later lane). Holds confirmed entities per scope, durable cursors, the
  FIFO pending-mutation queue, and client identity — **server-confirmed
  state only** (SPEC §7 invariant 2); optimistic effects never touch disk.
- **Optimistic mutators + rebase** — ✅ shipped (KS-5.2, `createOverlay`).
  Named client mutators apply to an **in-memory overlay only** (the
  durable store holds server-confirmed state exclusively — Linear's
  rule). On every delta: rewind overlay, apply confirmed entries,
  re-apply still-unconfirmed mutations, reveal atomically. Mutators are
  pure and replay-safe; server outcome wins. Verified by model-based
  property tests (SPEC §8): 50 seeded random client/server interleavings
  converge to server state with zero optimistic residue, the durable
  SQLite tables are inspected directly at every step, and rebase is
  deterministic.
- **Sync session + transport** — ✅ shipped (KS-5.3,
  `createKhalaSyncSession` + `createHttpKhalaSyncTransport`). Per-scope
  state machine `idle → bootstrapping → catching_up → live`, with
  `must_refetch` from any state (server `MustRefetchFrame` or a
  `cursor_behind_retained_window` error → reset + automatic re-bootstrap,
  bounded jittered retries). Transport is an injectable seam
  (`KhalaSyncTransport`); the production implementation speaks the SPEC §3
  routes (`POST /api/sync/push`, `POST /api/sync/bootstrap`,
  `GET /api/sync/log`, `WS /api/sync/connect`) over fetch + WebSocket with
  bearer auth, every payload round-tripped through the khala-sync codecs.
  The durable cursor, not the connection, is the source of truth:
  reconnect resumes catch-up from `(scope, cursor)` with jittered
  exponential backoff, forever, until `unsubscribe`/`close`. The push loop
  drains the durable FIFO queue in batches; rejections ACK in-band
  (surfaced through `onRejection`) and never block the queue; a terminal
  fault parks the queue until the next mutate/subscribe re-kick. All
  timing is injected (`sleep`/`random`) — no wall-clock reads in tested
  logic; the suite runs against a deterministic fake transport.
- **v1 offline contract** — online-optimistic: reads work offline, pushes
  wait for connectivity (bounded queue, honest expiry).

First consumer: the Khala Code desktop fleet cockpit (KS-6).

## Local store usage (`bun:sqlite`)

```ts
import { Effect } from "effect"
import { openKhalaSyncStore } from "@openagentsinc/khala-sync-client"

const store = openKhalaSyncStore("/path/to/store.sqlite") // or ":memory:"

// Apply a confirmed delta: entries + cursor land in ONE SQLite
// transaction. Idempotent under at-least-once redelivery (stale entry
// versions are skipped); a cursor behind the stored cursor fails typed
// with reason "cursor_regression".
await Effect.runPromise(store.applyConfirmed(scope, entries, nextCursor))

// MustRefetch: replace the scope from a bootstrap snapshot atomically.
await Effect.runPromise(store.resetScope(scope, snapshotEntities, cursor))

// Durable FIFO push queue: mutationId must be exactly last pending/acked
// + 1 (reason "mutation_id_gap" otherwise); ack drops rows through the id.
await Effect.runPromise(store.enqueueMutation(envelope))
await Effect.runPromise(store.ackMutations(lastMutationId))

await Effect.runPromise(store.close())
```

Tables: `entities(scope, entity_type, entity_id, post_image_json, version)`
(PK `scope, entity_type, entity_id`), `cursors(scope, version)`,
`pending_mutations(mutation_id, name, args_json, created_at)`, and
`meta(key, value)` for `client_id` / `client_group_id` / `schema_version` /
`last_mutation_id`. WAL journaling is enabled on file databases.

## Optimistic overlay + rebase (`createOverlay`)

```ts
import { Effect } from "effect"
import {
  type ClientMutator,
  createOverlay,
  openKhalaSyncStore,
} from "@openagentsinc/khala-sync-client"
import { canonicalJson, MutatorName, SyncScope } from "@openagentsinc/khala-sync"

const setTitle: ClientMutator<{ scope: string; id: string; title: string }> = {
  name: MutatorName.make("task.setTitle"),
  // pure + replay-safe: re-executed on every rebase against the
  // then-current confirmed view; args round-trip through canonical JSON
  apply: (args, view) => [{
    kind: "upsert",
    scope: SyncScope.make(args.scope),
    entityType: "task",
    entityId: args.id,
    postImageJson: canonicalJson({ title: args.title }),
  }],
}

const store = openKhalaSyncStore("/path/to/store.sqlite")
// the mutator registry: every mutator ever mutated with must be here so
// queued mutations replay on rebase and across restarts
const overlay = await Effect.runPromise(createOverlay(store, [setTitle]))

const view = await Effect.runPromise(overlay.read(scope)) // live view
view.get("task", "t1") // postImageJson | undefined (confirmed ⊕ optimistic)
view.list("task")      // ordered by entityId

// Optimistic write: durable intent on the FIFO queue + in-memory effects.
await Effect.runPromise(overlay.mutate(setTitle, { scope, id: "t1", title: "hi" }))

// Confirmed delta → store.applyConfirmed, then the rebase: rewind to
// confirmed, re-apply still-pending mutations in mutationId order,
// reveal atomically (readers never see a half-rebased view).
await Effect.runPromise(overlay.onConfirmed(scope, entries, cursor))

// Server ack (rejections ack too): drop those mutations' queue rows and
// overlay contributions, rebuild, reveal atomically.
await Effect.runPromise(overlay.onAck(lastMutationId))

const unsubscribe = overlay.subscribe((scope) => rerender(scope))
```

## The two hard invariants (SPEC §7)

1. **Optimistic effects never touch the durable store** (invariant 2,
   Linear's rule). `mutate` writes only the mutation *intent* to the
   durable FIFO queue; its effects live exclusively in the in-memory
   overlay. The `entities`/`cursors` tables are written by the confirmed
   paths alone, so the local DB is always reconstructible from the server
   changelog. The property suite inspects the raw SQLite tables after
   every step to enforce this.
2. **Apply is idempotent; the durable cursor is the source of truth**
   (invariant 4). At-least-once redelivery of confirmed batches is a
   no-op on the end state (stale entry versions are skipped, an equal
   cursor is a legal redelivery), and reconnect resumes from the durable
   `(scope, cursor)` — never from connection state.
