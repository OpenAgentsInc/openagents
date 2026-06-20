import { buildMarketingAgencyDeliveryReceipt, type MarketingAgencyDeliveryReceipt } from './marketing-agency-delivery-receipt'

/**
 * A dereferenceable first-paid delivery-receipt fixture for a marketing-agency
 * white-label work item. This provides the concrete evidence shape needed for
 * blocker.product_promises.marketing_agency_pack_first_paid_delivery_receipt_missing.
 */
export const firstPaidMarketingAgencyDeliveryReceiptFixture: MarketingAgencyDeliveryReceipt = buildMarketingAgencyDeliveryReceipt({
  workItemRef: 'work_item.marketing_agency.landing_page.fixture',
  outcomeKind: 'agency_white_label_landing_page',
  humanReviewAccepted: true,
  receiptedGates: {
    client_approval: true,
    domain_authority: true,
    channel_access: true,
    publish_authority: true,
    send_authority: true,
  },
  approvedDeliverableRefs: ['draft.landing_page.v1'],
  publishedArtifactRefs: ['site.acme.openagents.dev/launch'],
  whiteLabelSubdomainState: 'live',
  emailSendState: 'not_sent',
  operatorLaneAcceptance: false,
  metricWindow: '2026-06-20/2026-06-27',
  attributionCaveat: 'Engagement attribution is modeled, not deterministic; treat as directional.',
  paidSettlement: {
    amountCents: 150000,
    asset: 'usd',
    evidenced: true,
    publicPaymentRef: 'payment.public.ref.marketing_agency_launch.1',
  },
  freshnessTimestamp: '2026-06-20T12:00:00.000Z',
  publicSourceRefs: [
    'docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md',
  ],
})
