import { describe, expect, test } from 'vitest'

import {
  DataTraceMarketplaceGateUnsafe,
  dataTraceMarketplaceGateHasPrivateMaterial,
  deriveDataContributionTraceDigest,
  projectDataTraceMarketplaceGate,
  verifyDataContributionCorrectness,
} from './data-trace-marketplace-gate'

const sourceDigest =
  'sha256:1111111111111111111111111111111111111111111111111111111111111111'
const tamperedDigest =
  'sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'
const traceRows = [
  {
    evidenceRef: 'span.public.repo.readme_001',
    fieldRef: 'field.public.repo.path',
    valueDigest:
      'sha256:2222222222222222222222222222222222222222222222222222222222222222',
  },
]
const correctnessInputBase = {
  contributionRef: 'trace.public.data_market.submission_001',
  provenanceRefs: ['provenance.public.data_market.citation_001'],
  sourceDigest,
  sourceRefs: ['source.public.data_market.corpus_001'],
  transformRef: 'transform.public.data_market.derive_ref_rows_v1',
} as const

const publicSaleSmokeInput = {
  correctnessReceiptRefs: ['correctness.public.data_market.trace_sale_001'],
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
  test('verifies a re-derivable contribution by derived trace digest before settlement', async () => {
    const claimedTraceDigest = await deriveDataContributionTraceDigest({
      sourceDigest,
      traceRows,
      transformRef: correctnessInputBase.transformRef,
    })
    const correctnessVerification = await verifyDataContributionCorrectness({
      ...correctnessInputBase,
      claimedTraceDigest,
      derivedTraceRows: traceRows,
      verificationMode: 'derived_trace_replay',
    })
    const gate = projectDataTraceMarketplaceGate({
      ...publicSaleSmokeInput,
      correctnessReceiptRefs: [],
      correctnessVerification,
    })

    expect(correctnessVerification).toMatchObject({
      correctnessGatePassed: true,
      derivedTraceDigest: claimedTraceDigest,
      status: 'accepted',
      validatorReviewRequired: false,
    })
    expect(gate).toMatchObject({
      correctnessGatePassed: true,
      correctnessVerificationStatus: 'accepted',
      dataRevenueCopyAllowed: true,
      derivedTraceDigest: claimedTraceDigest,
      publicSafeDataSaleSmokePassed: true,
      settlementClaimAllowed: true,
      state: 'settled',
    })
    expect(gate.correctnessReceiptRefs).toEqual(
      correctnessVerification.correctnessReceiptRefs,
    )
    expect(gate.provenanceReceiptRefs).toHaveLength(1)
    expect(gate.dedupeReceiptRefs).toEqual([
      'dedupe.public.data_market.source.1111111111111111',
      `dedupe.public.data_market.trace.${claimedTraceDigest.replace('sha256:', '').slice(0, 16)}`,
    ])
    expect(gate.valuationRefs).toEqual(publicSaleSmokeInput.valuationRefs)
  })

  test('rejects a tampered derived trace digest before valuation can make it payable', async () => {
    const correctnessVerification = await verifyDataContributionCorrectness({
      ...correctnessInputBase,
      claimedTraceDigest: tamperedDigest,
      derivedTraceRows: traceRows,
      verificationMode: 'derived_trace_replay',
    })
    const gate = projectDataTraceMarketplaceGate({
      ...publicSaleSmokeInput,
      correctnessReceiptRefs: [],
      correctnessVerification,
    })

    expect(correctnessVerification).toMatchObject({
      correctnessGatePassed: false,
      status: 'rejected',
    })
    expect(correctnessVerification.blockerRefs).toContain(
      'blocker.public.data_market.derived_trace_digest_mismatch',
    )
    expect(gate).toMatchObject({
      correctnessGatePassed: false,
      correctnessVerificationStatus: 'rejected',
      dataRevenueCopyAllowed: false,
      settlementClaimAllowed: false,
      state: 'redacted',
    })
    expect(gate.correctnessReceiptRefs).toEqual([])
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.correctness_verdict_rejected',
    )
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.derived_trace_digest_mismatch',
    )
  })

  test('rejects unprovenanced contributions instead of minting correctness receipts', async () => {
    const claimedTraceDigest = await deriveDataContributionTraceDigest({
      sourceDigest,
      traceRows,
      transformRef: correctnessInputBase.transformRef,
    })
    const correctnessVerification = await verifyDataContributionCorrectness({
      contributionRef: correctnessInputBase.contributionRef,
      claimedTraceDigest,
      derivedTraceRows: traceRows,
      provenanceRefs: [],
      sourceDigest,
      sourceRefs: [],
      transformRef: correctnessInputBase.transformRef,
      verificationMode: 'derived_trace_replay',
    })
    const gate = projectDataTraceMarketplaceGate({
      ...publicSaleSmokeInput,
      correctnessReceiptRefs: [],
      correctnessVerification,
    })

    expect(correctnessVerification).toMatchObject({
      correctnessReceiptRefs: [],
      status: 'rejected',
    })
    expect(correctnessVerification.blockerRefs).toEqual([
      'blocker.public.data_market.provenance_ref_missing',
      'blocker.public.data_market.provenance_source_missing',
    ])
    expect(gate).toMatchObject({
      correctnessGatePassed: false,
      correctnessVerificationStatus: 'rejected',
      state: 'redacted',
    })
  })

  test('rejects duplicate contribution digests before payout evidence can settle', async () => {
    const claimedTraceDigest = await deriveDataContributionTraceDigest({
      sourceDigest,
      traceRows,
      transformRef: correctnessInputBase.transformRef,
    })
    const correctnessVerification = await verifyDataContributionCorrectness({
      ...correctnessInputBase,
      claimedTraceDigest,
      derivedTraceRows: traceRows,
      knownTraceDigests: [claimedTraceDigest],
      verificationMode: 'derived_trace_replay',
    })
    const gate = projectDataTraceMarketplaceGate({
      ...publicSaleSmokeInput,
      correctnessReceiptRefs: [],
      correctnessVerification,
    })

    expect(correctnessVerification).toMatchObject({
      correctnessReceiptRefs: [],
      dedupeReceiptRefs: [],
      status: 'rejected',
    })
    expect(correctnessVerification.blockerRefs).toContain(
      'blocker.public.data_market.duplicate_trace_digest',
    )
    expect(gate).toMatchObject({
      dataRevenueCopyAllowed: false,
      settlementClaimAllowed: false,
      state: 'redacted',
    })
  })

  test('routes nondeterministic data-contribution remainder to validator review', async () => {
    const correctnessVerification = await verifyDataContributionCorrectness({
      ...correctnessInputBase,
      claimedTraceDigest: tamperedDigest,
      validatorReviewRefs: ['validator_review.public.data_market.study_001'],
      verificationMode: 'validator_review_required',
    })
    const gate = projectDataTraceMarketplaceGate({
      ...publicSaleSmokeInput,
      correctnessReceiptRefs: [],
      correctnessVerification,
    })

    expect(correctnessVerification).toMatchObject({
      correctnessGatePassed: false,
      status: 'needs_validator_review',
      validatorReviewRequired: true,
      validatorReviewRefs: ['validator_review.public.data_market.study_001'],
    })
    expect(gate).toMatchObject({
      correctnessGatePassed: false,
      correctnessVerificationStatus: 'needs_validator_review',
      state: 'redacted',
      validatorReviewRequired: true,
      validatorReviewRefs: ['validator_review.public.data_market.study_001'],
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.validator_review_required',
    )
  })

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
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.correctness_verdict_missing',
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

  test('keeps correctness separate from valuation', () => {
    const gate = projectDataTraceMarketplaceGate({
      plannerMode: 'structured_query_planner',
      redactionReceiptRefs: ['redaction.public.data_market.trace_sale_001'],
      semanticPlannerRefs: ['planner.public.data_market.structured_001'],
      traceSubmissionRefs: ['trace.public.data_market.submission_001'],
      valuationRefs: ['valuation.public.data_market.trace_sale_001'],
    })

    expect(gate).toMatchObject({
      correctnessGatePassed: false,
      dataRevenueCopyAllowed: false,
      publicSafeDataSaleSmokePassed: false,
      state: 'redacted',
      valuationPayoutClaimAllowed: false,
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.correctness_verdict_missing',
    )
    expect(gate.caveatRefs).toContain(
      'caveat.public.data_market.correctness_verdict_required',
    )
  })

  test('keeps valuation separate from payout after correctness passes', () => {
    const gate = projectDataTraceMarketplaceGate({
      correctnessReceiptRefs: ['correctness.public.data_market.trace_sale_001'],
      plannerMode: 'structured_query_planner',
      redactionReceiptRefs: ['redaction.public.data_market.trace_sale_001'],
      semanticPlannerRefs: ['planner.public.data_market.structured_001'],
      traceSubmissionRefs: ['trace.public.data_market.submission_001'],
      valuationRefs: ['valuation.public.data_market.trace_sale_001'],
    })

    expect(gate).toMatchObject({
      correctnessGatePassed: true,
      correctnessVerificationStatus: 'accepted',
      dataRevenueCopyAllowed: false,
      publicSafeDataSaleSmokePassed: false,
      state: 'valued',
      valuationPayoutClaimAllowed: false,
    })
    expect(gate.caveatRefs).toContain(
      'caveat.public.data_market.valuation_is_not_payout',
    )
  })

  test('routes non-deterministic correctness remainder to validator review', () => {
    const gate = projectDataTraceMarketplaceGate({
      plannerMode: 'typed_semantic_selector',
      redactionReceiptRefs: ['redaction.public.data_market.trace_sale_001'],
      semanticPlannerRefs: ['planner.public.data_market.semantic_selector_001'],
      traceSubmissionRefs: ['trace.public.data_market.submission_001'],
      valuationRefs: ['valuation.public.data_market.trace_sale_001'],
      validatorReviewRefs: ['validator_review.public.data_market.study_001'],
    })

    expect(gate).toMatchObject({
      correctnessGatePassed: false,
      state: 'redacted',
      validatorReviewRequired: true,
      validatorReviewRefs: ['validator_review.public.data_market.study_001'],
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.data_market.correctness_verdict_missing',
    )
    expect(gate.caveatRefs).toContain(
      'caveat.public.data_market.validator_review_for_nondeterministic_remainder',
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

  test('rejects unsafe verifier refs before projecting correctness state', async () => {
    await expect(
      verifyDataContributionCorrectness({
        ...correctnessInputBase,
        claimedTraceDigest: tamperedDigest,
        sourceRefs: ['source_raw.private_customer_trace'],
        validatorReviewRefs: ['validator_review.public.data_market.study_001'],
        verificationMode: 'validator_review_required',
      }),
    ).rejects.toThrow(DataTraceMarketplaceGateUnsafe)
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
