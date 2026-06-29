import { Effect } from 'effect'

import {
  assignmentReadyCapabilityRef,
  onlineHeartbeatStatuses,
  pylonHeartbeatFresh,
} from './autopilot-work-placement-selector'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { safeJsonRecord } from './json-boundary'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  utcStartOfDayIsoTimestamp,
  utcStartOfHourIsoTimestamp,
} from './runtime-primitives'
import {
  type PylonApiAssignmentRecord,
  type PylonApiProviderJobLifecycleRecord,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
  makeD1PylonApiStore,
  pylonClientVersionMeetsMinimum,
} from './pylon-api'
import {
  PYLON_CAPACITY_FUNNEL_ACCOUNTING_READ_ONLY_AUTHORITY,
  type PylonCapacityFunnelAggregate,
  type PylonCapacityFunnelRecord,
  type PylonCapacityFunnelStage,
  aggregatePylonCapacityFunnel,
} from './pylon-capacity-funnel'
import { pylonJoinLifecycleLadderForFunnel } from './pylon-join-lifecycle'
import { PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION } from './public-pylon-stats'

export const PylonDarkCapacityReasonRefs = [
  'dark_capacity.public.never_heartbeated',
  'dark_capacity.public.stale_heartbeat',
  'dark_capacity.public.version_incompatible',
  'dark_capacity.public.capability_missing',
  'dark_capacity.public.wallet_not_ready',
  'dark_capacity.public.assignment_declined',
  'dark_capacity.public.assignment_expired',
  'dark_capacity.public.closeout_missing',
  'dark_capacity.public.no_assignments_offered',
] as const

export type PylonDarkCapacityReasonRef =
  (typeof PylonDarkCapacityReasonRefs)[number]

const registrationListLimit = 500
const assignmentListLimit = 20

const activeAssignmentStates = new Set([
  'accepted',
  'offered',
  'proof_submitted',
  'running',
])

const leaseExpired = (
  assignment: PylonApiAssignmentRecord,
  nowIso: string,
): boolean => {
  const lease = Date.parse(assignment.leaseExpiresAt)
  const now = Date.parse(nowIso)

  return Number.isFinite(lease) && Number.isFinite(now) && lease < now
}

const latestAssignment = (
  assignments: ReadonlyArray<PylonApiAssignmentRecord>,
): PylonApiAssignmentRecord | undefined =>
  assignments.length === 0
    ? undefined
    : assignments.reduce((latest, assignment) =>
        assignment.updatedAt >= latest.updatedAt ? assignment : latest,
      )

export const darkCapacityReasonRefForPylon = (
  input: Readonly<{
    assignments: ReadonlyArray<PylonApiAssignmentRecord>
    nowIso: string
    registration: PylonApiRegistrationRecord
  }>,
): PylonDarkCapacityReasonRef | null => {
  const { assignments, nowIso, registration } = input

  if (registration.latestHeartbeatAt === null) {
    return 'dark_capacity.public.never_heartbeated'
  }

  const heartbeatOnline = onlineHeartbeatStatuses.has(
    (registration.latestHeartbeatStatus ?? '').trim().toLowerCase(),
  )

  if (
    !heartbeatOnline ||
    !pylonHeartbeatFresh(registration.latestHeartbeatAt, nowIso)
  ) {
    return 'dark_capacity.public.stale_heartbeat'
  }

  if (
    !pylonClientVersionMeetsMinimum(
      registration.clientVersion,
      PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
    )
  ) {
    return 'dark_capacity.public.version_incompatible'
  }

  if (!registration.capabilityRefs.includes(assignmentReadyCapabilityRef)) {
    return 'dark_capacity.public.capability_missing'
  }

  if (!registration.walletReady) {
    return 'dark_capacity.public.wallet_not_ready'
  }

  const newest = latestAssignment(assignments)

  if (newest === undefined) {
    return 'dark_capacity.public.no_assignments_offered'
  }

  if (newest.state === 'rejected' || newest.state === 'cancelled') {
    return 'dark_capacity.public.assignment_declined'
  }

  if (newest.state === 'stale' || newest.state === 'blocked') {
    return 'dark_capacity.public.assignment_expired'
  }

  if (
    activeAssignmentStates.has(newest.state) &&
    leaseExpired(newest, nowIso) &&
    newest.closeoutRefs.length === 0
  ) {
    return 'dark_capacity.public.closeout_missing'
  }

  return null
}

const stageRank: Record<PylonCapacityFunnelStage, number> = {
  accepted: 6,
  artifact_producing: 5,
  assigned: 3,
  benchmarked: 1,
  dark: -1,
  eligible: 2,
  paid: 7,
  registered: 0,
  running: 4,
  settled: 8,
}

const assignmentStage = (
  assignment: PylonApiAssignmentRecord,
): PylonCapacityFunnelStage | null => {
  switch (assignment.state) {
    case 'accepted_work':
      return 'accepted'
    case 'closeout_submitted':
    case 'proof_submitted':
      return 'artifact_producing'
    case 'running':
      return 'running'
    case 'accepted':
    case 'offered':
      return 'assigned'
    default:
      return null
  }
}

const lifecycleStage = (
  lifecycle: PylonApiProviderJobLifecycleRecord,
): PylonCapacityFunnelStage => {
  switch (lifecycle.stage) {
    case 'accepted_work':
      return 'accepted'
    case 'artifact_submitted':
    case 'closeout_submitted':
      return 'artifact_producing'
    case 'running':
      return 'running'
    case 'accepted':
    case 'offered':
      return 'assigned'
  }
}

const highestStage = (
  stages: ReadonlyArray<PylonCapacityFunnelStage | null>,
): PylonCapacityFunnelStage | null =>
  stages
    .filter((stage): stage is PylonCapacityFunnelStage => stage !== null)
    .sort((left, right) => stageRank[right] - stageRank[left])[0] ?? null

const stageForPylon = (
  input: Readonly<{
    assignments: ReadonlyArray<PylonApiAssignmentRecord>
    darkReasonRef: PylonDarkCapacityReasonRef | null
    eligible: boolean
    lifecycle: ReadonlyArray<PylonApiProviderJobLifecycleRecord>
  }>,
): PylonCapacityFunnelStage => {
  const { assignments, darkReasonRef, eligible, lifecycle } = input

  if (
    darkReasonRef !== null &&
    darkReasonRef !== 'dark_capacity.public.no_assignments_offered'
  ) {
    return 'dark'
  }

  const projectedStage = highestStage([
    ...lifecycle.map(lifecycleStage),
    ...assignments.map(assignmentStage),
  ])

  if (projectedStage !== null) {
    return projectedStage
  }

  return eligible ? 'eligible' : 'registered'
}

export const pylonCapacityFunnelRecordsFromStore = (
  input: Readonly<{
    assignmentsByPylonRef: ReadonlyMap<
      string,
      ReadonlyArray<PylonApiAssignmentRecord>
    >
    lifecycleByPylonRef?: ReadonlyMap<
      string,
      ReadonlyArray<PylonApiProviderJobLifecycleRecord>
    >
    nowIso: string
    registrations: ReadonlyArray<PylonApiRegistrationRecord>
  }>,
): ReadonlyArray<PylonCapacityFunnelRecord> =>
  input.registrations.map((registration, index) => {
    const assignments =
      input.assignmentsByPylonRef.get(registration.pylonRef) ?? []
    const lifecycle =
      input.lifecycleByPylonRef?.get(registration.pylonRef) ?? []
    const heartbeatOnline = onlineHeartbeatStatuses.has(
      (registration.latestHeartbeatStatus ?? '').trim().toLowerCase(),
    )
    const fresh =
      heartbeatOnline &&
      pylonHeartbeatFresh(registration.latestHeartbeatAt, input.nowIso)
    const eligible =
      fresh &&
      pylonClientVersionMeetsMinimum(
        registration.clientVersion,
        PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
      ) &&
      registration.capabilityRefs.includes(assignmentReadyCapabilityRef) &&
      registration.walletReady
    const darkReasonRef = darkCapacityReasonRefForPylon({
      assignments,
      nowIso: input.nowIso,
      registration,
    })
    const ordinal = index + 1
    const stage = stageForPylon({
      assignments,
      darkReasonRef,
      eligible,
      lifecycle,
    })
    const reached = (threshold: PylonCapacityFunnelStage): boolean =>
      stageRank[stage] >= stageRank[threshold]

    return {
      acceptanceRefs: reached('accepted')
        ? [`acceptance.public.pylon_capacity.entry_${ordinal}`]
        : [],
      artifactRefs: reached('artifact_producing')
        ? [`artifact.public.pylon_capacity.entry_${ordinal}`]
        : [],
      assignmentRefs: reached('assigned')
        ? [`assignment.public.pylon_capacity.entry_${ordinal}`]
        : [],
      benchmarkRefs: reached('benchmarked')
        ? ['benchmark.public.pylon_capacity.version_capability_check']
        : [],
      capacityRef: `capacity.public.pylon_live.entry_${ordinal}`,
      caveatRefs: [
        'caveat.public.pylon_capacity_funnel.counts_only_no_device_identifiers',
      ],
      darkCapacityReasonRefs: darkReasonRef === null ? [] : [darkReasonRef],
      eligibilityRefs: reached('eligible')
        ? ['eligibility.public.pylon_capacity.assignment_ready']
        : [],
      evidenceRefs: [],
      id: `pylon_capacity_live_${ordinal}`,
      nodeRef: 'node.public.pylon_capacity.redacted',
      nodeVisibility: 'public',
      providerRef: 'provider.public.pylon_capacity.redacted',
      providerVisibility: 'public',
      rewardRefs: reached('paid')
        ? [`reward.public.pylon_capacity.entry_${ordinal}`]
        : [],
      runRefs: reached('running')
        ? [`run.public.pylon_capacity.entry_${ordinal}`]
        : [],
      settlementRefs: reached('settled')
        ? [`settlement.public.pylon_capacity.entry_${ordinal}`]
        : [],
      stage,
      updatedAtIso: registration.updatedAt,
      workClassRefs: [],
    }
  })

type PylonCapacityFunnelRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: () => string
  store?: PylonApiStore
}>

export type PylonCapacityFunnelSnapshotBucketKind = 'daily' | 'hourly'

export type PylonCapacityFunnelSnapshotRecord = Readonly<{
  aggregate: PylonCapacityFunnelAggregate
  bucketKind: PylonCapacityFunnelSnapshotBucketKind
  bucketStartAt: string
  createdAt: string
  id: string
  publicProjectionJson: string
  snapshotAt: string
  totalCount: number
  updatedAt: string
}>

type PylonCapacityFunnelSnapshotRow = Readonly<{
  aggregate_json: string
  bucket_kind: PylonCapacityFunnelSnapshotBucketKind
  bucket_start_at: string
  created_at: string
  id: string
  public_projection_json: string
  snapshot_at: string
  total_count: number
  updated_at: string
}>

export type PylonCapacityFunnelSnapshotStore = Readonly<{
  listSnapshots: (
    input: Readonly<{
      bucketKind: PylonCapacityFunnelSnapshotBucketKind
      limit: number
    }>,
  ) => Promise<ReadonlyArray<PylonCapacityFunnelSnapshotRecord>>
  pruneSnapshotsBefore: (
    input: Readonly<{
      bucketKind: PylonCapacityFunnelSnapshotBucketKind
      beforeIso: string
    }>,
  ) => Promise<void>
  upsertSnapshot: (
    record: PylonCapacityFunnelSnapshotRecord,
  ) => Promise<PylonCapacityFunnelSnapshotRecord>
}>

const snapshotRetentionPolicyRef =
  'retention.public.pylon_capacity_funnel.hourly_14d_daily_180d'
const hourlyRetentionMs = 14 * 24 * 60 * 60 * 1000
const dailyRetentionMs = 180 * 24 * 60 * 60 * 1000

const snapshotCaveatRefs = [
  'caveat.public.pylon_capacity_funnel.history_counts_only_no_device_identifiers',
  'caveat.public.pylon_capacity_funnel.retention_hourly_14d_daily_180d',
] as const

const bucketStartFor = (
  nowIso: string,
  bucketKind: PylonCapacityFunnelSnapshotBucketKind,
): string =>
  bucketKind === 'daily'
    ? utcStartOfDayIsoTimestamp(nowIso)
    : utcStartOfHourIsoTimestamp(nowIso)

const retentionCutoffFor = (
  nowIso: string,
  bucketKind: PylonCapacityFunnelSnapshotBucketKind,
): string =>
  isoTimestampAfterIso(
    nowIso,
    -(bucketKind === 'hourly' ? hourlyRetentionMs : dailyRetentionMs),
  )

const snapshotIdFor = (
  bucketKind: PylonCapacityFunnelSnapshotBucketKind,
  bucketStartAt: string,
): string =>
  `pylon_capacity_funnel_snapshot_${bucketKind}_${bucketStartAt.replaceAll(
    /[^A-Za-z0-9]+/g,
    '_',
  )}`

const publicSnapshotProjection = (
  record: PylonCapacityFunnelSnapshotRecord,
): Record<string, unknown> => ({
  bucketKind: record.bucketKind,
  bucketStartAt: record.bucketStartAt,
  funnel: record.aggregate,
  snapshotAt: record.snapshotAt,
})

export const buildPylonCapacityFunnelSnapshotRecord = (
  input: Readonly<{
    aggregate: PylonCapacityFunnelAggregate
    bucketKind: PylonCapacityFunnelSnapshotBucketKind
    nowIso: string
  }>,
): PylonCapacityFunnelSnapshotRecord => {
  const bucketStartAt = bucketStartFor(input.nowIso, input.bucketKind)
  const base: PylonCapacityFunnelSnapshotRecord = {
    aggregate: input.aggregate,
    bucketKind: input.bucketKind,
    bucketStartAt,
    createdAt: input.nowIso,
    id: snapshotIdFor(input.bucketKind, bucketStartAt),
    publicProjectionJson: '{}',
    snapshotAt: input.nowIso,
    totalCount: input.aggregate.totalCount,
    updatedAt: input.nowIso,
  }

  return {
    ...base,
    publicProjectionJson: JSON.stringify(publicSnapshotProjection(base)),
  }
}

const rowToSnapshot = (
  row: PylonCapacityFunnelSnapshotRow,
): PylonCapacityFunnelSnapshotRecord => ({
  aggregate: (safeJsonRecord(row.aggregate_json) ??
    {}) as unknown as PylonCapacityFunnelAggregate,
  bucketKind: row.bucket_kind,
  bucketStartAt: row.bucket_start_at,
  createdAt: row.created_at,
  id: row.id,
  publicProjectionJson: row.public_projection_json,
  snapshotAt: row.snapshot_at,
  totalCount: row.total_count,
  updatedAt: row.updated_at,
})

export const makeD1PylonCapacityFunnelSnapshotStore = (
  db: D1Database,
): PylonCapacityFunnelSnapshotStore => ({
  listSnapshots: async input => {
    const result = await db
      .prepare(
        `SELECT *
           FROM pylon_capacity_funnel_snapshots
          WHERE bucket_kind = ?
            AND archived_at IS NULL
          ORDER BY bucket_start_at DESC
          LIMIT ?`,
      )
      .bind(input.bucketKind, input.limit)
      .all<PylonCapacityFunnelSnapshotRow>()

    return (result.results ?? []).map(rowToSnapshot)
  },

  pruneSnapshotsBefore: async input => {
    await db
      .prepare(
        `UPDATE pylon_capacity_funnel_snapshots
            SET archived_at = ?
          WHERE bucket_kind = ?
            AND bucket_start_at < ?
            AND archived_at IS NULL`,
      )
      .bind(currentIsoTimestamp(), input.bucketKind, input.beforeIso)
      .run()
  },

  upsertSnapshot: async record => {
    await db
      .prepare(
        `INSERT INTO pylon_capacity_funnel_snapshots
          (id, bucket_kind, bucket_start_at, snapshot_at, total_count,
           aggregate_json, public_projection_json, created_at, updated_at,
           archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
         ON CONFLICT(bucket_kind, bucket_start_at) DO UPDATE SET
           snapshot_at = excluded.snapshot_at,
           total_count = excluded.total_count,
           aggregate_json = excluded.aggregate_json,
           public_projection_json = excluded.public_projection_json,
           updated_at = excluded.updated_at,
           archived_at = NULL`,
      )
      .bind(
        record.id,
        record.bucketKind,
        record.bucketStartAt,
        record.snapshotAt,
        record.totalCount,
        JSON.stringify(record.aggregate),
        record.publicProjectionJson,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return record
  },
})

export const readPylonCapacityFunnelRecords = async (
  input: Readonly<{
    nowIso: string
    store: PylonApiStore
  }>,
): Promise<ReadonlyArray<PylonCapacityFunnelRecord>> => {
  const registrations = await input.store.listRegistrations(
    registrationListLimit,
  )
  const assignmentsByPylonRef = new Map<
    string,
    ReadonlyArray<PylonApiAssignmentRecord>
  >()
  const lifecycleByPylonRef = new Map<
    string,
    ReadonlyArray<PylonApiProviderJobLifecycleRecord>
  >()

  // One batched query when the store supports it: the per-registration
  // loop is an N+1 that exceeds the Workers subrequest cap once enough
  // Pylons register (the live funnel 500 of 2026-06-11).
  if (input.store.listAssignmentsForPylons !== undefined) {
    const allAssignments = await input.store.listAssignmentsForPylons(
      registrations.map(registration => registration.pylonRef),
      registrationListLimit * assignmentListLimit,
    )
    for (const assignment of allAssignments) {
      const existing = assignmentsByPylonRef.get(assignment.pylonRef) ?? []
      if (existing.length >= assignmentListLimit) continue
      assignmentsByPylonRef.set(assignment.pylonRef, [...existing, assignment])
    }
    for (const registration of registrations) {
      if (!assignmentsByPylonRef.has(registration.pylonRef)) {
        assignmentsByPylonRef.set(registration.pylonRef, [])
      }
    }
  } else {
    for (const registration of registrations) {
      assignmentsByPylonRef.set(
        registration.pylonRef,
        await input.store.listAssignmentsForPylon(
          registration.pylonRef,
          assignmentListLimit,
        ),
      )
    }
  }

  const lifecycleRecords = await input.store.listProviderJobLifecycleForPylons(
    registrations.map(registration => registration.pylonRef),
    registrationListLimit * assignmentListLimit,
  )

  for (const record of lifecycleRecords) {
    lifecycleByPylonRef.set(record.pylonRef, [
      ...(lifecycleByPylonRef.get(record.pylonRef) ?? []),
      record,
    ])
  }

  return pylonCapacityFunnelRecordsFromStore({
    assignmentsByPylonRef,
    lifecycleByPylonRef,
    nowIso: input.nowIso,
    registrations,
  })
}

export const readPylonCapacityFunnelAggregate = async (
  input: Readonly<{
    nowIso: string
    store: PylonApiStore
  }>,
): Promise<PylonCapacityFunnelAggregate> =>
  aggregatePylonCapacityFunnel(
    await readPylonCapacityFunnelRecords(input),
    'public',
    input.nowIso,
  )

export const recordPylonCapacityFunnelSnapshots = async (
  input: Readonly<{
    nowIso: string
    snapshotStore: PylonCapacityFunnelSnapshotStore
    store: PylonApiStore
  }>,
): Promise<ReadonlyArray<PylonCapacityFunnelSnapshotRecord>> => {
  const aggregate = await readPylonCapacityFunnelAggregate({
    nowIso: input.nowIso,
    store: input.store,
  })
  const snapshots = (['hourly', 'daily'] as const).map(bucketKind =>
    buildPylonCapacityFunnelSnapshotRecord({
      aggregate,
      bucketKind,
      nowIso: input.nowIso,
    }),
  )

  await Promise.all(
    snapshots.map(snapshot => input.snapshotStore.upsertSnapshot(snapshot)),
  )
  await Promise.all(
    (['hourly', 'daily'] as const).map(bucketKind =>
      input.snapshotStore.pruneSnapshotsBefore({
        beforeIso: retentionCutoffFor(input.nowIso, bucketKind),
        bucketKind,
      }),
    ),
  )

  return snapshots
}

export const handlePylonCapacityFunnelHistoryApi = (
  request: Request,
  input: Readonly<{
    OPENAGENTS_DB?: D1Database
    nowIso?: () => string
    snapshotStore?: PylonCapacityFunnelSnapshotStore
  }>,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const snapshotStore =
    input.snapshotStore ??
    makeD1PylonCapacityFunnelSnapshotStore(input.OPENAGENTS_DB as D1Database)

  return Effect.promise(async () => {
    const [hourly, daily] = await Promise.all([
      snapshotStore.listSnapshots({ bucketKind: 'hourly', limit: 336 }),
      snapshotStore.listSnapshots({ bucketKind: 'daily', limit: 180 }),
    ])

    return noStoreJsonResponse({
      authority: PYLON_CAPACITY_FUNNEL_ACCOUNTING_READ_ONLY_AUTHORITY,
      caveatRefs: snapshotCaveatRefs,
      generatedAt: nowIso,
      history: {
        daily: daily.map(
          record => safeJsonRecord(record.publicProjectionJson) ?? {},
        ),
        hourly: hourly.map(record =>
          safeJsonRecord(record.publicProjectionJson) ?? {},
        ),
        retentionPolicyRef: snapshotRetentionPolicyRef,
      },
      kind: 'pylon_capacity_funnel_history',
      publicSafe: true,
    })
  })
}

export const handlePylonCapacityFunnelApi = (
  request: Request,
  input: PylonCapacityFunnelRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const store =
    input.store ?? makeD1PylonApiStore(input.OPENAGENTS_DB as D1Database)

  return Effect.promise(async () => {
    const records = await readPylonCapacityFunnelRecords({
      nowIso,
      store,
    })
    const funnel = aggregatePylonCapacityFunnel(records, 'public', nowIso)
    const joinLifecycleLadder = pylonJoinLifecycleLadderForFunnel(
      records,
      'public',
      nowIso,
    )

    return noStoreJsonResponse({
      authority: PYLON_CAPACITY_FUNNEL_ACCOUNTING_READ_ONLY_AUTHORITY,
      caveatRefs: [
        'caveat.public.pylon_capacity_funnel.paid_settled_pending_settlement_system',
        'caveat.public.pylon_capacity_funnel.counts_only_no_device_identifiers',
      ],
      funnel,
      generatedAt: nowIso,
      joinLifecycleLadder,
      kind: 'pylon_capacity_funnel_live',
      publicSafe: true,
    })
  })
}
