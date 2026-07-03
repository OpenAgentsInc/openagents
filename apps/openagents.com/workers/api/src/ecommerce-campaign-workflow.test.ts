import { describe, expect, test } from 'vitest'

import {
  type EcommerceCampaignWorkflowInput,
  EcommerceCampaignWorkflowInvariantError,
  buildEcommerceCampaignWorkflowReceipt,
  verifyEcommerceCampaignWorkflowReceipt,
} from './ecommerce-campaign-workflow'

const workflowInput = (
  overrides: Partial<EcommerceCampaignWorkflowInput> = {},
): EcommerceCampaignWorkflowInput => ({
  workflowRef: 'workflow.ecommerce.inventory_campaign.fixture',
  inventorySnapshotRef: 'inventory.snapshot.fixture',
  inventoryItems: [
    {
      skuRef: 'sku.in_stock',
      title: 'In-stock product',
      stockState: 'in_stock',
      availableQuantity: 12,
      productImageRef: 'image.in_stock',
      productImageVerified: true,
      productPageRef: 'product.in_stock',
    },
    {
      skuRef: 'sku.out_of_stock',
      title: 'Out-of-stock product',
      stockState: 'out_of_stock',
      availableQuantity: 0,
      productImageRef: 'image.out_of_stock',
      productImageVerified: true,
      productPageRef: 'product.out_of_stock',
    },
  ],
  selectedSkuRefs: ['sku.in_stock'],
  spendCapCents: 50_000,
  requestedSpendCents: 42_000,
  statsWindow: '2026-06-20/2026-06-27',
  conversationalEditRefs: ['conversation.edit.1'],
  merchantApprovalMode: 'approved_for_publish',
  publishState: 'published_with_receipt',
  ...overrides,
})

describe('e-commerce inventory-aware campaign workflow', () => {
  test('builds a clean receipt for in-stock products with verified imagery, edits, and spend cap', () => {
    const receipt = buildEcommerceCampaignWorkflowReceipt(workflowInput())

    expect(receipt.eligibleSkuRefs).toEqual(['sku.in_stock'])
    expect(receipt.excludedSkuRefs).toEqual(['sku.out_of_stock'])
    expect(receipt.productImageRefs).toEqual(['image.in_stock'])
    expect(receipt.blockerRefs).toEqual([])
    expect(verifyEcommerceCampaignWorkflowReceipt(receipt)).toEqual([])
  })

  test('rejects selected products that are not in stock', () => {
    expect(() =>
      buildEcommerceCampaignWorkflowReceipt(
        workflowInput({ selectedSkuRefs: ['sku.out_of_stock'] }),
      ),
    ).toThrow(EcommerceCampaignWorkflowInvariantError)
  })

  test('rejects selected products without verified imagery', () => {
    expect(() =>
      buildEcommerceCampaignWorkflowReceipt(
        workflowInput({
          inventoryItems: [
            {
              skuRef: 'sku.in_stock',
              title: 'In-stock product',
              stockState: 'in_stock',
              availableQuantity: 12,
              productImageRef: null,
              productImageVerified: false,
              productPageRef: 'product.in_stock',
            },
          ],
        }),
      ),
    ).toThrow(EcommerceCampaignWorkflowInvariantError)
  })

  test('rejects requested spend above the cap', () => {
    expect(() =>
      buildEcommerceCampaignWorkflowReceipt(
        workflowInput({ requestedSpendCents: 50_001 }),
      ),
    ).toThrow(EcommerceCampaignWorkflowInvariantError)
  })

  test('keeps draft workflows blocked until conversational edits and publish approval exist', () => {
    const receipt = buildEcommerceCampaignWorkflowReceipt(
      workflowInput({
        conversationalEditRefs: [],
        merchantApprovalMode: 'approval_required',
        publishState: 'not_published',
      }),
    )

    expect(receipt.blockerRefs).toEqual([
      'blocker.ecommerce_campaign.conversational_edit_thread_missing',
      'blocker.ecommerce_campaign.merchant_publish_approval_missing',
      'blocker.ecommerce_campaign.not_published_with_receipt',
    ])
    expect(verifyEcommerceCampaignWorkflowReceipt(receipt)).toContain(
      'conversational edit thread missing',
    )
  })
})
