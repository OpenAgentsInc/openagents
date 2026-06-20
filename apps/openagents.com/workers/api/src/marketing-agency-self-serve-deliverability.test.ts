import { describe, expect, test } from 'vitest'
import {
  assessMarketingAgencySelfServeDeliverability,
  MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA,
} from './marketing-agency-self-serve-deliverability'

describe('marketing-agency-self-serve-deliverability', () => {
  const baseInput = {
    workspaceId: 'ws_test_agency',
    customHostname: 'launch.acme-agency.test',
    emailDomain: 'acme-agency.test',
    checkedAt: '2026-06-20T12:00:00.000Z',
  }

  test('is selfServeReady when both publish and send are active', () => {
    const result = assessMarketingAgencySelfServeDeliverability({
      ...baseInput,
      customHostnameStatus: 'active',
      dkimStatus: 'active',
      spfStatus: 'active',
    })

    expect(result.schema).toBe(MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA)
    expect(result.publishDeliverabilityProven).toBe(true)
    expect(result.sendDeliverabilityProven).toBe(true)
    expect(result.selfServeReady).toBe(true)
  })

  test('is not selfServeReady when custom hostname is pending', () => {
    const result = assessMarketingAgencySelfServeDeliverability({
      ...baseInput,
      customHostnameStatus: 'pending',
      dkimStatus: 'active',
      spfStatus: 'active',
    })

    expect(result.publishDeliverabilityProven).toBe(false)
    expect(result.sendDeliverabilityProven).toBe(true)
    expect(result.selfServeReady).toBe(false)
  })

  test('is not selfServeReady when dkim is pending', () => {
    const result = assessMarketingAgencySelfServeDeliverability({
      ...baseInput,
      customHostnameStatus: 'active',
      dkimStatus: 'pending',
      spfStatus: 'active',
    })

    expect(result.publishDeliverabilityProven).toBe(true)
    expect(result.sendDeliverabilityProven).toBe(false)
    expect(result.selfServeReady).toBe(false)
  })

  test('is not selfServeReady when spf is pending', () => {
    const result = assessMarketingAgencySelfServeDeliverability({
      ...baseInput,
      customHostnameStatus: 'active',
      dkimStatus: 'active',
      spfStatus: 'pending',
    })

    expect(result.publishDeliverabilityProven).toBe(true)
    expect(result.sendDeliverabilityProven).toBe(false)
    expect(result.selfServeReady).toBe(false)
  })
})
