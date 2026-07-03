import { describe, expect, test } from 'vitest'

import {
  QA_SWARM_OWNER_ARM_TOKEN,
  QaSwarmHostedRunReceiptInvariantError,
  buildQaSwarmEngagementReceipt,
  buildQaSwarmHostedRunMeteringRow,
  buildQaSwarmHostedRunReceipt,
  publicQaSwarmHostedRunReceiptProjection,
} from './qa-swarm-hosted-run-receipt'

const runRef = 'qa-run.khala-code-nightly.20260702'

const exactMeteringRow = () =>
  buildQaSwarmHostedRunMeteringRow({
    rowRef: 'metering.qa_swarm.khala_code.20260702.turn_1',
    runRef,
    source: 'provider_usage',
    usageTruth: 'exact',
    inputTokens: 100,
    outputTokens: 20,
    reasoningTokens: 7,
    cacheReadTokens: 3,
  })

const hostedRunReceipt = () =>
  buildQaSwarmHostedRunReceipt({
    receiptRef: 'receipt.qa_swarm.hosted_run.khala_code.20260702',
    runRef,
    projectionRef: 'projection.qa_swarm.run.khala_code.20260702',
    verdict: 'warning',
    traceRefs: ['trace.qa_swarm.khala_code.seed_corpus.20260702'],
    coverageRefs: ['coverage.qa_swarm.khala_code.seed_corpus.20260702'],
    videoRefs: ['video.qa_swarm.khala_code.seed_corpus.20260702'],
    distilledTestRefs: ['test.qa_swarm.khala_code.distilled.20260702'],
    meteringRows: [exactMeteringRow()],
    publicSafetyRefs: ['redaction.qa_swarm.public_projection.reviewed.20260702'],
  })

describe('qa-swarm-hosted-run-receipt', () => {
  test('builds a hosted run receipt with trace, coverage, and exact metering refs', () => {
    const receipt = hostedRunReceipt()

    expect(receipt.receiptKind).toBe('qa_swarm_hosted_run')
    expect(receipt.traceRefs).toEqual([
      'trace.qa_swarm.khala_code.seed_corpus.20260702',
    ])
    expect(receipt.coverageRefs).toEqual([
      'coverage.qa_swarm.khala_code.seed_corpus.20260702',
    ])
    expect(receipt.meteringRowRefs).toEqual([
      'metering.qa_swarm.khala_code.20260702.turn_1',
    ])
    expect(receipt.exactTokenTotal).toBe(130)
    expect(receipt.settlement).toEqual({
      state: 'inert_owner_armed_required',
      movedMoney: false,
      ownerArmingRef: 'NEEDS_OWNER.qa_swarm_hosted_run_engagement_arming',
    })
  })

  test('rejects hosted run receipts without trace or coverage refs', () => {
    const base = {
      receiptRef: 'receipt.qa_swarm.hosted_run.khala_code.20260702',
      runRef,
      projectionRef: 'projection.qa_swarm.run.khala_code.20260702',
      verdict: 'passed' as const,
      traceRefs: ['trace.qa_swarm.khala_code.seed_corpus.20260702'],
      coverageRefs: ['coverage.qa_swarm.khala_code.seed_corpus.20260702'],
      meteringRows: [exactMeteringRow()],
      publicSafetyRefs: [
        'redaction.qa_swarm.public_projection.reviewed.20260702',
      ],
    }

    expect(() =>
      buildQaSwarmHostedRunReceipt({ ...base, traceRefs: [] }),
    ).toThrow(QaSwarmHostedRunReceiptInvariantError)
    expect(() =>
      buildQaSwarmHostedRunReceipt({ ...base, coverageRefs: [] }),
    ).toThrow(QaSwarmHostedRunReceiptInvariantError)
  })

  test('rejects estimated, missing, synthetic, or mismatched usage rows', () => {
    expect(() =>
      buildQaSwarmHostedRunMeteringRow({
        rowRef: 'metering.qa_swarm.khala_code.20260702.estimated',
        runRef,
        source: 'runner_report',
        usageTruth: 'estimated',
        inputTokens: 100,
        outputTokens: 20,
      }),
    ).toThrow(/exact-only/)

    expect(() =>
      buildQaSwarmHostedRunMeteringRow({
        rowRef: 'metering.qa_swarm.khala_code.20260702.synthetic',
        runRef,
        source: 'runner_report',
        usageTruth: 'synthetic',
        inputTokens: 100,
        outputTokens: 20,
      }),
    ).toThrow(/exact-only/)

    expect(() =>
      buildQaSwarmHostedRunMeteringRow({
        rowRef: 'metering.qa_swarm.khala_code.20260702.bad_total',
        runRef,
        source: 'provider_usage',
        usageTruth: 'exact',
        inputTokens: 100,
        outputTokens: 20,
        totalTokens: 999,
      }),
    ).toThrow(/totalTokens/)
  })

  test('requires metering rows to belong to the same run', () => {
    const row = buildQaSwarmHostedRunMeteringRow({
      rowRef: 'metering.qa_swarm.other.20260702.turn_1',
      runRef: 'qa-run.other-target.20260702',
      source: 'provider_usage',
      usageTruth: 'exact',
      inputTokens: 1,
      outputTokens: 1,
    })

    expect(() =>
      buildQaSwarmHostedRunReceipt({
        receiptRef: 'receipt.qa_swarm.hosted_run.khala_code.20260702',
        runRef,
        projectionRef: 'projection.qa_swarm.run.khala_code.20260702',
        verdict: 'passed',
        traceRefs: ['trace.qa_swarm.khala_code.seed_corpus.20260702'],
        coverageRefs: ['coverage.qa_swarm.khala_code.seed_corpus.20260702'],
        meteringRows: [row],
        publicSafetyRefs: [
          'redaction.qa_swarm.public_projection.reviewed.20260702',
        ],
      }),
    ).toThrow(/not qa-run\.khala-code-nightly\.20260702/)
  })

  test('bridges engagement receipts through quick-win lifecycle while settlement stays inert by default', () => {
    const engagement = buildQaSwarmEngagementReceipt({
      signupId: 'business_signup.qa_swarm.example.001',
      hostedRunReceipt: hostedRunReceipt(),
      quickWinScopedRef: 'scope.qa_swarm.audit.example.001',
      outcomeAcceptedRef: 'acceptance.qa_swarm.audit.example.001',
      buyerPaidRef: 'payment.qa_swarm.audit.example.001',
      providerSettledRef: 'settlement.qa_swarm.audit.example.001',
    })

    expect(engagement.paymentMode).toBe('inert_until_owner_armed')
    expect(engagement.movedMoney).toBe(false)
    expect(engagement.businessQuickWinReceipt.paidQuickWin).toBe(false)
    expect(engagement.businessQuickWinReceipt.unevidencedStateIds).toContain(
      'buyer_paid',
    )
    expect(engagement.businessQuickWinReceipt.unevidencedStateIds).toContain(
      'provider_settled',
    )
  })

  test('owner arming may evidence payment refs but still does not move money', () => {
    const engagement = buildQaSwarmEngagementReceipt({
      signupId: 'business_signup.qa_swarm.example.001',
      hostedRunReceipt: hostedRunReceipt(),
      quickWinScopedRef: 'scope.qa_swarm.audit.example.001',
      outcomeAcceptedRef: 'acceptance.qa_swarm.audit.example.001',
      buyerPaidRef: 'payment.qa_swarm.audit.example.001',
      providerSettledRef: 'settlement.qa_swarm.audit.example.001',
      ownerArmToken: QA_SWARM_OWNER_ARM_TOKEN,
    })

    expect(engagement.businessQuickWinReceipt.paidQuickWin).toBe(true)
    expect(engagement.businessQuickWinReceipt.evidencedStateCount).toBe(6)
    expect(engagement.paymentMode).toBe('inert_until_owner_armed')
    expect(engagement.movedMoney).toBe(false)
  })

  test('public hosted-run projection keeps refs and drops nothing private', () => {
    const projection = publicQaSwarmHostedRunReceiptProjection(hostedRunReceipt())

    expect(projection.exactTokenTotal).toBe(130)
    expect(projection.traceRefs).toHaveLength(1)
    expect(JSON.stringify(projection)).not.toContain('business_signup')
    expect(JSON.stringify(projection)).not.toContain('payment.qa_swarm')
  })
})
