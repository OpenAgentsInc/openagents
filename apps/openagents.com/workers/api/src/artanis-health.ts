import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisHealthAudience = S.Literals([
  'operator',
  'public_artanis',
  'public_forum',
])
export type ArtanisHealthAudience = typeof ArtanisHealthAudience.Type

export const ArtanisHealthSignalKind = S.Literals([
  'blocker_reason',
  'forum_publication_lag',
  'khala_readiness',
  'fleet_overseer',
  'last_tick',
  'loop_freshness',
  'model_lab_report_freshness',
  'nexus_public_stats_freshness',
  'pending_approvals',
  'pylon_stats_freshness',
  'runner_backend_availability',
])
export type ArtanisHealthSignalKind = typeof ArtanisHealthSignalKind.Type

export const ArtanisHealthSignalState = S.Literals([
  'available',
  'blocked',
  'degraded',
  'fresh',
  'missing',
  'stale',
  'unavailable',
  'unknown',
])
export type ArtanisHealthSignalState = typeof ArtanisHealthSignalState.Type

export const ArtanisHealthOverallState = S.Literals([
  'blocked',
  'degraded',
  'healthy',
  'stale',
  'unavailable',
])
export type ArtanisHealthOverallState = typeof ArtanisHealthOverallState.Type

export class ArtanisHealthSignalRecord extends S.Class<ArtanisHealthSignalRecord>(
  'ArtanisHealthSignalRecord',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  count: S.Number,
  kind: ArtanisHealthSignalKind,
  label: S.String,
  observedAtIso: S.String,
  operatorDetailRefs: S.Array(S.String),
  publicRecoveryActionRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  signalRef: S.String,
  sourceRefs: S.Array(S.String),
  state: ArtanisHealthSignalState,
  subjectUpdatedAtIso: S.NullOr(S.String),
}) {}

export class ArtanisHealthSnapshotRecord extends S.Class<ArtanisHealthSnapshotRecord>(
  'ArtanisHealthSnapshotRecord',
)({
  agentId: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  latestTickRef: S.NullOr(S.String),
  loopRef: S.String,
  operatorRecoveryActionRefs: S.Array(S.String),
  overallState: ArtanisHealthOverallState,
  overclaimBlocked: S.Boolean,
  overclaimBlockerRefs: S.Array(S.String),
  pendingApprovalRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  runnerBackendRefs: S.Array(S.String),
  signals: S.Array(ArtanisHealthSignalRecord),
  snapshotRef: S.String,
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisHealthSignalProjection extends S.Class<ArtanisHealthSignalProjection>(
  'ArtanisHealthSignalProjection',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  count: S.Number,
  kind: ArtanisHealthSignalKind,
  label: S.String,
  observedAtDisplay: S.String,
  operatorDetailRefs: S.Array(S.String),
  publicRecoveryActionRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  signalRef: S.String,
  sourceRefs: S.Array(S.String),
  state: ArtanisHealthSignalState,
  subjectUpdatedDisplay: S.NullOr(S.String),
}) {}

export class ArtanisHealthSnapshotProjection extends S.Class<ArtanisHealthSnapshotProjection>(
  'ArtanisHealthSnapshotProjection',
)({
  agentId: S.String,
  audience: ArtanisHealthAudience,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  latestTickRef: S.NullOr(S.String),
  loopRef: S.String,
  operatorRecoveryActionRefs: S.Array(S.String),
  overallState: ArtanisHealthOverallState,
  overclaimBlocked: S.Boolean,
  overclaimBlockerRefs: S.Array(S.String),
  pendingApprovalCount: S.Number,
  pendingApprovalRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  runnerBackendRefs: S.Array(S.String),
  signals: S.Array(ArtanisHealthSignalProjection),
  snapshotRef: S.String,
  sourceRefs: S.Array(S.String),
  staleOrBlockedSignalCount: S.Number,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisHealthUnsafe extends S.TaggedErrorClass<ArtanisHealthUnsafe>()(
  'ArtanisHealthUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_HEALTH_SIGNAL_KINDS: ReadonlyArray<ArtanisHealthSignalKind> =
  [
    'blocker_reason',
    'forum_publication_lag',
    'khala_readiness',
    'fleet_overseer',
    'last_tick',
    'loop_freshness',
    'model_lab_report_freshness',
    'nexus_public_stats_freshness',
    'pending_approvals',
    'pylon_stats_freshness',
    'runner_backend_availability',
  ]

const attentionStates = new Set<ArtanisHealthSignalState>([
  'blocked',
  'degraded',
  'missing',
  'stale',
  'unavailable',
  'unknown',
])

const healthyStates = new Set<ArtanisHealthSignalState>([
  'available',
  'fresh',
])

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeHealthPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(evidence\.private|health\.operator|operator\.|receipt\.operator|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisHealthUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertMaybeIso = (label: string, iso: string | null): void => {
  if (iso !== null) {
    assertValidIso(label, iso)
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeHealthPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisHealthUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, or raw timestamp material.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisHealthAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const signalNeedsAttention = (signal: ArtanisHealthSignalRecord): boolean =>
  attentionStates.has(signal.state)

const requiredSignalKindSet = new Set(ARTANIS_HEALTH_SIGNAL_KINDS)

const assertSignal = (signal: ArtanisHealthSignalRecord): void => {
  assertValidIso('signal.observedAtIso', signal.observedAtIso)
  assertMaybeIso('signal.subjectUpdatedAtIso', signal.subjectUpdatedAtIso)
  assertSafeRefs('Artanis health signal ref', [signal.signalRef])
  assertSafeRefs('Artanis health signal blocker refs', signal.blockerRefs)
  assertSafeRefs('Artanis health signal caveat refs', signal.caveatRefs)
  assertSafeRefs(
    'Artanis health signal operator detail refs',
    signal.operatorDetailRefs,
  )
  assertSafeRefs(
    'Artanis health signal public recovery action refs',
    signal.publicRecoveryActionRefs,
  )
  assertSafeRefs(
    'Artanis health signal public status refs',
    signal.publicStatusRefs,
  )
  assertSafeRefs('Artanis health signal source refs', signal.sourceRefs)

  if (!requiredSignalKindSet.has(signal.kind)) {
    throw new ArtanisHealthUnsafe({
      reason: 'Artanis health signals must use an enumerated signal kind.',
    })
  }

  if (signal.count < 0) {
    throw new ArtanisHealthUnsafe({
      reason: 'Artanis health signal counts cannot be negative.',
    })
  }

  if (
    signalNeedsAttention(signal) &&
    !hasAny(signal.publicRecoveryActionRefs) &&
    !hasAny(signal.blockerRefs)
  ) {
    throw new ArtanisHealthUnsafe({
      reason:
        'Stale, blocked, missing, unavailable, degraded, or unknown Artanis health signals require recovery action refs or blocker refs.',
    })
  }

  if (signal.state === 'blocked' && !hasAny(signal.blockerRefs)) {
    throw new ArtanisHealthUnsafe({
      reason: 'Blocked Artanis health signals require blocker refs.',
    })
  }

  if (healthyStates.has(signal.state) && hasAny(signal.blockerRefs)) {
    throw new ArtanisHealthUnsafe({
      reason: 'Fresh or available Artanis health signals cannot carry blockers.',
    })
  }
}

const assertSnapshot = (snapshot: ArtanisHealthSnapshotRecord): void => {
  assertValidIso('snapshot.createdAtIso', snapshot.createdAtIso)
  assertValidIso('snapshot.updatedAtIso', snapshot.updatedAtIso)
  assertSafeRefs('Artanis health snapshot agent id', [snapshot.agentId])
  assertSafeRefs('Artanis health snapshot blocker refs', snapshot.blockerRefs)
  assertSafeRefs('Artanis health snapshot caveat refs', snapshot.caveatRefs)
  assertSafeRefs('Artanis health latest tick ref', [
    snapshot.latestTickRef ?? 'tick.none',
  ])
  assertSafeRefs('Artanis health loop ref', [snapshot.loopRef])
  assertSafeRefs(
    'Artanis health operator recovery action refs',
    snapshot.operatorRecoveryActionRefs,
  )
  assertSafeRefs(
    'Artanis health overclaim blocker refs',
    snapshot.overclaimBlockerRefs,
  )
  assertSafeRefs(
    'Artanis health pending approval refs',
    snapshot.pendingApprovalRefs,
  )
  assertSafeRefs('Artanis health public status refs', snapshot.publicStatusRefs)
  assertSafeRefs('Artanis health runner backend refs', snapshot.runnerBackendRefs)
  assertSafeRefs('Artanis health snapshot ref', [snapshot.snapshotRef])
  assertSafeRefs('Artanis health source refs', snapshot.sourceRefs)

  if (snapshot.agentId !== 'agent_artanis') {
    throw new ArtanisHealthUnsafe({
      reason: 'Artanis health snapshots must use agent_artanis.',
    })
  }

  const kinds = new Set(snapshot.signals.map(signal => signal.kind))
  const missingKinds = ARTANIS_HEALTH_SIGNAL_KINDS.filter(kind => !kinds.has(kind))

  if (missingKinds.length > 0) {
    throw new ArtanisHealthUnsafe({
      reason: 'Artanis health snapshots must cover every required signal kind.',
    })
  }

  snapshot.signals.forEach(assertSignal)

  const attentionSignals = snapshot.signals.filter(signalNeedsAttention)

  if (
    attentionSignals.length > 0 &&
    (!snapshot.overclaimBlocked || !hasAny(snapshot.overclaimBlockerRefs))
  ) {
    throw new ArtanisHealthUnsafe({
      reason:
        'Stale or blocked Artanis health must block overclaiming with blocker refs.',
    })
  }

  if (
    attentionSignals.length === 0 &&
    (snapshot.overclaimBlocked || hasAny(snapshot.overclaimBlockerRefs))
  ) {
    throw new ArtanisHealthUnsafe({
      reason:
        'Healthy Artanis snapshots cannot claim overclaim blocking without stale or blocked signals.',
    })
  }

  if (snapshot.overallState === 'healthy' && attentionSignals.length > 0) {
    throw new ArtanisHealthUnsafe({
      reason:
        'Artanis health cannot be healthy while stale, blocked, missing, unavailable, degraded, or unknown signals exist.',
    })
  }

  if (snapshot.overallState === 'blocked' && !hasAny(snapshot.blockerRefs)) {
    throw new ArtanisHealthUnsafe({
      reason: 'Blocked Artanis health snapshots require blocker refs.',
    })
  }

  if (
    !hasAny(snapshot.operatorRecoveryActionRefs) &&
    snapshot.overclaimBlocked
  ) {
    throw new ArtanisHealthUnsafe({
      reason:
        'Overclaim-blocked Artanis health snapshots require operator recovery action refs.',
    })
  }
}

const projectSignal = (
  signal: ArtanisHealthSignalRecord,
  audience: ArtanisHealthAudience,
  nowIso: string,
): ArtanisHealthSignalProjection =>
  new ArtanisHealthSignalProjection({
    blockerRefs: refsForAudience(
      'Artanis health signal blocker refs',
      signal.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis health signal caveat refs',
      signal.caveatRefs,
      audience,
    ),
    count: signal.count,
    kind: signal.kind,
    label: signal.label,
    observedAtDisplay: friendlyBlueprintMissionBriefingTime(
      signal.observedAtIso,
      nowIso,
    ),
    operatorDetailRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis health signal operator detail refs',
          signal.operatorDetailRefs,
          audience,
        )
      : [],
    publicRecoveryActionRefs: refsForAudience(
      'Artanis health signal public recovery action refs',
      signal.publicRecoveryActionRefs,
      audience,
    ),
    publicStatusRefs: refsForAudience(
      'Artanis health signal public status refs',
      signal.publicStatusRefs,
      audience,
    ),
    signalRef: refsForAudience(
      'Artanis health signal ref',
      [signal.signalRef],
      audience,
    )[0] ?? 'health.redacted.artanis_signal',
    sourceRefs: refsForAudience(
      'Artanis health signal source refs',
      signal.sourceRefs,
      audience,
    ),
    state: signal.state,
    subjectUpdatedDisplay: signal.subjectUpdatedAtIso === null
      ? null
      : friendlyBlueprintMissionBriefingTime(signal.subjectUpdatedAtIso, nowIso),
  })

const projectionValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionValues)
  }

  return []
}

const allowedPublicLiteralValues = new Set<string>([
  ...ARTANIS_HEALTH_SIGNAL_KINDS,
  'available',
  'blocked',
  'degraded',
  'fresh',
  'healthy',
  'missing',
  'operator',
  'public_artanis',
  'public_forum',
  'stale',
  'unavailable',
  'unknown',
])

export const artanisHealthProjectionHasPrivateMaterial = (
  projection: ArtanisHealthSnapshotProjection,
): boolean =>
  projectionValues(projection).some(
    value =>
      !allowedPublicLiteralValues.has(value) &&
      (unsafeHealthPattern.test(value) ||
        rawTimestampPattern.test(value) ||
        publicUnsafeRefPattern.test(value)),
  )

export const projectArtanisHealthSnapshot = (
  snapshot: ArtanisHealthSnapshotRecord,
  audience: ArtanisHealthAudience,
  nowIso: string,
): ArtanisHealthSnapshotProjection => {
  assertSnapshot(snapshot)

  const signals = snapshot.signals.map(signal =>
    projectSignal(signal, audience, nowIso),
  )

  const projection = new ArtanisHealthSnapshotProjection({
    agentId: snapshot.agentId,
    audience,
    blockerRefs: refsForAudience(
      'Artanis health snapshot blocker refs',
      snapshot.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis health snapshot caveat refs',
      snapshot.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      snapshot.createdAtIso,
      nowIso,
    ),
    latestTickRef: snapshot.latestTickRef,
    loopRef: snapshot.loopRef,
    operatorRecoveryActionRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis health operator recovery action refs',
          snapshot.operatorRecoveryActionRefs,
          audience,
        )
      : [],
    overallState: snapshot.overallState,
    overclaimBlocked: snapshot.overclaimBlocked,
    overclaimBlockerRefs: refsForAudience(
      'Artanis health overclaim blocker refs',
      snapshot.overclaimBlockerRefs,
      audience,
    ),
    pendingApprovalCount: snapshot.pendingApprovalRefs.length,
    pendingApprovalRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis health pending approval refs',
          snapshot.pendingApprovalRefs,
          audience,
        )
      : [],
    publicStatusRefs: refsForAudience(
      'Artanis health public status refs',
      snapshot.publicStatusRefs,
      audience,
    ),
    runnerBackendRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis health runner backend refs',
          snapshot.runnerBackendRefs,
          audience,
        )
      : [],
    signals,
    snapshotRef: refsForAudience(
      'Artanis health snapshot ref',
      [snapshot.snapshotRef],
      audience,
    )[0] ?? 'health.redacted.artanis_snapshot',
    sourceRefs: refsForAudience(
      'Artanis health source refs',
      snapshot.sourceRefs,
      audience,
    ),
    staleOrBlockedSignalCount: signals.filter(signal =>
      attentionStates.has(signal.state),
    ).length,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      snapshot.updatedAtIso,
      nowIso,
    ),
  })

  if (
    audience !== 'operator' &&
    artanisHealthProjectionHasPrivateMaterial(projection)
  ) {
    throw new ArtanisHealthUnsafe({
      reason: 'Public Artanis health projection contains private material.',
    })
  }

  return projection
}

const signal = (
  input: Omit<ArtanisHealthSignalRecord, 'signalRef'> & {
    signalRefSuffix: string
  },
): ArtanisHealthSignalRecord => {
  const { signalRefSuffix, ...record } = input

  return new ArtanisHealthSignalRecord({
    ...record,
    signalRef: `health.public.artanis.${signalRefSuffix}`,
  })
}

const baseObservedAtIso = '2026-06-07T03:10:00.000Z'

export const exampleArtanisHealthSnapshot =
  new ArtanisHealthSnapshotRecord({
    agentId: 'agent_artanis',
    blockerRefs: ['blocker.public.artanis.model_lab_report_stale'],
    caveatRefs: ['caveat.public.artanis.health_blocks_overclaiming'],
    createdAtIso: baseObservedAtIso,
    latestTickRef: 'tick.public.artanis.20260607T0300',
    loopRef: 'loop.public.artanis.pylon_model_lab',
    operatorRecoveryActionRefs: [
      'recovery.operator.artanis.refresh_model_lab_report',
      'recovery.operator.artanis.inspect_publication_lag',
    ],
    overallState: 'stale',
    overclaimBlocked: true,
    overclaimBlockerRefs: ['overclaim.public.artanis.health_stale'],
    pendingApprovalRefs: ['approval.public.artanis.pylon_dispatch_pending'],
    publicStatusRefs: ['health.public.artanis.status.stale'],
    runnerBackendRefs: ['runner_backend.public.artanis.shc'],
    signals: [
      signal({
        blockerRefs: [],
        caveatRefs: ['caveat.public.loop_fresh'],
        count: 0,
        kind: 'loop_freshness',
        label: 'Loop is fresh',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.loop.last_claim'],
        publicRecoveryActionRefs: [],
        publicStatusRefs: ['health.public.artanis.loop_fresh'],
        signalRefSuffix: 'loop_freshness',
        sourceRefs: ['loop.public.artanis.pylon_model_lab'],
        state: 'fresh',
        subjectUpdatedAtIso: '2026-06-07T03:00:00.000Z',
      }),
      signal({
        blockerRefs: [],
        caveatRefs: ['caveat.public.last_tick_recorded'],
        count: 1,
        kind: 'last_tick',
        label: 'Last tick recorded',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.last_tick'],
        publicRecoveryActionRefs: [],
        publicStatusRefs: ['health.public.artanis.last_tick_seen'],
        signalRefSuffix: 'last_tick',
        sourceRefs: ['tick.public.artanis.20260607T0300'],
        state: 'fresh',
        subjectUpdatedAtIso: '2026-06-07T03:00:00.000Z',
      }),
      signal({
        blockerRefs: ['blocker.public.artanis.operator_approval_pending'],
        caveatRefs: ['caveat.public.approval_needed_before_dispatch'],
        count: 1,
        kind: 'pending_approvals',
        label: 'Approval pending',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.pending_approval_detail'],
        publicRecoveryActionRefs: ['recovery.public.artanis.wait_for_operator'],
        publicStatusRefs: ['health.public.artanis.approval_pending'],
        signalRefSuffix: 'pending_approvals',
        sourceRefs: ['approval.public.artanis.pylon_dispatch_pending'],
        state: 'blocked',
        subjectUpdatedAtIso: '2026-06-07T02:55:00.000Z',
      }),
      signal({
        blockerRefs: ['blocker.public.artanis.model_lab_report_stale'],
        caveatRefs: ['caveat.public.model_lab_report_needs_refresh'],
        count: 1,
        kind: 'blocker_reason',
        label: 'Blocked by stale Model Lab report',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.blocker_detail'],
        publicRecoveryActionRefs: [
          'recovery.public.artanis.refresh_model_lab_summary',
        ],
        publicStatusRefs: ['health.public.artanis.blocked_model_lab_stale'],
        signalRefSuffix: 'blocker_reason',
        sourceRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
        state: 'blocked',
        subjectUpdatedAtIso: '2026-06-07T01:20:00.000Z',
      }),
      signal({
        blockerRefs: [],
        caveatRefs: ['caveat.public.publication_lag_attention'],
        count: 1,
        kind: 'forum_publication_lag',
        label: 'Forum publication lag needs review',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.forum_publication_lag'],
        publicRecoveryActionRefs: [
          'recovery.public.artanis.inspect_forum_publication_queue',
        ],
        publicStatusRefs: ['health.public.artanis.forum_lag_attention'],
        signalRefSuffix: 'forum_publication_lag',
        sourceRefs: ['forum.public.artanis.status'],
        state: 'degraded',
        subjectUpdatedAtIso: '2026-06-07T02:15:00.000Z',
      }),
      signal({
        blockerRefs: [],
        caveatRefs: ['caveat.public.pylon_stats_fresh'],
        count: 0,
        kind: 'pylon_stats_freshness',
        label: 'Pylon stats are fresh',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.pylon_stats'],
        publicRecoveryActionRefs: [],
        publicStatusRefs: ['health.public.artanis.pylon_stats_fresh'],
        signalRefSuffix: 'pylon_stats_freshness',
        sourceRefs: ['pylon.public.stats'],
        state: 'fresh',
        subjectUpdatedAtIso: '2026-06-07T03:07:00.000Z',
      }),
      signal({
        blockerRefs: [],
        caveatRefs: ['caveat.public.omega_pylon_stats_fresh'],
        count: 0,
        kind: 'nexus_public_stats_freshness',
        label: 'Omega public Pylon stats are fresh',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.omega_pylon_stats'],
        publicRecoveryActionRefs: [],
        publicStatusRefs: ['health.public.artanis.omega_pylon_stats_fresh'],
        signalRefSuffix: 'nexus_public_stats_freshness',
        sourceRefs: ['omega.public.pylon_api.registrations'],
        state: 'fresh',
        subjectUpdatedAtIso: '2026-06-07T03:07:00.000Z',
      }),
      signal({
        blockerRefs: [],
        caveatRefs: [
          'authority.public.khala_readiness.credentialless_read_only',
          'authority.public.khala_readiness.no_chat_call',
          'authority.public.khala_readiness.no_mutation',
          'authority.public.khala_readiness.no_paid_call',
          'caveat.public.khala_public_catalog_single_model',
        ],
        count: 0,
        kind: 'khala_readiness',
        label: 'Khala no-spend readiness is clean',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.khala_readiness'],
        publicRecoveryActionRefs: [],
        publicStatusRefs: ['health.public.artanis.khala_ready'],
        signalRefSuffix: 'khala_readiness',
        sourceRefs: [
          'gateway.public.openagents.models',
          'gateway.public.openagents.readiness',
          'model.public.openagents.khala',
        ],
        state: 'available',
        subjectUpdatedAtIso: '2026-06-07T03:09:00.000Z',
      }),
      signal({
        blockerRefs: ['blocker.public.artanis.fleet_overseer_not_armed'],
        caveatRefs: [
          'authority.public.artanis.fleet_overseer.read_only_signal',
          'caveat.public.artanis.fleet_overseer_default_off',
        ],
        count: 0,
        kind: 'fleet_overseer',
        label: 'Fleet overseer not armed',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.fleet_overseer'],
        publicRecoveryActionRefs: [
          'recovery.public.artanis.complete_fleet_overseer_live_proof',
        ],
        publicStatusRefs: ['health.public.artanis.fleet_overseer_blocked'],
        signalRefSuffix: 'fleet_overseer',
        sourceRefs: ['tick.public.artanis.fleet_overseer'],
        state: 'blocked',
        subjectUpdatedAtIso: '2026-06-07T03:09:00.000Z',
      }),
      signal({
        blockerRefs: ['blocker.public.artanis.model_lab_report_stale'],
        caveatRefs: ['caveat.public.model_lab_report_stale'],
        count: 1,
        kind: 'model_lab_report_freshness',
        label: 'Model Lab report is stale',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.model_lab_report'],
        publicRecoveryActionRefs: [
          'recovery.public.artanis.refresh_model_lab_summary',
        ],
        publicStatusRefs: ['health.public.artanis.model_lab_report_stale'],
        signalRefSuffix: 'model_lab_report_freshness',
        sourceRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
        state: 'stale',
        subjectUpdatedAtIso: '2026-06-07T01:20:00.000Z',
      }),
      signal({
        blockerRefs: [],
        caveatRefs: ['caveat.public.runner_backend_available'],
        count: 1,
        kind: 'runner_backend_availability',
        label: 'Runner backend available',
        observedAtIso: baseObservedAtIso,
        operatorDetailRefs: ['health.operator.artanis.runner_backend'],
        publicRecoveryActionRefs: [],
        publicStatusRefs: ['health.public.artanis.runner_backend_available'],
        signalRefSuffix: 'runner_backend_availability',
        sourceRefs: ['runner_backend.public.artanis.shc'],
        state: 'available',
        subjectUpdatedAtIso: '2026-06-07T03:09:00.000Z',
      }),
    ],
    snapshotRef: 'health.public.artanis.snapshot.20260607T0310',
    sourceRefs: [
      'loop.public.artanis.pylon_model_lab',
      'forum.public.artanis.status',
      'model_lab.public.report.autopilot_benchmark_loop',
      'pylon.public.stats',
      'nexus.public.stats',
    ],
    updatedAtIso: baseObservedAtIso,
  })
