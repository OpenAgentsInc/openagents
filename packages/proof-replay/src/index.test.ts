import { describe, expect, test } from 'bun:test'

import {
  activeReplayEventsAt,
  assertProofReplayBundleShipmentGate,
  assertReplayPlanSourceCoverage,
  buildReplayRenderPlan,
  cameraPoseFor,
  createReplayDisposalRegistry,
  initialReplayPlaybackState,
  interpolateActorPosition,
  orderedReplayEvents,
  paymentVisualForEvent,
  reduceReplayClock,
  ReplayBundleShipmentGateError,
  type ProofReplayBundle,
  type ReplayEvent,
} from './index'

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
