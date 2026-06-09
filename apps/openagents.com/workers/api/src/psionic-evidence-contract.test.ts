import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_PSIONIC_EVIDENCE_ONLY_AUTHORITY,
  OpenAgentsPsionicEvidenceProjection,
  OpenAgentsPsionicEvidenceRecord,
  OpenAgentsPsionicEvidenceUnsafe,
  openAgentsPsionicAuthorityIsEvidenceOnly,
  openAgentsPsionicEvidenceCanMutateRuntime,
  openAgentsPsionicEvidenceNeedsReview,
  openAgentsPsionicEvidenceProjectionHasPrivateMaterial,
  projectOpenAgentsPsionicEvidence,
} from './psionic-evidence-contract'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T03:00:00.000Z'

const evidenceRecord = (
  overrides: Partial<OpenAgentsPsionicEvidenceRecord> = {},
): OpenAgentsPsionicEvidenceRecord =>
  S.decodeUnknownSync(OpenAgentsPsionicEvidenceRecord)({
    authority: OPENAGENTS_PSIONIC_EVIDENCE_ONLY_AUTHORITY,
    candidateModuleRefs: ['candidate_module.continuation_v2'],
    createdAtIso: '2026-06-07T02:30:00.000Z',
    datasetRefs: ['dataset.public_eval_fixture'],
    evidenceKind: 'optimizer_run',
    evidenceReceiptRefs: ['receipt.psionic.evidence_1'],
    failureRefs: [],
    fixtureRefs: ['fixture.continuation.release_gate'],
    id: 'psionic_evidence.optimizer_1',
    metricRefs: ['metric.scorecard.acceptance_rate'],
    modelRefs: ['model.qwen.local_adapter'],
    optimizerRefs: ['optimizer.gepa_style_reflection'],
    promotionProposalRefs: ['promotion_proposal.requires_operator_review'],
    providerRefs: ['provider.local_model_safe'],
    retainedFailureRefs: ['retained_failure.probe.timeout_1'],
    reviewRefs: ['review.operator_required'],
    rollbackRefs: ['rollback.anchor.module_v1'],
    scorecardRefs: ['scorecard.continuation_candidate'],
    sourceRefs: ['source.public_fixture_summary'],
    status: 'needs_review',
    trainingRunRefs: ['training_run.psionic.small_fixture'],
    updatedAtIso: '2026-06-07T02:45:00.000Z',
    ...overrides,
  })

describe('OpenAgents Psionic evidence contract', () => {
  test('projects evidence-only optimizer output without runtime mutation authority', () => {
    const record = evidenceRecord()
    const projection = projectOpenAgentsPsionicEvidence(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsPsionicEvidenceProjection)(projection))
      .toEqual(projection)
    expect(openAgentsPsionicAuthorityIsEvidenceOnly(record.authority)).toBe(
      true,
    )
    expect(openAgentsPsionicEvidenceCanMutateRuntime(record)).toBe(false)
    expect(openAgentsPsionicEvidenceNeedsReview(record)).toBe(true)
    expect(projection.evidenceOnly).toBe(true)
    expect(projection.promotionAllowed).toBe(false)
    expect(projection.routingMutationAllowed).toBe(false)
    expect(projection.settlementAllowed).toBe(false)
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(openAgentsPsionicEvidenceProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('redacts operator-only review, metric, and failure refs from public projection', () => {
    const projection = projectOpenAgentsPsionicEvidence(
      evidenceRecord({
        failureRefs: ['failure.operator.training_detail'],
        metricRefs: ['metric.operator.loss_curve_summary'],
        providerRefs: ['provider.private.local_gpu'],
        reviewRefs: ['review.operator_required'],
      }),
      'public',
      nowIso,
    )

    expect(projection.failureRefs).toEqual([])
    expect(projection.metricRefs).toEqual([])
    expect(projection.providerRefs).toEqual([])
    expect(projection.reviewRefs).toEqual([])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('does not require review for completed scorecard evidence without candidates or proposals', () => {
    const record = evidenceRecord({
      candidateModuleRefs: [],
      evidenceKind: 'scorecard',
      promotionProposalRefs: [],
      rollbackRefs: [],
      status: 'completed',
    })

    expect(openAgentsPsionicEvidenceNeedsReview(record)).toBe(false)
    expect(projectOpenAgentsPsionicEvidence(record, 'team', nowIso).status)
      .toBe('completed')
  })

  test('rejects raw datasets, private customer data, provider payloads, secrets, wallet/payment material, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw dataset', value: 'raw_dataset.training_dump' },
      { label: 'private dataset', value: 'dataset.private.customer_data' },
      { label: 'provider payload', value: 'provider_payload.full' },
    ]) {
      expect(() =>
        projectOpenAgentsPsionicEvidence(
          evidenceRecord({ datasetRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsPsionicEvidenceUnsafe)
    }
  })
})
