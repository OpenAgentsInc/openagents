import { assessMarketingAgencySelfServeDeliverability, type MarketingAgencySelfServeDeliverability } from './marketing-agency-self-serve-deliverability'

/**
 * A fixture demonstrating proven publish + send deliverability for a
 * marketing-agency white-label workspace.
 * 
 * Advances: blocker.product_promises.marketing_agency_pack_self_serve_missing.
 */
export const selfServeDeliverabilityFixture: MarketingAgencySelfServeDeliverability = assessMarketingAgencySelfServeDeliverability({
  workspaceId: 'ws.marketing_agency.self_serve.fixture',
  customHostname: 'launch.acme-agency.openagents.dev',
  customHostnameStatus: 'active',
  emailDomain: 'acme-agency.openagents.dev',
  dkimStatus: 'active',
  spfStatus: 'active',
  checkedAt: '2026-06-20T12:00:00.000Z',
})
