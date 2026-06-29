import { Effect, Schema as S } from 'effect'

import {
  ARTANIS_LOOP_READ_ONLY_AUTHORITY,
  ArtanisLoopLedgerRecord,
  ArtanisLoopTickRecord,
  type ArtanisLoopRecord,
  artanisLoopProjectionHasPrivateMaterial,
  projectArtanisLoopLedger,
} from './artanis-loop'
import {
  type ArtanisRuntimeRecord,
  artanisRuntimeProjectionHasPrivateMaterial,
  projectArtanisRuntime,
} from './artanis-runtime'
import {
  ArtanisApprovalGateLedgerRecord,
  type ArtanisApprovalGateRecord,
  artanisApprovalGateProjectionHasPrivateMaterial,
  artanisApprovalGateEffective,
  projectArtanisApprovalGateLedger,
} from './artanis-approval-gates'
import {
  type ArtanisHealthSnapshotRecord,
  artanisHealthProjectionHasPrivateMaterial,
  projectArtanisHealthSnapshot,
} from './artanis-health'
import {
  type ArtanisWorkRoutingProposalRecord,
  ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
  ArtanisWorkRoutingLedgerRecord,
  artanisWorkRoutingProjectionHasPrivateMaterial,
  projectArtanisWorkRoutingLedger,
} from './artanis-work-routing'
import {
  ArtanisForumPublicationIntentRecord,
  ArtanisForumPublicationQueueRecord,
  artanisForumPublicationProjectionHasPrivateMaterial,
  projectArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  ARTANIS_NEXUS_PYLON_ADMIN_NO_LIVE_AUTHORITY,
  ArtanisNexusPylonAdminAdapterRecord,
  ArtanisNexusPylonFleetSnapshotRecord,
  type ArtanisNexusPylonDispatchRecord,
  artanisNexusPylonProjectionHasPrivateMaterial,
  projectArtanisNexusPylonAdminAdapter,
} from './artanis-nexus-pylon-adapters'
import { decodeUnknownWithSchema, parseJsonUnknown } from './json-boundary'

export const ArtanisPersistenceRecordKind = S.Literals([
  'approval_gate',
  'forum_publication_intent',
  'health_snapshot',
  'loop_record',
  'loop_tick',
  'nexus_pylon_adapter_dispatch',
  'runtime_snapshot',
  'work_routing_proposal',
])
export type ArtanisPersistenceRecordKind =
  typeof ArtanisPersistenceRecordKind.Type

export const ArtanisPersistenceWriteState = S.Literals([
  'closed',
  'conflict',
  'inserted',
  'retried',
])
export type ArtanisPersistenceWriteState =
  typeof ArtanisPersistenceWriteState.Type

export class ArtanisPersistenceStoredRow extends S.Class<ArtanisPersistenceStoredRow>(
  'ArtanisPersistenceStoredRow',
)({
  agentId: S.String,
  closedAtIso: S.NullOr(S.String),
  closeoutJson: S.NullOr(S.String),
  contentHash: S.String,
  createdAtIso: S.String,
  executableAuthority: S.Boolean,
  idempotencyKey: S.String,
  kind: ArtanisPersistenceRecordKind,
  parentRef: S.NullOr(S.String),
  publicProjection: S.Unknown,
  record: S.Unknown,
  recordRef: S.String,
  scopeRef: S.NullOr(S.String),
  state: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisPersistenceWriteReceipt extends S.Class<ArtanisPersistenceWriteReceipt>(
  'ArtanisPersistenceWriteReceipt',
)({
  closedAtIso: S.NullOr(S.String),
  executableAuthority: S.Boolean,
  idempotent: S.Boolean,
  kind: ArtanisPersistenceRecordKind,
  publicProjection: S.Unknown,
  recordRef: S.String,
  state: ArtanisPersistenceWriteState,
}) {}

export class ArtanisPersistenceError extends S.TaggedErrorClass<ArtanisPersistenceError>()(
  'ArtanisPersistenceError',
  {
    kind: S.Literals(['conflict', 'not_found', 'storage_error', 'unsafe_record']),
    reason: S.String,
  },
) {}

type PersistableArtanisRecord =
  | ArtanisApprovalGateRecord
  | ArtanisForumPublicationIntentRecord
  | ArtanisHealthSnapshotRecord
  | ArtanisLoopRecord
  | ArtanisLoopTickRecord
  | ArtanisNexusPylonDispatchRecord
  | ArtanisRuntimeRecord
  | ArtanisWorkRoutingProposalRecord

type PersistenceTableSpec = Readonly<{
  kind: ArtanisPersistenceRecordKind
  tableName: string
}>

type PersistencePayload = Readonly<{
  active: boolean
  agentId: string
  createdAtIso: string
  idempotencyKey: string
  parentRef: string | null
  publicProjection: unknown
  record: PersistableArtanisRecord
  recordRef: string
  scopeRef: string | null
  state: string
  updatedAtIso: string
}>

type PersistenceRow = Readonly<{
  agent_id: string
  closed_at: string | null
  closeout_json: string | null
  content_hash: string
  created_at: string
  idempotency_key: string
  parent_ref: string | null
  public_projection_json: string
  record_json: string
  record_ref: string
  scope_ref: string | null
  state: string
  updated_at: string
}>

const tableSpecs = {
  approval_gate: {
    kind: 'approval_gate',
    tableName: 'artanis_approval_gates',
  },
  forum_publication_intent: {
    kind: 'forum_publication_intent',
    tableName: 'artanis_forum_publication_intents',
  },
  health_snapshot: {
    kind: 'health_snapshot',
    tableName: 'artanis_health_snapshots',
  },
  loop_record: {
    kind: 'loop_record',
    tableName: 'artanis_loop_records',
  },
  loop_tick: {
    kind: 'loop_tick',
    tableName: 'artanis_loop_ticks',
  },
  nexus_pylon_adapter_dispatch: {
    kind: 'nexus_pylon_adapter_dispatch',
    tableName: 'artanis_nexus_pylon_adapter_dispatches',
  },
  runtime_snapshot: {
    kind: 'runtime_snapshot',
    tableName: 'artanis_runtime_snapshots',
  },
  work_routing_proposal: {
    kind: 'work_routing_proposal',
    tableName: 'artanis_work_routing_proposals',
  },
} satisfies Record<ArtanisPersistenceRecordKind, PersistenceTableSpec>

const stableValue = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(stableValue)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, item]) => [key, stableValue(item)]),
    )
  }

  return value
}

const stableJson = (value: unknown): string => JSON.stringify(stableValue(value))

const storageEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ArtanisPersistenceError> =>
  Effect.tryPromise({
    catch: error =>
      new ArtanisPersistenceError({
        kind: 'storage_error',
        reason:
          error instanceof Error
            ? `${operation}: ${error.message}`
            : `${operation}: ${String(error)}`,
      }),
    try: run,
  })

const rowToStored = (
  spec: PersistenceTableSpec,
  row: PersistenceRow,
): ArtanisPersistenceStoredRow =>
  new ArtanisPersistenceStoredRow({
    agentId: row.agent_id,
    closedAtIso: row.closed_at,
    closeoutJson: row.closeout_json,
    contentHash: row.content_hash,
    createdAtIso: row.created_at,
    executableAuthority: false,
    idempotencyKey: row.idempotency_key,
    kind: spec.kind,
    parentRef: row.parent_ref,
    publicProjection: parseJsonUnknown(row.public_projection_json),
    record: parseJsonUnknown(row.record_json),
    recordRef: row.record_ref,
    scopeRef: row.scope_ref,
    state: row.state,
    updatedAtIso: row.updated_at,
  })

const readByIdempotencyKey = (
  db: D1Database,
  spec: PersistenceTableSpec,
  idempotencyKey: string,
): Effect.Effect<ArtanisPersistenceStoredRow | null, ArtanisPersistenceError> =>
  storageEffect(
    `read ${spec.kind} by idempotency key`,
    () =>
      db
        .prepare(
          `SELECT agent_id,
                  closed_at,
                  closeout_json,
                  content_hash,
                  created_at,
                  idempotency_key,
                  parent_ref,
                  public_projection_json,
                  record_json,
                  record_ref,
                  scope_ref,
                  state,
                  updated_at
             FROM ${spec.tableName}
            WHERE idempotency_key = ?`,
        )
        .bind(idempotencyKey)
        .first<PersistenceRow>(),
  ).pipe(
    Effect.map(row => row === null ? null : rowToStored(spec, row)),
  )

const readByRecordRef = (
  db: D1Database,
  spec: PersistenceTableSpec,
  recordRef: string,
): Effect.Effect<ArtanisPersistenceStoredRow | null, ArtanisPersistenceError> =>
  storageEffect(
    `read ${spec.kind} by record ref`,
    () =>
      db
        .prepare(
          `SELECT agent_id,
                  closed_at,
                  closeout_json,
                  content_hash,
                  created_at,
                  idempotency_key,
                  parent_ref,
                  public_projection_json,
                  record_json,
                  record_ref,
                  scope_ref,
                  state,
                  updated_at
             FROM ${spec.tableName}
            WHERE record_ref = ?`,
        )
        .bind(recordRef)
        .first<PersistenceRow>(),
  ).pipe(
    Effect.map(row => row === null ? null : rowToStored(spec, row)),
  )

const insertPayload = (
  db: D1Database,
  spec: PersistenceTableSpec,
  payload: PersistencePayload,
  recordJson: string,
  projectionJson: string,
  contentHash: string,
): Effect.Effect<void, ArtanisPersistenceError> =>
  storageEffect(
    `insert ${spec.kind}`,
    () =>
      db
        .prepare(
          `INSERT INTO ${spec.tableName} (
             id,
             agent_id,
             record_ref,
             idempotency_key,
             state,
             active,
             source_kind,
             scope_ref,
             parent_ref,
             record_json,
             public_projection_json,
             content_hash,
             closeout_json,
             created_at,
             updated_at,
             closed_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          `${spec.kind}:${payload.recordRef}`,
          payload.agentId,
          payload.recordRef,
          payload.idempotencyKey,
          payload.state,
          payload.active ? 1 : 0,
          spec.kind,
          payload.scopeRef,
          payload.parentRef,
          recordJson,
          projectionJson,
          contentHash,
          null,
          payload.createdAtIso,
          payload.updatedAtIso,
          null,
        )
        .run(),
  ).pipe(Effect.asVoid)

const writeReceipt = (
  kind: ArtanisPersistenceRecordKind,
  recordRef: string,
  publicProjection: unknown,
  input: Readonly<{
    closedAtIso: string | null
    idempotent: boolean
    state: ArtanisPersistenceWriteState
  }>,
): ArtanisPersistenceWriteReceipt =>
  new ArtanisPersistenceWriteReceipt({
    closedAtIso: input.closedAtIso,
    executableAuthority: false,
    idempotent: input.idempotent,
    kind,
    publicProjection,
    recordRef,
    state: input.state,
  })

const persistPayload = (
  db: D1Database,
  spec: PersistenceTableSpec,
  payload: PersistencePayload,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const recordJson = stableJson(payload.record)
    const projectionJson = stableJson(payload.publicProjection)
    const contentHash = stableJson({
      projection: payload.publicProjection,
      record: payload.record,
    })
    const existingByKey = yield* readByIdempotencyKey(
      db,
      spec,
      payload.idempotencyKey,
    )

    if (existingByKey !== null) {
      if (existingByKey.contentHash !== contentHash) {
        return yield* new ArtanisPersistenceError({
          kind: 'conflict',
          reason:
            'Artanis persistence idempotency key was reused with different record content.',
        })
      }

      return writeReceipt(spec.kind, existingByKey.recordRef, existingByKey.publicProjection, {
        closedAtIso: existingByKey.closedAtIso,
        idempotent: true,
        state: 'retried',
      })
    }

    const existingByRef = yield* readByRecordRef(db, spec, payload.recordRef)

    if (existingByRef !== null) {
      if (existingByRef.contentHash !== contentHash) {
        return yield* new ArtanisPersistenceError({
          kind: 'conflict',
          reason:
            'Artanis persistence record ref already exists with different record content.',
        })
      }

      return writeReceipt(spec.kind, existingByRef.recordRef, existingByRef.publicProjection, {
        closedAtIso: existingByRef.closedAtIso,
        idempotent: true,
        state: 'retried',
      })
    }

    yield* insertPayload(
      db,
      spec,
      payload,
      recordJson,
      projectionJson,
      contentHash,
    )

    return writeReceipt(spec.kind, payload.recordRef, payload.publicProjection, {
      closedAtIso: null,
      idempotent: false,
      state: 'inserted',
    })
  })

const ensurePublicSafeProjection = (
  unsafe: boolean,
  label: string,
): Effect.Effect<void, ArtanisPersistenceError> =>
  unsafe
    ? Effect.fail(
        new ArtanisPersistenceError({
          kind: 'unsafe_record',
          reason: `${label} public projection contains private material.`,
        }),
      )
    : Effect.void

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

// Dedupe while preserving first-seen order: `base` order is kept intact and
// only refs not already present are appended. Used where a stored array's
// order must stay stable across idempotent re-writes.
const orderedRefUnion = (
  base: ReadonlyArray<string>,
  additions: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const result: Array<string> = []

  for (const ref of [...base, ...additions]) {
    const trimmed = ref.trim()

    if (trimmed === '' || seen.has(trimmed)) {
      continue
    }

    seen.add(trimmed)
    result.push(trimmed)
  }

  return result
}

const forumPublicationProjection = (
  record: ArtanisForumPublicationIntentRecord,
  nowIso: string,
) =>
  projectArtanisForumPublicationQueue(
    new ArtanisForumPublicationQueueRecord({
      agentId: record.authorAgentId,
      caveatRefs: record.caveatRefs,
      createdAtIso: record.createdAtIso,
      intents: [record],
      queueRef: 'queue.public.artanis.persistence.forum_publications',
      redactionPolicyRef: record.redactionPolicyRef,
      updatedAtIso: record.updatedAtIso,
    }),
    nowIso,
  )

const deliveredForumPublicationIntent = (
  record: ArtanisForumPublicationIntentRecord,
  input: Readonly<{
    deliveredAtIso: string
    deliveryReceiptRefs: ReadonlyArray<string>
    postRef: string
    updatedAtIso: string
  }>,
): ArtanisForumPublicationIntentRecord =>
  new ArtanisForumPublicationIntentRecord({
    ...record,
    deliveredAtIso: input.deliveredAtIso,
    deliveryReceiptRefs: uniqueRefs([
      ...record.deliveryReceiptRefs,
      ...input.deliveryReceiptRefs,
    ]),
    deliveryState: 'delivered',
    postRef: input.postRef,
    updatedAtIso: input.updatedAtIso,
  })

const loopTickProjection = (
  record: ArtanisLoopTickRecord,
  nowIso: string,
) =>
  projectArtanisLoopLedger(
    new ArtanisLoopLedgerRecord({
      agentId: 'agent_artanis',
      authority: ARTANIS_LOOP_READ_ONLY_AUTHORITY,
      caveatRefs: record.caveatRefs,
      createdAtIso: record.createdAtIso,
      ledgerRef: 'ledger.public.artanis.persistence.tick',
      loops: [
        {
          active: !['completed', 'failed', 'paused'].includes(record.state),
          agentId: 'agent_artanis',
          blockerRefs: record.blockerRefs,
          caveatRefs: record.caveatRefs,
          createdAtIso: record.createdAtIso,
          goalRefs: [record.goalRef],
          loopRef: record.loopRef,
          scopeRef: 'scope.public.artanis.persistence.tick',
          state: record.state,
          ticks: [record],
          updatedAtIso: record.updatedAtIso,
        },
      ],
      updatedAtIso: record.updatedAtIso,
    }),
    'public',
    nowIso,
  )

export const saveArtanisRuntimeSnapshot = (
  db: D1Database,
  record: ArtanisRuntimeRecord,
  idempotencyKey: string,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = projectArtanisRuntime(record, 'public', nowIso)
    yield* ensurePublicSafeProjection(
      artanisRuntimeProjectionHasPrivateMaterial(projection),
      'Artanis runtime',
    )

    return yield* persistPayload(db, tableSpecs.runtime_snapshot, {
      active: record.state === 'running',
      agentId: record.agentId,
      createdAtIso: record.createdAtIso,
      idempotencyKey,
      parentRef: null,
      publicProjection: projection,
      record,
      recordRef: record.runtimeRef,
      scopeRef: 'scope.public.artanis.runtime',
      state: record.state,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisLoopRecord = (
  db: D1Database,
  record: ArtanisLoopRecord,
  idempotencyKey: string,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = projectArtanisLoopLedger(
      new ArtanisLoopLedgerRecord({
        agentId: record.agentId,
        authority: ARTANIS_LOOP_READ_ONLY_AUTHORITY,
        caveatRefs: record.caveatRefs,
        createdAtIso: record.createdAtIso,
        ledgerRef: 'ledger.public.artanis.persistence.loop',
        loops: [record],
        updatedAtIso: record.updatedAtIso,
      }),
      'public',
      nowIso,
    )
    yield* ensurePublicSafeProjection(
      artanisLoopProjectionHasPrivateMaterial(projection),
      'Artanis loop',
    )

    return yield* persistPayload(db, tableSpecs.loop_record, {
      active: record.active,
      agentId: record.agentId,
      createdAtIso: record.createdAtIso,
      idempotencyKey,
      parentRef: null,
      publicProjection: projection,
      record,
      recordRef: record.loopRef,
      scopeRef: record.scopeRef,
      state: record.state,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisLoopTick = (
  db: D1Database,
  record: ArtanisLoopTickRecord,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = loopTickProjection(record, nowIso)
    yield* ensurePublicSafeProjection(
      artanisLoopProjectionHasPrivateMaterial(projection),
      'Artanis loop tick',
    )

    return yield* persistPayload(db, tableSpecs.loop_tick, {
      active: false,
      agentId: 'agent_artanis',
      createdAtIso: record.createdAtIso,
      idempotencyKey: record.idempotencyKey,
      parentRef: record.loopRef,
      publicProjection: projection,
      record,
      recordRef: record.tickRef,
      scopeRef: null,
      state: record.state,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisApprovalGate = (
  db: D1Database,
  record: ArtanisApprovalGateRecord,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = projectArtanisApprovalGateLedger(
      new ArtanisApprovalGateLedgerRecord({
        agentId: 'agent_artanis',
        caveatRefs: record.caveatRefs,
        createdAtIso: record.createdAtIso,
        gates: [record],
        ledgerRef: 'ledger.public.artanis.persistence.approval_gates',
        updatedAtIso: record.updatedAtIso,
      }),
      'public_artanis',
      nowIso,
    )
    yield* ensurePublicSafeProjection(
      artanisApprovalGateProjectionHasPrivateMaterial(projection),
      'Artanis approval gate',
    )

    return yield* persistPayload(db, tableSpecs.approval_gate, {
      active: artanisApprovalGateEffective(record, nowIso),
      agentId: 'agent_artanis',
      createdAtIso: record.createdAtIso,
      idempotencyKey: record.idempotencyKey,
      parentRef: record.actionRef,
      publicProjection: projection,
      record,
      recordRef: record.gateRef,
      scopeRef: record.kind,
      state: record.state,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisHealthSnapshot = (
  db: D1Database,
  record: ArtanisHealthSnapshotRecord,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = projectArtanisHealthSnapshot(
      record,
      'public_artanis',
      nowIso,
    )
    yield* ensurePublicSafeProjection(
      artanisHealthProjectionHasPrivateMaterial(projection),
      'Artanis health',
    )

    return yield* persistPayload(db, tableSpecs.health_snapshot, {
      active: record.overallState === 'healthy',
      agentId: record.agentId,
      createdAtIso: record.createdAtIso,
      idempotencyKey: `artanis-health:${record.snapshotRef}`,
      parentRef: record.loopRef,
      publicProjection: projection,
      record,
      recordRef: record.snapshotRef,
      scopeRef: record.loopRef,
      state: record.overallState,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisWorkRoutingProposal = (
  db: D1Database,
  record: ArtanisWorkRoutingProposalRecord,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = projectArtanisWorkRoutingLedger(
      new ArtanisWorkRoutingLedgerRecord({
        agentId: 'agent_artanis',
        authority: ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
        caveatRefs: record.publicCaveatRefs,
        createdAtIso: record.createdAtIso,
        ledgerRef: 'ledger.public.artanis.persistence.work_routing',
        proposals: [record],
        publicStatusRefs: ['work_routing.public.artanis.persistence'],
        updatedAtIso: record.updatedAtIso,
      }),
      'public_artanis',
      nowIso,
    )
    yield* ensurePublicSafeProjection(
      artanisWorkRoutingProjectionHasPrivateMaterial(projection),
      'Artanis work routing',
    )

    return yield* persistPayload(db, tableSpecs.work_routing_proposal, {
      active: record.state === 'accepted',
      agentId: 'agent_artanis',
      createdAtIso: record.createdAtIso,
      idempotencyKey: `artanis-work-routing:${record.proposalRef}`,
      parentRef: record.target,
      publicProjection: projection,
      record,
      recordRef: record.proposalRef,
      scopeRef: record.workClass,
      state: record.state,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisForumPublicationIntent = (
  db: D1Database,
  record: ArtanisForumPublicationIntentRecord,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = forumPublicationProjection(record, nowIso)
    yield* ensurePublicSafeProjection(
      artanisForumPublicationProjectionHasPrivateMaterial(projection),
      'Artanis Forum publication',
    )

    return yield* persistPayload(db, tableSpecs.forum_publication_intent, {
      active: record.deliveryState === 'ready',
      agentId: record.authorAgentId,
      createdAtIso: record.createdAtIso,
      idempotencyKey: record.idempotencyKey,
      parentRef: record.targetTopicRef,
      publicProjection: projection,
      record,
      recordRef: record.intentRef,
      scopeRef: record.targetForumRef,
      state: record.deliveryState,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const saveArtanisNexusPylonAdapterDispatch = (
  db: D1Database,
  record: ArtanisNexusPylonDispatchRecord,
  nowIso: string,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const projection = projectArtanisNexusPylonAdminAdapter(
      new ArtanisNexusPylonAdminAdapterRecord({
        agentId: 'agent_artanis',
        authority: ARTANIS_NEXUS_PYLON_ADMIN_NO_LIVE_AUTHORITY,
        caveatRefs: record.caveatRefs,
        createdAtIso: record.createdAtIso,
        dispatchRecords: [record],
        fleetSnapshots: [
          new ArtanisNexusPylonFleetSnapshotRecord({
            blockerRefs: ['blocker.public.fleet_snapshot_not_persisted'],
            caveatRefs: ['caveat.public.fleet_snapshot_not_in_receipt'],
            createdAtIso: record.createdAtIso,
            fleetState: 'unavailable',
            hostedNexusRelayRef: null,
            nexusAcceptedWorkBitcoinPaidRef: null,
            pylonRefs: [],
            pylonsOnlineNow: 0,
            pylonsSeen24h: 0,
            pylonSessionsOnlineNow: 0,
            sellablePylonsOnlineNow: 0,
            snapshotRef:
              'snapshot.public.artanis.nexus_pylon.dispatch_receipt_context',
            sourceRefs: ['nexus.public.stats', 'pylon.public.stats'],
            staleAfterIso: record.updatedAtIso,
            surfaces: ['job_assignments', 'run_status'],
            trainingAcceptedContributors: 0,
            trainingAssignedContributors: 0,
            trainingModelProgressContributors: 0,
            updatedAtIso: record.updatedAtIso,
          }),
        ],
        ledgerRef: 'ledger.public.artanis.persistence.nexus_pylon_adapters',
        sourceRefs: record.sourceRefs,
        updatedAtIso: record.updatedAtIso,
      }),
      'public_artanis',
      nowIso,
    )
    yield* ensurePublicSafeProjection(
      artanisNexusPylonProjectionHasPrivateMaterial(projection),
      'Artanis Nexus/Pylon adapter dispatch',
    )

    return yield* persistPayload(db, tableSpecs.nexus_pylon_adapter_dispatch, {
      active: record.state === 'approved',
      agentId: 'agent_artanis',
      createdAtIso: record.createdAtIso,
      idempotencyKey: record.idempotencyKey,
      parentRef: record.proposalRef,
      publicProjection: projection,
      record,
      recordRef: record.dispatchRef,
      scopeRef: record.jobKind,
      state: record.state,
      updatedAtIso: record.updatedAtIso,
    })
  })

export const markArtanisForumPublicationIntentDelivered = (
  db: D1Database,
  intentRef: string,
  input: Readonly<{
    deliveredAtIso: string
    deliveryReceiptRefs: ReadonlyArray<string>
    postRef: string
    updatedAtIso: string
  }>,
): Effect.Effect<ArtanisPersistenceStoredRow, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const spec = tableSpecs.forum_publication_intent
    const existing = yield* readByRecordRef(db, spec, intentRef)

    if (existing === null) {
      return yield* new ArtanisPersistenceError({
        kind: 'not_found',
        reason: 'Artanis Forum publication intent was not found.',
      })
    }

    const current = decodeUnknownWithSchema(
      ArtanisForumPublicationIntentRecord,
      existing.record,
    )

    if (current.deliveryState === 'delivered') {
      if (current.postRef !== input.postRef) {
        return yield* new ArtanisPersistenceError({
          kind: 'conflict',
          reason:
            'Artanis Forum publication intent was already delivered to a different post.',
        })
      }

      return existing
    }

    if (current.deliveryState !== 'ready') {
      return yield* new ArtanisPersistenceError({
        kind: 'conflict',
        reason:
          'Only ready Artanis Forum publication intents can be marked delivered.',
      })
    }

    const record = deliveredForumPublicationIntent(current, input)
    const projection = forumPublicationProjection(record, input.updatedAtIso)
    yield* ensurePublicSafeProjection(
      artanisForumPublicationProjectionHasPrivateMaterial(projection),
      'Artanis Forum publication delivery',
    )

    yield* storageEffect(
      'mark Artanis Forum publication delivered',
      () =>
        db
          .prepare(
            `UPDATE ${spec.tableName}
                SET state = ?,
                    active = ?,
                    record_json = ?,
                    public_projection_json = ?,
                    content_hash = ?,
                    updated_at = ?
              WHERE record_ref = ?`,
          )
          .bind(
            record.deliveryState,
            0,
            stableJson(record),
            stableJson(projection),
            stableJson({ projection, record }),
            record.updatedAtIso,
            record.intentRef,
          )
          .run(),
    )

    const updated = yield* readByRecordRef(db, spec, intentRef)

    if (updated === null) {
      return yield* new ArtanisPersistenceError({
        kind: 'not_found',
        reason: 'Delivered Artanis Forum publication intent was not readable.',
      })
    }

    return updated
  })

export const readArtanisPersistedRecord = (
  db: D1Database,
  kind: ArtanisPersistenceRecordKind,
  recordRef: string,
): Effect.Effect<ArtanisPersistenceStoredRow | null, ArtanisPersistenceError> =>
  readByRecordRef(db, tableSpecs[kind], recordRef)

export const readLatestArtanisPersistedRows = (
  db: D1Database,
  kind: ArtanisPersistenceRecordKind,
  limit: number,
): Effect.Effect<ReadonlyArray<ArtanisPersistenceStoredRow>, ArtanisPersistenceError> =>
  storageEffect(
    `read latest ${kind}`,
    () =>
      db
        .prepare(
          `SELECT agent_id,
                  closed_at,
                  closeout_json,
                  content_hash,
                  created_at,
                  idempotency_key,
                  parent_ref,
                  public_projection_json,
                  record_json,
                  record_ref,
                  scope_ref,
                  state,
                  updated_at
             FROM ${tableSpecs[kind].tableName}
            ORDER BY updated_at DESC
            LIMIT ?`,
        )
        .bind(Math.max(1, Math.min(50, Math.trunc(limit))))
        .all<PersistenceRow>(),
  ).pipe(
    Effect.map(result =>
      (result.results ?? []).map(row => rowToStored(tableSpecs[kind], row)),
    ),
  )

export const closeArtanisPersistedLoopTick = (
  db: D1Database,
  tickRef: string,
  input: Readonly<{
    closedAtIso: string
    closeoutReceiptRefs: ReadonlyArray<string>
    state: 'completed' | 'failed' | 'blocked'
    updatedAtIso: string
  }>,
): Effect.Effect<ArtanisPersistenceWriteReceipt, ArtanisPersistenceError> =>
  Effect.gen(function* () {
    const spec = tableSpecs.loop_tick
    const existing = yield* readByRecordRef(db, spec, tickRef)

    if (existing === null) {
      return yield* new ArtanisPersistenceError({
        kind: 'not_found',
        reason: 'Artanis loop tick was not found for closeout.',
      })
    }

    const current = yield* Effect.try({
      catch: () =>
        new ArtanisPersistenceError({
          kind: 'unsafe_record',
          reason: 'Stored Artanis loop tick record could not be decoded for closeout.',
        }),
      try: () => decodeUnknownWithSchema(ArtanisLoopTickRecord, existing.record),
    })
    // Preserve the existing closeout-receipt order and only append genuinely
    // new refs. Re-sorting here would rewrite the row into a form that a
    // faithful re-run of saveArtanisLoopTick cannot reproduce, breaking
    // full-tick idempotency (same idempotency key, different content_hash).
    const closeoutReceiptRefs = orderedRefUnion(
      current.closeoutReceiptRefs,
      input.closeoutReceiptRefs,
    )
    const record = new ArtanisLoopTickRecord({
      ...current,
      closeoutReceiptRefs,
      state: input.state,
      updatedAtIso: input.updatedAtIso,
    })
    const projection = loopTickProjection(record, input.updatedAtIso)
    yield* ensurePublicSafeProjection(
      artanisLoopProjectionHasPrivateMaterial(projection),
      'Artanis loop tick closeout',
    )
    const closeoutJson = stableJson({
      closeoutReceiptRefs,
      state: input.state,
    })

    if (existing.closedAtIso !== null) {
      if (existing.closeoutJson !== closeoutJson) {
        return yield* new ArtanisPersistenceError({
          kind: 'conflict',
          reason: 'Artanis loop tick closeout was retried with different content.',
        })
      }

      return writeReceipt(spec.kind, existing.recordRef, existing.publicProjection, {
        closedAtIso: existing.closedAtIso,
        idempotent: true,
        state: 'closed',
      })
    }

    const recordJson = stableJson(record)
    const projectionJson = stableJson(projection)
    const contentHash = stableJson({
      projection,
      record,
    })

    yield* storageEffect(
      'close Artanis loop tick',
      () =>
        db
          .prepare(
            `UPDATE ${spec.tableName}
                SET state = ?,
                    closeout_json = ?,
                    record_json = ?,
                    public_projection_json = ?,
                    content_hash = ?,
                    updated_at = ?,
                    closed_at = ?
              WHERE record_ref = ?
                AND closed_at IS NULL`,
          )
          .bind(
            input.state,
            closeoutJson,
            recordJson,
            projectionJson,
            contentHash,
            input.updatedAtIso,
            input.closedAtIso,
            tickRef,
          )
          .run(),
    )

    return writeReceipt(spec.kind, existing.recordRef, projection, {
      closedAtIso: input.closedAtIso,
      idempotent: false,
      state: 'closed',
    })
  })
