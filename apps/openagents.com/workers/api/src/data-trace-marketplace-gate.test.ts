import { describe, expect, test } from 'vitest'

import {
  DataTraceMarketplaceGateUnsafe,
  dataTraceMarketplaceGateHasPrivateMaterial,
  projectDataTraceMarketplaceGate,
} from './data-trace-marketplace-gate'

const publicSaleSmokeInput = {
  entitlementRefs: ['entitlement.public.data_market.trace_sale_001'],
  payoutContractRefs: ['payout_contract.public.data_market.trace_sale_001'],
  plannerMode: 'typed_semantic_selector' as const,
  purchaseReceiptRefs: ['purchase.public.data_market.trace_sale_001'],
  redactionReceiptRefs: ['redaction.public.data_market.trace_sale_001'],
  semanticPlannerRefs: ['planner.public.data_market.semantic_selector_001'],
  settlementReceiptRefs: ['settlement.public.data_market.trace_sale_001'],
  traceSubmissionRefs: ['trace.public.data_market.submission_001'],
  valuationRefs: ['valuation.public.data_market.trace_sale_001'],
}

describe('Data trace marketplace gate', () => {
  test('keeps submitted traces blocked until redaction and planner evidence exist', () => {
    const gate = projectDataTraceMarketplaceGate({
      plannerMode: 'typed_semantic_selector',
      traceSubmissionRefs: ['trace.public.data_market.submission_001'],
    })

    expect(gate).toMatchObject({
      dataRevenueCopyAllowed: false,
      publicSafeDataSaleSmokePassed: false,
      settlementClaimAllowed: false,
      state: 'submitted',
      valuationPayoutClaimAllowed: false,
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.redaction_receipt_missing',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.semantic_planner_missing',
    )
  })

  test('blocks keyword routing and requires a semantic planner ref', () => {
    const gate = projectDataTraceMarketplaceGate({
      plannerMode: 'keyword_route',
      redactionReceiptRefs: ['redaction.public.data_market.trace_sale_001'],
      semanticPlannerRefs: ['planner.public.data_market.keyword_fixture'],
      traceSubmissionRefs: ['trace.public.data_market.submission_001'],
      valuationRefs: ['valuation.public.data_market.trace_sale_001'],
    })

    expect(gate).toMatchObject({
      dataRevenueCopyAllowed: false,
      state: 'blocked',
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.keyword_routing_disallowed',
    )
  })

  test('keeps valuation separate from payout', () => {
    const gate = projectDataTraceMarketplaceGate({
      plannerMode: 'structured_query_planner',
      redactionReceiptRefs: ['redaction.public.data_market.trace_sale_001'],
      semanticPlannerRefs: ['planner.public.data_market.structured_001'],
      traceSubmissionRefs: ['trace.public.data_market.submission_001'],
      valuationRefs: ['valuation.public.data_market.trace_sale_001'],
    })

    expect(gate).toMatchObject({
      dataRevenueCopyAllowed: false,
      publicSafeDataSaleSmokePassed: false,
      state: 'valued',
      valuationPayoutClaimAllowed: false,
    })
    expect(gate.caveatRefs).toContain(
      'caveat.public.data_market.valuation_is_not_payout',
    )
  })

  test('keeps purchase and entitlement separate from settlement', () => {
    const gate = projectDataTraceMarketplaceGate({
      ...publicSaleSmokeInput,
      payoutContractRefs: [],
      settlementReceiptRefs: [],
    })

    expect(gate).toMatchObject({
      dataRevenueCopyAllowed: false,
      publicSafeDataSaleSmokePassed: false,
      settlementClaimAllowed: false,
      state: 'entitled',
    })
    expect(gate.blockerRefs).toEqual([
      'blocker.public.data_market.payout_contract_missing',
      'blocker.public.data_market.settlement_receipt_missing',
    ])
    expect(gate.caveatRefs).toContain(
      'caveat.public.data_market.purchase_is_not_settlement',
    )
  })

  test('allows data revenue copy only after a public-safe sale smoke settles', () => {
    const gate = projectDataTraceMarketplaceGate(publicSaleSmokeInput)

    expect(gate).toMatchObject({
      dataRevenueCopyAllowed: true,
      publicSafeDataSaleSmokePassed: true,
      settlementClaimAllowed: true,
      state: 'settled',
    })
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.data_market.sale_settlement_receipts_visible',
    ])
  })

  test('rejects raw traces, prompts, private repos, provider payloads, customer data, wallet material, and timestamps', () => {
    const unsafeInputs = [
      {
        plannerMode: 'typed_semantic_selector' as const,
        traceSubmissionRefs: ['trace_raw.full_prompt_payload'],
      },
      {
        plannerMode: 'typed_semantic_selector' as const,
        redactionReceiptRefs: ['github.com/acme/private/customer-repo'],
      },
      {
        plannerMode: 'typed_semantic_selector' as const,
        semanticPlannerRefs: ['provider_payload.openai.raw'],
      },
      {
        plannerMode: 'typed_semantic_selector' as const,
        valuationRefs: ['customer_email.alice@example.com'],
      },
      {
        plannerMode: 'typed_semantic_selector' as const,
        purchaseReceiptRefs: ['wallet.private.trace_market'],
      },
      {
        plannerMode: 'typed_semantic_selector' as const,
        settlementReceiptRefs: ['2026-06-08T12:00:00Z'],
      },
    ]

    unsafeInputs.forEach(input => {
      expect(() => projectDataTraceMarketplaceGate(input)).toThrow(
        DataTraceMarketplaceGateUnsafe,
      )
    })
  })

  test('keeps settled sale projection free of private material', () => {
    const gate = projectDataTraceMarketplaceGate(publicSaleSmokeInput)
    const json = JSON.stringify(gate)

    expect(dataTraceMarketplaceGateHasPrivateMaterial(gate)).toBe(false)
    expect(json).not.toMatch(
      /trace_raw|raw_prompt|provider_payload|customer_email|wallet|preimage|lnbc|@|github\.com\/[^:/]+\/private/i,
    )
  })
})
