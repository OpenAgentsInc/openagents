import {
  buildEcommerceCampaignDeliveryReceipt,
  toEcommerceCampaignDeliveryReceiptDocument,
  type EcommerceCampaignDeliveryReceiptDocument,
} from './ecommerce-campaign-delivery-receipt'
import { buildEcommerceCampaignWorkflowReceipt } from './ecommerce-campaign-workflow'

/**
 * A dereferenceable first-paid delivery-receipt fixture for an e-commerce
 * inventory-aware campaign work item. This provides the concrete evidence shape
 * needed for blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing.
 */
export const firstPaidEcommerceCampaignDeliveryReceiptFixture: EcommerceCampaignDeliveryReceiptDocument =
  toEcommerceCampaignDeliveryReceiptDocument(
    buildEcommerceCampaignDeliveryReceipt({
      workItemRef: 'work_item.ecommerce.inventory_campaign.fixture',
      outcomeKind: 'campaign_receipt_stats',
      humanReviewAccepted: true,
      receiptedGates: {
        merchant_approval: true,
        channel_access: true,
        ad_account_access: true,
        spend_cap_accepted: true,
        publish_authority: true,
      },
      spendCapCents: 50000,
      spendObservedCents: 45000,
      publishedArtifactRefs: ['ad.acme.openagents.dev/campaign/1'],
      statsWindow: '2026-06-20/2026-06-27',
      attributionCaveat:
        'Engagement attribution is modeled, not deterministic; treat as directional.',
      stockoutFollowUp: 'Alert merchant if any advertised SKU stock drops below 5 units.',
      campaignWorkflow: buildEcommerceCampaignWorkflowReceipt({
        workflowRef: 'workflow.ecommerce.inventory_campaign.fixture',
        inventorySnapshotRef: 'inventory.snapshot.ecommerce_campaign.fixture',
        inventoryItems: [
          {
            skuRef: 'sku.fixture.in_stock',
            title: 'Fixture in-stock product',
            stockState: 'in_stock',
            availableQuantity: 12,
            productImageRef: 'image.fixture.in_stock',
            productImageVerified: true,
            productPageRef: 'product.fixture.in_stock',
          },
          {
            skuRef: 'sku.fixture.out_of_stock',
            title: 'Fixture out-of-stock product',
            stockState: 'out_of_stock',
            availableQuantity: 0,
            productImageRef: 'image.fixture.out_of_stock',
            productImageVerified: true,
            productPageRef: 'product.fixture.out_of_stock',
          },
        ],
        selectedSkuRefs: ['sku.fixture.in_stock'],
        spendCapCents: 50000,
        requestedSpendCents: 45000,
        statsWindow: '2026-06-20/2026-06-27',
        conversationalEditRefs: ['conversation.ecommerce_campaign.fixture_edit'],
        merchantApprovalMode: 'approved_for_publish',
        publishState: 'published_with_receipt',
      }),
      paidSettlement: {
        amountCents: 15000,
        asset: 'usd',
        evidenced: true,
        publicPaymentRef: 'payment.public.ref.ecommerce_campaign.1',
      },
      freshnessTimestamp: '2026-06-20T12:00:00.000Z',
      publicSourceRefs: [
        'docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md',
      ],
    }),
  )
