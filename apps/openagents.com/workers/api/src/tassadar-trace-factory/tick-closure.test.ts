import { describe, expect, test } from 'vitest'

import {
  closeTick,
  trainingRecordRefFromClosedTick,
  type TassadarTickFaces,
} from './tick-closure'

const fullFaces: TassadarTickFaces = {
  evaluation: {
    outcome: 'verified',
    tier: 1,
    verdictRef: 'verdict.trace_abc.tier1',
  },
  execution: {
    executorHash: 'e'.repeat(64),
    fullTraceDigest: 'f'.repeat(64),
    stepCount: 256,
  },
  intent: {
    assignmentRef: 'assignment.trace_factory.1',
    declaredStepCount: 256,
    familyId: 'family.arithmetic_carry.v1',
    inputSeed: 'a1b2c3d4e5f60718',
  },
  stateDelta: {
    admittedTo: 'corpus',
    recordId: 'trace_abc',
    tokenCount: 6144,
  },
}

describe('tick closure v0.1 (the tetrahedron acceptance predicate)', () => {
  test('a tick closes only when intent, execution, state delta, and evaluation all close', () => {
    const closed = closeTick(fullFaces)
    expect(closed.closed).toBe(true)

    for (const face of [
      'intent',
      'execution',
      'stateDelta',
      'evaluation',
    ] as const) {
      const open = closeTick({ ...fullFaces, [face]: null })
      expect(open.closed).toBe(false)
      if (!open.closed) expect(open.openFaces).toHaveLength(1)
    }

    const fullyOpen = closeTick({
      evaluation: null,
      execution: null,
      intent: null,
      stateDelta: null,
    })
    expect(fullyOpen.closed).toBe(false)
    if (!fullyOpen.closed) {
      expect(fullyOpen.openFaces).toEqual([
        'intent',
        'execution',
        'state_delta',
        'evaluation',
      ])
    }
  })

  test('closed verified corpus-admitted ticks ARE training records', () => {
    const closed = closeTick(fullFaces)
    if (!closed.closed) throw new globalThis.Error('expected closure')
    const ref = trainingRecordRefFromClosedTick(closed.tick)
    expect(ref.ok).toBe(true)
    if (ref.ok) {
      expect(ref.trainingRecordRef).toBe(
        'training_record.trace_abc.verdict.trace_abc.tier1',
      )
    }
  })

  test('a closed tick whose evaluation rejected the work never mints a training record', () => {
    const closed = closeTick({
      ...fullFaces,
      evaluation: {
        outcome: 'rejected',
        tier: 1,
        verdictRef: 'verdict.trace_abc.tier1',
      },
      stateDelta: { ...fullFaces.stateDelta!, admittedTo: 'quarantine' },
    })
    if (!closed.closed) throw new globalThis.Error('expected closure')
    const ref = trainingRecordRefFromClosedTick(closed.tick)
    expect(ref.ok).toBe(false)
    if (!ref.ok) expect(ref.reason).toBe('evaluation_rejected')
  })

  test('a verified tick still quarantined never mints a training record', () => {
    const closed = closeTick({
      ...fullFaces,
      stateDelta: { ...fullFaces.stateDelta!, admittedTo: 'quarantine' },
    })
    if (!closed.closed) throw new globalThis.Error('expected closure')
    const ref = trainingRecordRefFromClosedTick(closed.tick)
    expect(ref.ok).toBe(false)
    if (!ref.ok) expect(ref.reason).toBe('not_admitted_to_corpus')
  })
})
