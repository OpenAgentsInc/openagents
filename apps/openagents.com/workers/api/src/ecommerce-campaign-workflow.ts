import { Schema as S } from 'effect'

export const ECOMMERCE_CAMPAIGN_WORKFLOW_SCHEMA =
  'openagents.ecommerce_campaign.inventory_workflow.v1' as const

export const EcommerceCampaignInventoryItem = S.Struct({
  skuRef: S.String,
  title: S.String,
  stockState: S.Literals(['in_stock', 'out_of_stock']),
  availableQuantity: S.Number,
  productImageRef: S.NullOr(S.String),
  productImageVerified: S.Boolean,
  productPageRef: S.String,
})
export type EcommerceCampaignInventoryItem =
  typeof EcommerceCampaignInventoryItem.Type

export const EcommerceCampaignWorkflowReceipt = S.Struct({
  schema: S.Literal(ECOMMERCE_CAMPAIGN_WORKFLOW_SCHEMA),
  workflowRef: S.String,
  inventorySnapshotRef: S.String,
  eligibleSkuRefs: S.Array(S.String),
  excludedSkuRefs: S.Array(S.String),
  productImageRefs: S.Array(S.String),
  spendCapCents: S.Number,
  requestedSpendCents: S.Number,
  statsWindow: S.NullOr(S.String),
  conversationalEditRefs: S.Array(S.String),
  merchantApprovalMode: S.Literals([
    'draft_only',
    'approval_required',
    'approved_for_publish',
  ]),
  publishState: S.Literals(['not_published', 'published_with_receipt']),
  blockerRefs: S.Array(S.String),
})
export type EcommerceCampaignWorkflowReceipt =
  typeof EcommerceCampaignWorkflowReceipt.Type

export class EcommerceCampaignWorkflowInvariantError extends S.TaggedErrorClass<EcommerceCampaignWorkflowInvariantError>()(
  'EcommerceCampaignWorkflowInvariantError',
  { reason: S.String },
) {}

export type EcommerceCampaignWorkflowInput = Readonly<{
  workflowRef: string
  inventorySnapshotRef: string
  inventoryItems: ReadonlyArray<EcommerceCampaignInventoryItem>
  selectedSkuRefs: ReadonlyArray<string>
  spendCapCents: number
  requestedSpendCents: number
  statsWindow: string | null
  conversationalEditRefs: ReadonlyArray<string>
  merchantApprovalMode: EcommerceCampaignWorkflowReceipt['merchantApprovalMode']
  publishState: EcommerceCampaignWorkflowReceipt['publishState']
}>

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(values),
]

export const buildEcommerceCampaignWorkflowReceipt = (
  input: EcommerceCampaignWorkflowInput,
): EcommerceCampaignWorkflowReceipt => {
  if (input.spendCapCents < 0 || input.requestedSpendCents < 0) {
    throw new EcommerceCampaignWorkflowInvariantError({
      reason: 'spend cap and requested spend must not be negative',
    })
  }

  if (input.requestedSpendCents > input.spendCapCents) {
    throw new EcommerceCampaignWorkflowInvariantError({
      reason: `requested spend ${input.requestedSpendCents} exceeds spend cap ${input.spendCapCents}`,
    })
  }

  const itemsBySku = new Map(input.inventoryItems.map(item => [item.skuRef, item]))
  const selectedSkuRefs = unique(input.selectedSkuRefs)
  const missingSkuRefs = selectedSkuRefs.filter(skuRef => !itemsBySku.has(skuRef))

  if (missingSkuRefs.length > 0) {
    throw new EcommerceCampaignWorkflowInvariantError({
      reason: 'selected SKU missing from inventory snapshot: ' + missingSkuRefs.join(', '),
    })
  }

  const selectedItems = selectedSkuRefs.map(skuRef => itemsBySku.get(skuRef)!)
  const invalidStockSkuRefs = selectedItems
    .filter(item => item.stockState !== 'in_stock' || item.availableQuantity <= 0)
    .map(item => item.skuRef)

  if (invalidStockSkuRefs.length > 0) {
    throw new EcommerceCampaignWorkflowInvariantError({
      reason: 'selected SKU is not in stock: ' + invalidStockSkuRefs.join(', '),
    })
  }

  const invalidImageSkuRefs = selectedItems
    .filter(item => item.productImageRef === null || !item.productImageVerified)
    .map(item => item.skuRef)

  if (invalidImageSkuRefs.length > 0) {
    throw new EcommerceCampaignWorkflowInvariantError({
      reason: 'selected SKU lacks verified product imagery: ' + invalidImageSkuRefs.join(', '),
    })
  }

  if (
    input.publishState === 'published_with_receipt' &&
    input.merchantApprovalMode !== 'approved_for_publish'
  ) {
    throw new EcommerceCampaignWorkflowInvariantError({
      reason: 'published campaigns require merchant approval for publish',
    })
  }

  const blockerRefs: Array<string> = []
  if (selectedSkuRefs.length === 0) {
    blockerRefs.push('blocker.ecommerce_campaign.no_in_stock_products_selected')
  }
  if (input.conversationalEditRefs.length === 0) {
    blockerRefs.push('blocker.ecommerce_campaign.conversational_edit_thread_missing')
  }
  if (input.merchantApprovalMode !== 'approved_for_publish') {
    blockerRefs.push('blocker.ecommerce_campaign.merchant_publish_approval_missing')
  }
  if (input.publishState !== 'published_with_receipt') {
    blockerRefs.push('blocker.ecommerce_campaign.not_published_with_receipt')
  }

  return {
    schema: ECOMMERCE_CAMPAIGN_WORKFLOW_SCHEMA,
    workflowRef: input.workflowRef,
    inventorySnapshotRef: input.inventorySnapshotRef,
    eligibleSkuRefs: selectedSkuRefs,
    excludedSkuRefs: input.inventoryItems
      .filter(item => !selectedSkuRefs.includes(item.skuRef))
      .map(item => item.skuRef),
    productImageRefs: selectedItems.map(item => item.productImageRef!),
    spendCapCents: input.spendCapCents,
    requestedSpendCents: input.requestedSpendCents,
    statsWindow: input.statsWindow,
    conversationalEditRefs: [...input.conversationalEditRefs],
    merchantApprovalMode: input.merchantApprovalMode,
    publishState: input.publishState,
    blockerRefs,
  }
}

export const verifyEcommerceCampaignWorkflowReceipt = (
  receipt: EcommerceCampaignWorkflowReceipt,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []

  if (receipt.inventorySnapshotRef.trim().length === 0) {
    reasons.push('inventory snapshot ref missing')
  }
  if (receipt.eligibleSkuRefs.length === 0) {
    reasons.push('no eligible in-stock SKU refs')
  }
  if (receipt.productImageRefs.length < receipt.eligibleSkuRefs.length) {
    reasons.push('verified product image refs missing for eligible SKUs')
  }
  if (receipt.requestedSpendCents > receipt.spendCapCents) {
    reasons.push('requested spend exceeds spend cap')
  }
  if (receipt.conversationalEditRefs.length === 0) {
    reasons.push('conversational edit thread missing')
  }
  if (receipt.publishState !== 'published_with_receipt') {
    reasons.push(`publish state is ${receipt.publishState}, not published_with_receipt`)
  }
  if (receipt.merchantApprovalMode !== 'approved_for_publish') {
    reasons.push('merchant publish approval missing')
  }
  if (receipt.blockerRefs.length > 0) {
    reasons.push('workflow blockers still present: ' + receipt.blockerRefs.join(', '))
  }

  return reasons
}
