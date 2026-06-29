import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import {
  type PylonCapacityFunnelRecord,
  type PylonCapacityFunnelStage,
  projectPylonCapacityFunnelRecord,
} from './pylon-capacity-funnel'

// Pluralis-style staged join ladder for Pylon contributors (openagents
// issue #4848, Pluralis roadmap P0.1). The forward path is
// registered -> qualified -> state_synced -> warmup -> active, with the
// back edge active -> lagged -> sync_reentry -> state_synced when a
// device falls beyond max_allowed_stale and re-ramps through the same
// sync path instead of being rejected or trusted.
export const PylonJoinLifecycleState = S.Literals([
  'active',
  'lagged',
  'qualified',
  'registered',
  'state_synced',
  'sync_reentry',
  'warmup',
])
export type PylonJoinLifecycleState = typeof PylonJoinLifecycleState.Type

// `join_deferred_seal_in_flight` is a queue-visibility code, not a ladder
// edge: it marks a join transition or bootstrap grant that the dispatcher
// queued while a merge/seal operation was in flight (Pluralis roadmap
// P1.3, openagents issue #4851). No transition in the closed set below
// carries it; the deferred request replays unchanged once the seal
// barrier clears.
export const PylonJoinLifecycleReasonCodes = [
  'join_lifecycle.public.beyond_max_allowed_stale',
  'join_lifecycle.public.durable_seal_digest_synced',
  'join_lifecycle.public.join_deferred_seal_in_flight',
  'join_lifecycle.public.qualification_gate_passed',
  'join_lifecycle.public.reentry_seal_digest_synced',
  'join_lifecycle.public.shadow_work_verified',
  'join_lifecycle.public.sync_reentry_started',
  'join_lifecycle.public.warmup_started',
] as const
export type PylonJoinLifecycleReasonCode =
  (typeof PylonJoinLifecycleReasonCodes)[number]

export type PylonJoinLifecycleTransition = Readonly<{
  fromState: PylonJoinLifecycleState
  reasonCode: PylonJoinLifecycleReasonCode
  toState: PylonJoinLifecycleState
}>

// Closed transition set. Every legal transition carries exactly one
// reason code from the closed taxonomy above, so a transition event is
// never emitted without a typed reason.
export const PYLON_JOIN_LIFECYCLE_TRANSITIONS: ReadonlyArray<PylonJoinLifecycleTransition> =
  Object.freeze([
    {
      fromState: 'registered',
      reasonCode: 'join_lifecycle.public.qualification_gate_passed',
      toState: 'qualified',
    },
    {
      fromState: 'qualified',
      reasonCode: 'join_lifecycle.public.durable_seal_digest_synced',
      toState: 'state_synced',
    },
    {
      fromState: 'state_synced',
      reasonCode: 'join_lifecycle.public.warmup_started',
      toState: 'warmup',
    },
    {
      fromState: 'warmup',
      reasonCode: 'join_lifecycle.public.shadow_work_verified',
      toState: 'active',
    },
    {
      fromState: 'active',
      reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
      toState: 'lagged',
    },
    {
      fromState: 'lagged',
      reasonCode: 'join_lifecycle.public.sync_reentry_started',
      toState: 'sync_reentry',
    },
    {
      fromState: 'sync_reentry',
      reasonCode: 'join_lifecycle.public.reentry_seal_digest_synced',
      toState: 'state_synced',
    },
  ])

export class PylonJoinLifecycleTransitionError extends S.TaggedErrorClass<PylonJoinLifecycleTransitionError>()(
  'PylonJoinLifecycleTransitionError',
  {
    kind: S.Literals([
      'illegal_transition',
      'reason_code_mismatch',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export class PylonJoinLifecycleUnsafe extends S.TaggedErrorClass<PylonJoinLifecycleUnsafe>()(
  'PylonJoinLifecycleUnsafe',
  {
    reason: S.String,
  },
) {}

export type PylonJoinLifecycleRecord = Readonly<{
  capacityRef: string
  publicProjectionJson: string
  reasonCode: PylonJoinLifecycleReasonCode | null
  receiptRefs: ReadonlyArray<string>
  state: PylonJoinLifecycleState
  updatedAtIso: string
}>

// Receipt-compatible transition event. Timestamps are always passed in
// by the caller; this module never reads a clock.
export type PylonJoinLifecycleEventRecord = Readonly<{
  capacityRef: string
  fromState: PylonJoinLifecycleState
  id: string
  occurredAtIso: string
  reasonCode: PylonJoinLifecycleReasonCode
  receiptRef: string
  toState: PylonJoinLifecycleState
}>

export type PylonJoinLifecycleProjection = Readonly<{
  capacityRef: string
  ladderRank: number
  reasonCode: PylonJoinLifecycleReasonCode | null
  receiptRefs: ReadonlyArray<string>
  state: PylonJoinLifecycleState
  stateLabel: string
  updatedAtDisplay: string
}>

export type PylonJoinLifecycleLadderEntry = Readonly<{
  capacityRef: string
  ladderRank: number
  state: PylonJoinLifecycleState
  stateLabel: string
}>

export type PylonJoinLifecycleLadderCount = Readonly<{
  count: number
  key: string
}>

export type PylonJoinLifecycleLadderProjection = Readonly<{
  audience: BlueprintMissionBriefingAudience
  byState: ReadonlyArray<PylonJoinLifecycleLadderCount>
  caveatRefs: ReadonlyArray<string>
  entries: ReadonlyArray<PylonJoinLifecycleLadderEntry>
  schemaVersion: 'openagents.pylon.join_lifecycle_ladder.v1'
  sourceRefs: ReadonlyArray<string>
  totalCount: number
}>

export const pylonJoinLifecycleStateLabelByState: Record<
  PylonJoinLifecycleState,
  string
> = {
  active: 'Active',
  lagged: 'Lagged',
  qualified: 'Qualified',
  registered: 'Registered',
  state_synced: 'State synced',
  sync_reentry: 'Sync re-entry',
  warmup: 'Warmup',
}

// Position on the bright ladder. Back-edge states are negative: a
// lagged device has fallen off the ladder and a sync_reentry device is
// one re-sync away from rejoining it at state_synced.
export const pylonJoinLifecycleLadderRankByState: Record<
  PylonJoinLifecycleState,
  number
> = {
  active: 4,
  lagged: -2,
  qualified: 1,
  registered: 0,
  state_synced: 2,
  sync_reentry: -1,
  warmup: 3,
}

// Conservative claim strength used when multiple funnel reason codes
// map to different ladder states: the funnel row claims the weakest
// supported rung.
const claimRankByState: Record<PylonJoinLifecycleState, number> = {
  active: 6,
  lagged: 1,
  qualified: 3,
  registered: 0,
  state_synced: 4,
  sync_reentry: 2,
  warmup: 5,
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|access[_-]?token|bearer|cookie|email|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mnemonic|oauth|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination)|preimage|private[_-]?key|secret|serial[_-]?number|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

// Platform-issued ladder taxonomy constants are a closed enum; strip
// them before the substring scan so the closed set cannot trip its own
// scanner (the dark_capacity.public.wallet_not_ready lesson from the
// live funnel 500 of 2026-06-11).
const platformIssuedReasonPattern = /join_lifecycle\.public\.[a-z0-9_]+/g

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRef = (label: string, ref: string): void => {
  const scrubbed = ref.replaceAll(
    platformIssuedReasonPattern,
    'join_lifecycle.public.reason',
  )

  if (
    !safeRefPattern.test(ref) ||
    unsafeRefPattern.test(scrubbed) ||
    isoTimestampPattern.test(ref)
  ) {
    throw new PylonJoinLifecycleUnsafe({
      reason: `${label} contains private host, wallet, payment, payout target, or raw timestamp material.`,
    })
  }
}

const projectionText = (
  projection: PylonJoinLifecycleProjection,
): string =>
  [
    projection.capacityRef,
    projection.reasonCode ?? '',
    ...projection.receiptRefs,
  ].join(' ')

export const pylonJoinLifecycleProjectionHasPrivateMaterial = (
  projection: PylonJoinLifecycleProjection,
): boolean => {
  const text = projectionText(projection).replaceAll(
    platformIssuedReasonPattern,
    'join_lifecycle.public.reason',
  )

  return unsafeRefPattern.test(text) || isoTimestampPattern.test(text)
}

export const projectPylonJoinLifecycleRecord = (
  record: PylonJoinLifecycleRecord,
  nowIso: string,
): PylonJoinLifecycleProjection => {
  assertSafeRef('join lifecycle capacity ref', record.capacityRef)

  for (const receiptRef of record.receiptRefs) {
    assertSafeRef('join lifecycle receipt ref', receiptRef)
  }

  const projection: PylonJoinLifecycleProjection = {
    capacityRef: record.capacityRef,
    ladderRank: pylonJoinLifecycleLadderRankByState[record.state],
    reasonCode: record.reasonCode,
    receiptRefs: uniqueRefs(record.receiptRefs),
    state: record.state,
    stateLabel: pylonJoinLifecycleStateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (pylonJoinLifecycleProjectionHasPrivateMaterial(projection)) {
    throw new PylonJoinLifecycleUnsafe({
      reason: 'Join lifecycle projection contains private material.',
    })
  }

  return projection
}

export const buildPylonJoinLifecycleRecord = (
  input: Readonly<{
    capacityRef: string
    nowIso: string
  }>,
): PylonJoinLifecycleRecord => {
  assertSafeRef('join lifecycle capacity ref', input.capacityRef)

  const record: PylonJoinLifecycleRecord = {
    capacityRef: input.capacityRef,
    publicProjectionJson: '{}',
    reasonCode: null,
    receiptRefs: [],
    state: 'registered',
    updatedAtIso: input.nowIso,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      projectPylonJoinLifecycleRecord(record, input.nowIso),
    ),
  }
}

/**
 * Applies one reason-coded ladder transition. Illegal edges and
 * mismatched reason codes are rejected with typed errors, every legal
 * transition emits a receipt-compatible event, and the public
 * projection is rebuilt as part of the transition itself (projections
 * rebuild on transitions, not registration events).
 */
export const transitionPylonJoinLifecycleRecord = (
  input: Readonly<{
    eventId: string
    nowIso: string
    reasonCode: PylonJoinLifecycleReasonCode
    receiptRef: string
    record: PylonJoinLifecycleRecord
    toState: PylonJoinLifecycleState
  }>,
): Readonly<{
  event: PylonJoinLifecycleEventRecord
  record: PylonJoinLifecycleRecord
}> => {
  assertSafeRef('join lifecycle receipt ref', input.receiptRef)

  const transition = PYLON_JOIN_LIFECYCLE_TRANSITIONS.find(
    candidate =>
      candidate.fromState === input.record.state &&
      candidate.toState === input.toState,
  )

  if (transition === undefined) {
    throw new PylonJoinLifecycleTransitionError({
      kind: 'illegal_transition',
      reason: `Cannot transition Pylon join lifecycle from ${input.record.state} to ${input.toState}.`,
    })
  }

  if (transition.reasonCode !== input.reasonCode) {
    throw new PylonJoinLifecycleTransitionError({
      kind: 'reason_code_mismatch',
      reason: `Transition from ${input.record.state} to ${input.toState} requires reason code ${transition.reasonCode}.`,
    })
  }

  const nextRecord: PylonJoinLifecycleRecord = {
    ...input.record,
    reasonCode: input.reasonCode,
    receiptRefs: uniqueRefs([...input.record.receiptRefs, input.receiptRef]),
    state: input.toState,
    updatedAtIso: input.nowIso,
  }

  return {
    event: {
      capacityRef: input.record.capacityRef,
      fromState: input.record.state,
      id: `pylon_join_lifecycle_event_${input.eventId}`,
      occurredAtIso: input.nowIso,
      reasonCode: input.reasonCode,
      receiptRef: input.receiptRef,
      toState: input.toState,
    },
    record: {
      ...nextRecord,
      publicProjectionJson: JSON.stringify(
        projectPylonJoinLifecycleRecord(nextRecord, input.nowIso),
      ),
    },
  }
}

// Mapping table from the funnel's existing reason-code taxonomy to
// ladder states. The dark-capacity taxonomy is untouched; this is the
// additive bright-side reading of the same evidence. sync_reentry is
// intentionally absent: it is reachable only through an explicit
// lagged -> sync_reentry transition, never inferred from funnel
// snapshots, because the funnel cannot observe re-ramping yet.
const joinLifecycleStateByDarkReasonRef: Readonly<
  Record<string, PylonJoinLifecycleState>
> = Object.freeze({
  'dark_capacity.public.assignment_declined': 'qualified',
  'dark_capacity.public.assignment_expired': 'lagged',
  'dark_capacity.public.capability_missing': 'registered',
  'dark_capacity.public.closeout_missing': 'lagged',
  'dark_capacity.public.never_heartbeated': 'registered',
  'dark_capacity.public.no_assignments_offered': 'qualified',
  'dark_capacity.public.stale_heartbeat': 'lagged',
  'dark_capacity.public.version_incompatible': 'registered',
  'dark_capacity.public.wallet_not_ready': 'registered',
})

const joinLifecycleStateByFunnelStage: Record<
  Exclude<PylonCapacityFunnelStage, 'dark'>,
  PylonJoinLifecycleState
> = {
  accepted: 'active',
  artifact_producing: 'warmup',
  assigned: 'state_synced',
  benchmarked: 'qualified',
  eligible: 'qualified',
  paid: 'active',
  registered: 'registered',
  running: 'warmup',
  settled: 'active',
}

export const joinLifecycleStateForFunnel = (
  input: Readonly<{
    darkCapacityReasonRefs: ReadonlyArray<string>
    stage: PylonCapacityFunnelStage
  }>,
): PylonJoinLifecycleState => {
  if (input.stage !== 'dark') {
    return joinLifecycleStateByFunnelStage[input.stage]
  }

  // Multiple dark reasons claim the weakest supported rung; an unknown
  // reason ref or a reasonless dark row proves nothing beyond
  // registration.
  const states = input.darkCapacityReasonRefs.map(
    (reasonRef): PylonJoinLifecycleState =>
      joinLifecycleStateByDarkReasonRef[reasonRef] ?? 'registered',
  )

  if (states.length === 0) {
    return 'registered'
  }

  return states.reduce((weakest, state) =>
    claimRankByState[state] < claimRankByState[weakest] ? state : weakest,
  )
}

const countBy = (
  values: ReadonlyArray<string>,
): ReadonlyArray<PylonJoinLifecycleLadderCount> =>
  [...values.reduce((counts, value) => {
    counts.set(value, (counts.get(value) ?? 0) + 1)

    return counts
  }, new Map<string, number>())]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, count]) => ({ count, key }))

/**
 * Public-safe ladder rendering for the capacity funnel: one entry per
 * funnel record carrying its join-lifecycle position. The entries are
 * derived from the funnel's own audience-redacted projections, so the
 * ladder never sees more than the funnel already shows that audience.
 */
export const pylonJoinLifecycleLadderForFunnel = (
  records: ReadonlyArray<PylonCapacityFunnelRecord>,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonJoinLifecycleLadderProjection => {
  const entries = records
    .map(record => projectPylonCapacityFunnelRecord(record, audience, nowIso))
    .map((projection): PylonJoinLifecycleLadderEntry => {
      const state = joinLifecycleStateForFunnel({
        darkCapacityReasonRefs: projection.darkCapacityReasonRefs,
        stage: projection.stage,
      })

      return {
        capacityRef: projection.capacityRef,
        ladderRank: pylonJoinLifecycleLadderRankByState[state],
        state,
        stateLabel: pylonJoinLifecycleStateLabelByState[state],
      }
    })

  for (const entry of entries) {
    assertSafeRef('join lifecycle ladder capacity ref', entry.capacityRef)
  }

  return {
    audience,
    byState: countBy(entries.map(entry => entry.state)),
    caveatRefs: [
      'caveat.public.pylon_join_lifecycle.counts_and_refs_only_no_device_identifiers',
      'caveat.public.pylon_join_lifecycle.ladder_position_is_contract_projection_not_live_device_claim',
    ],
    entries,
    schemaVersion: 'openagents.pylon.join_lifecycle_ladder.v1',
    sourceRefs: [
      'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p0.1',
      'route:/api/public/pylon-capacity-funnel',
    ],
    totalCount: entries.length,
  }
}
