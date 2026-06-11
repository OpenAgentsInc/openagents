import { describe, expect, test } from 'vitest'

import {
  emptyFactoryProjection,
  projectionRebuildCompliance,
  rebuildFactoryProjection,
  tassadarFactoryReferenceProjection,
  type TassadarFactoryProjectionEvent,
  type TassadarFactoryProjectionModule,
} from './projection-rebuild'

const registration = (
  recordId: string,
  tokenCount: number,
): TassadarFactoryProjectionEvent => ({
  familyId: 'family.arithmetic_carry.v1',
  kind: 'record_registered',
  occurredAtIso: '2026-06-11T01:00:00.000Z',
  recordId,
  tokenCount,
})

const verifiedTransition = (
  recordId: string,
  tokenCount: number,
  occurredAtIso: string,
): TassadarFactoryProjectionEvent => ({
  familyId: 'family.arithmetic_carry.v1',
  fromStatus: 'quarantined',
  kind: 'validation_transition',
  occurredAtIso,
  recordId,
  tokenCount,
  toStatus: 'verified',
  verdictRef: `verdict.${recordId}`,
})

describe('projection-rebuild rules v0.1 (case law: #4744, #4745, #4746)', () => {
  test('registration events move intake counts but never the public verified counters or the rebuild timestamp', () => {
    const projection = rebuildFactoryProjection([
      registration('trace_a', 100),
      registration('trace_b', 200),
    ])
    expect(projection.registeredRecords).toBe(2)
    expect(projection.quarantinedRecords).toBe(2)
    expect(projection.verifiedRecords).toBe(0)
    expect(projection.verifiedTokens).toBe(0)
    expect(projection.familyCoverage).toEqual([])
    expect(projection.validationRate).toBeNull()
    expect(projection.rebuiltAtIso).toBeNull()
  })

  test('validation transitions move the public counters, family coverage, validation rate, and the rebuild timestamp', () => {
    const projection = rebuildFactoryProjection([
      registration('trace_a', 100),
      registration('trace_b', 200),
      registration('trace_c', 300),
      verifiedTransition('trace_a', 100, '2026-06-11T01:01:00.000Z'),
      verifiedTransition('trace_b', 200, '2026-06-11T01:02:00.000Z'),
      {
        familyId: 'family.arithmetic_carry.v1',
        fromStatus: 'quarantined',
        kind: 'validation_transition',
        occurredAtIso: '2026-06-11T01:03:00.000Z',
        recordId: 'trace_c',
        tokenCount: 300,
        toStatus: 'rejected',
        verdictRef: 'verdict.trace_c',
      },
    ])
    expect(projection.verifiedRecords).toBe(2)
    expect(projection.verifiedTokens).toBe(300)
    expect(projection.rejectedRecords).toBe(1)
    expect(projection.quarantinedRecords).toBe(0)
    expect(projection.validationRate).toBeCloseTo(2 / 3)
    expect(projection.familyCoverage).toEqual([
      {
        familyId: 'family.arithmetic_carry.v1',
        verifiedRecords: 2,
        verifiedTokens: 300,
      },
    ])
    expect(projection.rebuiltAtIso).toBe('2026-06-11T01:03:00.000Z')
  })

  test('a later revocation transition (verified -> rejected) is reflected, not frozen', () => {
    const projection = rebuildFactoryProjection([
      registration('trace_a', 100),
      verifiedTransition('trace_a', 100, '2026-06-11T01:01:00.000Z'),
      {
        familyId: 'family.arithmetic_carry.v1',
        fromStatus: 'verified',
        kind: 'validation_transition',
        occurredAtIso: '2026-06-11T02:00:00.000Z',
        recordId: 'trace_a',
        tokenCount: 100,
        toStatus: 'rejected',
        verdictRef: 'verdict.trace_a.adversarial',
      },
    ])
    expect(projection.verifiedRecords).toBe(0)
    expect(projection.verifiedTokens).toBe(0)
    expect(projection.rejectedRecords).toBe(1)
    expect(projection.rebuiltAtIso).toBe('2026-06-11T02:00:00.000Z')
  })

  test('the reference projection module is compliant', () => {
    expect(
      projectionRebuildCompliance(tassadarFactoryReferenceProjection),
    ).toEqual([])
  })

  test('the compliance checker catches the #4744 class: a projection frozen at registration time', () => {
    const frozenAtRegistration: TassadarFactoryProjectionModule = {
      contractVersion: 'projection_rebuild.v0.1',
      projectionId: 'projection.frozen_at_registration',
      // claims the right trigger but ignores transitions entirely
      rebuild: events => {
        const registrations = events.filter(
          event => event.kind === 'record_registered',
        )

        return {
          ...emptyFactoryProjection(),
          registeredRecords: registrations.length,
        }
      },
      rebuildTriggers: ['validation_transition'],
    }
    const violations = projectionRebuildCompliance(frozenAtRegistration)
    expect(
      violations.some(
        violation => violation.kind === 'public_counter_frozen_on_transition',
      ),
    ).toBe(true)
    expect(
      violations.some(
        violation => violation.kind === 'rebuilt_at_frozen_on_transition',
      ),
    ).toBe(true)
  })

  test('the compliance checker catches a projection that counts registrations as verified work', () => {
    const eagerProjection: TassadarFactoryProjectionModule = {
      contractVersion: 'projection_rebuild.v0.1',
      projectionId: 'projection.counts_registrations',
      rebuild: events => {
        const projection = rebuildFactoryProjection(events)
        const registrations = events.filter(
          event => event.kind === 'record_registered',
        )

        return {
          ...projection,
          // the bug class: public counter inflated by intake
          verifiedRecords: projection.verifiedRecords + registrations.length,
        }
      },
      rebuildTriggers: ['validation_transition'],
    }
    const violations = projectionRebuildCompliance(eagerProjection)
    expect(
      violations.some(
        violation =>
          violation.kind === 'public_counter_moved_on_registration' &&
          violation.counter === 'verifiedRecords',
      ),
    ).toBe(true)
  })
})
