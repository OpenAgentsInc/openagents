import { describe, expect, it } from 'vitest'

import { assessMarketingAgencySelfServeClaim } from './marketing-agency-self-serve-claim-upgrade'
import { MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA } from './marketing-agency-self-serve-deliverability'

describe('marketing agency self serve claim upgrade', () => {
  it('substantiates when deliverability is ready and owner sign-off is present', () => {
    const claim = assessMarketingAgencySelfServeClaim({
      deliverability: {
        schema: MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA,
        workspaceId: 'ws_123',
        customHostname: 'site.acme.com',
        customHostnameStatus: 'active',
        emailDomain: 'acme.com',
        dkimStatus: 'active',
        spfStatus: 'active',
        publishDeliverabilityProven: true,
        sendDeliverabilityProven: true,
        selfServeReady: true,
        checkedAt: '2026-06-20T12:00:00.000Z',
      },
      deliverabilityRef: 'deliverability.marketing_agency.fixture',
      ownerSignOffRef: 'https://github.com/OpenAgentsInc/openagents/issues/5102#issuecomment-123',
    })

    expect(claim.selfServeSubstantiated).toBe(true)
    expect(claim.failingGateRefs).toHaveLength(0)
    expect(claim.unclearedBlockerRefs).toHaveLength(0)
  })

  it('fails when deliverability is not ready', () => {
    const claim = assessMarketingAgencySelfServeClaim({
      deliverability: {
        schema: MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA,
        workspaceId: 'ws_123',
        customHostname: 'site.acme.com',
        customHostnameStatus: 'pending',
        emailDomain: 'acme.com',
        dkimStatus: 'pending',
        spfStatus: 'pending',
        publishDeliverabilityProven: false,
        sendDeliverabilityProven: false,
        selfServeReady: false,
        checkedAt: '2026-06-20T12:00:00.000Z',
      },
      deliverabilityRef: 'deliverability.marketing_agency.fixture',
      ownerSignOffRef: 'https://github.com/OpenAgentsInc/openagents/issues/5102#issuecomment-123',
    })

    expect(claim.selfServeSubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain('gate.self_serve.deliverability_proven')
    expect(claim.unclearedBlockerRefs).toContain('blocker.product_promises.marketing_agency_pack_self_serve_missing')
  })

  it('fails when owner sign-off is missing', () => {
    const claim = assessMarketingAgencySelfServeClaim({
      deliverability: {
        schema: MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA,
        workspaceId: 'ws_123',
        customHostname: 'site.acme.com',
        customHostnameStatus: 'active',
        emailDomain: 'acme.com',
        dkimStatus: 'active',
        spfStatus: 'active',
        publishDeliverabilityProven: true,
        sendDeliverabilityProven: true,
        selfServeReady: true,
        checkedAt: '2026-06-20T12:00:00.000Z',
      },
      deliverabilityRef: 'deliverability.marketing_agency.fixture',
    })

    expect(claim.selfServeSubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain('gate.self_serve.owner_sign_off_present')
    expect(claim.unclearedBlockerRefs).toContain('blocker.product_promises.marketing_agency_pack_self_serve_missing')
  })
})
