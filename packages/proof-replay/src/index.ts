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

export type ReplayDisposable = Readonly<{
  dispose: () => void
}>

export type ReplayDisposalRegistry = Readonly<{
  add: (disposable: ReplayDisposable) => void
  disposeAll: () => void
  disposedCount: () => number
  pendingCount: () => number
}>

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values.map(value => value.trim()).filter(value => value !== ''))]

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
