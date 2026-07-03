import { describe, expect, test } from 'vitest'

import {
  type BusinessMarketingProgramComponent,
  type BusinessMarketingProgramPackageInput,
  BusinessMarketingProgramPackageInvariantError,
  buildBusinessMarketingProgramPackageReceipt,
  verifyBusinessMarketingProgramSoldAndDelivered,
} from './business-marketing-program-package'

const deliveredComponent = (
  kind: BusinessMarketingProgramComponent['kind'],
  receiptRef = `receipt.${kind}.fixture`,
): BusinessMarketingProgramComponent => ({
  kind,
  state: 'delivered_with_receipt',
  receiptRefs: [receiptRef],
})
const baseInput = (
  overrides: Partial<BusinessMarketingProgramPackageInput> = {},
): BusinessMarketingProgramPackageInput => ({
  packageRef: 'business.marketing_program.fixture.1',
  tier: 'full_funnel',
  humanReviewAccepted: true,
  buyerPayment: {
    amountCents: 250_000,
    asset: 'usd',
    evidenced: true,
    publicPaymentRef: 'payment.public.marketing_program.fixture.1',
  },
  components: [
    deliveredComponent('site_or_landing_page', 'receipt.site.publish.fixture'),
    deliveredComponent(
      'email_sequence_or_list',
      'receipt.email.sequence.fixture',
    ),
    deliveredComponent(
      'inventory_aware_campaign',
      'receipt.inventory.campaign.fixture',
    ),
    deliveredComponent('geo_content_brief', 'receipt.geo.content.fixture'),
    deliveredComponent('outbound_assist_plan', 'receipt.outbound.plan.fixture'),
  ],
  publicSourceRefs: [
    'docs/fable/ROADMAP_BIZ.md#bf-8--retain-and-multiply',
    'apps/openagents.com/workers/api/src/marketing-agency-delivery-receipt.ts',
    'apps/openagents.com/workers/api/src/ecommerce-campaign-workflow.ts',
    'apps/openagents.com/workers/api/src/email-sequence-send-service.ts',
  ],
  freshnessTimestamp: '2026-07-03T12:00:00.000Z',
  ...overrides,
})

describe('business marketing program package receipt', () => {
  test('verifies a sold and delivered full-funnel program with every backing receipt', () => {
    const receipt = buildBusinessMarketingProgramPackageReceipt(baseInput())

    expect(receipt.promiseId).toBe('business.marketing_agency_workspace_pack.v1')
    expect(receipt.noAutoPublish).toBe(true)
    expect(receipt.noAutoSend).toBe(true)
    expect(receipt.requiredComponentKinds).toEqual([
      'site_or_landing_page',
      'email_sequence_or_list',
      'inventory_aware_campaign',
      'geo_content_brief',
      'outbound_assist_plan',
    ])
    expect(receipt.missingComponentKinds).toEqual([])
    expect(receipt.blockerRefs).toEqual([])
    expect(verifyBusinessMarketingProgramSoldAndDelivered(receipt)).toEqual([])
  })

  test('keeps a content-only package blocked until payment and review evidence exist', () => {
    const receipt = buildBusinessMarketingProgramPackageReceipt(
      baseInput({
        tier: 'content',
        humanReviewAccepted: false,
        buyerPayment: {
          amountCents: 0,
          asset: 'usd',
          evidenced: false,
          publicPaymentRef: null,
        },
        components: [
          {
            kind: 'geo_content_brief',
            state: 'drafted',
            receiptRefs: [],
          },
        ],
      }),
    )

    expect(receipt.missingComponentKinds).toEqual(['geo_content_brief'])
    expect(receipt.blockerRefs).toEqual([
      'blocker.business_marketing_program.human_review_missing',
      'blocker.business_marketing_program.paid_receipt_missing',
      'blocker.business_marketing_program.delivery_receipts_missing',
    ])
    expect(verifyBusinessMarketingProgramSoldAndDelivered(receipt)).toContain(
      'human-review gate not accepted',
    )
  })

  test('does not allow a delivered component without a receipt ref', () => {
    expect(() =>
      buildBusinessMarketingProgramPackageReceipt(
        baseInput({
          components: [
            {
              kind: 'site_or_landing_page',
              state: 'delivered_with_receipt',
              receiptRefs: [],
            },
          ],
        }),
      ),
    ).toThrow(BusinessMarketingProgramPackageInvariantError)
  })

  test('keeps self-serve packages blocked until self-serve delivery is proven', () => {
    const receipt = buildBusinessMarketingProgramPackageReceipt(
      baseInput({ deliveryMode: 'self_serve' }),
    )

    expect(receipt.blockerRefs).toContain(
      'blocker.business_marketing_program.self_serve_unsupported',
    )
    expect(verifyBusinessMarketingProgramSoldAndDelivered(receipt)).toContain(
      'self-serve marketing program delivery is not proven',
    )
  })

  test('requires every tier-specific component to have a delivered receipt', () => {
    const receipt = buildBusinessMarketingProgramPackageReceipt(
      baseInput({
        tier: 'outbound_assist',
        components: [
          deliveredComponent('site_or_landing_page'),
          {
            kind: 'outbound_assist_plan',
            state: 'review_accepted',
            receiptRefs: ['receipt.outbound.review.fixture'],
          },
        ],
      }),
    )

    expect(receipt.requiredComponentKinds).toEqual([
      'site_or_landing_page',
      'outbound_assist_plan',
    ])
    expect(receipt.missingComponentKinds).toEqual(['outbound_assist_plan'])
    expect(verifyBusinessMarketingProgramSoldAndDelivered(receipt)).toContain(
      'required package components missing delivered receipts: outbound_assist_plan',
    )
  })
})
