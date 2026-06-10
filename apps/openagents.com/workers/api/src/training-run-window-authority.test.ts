import { describe, expect, it } from 'vitest'

import {
  TrainingAuthorityStoreError,
  buildTrainingWindowRecord,
  selectTrainingLeaseCandidate,
  transitionTrainingWindowRecord,
} from './training-run-window-authority'

describe('training run window authority', () => {
  it('prefers admin-dispatched homework before auto-launched starter windows', () => {
    const starter = buildTrainingWindowRecord({
      makeId: () => 'starter',
      nowIso: '2026-06-10T10:00:00.000Z',
      request: {
        homeworkKind: 'auto_starter',
        priority: 100,
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.starter',
      },
    })
    const admin = buildTrainingWindowRecord({
      makeId: () => 'admin',
      nowIso: '2026-06-10T10:01:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        priority: 0,
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.admin',
      },
    })

    expect(
      selectTrainingLeaseCandidate([
        { ...starter, state: 'active' },
        { ...admin, state: 'active' },
      ])?.windowRef,
    ).toBe('training.window.admin')
  })

  it('requires planned active sealed reconciled window transition order', () => {
    const planned = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-10T10:00:00.000Z',
      request: {
        trainingRunRef: 'training.run.0001',
        windowRef: 'training.window.0001',
      },
    })
    const active = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'activate',
      nextState: 'active',
      nowIso: '2026-06-10T10:05:00.000Z',
      receiptRef: 'receipt.training.activate',
      transitionKind: 'window_activate',
      window: planned,
    }).window
    const sealed = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'seal',
      nextState: 'sealed',
      nowIso: '2026-06-10T10:10:00.000Z',
      receiptRef: 'receipt.training.seal',
      transitionKind: 'window_seal',
      window: active,
    }).window
    const reconciled = transitionTrainingWindowRecord({
      actorRef: 'operator.training',
      eventId: 'reconcile',
      nextState: 'reconciled',
      nowIso: '2026-06-10T10:15:00.000Z',
      receiptRef: 'receipt.training.reconcile',
      transitionKind: 'window_reconcile',
      window: sealed,
    }).window

    expect(reconciled.state).toBe('reconciled')
    expect(reconciled.receiptRefs).toEqual([
      'receipt.training.activate',
      'receipt.training.reconcile',
      'receipt.training.seal',
    ])
    expect(() =>
      transitionTrainingWindowRecord({
        actorRef: 'operator.training',
        eventId: 'invalid',
        nextState: 'reconciled',
        nowIso: '2026-06-10T10:20:00.000Z',
        receiptRef: 'receipt.training.invalid',
        transitionKind: 'window_reconcile',
        window: active,
      }),
    ).toThrow(TrainingAuthorityStoreError)
  })
})
