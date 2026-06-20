import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  SignatureUsageMeteringEndpoint,
  handleSignatureUsageMeteringApi,
  isSignatureUsageMeteringEnabled,
} from './signature-usage-metering-routes'
import {
  makeInMemorySignatureUsageMeteringStore,
  recordSignatureUsage,
} from './signature-usage-metering'

const meteredStore = () => {
  const result = recordSignatureUsage({
    signatureSubjectRef: 'package_site_builder.version_v1',
    packageRef: 'package.public.signature_market.site_builder',
    idempotencyToken: 'usage-001',
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return makeInMemorySignatureUsageMeteringStore([result.event])
}

const request = (suffix = '') =>
  new Request(`https://openagents.com${SignatureUsageMeteringEndpoint}${suffix}`)

describe('signature usage-metering flag (#5529)', () => {
  test('flag defaults OFF', () => {
    expect(isSignatureUsageMeteringEnabled(undefined)).toBe(false)
    expect(isSignatureUsageMeteringEnabled('false')).toBe(false)
    expect(isSignatureUsageMeteringEnabled('0')).toBe(false)
    expect(isSignatureUsageMeteringEnabled('on')).toBe(true)
    expect(isSignatureUsageMeteringEnabled('TRUE')).toBe(true)
  })
})

describe('signature usage-metering route (#5529)', () => {
  test('is INERT (empty) when disabled, even with a populated store', async () => {
    const response = await Effect.runPromise(
      handleSignatureUsageMeteringApi(request(), {
        enabled: false,
        store: meteredStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      meteredUsageEventCount: number
    }
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('red')
    expect(body.meteredUsageEventCount).toBe(0)
  })

  test('surfaces recorded usage when armed, still reporting inert/red', async () => {
    const response = await Effect.runPromise(
      handleSignatureUsageMeteringApi(request(), {
        enabled: true,
        store: meteredStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      meteredUsageEventCount: number
      usageEventRefs: ReadonlyArray<string>
      remainingOwnerGatedBlocker: string
    }
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('red')
    expect(body.meteredUsageEventCount).toBe(1)
    expect(body.usageEventRefs).toHaveLength(1)
    expect(body.remainingOwnerGatedBlocker).toBe(
      'blocker.product_promises.signature_settlement_missing',
    )
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handleSignatureUsageMeteringApi(
        new Request(`https://openagents.com${SignatureUsageMeteringEndpoint}`, {
          method: 'POST',
        }),
        { enabled: false },
      ),
    )
    expect(response.status).toBe(405)
  })
})
