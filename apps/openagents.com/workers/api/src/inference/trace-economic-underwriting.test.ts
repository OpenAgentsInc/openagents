import { describe, expect, test } from 'vitest'

import {
  priceOutcomeWarrantyQuote,
  type OutcomeWarrantyQuoteInput,
} from './trace-economic-underwriting'

const baseInput = (
  overrides: Partial<OutcomeWarrantyQuoteInput> = {},
): OutcomeWarrantyQuoteInput => ({
  acceptedOutcomeRef: 'accepted_outcome.khala_coding.6426.001',
  assignmentRef: 'assignment.public.khala_coding.issue_6426',
  customerAccountRef: 'agent:customer-001',
  evidence: {
    meteringReceiptRefs: ['receipt.inference.charge.req_6426'],
    settlementReceiptRefs: ['receipt.serving.payout.accepted_outcome.req_6426'],
    tokenUsageEventRefs: ['token_usage_event.khala_coding.req_6426.turn_1'],
    traceRef: 'trace.public.issue_6426.accepted',
    usageTruth: 'exact',
    verdictRef: 'verdict.khala_coding.accepted.req_6426',
    verdictState: 'accepted',
  },
  premiumMarginBps: 2_500,
  rejectionRiskBps: 400,
  sla: {
    coverageKind: 'refund_on_rejection',
    refundCapMsat: 100_000,
    responseWindowSeconds: 3_600,
    verificationWindowSeconds: 86_400,
  },
  ...overrides,
})

describe('trace-economic underwriting warranty quote', () => {
  test('prices an inert refund-on-rejection warranty from public-safe proof refs', () => {
    const quote = priceOutcomeWarrantyQuote(baseInput())

    expect(quote.schemaVersion).toBe(
      'openagents.trace_economic_underwriting.quote.v1',
    )
    expect(quote.state).toBe('offered')
    expect(quote.expectedLossMsat).toBe(4_000)
    expect(quote.premiumMsat).toBe(5_000)
    expect(quote.refundCapMsat).toBe(100_000)
    expect(quote.coverageKind).toBe('refund_on_rejection')
    expect(quote.blockerRefs).toEqual([])
    expect(quote.evidenceRefs).toEqual([
      'receipt.inference.charge.req_6426',
      'receipt.serving.payout.accepted_outcome.req_6426',
      'token_usage_event.khala_coding.req_6426.turn_1',
      'trace.public.issue_6426.accepted',
      'verdict.khala_coding.accepted.req_6426',
    ])
    expect(quote.refundExecutionAuthority).toBe(false)
    expect(quote.settlementMutationAllowed).toBe(false)
    expect(quote.publicClaimEligible).toBe(false)
  })

  test('blocks warranties without accepted verdict and exact metering proof', () => {
    const quote = priceOutcomeWarrantyQuote(
      baseInput({
        evidence: {
          meteringReceiptRefs: [],
          settlementReceiptRefs: [],
          tokenUsageEventRefs: [],
          traceRef: null,
          usageTruth: 'estimated',
          verdictRef: 'verdict.khala_coding.rejected.req_6426',
          verdictState: 'rejected',
        },
      }),
    )

    expect(quote.state).toBe('blocked')
    expect(quote.premiumMsat).toBe(0)
    expect(quote.expectedLossMsat).toBe(0)
    expect(quote.blockerRefs).toEqual([
      'blocker.trace_underwriting.accepted_verdict_missing',
      'blocker.trace_underwriting.exact_usage_missing',
      'blocker.trace_underwriting.metering_receipt_missing',
      'blocker.trace_underwriting.public_safe_trace_missing',
      'blocker.trace_underwriting.token_usage_event_missing',
    ])
    expect(quote.caveatRefs).toContain(
      'caveat.trace_underwriting.settlement_receipt_not_attached',
    )
  })

  test('produces a stable quote ref for the same risk terms and proof anchors', () => {
    const first = priceOutcomeWarrantyQuote(baseInput())
    const second = priceOutcomeWarrantyQuote(
      baseInput({
        evidence: {
          ...baseInput().evidence,
          settlementReceiptRefs: [
            'receipt.serving.payout.accepted_outcome.req_6426',
          ],
        },
      }),
    )

    expect(second.quoteRef).toBe(first.quoteRef)
  })
})
