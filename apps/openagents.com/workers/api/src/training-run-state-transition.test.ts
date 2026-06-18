import { describe, expect, it } from 'vitest'

import {
  TrainingAuthorityStoreError,
  type TrainingRunManifest,
  TrainingRunPlannedWithReconciledWindowsBlocker,
  type TrainingWindowRecord,
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
  publicTrainingRunProjection,
  publicTrainingRunSummary,
  transitionTrainingRunRecord,
  transitionTrainingWindowRecord,
} from './training-run-window-authority'

const nowIso = '2026-06-14T10:00:00.000Z'

const manifest: TrainingRunManifest = {
  blockerRefs: [
    'blocker.training.monday_launch_self_serve_stranger_payout_pending',
  ],
  objective: 'Grow the Tassadar verified-trace corpus via paid executor-trace work.',
  paymentMode: 'operator_approved_small_sats',
  settlementState: 'pending',
  verifierPolicy: 'exact_trace_replay',
  workloadFamily: 'executor-trace',
}

const makeTassadarRun = () =>
  buildTrainingRunRecord({
    makeId: () => 'tassadar',
    nowIso,
    request: {
      manifest,
      promiseRef: 'training.decentralized_training_launch.v1',
      receiptRefs: [],
      sourceRefs: [],
      trainingRunRef: 'run.tassadar.executor.20260615',
    },
  })

const reconciledWindow = (trainingRunRef: string): TrainingWindowRecord => {
  const planned = buildTrainingWindowRecord({
    makeId: () => 'w1',
    nowIso,
    request: { trainingRunRef },
  })
  const active = transitionTrainingWindowRecord({
    actorRef: 'op',
    eventId: 'e1',
    nextState: 'active',
    nowIso,
    receiptRef: 'receipt.window.activate.1',
    transitionKind: 'window_activate',
    window: planned,
  }).window
  const sealed = transitionTrainingWindowRecord({
    actorRef: 'op',
    eventId: 'e2',
    nextState: 'sealed',
    nowIso,
    receiptRef: 'receipt.window.seal.1',
    transitionKind: 'window_seal',
    window: active,
  }).window

  return transitionTrainingWindowRecord({
    actorRef: 'op',
    eventId: 'e3',
    nextState: 'reconciled',
    nowIso,
    receiptRef: 'receipt.window.reconcile.1',
    transitionKind: 'window_reconcile',
    window: sealed,
  }).window
}

describe('training run state transition (#5006)', () => {
  it('builds the run planned and moves it planned -> active receipt-first', () => {
    const run = makeTassadarRun()

    expect(run.state).toBe('planned')

    const { run: active } = transitionTrainingRunRecord({
      nextState: 'active',
      nowIso,
      receiptRef: 'receipt.run.transition.activate.1',
      run,
    })

    expect(active.state).toBe('active')
    expect(active.receiptRefs).toContain('receipt.run.transition.activate.1')
    expect(publicTrainingRunProjection(active, nowIso).state).toBe('active')
  })

  it('rejects an illegal run transition (planned -> sealed)', () => {
    expect(() =>
      transitionTrainingRunRecord({
        nextState: 'sealed',
        nowIso,
        receiptRef: 'receipt.run.transition.bad.1',
        run: makeTassadarRun(),
      }),
    ).toThrow(TrainingAuthorityStoreError)
  })

  it('projects manifest fields, a live-at-read staleness contract, and manifest blockers', () => {
    const projection = publicTrainingRunProjection(makeTassadarRun(), nowIso)

    expect(projection.manifest?.workloadFamily).toBe('executor-trace')
    expect(projection.manifest?.verifierPolicy).toBe('exact_trace_replay')
    expect(projection.manifest?.paymentMode).toBe('operator_approved_small_sats')
    expect(projection.generatedAt).toBe(nowIso)
    expect(projection.maxStalenessSeconds).toBe(0)
    expect(projection.staleness.composition).toBe('live_at_read')
    expect(projection.blockers).toContain(
      'blocker.training.monday_launch_self_serve_stranger_payout_pending',
    )
  })

  it('raises a typed caveat when a run is planned but a window has reconciled', () => {
    const run = makeTassadarRun()
    const summary = publicTrainingRunSummary({
      challenges: [],
      leases: [],
      nowIso,
      run,
      windows: [reconciledWindow(run.trainingRunRef)],
    })

    expect(summary.run.state).toBe('planned')
    expect(summary.run.blockers).toContain(
      TrainingRunPlannedWithReconciledWindowsBlocker,
    )
  })

  it('drops the planned-with-reconciled caveat once the run is active', () => {
    const run = transitionTrainingRunRecord({
      nextState: 'active',
      nowIso,
      receiptRef: 'receipt.run.transition.activate.1',
      run: makeTassadarRun(),
    }).run
    const summary = publicTrainingRunSummary({
      challenges: [],
      leases: [],
      nowIso,
      run,
      windows: [reconciledWindow(run.trainingRunRef)],
    })

    expect(summary.run.state).toBe('active')
    expect(summary.run.blockers).not.toContain(
      TrainingRunPlannedWithReconciledWindowsBlocker,
    )
  })
})
