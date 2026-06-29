import { Schema as S } from 'effect'

/**
 * Proof of self-serve deliverability for the marketing-agency vertical pack.
 *
 * Promise: business.marketing_agency_workspace_pack.v1 (yellow).
 * Advances: blocker.product_promises.marketing_agency_pack_self_serve_missing.
 *
 * A self-serve pack must prove that it can publish and send WITHOUT operator
 * assistance. This requires the workspace's custom hostname to be active (proven
 * publish deliverability) and the email domain's DKIM/SPF to be active (proven
 * send deliverability).
 */

export const MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA =
  'openagents.marketing_agency.self_serve_deliverability.v1' as const

export const MarketingAgencySelfServeDeliverability = S.Struct({
  schema: S.Literal(MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA),
  workspaceId: S.String,
  customHostname: S.String,
  customHostnameStatus: S.Literals(['active', 'pending', 'error', 'not_provisioned']),
  emailDomain: S.String,
  dkimStatus: S.Literals(['active', 'pending', 'error', 'not_provisioned']),
  spfStatus: S.Literals(['active', 'pending', 'error', 'not_provisioned']),
  publishDeliverabilityProven: S.Boolean,
  sendDeliverabilityProven: S.Boolean,
  selfServeReady: S.Boolean,
  checkedAt: S.String,
})
export type MarketingAgencySelfServeDeliverability =
  typeof MarketingAgencySelfServeDeliverability.Type

export type MarketingAgencyDeliverabilityInput = Readonly<{
  workspaceId: string
  customHostname: string
  customHostnameStatus: MarketingAgencySelfServeDeliverability['customHostnameStatus']
  emailDomain: string
  dkimStatus: MarketingAgencySelfServeDeliverability['dkimStatus']
  spfStatus: MarketingAgencySelfServeDeliverability['spfStatus']
  checkedAt: string
}>

/**
 * Assess if a marketing-agency workspace has proven self-serve deliverability.
 * True self-serve requires both the publish channel (custom hostname) and
 * send channel (DKIM/SPF) to be fully active without operator intervention.
 */
export const assessMarketingAgencySelfServeDeliverability = (
  input: MarketingAgencyDeliverabilityInput,
): MarketingAgencySelfServeDeliverability => {
  const publishDeliverabilityProven = input.customHostnameStatus === 'active'
  const sendDeliverabilityProven =
    input.dkimStatus === 'active' && input.spfStatus === 'active'

  const selfServeReady = publishDeliverabilityProven && sendDeliverabilityProven

  return {
    schema: MARKETING_AGENCY_SELF_SERVE_DELIVERABILITY_SCHEMA,
    workspaceId: input.workspaceId,
    customHostname: input.customHostname,
    customHostnameStatus: input.customHostnameStatus,
    emailDomain: input.emailDomain,
    dkimStatus: input.dkimStatus,
    spfStatus: input.spfStatus,
    publishDeliverabilityProven,
    sendDeliverabilityProven,
    selfServeReady,
    checkedAt: input.checkedAt,
  }
}
