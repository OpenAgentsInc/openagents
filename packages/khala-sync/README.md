# @openagentsinc/khala-sync

Wire and domain contracts for **Khala Sync**, the owned replication
substrate: Cloud SQL Postgres (authoritative) → Cloudflare edge (per-scope
Durable Object hubs) → SQLite clients (desktop/web/mobile).

> Naming: always the two-word compound **Khala Sync** / `khala-sync`. Bare
> "Khala" is the collective-intelligence product (Episode 242), never this
> engine.

This package is the single source of truth both sides depend on:

- **Branded primitives** — `SyncScope`, `SyncVersion` (per-scope dense
  monotonic server-assigned versions), `MutationId`, `ClientId` /
  `ClientGroupId`, `EntityType` / `EntityId`, `SyncSchemaVersion`,
  `MutatorName`.
- **Scope constructors** — `personalScope`, `teamScope`, `agentRunScope`,
  `threadScope`, `fleetRunScope`, `publicScope` (aligned with the
  `@openagentsinc/sync-worker` taxonomy).
- **Changelog** — `ChangelogEntry` (post-image v1 model; deletes are
  tombstones; apply idempotent by `(scope, version, entityType, entityId)`).
- **Mutations** — `MutationEnvelope` / `MutationResult` for named,
  server-authoritative mutators with in-band rejection (rejections ACK and
  never block the client queue).
- **Wire protocol** — `PushRequest`/`PushResponse`,
  `BootstrapRequest`/`BootstrapResponse` (snapshot pages stitched to a
  cursor), `LogPage` (offset-resumable catch-up), and the WebSocket
  `LiveFrame` union (`DeltaFrame`, `MutationAckFrame`, `MustRefetchFrame`,
  `PingFrame`).
- **Error taxonomy** — `SyncError` with the closed `SyncErrorCode` set.
- **Canonical JSON** — `canonicalJson` / `CanonicalJsonError`, the required
  producer for all `postImageJson` (and mutator `argsJson`) strings.

## Canonical JSON (`postImageJson`)

All post-images MUST be produced via the exported
`canonicalJson(value: unknown): string` helper — on the server and on every
client — so that byte-wise comparison, hashing, and diffing of post-images
is stable across implementations. Never hand `JSON.stringify` output into
`postImageJson`: insertion-order-dependent key order breaks hash/diff
stability.

Rules (a strict subset of RFC 8785 / JCS):

- **Key order**: object keys are sorted recursively, lexicographic by UTF-16
  code unit (plain `Array.prototype.sort` on the key strings).
- **`undefined`**: object members whose value is `undefined` are dropped
  (matching `JSON.stringify`); `undefined` array elements throw
  `CanonicalJsonError`.
- **Numbers**: must be finite — `NaN` / `±Infinity` throw
  `CanonicalJsonError` (with a `path` to the offending value); `-0`
  normalizes to `0`; tokens use the shortest ES round-trip form
  (`JSON.stringify`), matching RFC 8785.
- **Allowed values**: `null`, booleans, finite numbers, strings, arrays, and
  objects of those. Functions, symbols, and bigints throw
  `CanonicalJsonError`.
- **Whitespace**: none.

## Conformance fixtures

`fixtures/` contains golden wire-format JSON for every protocol message
type — `PushRequest`, `PushResponse`, `BootstrapRequest`,
`BootstrapResponse` (paging + final), `LogPage`, every `LiveFrame` variant
(`DeltaFrame`, `MutationAckFrame`, `MustRefetchFrame`, `PingFrame`),
`SyncError`, and `ChangelogEntry` (upsert + tombstone). File naming is
`<MessageType>[.<variant>].json`.

These fixtures are the **cross-implementation conformance contract**: any
implementation of the protocol (server, SQLite client, mobile) must decode
each fixture and re-encode it to a deeply-equal JSON value.
`src/conformance.test.ts` enforces this for the TypeScript codecs. Do not
edit a fixture to make a code change pass — that is a wire-protocol change
and needs a protocol-version review.

## Property tests

`src/property.test.ts` derives a fast-check arbitrary from every top-level
message schema via `Schema.toArbitrary` (effect v4) and asserts
arbitrary → encode → JSON string round-trip → decode → re-encode is stable.
Runs are deterministic (fixed seed, no wall-clock input).

Related packages:

- `@openagentsinc/khala-sync-server` — Postgres substrate, mutator engine,
  capture, `KhalaSyncHubDO`.
- `@openagentsinc/khala-sync-client` — local store, transport, rebase.

Spec: `docs/khala-sync/SPEC.md`. Design rationale:
`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`.

```sh
bun test packages/khala-sync
bun run --cwd packages/khala-sync typecheck
```
