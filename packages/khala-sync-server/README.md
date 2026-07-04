# @openagentsinc/khala-sync-server

Server substrate for **Khala Sync** (see `packages/khala-sync` for the wire
contracts and `docs/khala-sync/SPEC.md` for the spec).

Components (workstreams in parentheses; see the KS epic on GitHub):

- **`migrations/`** — Cloud SQL Postgres schema (KS-2):
  `khala_sync_scopes` (per-scope version counters + retention watermark),
  `khala_sync_changelog` (the transactional outbox / replication log, with
  a `pg_notify` wake trigger), `khala_sync_mutations` (per-client mutation
  ledger), `khala_sync_client_state`.
- **Mutator engine** (KS-3) — `MutatorRegistry` of named,
  server-authoritative mutators; each executes in ONE Postgres transaction:
  permission check → validation → business writes → changelog appends →
  mutation ledger upsert. Rejections are in-band values that ACK the
  mutation; they never block the client queue.
- **Read service** (KS-2) — consistent bootstrap snapshot pages stitched to
  a cursor, and offset-resumable `LogPage` catch-up.
- **Capture** (KS-4) — tails `khala_sync_changelog` over a DIRECT Postgres
  connection (`LISTEN khala_sync_changelog_append` wake + poll fallback —
  Hyperdrive drops LISTEN/NOTIFY) and pushes `DeltaFrame`s to per-scope
  hubs.
- **`KhalaSyncHubDO`** (KS-4) — one Durable Object per scope: recent log
  window in DO SQLite, hibernating WebSockets, offset-resumable HTTP
  catch-up, `MustRefetch` for cursors behind the window. The DO is a cache
  and fan-out layer only; Postgres is authoritative.

Connectivity invariants:

- Worker request paths (push, bootstrap, log) reach Postgres through
  **Hyperdrive** — transaction-mode pooling, so mutators must be
  single-transaction and session state is forbidden.
- Capture and migrations use a direct connection path.

Status: contracts and schema landed; engine/capture/hub implementations land
per the KS-2/KS-3/KS-4 issues.
