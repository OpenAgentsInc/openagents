# @openagentsinc/khala-sync-client

Client engine for **Khala Sync** (contracts in `packages/khala-sync`, spec
in `docs/khala-sync/SPEC.md` §6, server in `packages/khala-sync-server`).

Components (KS-5 workstream):

- **Local store** — SQLite (`bun:sqlite` on Khala Code desktop;
  SQLite-WASM/`opfs-sahpool` + SharedWorker single-writer on web, later
  lane). Holds confirmed entities per scope, durable cursors, the FIFO
  pending-mutation queue, and client identity.
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
