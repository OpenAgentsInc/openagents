import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeEvaluationRegressionEvidenceInput,
  projectForgeEvaluationRegressionEvidence,
} from './evaluation-regression-evidence'

const baseInput = {
  generatedAt: '2026-06-18T04:40:00.000Z',
  snapshotRef: 'evaluation-regression-snapshot.public.work_1',
  versionRef: 'evaluation-regression-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const passedEntry = {
  adapterRefs: ['adapter.public.pylon'],
  artifactRefs: ['eval-artifact.public.summary'],
  budgetPolicyRefs: ['budget-policy.public.equivalent'],
  costSummaryRefs: ['eval-cost.public.summary'],
  evaluationRef: 'evaluation.public.suite_small',
  firstDivergenceRefs: ['first-divergence.public.none'],
  fixtureProvenanceRefs: ['fixture-provenance.public.redacted_failure'],
  fixtureRedactionRefs: ['fixture-redaction.public.reviewed'],
  fixtureRefs: ['fixture.public.redacted_task'],
  freshness: 'fresh' as const,
  latencySummaryRefs: ['eval-latency.public.summary'],
  modelRefs: ['model.public.gpt'],
  productClaimRefs: ['product-claim.public.bounded_eval'],
  providerRefs: ['provider.public.openai'],
  publicReportRefs: ['eval-report.public.summary'],
  resultVerdictRefs: ['eval-result.public.solved'],
  safetyVerdictRefs: ['eval-safety.public.public_safe'],
  status: 'passed' as const,
  suiteRefs: ['eval-suite.public.small'],
  toolPolicyRefs: ['tool-policy.public.equivalent'],
  versionRefs: ['runtime-version.public.v1'],
}

describe('Forge evaluation and regression evidence projection', () => {
  test('projects eval evidence as refs-only non-authoritative state', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      entries: [passedEntry],
    })

    expect(view.status).toBe('passed')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      entries: 1,
      failed: 0,
      passed: 1,
      pending: 0,
      publicReports: 1,
      regressed: 0,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      evalExecutionAuthority: false,
      evalSuiteLoadAuthority: false,
      fixturePromotionAuthority: false,
      modelProviderCallAuthority: false,
      productPromiseMutationAuthority: false,
      publicClaimMutationAuthority: false,
      regressionGateMutationAuthority: false,
      releaseGateEnforcementAuthority: false,
      reportGenerationAuthority: false,
      settlementAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing evaluation state as empty', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      generatedAt: '2026-06-18T04:40:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks public eval claims missing required evidence refs', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      entries: [
        {
          evaluationRef: 'evaluation.public.incomplete_claim',
          productClaimRefs: ['product-claim.public.eval'],
          status: 'passed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.work_1:public-eval-claim-missing-evidence:evaluation.public.incomplete_claim',
    )
  })

  test('blocks provider and model comparisons without policy equivalence refs', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          budgetPolicyRefs: [],
          evaluationRef: 'evaluation.public.comparison',
          modelRefs: ['model.public.a', 'model.public.b'],
          providerRefs: ['provider.public.a', 'provider.public.b'],
          toolPolicyRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.work_1:comparison-missing-policy-equivalence:evaluation.public.comparison',
    )
  })

  test('blocks regression gate failures without threshold and blocker refs', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          blockerRefs: [],
          evaluationRef: 'evaluation.public.regressed',
          failureRefs: [],
          regressionGateRefs: ['regression-gate.public.release'],
          status: 'regressed',
          thresholdRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.work_1:regression-gate-missing-threshold-blocker:evaluation.public.regressed',
    )
  })

  test('blocks fixture promotion without review and redaction refs', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          evaluationRef: 'evaluation.public.promotion',
          fixturePromotionRefs: ['fixture-promotion.public.failure_1'],
          fixtureRedactionRefs: [],
          reviewRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.work_1:fixture-promotion-missing-review-redaction:evaluation.public.promotion',
    )
  })

  test('blocks stale evaluation evidence', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      entries: [
        {
          ...passedEntry,
          evaluationRef: 'evaluation.public.stale',
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.work_1:stale-evaluation-evidence:evaluation.public.stale',
    )
  })

  test('blocks populated eval entries without snapshot refs', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      entries: [passedEntry],
      generatedAt: '2026-06-18T04:40:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.no_snapshot:missing-evaluation-regression-evidence-snapshot-ref',
    )
  })

  test('omits unsafe private eval material before projection', () => {
    const view = projectForgeEvaluationRegressionEvidence({
      ...baseInput,
      blockerRefs: [
        'eval-blocker.public.safe',
        'raw transcript /Users/christopher/eval.log',
      ],
      entries: [
        {
          ...passedEntry,
          artifactRefs: ['eval-artifact.public.safe', 'artifact content /Users/christopher/artifact.json'],
          evaluationRef: 'evaluation.public.safe',
          fixtureRefs: ['fixture.public.safe', 'fixture body customer data private'],
          privateReportRefs: ['eval-report.private.safe', 'provider payload sk-private'],
          publicReportRefs: ['eval-report.public.safe'],
          resultVerdictRefs: ['eval-result.public.safe'],
          suiteRefs: ['eval-suite.public.safe', 'task body bearer token private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.artifactRefs).toEqual(['eval-artifact.public.safe'])
    expect(view.entries[0]?.fixtureRefs).toEqual(['fixture.public.safe'])
    expect(view.entries[0]?.publicReportRefs).toEqual(['eval-report.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-evaluation-regression-blocker:work.public.work_1:unsafe-evaluation-regression-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('artifact content')
    expect(payload).not.toContain('fixture body')
    expect(payload).not.toContain('customer data')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('task body')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      evaluationRegressionEvidence: {
        entries: [passedEntry],
        generatedAt: '2026-06-18T04:41:00.000Z',
        snapshotRef: 'evaluation-regression-snapshot.public.work_2',
        versionRef: 'evaluation-regression-version.public.v2',
      },
      generatedAt: '2026-06-18T04:40:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeEvaluationRegressionEvidenceInput(work)).toEqual({
      entries: [passedEntry],
      generatedAt: '2026-06-18T04:41:00.000Z',
      snapshotRef: 'evaluation-regression-snapshot.public.work_2',
      versionRef: 'evaluation-regression-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
