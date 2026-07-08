import { Schema as S } from "effect"

/**
 * @openagentsinc/khala-sync — wire and domain contracts for Khala Sync,
 * the owned replication substrate (Cloud SQL Postgres → Cloudflare edge →
 * SQLite clients).
 *
 * Naming rule: always the two-word compound "Khala Sync" / `khala-sync`.
 * Bare "Khala" is the collective-intelligence product (Episode 242) and is
 * never this engine.
 *
 * Spec: docs/khala-sync/SPEC.md
 */

export const KHALA_SYNC_PROTOCOL_VERSION = 1

// Fleet cockpit entity contracts (KS-6.1): post-image shapes for
// scope.fleet_run.<id> changelog entries. Self-contained module (no cycle).
export * from "./fleet.js"

// Capacity-aware dispatch account selection (#8389/#8388): pure selector
// over FleetAccountEntity, shared by khala-sync-server and the published
// Pylon runtime dispatch consumer (see its own doc header for why it lives
// here and not in khala-sync-server).
export * from "./fleet-account-selection.js"

// Owner-private chat entity contracts (MC-1): thread metadata and messages
// for scope.user.<owner> + scope.thread.<threadId> clients.
export * from "./chat.js"

// Public-counter entity contract (KS-6.3): post-image shape for
// scope.public.<channel> counter projections (tokens-served).
// Self-contained module (no cycle).
export * from "./public-counter.js"

// Per-user credit-balance entity contract (issue #8505, Part 2): post-image
// shape for scope.user.<userId> credit-balance projections, mirroring the
// public-counter shape but keyed by user id and appended into the owner's
// personal scope. Self-contained module (no cycle).
export * from "./credit-balance.js"

// Public settled-feed entity contracts (KS-6.4, #8414): post-image shapes
// for the scope.public.settled-feed event/summary projection.
// Self-contained module (no cycle).
export * from "./settled-feed.js"

// Khala Code product-state entity contracts (KS-8.13): thread/team/workspace
// post-image shapes for scope.team.<teamId> + scope.thread.<threadId>
// changelog entries. Self-contained module (no cycle).
export * from "./khala-code.js"

// Khala Code runtime entity contracts (#8370): AI SDK-shaped turns,
// body-free control intents, and private thread-scoped event streams.
export * from "./runtime.js"

// Gym / Harbor live run-progress entity contract (KS-6.5, #8415): post-image
// shape for scope.public.gym-run-progress changelog entries (one entity per
// runRef). Self-contained module (no cycle).
export * from "./gym.js"

// Agent run + goal entity contract (KS-6.6, #8416): post-image shape for
// scope.agent_run.<runId> changelog entries. Self-contained module (no
// cycle).
export * from "./agent-run.js"

// Public tokens-served aggregate snapshot entity contracts (KS-6.7, #8417):
// post-image shapes for scope.public.tokens-served-aggregates (model-mix,
// demand-mix, channel-mix, and per-day history snapshots, one entity per
// window). Self-contained module (no cycle).
export * from "./tokens-served-mix.js"

// Public activity-timeline stored-snapshot entity contract (KS-6.7b, #8421):
// post-image shape for scope.public.activity-timeline (one whole-window
// snapshot entity refreshed on a cron tick, not event-sourced — see the
// module doc for why). Self-contained module (no cycle).
export * from "./activity-timeline-snapshot.js"

// ---------------------------------------------------------------------------
// Branded primitives
// ---------------------------------------------------------------------------

/** Structured scope id, e.g. `scope.team.<teamId>`. Unit of sync/auth/fan-out. */
export const SyncScope = S.String.check(
  S.isPattern(/^scope\.[a-z_]+\.[A-Za-z0-9._:-]+$/),
).pipe(S.brand("SyncScope"))
export type SyncScope = typeof SyncScope.Type

/** Server-assigned, per-scope, dense monotonic version (starts at 1). */
export const SyncVersion = S.Number.check(S.isInt(), S.isGreaterThan(0)).pipe(
  S.brand("SyncVersion"),
)
export type SyncVersion = typeof SyncVersion.Type

/**
 * A log position expressed as a version watermark. Unlike {@link SyncVersion}
 * (entry versions, which start at 1), a watermark of 0 is valid and means
 * "scope start — before the first entry". Used where the protocol must be
 * able to express a position at the very beginning of a scope's log:
 * `LogPage.nextCursor` and the bootstrap snapshot `cursor` of a scope that
 * has no committed versions yet.
 */
export const SyncVersionWatermark = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
).pipe(S.brand("SyncVersionWatermark"))
export type SyncVersionWatermark = typeof SyncVersionWatermark.Type

/** Per-client sequential mutation id (starts at 1). */
export const MutationId = S.Number.check(S.isInt(), S.isGreaterThan(0)).pipe(
  S.brand("MutationId"),
)
export type MutationId = typeof MutationId.Type

/** One logical client installation (one local store). */
export const ClientId = S.String.check(S.isMinLength(1)).pipe(
  S.brand("ClientId"),
)
export type ClientId = typeof ClientId.Type

/**
 * A group of clients sharing one local store lineage (e.g. all tabs of one
 * browser profile). `lastMutationId` is tracked per (clientGroup, client).
 */
export const ClientGroupId = S.String.check(S.isMinLength(1)).pipe(
  S.brand("ClientGroupId"),
)
export type ClientGroupId = typeof ClientGroupId.Type

export const EntityType = S.String.check(
  S.isPattern(/^[a-z][a-z0-9_]*$/),
).pipe(S.brand("EntityType"))
export type EntityType = typeof EntityType.Type

export const EntityId = S.String.check(S.isMinLength(1)).pipe(
  S.brand("EntityId"),
)
export type EntityId = typeof EntityId.Type

/** Client data-schema version; bumped when entity shapes change. */
export const SyncSchemaVersion = S.Number.check(
  S.isInt(),
  S.isGreaterThan(0),
).pipe(S.brand("SyncSchemaVersion"))
export type SyncSchemaVersion = typeof SyncSchemaVersion.Type

// ---------------------------------------------------------------------------
// Scope constructors (aligned with @openagentsinc/sync-worker taxonomy)
// ---------------------------------------------------------------------------

const scope = (kind: string, id: string): SyncScope =>
  SyncScope.make(`scope.${kind}.${id}`)

export const personalScope = (userId: string): SyncScope =>
  scope("user", userId)
export const teamScope = (teamId: string): SyncScope => scope("team", teamId)
export const agentRunScope = (runId: string): SyncScope =>
  scope("agent_run", runId)
export const threadScope = (threadId: string): SyncScope =>
  scope("thread", threadId)
export const fleetRunScope = (fleetRunId: string): SyncScope =>
  scope("fleet_run", fleetRunId)
export const publicScope = (channel: string): SyncScope =>
  scope("public", channel)

const isSyncScope = S.is(SyncScope)

/**
 * True when `userId` can form a valid personal `SyncScope`
 * (`scope.user.<userId>`) — i.e. `personalScope(userId)` will NOT throw.
 *
 * Legacy `email:`-form user IDs contain an `@` (and sometimes `+`) that is
 * outside the entity-id charset (`[A-Za-z0-9._:-]`), so they are
 * scope-incompatible: they can never subscribe to any personal scope on
 * either the server or the mobile client (the same schema runs on both).
 * Lets the credit-balance producer and backfill pre-check without a throw
 * so they can skip such IDs cleanly instead of counting them as failures.
 * See #8557. Broadening the charset is deliberately NOT the fix — identity
 * migration is the only path to sync these accounts.
 */
export const isScopeCompatibleUserId = (userId: string): boolean =>
  isSyncScope(`scope.user.${userId}`)

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

/**
 * A client's durable position in one scope's log. Version 0 is expressed by
 * an absent cursor at the call site (never a SyncVersion of 0).
 */
export class SyncCursor extends S.Class<SyncCursor>("SyncCursor")({
  scope: SyncScope,
  version: SyncVersion,
}) {}

// ---------------------------------------------------------------------------
// Changelog
// ---------------------------------------------------------------------------

export const ChangeOp = S.Literals(["upsert", "delete"])
export type ChangeOp = typeof ChangeOp.Type

/**
 * One changed entity within one committed transaction, in one scope. The
 * post-image is the full entity value (v1: post-images, not diffs), absent
 * for deletes. Apply is idempotent by (scope, version, entityType, entityId).
 */
export class ChangelogEntry extends S.Class<ChangelogEntry>("ChangelogEntry")({
  scope: SyncScope,
  version: SyncVersion,
  entityType: EntityType,
  entityId: EntityId,
  op: ChangeOp,
  postImageJson: S.optionalKey(S.String),
  mutationRef: S.optionalKey(S.String),
  committedAt: S.String,
}) {}

// ---------------------------------------------------------------------------
// Mutations (named, server-authoritative)
// ---------------------------------------------------------------------------

export const MutatorName = S.String.check(
  S.isPattern(/^[a-z][a-zA-Z0-9_.]*$/),
).pipe(S.brand("MutatorName"))
export type MutatorName = typeof MutatorName.Type

export class MutationEnvelope extends S.Class<MutationEnvelope>(
  "MutationEnvelope",
)({
  mutationId: MutationId,
  name: MutatorName,
  argsJson: S.String,
}) {}

export const MutationStatus = S.Literals(["applied", "rejected", "duplicate"])
export type MutationStatus = typeof MutationStatus.Type

/**
 * Per-mutation outcome. Rejections ACK the mutation (advance
 * lastMutationId) and carry the error in-band — they never block the queue.
 */
export class MutationResult extends S.Class<MutationResult>("MutationResult")({
  mutationId: MutationId,
  status: MutationStatus,
  errorCode: S.optionalKey(S.String),
  errorMessageSafe: S.optionalKey(S.String),
}) {}

// ---------------------------------------------------------------------------
// Wire protocol — push
// ---------------------------------------------------------------------------

export class PushRequest extends S.Class<PushRequest>("PushRequest")({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  schemaVersion: SyncSchemaVersion,
  clientGroupId: ClientGroupId,
  clientId: ClientId,
  mutations: S.Array(MutationEnvelope),
}) {}

/**
 * Highest mutation id acked for the pushing `(clientGroup, client)` pair —
 * the ledger watermark, not an id of any one mutation. `0` means nothing has
 * been acked yet (fresh client, empty push, or an all-`out_of_order` batch),
 * which is why this is a plain non-negative int rather than a `MutationId`.
 */
export const LastMutationId = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)
export type LastMutationId = typeof LastMutationId.Type

export class PushResponse extends S.Class<PushResponse>("PushResponse")({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  results: S.Array(MutationResult),
  lastMutationId: LastMutationId,
}) {}

// ---------------------------------------------------------------------------
// Wire protocol — bootstrap
// ---------------------------------------------------------------------------

export class BootstrapRequest extends S.Class<BootstrapRequest>(
  "BootstrapRequest",
)({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  schemaVersion: SyncSchemaVersion,
  scope: SyncScope,
  clientGroupId: ClientGroupId,
  /**
   * Requested entities-scanned-per-page bound. Advisory: the server clamps
   * to its own maximum. Absent ⇒ the server default.
   */
  pageSize: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThan(0))),
  /**
   * Opaque continuation token echoed from the previous page's
   * `nextPageToken`; absent on the first page request (the server pins the
   * snapshot cursor on that request). Pure widening (KS-5.3):
   * pre-existing single-page requests remain valid, and a server that
   * never pages ignores it.
   */
  pageToken: S.optionalKey(S.String),
}) {}

export class BootstrapEntity extends S.Class<BootstrapEntity>(
  "BootstrapEntity",
)({
  entityType: EntityType,
  entityId: EntityId,
  postImageJson: S.String,
}) {}

/**
 * A consistent snapshot page. `cursor` is the scope version at which the
 * snapshot was taken; the client catches up from exactly there. A cursor of
 * 0 (watermark) means the scope had no committed versions at snapshot time.
 */
export class BootstrapResponse extends S.Class<BootstrapResponse>(
  "BootstrapResponse",
)({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  scope: SyncScope,
  entities: S.Array(BootstrapEntity),
  /** Absent while paging; present with the snapshot cursor on the last page. */
  cursor: S.optionalKey(SyncVersionWatermark),
  nextPageToken: S.optionalKey(S.String),
}) {}

// ---------------------------------------------------------------------------
// Wire protocol — CVR diff pull (KS-7.2, #8306; flag-gated v2 surface)
// ---------------------------------------------------------------------------

/**
 * Per-(clientGroup, scope) Client View Record version, allocated by the
 * server on every CVR pull (dense, starts at 1). Design:
 * docs/khala-sync/CVR_DESIGN.md; reference spec: the Replicache row-version
 * strategy. The whole surface is additive and gated behind
 * `KHALA_SYNC_CVR=1` — unflagged deployments never produce or consume it.
 */
export const CvrVersion = S.Number.check(S.isInt(), S.isGreaterThan(0)).pipe(
  S.brand("CvrVersion"),
)
export type CvrVersion = typeof CvrVersion.Type

/**
 * One client-side row the client applied AFTER its last CVR pull (its store
 * version is greater than that pull's snapshot cursor). The hybrid live
 * path (log/DeltaFrame) mutates client state without touching the CVR, so
 * a diff pull must widen its base by these rows or a row acquired live and
 * later deleted+compacted would never be retracted (CVR_DESIGN.md §5).
 */
export class CvrDriftEntry extends S.Class<CvrDriftEntry>("CvrDriftEntry")({
  entityType: EntityType,
  entityId: EntityId,
  version: SyncVersion,
}) {}

export class CvrPullRequest extends S.Class<CvrPullRequest>(
  "CvrPullRequest",
)({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  schemaVersion: SyncSchemaVersion,
  scope: SyncScope,
  clientGroupId: ClientGroupId,
  /**
   * The CVR the client's durable state was last reconciled against. Absent
   * ⇒ no usable CVR (first pull, restart, or post-bootstrap): the server
   * answers in `reset` mode with the full current row set.
   */
  cvrVersion: S.optionalKey(CvrVersion),
  /** Rows applied after the referenced CVR's snapshot (see CvrDriftEntry). */
  drift: S.optionalKey(S.Array(CvrDriftEntry)),
}) {}

/** A retraction: the row left the authorized set — remove it locally. */
export class CvrDel extends S.Class<CvrDel>("CvrDel")({
  entityType: EntityType,
  entityId: EntityId,
}) {}

/**
 * `reset`: `puts` is the COMPLETE current row set — replace scope-local
 * state with exactly it (same client semantics as a bootstrap snapshot).
 * `diff`: apply `puts` and `dels` incrementally to the existing state.
 */
export const CvrPullMode = S.Literals(["reset", "diff"])
export type CvrPullMode = typeof CvrPullMode.Type

export class CvrPullResponse extends S.Class<CvrPullResponse>(
  "CvrPullResponse",
)({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  scope: SyncScope,
  mode: CvrPullMode,
  puts: S.Array(BootstrapEntity),
  dels: S.Array(CvrDel),
  /** The freshly stored CVR; send it back on the next diff pull. */
  cvrVersion: CvrVersion,
  /** Snapshot cursor: stitch with `logPage(afterVersion = cursor)`. */
  cursor: SyncVersionWatermark,
}) {}

// ---------------------------------------------------------------------------
// Wire protocol — catch-up log page
// ---------------------------------------------------------------------------

export class LogPage extends S.Class<LogPage>("LogPage")({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  scope: SyncScope,
  entries: S.Array(ChangelogEntry),
  /**
   * The client's position after applying this page: the highest version in
   * `entries`, or the request's `afterVersion` when the page is empty. A
   * watermark — 0 means "still at scope start" (empty scope, no cursor yet).
   */
  nextCursor: SyncVersionWatermark,
  upToDate: S.Boolean,
}) {}

// ---------------------------------------------------------------------------
// Wire protocol — live frames (WebSocket)
// ---------------------------------------------------------------------------

export class DeltaFrame extends S.TaggedClass<DeltaFrame>()("DeltaFrame", {
  scope: SyncScope,
  entries: S.Array(ChangelogEntry),
  cursor: SyncVersion,
}) {}

export class MutationAckFrame extends S.TaggedClass<MutationAckFrame>()(
  "MutationAckFrame",
  {
    clientId: ClientId,
    lastMutationId: MutationId,
  },
) {}

export const MustRefetchReason = S.Literals([
  "cursor_behind_retained_window",
  "schema_version_unsupported",
  "access_changed",
  "scope_reset",
])
export type MustRefetchReason = typeof MustRefetchReason.Type

/** First-class escape hatch: clear scope-local state and re-bootstrap. */
export class MustRefetchFrame extends S.TaggedClass<MustRefetchFrame>()(
  "MustRefetchFrame",
  {
    scope: SyncScope,
    reason: MustRefetchReason,
  },
) {}

export class PingFrame extends S.TaggedClass<PingFrame>()("PingFrame", {}) {}

export const LiveFrame = S.Union([
  DeltaFrame,
  MutationAckFrame,
  MustRefetchFrame,
  PingFrame,
])
export type LiveFrame = typeof LiveFrame.Type

// ---------------------------------------------------------------------------
// Error taxonomy
// ---------------------------------------------------------------------------

export const SyncErrorCode = S.Literals([
  "unauthenticated",
  "unauthorized_scope",
  "unknown_scope",
  "unknown_mutator",
  "mutation_rejected",
  "invalid_request",
  "protocol_version_unsupported",
  "schema_version_unsupported",
  "cursor_behind_retained_window",
  "storage_unavailable",
  /**
   * Anonymous-read abuse guard (KS-7.1 anonymous `scope.public.*` exception,
   * docs/khala-sync/RUNBOOK.md "Anonymous read scopes"): the best-effort
   * per-IP window limiter on log/bootstrap/connect rejected an
   * UNAUTHENTICATED request. Authenticated requests never see this code —
   * they carry no rate limit at this seam. Always `retryable: true`
   * (HTTP 429); back off and retry.
   */
  "rate_limited",
  "internal",
])
export type SyncErrorCode = typeof SyncErrorCode.Type

export class SyncError extends S.TaggedClass<SyncError>()("SyncError", {
  code: SyncErrorCode,
  messageSafe: S.String,
  retryable: S.Boolean,
}) {}

// ---------------------------------------------------------------------------
// Canonical JSON (postImageJson / argsJson producer)
// ---------------------------------------------------------------------------

/**
 * Typed error thrown by {@link canonicalJson} when a value cannot be
 * represented in canonical JSON (non-finite number, unsupported type, or an
 * `undefined` array element). `path` locates the offending value.
 */
export class CanonicalJsonError extends Error {
  readonly _tag = "CanonicalJsonError"
  override readonly name = "CanonicalJsonError"
  readonly path: ReadonlyArray<string | number>
  constructor(message: string, path: ReadonlyArray<string | number>) {
    super(
      path.length === 0 ? message : `${message} (at ${path.map(String).join(".")})`,
    )
    this.path = path
  }
}

/**
 * Serialize a value as **canonical JSON**. All `postImageJson` (and mutator
 * `argsJson`) strings MUST be produced through this function — on the server
 * and on every client — so that byte-wise comparison, hashing, and diffing of
 * post-images is stable across implementations.
 *
 * Rules (a strict subset of RFC 8785 / JCS):
 *
 * - Object keys are sorted recursively (lexicographic by UTF-16 code unit,
 *   i.e. plain `Array.prototype.sort` on the key strings).
 * - Object members whose value is `undefined` are dropped (matching
 *   `JSON.stringify` semantics); `undefined` array elements are rejected.
 * - Numbers must be finite; `NaN` / `Infinity` throw {@link CanonicalJsonError}.
 *   `-0` normalizes to `0`. Number/string tokens use `JSON.stringify`
 *   (shortest ES round-trip form, matching RFC 8785).
 * - Allowed values: `null`, booleans, finite numbers, strings, arrays, and
 *   objects of those. Anything else (function, symbol, bigint) throws
 *   {@link CanonicalJsonError}.
 * - No whitespace is emitted.
 */
export const canonicalJson = (value: unknown): string => {
  const go = (v: unknown, path: ReadonlyArray<string | number>): string => {
    if (v === null) return "null"
    switch (typeof v) {
      case "boolean":
        return v ? "true" : "false"
      case "number":
        if (!Number.isFinite(v)) {
          throw new CanonicalJsonError(`non-finite number: ${String(v)}`, path)
        }
        return JSON.stringify(v)
      case "string":
        return JSON.stringify(v)
      case "object": {
        if (Array.isArray(v)) {
          const items = v.map((item, index) => {
            if (item === undefined) {
              throw new CanonicalJsonError("undefined array element", [
                ...path,
                index,
              ])
            }
            return go(item, [...path, index])
          })
          return `[${items.join(",")}]`
        }
        const record = v as Record<string, unknown>
        const members: Array<string> = []
        for (const key of Object.keys(record).sort()) {
          const member = record[key]
          if (member === undefined) continue
          members.push(`${JSON.stringify(key)}:${go(member, [...path, key])}`)
        }
        return `{${members.join(",")}}`
      }
      default:
        throw new CanonicalJsonError(`unsupported value of type ${typeof v}`, path)
    }
  }
  return go(value, [])
}

// ---------------------------------------------------------------------------
// Boundary codecs (throwing; pair with decodeUnknownExit at fallible edges)
// ---------------------------------------------------------------------------

export const decodePushRequest = S.decodeUnknownSync(PushRequest)
export const encodePushRequest = S.encodeSync(PushRequest)
export const decodePushResponse = S.decodeUnknownSync(PushResponse)
export const encodePushResponse = S.encodeSync(PushResponse)
export const encodeMutationResult = S.encodeSync(MutationResult)
export const decodeSyncError = S.decodeUnknownSync(SyncError)
export const encodeSyncError = S.encodeSync(SyncError)
export const decodeBootstrapRequest = S.decodeUnknownSync(BootstrapRequest)
export const encodeBootstrapRequest = S.encodeSync(BootstrapRequest)
export const decodeBootstrapResponse = S.decodeUnknownSync(BootstrapResponse)
export const decodeCvrPullRequest = S.decodeUnknownSync(CvrPullRequest)
export const encodeCvrPullRequest = S.encodeSync(CvrPullRequest)
export const decodeCvrPullResponse = S.decodeUnknownSync(CvrPullResponse)
export const encodeCvrPullResponse = S.encodeSync(CvrPullResponse)
export const decodeLogPage = S.decodeUnknownSync(LogPage)
export const decodeLiveFrame = S.decodeUnknownSync(LiveFrame)
export const encodeLiveFrame = S.encodeSync(LiveFrame)
export const encodeChangelogEntry = S.encodeSync(ChangelogEntry)
export const decodeChangelogEntry = S.decodeUnknownSync(ChangelogEntry)
