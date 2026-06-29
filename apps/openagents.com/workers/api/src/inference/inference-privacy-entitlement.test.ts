// Tests for the paid-privacy / confidential-compute capture opt-OUT (#6295).
//
// The safety bar: a caller paying for privacy (or in confidential-compute mode)
// is NEVER captured, and an UNSAFE read (error) fails CLOSED-TO-PRIVATE (treated
// as paid-privacy => not captured). A free caller WITHOUT privacy is capturable.

import { describe, expect, it } from 'vitest'
import {
  isConfidentialComputeEnabled,
  makePaidPrivacyResolver,
  PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT,
  PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE,
  PAID_PRIVACY_REASON_NONE,
  PAID_PRIVACY_REASON_READ_ERROR,
  readAccountPaidPrivacy,
} from './inference-privacy-entitlement'

// Minimal D1 fake: `entitled` is the set of account_refs with a row; `throwOnRead`
// forces the read path to error (to exercise fail-closed-to-private).
const makeFakeDb = (opts: {
  entitled?: ReadonlySet<string>
  throwOnRead?: boolean
}): D1Database => {
  const entitled = opts.entitled ?? new Set<string>()
  return {
    prepare: (_sql: string) => ({
      bind: (accountRef: string) => ({
        first: async <T>(): Promise<T | null> => {
          if (opts.throwOnRead) {
            throw new Error('d1 down')
          }
          return entitled.has(accountRef)
            ? ({ account_ref: accountRef } as unknown as T)
            : null
        },
      }),
    }),
  } as unknown as D1Database
}

describe('isConfidentialComputeEnabled — fail-closed flag', () => {
  it('defaults OFF', () => {
    expect(isConfidentialComputeEnabled(undefined)).toBe(false)
    expect(isConfidentialComputeEnabled('')).toBe(false)
    expect(isConfidentialComputeEnabled('false')).toBe(false)
    expect(isConfidentialComputeEnabled('on')).toBe(true)
    expect(isConfidentialComputeEnabled('1')).toBe(true)
    expect(isConfidentialComputeEnabled('TRUE')).toBe(true)
  })
})

describe('readAccountPaidPrivacy', () => {
  it('no row => NOT paid-privacy (capturable)', async () => {
    const db = makeFakeDb({})
    const d = await readAccountPaidPrivacy(db, 'agent:u1')
    expect(d.enabled).toBe(false)
    expect(d.reasonRef).toBe(PAID_PRIVACY_REASON_NONE)
  })

  it('row present => paid-privacy (excluded)', async () => {
    const db = makeFakeDb({ entitled: new Set(['agent:u1']) })
    const d = await readAccountPaidPrivacy(db, 'agent:u1')
    expect(d.enabled).toBe(true)
    expect(d.reasonRef).toBe(PAID_PRIVACY_REASON_ACCOUNT_ENTITLEMENT)
  })

  it('read error => FAIL-CLOSED-TO-PRIVATE (excluded)', async () => {
    const db = makeFakeDb({ throwOnRead: true })
    const d = await readAccountPaidPrivacy(db, 'agent:u1')
    expect(d.enabled).toBe(true)
    expect(d.reasonRef).toBe(PAID_PRIVACY_REASON_READ_ERROR)
  })
})

describe('makePaidPrivacyResolver', () => {
  it('confidential-compute mode excludes EVERY caller', async () => {
    const db = makeFakeDb({})
    const resolve = makePaidPrivacyResolver({
      db,
      confidentialComputeEnabled: true,
    })
    const d = await resolve('agent:anyone')
    expect(d.enabled).toBe(true)
    expect(d.reasonRef).toBe(PAID_PRIVACY_REASON_CONFIDENTIAL_COMPUTE)
  })

  it('without confidential mode, per-account row decides', async () => {
    const db = makeFakeDb({ entitled: new Set(['agent:paid']) })
    const resolve = makePaidPrivacyResolver({
      db,
      confidentialComputeEnabled: false,
    })
    expect((await resolve('agent:paid')).enabled).toBe(true)
    expect((await resolve('agent:free')).enabled).toBe(false)
  })

  it('without confidential mode, a read error still fails closed-to-private', async () => {
    const db = makeFakeDb({ throwOnRead: true })
    const resolve = makePaidPrivacyResolver({
      db,
      confidentialComputeEnabled: false,
    })
    expect((await resolve('agent:u1')).enabled).toBe(true)
  })
})
