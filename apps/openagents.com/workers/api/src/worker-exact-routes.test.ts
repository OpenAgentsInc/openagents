import { describe, expect, test } from 'vite-plus/test'

import { exactRoutePathManifest } from './index'

const retiredCapabilityPath =
  /(?:^|[\/-])(?:adjutant|balances?|billing|checkout|credits?|markets?|marketplace|payments?|payouts?|settled|settlements?|sites?|tips?|treasury|wallets?|work-requests)(?:[\/-]|$)|paid-privacy|l402/i

describe('Worker exact route manifest', () => {
  test('retains a substantial active route graph without retired money or Sites capability', () => {
    expect(exactRoutePathManifest.length).toBeGreaterThan(50)
    expect(exactRoutePathManifest).toContain('/api/public/home')
    expect(exactRoutePathManifest).toContain('/api/openapi.json')
    expect(exactRoutePathManifest.filter(path => retiredCapabilityPath.test(path))).toEqual([])
  })

  test('does not contain duplicate exact paths', () => {
    expect(new Set(exactRoutePathManifest).size).toBe(exactRoutePathManifest.length)
  })
})
