import { Effect } from 'effect'

import {
  assignmentReadyCapabilityRef,
  onlineHeartbeatStatuses,
  pylonHeartbeatFresh,
} from './autopilot-work-placement-selector'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'
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
  type PylonCapacityFunnelRecord,
  type PylonCapacityFunnelStage,
  aggregatePylonCapacityFunnel,
} from './pylon-capacity-funnel'
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

const stageForPylon = (
  input: Readonly<{
    darkReasonRef: PylonDarkCapacityReasonRef | null
    eligible: boolean
    lifecycle: ReadonlyArray<PylonApiProviderJobLifecycleRecord>
  }>,
): PylonCapacityFunnelStage => {
  const { darkReasonRef, eligible, lifecycle } = input

  if (
    darkReasonRef !== null &&
    darkReasonRef !== 'dark_capacity.public.no_assignments_offered'
  ) {
    return 'dark'
  }

  if (lifecycle.some(record => record.stage === 'accepted_work')) {
    return 'accepted'
  }

  if (
    lifecycle.some(record =>
      record.stage === 'artifact_submitted' ||
      record.stage === 'closeout_submitted',
    )
  ) {
    return 'artifact_producing'
  }

  if (lifecycle.some(record => record.stage === 'running')) {
    return 'running'
  }

  if (
    lifecycle.some(record =>
      record.stage === 'accepted' || record.stage === 'offered',
    )
  ) {
    return 'assigned'
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
      darkReasonRef,
      eligible,
      lifecycle,
    })
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
      rewardRefs: [],
      runRefs: reached('running')
        ? [`run.public.pylon_capacity.entry_${ordinal}`]
        : [],
      settlementRefs: [],
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
    const registrations = await store.listRegistrations(registrationListLimit)
    const assignmentsByPylonRef = new Map<
      string,
      ReadonlyArray<PylonApiAssignmentRecord>
    >()
    const lifecycleByPylonRef = new Map<
      string,
      ReadonlyArray<PylonApiProviderJobLifecycleRecord>
    >()

    for (const registration of registrations) {
      assignmentsByPylonRef.set(
        registration.pylonRef,
        await store.listAssignmentsForPylon(
          registration.pylonRef,
          assignmentListLimit,
        ),
      )
    }
    const lifecycleRecords = await store.listProviderJobLifecycleForPylons(
      registrations.map(registration => registration.pylonRef),
      registrationListLimit * assignmentListLimit,
    )

    for (const record of lifecycleRecords) {
      lifecycleByPylonRef.set(record.pylonRef, [
        ...(lifecycleByPylonRef.get(record.pylonRef) ?? []),
        record,
      ])
    }

    const records = pylonCapacityFunnelRecordsFromStore({
      assignmentsByPylonRef,
      lifecycleByPylonRef,
      nowIso,
      registrations,
    })

    return noStoreJsonResponse({
      authority: PYLON_CAPACITY_FUNNEL_ACCOUNTING_READ_ONLY_AUTHORITY,
      caveatRefs: [
        'caveat.public.pylon_capacity_funnel.paid_settled_pending_settlement_system',
        'caveat.public.pylon_capacity_funnel.counts_only_no_device_identifiers',
      ],
      funnel: aggregatePylonCapacityFunnel(records, 'public', nowIso),
      generatedAt: nowIso,
      kind: 'pylon_capacity_funnel_live',
      publicSafe: true,
    })
  })
}
