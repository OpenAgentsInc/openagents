import { describe, expect, it } from 'vitest'

import {
  type TassadarRunPublicSummary,
  applyWorldEntitySpatialLayout,
  cloudflareWorldSummaryFromRows,
  tassadarRunBulletinWorldItem,
  tassadarRunVisualizationOptions,
  trainingRunEntityLayerFromPublicSummary,
  trainingRunSnapshotFromPublicSummary,
} from './tassadarRunSnapshot'

const m = (value: number) => ({
  value,
  provenanceLabel: 'observed',
  sourceRefs: [],
})

const activitySummary = (
  input: Readonly<{
    cursor?: string
    eventRef?: string
    expiresAt?: string
    generatedAt?: string
    kind?: string
    sourceKind?: string
    sourceLagStatus?: string
    sourceRefs?: ReadonlyArray<string>
    text?: string
  }>,
) =>
  JSON.stringify({
    authority: 'worker_d1_public_projection_only',
    blockerRefs: [],
    caveatRefs: ['caveat.public.activity_timeline.source_lag_exceeds_contract'],
    cursor:
      input.cursor ??
      '2026-06-18T18:00:02.000Z:pylon_presence:pylon.world.one',
    eventRef: input.eventRef ?? 'pylon.world.one',
    eventTs: '2026-06-18T18:00:02.000Z',
    expiresAt: input.expiresAt ?? '2026-06-18T18:05:02.000Z',
    generatedAt: input.generatedAt ?? '2026-06-18T18:00:02.000Z',
    kind: input.kind ?? 'pylon_heartbeat',
    schema: 'openagents.world.public_activity_event_summary.v1',
    sourceKind: input.sourceKind ?? 'pylon_presence',
    sourceLagStatus: input.sourceLagStatus ?? 'stale',
    sourceRefs: input.sourceRefs ?? [
      'pylon.world.one',
      'route:/api/public/pylon-stats',
    ],
    text:
      input.text ?? 'Pylon heartbeat observed in the public activity timeline.',
  })

const populated: TassadarRunPublicSummary = {
  runRef: 'run.tassadar.executor.20260615',
  runLabel: 'Tassadar executor run',
  runState: 'active',
  bulletin: {
    schemaVersion: 'openagents.public_tassadar_run_bulletin.v1',
    title: 'Tassadar Run Board',
    headline: 'Tassadar is active: 7 pylons, 2 active.',
    summary:
      'Tassadar is active: 7 pylons, 2 active. 2 training windows active right now.',
    statusLine: 'active · 7 pylons, 2 active',
    onBoardLines: ['Status: active', '7 pylons, 2 active', '2,100 sats paid'],
    metrics: {
      acceptedTraceCount: 2,
      activePylonCount: 2,
      activeWindowCount: 2,
      realSettlementCount: 1,
      settledSats: 2100,
      totalPylonCount: 7,
      verifiedWorkCount: 9,
    },
    sourceRefs: [
      'run.tassadar.executor.20260615',
      'route:/api/public/tassadar-run-summary',
    ],
  },
  emptyState: { idle: false, reason: '' },
  metrics: {
    activeWindowCount: m(2),
    plannedWindowCount: m(1),
    sealedWindowCount: m(3),
    reconciledWindowCount: m(1),
    assignedContributorCount: m(7),
    verifiedWorkCount: m(9),
    rejectedWorkCount: m(1),
    pendingPayoutCount: m(2),
    receiptRefCount: m(12),
    providerConfirmedSettledPayoutSats: m(2100),
  },
  corpus: {
    acceptedTraceCount: 2,
    traceRefs: ['trace.tassadar.accepted.1', 'trace.tassadar.accepted.2'],
    verdictRefs: ['verdict.tassadar.replay.1'],
  },
  realGradient: {
    deviceRequirement: {
      observedDistinctContributorDevices: 4,
      requiredDistinctContributorDevices: 4,
    },
    lossUnderBudget: {
      finalValidationLoss: 2.74,
      maxValidationLoss: 3.1,
      satisfied: true,
    },
    closeoutRequirement: {
      satisfied: true,
      freivaldsCommitmentRefs: ['a', 'b', 'c', 'd', 'e'],
      gradientCloseoutRefs: ['g1', 'g2', 'g3', 'g4'],
    },
    externalAsk: { blockerRefs: [] },
    leaderboardRows: [
      {
        pylonRef: 'pylon.worker.one',
        rank: 1,
        settledPayoutSats: 0,
        sourceRefs: ['training.lease.worker.one'],
        trainingRunRef: 'run.tassadar.executor.20260615',
        verifiedWindowCount: 1,
      },
      {
        pylonRef: 'pylon.worker.two',
        rank: 2,
        settledPayoutSats: 2100,
        sourceRefs: ['training.lease.worker.two'],
        trainingRunRef: 'run.tassadar.executor.20260615',
        verifiedWindowCount: 1,
      },
    ],
    verifiedReplayPairs: [
      {
        challengeRef: 'challenge.tassadar.replay.1',
        sourceRefs: [
          'challenge.tassadar.replay.1',
          'contribution.tassadar.worker.1',
          'validator.tassadar.1',
          'verdict.tassadar.replay.1',
        ],
        validatorRef: 'validator.tassadar.1',
        verdictRefs: ['verdict.tassadar.replay.1'],
        workerRef: 'contribution.tassadar.worker.1',
      },
    ],
    rejectedReplayPairs: [
      {
        challengeRef: 'challenge.tassadar.replay.rejected.1',
        failureCodes: ['DigestMismatch'],
        sourceRefs: [
          'challenge.tassadar.replay.rejected.1',
          'contribution.tassadar.worker.rejected.1',
          'validator.tassadar.rejected.1',
          'verdict.tassadar.replay.rejected.1',
        ],
        validatorRef: 'validator.tassadar.rejected.1',
        verdictRefs: ['verdict.tassadar.replay.rejected.1'],
        workerRef: 'contribution.tassadar.worker.rejected.1',
      },
    ],
  },
  receiptRefs: ['receipt.pylon.settlement.1'],
  settlementRows: [
    {
      amountSats: 2100,
      contributorRef: 'pylon.worker.two',
      movementMode: 'real_bitcoin',
      realBitcoinMoved: true,
      receiptKind: 'settlement_recorded',
      receiptRef: 'receipt.nexus.tassadar.settlement.real.1',
      sourceRefs: [
        'receipt.nexus.tassadar.settlement.real.1',
        'pylon.worker.two',
        'challenge.tassadar.replay.1',
      ],
      state: 'settled',
      trainingRunRef: 'run.tassadar.executor.20260615',
      verificationChallengeRef: 'challenge.tassadar.replay.1',
    },
  ],
}

describe('trainingRunSnapshotFromPublicSummary', () => {
  it('maps a populated public summary 1:1 into the visualization snapshot', () => {
    const s = trainingRunSnapshotFromPublicSummary(populated)
    expect(s.runState).toBe('active')
    expect(s.runLabel).toBe('Tassadar executor run')
    expect(s.runDetail).toBe('run.tassadar.executor.20260615')
    expect(s.activeWindowCount).toBe(2)
    expect(s.plannedWindowCount).toBe(1)
    expect(s.sealedWindowCount).toBe(3)
    expect(s.reconciledWindowCount).toBe(1)
    expect(s.assignedContributorCount).toBe(7)
    expect(s.verifiedWorkCount).toBe(9)
    expect(s.rejectedWorkCount).toBe(1)
    expect(s.pendingPayoutCount).toBe(2)
    expect(s.receiptRefCount).toBe(12)
    expect(s.settledPayoutSats).toBe(2100)
    expect(s.deviceObserved).toBe(4)
    expect(s.deviceRequired).toBe(4)
    expect(s.finalValidationLoss).toBe(2.74)
    expect(s.maxValidationLoss).toBe(3.1)
    expect(s.lossUnderBudget).toBe(true)
    expect(s.closeoutSatisfied).toBe(true)
    expect(s.freivaldsRefCount).toBe(5)
    expect(s.gradientCloseoutRefCount).toBe(4)
    expect(s.blockerRefCount).toBe(0)
  })

  it('renders a just-launched / idle run honestly — all zeros, no faked values (receipt-first)', () => {
    const empty: TassadarRunPublicSummary = {
      runRef: 'run.tassadar.executor.20260615',
      emptyState: { idle: true, reason: 'no verified work yet' },
    }
    const s = trainingRunSnapshotFromPublicSummary(empty)
    expect(s.runState).toBe('planned') // idle → planned, not "active"
    expect(s.verifiedWorkCount).toBe(0)
    expect(s.settledPayoutSats).toBe(0)
    expect(s.activeWindowCount).toBe(0)
    expect(s.assignedContributorCount).toBe(0)
    expect(s.deviceObserved).toBe(0)
    expect(s.deviceRequired).toBe(0)
    expect(s.receiptRefCount).toBe(0)
    expect(s.finalValidationLoss).toBeNull()
    expect(s.maxValidationLoss).toBeNull()
    expect(s.lossUnderBudget).toBe(false)
    expect(s.closeoutSatisfied).toBe(false)
    expect(s.blockerRefCount).toBe(0)
  })

  it('is defensive — partial/missing/garbage fields default to honest zeros without throwing', () => {
    expect(() => trainingRunSnapshotFromPublicSummary({})).not.toThrow()
    const s = trainingRunSnapshotFromPublicSummary({
      metrics: {
        verifiedWorkCount: { value: Number.NaN },
      },
      realGradient: { lossUnderBudget: {} },
    })
    expect(s.verifiedWorkCount).toBe(0) // NaN → 0
    expect(s.settledPayoutSats).toBe(0)
    expect(s.finalValidationLoss).toBeNull()
    expect(s.runLabel).toBe('Tassadar executor run') // default label
    expect(s.runState).toBe('active') // not idle → active
  })

  it('produces resolvable trainingRunView options end-to-end', () => {
    const options = tassadarRunVisualizationOptions(populated)
    expect(options).toBeTruthy()
    // the resolver always yields a renderable option object (nodes/contributors derived)
    expect(typeof options).toBe('object')
    expect(options.lossCurve).toEqual([])
    expect(options.motionPolicy).toEqual({
      ambient: 'static',
      bursts: 'once',
      evidence: 'required',
      structuralEdges: 'static',
    })
    expect(options.nodes).toEqual([
      {
        connectedTo: [],
        detail: '',
        id: 'run',
        label: 'Tassadar',
        position: [-0.15, 0.28, 0],
        role: 'run',
        status: 'active',
      },
    ])
    expect(options.worldItems).toEqual([
      expect.objectContaining({
        id: 'bulletin.tassadar.run',
        kind: 'bulletin_board',
        label: 'Tassadar Run Board',
        lines: ['Status: active', '7 pylons, 2 active', '2,100 sats paid'],
      }),
    ])
    const spatialLifecycleIds = [
      'registered',
      'qualified',
      'state_synced',
      'warmup',
      'active',
      'sync_reentry',
    ]
    expect(
      options.entities?.some(entity => spatialLifecycleIds.includes(entity.id)),
    ).toBe(false)
    expect(options.sceneChrome).toEqual({
      contributorOrbit: 'hidden',
      lossPanel: 'hidden',
      staleRing: 'hidden',
      statusChart: 'hidden',
    })
    expect(options.stageNodeGlyph).toBe('compact_gate')
  })

  it('maps the bulletin block to an in-world board item', () => {
    expect(tassadarRunBulletinWorldItem(populated)).toEqual(
      expect.objectContaining({
        id: 'bulletin.tassadar.run',
        kind: 'bulletin_board',
        title: 'Tassadar Run Board',
        detail:
          'Tassadar is active: 7 pylons, 2 active. 2 training windows active right now.',
        interactionRadius: 2.8,
        status: 'active',
        sourceRefs: [
          'run.tassadar.executor.20260615',
          'route:/api/public/tassadar-run-summary',
        ],
      }),
    )
    expect(tassadarRunBulletinWorldItem({ runRef: 'run.empty' })).toBeNull()
  })

  it('adds data-bound pylon/proof entities without duplicate orbit dots or main-view motion', () => {
    const layer = trainingRunEntityLayerFromPublicSummary(populated)
    expect(layer.contributors).toEqual([])
    expect(layer.entities).toEqual([
      {
        id: 'pylon.worker.one',
        label: 'P1',
        position: [-2.35, 1.5, 0.12],
        status: 'verified',
      },
      {
        id: 'pylon.worker.two',
        label: 'P2',
        position: [-2.35, -1.5, 0.12],
        status: 'real_settled',
      },
      {
        id: 'contribution.tassadar.worker.1',
        label: 'W1',
        position: [0, 2.05, 0.12],
        status: 'verified',
      },
      {
        id: 'validator.tassadar.1',
        label: 'V1',
        position: [0, 1.48, 0.12],
        status: 'verified',
      },
      {
        id: 'contribution.tassadar.worker.rejected.1',
        label: 'RW1',
        position: [0, -1.14, 0.12],
        status: 'rejected',
      },
      {
        id: 'validator.tassadar.rejected.1',
        label: 'RV1',
        position: [0, -1.86, 0.12],
        status: 'rejected',
      },
      {
        id: 'receipt.nexus.tassadar.settlement.real.1',
        label: '2100s',
        position: [-1.15, -2.2, 0.12],
        status: 'real_settled',
      },
      {
        id: 'trace.tassadar.accepted.1',
        label: 'T1',
        position: [2.25, 0.8, 0.12],
        status: 'accepted_trace',
      },
      {
        id: 'trace.tassadar.accepted.2',
        label: 'T2',
        position: [2.25, -0.6, 0.12],
        status: 'accepted_trace',
      },
    ])
    expect(layer.beams).toEqual([])
    expect(layer.bursts).toEqual([])
    expect(layer.lossCurve).toEqual([])
    expect(layer.motionPolicy).toEqual({
      ambient: 'static',
      bursts: 'once',
      evidence: 'required',
      structuralEdges: 'static',
    })
    expect(layer.nodes).toEqual([
      {
        connectedTo: [],
        detail: '',
        id: 'run',
        label: 'Tassadar',
        position: [-0.15, 0.28, 0],
        role: 'run',
        status: 'active',
      },
    ])
    expect(layer.sceneChrome).toEqual({
      contributorOrbit: 'hidden',
      lossPanel: 'hidden',
      staleRing: 'hidden',
      statusChart: 'hidden',
    })
    expect(layer.stageNodeGlyph).toBe('compact_gate')
  })

  it('does not emit payout bursts for simulation-backed settlement rows', () => {
    const layer = trainingRunEntityLayerFromPublicSummary({
      realGradient: {
        leaderboardRows: [
          {
            pylonRef: 'pylon.simulation.only',
            rank: 1,
            sourceRefs: ['training.lease.simulation.only'],
            verifiedWindowCount: 1,
          },
        ],
      },
      settlementRows: [
        {
          amountSats: 5,
          contributorRef: 'pylon.simulation.only',
          movementMode: 'simulation',
          realBitcoinMoved: false,
          receiptKind: 'settlement_recorded',
          receiptRef: 'receipt.nexus.tassadar.simulation.only',
          sourceRefs: ['receipt.nexus.tassadar.simulation.only'],
          state: 'settled',
        },
      ],
    })
    expect(layer.entities).toContainEqual({
      id: 'pylon.simulation.only',
      label: 'P1',
      position: [-2.35, 0, 0.12],
      status: 'simulation_settled',
    })
    expect(layer.entities).toContainEqual({
      id: 'receipt.nexus.tassadar.simulation.only',
      label: '5s',
      position: [-1.15, -2.2, 0.12],
      status: 'simulation_settled',
    })
    expect(layer.bursts).toEqual([])
  })

  it('does not fabricate proof beams, payout bursts, contributors, or loss curves without public refs', () => {
    const layer = trainingRunEntityLayerFromPublicSummary({
      realGradient: {
        leaderboardRows: [],
      },
    })
    expect(layer.contributors).toEqual([])
    expect(layer.entities).toEqual([])
    expect(layer.beams).toEqual([])
    expect(layer.bursts).toEqual([])
    expect(layer.lossCurve).toEqual([])
  })

  it('keeps the main view loss-free even if internal loss points appear before the product is ready', () => {
    const options = tassadarRunVisualizationOptions({
      realGradient: {
        lossCurve: [
          { step: 0, validationLoss: 3.2 },
          { step: 1, validationLoss: 2.9 },
        ],
      },
    })
    expect(options.lossCurve).toEqual([])
    expect(options.sceneChrome?.lossPanel).toBe('hidden')
  })

  it('separates crowded row-backed world entities with the shared spatial layout helper', () => {
    const entities = applyWorldEntitySpatialLayout([
      {
        id: 'station.pylon.world.one',
        label: 'P1 hub',
        position: [-2, 0, 0.2],
        status: 'registered',
      },
      {
        id: 'avatar.pylon_agent.pylon.world.one',
        label: 'P1 agent',
        position: [-2.05, 0.03, 0.28],
        status: 'idle',
      },
    ])

    const [station, avatar] = entities
    expect(station?.position).toBeDefined()
    expect(avatar?.position).toBeDefined()
    const distance = Math.hypot(
      (station?.position?.[0] ?? 0) - (avatar?.position?.[0] ?? 0),
      (station?.position?.[1] ?? 0) - (avatar?.position?.[1] ?? 0),
    )
    expect(distance).toBeGreaterThanOrEqual(0.82)
  })

  it('maps Cloudflare world projection rows through the existing summary visualization shape', () => {
    const fromWorld = cloudflareWorldSummaryFromRows(populated, {
      agentAvatars: [
        {
          actorKind: 'pylon_agent',
          actorRef: 'pylon_agent.pylon.world.one',
          avatarRef: 'avatar.pylon_agent.pylon.world.one',
          displayName: 'P1 agent',
          homePylonRef: 'pylon.world.one',
          publicProfileUrl: undefined,
        } as never,
        {
          actorKind: 'guest',
          actorRef: 'identity.viewer.one',
          avatarRef: 'avatar.viewer.one',
          displayName: 'Viewer one',
          homePylonRef: undefined,
          publicProfileUrl: undefined,
        } as never,
      ],
      avatarPositions: [
        {
          avatarRef: 'avatar.pylon_agent.pylon.world.one',
          movementMode: 'idle',
          pitch: 0,
          positionX: -1.9,
          positionY: 0,
          positionZ: 0,
          regionRef: 'region.run.tassadar.executor.20260615.main',
          yaw: 0,
        } as never,
        {
          avatarRef: 'avatar.viewer.one',
          movementMode: 'walking',
          pitch: 0.1,
          positionX: -2.1,
          positionY: 0,
          positionZ: 0.1,
          regionRef: 'region.run.tassadar.executor.20260615.main',
          yaw: -0.2,
        } as never,
      ],
      proofRefs: [
        {
          entityRef: 'worker.world.1',
          proofKind: 'verified_replay_ref',
          proofRef: 'challenge.world.1',
          runRef: 'run.tassadar.executor.20260615',
          title: 'challenge world',
          url: '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=challenge.world.1',
        } as never,
      ],
      worldRegions: [
        {
          avatarPositionMinIntervalMs: 100n,
          label: 'Tassadar main run space',
          localOriginX: 0,
          localOriginY: 0,
          localOriginZ: 0,
          maxX: 160,
          maxY: 40,
          maxZ: 160,
          minX: -160,
          minY: 0,
          minZ: -160,
          proximityRadiusMeters: 12,
          regionRef: 'region.run.tassadar.executor.20260615.main',
          roadDirectionX: 0,
          roadDirectionY: 0,
          roadDirectionZ: 1,
          runRef: 'run.tassadar.executor.20260615',
          staleAvatarPositionMs: 20_000n,
          starterPylonSiteOffsetX: 24,
          starterPylonSiteOffsetY: 0,
          starterPylonSiteOffsetZ: 0,
          streetNextRegionRef: 'region.run.tassadar.executor.20260615.street.next',
          streetPrevRegionRef: 'region.run.tassadar.executor.20260615.street.prev',
        } as never,
      ],
      pylonStations: [
        {
          interactionRadiusMeters: 2.4,
          label: 'P1',
          positionX: -2.35,
          positionY: 0,
          positionZ: 0,
          pylonRef: 'pylon.world.one',
          regionRef: 'region.run.tassadar.executor.20260615.main',
          runRef: 'run.tassadar.executor.20260615',
          sourceUrl:
            '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=pylon.world.one',
        } as never,
      ],
      pylonAttention: [
        {
          attentionKind: 'nearby',
          attentionRef: 'attention.pylon.world.one.avatar.viewer.one',
          avatarRef: 'avatar.viewer.one',
          distanceMeters: 0.35,
          pylonRef: 'pylon.world.one',
          sourceEntityRef: undefined,
        } as never,
      ],
      localChatMessages: [
        {
          body: 'hello nearby agents',
          bodyFormat: 'plain_text',
          channelKind: 'local',
          messageRef: 'message.local.1',
          moderationState: 'visible',
          radiusMeters: 8,
          regionRef: 'region.run.tassadar.executor.20260615.main',
          speakerAvatarRef: 'avatar.viewer.one',
          targetRef: undefined,
        } as never,
        {
          body: 'checking your receipt',
          bodyFormat: 'plain_text',
          channelKind: 'pylon',
          messageRef: 'message.pylon.1',
          moderationState: 'visible',
          radiusMeters: 8,
          regionRef: 'region.run.tassadar.executor.20260615.main',
          speakerAvatarRef: 'avatar.viewer.one',
          targetRef: 'pylon.world.one',
        } as never,
      ],
      chatBubbles: [
        {
          anchorEntityRef: 'avatar.viewer.one',
          bubbleRef: 'bubble.message.local.1',
          messageRef: 'message.local.1',
          speakerAvatarRef: 'avatar.viewer.one',
        } as never,
        {
          anchorEntityRef: 'pylon.world.one',
          bubbleRef: 'bubble.message.pylon.1',
          messageRef: 'message.pylon.1',
          speakerAvatarRef: 'avatar.viewer.one',
        } as never,
      ],
      runEntities: [
        {
          entityKind: 'pylon',
          entityRef: 'pylon.world.one',
          label: 'P1',
          lane: 'pylon',
          proofCount: 1,
          runRef: 'run.tassadar.executor.20260615',
          sourceRef: 'training.lease.world.one',
          status: 'simulation_settled',
        } as never,
        {
          entityKind: 'verified_replay_worker',
          entityRef: 'worker.world.1',
          label: 'W1',
          lane: 'verified_replay',
          proofCount: 1,
          runRef: 'run.tassadar.executor.20260615',
          sourceRef: 'challenge.world.1',
          status: 'verified',
        } as never,
        {
          entityKind: 'verified_replay_validator',
          entityRef: 'validator.world.1',
          label: 'V1',
          lane: 'verified_replay',
          proofCount: 1,
          runRef: 'run.tassadar.executor.20260615',
          sourceRef: 'challenge.world.1',
          status: 'verified',
        } as never,
        {
          entityKind: 'settlement_receipt',
          entityRef: 'receipt.nexus.world.1',
          label: '5s',
          lane: 'settlement',
          proofCount: 1,
          runRef: 'run.tassadar.executor.20260615',
          sourceRef: 'receipt.nexus.world.1',
          status: 'simulation_settled',
        } as never,
        {
          entityKind: 'accepted_trace',
          entityRef: 'trace.world.1',
          label: 'T1',
          lane: 'trace',
          proofCount: 1,
          runRef: 'run.tassadar.executor.20260615',
          sourceRef: 'trace.world.1',
          status: 'accepted_trace',
        } as never,
      ],
      settlementRefs: [
        {
          amountSats: 5,
          entityRef: 'receipt.nexus.world.1',
          movementMode: 'simulation',
          realBitcoinMoved: false,
          receiptRef: 'receipt.nexus.world.1',
          runRef: 'run.tassadar.executor.20260615',
          settlementRef: 'receipt.nexus.world.1',
          url: '/api/public/nexus-pylon/receipts/receipt.nexus.world.1',
        } as never,
      ],
      trainingRuns: [
        {
          maxStalenessSeconds: 0,
          publicSummaryHash: 'hash.world',
          runRef: 'run.tassadar.executor.20260615',
          runState: 'active',
          sourceGeneratedAt: '2026-06-17T20:43:06.789Z',
          sourceUrl: 'https://openagents.com/api/public/tassadar-run-summary',
          stalenessKind: 'live_at_read',
        } as never,
      ],
      worldEdges: [
        {
          edgeKind: 'pylon_to_settlement',
          edgeRef: 'edge.world.1',
          fromEntityRef: 'pylon.world.one',
          runRef: 'run.tassadar.executor.20260615',
          sourceRef: 'receipt.nexus.world.1',
          toEntityRef: 'receipt.nexus.world.1',
        } as never,
      ],
      worldEvents: [
        {
          entityRef: 'worker.world.1',
          eventKind: 'verified_replay_pair',
          eventRef: 'world_event.world.1',
          runRef: 'run.tassadar.executor.20260615',
          sourceGeneratedAt: '2026-06-17T20:43:06.789Z',
          sourceRef: 'challenge.world.1',
          summary: 'Verified replay pair 1',
        } as never,
        {
          entityRef: 'pylon.world.one',
          eventKind: 'pylon_heartbeat',
          eventRef: 'world_event.public_activity.pylon_heartbeat.1',
          runRef: 'run.public_activity_timeline',
          sourceGeneratedAt: '2026-06-18T18:00:02.000Z',
          sourceRef: 'pylon.world.one',
          summary: activitySummary({}),
        } as never,
      ],
    })

    expect(fromWorld.generatedAt).toBe('2026-06-17T20:43:06.789Z')
    expect(fromWorld.realGradient?.leaderboardRows?.[0]).toMatchObject({
      pylonRef: 'pylon.world.one',
      rank: 1,
      verifiedWindowCount: 1,
    })
    expect(fromWorld.realGradient?.verifiedReplayPairs?.[0]).toMatchObject({
      challengeRef: 'challenge.world.1',
      validatorRef: 'validator.world.1',
      workerRef: 'worker.world.1',
    })
    expect(fromWorld.settlementRows?.[0]).toMatchObject({
      amountSats: 5,
      contributorRef: 'pylon.world.one',
      movementMode: 'simulation',
      realBitcoinMoved: false,
      receiptRef: 'receipt.nexus.world.1',
      state: 'settled',
    })
    expect(fromWorld.corpus?.traceRefs).toEqual(['trace.world.1'])
    expect(fromWorld.world?.pylonStations).toEqual([
      {
        interactionRadiusMeters: 2.4,
        label: 'P1',
        position: { x: -2.35, y: 0, z: 0 },
        pylonRef: 'pylon.world.one',
        regionRef: 'region.run.tassadar.executor.20260615.main',
        sourceUrl:
          '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=pylon.world.one',
      },
    ])
    expect(fromWorld.world?.worldRegions).toEqual([
      {
        avatarPositionMinIntervalMs: 100,
        bounds: {
          maxX: 160,
          maxY: 40,
          maxZ: 160,
          minX: -160,
          minY: 0,
          minZ: -160,
        },
        label: 'Tassadar main run space',
        localOrigin: { x: 0, y: 0, z: 0 },
        proximityRadiusMeters: 12,
        regionRef: 'region.run.tassadar.executor.20260615.main',
        roadDirection: { x: 0, y: 0, z: 1 },
        runRef: 'run.tassadar.executor.20260615',
        staleAvatarPositionMs: 20000,
        starterPylonSiteOffset: { x: 24, y: 0, z: 0 },
        streetNextRegionRef: 'region.run.tassadar.executor.20260615.street.next',
        streetPrevRegionRef: 'region.run.tassadar.executor.20260615.street.prev',
      },
    ])
    expect(fromWorld.world?.agentAvatars).toEqual([
      {
        actorKind: 'pylon_agent',
        avatarRef: 'avatar.pylon_agent.pylon.world.one',
        displayName: 'P1 agent',
        homePylonRef: 'pylon.world.one',
      },
      {
        actorKind: 'guest',
        avatarRef: 'avatar.viewer.one',
        displayName: 'Viewer one',
      },
    ])
    expect(fromWorld.world?.avatarPositions).toEqual([
      {
        avatarRef: 'avatar.pylon_agent.pylon.world.one',
        movementMode: 'idle',
        pitch: 0,
        position: { x: -1.9, y: 0, z: 0 },
        regionRef: 'region.run.tassadar.executor.20260615.main',
        yaw: 0,
      },
      {
        avatarRef: 'avatar.viewer.one',
        movementMode: 'walking',
        pitch: 0.1,
        position: { x: -2.1, y: 0, z: 0.1 },
        regionRef: 'region.run.tassadar.executor.20260615.main',
        yaw: -0.2,
      },
    ])
    expect(fromWorld.world?.pylonAttention).toEqual([
      {
        attentionKind: 'nearby',
        attentionRef: 'attention.pylon.world.one.avatar.viewer.one',
        avatarRef: 'avatar.viewer.one',
        distanceMeters: 0.35,
        pylonRef: 'pylon.world.one',
      },
    ])
    expect(fromWorld.world?.localChatMessages).toEqual([
      {
        body: 'hello nearby agents',
        channelKind: 'local',
        messageRef: 'message.local.1',
        moderationState: 'visible',
        radiusMeters: 8,
        regionRef: 'region.run.tassadar.executor.20260615.main',
        speakerAvatarRef: 'avatar.viewer.one',
      },
      {
        body: 'checking your receipt',
        channelKind: 'pylon',
        messageRef: 'message.pylon.1',
        moderationState: 'visible',
        radiusMeters: 8,
        regionRef: 'region.run.tassadar.executor.20260615.main',
        speakerAvatarRef: 'avatar.viewer.one',
        targetRef: 'pylon.world.one',
      },
    ])
    expect(fromWorld.world?.chatBubbles).toEqual([
      {
        anchorEntityRef: 'avatar.viewer.one',
        bubbleRef: 'bubble.message.local.1',
        messageRef: 'message.local.1',
        speakerAvatarRef: 'avatar.viewer.one',
      },
      {
        anchorEntityRef: 'pylon.world.one',
        bubbleRef: 'bubble.message.pylon.1',
        messageRef: 'message.pylon.1',
        speakerAvatarRef: 'avatar.viewer.one',
      },
    ])
    expect(fromWorld.world?.activityMotions).toEqual([
      {
        atId: 'pylon.world.one',
        cursor: '2026-06-18T18:00:02.000Z:pylon_presence:pylon.world.one',
        eventRef: 'pylon.world.one',
        expiresAt: '2026-06-18T18:05:02.000Z',
        generatedAt: '2026-06-18T18:00:02.000Z',
        motionId: 'world_event.public_activity.pylon_heartbeat.1',
        motionKind: 'pylon_heartbeat',
        sourceKind: 'pylon_presence',
        sourceLagStatus: 'stale',
        sourceRefs: ['pylon.world.one', 'route:/api/public/pylon-stats'],
        text: 'Pylon heartbeat observed in the public activity timeline.',
      },
    ])

    const options = tassadarRunVisualizationOptions(fromWorld)
    expect(options.entities).toContainEqual({
      id: 'pylon.world.one',
      label: 'P1',
      position: [-2.35, 0, 0.12],
      status: 'simulation_settled',
    })
    expect(options.entities).toContainEqual({
      id: 'station.pylon.world.one',
      label: 'P1 hub +1',
      position: [-3.045, -0.293, 0.2],
      status: 'nearby',
    })
    expect(options.entities).toContainEqual({
      id: 'avatar.pylon_agent.pylon.world.one',
      label: 'P1 agent',
      position: [-1.398, -0.284, 0.28],
      status: 'idle',
    })
    expect(options.entities).toContainEqual({
      id: 'avatar.viewer.one',
      label: 'Viewer one',
      position: [-2.227, 0.557, 0.28],
      status: 'walking',
    })
    expect(options.entities).toContainEqual({
      id: 'bubble.message.local.1',
      label: 'hello nearby agents',
      position: [-2.227, 0.877, 0.38],
      status: 'chat',
    })
    expect(options.entities).toContainEqual({
      id: 'bubble.message.pylon.1.speaker',
      label: 'checking your receipt',
      position: [-2.227, 0.877, 0.38],
      status: 'chat',
    })
    expect(options.entities).toContainEqual({
      id: 'bubble.message.pylon.1',
      label: 'checking your receipt',
      position: [-3.045, 0.027, 0.38],
      status: 'talking_to_pylon',
    })
    expect(options.entities).toContainEqual({
      id: 'worker.world.1',
      label: 'W1',
      position: [0, 2.05, 0.12],
      status: 'verified',
    })
    expect(options.entities).toContainEqual({
      id: 'receipt.nexus.world.1',
      label: '5s',
      position: [-1.15, -2.2, 0.12],
      status: 'simulation_settled',
    })
    expect(options.beams).toEqual([])
    expect(options.bursts).toEqual([
      {
        atId: 'pylon.world.one',
        cursor: '2026-06-18T18:00:02.000Z:pylon_presence:pylon.world.one',
        eventRef: 'pylon.world.one',
        expiresAt: '2026-06-18T18:05:02.000Z',
        generatedAt: '2026-06-18T18:00:02.000Z',
        motionId: 'world_event.public_activity.pylon_heartbeat.1',
        motionKind: 'pylon_heartbeat',
        simulated: false,
        sourceKind: 'pylon_presence',
        sourceLagStatus: 'stale',
        sourceRefs: ['pylon.world.one', 'route:/api/public/pylon-stats'],
      },
    ])
    expect(options.contributors).toEqual([])
  })

  it('does not render avatar entities without matching avatar_position rows', () => {
    const options = tassadarRunVisualizationOptions({
      world: {
        agentAvatars: [
          {
            actorKind: 'pylon_agent',
            avatarRef: 'avatar.pylon_agent.missing_position',
            displayName: 'Missing position',
            homePylonRef: 'pylon.world.missing',
          },
        ],
        pylonStations: [
          {
            interactionRadiusMeters: 2.4,
            label: 'P1',
            position: { x: -2.35, y: 0, z: 0 },
            pylonRef: 'pylon.world.missing',
            regionRef: 'region.run.tassadar.executor.20260615.main',
            sourceUrl:
              '/api/public/training/runs/run.tassadar.executor.20260615?focusRef=pylon.world.missing',
          },
        ],
      },
    })

    expect(
      options.entities?.some(
        entity => entity.id === 'station.pylon.world.missing',
      ),
    ).toBe(true)
    expect(
      options.entities?.some(
        entity => entity.id === 'avatar.pylon_agent.missing_position',
      ),
    ).toBe(false)
  })

  it('does not animate activity rows without public refs or liveness metadata', () => {
    const summary = cloudflareWorldSummaryFromRows(
      {
        realGradient: {
          leaderboardRows: [
            {
              pylonRef: 'pylon.world.one',
              rank: 1,
              sourceRefs: ['training.lease.world.one'],
              verifiedWindowCount: 1,
            },
          ],
        },
        runRef: 'run.tassadar.executor.20260615',
      },
      {
        worldEvents: [
          {
            entityRef: 'pylon.world.one',
            eventKind: 'pylon_heartbeat',
            eventRef: 'world_event.public_activity.missing_refs',
            runRef: 'run.public_activity_timeline',
            sourceGeneratedAt: '2026-06-18T18:00:02.000Z',
            sourceRef: '',
            summary: activitySummary({ sourceRefs: [] }),
          } as never,
          {
            entityRef: 'pylon.world.one',
            eventKind: 'pylon_heartbeat',
            eventRef: 'world_event.public_activity.missing_liveness',
            runRef: 'run.public_activity_timeline',
            sourceGeneratedAt: '',
            sourceRef: 'pylon.world.one',
            summary: activitySummary({
              expiresAt: '',
              generatedAt: '',
              sourceLagStatus: '',
            }),
          } as never,
          {
            entityRef: 'pylon.world.one',
            eventKind: 'pylon_heartbeat',
            eventRef: 'world_event.public_activity.private_payload',
            runRef: 'run.public_activity_timeline',
            sourceGeneratedAt: '2026-06-18T18:00:02.000Z',
            sourceRef: 'pylon.world.one',
            summary: activitySummary({
              text: 'raw_prompt customer_email@example.com sk-test-private',
            }),
          } as never,
        ],
      },
    )
    const options = tassadarRunVisualizationOptions(summary)

    expect(summary.world?.activityMotions).toBeUndefined()
    expect(options.entities).toContainEqual({
      id: 'pylon.world.one',
      label: 'P1',
      position: [-2.35, 0, 0.12],
      status: 'verified',
    })
    expect(options.beams).toEqual([])
    expect(options.bursts).toEqual([])
  })
})
