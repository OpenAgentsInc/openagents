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

export class PushResponse extends S.Class<PushResponse>("PushResponse")({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  results: S.Array(MutationResult),
  lastMutationId: MutationId,
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
 * snapshot transaction ran; the client catches up from exactly there.
 */
export class BootstrapResponse extends S.Class<BootstrapResponse>(
  "BootstrapResponse",
)({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  scope: SyncScope,
  entities: S.Array(BootstrapEntity),
  /** Absent while paging; present with the snapshot cursor on the last page. */
  cursor: S.optionalKey(SyncVersion),
  nextPageToken: S.optionalKey(S.String),
}) {}

// ---------------------------------------------------------------------------
// Wire protocol — catch-up log page
// ---------------------------------------------------------------------------

export class LogPage extends S.Class<LogPage>("LogPage")({
  protocolVersion: S.Literal(KHALA_SYNC_PROTOCOL_VERSION),
  scope: SyncScope,
  entries: S.Array(ChangelogEntry),
  nextCursor: SyncVersion,
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
  "unauthorized_scope",
  "unknown_scope",
  "unknown_mutator",
  "mutation_rejected",
  "protocol_version_unsupported",
  "schema_version_unsupported",
  "cursor_behind_retained_window",
  "storage_unavailable",
  "internal",
])
export type SyncErrorCode = typeof SyncErrorCode.Type

export class SyncError extends S.TaggedClass<SyncError>()("SyncError", {
  code: SyncErrorCode,
  messageSafe: S.String,
  retryable: S.Boolean,
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs (throwing; pair with decodeUnknownExit at fallible edges)
// ---------------------------------------------------------------------------

export const decodePushRequest = S.decodeUnknownSync(PushRequest)
export const decodePushResponse = S.decodeUnknownSync(PushResponse)
export const decodeBootstrapRequest = S.decodeUnknownSync(BootstrapRequest)
export const decodeBootstrapResponse = S.decodeUnknownSync(BootstrapResponse)
export const decodeLogPage = S.decodeUnknownSync(LogPage)
export const decodeLiveFrame = S.decodeUnknownSync(LiveFrame)
export const encodeLiveFrame = S.encodeSync(LiveFrame)
export const encodeChangelogEntry = S.encodeSync(ChangelogEntry)
export const decodeChangelogEntry = S.decodeUnknownSync(ChangelogEntry)
