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

## Locking

Two locks, and they compose: every store-wide operation (create, delete,
import upsert) takes the store lock and then the target session's named
lock, so it serialises against any in-flight `update` of that session — a
delete can never be undone by a status fold that was mid-update when the
user confirmed it. `update` takes only the session lock, so the acquisition
order is store→session everywhere and cannot deadlock. Session semaphores
live for the store instance and are never discarded.

## Schema versioning

Envelopes are versioned from the first commit; the current version is
`CURRENT_SCHEMA_VERSION` (2: v2 added the definitive `failure` state to
effect-ledger entries). Migrations are sequential and rewrite every record
at store open (the Boltz pattern); the same shipped chain is the DEFAULT
for `importPrivateHistory`, so an older build's export always ingests. A
record or import from a FUTURE version refuses loudly
(`UnsupportedSchemaVersionError`) instead of being silently mangled, and a
migration rewrite runs the custody tripwire before it commits.

## The effect ledger's three states

An external effect (wallet call, broadcast) is PENDING (requested, no
outcome — the reload guard blocks, History pins the exit), SUCCEEDED (the
persisted result suppresses the resume callback), or DEFINITIVELY FAILED
(`recordEffectFailure` — the wallet reported no side effect happened: the
guard releases and `priorEffectResult` returns null so a retry legitimately
re-drives the exact persisted request). An UNKNOWN outcome must stay
pending; only a definitive no-effect report may be recorded as failure.

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
financial history. The tripwire runs per session on the export path itself.
Import validates structurally, re-verifies per-session digests, runs the
tripwire, migrates older versions (shipped chain by default), and is
all-or-nothing with typed refusals (`HistoryImportError`): a validation
refusal writes nothing, and a mid-apply driver failure (quota) rolls this
import's writes back before refusing (`storage_failure`). Surfaces render
refusals through `importRefusalKeyOf` and present `EXPORT_SENSITIVITY_KEY`
alongside the download. Behaviour contracts:
`openagents_web.swap_history.export_import_round_trip.v1` and
`openagents_web.swap_history.resume_after_reload.v1` in
`@openagentsinc/behavior-contracts`.
