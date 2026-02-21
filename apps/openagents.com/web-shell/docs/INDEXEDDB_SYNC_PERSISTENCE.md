# IndexedDB Sync Persistence (OA-RUST-081)

## Scope

The web shell persists Khala resume data and local stream view state in IndexedDB.

- Database: `openagents.web.sync`
- Version: `2`
- Stores:
  - `sync_state` (`primary` key): encoded `PersistedSyncState`
  - `view_state` (`primary` key): encoded `PersistedViewState`
  - `meta`: schema metadata (`view_schema_version`)

## Persisted State

1. `PersistedSyncState`
   - `topic_watermarks`
   - `subscribed_topics`
   - `updated_at_unix_ms`
2. `PersistedViewState`
   - `active_worker_id`
   - `last_seq`
   - `updated_at_unix_ms`

The web shell resumes using persisted watermarks to build deterministic `resume_after` payloads for Khala subscribe.

## Migration Strategy

1. IndexedDB store migration is handled by DB version upgrade (`WEB_SYNC_DB_VERSION = 2`) and object-store creation on `onupgradeneeded`.
2. Payload migration is schema-based:
   - `sync_state` supports legacy v0 payloads via `decode_sync_state` migration.
   - `view_state` supports legacy payloads without `schema_version`.
3. One-time legacy bootstrap migration:
   - If IndexedDB has no snapshot, read legacy localStorage key `openagents.web.sync.v1`.
   - Decode legacy payload.
   - Write normalized snapshot to IndexedDB.
   - Remove legacy localStorage key.

## Corruption / Incompatible Recovery

If IndexedDB or legacy payload decode fails:

1. Clear IndexedDB snapshot stores.
2. Clear legacy localStorage key.
3. Reset in-memory sync runtime state.
4. Continue boot with empty sync state.

This guarantees the app is never deadlocked by bad local persistence.

## Observability

Migration/reset events are surfaced in boot diagnostics via `set_boot_phase` details and `last_error` updates.

Typical strings:

- `migrated sync persistence payload in IndexedDB`
- `migrating legacy sync persistence from localStorage to IndexedDB`
- `resetting invalid indexeddb sync persistence payload: ...`
- `resetting invalid legacy sync persistence payload: ...`

## Verification

- Unit tests in `crates/openagents-client-core/src/web_sync_storage.rs` cover:
  - current schema round-trip
  - legacy migration
  - downgrade rejection (`UnsupportedSchema`)
  - corruption handling (`InvalidPayload`)
- WASM build check:

```bash
cargo check -p openagents-web-shell --target wasm32-unknown-unknown
```
