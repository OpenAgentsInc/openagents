import { describe, expect, test } from 'vitest'

import {
  SIGNATURE_SETTLEMENT_BLOCKER,
  SIGNATURE_USAGE_METERING_BLOCKER,
  SIGNATURE_USAGE_METERING_SCHEMA,
  SignatureUsageMeteringError,
  emptySignatureUsageMeteringStore,
  makeInMemorySignatureUsageMeteringStore,
  meteringExactUsageSubjectRefs,
  meteringUsageEventRefs,
  meteringUsageIdempotencyRefs,
  projectSignatureMeteringGate,
  projectSignatureUsageMetering,
  recordSignatureUsage,
} from './signature-usage-metering'
import {
  projectSignatureMarketplaceRevenueGate,
  signatureMarketplaceRevenueGateHasPrivateMaterial,
} from './signature-marketplace-revenue-gate'

const validationInput = {
  activationRefs: ['activation.public.signature_market.site_builder_v1'],
  packagePublicationRefs: [
    'publication.public.signature_market.site_builder_v1',
  ],
  packageValidationRefs: ['validation.public.signature_market.site_builder_v1'],
  packageRefs: ['package.public.signature_market.site_builder'],
  programSignatureRefs: ['program_signature.public.site_builder_v1'],
}

const recordOk = (input: {
  signatureSubjectRef: string
  packageRef: string
  idempotencyToken: string
}) => {
  const result = recordSignatureUsage(input)
  if (!result.ok) {
    throw new Error(`expected metering record, got error: ${result.error.reason}`)
  }
  return result.event
}

describe('signature usage metering — record model (#5529)', () => {
  test('records a public-safe, exact-subject-bound, idempotent usage event', () => {
    const event = recordOk({
      signatureSubjectRef: 'package_site_builder.version_v1',
      packageRef: 'package.public.signature_market.site_builder',
      idempotencyToken: 'usage-001',
    })

    expect(event.schema).toBe(SIGNATURE_USAGE_METERING_SCHEMA)
    expect(event.usageEventRef).toBe(
      'usage_event.public.signature_market.package_site_builder.version_v1.usage-001',
    )
    expect(event.usageIdempotencyRef).toBe(
      'usage_idempotency.public.signature_market.usage-001',
    )
    expect(event.exactUsageSubjectRef).toBe(
      'usage_subject.public.signature_market.package_site_builder.version_v1',
    )
  })

  test('derivation is deterministic for the same subject + idempotency token', () => {
    const a = recordOk({
      signatureSubjectRef: 'sig_a',
      packageRef: 'package.public.a',
      idempotencyToken: 'tok-1',
    })
    const b = recordOk({
      signatureSubjectRef: 'sig_a',
      packageRef: 'package.public.a',
      idempotencyToken: 'tok-1',
    })
    expect(b.usageEventRef).toBe(a.usageEventRef)
    expect(b.usageIdempotencyRef).toBe(a.usageIdempotencyRef)
    expect(b.exactUsageSubjectRef).toBe(a.exactUsageSubjectRef)
  })

  test('rejects empty and unsafe (payload/secret/wallet/timestamp) tokens', () => {
    for (const bad of [
      '',
      '   ',
      'usage payload with spaces',
      'sk-livesecret0',
      'lnbc10n1psecret',
      '2026-06-19T12:00:00',
      '/Users/private/material',
    ]) {
      const result = recordSignatureUsage({
        signatureSubjectRef: bad,
        packageRef: 'package.public.a',
        idempotencyToken: 'tok-1',
      })
      expect(result.ok).toBe(false)
      if (!result.ok) {
        expect(result.error).toBeInstanceOf(SignatureUsageMeteringError)
      }
    }
  })
})

describe('signature usage metering — idempotent store (#5529)', () => {
  test('collapses duplicate idempotency tokens to one event', () => {
    const event = recordOk({
      signatureSubjectRef: 'sig_a',
      packageRef: 'package.public.a',
      idempotencyToken: 'tok-1',
    })
    const store = makeInMemorySignatureUsageMeteringStore([event, event, event])
    expect(store.list()).toHaveLength(1)
    expect(meteringUsageEventRefs(store)).toHaveLength(1)
    expect(meteringUsageIdempotencyRefs(store)).toHaveLength(1)
    expect(meteringExactUsageSubjectRefs(store)).toHaveLength(1)
  })

  test('distinct idempotency tokens are kept separate', () => {
    const store = makeInMemorySignatureUsageMeteringStore([
      recordOk({
        signatureSubjectRef: 'sig_a',
        packageRef: 'package.public.a',
        idempotencyToken: 'tok-1',
      }),
      recordOk({
        signatureSubjectRef: 'sig_a',
        packageRef: 'package.public.a',
        idempotencyToken: 'tok-2',
      }),
    ])
    expect(store.list()).toHaveLength(2)
    expect(meteringUsageEventRefs(store)).toHaveLength(2)
  })
})

describe('signature usage metering — clears the revenue-gate metering rung (#5529)', () => {
  test('with publication + activation + metering and nothing past it, the gate reaches state "metered"', () => {
    const store = makeInMemorySignatureUsageMeteringStore([
      recordOk({
        signatureSubjectRef: 'package_site_builder.version_v1',
        packageRef: 'package.public.signature_market.site_builder',
        idempotencyToken: 'usage-001',
      }),
    ])

    const gate = projectSignatureMeteringGate(store, validationInput)

    // THE RECEIPT: metering output drives the gate from activated package usage to `metered`.
    expect(gate.state).toBe('metered')
    expect(gate.installAllowed).toBe(true)
    expect(gate.meteredUsageEventCount).toBe(1)
    // The metering-stage blockers are now cleared on the gate...
    for (const cleared of [
      'blocker.public.signature_market.usage_event_missing',
      'blocker.public.signature_market.usage_idempotency_missing',
      'blocker.public.signature_market.exact_usage_subject_missing',
    ]) {
      expect(gate.blockerRefs).not.toContain(cleared)
    }
    // ...but settlement is untouched and the gate refuses any payout/settlement.
    expect(gate.blockerRefs).toContain(
      'blocker.public.signature_market.settlement_receipt_missing',
    )
    expect(gate.settlementClaimAllowed).toBe(false)
    expect(gate.payoutClaimAllowed).toBe(false)
    expect(gate.signatureRevenueCopyAllowed).toBe(false)
  })

  test('without metering the activated gate stays "validated" (metering is the gating rung)', () => {
    const gate = projectSignatureMarketplaceRevenueGate(validationInput)
    expect(gate.state).toBe('validated')
    expect(gate.installAllowed).toBe(true)
  })
})

describe('signature usage metering — public projection (#5529)', () => {
  test('empty store: inert/red projection with both blockers honest', () => {
    const projection = projectSignatureUsageMetering(
      emptySignatureUsageMeteringStore,
    )
    expect(projection.schema).toBe(SIGNATURE_USAGE_METERING_SCHEMA)
    expect(projection.promiseId).toBe('marketplace.signature_monetization.v1')
    expect(projection.promiseState).toBe('red')
    expect(projection.inert).toBe(true)
    expect(projection.meteredUsageEventCount).toBe(0)
    expect(projection.clearsBlocker).toBe(SIGNATURE_USAGE_METERING_BLOCKER)
    expect(projection.remainingOwnerGatedBlocker).toBe(
      SIGNATURE_SETTLEMENT_BLOCKER,
    )
  })

  test('armed store: surfaces recorded usage refs, stays red/inert, leaks no private material', () => {
    const store = makeInMemorySignatureUsageMeteringStore([
      recordOk({
        signatureSubjectRef: 'package_site_builder.version_v1',
        packageRef: 'package.public.signature_market.site_builder',
        idempotencyToken: 'usage-001',
      }),
    ])
    const projection = projectSignatureUsageMetering(store)

    expect(projection.promiseState).toBe('red')
    expect(projection.inert).toBe(true)
    expect(projection.meteredUsageEventCount).toBe(1)
    expect(projection.usageEventRefs).toHaveLength(1)
    expect(projection.usageIdempotencyRefs).toHaveLength(1)
    expect(projection.exactUsageSubjectRefs).toHaveLength(1)

    // Every derived ref round-trips the revenue gate's public-safe guard.
    const gate = projectSignatureMeteringGate(store, validationInput)
    expect(signatureMarketplaceRevenueGateHasPrivateMaterial(gate)).toBe(false)
  })
})
