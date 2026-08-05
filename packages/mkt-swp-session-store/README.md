# @openagentsinc/mkt-swp-session-store

The local swap session store for the MKT-SWP web surface (openagents#9320,
SWAP-5): persistence, History view-model, app-wide resume planning, and the
export/import escape hatch. This is the layer that makes the doomsday drill
real in a browser — if the relay disappears mid-swap, the user's own records
must be enough to complete or refund.

## Storage and durability

Records live behind a `StringKv` port (`kv.ts`) with **no** atomicity
assumptions. Above it, `journal.ts` makes every write safe on any backend:

- every committed value is an envelope whose SHA-256 digest covers the
  schema version, write sequence, and canonical payload — a partially
  written record cannot verify and is **never** loadable as complete;
- writes are two-phase (staging key, base key, staging delete), so every
  single-crash point recovers to a complete old or complete new record;
  only external corruption of both copies refuses loudly
  (`TornSessionRecordError`).

Shipped bindings: in-memory (tests/SSR) and `webStorageStringKv`
(localStorage-shaped). The IndexedDB binding is app-shell wiring
(SWAP-0/SWAP-7): implement `StringKv` over an object store there; its
transactions only strengthen the journal.

## Schema versioning

Envelopes are versioned from the first commit (`CURRENT_SCHEMA_VERSION = 1`).
Migrations are sequential and rewrite every record at store open (the Boltz
pattern). A record or import from a FUTURE version refuses loudly
(`UnsupportedSchemaVersionError`) instead of being silently mangled.

## Custody boundary (SWAP-4)

The store persists **public data only**: signed records, exit packages,
public commitments, effect requests/results, and opaque **non-secret
handles** (`secretHandles`) into the SWAP-4 secret store (openagents#9319,
not yet built — `SwapSecretStoreProbe` is the injection point). A recursive
tripwire (`secret-boundary.ts`) refuses payloads that appear to carry
seeds, keys, preimages, macaroons, NWC strings, or nonces
(`swp_secret_material_forbidden`), and never logs or echoes the value.

## Export/import

`exportPrivateHistory` / `importPrivateHistory` — named for what the
document is: no keys, no spend authority, but the user's complete private
financial history. Import validates structurally, re-verifies per-session
digests, runs the tripwire, migrates older versions, and is all-or-nothing
with typed refusals (`HistoryImportError`). Behaviour contracts:
`openagents_web.swap_history.export_import_round_trip.v1` and
`openagents_web.swap_history.resume_after_reload.v1` in
`@openagentsinc/behavior-contracts`.
