import { describe, expect, it } from 'vitest'

import {
  type TassadarRunPublicSummary,
  tassadarRunVisualizationOptions,
  trainingRunEntityLayerFromPublicSummary,
  trainingRunSnapshotFromPublicSummary,
} from './tassadarRunSnapshot'

const m = (value: number) => ({
  value,
  provenanceLabel: 'observed',
  sourceRefs: [],
})

const populated: TassadarRunPublicSummary = {
  runRef: 'run.tassadar.executor.20260615',
  runLabel: 'Tassadar executor run',
  runState: 'active',
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
  },
  receiptRefs: ['receipt.pylon.settlement.1'],
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
  })

  it('adds data-bound pylon contributors/entities, verified replay beams, and settlement bursts', () => {
    const layer = trainingRunEntityLayerFromPublicSummary(populated)
    expect(layer.contributors).toEqual([
      {
        id: 'pylon.worker.one',
        label: 'P1',
        lifecycleState: 'active',
        phase: 0,
      },
      {
        id: 'pylon.worker.two',
        label: 'P2',
        lifecycleState: 'active',
        phase: 0.5,
      },
    ])
    expect(layer.entities).toEqual([
      { id: 'pylon.worker.one', label: 'P1', status: 'verified' },
      { id: 'pylon.worker.two', label: 'P2', status: 'settled' },
      { id: 'contribution.tassadar.worker.1', label: 'W1', status: 'verified' },
      { id: 'validator.tassadar.1', label: 'V1', status: 'verified' },
    ])
    expect(layer.beams).toEqual([
      {
        fromId: 'contribution.tassadar.worker.1',
        toId: 'validator.tassadar.1',
      },
    ])
    expect(layer.bursts).toEqual([{ atId: 'pylon.worker.two' }])
    expect(layer.lossCurve).toEqual([])
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

  it('passes through public loss curve points only when the summary supplies them', () => {
    const options = tassadarRunVisualizationOptions({
      realGradient: {
        lossCurve: [
          { step: 0, validationLoss: 3.2 },
          { step: 1, validationLoss: 2.9 },
        ],
      },
    })
    expect(options.lossCurve).toEqual([
      { step: 0, validationLoss: 3.2 },
      { step: 1, validationLoss: 2.9 },
    ])
  })
})
