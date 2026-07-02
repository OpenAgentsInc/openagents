import { Effect, Schema as S } from 'effect'

import {
  ArtanisApprovalGateRecord,
  ArtanisApprovalGateLedgerRecord,
  artanisApprovalGateEffective,
  artanisApprovalGateProjectionHasPrivateMaterial,
  projectArtanisApprovalGateLedger,
} from './artanis-approval-gates'
import { ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE } from './artanis-authority-scope'
import {
  artanisHealthProjectionHasPrivateMaterial,
  ArtanisHealthSignalRecord,
  ArtanisHealthSnapshotRecord,
  exampleArtanisHealthSnapshot,
  projectArtanisHealthSnapshot,
} from './artanis-health'
import { artanisMindComplete } from './artanis-mind'
import { parseJsonWithSchema } from './json-boundary'
import { epochMillisToIsoTimestamp, randomUuid } from './runtime-primitives'

type FleetHeartbeatRow = Readonly<{
  observed_at: string | null
  replica_id: string | null
  heartbeat_run_ref: string | null
  warm_state: string | null
  watchdog_status: string | null
}>

const FleetOverseerAction = S.Union([
  S.Struct({
    kind: S.Literal('start_stress_load'),
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('scale_stress_load'),
    targetConcurrency: S.Number,
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('back_off_stress_load'),
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('readmit_recovered_replica'),
    replicaRef: S.String,
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('emit_health_report'),
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('request_paid_scale_out'),
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('request_replica_quarantine'),
    replicaRef: S.String,
    rationale: S.String,
  }),
  S.Struct({
    kind: S.Literal('no_action'),
    rationale: S.String,
  }),
])

type FleetOverseerAction = typeof FleetOverseerAction.Type

export type ArtanisFleetOverseerContext = Readonly<{
  externalDemandTokens10m: number
  heartbeatRunRefs: ReadonlyArray<string>
  readyReplicaCount: number
  reclaimedReplicaRefs: ReadonlyArray<string>
  totalReplicaCount: number
  warmOrReadyMaxInflight: number
}>

export type ArtanisFleetOverseerDecisionState =
  | 'reported'
  | 'autonomous_intent_recorded'
  | 'approval_requested'
  | 'no_action'
  | 'blocked'
  | 'skipped'

export type ArtanisFleetOverseerOutcome = Readonly<{
  approvalGateRef: string | null
  decisionId: string | null
  healthSnapshotRef: string | null
  reason: string | null
  state: ArtanisFleetOverseerDecisionState
}>

class ArtanisFleetOverseerPersistenceUnsafe extends S.TaggedErrorClass<ArtanisFleetOverseerPersistenceUnsafe>()(
  'ArtanisFleetOverseerPersistenceUnsafe',
  {
    reason: S.String,
  },
) {}

type MindComplete = (input: {
  prompt: string
  system: string
}) => Promise<{ text: string } | { error: string }>

const isoSlug = (iso: string): string =>
  iso.replace(/[^0-9a-z]/giu, '').toLowerCase()

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

const insertPersistencePayload = async (
  db: D1Database,
  input: Readonly<{
    active: boolean
    agentId: string
    createdAtIso: string
    idempotencyKey: string
    parentRef: string | null
    publicProjection: unknown
    record: unknown
    recordRef: string
    scopeRef: string | null
    sourceKind: 'approval_gate' | 'health_snapshot'
    state: string
    tableName: 'artanis_approval_gates' | 'artanis_health_snapshots'
    updatedAtIso: string
  }>,
): Promise<void> => {
  const recordJson = stableJson(input.record)
  const projectionJson = stableJson(input.publicProjection)
  const contentHash = stableJson({
    projection: input.publicProjection,
    record: input.record,
  })

  await db
    .prepare(
      `INSERT INTO ${input.tableName} (
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
      `${input.sourceKind}:${input.recordRef}`,
      input.agentId,
      input.recordRef,
      input.idempotencyKey,
      input.state,
      input.active ? 1 : 0,
      input.sourceKind,
      input.scopeRef,
      input.parentRef,
      recordJson,
      projectionJson,
      contentHash,
      null,
      input.createdAtIso,
      input.updatedAtIso,
      null,
    )
    .run()
}

const saveHealthSnapshot = async (
  db: D1Database,
  record: ArtanisHealthSnapshotRecord,
  nowIso: string,
): Promise<void> => {
  const projection = projectArtanisHealthSnapshot(
    record,
    'public_artanis',
    nowIso,
  )
  if (artanisHealthProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisFleetOverseerPersistenceUnsafe({
      reason: 'Artanis health public projection contains private material.',
    })
  }

  await insertPersistencePayload(db, {
    active: record.overallState === 'healthy',
    agentId: record.agentId,
    createdAtIso: record.createdAtIso,
    idempotencyKey: `artanis-health:${record.snapshotRef}`,
    parentRef: record.loopRef,
    publicProjection: projection,
    record,
    recordRef: record.snapshotRef,
    scopeRef: record.loopRef,
    sourceKind: 'health_snapshot',
    state: record.overallState,
    tableName: 'artanis_health_snapshots',
    updatedAtIso: record.updatedAtIso,
  })
}

const saveApprovalGate = async (
  db: D1Database,
  record: ArtanisApprovalGateRecord,
  nowIso: string,
): Promise<void> => {
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
  if (artanisApprovalGateProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisFleetOverseerPersistenceUnsafe({
      reason:
        'Artanis approval gate public projection contains private material.',
    })
  }

  await insertPersistencePayload(db, {
    active: artanisApprovalGateEffective(record, nowIso),
    agentId: 'agent_artanis',
    createdAtIso: record.createdAtIso,
    idempotencyKey: record.idempotencyKey,
    parentRef: record.actionRef,
    publicProjection: projection,
    record,
    recordRef: record.gateRef,
    scopeRef: record.kind,
    sourceKind: 'approval_gate',
    state: record.state,
    tableName: 'artanis_approval_gates',
    updatedAtIso: record.updatedAtIso,
  })
}

const actionSummary = (
  action: FleetOverseerAction | Readonly<{ kind: string; reason: string }>,
): Record<string, unknown> => ({
  ...action,
  executionAllowed: false,
  authorityRefs: [
    'authority.public.artanis.fleet_overseer.decision_is_not_execution',
    'authority.public.artanis.approval_gate_required_for_fleet_mutation',
  ],
})

const insertDecision = async (
  db: D1Database,
  input: Readonly<{
    action: Record<string, unknown>
    approvalGateRef: string | null
    context: ArtanisFleetOverseerContext
    decisionId: string
    healthSnapshotRef: string | null
    nowIso: string
    state: ArtanisFleetOverseerDecisionState
  }>,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO artanis_fleet_overseer_decisions
       (id, state, action_json, context_json, approval_gate_ref, health_snapshot_ref, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      input.decisionId,
      input.state,
      JSON.stringify(input.action),
      JSON.stringify(input.context),
      input.approvalGateRef,
      input.healthSnapshotRef,
      input.nowIso,
    )
    .run()
}

const readExternalDemandTokens10m = async (
  db: D1Database,
  nowIso: string,
): Promise<number> => {
  const sinceIso = epochMillisToIsoTimestamp(Date.parse(nowIso) - 10 * 60_000)
  const row = await db
    .prepare(
      `SELECT COALESCE(SUM(total_tokens), 0) AS total
         FROM token_usage_events
        WHERE observed_at >= ?
          AND demand_kind = 'external'`,
    )
    .bind(sinceIso)
    .first<{ total: number | string | null }>()

  return Number(row?.total ?? 0)
}

const readLatestFleetHeartbeatRows = async (
  db: D1Database,
): Promise<ReadonlyArray<FleetHeartbeatRow>> => {
  const response = await db
    .prepare(
      `
        SELECT
          observed_at,
          json_extract(safe_metadata_json, '$.selectedReplicaId') AS replica_id,
          json_extract(safe_metadata_json, '$.heartbeatRunRef') AS heartbeat_run_ref,
          json_extract(safe_metadata_json, '$.replicaWarmState') AS warm_state,
          json_extract(safe_metadata_json, '$.watchdogStatus') AS watchdog_status
        FROM token_usage_events
        WHERE model = 'openagents/glm-5.2-reap-504b'
          AND demand_source = 'glm-pool-heartbeat'
          AND json_extract(safe_metadata_json, '$.heartbeatKind') = 'glm_pool_heartbeat'
        ORDER BY observed_at DESC
        LIMIT 100
      `,
    )
    .all<FleetHeartbeatRow>()

  const latestByReplica = (response.results ?? []).reduce(
    (accumulator, row) => {
      if (
        row.replica_id !== null &&
        !accumulator.has(row.replica_id)
      ) {
        accumulator.set(row.replica_id, row)
      }

      return accumulator
    },
    new Map<string, FleetHeartbeatRow>(),
  )
  return [...latestByReplica.values()]
}

const assembleContext = async (
  db: D1Database,
  nowIso: string,
): Promise<ArtanisFleetOverseerContext> => {
  const records = await readLatestFleetHeartbeatRows(db)
  const ready = records.filter(
    record =>
      record.watchdog_status === 'healthy' &&
      (record.warm_state === 'warm' || record.warm_state === 'unknown'),
  )
  const reclaimed = records.filter(
    record => record.watchdog_status === 'unhealthy',
  )

  return {
    externalDemandTokens10m: await readExternalDemandTokens10m(db, nowIso),
    heartbeatRunRefs: [
      ...new Set(records.flatMap(record =>
        record.heartbeat_run_ref === null ? [] : [record.heartbeat_run_ref],
      )),
    ].sort(),
    readyReplicaCount: ready.length,
    reclaimedReplicaRefs: reclaimed.map(
      record => `replica.hydralisk.glm_52_reap_504b.${record.replica_id}`,
    ),
    totalReplicaCount: records.length,
    warmOrReadyMaxInflight: ready.length,
  }
}

const fleetSignal = (
  input: Readonly<{
    context: ArtanisFleetOverseerContext
    decisionState: ArtanisFleetOverseerDecisionState
    nowIso: string
  }>,
): ArtanisHealthSignalRecord =>
  new ArtanisHealthSignalRecord({
    blockerRefs:
      input.decisionState === 'blocked'
        ? ['blocker.public.artanis.fleet_overseer_decision_blocked']
        : ['blocker.public.artanis.fleet_overseer_live_proof_missing'],
    caveatRefs: [
      'authority.public.artanis.fleet_overseer.read_only_signal',
      'caveat.public.artanis.fleet_overseer_default_off',
    ],
    count: input.context.readyReplicaCount,
    kind: 'fleet_overseer',
    label:
      input.decisionState === 'blocked'
        ? 'Fleet overseer decision blocked'
        : 'Fleet overseer recorded read-only decision',
    observedAtIso: input.nowIso,
    operatorDetailRefs: ['health.operator.artanis.fleet_overseer'],
    publicRecoveryActionRefs: [
      'recovery.public.artanis.complete_fleet_overseer_live_proof',
    ],
    publicStatusRefs: ['health.public.artanis.fleet_overseer_blocked'],
    signalRef: 'health.public.artanis.fleet_overseer',
    sourceRefs: [
      'tick.public.artanis.fleet_overseer',
      ...input.context.heartbeatRunRefs,
    ],
    state: 'blocked',
    subjectUpdatedAtIso: input.nowIso,
  })

const healthSnapshot = (
  context: ArtanisFleetOverseerContext,
  decisionState: ArtanisFleetOverseerDecisionState,
  nowIso: string,
): ArtanisHealthSnapshotRecord =>
  new ArtanisHealthSnapshotRecord({
    ...exampleArtanisHealthSnapshot,
    blockerRefs: [
      'blocker.public.artanis.fleet_overseer_live_proof_missing',
    ],
    caveatRefs: ['caveat.public.artanis.health_blocks_overclaiming'],
    createdAtIso: nowIso,
    latestTickRef: 'tick.public.artanis.fleet_overseer',
    operatorRecoveryActionRefs: [
      'recovery.operator.artanis.complete_fleet_overseer_live_proof',
    ],
    overallState: 'blocked',
    overclaimBlocked: true,
    overclaimBlockerRefs: ['overclaim.public.artanis.fleet_overseer_blocked'],
    publicStatusRefs: ['health.public.artanis.status.blocked'],
    signals: exampleArtanisHealthSnapshot.signals.map(signal =>
      signal.kind === 'fleet_overseer'
        ? fleetSignal({ context, decisionState, nowIso })
        : signal,
    ),
    snapshotRef: `health.public.artanis.snapshot.fleet_overseer.${isoSlug(nowIso)}`,
    sourceRefs: [
      'tick.public.artanis.fleet_overseer',
      ...context.heartbeatRunRefs,
    ],
    updatedAtIso: nowIso,
  })

const approvalGateFor = (
  action: Extract<
    FleetOverseerAction,
    { kind: 'request_paid_scale_out' | 'request_replica_quarantine' }
  >,
  nowIso: string,
): ArtanisApprovalGateRecord => {
  const suffix = `${action.kind}.${isoSlug(nowIso)}`
  const kind =
    action.kind === 'request_replica_quarantine'
      ? 'fleet_mutation'
      : 'provider_call'

  return new ArtanisApprovalGateRecord({
    actionRef: `action.public.artanis.fleet_overseer.${suffix}`,
    authorityScope: ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
    authorityReceiptRefs: [],
    authoritySourceKinds: ['operator_policy'],
    caveatRefs: [
      'caveat.public.artanis.fleet_overseer_decision_not_execution_authority',
    ],
    createdAtIso: nowIso,
    expiresAtIso: epochMillisToIsoTimestamp(Date.parse(nowIso) + 60 * 60_000),
    gateRef: `gate.public.artanis.fleet_overseer.${suffix}`,
    idempotencyKey: `artanis-fleet-overseer:${suffix}`,
    kind,
    operatorReceiptRefs: [
      'receipt.operator.artanis.fleet_overseer_review_required',
    ],
    policyRefs: [
      'policy.public.artanis.fleet_overseer_requires_operator_approval',
    ],
    privateEvidenceRefs: [],
    publicStatusRefs: [
      'approval.public.artanis.fleet_overseer.pending',
    ],
    resolvedAtIso: null,
    rollbackPosture: 'rollback_plan_recorded',
    rollbackRefs: [
      action.kind === 'request_replica_quarantine'
        ? 'rollback.public.artanis.fleet_overseer.cancel_quarantine'
        : 'rollback.public.artanis.fleet_overseer.cancel_paid_scale_out',
    ],
    sourceRefs: ['tick.public.artanis.fleet_overseer'],
    state: 'pending',
    supersededByGateRef: null,
    updatedAtIso: nowIso,
  })
}

const actionState = (
  action: FleetOverseerAction,
): ArtanisFleetOverseerDecisionState => {
  if (action.kind === 'no_action') {
    return 'no_action'
  }
  if (action.kind === 'emit_health_report') {
    return 'reported'
  }
  if (
    action.kind === 'request_paid_scale_out' ||
    action.kind === 'request_replica_quarantine'
  ) {
    return 'approval_requested'
  }
  return 'autonomous_intent_recorded'
}

export const runArtanisFleetOverseerTick = async (
  db: D1Database,
  deps: Readonly<{
    assembleContext?: (() => Promise<ArtanisFleetOverseerContext>) | undefined
    gatewayToken?: string | undefined
    geminiApiKey: string | null
    mindComplete?: MindComplete | undefined
    nowIso: string
  }>,
): Promise<ArtanisFleetOverseerOutcome> => {
  const context = await (deps.assembleContext ??
    (() => assembleContext(db, deps.nowIso)))()
  const decisionId = randomUuid()
  const skipped = async (reason: string): Promise<ArtanisFleetOverseerOutcome> => {
    await insertDecision(db, {
      action: actionSummary({ kind: 'skipped', reason }),
      approvalGateRef: null,
      context,
      decisionId,
      healthSnapshotRef: null,
      nowIso: deps.nowIso,
      state: 'skipped',
    })
    return {
      approvalGateRef: null,
      decisionId,
      healthSnapshotRef: null,
      reason,
      state: 'skipped',
    }
  }

  if (deps.geminiApiKey === null || deps.geminiApiKey === '') {
    return skipped('mind_unconfigured')
  }

  const mindComplete = deps.mindComplete ?? (input =>
    artanisMindComplete({
      apiKey: deps.geminiApiKey ?? '',
      ...(deps.gatewayToken === undefined || deps.gatewayToken === ''
        ? {}
        : { gatewayToken: deps.gatewayToken }),
      prompt: input.prompt,
      system: input.system,
    }))

  const mindResult = await mindComplete({
    prompt: [
      'Fleet overseer context for this tick:',
      JSON.stringify(context),
      'Choose one action only. Autonomous actions are read-only intent records until a separate approved executor exists. Risky actions only request approval.',
      'Output STRICT JSON matching one of: {"kind":"start_stress_load","rationale":"..."}, {"kind":"scale_stress_load","targetConcurrency":1,"rationale":"..."}, {"kind":"back_off_stress_load","rationale":"..."}, {"kind":"readmit_recovered_replica","replicaRef":"...","rationale":"..."}, {"kind":"emit_health_report","rationale":"..."}, {"kind":"request_paid_scale_out","rationale":"..."}, {"kind":"request_replica_quarantine","replicaRef":"...","rationale":"..."}, {"kind":"no_action","rationale":"..."}',
    ].join('\n'),
    system:
      'You are Artanis, the fleet overseer. Preserve external demand, keep public output safe, and never execute spend, scale-out, quarantine, or fleet mutation directly.',
  })

  if ('error' in mindResult) {
    return skipped('mind_unavailable')
  }

  let action: FleetOverseerAction | null = null
  try {
    const cleaned = mindResult.text
      .replace(/^```(?:json)?\s*/m, '')
      .replace(/```\s*$/m, '')
      .trim()
    action = parseJsonWithSchema(FleetOverseerAction, cleaned)
  } catch {
    action = null
  }

  if (action === null) {
    const snapshot = healthSnapshot(context, 'blocked', deps.nowIso)
    await saveHealthSnapshot(db, snapshot, deps.nowIso)
    await insertDecision(db, {
      action: actionSummary({
        kind: 'blocked',
        reason: 'schema_invalid_mind_output',
      }),
      approvalGateRef: null,
      context,
      decisionId,
      healthSnapshotRef: snapshot.snapshotRef,
      nowIso: deps.nowIso,
      state: 'blocked',
    })
    return {
      approvalGateRef: null,
      decisionId,
      healthSnapshotRef: snapshot.snapshotRef,
      reason: 'schema_invalid_mind_output',
      state: 'blocked',
    }
  }

  const state = actionState(action)
  const snapshot = healthSnapshot(context, state, deps.nowIso)
  await saveHealthSnapshot(db, snapshot, deps.nowIso)
  const gate =
    action.kind === 'request_paid_scale_out' ||
    action.kind === 'request_replica_quarantine'
      ? approvalGateFor(action, deps.nowIso)
      : null

  if (gate !== null) {
    await saveApprovalGate(db, gate, deps.nowIso)
  }

  await insertDecision(db, {
    action: actionSummary(action),
    approvalGateRef: gate?.gateRef ?? null,
    context,
    decisionId,
    healthSnapshotRef: snapshot.snapshotRef,
    nowIso: deps.nowIso,
    state,
  })

  return {
    approvalGateRef: gate?.gateRef ?? null,
    decisionId,
    healthSnapshotRef: snapshot.snapshotRef,
    reason: action.rationale.slice(0, 200),
    state,
  }
}

export const runArtanisFleetOverseerTickScheduled = (
  db: D1Database,
  deps: Readonly<{
    enabled: boolean
    gatewayToken?: string | undefined
    geminiApiKey: string | null
    nowIso: string
  }>,
): Effect.Effect<ArtanisFleetOverseerOutcome, never> =>
  deps.enabled
    ? Effect.tryPromise({
        catch: () => 'fleet_overseer_tick_error' as const,
        try: () => runArtanisFleetOverseerTick(db, deps),
      }).pipe(
        Effect.catch(reason =>
          Effect.succeed({
            approvalGateRef: null,
            decisionId: null,
            healthSnapshotRef: null,
            reason,
            state: 'skipped',
          } satisfies ArtanisFleetOverseerOutcome),
        ),
      )
    : Effect.succeed({
        approvalGateRef: null,
        decisionId: null,
        healthSnapshotRef: null,
        reason: 'fleet_overseer_disabled',
        state: 'skipped',
      })
