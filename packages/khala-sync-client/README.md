# @openagentsinc/khala-sync-client

Client engine for **Khala Sync** (contracts in `packages/khala-sync`, spec
in `docs/khala-sync/SPEC.md` §6, server in `packages/khala-sync-server`).

Components (KS-5 workstream):

- **Local store** — ✅ shipped on `bun:sqlite` (KS-5.1, Khala Code
  desktop; SQLite-WASM/`opfs-sahpool` + SharedWorker single-writer on web,
  later lane). Holds confirmed entities per scope, durable cursors, the
  FIFO pending-mutation queue, and client identity — **server-confirmed
  state only** (SPEC §7 invariant 2); optimistic effects never touch disk.
- **Optimistic mutators + rebase** — named client mutators apply to an
  **in-memory overlay only** (the durable store holds server-confirmed
  state exclusively — Linear's rule). On every delta: rewind overlay,
  apply confirmed entries, re-apply still-unconfirmed mutations, reveal
  atomically. Mutators are pure and replay-safe; server outcome wins.
- **Sync session** — per-scope state machine
  `idle → bootstrapping → catching_up → live`, with `must_refetch` from any
  state. Transport = hibernated WebSocket live tail + HTTP bootstrap and
  offset-resumable catch-up. The durable cursor, not the connection, is the
  source of truth; reconnect resumes from `(scope, cursor)`.
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

The durable store never contains optimistic effects — rebase and the
optimistic overlay are the in-memory layer's job (KS-5.2).
