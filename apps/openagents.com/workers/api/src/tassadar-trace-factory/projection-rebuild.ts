/**
 * Projection-rebuild rules v0.1 for the Tassadar verified trace factory
 * (issue #4748) — the amendment this platform earned the hard way.
 *
 * Case law: #4744 (agent profile frozen at registration, never rebuilt
 * after owner-claim approval), #4745 (Artanis report and capacity
 * funnel frozen, blind to their own accepted work), #4746
 * (announcement-grade evidence refs that no public endpoint resolves) —
 * four frozen-projection incidents in 24 hours.
 *
 * The rule, as a typed contract: every public surface reporting factory
 * state (counters, corpus size, family coverage, validation rates)
 * rebuilds on VALIDATION TRANSITIONS, never on registration events.
 * Registration may only move pending/quarantine counts; the public
 * verified counters and the rebuild timestamp move when, and only when,
 * a validation transition lands.
 */

export const TASSADAR_PROJECTION_REBUILD_CONTRACT_VERSION =
  'projection_rebuild.v0.1'

export type TassadarRecordStatus =
  | 'registered'
  | 'quarantined'
  | 'verified'
  | 'rejected'

export type TassadarFactoryRegistrationEvent = Readonly<{
  kind: 'record_registered'
  recordId: string
  familyId: string
  tokenCount: number
  occurredAtIso: string
}>

export type TassadarFactoryValidationTransition = Readonly<{
  kind: 'validation_transition'
  recordId: string
  familyId: string
  tokenCount: number
  fromStatus: TassadarRecordStatus
  toStatus: Exclude<TassadarRecordStatus, 'registered'>
  verdictRef: string
  occurredAtIso: string
}>

export type TassadarFactoryProjectionEvent =
  | TassadarFactoryRegistrationEvent
  | TassadarFactoryValidationTransition

/**
 * The trigger type any compliant projection module may rebuild public
 * counters from. Registration events are unrepresentable here by
 * construction — that is the contract.
 */
export type TassadarProjectionRebuildTrigger = 'validation_transition'

export type TassadarFamilyCoverage = Readonly<{
  familyId: string
  verifiedRecords: number
  verifiedTokens: number
}>

export type TassadarFactoryProjection = Readonly<{
  contractVersion: typeof TASSADAR_PROJECTION_REBUILD_CONTRACT_VERSION
  /** advances ONLY on validation transitions; null until the first one */
  rebuiltAtIso: string | null
  /** private intake surface — may move on registration */
  registeredRecords: number
  quarantinedRecords: number
  /** public counters — move only on validation transitions */
  verifiedRecords: number
  rejectedRecords: number
  verifiedTokens: number
  familyCoverage: ReadonlyArray<TassadarFamilyCoverage>
  /** verified / (verified + rejected), null before any transition */
  validationRate: number | null
}>

export const emptyFactoryProjection = (): TassadarFactoryProjection => ({
  contractVersion: TASSADAR_PROJECTION_REBUILD_CONTRACT_VERSION,
  familyCoverage: [],
  quarantinedRecords: 0,
  rebuiltAtIso: null,
  registeredRecords: 0,
  rejectedRecords: 0,
  validationRate: null,
  verifiedRecords: 0,
  verifiedTokens: 0,
})

/**
 * The reference pure rebuild: a fold over the event log in which every
 * public counter derives exclusively from validation transitions.
 */
export const rebuildFactoryProjection = (
  events: ReadonlyArray<TassadarFactoryProjectionEvent>,
): TassadarFactoryProjection => {
  let registeredRecords = 0
  let quarantinedRecords = 0
  let verifiedRecords = 0
  let rejectedRecords = 0
  let verifiedTokens = 0
  let rebuiltAtIso: string | null = null
  const coverage = new Map<string, { records: number; tokens: number }>()
  for (const event of events) {
    if (event.kind === 'record_registered') {
      registeredRecords += 1
      quarantinedRecords += 1
      continue
    }
    rebuiltAtIso = event.occurredAtIso
    if (event.fromStatus === 'quarantined' || event.fromStatus === 'registered') {
      quarantinedRecords = Math.max(0, quarantinedRecords - 1)
    }
    if (event.fromStatus === 'verified') {
      verifiedRecords = Math.max(0, verifiedRecords - 1)
      verifiedTokens = Math.max(0, verifiedTokens - event.tokenCount)
      const entry = coverage.get(event.familyId)
      if (entry !== undefined) {
        entry.records = Math.max(0, entry.records - 1)
        entry.tokens = Math.max(0, entry.tokens - event.tokenCount)
      }
    }
    if (event.toStatus === 'verified') {
      verifiedRecords += 1
      verifiedTokens += event.tokenCount
      const entry = coverage.get(event.familyId) ?? { records: 0, tokens: 0 }
      entry.records += 1
      entry.tokens += event.tokenCount
      coverage.set(event.familyId, entry)
    }
    if (event.toStatus === 'rejected') {
      rejectedRecords += 1
    }
    if (event.toStatus === 'quarantined') {
      quarantinedRecords += 1
    }
  }
  const judged = verifiedRecords + rejectedRecords

  return {
    contractVersion: TASSADAR_PROJECTION_REBUILD_CONTRACT_VERSION,
    familyCoverage: [...coverage.entries()]
      .map(([familyId, entry]) => ({
        familyId,
        verifiedRecords: entry.records,
        verifiedTokens: entry.tokens,
      }))
      .sort((left, right) => (left.familyId < right.familyId ? -1 : 1)),
    quarantinedRecords,
    rebuiltAtIso,
    registeredRecords,
    rejectedRecords,
    validationRate: judged === 0 ? null : verifiedRecords / judged,
    verifiedRecords,
    verifiedTokens,
  }
}

/**
 * What any factory projection module must declare and satisfy. The
 * `rebuildTriggers` element type makes registration-triggered public
 * rebuilds a compile error; the runtime checker below catches modules
 * that lie.
 */
export type TassadarFactoryProjectionModule = Readonly<{
  contractVersion: typeof TASSADAR_PROJECTION_REBUILD_CONTRACT_VERSION
  projectionId: string
  rebuildTriggers: ReadonlyArray<TassadarProjectionRebuildTrigger>
  rebuild: (
    events: ReadonlyArray<TassadarFactoryProjectionEvent>,
  ) => TassadarFactoryProjection
}>

export type TassadarProjectionComplianceViolation = Readonly<
  | { kind: 'missing_validation_transition_trigger'; projectionId: string }
  | {
      kind: 'public_counter_moved_on_registration'
      projectionId: string
      counter: string
    }
  | { kind: 'rebuilt_at_moved_on_registration'; projectionId: string }
  | {
      kind: 'public_counter_frozen_on_transition'
      projectionId: string
      counter: string
    }
  | { kind: 'rebuilt_at_frozen_on_transition'; projectionId: string }
>

const REGISTRATION_ONLY_LOG: ReadonlyArray<TassadarFactoryProjectionEvent> = [
  {
    familyId: 'family.compliance_probe.v1',
    kind: 'record_registered',
    occurredAtIso: '2026-06-11T00:00:00.000Z',
    recordId: 'trace_compliance_probe_a',
    tokenCount: 96,
  },
  {
    familyId: 'family.compliance_probe.v1',
    kind: 'record_registered',
    occurredAtIso: '2026-06-11T00:00:01.000Z',
    recordId: 'trace_compliance_probe_b',
    tokenCount: 128,
  },
]

const TRANSITION_LOG: ReadonlyArray<TassadarFactoryProjectionEvent> = [
  ...REGISTRATION_ONLY_LOG,
  {
    familyId: 'family.compliance_probe.v1',
    fromStatus: 'quarantined',
    kind: 'validation_transition',
    occurredAtIso: '2026-06-11T00:00:02.000Z',
    recordId: 'trace_compliance_probe_a',
    tokenCount: 96,
    toStatus: 'verified',
    verdictRef: 'verdict.compliance_probe.a',
  },
  {
    familyId: 'family.compliance_probe.v1',
    fromStatus: 'quarantined',
    kind: 'validation_transition',
    occurredAtIso: '2026-06-11T00:00:03.000Z',
    recordId: 'trace_compliance_probe_b',
    tokenCount: 128,
    toStatus: 'rejected',
    verdictRef: 'verdict.compliance_probe.b',
  },
]

/**
 * Runtime compliance check: replays a registration-only log and a
 * transition log through the module and verifies the #4744/#4745
 * failure classes cannot recur — public counters must stay frozen on
 * registration and must move (with the rebuild timestamp) on
 * validation transitions.
 */
export const projectionRebuildCompliance = (
  module: TassadarFactoryProjectionModule,
): ReadonlyArray<TassadarProjectionComplianceViolation> => {
  const violations: Array<TassadarProjectionComplianceViolation> = []
  if (!module.rebuildTriggers.includes('validation_transition')) {
    violations.push({
      kind: 'missing_validation_transition_trigger',
      projectionId: module.projectionId,
    })
  }
  const afterRegistration = module.rebuild(REGISTRATION_ONLY_LOG)
  const publicCounters: ReadonlyArray<
    readonly [string, (projection: TassadarFactoryProjection) => number]
  > = [
    ['verifiedRecords', projection => projection.verifiedRecords],
    ['verifiedTokens', projection => projection.verifiedTokens],
    ['rejectedRecords', projection => projection.rejectedRecords],
    [
      'familyCoverage',
      projection =>
        projection.familyCoverage.reduce(
          (total, family) => total + family.verifiedRecords,
          0,
        ),
    ],
  ]
  for (const [counter, readCounter] of publicCounters) {
    if (readCounter(afterRegistration) !== 0) {
      violations.push({
        counter,
        kind: 'public_counter_moved_on_registration',
        projectionId: module.projectionId,
      })
    }
  }
  if (afterRegistration.rebuiltAtIso !== null) {
    violations.push({
      kind: 'rebuilt_at_moved_on_registration',
      projectionId: module.projectionId,
    })
  }
  const afterTransitions = module.rebuild(TRANSITION_LOG)
  if (
    afterTransitions.verifiedRecords !== 1 ||
    afterTransitions.verifiedTokens !== 96
  ) {
    violations.push({
      counter: 'verifiedRecords/verifiedTokens',
      kind: 'public_counter_frozen_on_transition',
      projectionId: module.projectionId,
    })
  }
  if (afterTransitions.rejectedRecords !== 1) {
    violations.push({
      counter: 'rejectedRecords',
      kind: 'public_counter_frozen_on_transition',
      projectionId: module.projectionId,
    })
  }
  if (afterTransitions.rebuiltAtIso !== '2026-06-11T00:00:03.000Z') {
    violations.push({
      kind: 'rebuilt_at_frozen_on_transition',
      projectionId: module.projectionId,
    })
  }

  return violations
}

/** The reference projection module, compliant by construction. */
export const tassadarFactoryReferenceProjection: TassadarFactoryProjectionModule =
  {
    contractVersion: TASSADAR_PROJECTION_REBUILD_CONTRACT_VERSION,
    projectionId: 'projection.tassadar_trace_factory.reference.v0_1',
    rebuild: rebuildFactoryProjection,
    rebuildTriggers: ['validation_transition'],
  }
