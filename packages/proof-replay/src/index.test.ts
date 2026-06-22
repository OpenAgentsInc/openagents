import { describe, expect, test } from 'bun:test'

import {
  activeReplayEventsAt,
  assertProofReplayBundleShipmentGate,
  assertReplayPlanSourceCoverage,
  buildReplayRenderPlan,
  buildProofReplayBundleFromPublicActivityTimeline,
  cameraPoseFor,
  createReplayDisposalRegistry,
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  initialReplayPlaybackState,
  interpolateActorPosition,
  LAUNCH_RECOGNITION_REPLAY_SLUG,
  orderedReplayEvents,
  paymentVisualForEvent,
  proofReplayBundleEndpointForSlug,
  proofReplayCatalog,
  proofReplayCatalogEntryForSlug,
  reduceReplayClock,
  ReplayBundleShipmentGateError,
  TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT,
  type ProofReplayBundle,
  type ReplayEvent,
} from './index'

type TimelineEventInput = {
  amountSats?: number
  actorRef?: string
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
  eventRef: string
  kind:
    | 'pylon_registered'
    | 'pylon_heartbeat'
    | 'wallet_ready'
    | 'assignment_ready'
    | 'work_claimed'
    | 'trace_submitted'
    | 'verification_verified'
    | 'khala_inference_served'
    | 'settlement_recorded'
    | 'real_bitcoin_moved'
    | 'forum_topic_created'
    | 'forum_posted'
    | 'capacity_snapshot'
    | 'projection_gap'
  realBitcoinMoved?: boolean
  refs?: ReadonlyArray<string>
  sourceKind:
    | 'pylon_api'
    | 'pylon_presence'
    | 'training_trace'
    | 'training_verification'
    | 'inference_receipt'
    | 'settlement_receipt'
    | 'forum'
    | 'capacity_funnel'
    | 'projection_gap'
  sourceRefs?: ReadonlyArray<string>
  state?: string
  text: string
  ts: string
}

const timelineEvent = (input: TimelineEventInput) => ({
  blockerRefs: [...(input.blockerRefs ?? [])],
  caveatRefs: [...(input.caveatRefs ?? [])],
  cursor: `${input.ts}:${input.sourceKind}:${input.eventRef}`,
  eventRef: input.eventRef,
  kind: input.kind,
  refs: [...(input.refs ?? [])],
  sourceKind: input.sourceKind,
  sourceRefs: [...(input.sourceRefs ?? [])],
  text: input.text,
  ts: input.ts,
  ...(input.actorRef === undefined ? {} : { actorRef: input.actorRef }),
  ...(input.amountSats === undefined ? {} : { amountSats: input.amountSats }),
  ...(input.realBitcoinMoved === undefined
    ? {}
    : { realBitcoinMoved: input.realBitcoinMoved }),
  ...(input.state === undefined ? {} : { state: input.state }),
})

const generatedTimelineEnvelope = {
  events: [
    timelineEvent({
      actorRef: 'pylon.worker.alpha',
      eventRef: 'activity.pylon.registered.1',
      kind: 'pylon_registered',
      refs: ['pylon.worker.alpha'],
      sourceKind: 'pylon_api',
      sourceRefs: ['pylon.worker.alpha', 'route:/api/public/pylon-stats'],
      state: 'active',
      text: 'Pylon registered in the public activity timeline.',
      ts: '2026-06-18T11:58:00.000Z',
    }),
    timelineEvent({
      actorRef: 'pylon.worker.alpha',
      caveatRefs: [
        'caveat.public.activity_timeline.wallet_ready_public_projection',
      ],
      eventRef: 'activity.wallet.ready.1',
      kind: 'wallet_ready',
      refs: ['pylon.worker.alpha', 'wallet.public.ref'],
      sourceKind: 'pylon_presence',
      sourceRefs: ['pylon.worker.alpha', 'route:/api/public/pylon-stats'],
      state: 'ready',
      text: 'Pylon reports public wallet readiness with private receive data omitted.',
      ts: '2026-06-18T11:59:00.000Z',
    }),
    timelineEvent({
      actorRef: 'pylon.worker.alpha',
      caveatRefs: [
        'caveat.public.activity_timeline.assignment_ready_is_not_assignment_or_payout',
      ],
      eventRef: 'activity.assignment.ready.1',
      kind: 'assignment_ready',
      refs: ['pylon.worker.alpha', 'capability.public.ref'],
      sourceKind: 'pylon_presence',
      sourceRefs: ['pylon.worker.alpha', 'route:/api/public/pylon-stats'],
      state: 'ready',
      text: 'Pylon has public assignment-readiness evidence.',
      ts: '2026-06-18T11:59:30.000Z',
    }),
    timelineEvent({
      actorRef: 'pylon.worker.alpha',
      eventRef: 'activity.work.claimed.1',
      kind: 'work_claimed',
      refs: ['training.run.demo'],
      sourceKind: 'training_trace',
      sourceRefs: ['training.lease.demo.1'],
      text: 'Worker claimed the public training window.',
      ts: '2026-06-18T12:00:00.000Z',
    }),
    timelineEvent({
      actorRef: 'pylon.worker.alpha',
      eventRef: 'activity.trace.submitted.1',
      kind: 'trace_submitted',
      refs: ['training.run.demo'],
      sourceKind: 'training_trace',
      sourceRefs: ['trace.training.demo.1'],
      text: 'Trace submitted for public verification.',
      ts: '2026-06-18T12:01:00.000Z',
    }),
    timelineEvent({
      eventRef: 'activity.verification.verified.1',
      kind: 'verification_verified',
      refs: ['training.verification.challenge.demo.1'],
      sourceKind: 'training_verification',
      sourceRefs: ['training.verification.challenge.demo.1'],
      text: 'Verifier accepted the replayed trace.',
      ts: '2026-06-18T12:02:00.000Z',
    }),
    timelineEvent({
      actorRef: 'gateway.fireworks',
      caveatRefs: [
        'caveat.public.activity_timeline.inference_receipt_public_projection_only',
      ],
      eventRef: 'activity.khala.inference.1',
      kind: 'khala_inference_served',
      refs: ['receipt.inference.charge.demo.1', 'openagents/khala-code'],
      sourceKind: 'inference_receipt',
      sourceRefs: [
        'receipt.inference.charge.demo.1',
        'https://openagents.com/api/public/inference/receipts/receipt.inference.charge.demo.1',
      ],
      state: 'openagents/khala-code',
      text: 'Khala inference served with a public ledger receipt.',
      ts: '2026-06-18T12:02:30.000Z',
    }),
    timelineEvent({
      amountSats: 1000,
      eventRef: 'activity.settlement.recorded.1',
      kind: 'settlement_recorded',
      refs: ['receipt.training.demo.1'],
      sourceKind: 'settlement_receipt',
      sourceRefs: ['receipt.training.demo.1'],
      text: 'Settlement receipt recorded.',
      ts: '2026-06-18T12:03:00.000Z',
    }),
    timelineEvent({
      amountSats: 1000,
      eventRef: 'activity.real.bitcoin.1',
      kind: 'real_bitcoin_moved',
      realBitcoinMoved: true,
      refs: ['receipt.training.demo.1'],
      sourceKind: 'settlement_receipt',
      sourceRefs: ['receipt.training.demo.1'],
      text: 'Receipt-backed Spark payment confirmed.',
      ts: '2026-06-18T12:04:00.000Z',
    }),
    timelineEvent({
      eventRef: 'activity.forum.posted.1',
      kind: 'forum_posted',
      refs: ['forum.topic.demo'],
      sourceKind: 'forum',
      sourceRefs: ['forum.post.demo.1'],
      text: 'Forum post announced the verified payment.',
      ts: '2026-06-18T12:05:00.000Z',
    }),
    timelineEvent({
      caveatRefs: [
        'caveat.public.activity_timeline.capacity_snapshot_counts_only',
      ],
      eventRef: 'activity.capacity.snapshot.1',
      kind: 'capacity_snapshot',
      refs: ['capacity.snapshot.demo.1'],
      sourceKind: 'capacity_funnel',
      sourceRefs: [
        'capacity.snapshot.demo.1',
        'route:/api/public/pylon-capacity-funnel/history',
      ],
      state: 'total:7',
      text: 'Capacity funnel snapshot recorded.',
      ts: '2026-06-18T12:05:30.000Z',
    }),
    timelineEvent({
      blockerRefs: ['blocker.activity.capacity_source_unavailable'],
      eventRef: 'activity.projection.gap.1',
      kind: 'projection_gap',
      refs: ['capacity_snapshot'],
      sourceKind: 'projection_gap',
      text: 'Capacity source unavailable at read time.',
      ts: '2026-06-18T12:06:00.000Z',
    }),
  ],
  generatedAt: '2026-06-18T12:07:00.000Z',
  nextCursor: null,
  schemaVersion: 'openagents.public_activity_timeline.v1',
  sourceLag: [
    {
      blockerRefs: [],
      caveatRefs: [],
      lagSeconds: 0,
      latestSourceEventAt: '2026-06-18T12:06:00.000Z',
      maxStalenessSeconds: 0,
      observedAt: '2026-06-18T12:07:00.000Z',
      sourceKind: 'settlement_receipt',
      sourceRefs: ['receipt.training.demo.1'],
      status: 'current',
    },
    {
      blockerRefs: ['blocker.activity.forum_lag'],
      caveatRefs: ['caveat.activity.forum_stale'],
      lagSeconds: 90,
      latestSourceEventAt: '2026-06-18T12:05:30.000Z',
      maxStalenessSeconds: 30,
      observedAt: '2026-06-18T12:07:00.000Z',
      sourceKind: 'forum',
      sourceRefs: ['forum.post.demo.1'],
      status: 'stale',
    },
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['read'],
  },
}

const event = (
  kind: ReplayEvent['kind'],
  sequenceIndex: number,
  timelineSecond: number,
  overrides: Partial<ReplayEvent> = {},
): ReplayEvent => ({
  actorRefs: ['actor.pylon'],
  displayText: kind,
  eventRef: `event.${sequenceIndex}.${kind}`,
  kind,
  sequenceIndex,
  sourceRefs: [`source.${sequenceIndex}`],
  targetRefs: ['stage.proof'],
  timelineSecond,
  ...overrides,
})

const bundle = {
  actors: [
    {
      actorRef: 'actor.pylon',
      avatarRole: 'contributor',
      displayName: 'Contributor',
      fallbackAssetId: 'procedural.avatar',
      pylonRef: 'pylon.public.worker',
    },
    {
      actorRef: 'actor.treasury',
      avatarRole: 'settlement_terminal',
      displayName: 'Treasury',
      fallbackAssetId: 'procedural.terminal',
    },
  ],
  bundleRef: 'proof_replay_bundle.test.first',
  cameraCues: [
    {
      cueRef: 'cue.overview',
      durationSecond: 8,
      focusRefs: ['stage.run'],
      mode: 'overview',
      sourceRefs: ['run.test'],
      startSecond: 0,
    },
    {
      cueRef: 'cue.proof',
      durationSecond: 12,
      focusRefs: ['stage.proof'],
      mode: 'orbit_proof',
      sourceRefs: ['challenge.test'],
      startSecond: 8,
    },
    {
      cueRef: 'cue.zap',
      durationSecond: 8,
      focusRefs: ['stage.settlement'],
      mode: 'zap_focus',
      sourceRefs: ['receipt.test.real'],
      startSecond: 20,
    },
  ],
  captions: [
    {
      captionRef: 'caption.title',
      sequenceIndex: 0,
      sourceRefs: ['run.test'],
      text: 'Replay title',
      timelineSecond: 0,
    },
  ],
  claimScope: 'evidence_presentation_only',
  events: [
    event('proof_verified', 2, 10),
    event('actor_entered_region', 0, 0, {
      targetRefs: ['stage.run'],
    }),
    event('payment_zap_simulated', 3, 14, {
      amountSats: 5,
      sourceRefs: ['receipt.test.simulation'],
      targetRefs: ['stage.settlement'],
    }),
    event('settlement_recorded', 4, 22, {
      amountSats: 1_000,
      rail: 'spark_treasury',
      sourceRefs: ['receipt.test.real'],
      targetRefs: ['stage.settlement'],
    }),
    event('payment_zap_confirmed', 5, 24, {
      actorRefs: ['actor.treasury'],
      amountSats: 1_000,
      rail: 'spark_treasury',
      sourceRefs: ['receipt.test.real'],
      targetRefs: ['actor.pylon', 'stage.pylon'],
    }),
    event('settlement_blocked_closed', 1, 6, {
      sourceRefs: ['forum.failed_closed'],
      targetRefs: ['stage.settlement'],
    }),
  ],
  flows: [
    {
      flowKind: 'payment_movement',
      flowRef: 'flow.real',
      fromRef: 'actor.treasury',
      sourceRefs: ['receipt.test.real'],
      toRef: 'actor.pylon',
    },
  ],
  gaps: [
    {
      affectedRefs: ['settlement_blocked_closed'],
      gapRef: 'gap.sequence',
      reason: 'ordered by sequence',
      sourceRefs: ['forum.failed_closed'],
    },
  ],
  generatedAt: '2026-06-18T02:00:00.000Z',
  privacyLevel: 'public_safe',
  schemaVersion: 'proof_replay_bundle.v1',
  sourceAuthority: 'worker_d1_public',
  sourceRefs: [
    { kind: 'run', ref: 'run.test' },
    { kind: 'receipt', ref: 'receipt.test.real' },
  ],
  stages: [
    {
      label: 'Tassadar',
      sourceRefs: ['run.test'],
      stageKind: 'run_core',
      stageRef: 'stage.run',
    },
    {
      label: 'Proof gate',
      sourceRefs: ['challenge.test'],
      stageKind: 'proof_gate',
      stageRef: 'stage.proof',
    },
    {
      label: 'Settlement terminal',
      sourceRefs: ['receipt.test.real'],
      stageKind: 'settlement_terminal',
      stageRef: 'stage.settlement',
    },
    {
      label: 'Contributor station',
      sourceRefs: ['pylon.public.worker'],
      stageKind: 'pylon_station',
      stageRef: 'stage.pylon',
    },
  ],
  title: 'Tassadar Run 1',
} satisfies ProofReplayBundle

describe('@openagentsinc/proof-replay', () => {
  test('exports one replay catalog for web and desktop surfaces', () => {
    const catalog = proofReplayCatalog('https://openagents.com/')

    expect(catalog.map(entry => entry.slug)).toEqual([
      FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
      LAUNCH_RECOGNITION_REPLAY_SLUG,
    ])
    expect(catalog[0]?.bundleEndpoint).toBe(
      `https://openagents.com${TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT}`,
    )
    expect(catalog[0]?.websitePath).toBe(
      'https://openagents.com/tassadar/replay/first-real-settlement',
    )
    expect(catalog[0]?.socialPath).toBe(
      'https://openagents.com/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social',
    )
    expect(catalog[1]?.bundleEndpoint).toBe(
      'https://openagents.com/api/public/proof-replays?ref=launch-recognition-payments',
    )
    expect(
      proofReplayCatalogEntryForSlug(LAUNCH_RECOGNITION_REPLAY_SLUG)?.websitePath,
    ).toBe('/tassadar/replay/launch-recognition-payments')
  })

  test('resolves first replay through its compatibility endpoint and generic refs through proof-replays', () => {
    expect(proofReplayBundleEndpointForSlug(FIRST_REAL_SETTLEMENT_REPLAY_SLUG)).toBe(
      TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT,
    )
    expect(
      proofReplayBundleEndpointForSlug(
        LAUNCH_RECOGNITION_REPLAY_SLUG,
        'https://openagents.com',
      ),
    ).toBe(
      'https://openagents.com/api/public/proof-replays?ref=launch-recognition-payments',
    )
  })

  test('generates a shipment-gated replay bundle from public activity timeline events', () => {
    const generated = buildProofReplayBundleFromPublicActivityTimeline(
      generatedTimelineEnvelope,
      {
        bundleRef: 'proof_replay_bundle.public_activity.test',
        origin: 'https://openagents.com',
        title: 'Generated Public Activity Replay',
      },
    )

    expect(generated.schemaVersion).toBe('proof_replay_bundle.v1')
    expect(generated.sourceAuthority).toBe('public_activity_timeline')
    expect(generated.title).toBe('Generated Public Activity Replay')
    expect(generated.events.map(item => item.kind)).toEqual([
      'actor_entered_region',
      'actor_focused_pylon',
      'actor_focused_pylon',
      'actor_focused_pylon',
      'trace_linked',
      'proof_verified',
      'receipt_recorded',
      'settlement_recorded',
      'payment_zap_confirmed',
      'forum_announcement_posted',
      'artifact_opened',
    ])
    expect(generated.flows.map(flow => flow.flowKind)).toEqual([
      'fleet_readiness_track',
      'fleet_readiness_track',
      'fleet_readiness_track',
      'fleet_readiness_track',
      'payment_movement',
      'discussion_track',
      'capacity_snapshot_track',
    ])
    expect(generated.events.find(item => item.kind === 'payment_zap_confirmed'))
      .toEqual(
        expect.objectContaining({
          amountSats: 1000,
          rail: 'spark_treasury',
          sourceRefs: ['receipt.training.demo.1'],
        }),
      )
    const khalaReceiptEvent = generated.events.find(
      item => item.kind === 'receipt_recorded',
    )
    expect(khalaReceiptEvent).toEqual(
      expect.objectContaining({
        caveat:
          'caveat.public.activity_timeline.inference_receipt_public_projection_only',
        displayText: 'Khala inference served with a public ledger receipt.',
        stateAfter: 'openagents/khala-code',
      }),
    )
    expect(khalaReceiptEvent?.sourceRefs).toEqual(
      expect.arrayContaining(['receipt.inference.charge.demo.1']),
    )
    expect(generated.sourceRefs).toContainEqual(
      expect.objectContaining({
        kind: 'receipt',
        ref: 'receipt.training.demo.1',
        url: 'https://openagents.com/api/public/nexus-pylon/receipts/receipt.training.demo.1',
      }),
    )
    expect(generated.gaps.map(gap => gap.gapRef)).toEqual([
      'gap.activity.projection.gap.1',
      'gap.source_lag.1.forum',
    ])
    expect(generated.gaps[0]?.affectedRefs).toContain('capacity_snapshot')
    expect(generated.events.find(item => item.kind === 'artifact_opened'))
      .toEqual(
        expect.objectContaining({
          caveat: 'caveat.public.activity_timeline.capacity_snapshot_counts_only',
          displayText: 'Capacity funnel snapshot recorded.',
          stateAfter: 'total:7',
        }),
      )
    expect(generated.events.find(item => item.kind === 'forum_announcement_posted'))
      .toEqual(
        expect.objectContaining({
          displayText: 'Forum post announced the verified payment.',
          sourceRefs: ['forum.post.demo.1'],
        }),
      )
    expect(generated.cameraCues.map(cue => cue.mode)).toContain('zap_focus')

    assertProofReplayBundleShipmentGate(generated)
    assertReplayPlanSourceCoverage(buildReplayRenderPlan(generated))
  })

  test('generated replay rejects private material in Forum text and actor refs', () => {
    const unsafe = {
      ...generatedTimelineEnvelope,
      events: [
        timelineEvent({
          actorRef: 'agent@example.com',
          eventRef: 'activity.forum.private.1',
          kind: 'forum_posted',
          refs: ['forum.topic.demo'],
          sourceKind: 'forum',
          sourceRefs: ['forum.post.demo.private'],
          text: 'Public Forum post mentions customer email address.',
          ts: '2026-06-18T12:05:00.000Z',
        }),
      ],
      sourceLag: [],
    }

    expect(() =>
      buildProofReplayBundleFromPublicActivityTimeline(unsafe),
    ).toThrow(/raw\/private material/)
  })

  test('generated replay rejects real bitcoin timeline rows without public receipt evidence', () => {
    const unsafe = {
      ...generatedTimelineEnvelope,
      events: [
        timelineEvent({
          amountSats: 1000,
          eventRef: 'activity.real.bitcoin.unsafe',
          kind: 'real_bitcoin_moved',
          realBitcoinMoved: true,
          sourceKind: 'settlement_receipt',
          sourceRefs: ['settlement.row.without.receipt'],
          text: 'Unsafe payment row.',
          ts: '2026-06-18T12:00:00.000Z',
        }),
      ],
      sourceLag: [],
    }

    expect(() =>
      buildProofReplayBundleFromPublicActivityTimeline(unsafe),
    ).toThrow(/realBitcoinMoved:true requires a public receipt source ref/)
  })

  test('builds a deterministic render plan with stable event ordering and hit targets', () => {
    const first = buildReplayRenderPlan(bundle)
    const second = buildReplayRenderPlan(bundle)

    expect(first).toEqual(second)
    expect(first.orderedEvents.map(item => item.kind)).toEqual([
      'actor_entered_region',
      'settlement_blocked_closed',
      'proof_verified',
      'payment_zap_simulated',
      'settlement_recorded',
      'payment_zap_confirmed',
    ])
    expect(first.stagePlacements.find(stage => stage.ref === 'stage.run')?.position)
      .toEqual({ x: 0, y: 0, z: 0 })
    expect(
      first.hitTargets.find(target => target.targetRef === 'event.5.payment_zap_confirmed'),
    ).toEqual(
      expect.objectContaining({
        inspectable: true,
        kind: 'payment',
        sourceRefs: ['receipt.test.real'],
      }),
    )
    assertReplayPlanSourceCoverage(first)
  })

  test('orders equal-sequence events by authoritative replay second and stable ref', () => {
    const events = orderedReplayEvents({
      events: [
        event('proof_verified', 7, 10, { eventRef: 'event.c' }),
        event('proof_submitted', 7, 4, { eventRef: 'event.a' }),
        event('trace_linked', 7, 4, { eventRef: 'event.b' }),
      ],
    })

    expect(events.map(item => item.eventRef)).toEqual([
      'event.a',
      'event.b',
      'event.c',
    ])
  })

  test('clock reducer gates active events by replay time', () => {
    const plan = buildReplayRenderPlan(bundle)
    const initial = initialReplayPlaybackState(bundle)
    const playing = reduceReplayClock(initial, { type: 'play' })
    const fast = reduceReplayClock(playing, {
      playbackRate: 2,
      type: 'set_speed',
    })
    const ticked = reduceReplayClock(fast, { deltaSecond: 6, type: 'tick' })
    const paused = reduceReplayClock(ticked, { type: 'pause' })
    const ignoredTick = reduceReplayClock(paused, { deltaSecond: 40, type: 'tick' })
    const seeked = reduceReplayClock(ignoredTick, { second: 1000, type: 'seek' })

    expect(ticked.second).toBe(12)
    expect(ignoredTick.second).toBe(12)
    expect(seeked.second).toBe(plan.durationSecond)
    expect(activeReplayEventsAt(bundle, 12).map(item => item.kind)).toEqual([
      'actor_entered_region',
      'settlement_blocked_closed',
      'proof_verified',
    ])
  })

  test('camera director returns deterministic poses for overview, proof, zap, and free camera modes', () => {
    const plan = buildReplayRenderPlan(bundle)

    expect(cameraPoseFor(plan, 0).mode).toBe('overview')
    expect(cameraPoseFor(plan, 12).mode).toBe('orbit_proof')
    expect(cameraPoseFor(plan, 24).mode).toBe('zap_focus')
    expect(cameraPoseFor(plan, 24, 'free_camera')).toEqual(
      expect.objectContaining({
        cameraRef: 'replay_camera.free_camera',
        mode: 'free_camera',
      }),
    )
    expect(cameraPoseFor(plan, 24)).toEqual(cameraPoseFor(plan, 24))
  })

  test('interpolates actor tracks without advancing to future events', () => {
    const plan = buildReplayRenderPlan(bundle)
    const track = plan.actorTracks.find(item => item.actorRef === 'actor.pylon')

    expect(track).toBeDefined()
    if (track === undefined) {
      throw new Error('missing actor track')
    }

    const beforeProof = interpolateActorPosition(track, 4)
    const atProof = interpolateActorPosition(track, 10)
    expect(beforeProof).not.toEqual(atProof)
    expect(interpolateActorPosition(track, -1)).toEqual(track.keyframes[0]?.position)
  })

  test('classifies payment visuals so only confirmed payment events become zaps', () => {
    const plan = buildReplayRenderPlan(bundle)
    const visuals = new Map(
      plan.paymentVisuals.map(visual => [visual.eventRef, visual.kind]),
    )

    expect(visuals.get('event.5.payment_zap_confirmed')).toBe('confirmed_zap')
    expect(visuals.get('event.3.payment_zap_simulated')).toBe('simulation_path')
    expect(visuals.get('event.1.settlement_blocked_closed')).toBe(
      'blocked_marker',
    )
    expect(paymentVisualForEvent(event('proof_submitted', 99, 1)).kind).toBe(
      'neutral_event',
    )
    expect(
      paymentVisualForEvent(event('recognition_reward_recorded', 98, 1)).kind,
    ).toBe('recognition_marker')
    expect(
      paymentVisualForEvent(event('overpayment_detected', 97, 1)).kind,
    ).toBe('recognition_marker')
  })

  test('shipment gate accepts public-safe evidence bundles and rejects missing coverage', () => {
    expect(() => assertProofReplayBundleShipmentGate(bundle)).not.toThrow()

    expect(() =>
      assertProofReplayBundleShipmentGate({
        ...bundle,
        captions: [
          {
            captionRef: 'caption.missing_source',
            sequenceIndex: 0,
            sourceRefs: [],
            text: 'Missing source',
            timelineSecond: 0,
          },
        ],
      }),
    ).toThrow(ReplayBundleShipmentGateError)
  })

  test('shipment gate blocks unsafe private material and unsupported confirmed zaps', () => {
    expect(() =>
      assertProofReplayBundleShipmentGate({
        ...bundle,
        captions: [
          {
            captionRef: 'caption.private',
            sequenceIndex: 0,
            sourceRefs: ['source.private'],
            text: 'payment_hash should not render',
            timelineSecond: 0,
          },
        ],
      }),
    ).toThrow(/private payment/)

    expect(() =>
      assertProofReplayBundleShipmentGate({
        ...bundle,
        events: [
          event('payment_zap_confirmed', 0, 1, {
            eventRef: 'event.unsafe_zap',
            sourceRefs: ['ledger.pending.only'],
          }),
        ],
      }),
    ).toThrow(/lacks public payment evidence/)

    expect(() =>
      assertProofReplayBundleShipmentGate({
        ...bundle,
        events: [
          event('settlement_blocked_closed', 0, 1, {
            amountSats: 100,
            eventRef: 'event.blocked_money',
            sourceRefs: ['source.blocked'],
          }),
        ],
      }),
    ).toThrow(/cannot carry moving sats/)
  })

  test('disposal registry cleans registered render resources exactly once', () => {
    const registry = createReplayDisposalRegistry()
    let disposed = 0
    registry.add({ dispose: () => { disposed += 1 } })
    registry.add({ dispose: () => { disposed += 1 } })

    expect(registry.pendingCount()).toBe(2)
    registry.disposeAll()
    registry.disposeAll()
    expect(disposed).toBe(2)
    expect(registry.disposedCount()).toBe(2)
    expect(registry.pendingCount()).toBe(0)
  })
})
