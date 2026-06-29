import {
  assertPublicActivityTimelineEnvelopeSafe,
  orderPublicActivityTimelineEvents,
  type PublicActivityTimelineEnvelope,
  type PublicActivityTimelineEvent,
} from '@openagentsinc/public-activity-timeline'

export type ReplayVector3 = Readonly<{
  x: number
  y: number
  z: number
}>

export type ReplayEventKind =
  | 'actor_entered_region'
  | 'actor_moved'
  | 'actor_focused_pylon'
  | 'actor_said_public_message'
  | 'proof_submitted'
  | 'proof_verified'
  | 'proof_rejected'
  | 'trace_linked'
  | 'receipt_recorded'
  | 'settlement_blocked_closed'
  | 'payout_intent_persisted'
  | 'settlement_recorded'
  | 'payment_zap_confirmed'
  | 'payment_zap_simulated'
  | 'recognition_reward_recorded'
  | 'recipient_confirmation_recorded'
  | 'overpayment_detected'
  | 'artifact_opened'
  | 'forum_announcement_posted'
  | 'claim_boundary_shown'

export type ReplayCameraMode =
  | 'overview'
  | 'follow_actor'
  | 'orbit_proof'
  | 'zap_focus'
  | 'free_camera'
  | 'director_track'

export type ReplaySourceRef = Readonly<{
  ref: string
  kind?: string
  url?: string
  observedAt?: string
}>

export type ReplayActor = Readonly<{
  actorRef: string
  avatarRole: string
  displayName: string
  pylonRef?: string
  fallbackAssetId?: string
}>

export type ReplayStage = Readonly<{
  stageRef: string
  stageKind: string
  label: string
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayEvent = Readonly<{
  eventRef: string
  kind: ReplayEventKind
  sequenceIndex: number
  timelineSecond: number
  observedAt?: string
  actorRefs: ReadonlyArray<string>
  targetRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  displayText: string
  stateBefore?: string
  stateAfter?: string
  amountSats?: number
  rail?: string
  caveat?: string
}>

export type ReplayFlow = Readonly<{
  flowRef: string
  flowKind: string
  fromRef: string
  toRef: string
  sourceRefs: ReadonlyArray<string>
  amountSats?: number
  rail?: string
}>

export type ReplayCameraCue = Readonly<{
  cueRef: string
  mode: ReplayCameraMode
  startSecond: number
  durationSecond: number
  focusRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayCaption = Readonly<{
  captionRef: string
  sequenceIndex: number
  timelineSecond: number
  text: string
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayGap = Readonly<{
  gapRef: string
  reason: string
  affectedRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type ProofReplayBundle = Readonly<{
  bundleRef: string
  schemaVersion: 'proof_replay_bundle.v1'
  generatedAt: string
  title: string
  socialDisplayTime?: string
  sourceRefs: ReadonlyArray<ReplaySourceRef>
  sourceAuthority: string
  privacyLevel: 'public_safe'
  claimScope: 'evidence_presentation_only'
  actors: ReadonlyArray<ReplayActor>
  stages: ReadonlyArray<ReplayStage>
  events: ReadonlyArray<ReplayEvent>
  flows: ReadonlyArray<ReplayFlow>
  cameraCues: ReadonlyArray<ReplayCameraCue>
  captions: ReadonlyArray<ReplayCaption>
  gaps: ReadonlyArray<ReplayGap>
}>

export const OPENAGENTS_PUBLIC_ORIGIN = 'https://openagents.com'
export const FIRST_REAL_SETTLEMENT_REPLAY_SLUG = 'first-real-settlement'
export const LAUNCH_RECOGNITION_REPLAY_SLUG = 'launch-recognition-payments'
export const TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT =
  '/api/public/tassadar-replays/first-real-settlement'

export type ProofReplayCatalogSlug =
  | typeof FIRST_REAL_SETTLEMENT_REPLAY_SLUG
  | typeof LAUNCH_RECOGNITION_REPLAY_SLUG

export type ProofReplayCatalogEntry = Readonly<{
  slug: ProofReplayCatalogSlug
  title: string
  summary: string
  bundleEndpoint: string
  websitePath: string
  socialPath?: string
  primarySourceRefs: ReadonlyArray<string>
}>

const proofReplayCatalogEntries: ReadonlyArray<ProofReplayCatalogEntry> = [
  {
    bundleEndpoint: TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT,
    primarySourceRefs: [
      'run.tassadar.executor.20260615',
      'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c',
      'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
    ],
    slug: FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
    socialPath:
      '/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social',
    summary:
      'Tassadar Run 1 proof replay with failed-closed settlement blockers, owner gate, and the first receipt-backed 1,000-sat Spark zap.',
    title: 'Tassadar Run 1: First Real Bitcoin Settlement',
    websitePath: '/tassadar/replay/first-real-settlement',
  },
  {
    bundleEndpoint: `/api/public/proof-replays?ref=${encodeURIComponent(
      LAUNCH_RECOGNITION_REPLAY_SLUG,
    )}`,
    primarySourceRefs: [
      'docs/launch/JUNE17_ROADMAP.md',
      'docs/payments/2026-06-17-launch-recognition-closeout.md',
      'docs/payments/2026-06-17-launch-recognition-spark-recipient-status.md',
    ],
    slug: LAUNCH_RECOGNITION_REPLAY_SLUG,
    summary:
      'Launch recognition replay for Trigger, Whitefang, and Orrery with intended 50,000-sat lanes, confirmed public receipts, gaps, and overpayment accounting.',
    title: 'Launch Recognition Payment Replay',
    websitePath: '/tassadar/replay/launch-recognition-payments',
  },
]

export type ReplayPlaybackState = Readonly<{
  durationSecond: number
  isPlaying: boolean
  playbackRate: number
  second: number
}>

export type ReplayClockCommand =
  | Readonly<{ type: 'play' }>
  | Readonly<{ type: 'pause' }>
  | Readonly<{ type: 'reset' }>
  | Readonly<{ type: 'seek'; second: number }>
  | Readonly<{ type: 'set_speed'; playbackRate: number }>
  | Readonly<{ type: 'tick'; deltaSecond: number }>

export type ReplayPaymentVisualKind =
  | 'confirmed_zap'
  | 'simulation_path'
  | 'blocked_marker'
  | 'recognition_marker'
  | 'neutral_event'

export type ReplayPaymentVisual = Readonly<{
  eventRef: string
  kind: ReplayPaymentVisualKind
  amountSats?: number
  rail?: string
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayHitTargetKind =
  | 'actor'
  | 'stage'
  | 'event'
  | 'payment'
  | 'caption'
  | 'gap'

export type ReplayHitTarget = Readonly<{
  targetRef: string
  kind: ReplayHitTargetKind
  label: string
  sourceRefs: ReadonlyArray<string>
  inspectable: boolean
}>

export type ReplayActorKeyframe = Readonly<{
  actorRef: string
  eventRef: string
  second: number
  position: ReplayVector3
  state: 'blocked' | 'idle' | 'inspect' | 'settle' | 'talk' | 'verify' | 'walk'
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayActorTrack = Readonly<{
  actorRef: string
  keyframes: ReadonlyArray<ReplayActorKeyframe>
}>

export type ReplayCameraPose = Readonly<{
  cameraRef: string
  mode: ReplayCameraMode
  second: number
  position: ReplayVector3
  target: ReplayVector3
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayStagePlacement = Readonly<{
  ref: string
  label: string
  kind: string
  position: ReplayVector3
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayRenderPlan = Readonly<{
  bundleRef: string
  durationSecond: number
  orderedEvents: ReadonlyArray<ReplayEvent>
  stagePlacements: ReadonlyArray<ReplayStagePlacement>
  actorTracks: ReadonlyArray<ReplayActorTrack>
  paymentVisuals: ReadonlyArray<ReplayPaymentVisual>
  hitTargets: ReadonlyArray<ReplayHitTarget>
  cameraCues: ReadonlyArray<ReplayCameraCue>
  captions: ReadonlyArray<ReplayCaption>
}>

export type ReplayBundleShipmentGateSubject =
  | 'bundle'
  | 'caption'
  | 'event'
  | 'flow'
  | 'gap'
  | 'payment'
  | 'privacy'
  | 'source'

export class ReplayBundleShipmentGateError extends Error {
  readonly subject: ReplayBundleShipmentGateSubject

  constructor(subject: ReplayBundleShipmentGateSubject, message: string) {
    super(message)
    this.name = 'ReplayBundleShipmentGateError'
    this.subject = subject
  }
}

export type ReplayDisposable = Readonly<{
  dispose: () => void
}>

export type ReplayDisposalRegistry = Readonly<{
  add: (disposable: ReplayDisposable) => void
  disposeAll: () => void
  disposedCount: () => number
  pendingCount: () => number
}>

export type GeneratedProofReplayBundleOptions = Readonly<{
  bundleRef?: string
  generatedAt?: string
  origin?: string
  sourceAuthority?: string
  title?: string
}>

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values.map(value => value.trim()).filter(value => value !== ''))]

const stableHash = (value: string): string => {
  let hash = 2166136261
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(16).padStart(8, '0')
}

const originPrefix = (origin: string | undefined): string => {
  const value = origin?.trim()
  return value === undefined || value === '' ? '' : value.replace(/\/+$/, '')
}

const withOrigin = (path: string, origin?: string): string => {
  if (/^https?:\/\//i.test(path)) return path
  return `${originPrefix(origin)}${path}`
}

export const proofReplayCatalog = (
  origin?: string,
): ReadonlyArray<ProofReplayCatalogEntry> =>
  proofReplayCatalogEntries.map(entry => {
    const base = {
      ...entry,
      bundleEndpoint: withOrigin(entry.bundleEndpoint, origin),
      websitePath: withOrigin(entry.websitePath, origin),
    }
    return entry.socialPath === undefined
      ? base
      : { ...base, socialPath: withOrigin(entry.socialPath, origin) }
  })

export const proofReplayCatalogEntryForSlug = (
  slug: string,
  origin?: string,
): ProofReplayCatalogEntry | undefined =>
  proofReplayCatalog(origin).find(entry => entry.slug === slug)

export const proofReplayBundleEndpointForSlug = (
  slug: string,
  origin?: string,
): string =>
  slug === FIRST_REAL_SETTLEMENT_REPLAY_SLUG
    ? withOrigin(TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT, origin)
    : withOrigin(
        `/api/public/proof-replays?ref=${encodeURIComponent(slug)}`,
        origin,
      )

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '.')
    .replace(/^\.+|\.+$/g, '')
    .slice(0, 96) || 'unknown'

const sourceRefKind = (ref: string): string => {
  if (ref.startsWith('receipt.') || ref.includes('/receipts/')) return 'receipt'
  if (ref.startsWith('training.verification.challenge.')) return 'challenge'
  if (ref.startsWith('run.') || ref.startsWith('training.run.')) return 'run'
  if (ref.startsWith('window.') || ref.startsWith('training.window.')) return 'window'
  if (ref.startsWith('pylon.')) return 'pylon'
  if (ref.startsWith('forum.') || ref.includes('/forum/')) return 'forum'
  if (ref.startsWith('blocker.')) return 'blocker'
  if (ref.startsWith('caveat.')) return 'caveat'
  return 'public_activity_source'
}

const sourceRefUrl = (ref: string, origin?: string): string | undefined => {
  if (/^https?:\/\//i.test(ref)) return ref
  const base = originPrefix(origin)
  if (base === '') return undefined
  if (ref.startsWith('receipt.')) {
    return `${base}/api/public/nexus-pylon/receipts/${encodeURIComponent(ref)}`
  }
  if (ref.startsWith('training.verification.challenge.')) {
    return `${base}/api/public/training/verification-challenges/${encodeURIComponent(ref)}`
  }
  if (ref.startsWith('training.run.') || ref.startsWith('run.')) {
    return `${base}/api/public/tassadar-run-summary?run=${encodeURIComponent(ref)}`
  }
  return undefined
}

const replaySourceRefsFrom = (
  refs: ReadonlyArray<string>,
  origin?: string,
): ReadonlyArray<ReplaySourceRef> =>
  unique(refs).map(ref => {
    const url = sourceRefUrl(ref, origin)
    return {
      kind: sourceRefKind(ref),
      ref,
      ...(url === undefined ? {} : { url }),
    }
  })

const eventEvidenceRefs = (
  event: Pick<
    PublicActivityTimelineEvent,
    'blockerRefs' | 'caveatRefs' | 'refs' | 'sourceRefs'
  >,
): ReadonlyArray<string> =>
  unique([
    ...event.sourceRefs,
    ...event.blockerRefs,
    ...event.caveatRefs,
    ...event.refs,
  ])

const primaryEventSourceRefs = (
  event: Pick<PublicActivityTimelineEvent, 'blockerRefs' | 'sourceRefs'>,
): ReadonlyArray<string> => {
  const sourceRefs = unique(event.sourceRefs)
  return sourceRefs.length > 0 ? sourceRefs : unique(event.blockerRefs)
}

const actorRefForTimelineEvent = (event: PublicActivityTimelineEvent): string => {
  if (event.actorRef !== undefined && event.actorRef.trim() !== '') {
    return `actor.timeline.${safeRefSegment(event.actorRef)}`
  }
  if (
    event.kind === 'settlement_recorded' ||
    event.kind === 'real_bitcoin_moved'
  ) {
    return 'actor.openagents.settlement'
  }
  if (
    event.kind === 'verification_queued' ||
    event.kind === 'verification_verified' ||
    event.kind === 'verification_rejected' ||
    event.kind === 'trace_submitted'
  ) {
    return 'actor.openagents.verifier'
  }
  if (event.kind === 'forum_topic_created' || event.kind === 'forum_posted') {
    return 'actor.openagents.forum'
  }
  if (event.kind === 'khala_inference_served') {
    return 'actor.openagents.khala_gateway'
  }
  if (event.kind === 'artanis_tick') return 'actor.openagents.artanis'
  return 'actor.openagents.network'
}

const actorDisplayName = (actorRef: string): string => {
  if (actorRef === 'actor.openagents.settlement') return 'Settlement'
  if (actorRef === 'actor.openagents.verifier') return 'Verifier'
  if (actorRef === 'actor.openagents.forum') return 'Forum'
  if (actorRef === 'actor.openagents.khala_gateway') return 'Khala Gateway'
  if (actorRef === 'actor.openagents.artanis') return 'Artanis'
  if (actorRef === 'actor.openagents.network') return 'OpenAgents Network'
  return actorRef.replace(/^actor\.timeline\./, '').replace(/[._-]+/g, ' ')
}

const stageForTimelineEvent = (
  event: Pick<PublicActivityTimelineEvent, 'kind' | 'runRef' | 'sourceKind'>,
): ReplayStage => {
  const sourceRefs = unique([
    ...(event.runRef === undefined ? [] : [event.runRef]),
    event.sourceKind,
  ])
  if (
    event.kind === 'verification_queued' ||
    event.kind === 'verification_verified' ||
    event.kind === 'verification_rejected' ||
    event.kind === 'trace_submitted' ||
    event.kind === 'work_claimed'
  ) {
    return {
      label: 'Proof gate',
      sourceRefs,
      stageKind: 'proof_gate',
      stageRef: 'stage.timeline.proof',
    }
  }
  if (event.kind === 'settlement_recorded' || event.kind === 'real_bitcoin_moved') {
    return {
      label: 'Settlement terminal',
      sourceRefs,
      stageKind: 'settlement_terminal',
      stageRef: 'stage.timeline.settlement',
    }
  }
  if (event.kind === 'forum_topic_created' || event.kind === 'forum_posted') {
    return {
      label: 'Forum',
      sourceRefs,
      stageKind: 'forum_surface',
      stageRef: 'stage.timeline.forum',
    }
  }
  if (event.kind === 'khala_inference_served') {
    return {
      label: 'Khala inference',
      sourceRefs,
      stageKind: 'inference_gateway',
      stageRef: 'stage.timeline.khala_inference',
    }
  }
  if (event.kind === 'projection_gap') {
    return {
      label: 'Projection gaps',
      sourceRefs,
      stageKind: 'replay_gap',
      stageRef: 'stage.timeline.gaps',
    }
  }
  if (event.kind === 'capacity_snapshot') {
    return {
      label: 'Capacity',
      sourceRefs,
      stageKind: 'capacity_surface',
      stageRef: 'stage.timeline.capacity',
    }
  }
  return {
    label: 'Fleet',
    sourceRefs,
    stageKind: 'pylon_station',
    stageRef: 'stage.timeline.fleet',
  }
}

const replayKindForTimelineEvent = (
  event: PublicActivityTimelineEvent,
): ReplayEventKind | null => {
  switch (event.kind) {
    case 'pylon_registered':
      return 'actor_entered_region'
    case 'pylon_heartbeat':
      return 'actor_moved'
    case 'wallet_ready':
    case 'assignment_ready':
    case 'work_claimed':
      return 'actor_focused_pylon'
    case 'window_opened':
    case 'window_closed':
    case 'artanis_tick':
      return 'actor_said_public_message'
    case 'trace_submitted':
      return 'trace_linked'
    case 'verification_queued':
      return 'proof_submitted'
    case 'verification_verified':
      return 'proof_verified'
    case 'verification_rejected':
      return 'proof_rejected'
    case 'settlement_recorded':
      return 'settlement_recorded'
    case 'real_bitcoin_moved':
      return 'payment_zap_confirmed'
    case 'khala_inference_served':
      return 'receipt_recorded'
    case 'forum_topic_created':
    case 'forum_posted':
      return 'forum_announcement_posted'
    case 'capacity_snapshot':
      return 'artifact_opened'
    case 'projection_gap':
      return null
  }
}

const cameraModeForReplayEvent = (kind: ReplayEventKind): ReplayCameraMode => {
  if (kind === 'payment_zap_confirmed' || kind === 'settlement_recorded') {
    return 'zap_focus'
  }
  if (
    kind === 'proof_submitted' ||
    kind === 'proof_verified' ||
    kind === 'proof_rejected' ||
    kind === 'trace_linked'
  ) {
    return 'orbit_proof'
  }
  return 'overview'
}

const replayEventForTimelineEvent = (
  event: PublicActivityTimelineEvent,
  sequenceIndex: number,
): ReplayEvent | null => {
  const kind = replayKindForTimelineEvent(event)
  if (kind === null) return null
  const stage = stageForTimelineEvent(event)
  const sourceRefs = primaryEventSourceRefs(event)
  return {
    actorRefs: [actorRefForTimelineEvent(event)],
    displayText: event.text,
    eventRef: `replay.${safeRefSegment(event.eventRef)}`,
    kind,
    sequenceIndex,
    sourceRefs,
    targetRefs: unique([
      stage.stageRef,
      ...(event.targetRef === undefined
        ? []
        : [`target.timeline.${safeRefSegment(event.targetRef)}`]),
    ]),
    timelineSecond: sequenceIndex * 6,
    observedAt: event.ts,
    ...(event.amountSats === undefined ? {} : { amountSats: event.amountSats }),
    ...(event.realBitcoinMoved === true ? { rail: 'spark_treasury' } : {}),
    ...(event.caveatRefs.length === 0 && event.blockerRefs.length === 0
      ? {}
      : { caveat: unique([...event.caveatRefs, ...event.blockerRefs]).join(', ') }),
    ...(event.state === undefined ? {} : { stateAfter: event.state }),
  }
}

const replayGapForTimelineEvent = (
  event: PublicActivityTimelineEvent,
  sequenceIndex: number,
): ReplayGap | null => {
  if (event.kind !== 'projection_gap') return null
  const sourceRefs = primaryEventSourceRefs(event)
  return {
    affectedRefs: unique([event.eventRef, ...event.refs]),
    gapRef: `gap.${safeRefSegment(event.eventRef)}`,
    reason: event.text || event.state || 'Public activity projection gap',
    sourceRefs,
  }
}

const replayGapForSourceLag = (
  lag: PublicActivityTimelineEnvelope['sourceLag'][number],
  index: number,
): ReplayGap | null => {
  if (lag.status === 'current') return null
  const sourceRefs = unique([...lag.sourceRefs, ...lag.blockerRefs, ...lag.caveatRefs])
  return {
    affectedRefs: unique([lag.sourceKind, ...sourceRefs]),
    gapRef: `gap.source_lag.${index}.${lag.sourceKind}`,
    reason: `Public activity source ${lag.sourceKind} is ${lag.status}`,
    sourceRefs,
  }
}

const captionForReplayEvent = (event: ReplayEvent): ReplayCaption => ({
  captionRef: `caption.${event.eventRef}`,
  sequenceIndex: event.sequenceIndex,
  sourceRefs: event.sourceRefs,
  text: event.displayText,
  timelineSecond: event.timelineSecond,
})

const cameraCueForReplayEvent = (event: ReplayEvent): ReplayCameraCue => ({
  cueRef: `cue.${event.eventRef}`,
  durationSecond: 6,
  focusRefs: event.targetRefs,
  mode: cameraModeForReplayEvent(event.kind),
  sourceRefs: event.sourceRefs,
  startSecond: event.timelineSecond,
})

const flowForReplayEvent = (event: ReplayEvent): ReplayFlow | null => {
  if (event.kind === 'payment_zap_confirmed') {
    return {
      flowKind: 'payment_movement',
      flowRef: `flow.${event.eventRef}`,
      fromRef: 'actor.openagents.settlement',
      sourceRefs: event.sourceRefs,
      toRef: event.actorRefs[0] ?? 'actor.openagents.network',
      ...(event.amountSats === undefined ? {} : { amountSats: event.amountSats }),
      ...(event.rail === undefined ? {} : { rail: event.rail }),
    }
  }

  if (
    event.kind === 'actor_entered_region' ||
    event.kind === 'actor_moved' ||
    event.kind === 'actor_focused_pylon'
  ) {
    return {
      flowKind: 'fleet_readiness_track',
      flowRef: `flow.${event.eventRef}`,
      fromRef: event.actorRefs[0] ?? 'actor.openagents.network',
      sourceRefs: event.sourceRefs,
      toRef: 'stage.timeline.fleet',
    }
  }

  if (event.kind === 'forum_announcement_posted') {
    return {
      flowKind: 'discussion_track',
      flowRef: `flow.${event.eventRef}`,
      fromRef: event.actorRefs[0] ?? 'actor.openagents.forum',
      sourceRefs: event.sourceRefs,
      toRef: 'stage.timeline.forum',
    }
  }

  if (event.kind === 'artifact_opened') {
    return {
      flowKind: 'capacity_snapshot_track',
      flowRef: `flow.${event.eventRef}`,
      fromRef: event.actorRefs[0] ?? 'actor.openagents.network',
      sourceRefs: event.sourceRefs,
      toRef: 'stage.timeline.capacity',
    }
  }

  return null
}

export const buildProofReplayBundleFromPublicActivityTimeline = (
  input: PublicActivityTimelineEnvelope | unknown,
  options: GeneratedProofReplayBundleOptions = {},
): ProofReplayBundle => {
  const envelope = assertPublicActivityTimelineEnvelopeSafe(input)
  const timelineEvents = orderPublicActivityTimelineEvents(envelope.events)
  const replayEvents = timelineEvents
    .map((event, index) => replayEventForTimelineEvent(event, index))
    .filter((event): event is ReplayEvent => event !== null)
  const projectionGaps = timelineEvents
    .map((event, index) => replayGapForTimelineEvent(event, index))
    .filter((gap): gap is ReplayGap => gap !== null)
  const sourceLagGaps = envelope.sourceLag
    .map(replayGapForSourceLag)
    .filter((gap): gap is ReplayGap => gap !== null)
  const stageMap = new Map<string, ReplayStage>()
  stageMap.set('stage.timeline', {
    label: 'Public activity timeline',
    sourceRefs: unique(timelineEvents.flatMap(eventEvidenceRefs)),
    stageKind: 'run_core',
    stageRef: 'stage.timeline',
  })
  for (const event of timelineEvents) {
    const stage = stageForTimelineEvent(event)
    const existing = stageMap.get(stage.stageRef)
    stageMap.set(
      stage.stageRef,
      existing === undefined
        ? { ...stage, sourceRefs: unique([...stage.sourceRefs, ...eventEvidenceRefs(event)]) }
        : {
            ...existing,
            sourceRefs: unique([
              ...existing.sourceRefs,
              ...stage.sourceRefs,
              ...eventEvidenceRefs(event),
            ]),
          },
    )
  }

  const actorRefs = unique(replayEvents.flatMap(event => event.actorRefs))
  const sourceRefs = unique([
    ...timelineEvents.flatMap(eventEvidenceRefs),
    ...envelope.sourceLag.flatMap(lag => [
      ...lag.sourceRefs,
      ...lag.blockerRefs,
      ...lag.caveatRefs,
    ]),
  ])
  const generatedAt = options.generatedAt ?? envelope.generatedAt
  const bundle: ProofReplayBundle = {
    actors: actorRefs.map(actorRef => ({
      actorRef,
      avatarRole: actorRef.replace(/^actor\./, ''),
      displayName: actorDisplayName(actorRef),
      fallbackAssetId: 'procedural.activity',
    })),
    bundleRef:
      options.bundleRef ??
      `proof_replay_bundle.public_activity.${stableHash(
        JSON.stringify({
          cursors: timelineEvents.map(event => event.cursor),
          generatedAt,
        }),
      )}`,
    cameraCues: replayEvents.map(cameraCueForReplayEvent),
    captions: replayEvents.map(captionForReplayEvent),
    claimScope: 'evidence_presentation_only',
    events: replayEvents,
    flows: replayEvents
      .map(flowForReplayEvent)
      .filter((flow): flow is ReplayFlow => flow !== null),
    gaps: [...projectionGaps, ...sourceLagGaps],
    generatedAt,
    privacyLevel: 'public_safe',
    schemaVersion: 'proof_replay_bundle.v1',
    sourceAuthority: options.sourceAuthority ?? 'public_activity_timeline',
    sourceRefs: replaySourceRefsFrom(sourceRefs, options.origin),
    stages: [...stageMap.values()].filter(stage => stage.sourceRefs.length > 0),
    title: options.title ?? 'Public Activity Timeline Replay',
  }

  assertProofReplayBundleShipmentGate(bundle)
  return bundle
}

const unsafeReplayMaterialPatterns = [
  /\b(?:lnbc|lntb|lnbcrt|lno1)[a-z0-9]{12,}/i,
  /\bspark1[a-z0-9]{12,}/i,
  /\bbc1[ac-hj-np-z02-9]{20,}/i,
  /\bxprv[a-z0-9]{12,}/i,
  /\bmnemonic\b/i,
  /\bpreimage\b/i,
  /\bpayment[_-]?hash\b/i,
  /\bservice[_-]?token\b/i,
  /\bbearer\s+[a-z0-9._-]{12,}/i,
  /\bprovider[_-]?payload\b/i,
  /\braw[_-]?prompt\b/i,
  /\bprivate[_-]?log\b/i,
  /\bwallet[_-]?path\b/i,
  /\bcustomer[_-]?(?:data|email|record)\b/i,
  /\bbolt11\b/i,
]

const hasUnsafeReplayMaterial = (value: unknown): boolean => {
  const serialized = JSON.stringify(value)
  return unsafeReplayMaterialPatterns.some(pattern => pattern.test(serialized))
}

const hasSourceRefs = (refs: ReadonlyArray<string>): boolean =>
  refs.some(ref => ref.trim() !== '')

const hasConfirmedPaymentEvidence = (
  refs: ReadonlyArray<string>,
): boolean =>
  refs.some(
    ref =>
      ref.startsWith('receipt.') ||
      ref.includes('/api/public/nexus-pylon/receipts/') ||
      ref.startsWith('recipient_confirmation.') ||
      ref.startsWith(
        'https://github.com/OpenAgentsInc/openagents/blob/main/docs/',
      ),
  )

const add = (a: ReplayVector3, b: ReplayVector3): ReplayVector3 => ({
  x: a.x + b.x,
  y: a.y + b.y,
  z: a.z + b.z,
})

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t

const lerpVector = (
  a: ReplayVector3,
  b: ReplayVector3,
  t: number,
): ReplayVector3 => ({
  x: lerp(a.x, b.x, t),
  y: lerp(a.y, b.y, t),
  z: lerp(a.z, b.z, t),
})

const origin: ReplayVector3 = { x: 0, y: 0, z: 0 }

export const orderedReplayEvents = (
  bundle: Pick<ProofReplayBundle, 'events'>,
): ReadonlyArray<ReplayEvent> =>
  [...bundle.events].sort(
    (a, b) =>
      a.sequenceIndex - b.sequenceIndex ||
      a.timelineSecond - b.timelineSecond ||
      a.eventRef.localeCompare(b.eventRef),
  )

export const replayDurationSecond = (
  bundle: Pick<ProofReplayBundle, 'cameraCues' | 'captions' | 'events'>,
): number => {
  const eventEnd = bundle.events.reduce(
    (max, event) => Math.max(max, event.timelineSecond),
    0,
  )
  const captionEnd = bundle.captions.reduce(
    (max, caption) => Math.max(max, caption.timelineSecond),
    0,
  )
  const cameraEnd = bundle.cameraCues.reduce(
    (max, cue) => Math.max(max, cue.startSecond + cue.durationSecond),
    0,
  )

  return Math.max(1, Math.ceil(Math.max(eventEnd, captionEnd, cameraEnd)))
}

export const initialReplayPlaybackState = (
  bundle: Pick<ProofReplayBundle, 'cameraCues' | 'captions' | 'events'>,
): ReplayPlaybackState => ({
  durationSecond: replayDurationSecond(bundle),
  isPlaying: false,
  playbackRate: 1,
  second: 0,
})

export const reduceReplayClock = (
  state: ReplayPlaybackState,
  command: ReplayClockCommand,
): ReplayPlaybackState => {
  if (command.type === 'play') {
    return { ...state, isPlaying: true }
  }

  if (command.type === 'pause') {
    return { ...state, isPlaying: false }
  }

  if (command.type === 'reset') {
    return { ...state, isPlaying: false, second: 0 }
  }

  if (command.type === 'seek') {
    return {
      ...state,
      second: clamp(command.second, 0, state.durationSecond),
    }
  }

  if (command.type === 'set_speed') {
    return {
      ...state,
      playbackRate: clamp(command.playbackRate, 0.125, 8),
    }
  }

  if (!state.isPlaying) {
    return state
  }

  return {
    ...state,
    second: clamp(
      state.second + Math.max(0, command.deltaSecond) * state.playbackRate,
      0,
      state.durationSecond,
    ),
  }
}

export const activeReplayEventsAt = (
  bundle: Pick<ProofReplayBundle, 'events'>,
  second: number,
): ReadonlyArray<ReplayEvent> =>
  orderedReplayEvents(bundle).filter(event => event.timelineSecond <= second)

const stagePlacementFor = (
  stage: ReplayStage,
  index: number,
  total: number,
): ReplayStagePlacement => {
  if (stage.stageKind === 'run_core') {
    return {
      kind: stage.stageKind,
      label: stage.label,
      position: origin,
      ref: stage.stageRef,
      sourceRefs: stage.sourceRefs,
    }
  }

  const ringIndex = Math.max(0, index - 1)
  const ringTotal = Math.max(1, total - 1)
  const angle = (ringIndex / ringTotal) * Math.PI * 2 - Math.PI / 2
  const radius =
    stage.stageKind === 'settlement_terminal'
      ? 5.5
      : stage.stageKind === 'proof_gate'
        ? 4.25
        : 6.75

  return {
    kind: stage.stageKind,
    label: stage.label,
    position: {
      x: Math.cos(angle) * radius,
      y: stage.stageKind === 'replay_gap' ? 1.2 : 0,
      z: Math.sin(angle) * radius,
    },
    ref: stage.stageRef,
    sourceRefs: stage.sourceRefs,
  }
}

const targetPosition = (
  targetRefs: ReadonlyArray<string>,
  stagePlacements: ReadonlyArray<ReplayStagePlacement>,
): ReplayVector3 => {
  const target = targetRefs
    .map(ref => stagePlacements.find(stage => stage.ref === ref))
    .find(stage => stage !== undefined)

  return target?.position ?? origin
}

const actorEventState = (
  event: ReplayEvent,
): ReplayActorKeyframe['state'] => {
  if (event.kind === 'settlement_blocked_closed') {
    return 'blocked'
  }

  if (event.kind === 'payment_zap_confirmed') {
    return 'settle'
  }

  if (
    event.kind === 'actor_said_public_message' ||
    event.kind === 'forum_announcement_posted'
  ) {
    return 'talk'
  }

  if (event.kind === 'proof_verified' || event.kind === 'proof_rejected') {
    return 'verify'
  }

  if (event.kind === 'proof_submitted' || event.kind === 'trace_linked') {
    return 'inspect'
  }

  return event.kind === 'actor_entered_region' || event.kind === 'actor_moved'
    ? 'walk'
    : 'idle'
}

const actorHomePosition = (
  actor: ReplayActor,
  index: number,
  stagePlacements: ReadonlyArray<ReplayStagePlacement>,
): ReplayVector3 => {
  const pylonStage = actor.pylonRef
    ? stagePlacements.find(stage => stage.ref.includes(actor.pylonRef ?? ''))
    : undefined

  if (pylonStage !== undefined) {
    return add(pylonStage.position, { x: 0, y: 0, z: 0.8 })
  }

  const angle = (index / Math.max(1, stagePlacements.length)) * Math.PI * 2
  return { x: Math.cos(angle) * 7.8, y: 0, z: Math.sin(angle) * 7.8 }
}

const actorTracksFor = (
  bundle: Pick<ProofReplayBundle, 'actors' | 'events' | 'stages'>,
  stagePlacements: ReadonlyArray<ReplayStagePlacement>,
): ReadonlyArray<ReplayActorTrack> =>
  bundle.actors.map((actor, actorIndex) => {
    const home = actorHomePosition(actor, actorIndex, stagePlacements)
    const events = orderedReplayEvents(bundle)
      .filter(event => event.actorRefs.includes(actor.actorRef))
      .map((event, eventIndex): ReplayActorKeyframe => {
        const target = targetPosition(event.targetRefs, stagePlacements)
        const offset = {
          x: ((actorIndex % 3) - 1) * 0.45,
          y: 0,
          z: (eventIndex % 2 === 0 ? 1 : -1) * 0.45,
        }

        return {
          actorRef: actor.actorRef,
          eventRef: event.eventRef,
          position: add(target, offset),
          second: event.timelineSecond,
          sourceRefs: event.sourceRefs,
          state: actorEventState(event),
        }
      })
    const keyframes: ReadonlyArray<ReplayActorKeyframe> = [
      ({
        actorRef: actor.actorRef,
        eventRef: `actor_home.${actor.actorRef}`,
        position: home,
        second: 0,
        sourceRefs: unique([
          ...(actor.pylonRef === undefined ? [] : [actor.pylonRef]),
        ]),
        state: 'idle',
      }) satisfies ReplayActorKeyframe,
      ...events,
    ].sort((a, b) => a.second - b.second || a.eventRef.localeCompare(b.eventRef))

    return { actorRef: actor.actorRef, keyframes }
  })

export const interpolateActorPosition = (
  track: ReplayActorTrack,
  second: number,
): ReplayVector3 => {
  const keyframes = track.keyframes
  const first = keyframes[0]

  if (first === undefined) {
    return origin
  }

  const previous =
    [...keyframes]
      .reverse()
      .find(keyframe => keyframe.second <= second) ?? first
  const next =
    keyframes.find(keyframe => keyframe.second >= second) ?? previous

  if (next.second === previous.second) {
    return previous.position
  }

  return lerpVector(
    previous.position,
    next.position,
    clamp((second - previous.second) / (next.second - previous.second), 0, 1),
  )
}

export const paymentVisualForEvent = (
  event: ReplayEvent,
): ReplayPaymentVisual => {
  const base = {
    eventRef: event.eventRef,
    sourceRefs: event.sourceRefs,
    ...(event.amountSats === undefined ? {} : { amountSats: event.amountSats }),
    ...(event.rail === undefined ? {} : { rail: event.rail }),
  }

  if (event.kind === 'payment_zap_confirmed') {
    return { ...base, kind: 'confirmed_zap' }
  }

  if (event.kind === 'payment_zap_simulated') {
    return { ...base, kind: 'simulation_path' }
  }

  if (event.kind === 'settlement_blocked_closed') {
    return { ...base, kind: 'blocked_marker' }
  }

  if (
    event.kind === 'recognition_reward_recorded' ||
    event.kind === 'recipient_confirmation_recorded' ||
    event.kind === 'overpayment_detected'
  ) {
    return { ...base, kind: 'recognition_marker' }
  }

  return { ...base, kind: 'neutral_event' }
}

const sourceRefsForTarget = (
  refs: ReadonlyArray<string>,
  fallback: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const sourceRefs = unique(refs)
  return sourceRefs.length > 0 ? sourceRefs : fallback
}

const hitTargetsFor = (
  bundle: Pick<
    ProofReplayBundle,
    'actors' | 'captions' | 'events' | 'gaps' | 'stages'
  >,
): ReadonlyArray<ReplayHitTarget> => [
  ...bundle.actors.map(
    (actor): ReplayHitTarget => ({
      inspectable: true,
      kind: 'actor',
      label: actor.displayName,
      sourceRefs: sourceRefsForTarget(
        actor.pylonRef === undefined ? [] : [actor.pylonRef],
        [actor.actorRef],
      ),
      targetRef: actor.actorRef,
    }),
  ),
  ...bundle.stages.map(
    (stage): ReplayHitTarget => ({
      inspectable: stage.sourceRefs.length > 0,
      kind: 'stage',
      label: stage.label,
      sourceRefs: stage.sourceRefs,
      targetRef: stage.stageRef,
    }),
  ),
  ...bundle.events.map(
    (event): ReplayHitTarget => ({
      inspectable: event.sourceRefs.length > 0,
      kind:
        event.kind === 'payment_zap_confirmed' ||
        event.kind === 'payment_zap_simulated'
          ? 'payment'
          : 'event',
      label: event.displayText,
      sourceRefs: event.sourceRefs,
      targetRef: event.eventRef,
    }),
  ),
  ...bundle.captions.map(
    (caption): ReplayHitTarget => ({
      inspectable: caption.sourceRefs.length > 0,
      kind: 'caption',
      label: caption.text,
      sourceRefs: caption.sourceRefs,
      targetRef: caption.captionRef,
    }),
  ),
  ...bundle.gaps.map(
    (gap): ReplayHitTarget => ({
      inspectable: gap.sourceRefs.length > 0,
      kind: 'gap',
      label: gap.reason,
      sourceRefs: gap.sourceRefs,
      targetRef: gap.gapRef,
    }),
  ),
]

export const buildReplayRenderPlan = (
  bundle: ProofReplayBundle,
): ReplayRenderPlan => {
  const stagePlacements = bundle.stages.map((stage, index) =>
    stagePlacementFor(stage, index, bundle.stages.length),
  )
  const orderedEvents = orderedReplayEvents(bundle)

  return {
    actorTracks: actorTracksFor(bundle, stagePlacements),
    bundleRef: bundle.bundleRef,
    cameraCues: [...bundle.cameraCues].sort(
      (a, b) => a.startSecond - b.startSecond || a.cueRef.localeCompare(b.cueRef),
    ),
    captions: [...bundle.captions].sort(
      (a, b) =>
        a.sequenceIndex - b.sequenceIndex ||
        a.timelineSecond - b.timelineSecond ||
        a.captionRef.localeCompare(b.captionRef),
    ),
    durationSecond: replayDurationSecond(bundle),
    hitTargets: hitTargetsFor(bundle),
    orderedEvents,
    paymentVisuals: orderedEvents.map(paymentVisualForEvent),
    stagePlacements,
  }
}

export const cameraCueAt = (
  plan: Pick<ReplayRenderPlan, 'cameraCues'>,
  second: number,
): ReplayCameraCue | undefined =>
  [...plan.cameraCues]
    .reverse()
    .find(
      cue =>
        cue.startSecond <= second &&
        second <= cue.startSecond + cue.durationSecond,
    ) ?? plan.cameraCues[0]

const placementForFocus = (
  plan: Pick<ReplayRenderPlan, 'actorTracks' | 'stagePlacements'>,
  focusRefs: ReadonlyArray<string>,
  second: number,
): ReplayVector3 => {
  const stage = focusRefs
    .map(ref => plan.stagePlacements.find(placement => placement.ref === ref))
    .find(placement => placement !== undefined)

  if (stage !== undefined) {
    return stage.position
  }

  const actor = focusRefs
    .map(ref => plan.actorTracks.find(track => track.actorRef === ref))
    .find(track => track !== undefined)

  return actor === undefined ? origin : interpolateActorPosition(actor, second)
}

export const cameraPoseFor = (
  plan: Pick<ReplayRenderPlan, 'actorTracks' | 'cameraCues' | 'stagePlacements'>,
  second: number,
  requestedMode?: ReplayCameraMode,
): ReplayCameraPose => {
  const cue = cameraCueAt(plan, second)
  const mode = requestedMode ?? cue?.mode ?? 'overview'
  const focusRefs = cue?.focusRefs ?? []
  const target = placementForFocus(plan, focusRefs, second)
  const orbitSecond = cue === undefined ? second : second - cue.startSecond
  const orbit = {
    x: Math.cos(orbitSecond * 0.18) * 1.5,
    y: 0,
    z: Math.sin(orbitSecond * 0.18) * 1.5,
  }
  const basePosition =
    mode === 'zap_focus'
      ? { x: 3.2, y: 3.4, z: 5.2 }
      : mode === 'follow_actor'
        ? { x: 0.8, y: 2.2, z: 4.6 }
        : mode === 'orbit_proof'
          ? add({ x: 4.8, y: 3.2, z: 4.8 }, orbit)
          : mode === 'free_camera'
            ? { x: 0, y: 5.5, z: 8.5 }
            : { x: 0, y: 7.8, z: 10.5 }

  return {
    cameraRef: `replay_camera.${mode}`,
    mode,
    position: add(target, basePosition),
    second,
    sourceRefs: cue?.sourceRefs ?? [],
    target,
  }
}

export const createReplayDisposalRegistry = (): ReplayDisposalRegistry => {
  const disposables = new Set<ReplayDisposable>()
  let disposedCount = 0

  return {
    add: disposable => {
      disposables.add(disposable)
    },
    disposeAll: () => {
      const pending = [...disposables]
      disposables.clear()
      pending.forEach(disposable => {
        disposable.dispose()
        disposedCount += 1
      })
    },
    disposedCount: () => disposedCount,
    pendingCount: () => disposables.size,
  }
}

export const assertReplayPlanSourceCoverage = (
  plan: ReplayRenderPlan,
): void => {
  const missingEvent = plan.orderedEvents.find(
    event => event.sourceRefs.length === 0,
  )

  if (missingEvent !== undefined) {
    throw new Error(`Replay event missing source refs: ${missingEvent.eventRef}`)
  }

  const missingHitTarget = plan.hitTargets.find(
    target => target.inspectable && target.sourceRefs.length === 0,
  )

  if (missingHitTarget !== undefined) {
    throw new Error(
      `Replay hit target missing source refs: ${missingHitTarget.targetRef}`,
    )
  }
}

export const assertProofReplayBundleShipmentGate = (
  bundle: ProofReplayBundle,
): void => {
  if (bundle.schemaVersion !== 'proof_replay_bundle.v1') {
    throw new ReplayBundleShipmentGateError(
      'bundle',
      `Unsupported replay bundle schema: ${bundle.schemaVersion}`,
    )
  }

  if (
    bundle.privacyLevel !== 'public_safe' ||
    bundle.claimScope !== 'evidence_presentation_only'
  ) {
    throw new ReplayBundleShipmentGateError(
      'privacy',
      'Replay bundle must be public_safe and evidence_presentation_only.',
    )
  }

  if (bundle.sourceRefs.length === 0) {
    throw new ReplayBundleShipmentGateError(
      'source',
      'Replay bundle must include public source refs.',
    )
  }

  if (hasUnsafeReplayMaterial(bundle)) {
    throw new ReplayBundleShipmentGateError(
      'privacy',
      'Replay bundle contains private payment/operator material.',
    )
  }

  const eventWithoutSource = bundle.events.find(
    event => !hasSourceRefs(event.sourceRefs),
  )
  if (eventWithoutSource !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'event',
      `Replay event missing source refs: ${eventWithoutSource.eventRef}`,
    )
  }

  const flowWithoutSource = bundle.flows.find(
    flow => !hasSourceRefs(flow.sourceRefs),
  )
  if (flowWithoutSource !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'flow',
      `Replay flow missing source refs: ${flowWithoutSource.flowRef}`,
    )
  }

  const captionWithoutSource = bundle.captions.find(
    caption => !hasSourceRefs(caption.sourceRefs),
  )
  if (captionWithoutSource !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'caption',
      `Replay caption missing source refs: ${captionWithoutSource.captionRef}`,
    )
  }

  const gapWithoutSource = bundle.gaps.find(gap => !hasSourceRefs(gap.sourceRefs))
  if (gapWithoutSource !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'gap',
      `Replay gap missing source refs: ${gapWithoutSource.gapRef}`,
    )
  }

  const unsupportedZap = bundle.events.find(
    event =>
      event.kind === 'payment_zap_confirmed' &&
      !hasConfirmedPaymentEvidence(event.sourceRefs),
  )
  if (unsupportedZap !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'payment',
      `Confirmed payment zap lacks public payment evidence: ${unsupportedZap.eventRef}`,
    )
  }

  const blockedMoneyMovement = bundle.events.find(
    event =>
      event.kind === 'settlement_blocked_closed' &&
      event.amountSats !== undefined,
  )
  if (blockedMoneyMovement !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'payment',
      `Blocked settlement event cannot carry moving sats: ${blockedMoneyMovement.eventRef}`,
    )
  }

  const simulatedRealCopy = bundle.events.find(
    event =>
      event.kind === 'payment_zap_simulated' &&
      /realBitcoinMoved:true/i.test(
        `${event.displayText} ${event.stateAfter ?? ''} ${event.caveat ?? ''}`,
      ),
  )
  if (simulatedRealCopy !== undefined) {
    throw new ReplayBundleShipmentGateError(
      'payment',
      `Simulated payment event cannot claim realBitcoinMoved:true: ${simulatedRealCopy.eventRef}`,
    )
  }
}
