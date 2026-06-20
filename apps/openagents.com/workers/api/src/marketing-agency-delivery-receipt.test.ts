import { describe, expect, test } from 'vitest'

import { MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF } from './prefilled-workspace-vertical-templates'
import {
  type MarketingAgencyAuthorityGateId,
  type MarketingAgencyDeliveryInput,
  MarketingAgencyDeliveryReceiptInvariantError,
  buildMarketingAgencyDeliveryReceipt,
  verifyMarketingAgencyPaidDelivery,
} from './marketing-agency-delivery-receipt'

const allBlockedGates: Record<MarketingAgencyAuthorityGateId, boolean> = {
  client_approval: false,
  domain_authority: false,
  channel_access: false,
  publish_authority: false,
  send_authority: false,
}

const allReceiptedGates: Record<MarketingAgencyAuthorityGateId, boolean> = {
  client_approval: true,
  domain_authority: true,
  channel_access: true,
  publish_authority: true,
  send_authority: true,
}

const baseInput = (
  overrides: Partial<MarketingAgencyDeliveryInput> = {},
): MarketingAgencyDeliveryInput => ({
  workItemRef: 'work_item.marketing_agency.landing_page.fixture',
  outcomeKind: 'agency_white_label_landing_page',
  humanReviewAccepted: false,
  receiptedGates: allBlockedGates,
  approvedDeliverableRefs: [],
  publishedArtifactRefs: [],
  whiteLabelSubdomainState: 'not_provisioned',
  emailSendState: 'not_sent',
  operatorLaneAcceptance: false,
  metricWindow: null,
  attributionCaveat:
    'Engagement attribution is modeled, not deterministic; treat as directional.',
  paidSettlement: {
    amountCents: 0,
    asset: 'usd',
    evidenced: false,
    publicPaymentRef: null,
  },
  freshnessTimestamp: '2026-06-20T12:00:00.000Z',
  publicSourceRefs: [
    'docs/blitz/forge/2026-06-16-marketing-agency-prefilled-workspace.md',
  ],
  ...overrides,
})

describe('marketing-agency white-label delivery receipt', () => {
  test('a fresh draft with all gates blocked is blocked, not delivered', () => {
    const receipt = buildMarketingAgencyDeliveryReceipt(baseInput())

    expect(receipt.templateRef).toBe(MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF)
    expect(receipt.deliveryStage).toBe('blocked')
    expect(receipt.evidenceState).toBe('not_yet_evidenced')
    expect(receipt.noAutoPublish).toBe(true)
    expect(receipt.noAutoSend).toBe(true)
    expect(receipt.outstandingAuthorityBlockers).toHaveLength(5)
    expect(verifyMarketingAgencyPaidDelivery(receipt).length).toBeGreaterThan(0)
  })

  test('reviewed draft with no blocked gates is drafted_for_review until paid', () => {
    const receipt = buildMarketingAgencyDeliveryReceipt(
      baseInput({
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        approvedDeliverableRefs: ['draft.landing_page.v1'],
      }),
    )

    expect(receipt.deliveryStage).toBe('drafted_for_review')
    expect(receipt.evidenceState).toBe('review_accepted')
    expect(receipt.outstandingAuthorityBlockers).toHaveLength(0)
    expect(verifyMarketingAgencyPaidDelivery(receipt)).toContain(
      'buyer payment not recorded',
    )
  })

  test('a fully gated, reviewed, paid landing-page delivery verifies as evidenced', () => {
    const receipt = buildMarketingAgencyDeliveryReceipt(
      baseInput({
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        approvedDeliverableRefs: ['draft.landing_page.v1'],
        publishedArtifactRefs: ['site.acme.openagents.dev/launch'],
        whiteLabelSubdomainState: 'live',
        metricWindow: '2026-06-20/2026-06-27',
        paidSettlement: {
          amountCents: 75_000,
          asset: 'usd',
          evidenced: true,
          publicPaymentRef: 'payment.public.ref.abc',
        },
      }),
    )

    expect(receipt.deliveryStage).toBe('delivered')
    expect(receipt.evidenceState).toBe('paid')
    expect(verifyMarketingAgencyPaidDelivery(receipt)).toEqual([])
  })

  test('a paid welcome-email delivery requires the email to be sent', () => {
    const notSent = buildMarketingAgencyDeliveryReceipt(
      baseInput({
        outcomeKind: 'agency_welcome_email',
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        emailSendState: 'scheduled',
        paidSettlement: {
          amountCents: 30_000,
          asset: 'usd',
          evidenced: true,
          publicPaymentRef: 'payment.public.ref.def',
        },
      }),
    )
    expect(verifyMarketingAgencyPaidDelivery(notSent)).toContain(
      'welcome email not sent (state: scheduled)',
    )

    const sent = buildMarketingAgencyDeliveryReceipt(
      baseInput({
        outcomeKind: 'agency_welcome_email',
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        emailSendState: 'sent',
        paidSettlement: {
          amountCents: 30_000,
          asset: 'usd',
          evidenced: true,
          publicPaymentRef: 'payment.public.ref.def',
        },
      }),
    )
    expect(verifyMarketingAgencyPaidDelivery(sent)).toEqual([])
  })

  test('a paid operator-lane delivery requires lane acceptance', () => {
    const receipt = buildMarketingAgencyDeliveryReceipt(
      baseInput({
        outcomeKind: 'agency_operator_autopilot_lane',
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        operatorLaneAcceptance: true,
        paidSettlement: {
          amountCents: 50_000,
          asset: 'credits',
          evidenced: true,
          publicPaymentRef: 'payment.public.ref.ghi',
        },
      }),
    )
    expect(verifyMarketingAgencyPaidDelivery(receipt)).toEqual([])
  })

  test('rejects published landing pages while authority gates are blocked', () => {
    expect(() =>
      buildMarketingAgencyDeliveryReceipt(
        baseInput({ publishedArtifactRefs: ['site.acme.openagents.dev/launch'] }),
      ),
    ).toThrow(MarketingAgencyDeliveryReceiptInvariantError)
  })

  test('rejects a live subdomain while authority gates are blocked', () => {
    expect(() =>
      buildMarketingAgencyDeliveryReceipt(
        baseInput({ whiteLabelSubdomainState: 'live' }),
      ),
    ).toThrow(MarketingAgencyDeliveryReceiptInvariantError)
  })

  test('rejects a sent or scheduled email while authority gates are blocked', () => {
    expect(() =>
      buildMarketingAgencyDeliveryReceipt(
        baseInput({ emailSendState: 'sent' }),
      ),
    ).toThrow(MarketingAgencyDeliveryReceiptInvariantError)
  })

  test('does not verify a paid delivery that lacks a dereferenceable payment ref', () => {
    const receipt = buildMarketingAgencyDeliveryReceipt(
      baseInput({
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        publishedArtifactRefs: ['site.acme.openagents.dev/launch'],
        whiteLabelSubdomainState: 'live',
        paidSettlement: {
          amountCents: 75_000,
          asset: 'usd',
          evidenced: true,
          publicPaymentRef: null,
        },
      }),
    )

    expect(verifyMarketingAgencyPaidDelivery(receipt)).toContain(
      'paid settlement recorded without a dereferenceable payment ref',
    )
  })
})
