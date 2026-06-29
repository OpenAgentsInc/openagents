import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisModelLabContextProjection,
  ArtanisModelLabContextRecord,
  ArtanisModelLabContextUnsafe,
  artanisModelLabContextProjectionHasPrivateMaterial,
  exampleArtanisModelLabContext,
  projectArtanisModelLabContext,
} from './artanis-model-lab-context'
import { exampleOmniBenchmarkCloud } from './omni-model-lab-benchmark-cloud'
import { exampleOmniModelArtifact } from './omni-model-lab-model-artifact'
import { exampleOmniModelLabEvidenceGraph } from './omni-model-lab-evidence-graph'
import { exampleOmniModelLabReport } from './omni-model-lab-report'
import { exampleOmniModelLabRetainedFailureLoop } from './omni-model-lab-retained-failure-loop'
import { exampleOmniPromotionDecisionLedger } from './omni-model-lab-promotion-decision'
import { exampleOmniTrainingRun } from './omni-model-lab-training-run'

const nowIso = '2026-06-07T02:00:00.000Z'

const examples = () => ({
  benchmarkCloud: exampleOmniBenchmarkCloud(),
  evidenceGraph: exampleOmniModelLabEvidenceGraph(),
  modelArtifact: exampleOmniModelArtifact(),
  promotionDecisionLedger: exampleOmniPromotionDecisionLedger(),
  publicReport: exampleOmniModelLabReport(),
  retainedFailureLoop: exampleOmniModelLabRetainedFailureLoop(),
  trainingRun: exampleOmniTrainingRun(),
})

const contextRecord = (
  overrides: Partial<ArtanisModelLabContextRecord> = {},
): ArtanisModelLabContextRecord =>
  S.decodeUnknownSync(ArtanisModelLabContextRecord)({
    ...exampleArtanisModelLabContext(examples()),
    ...overrides,
  })

describe('Artanis Model Lab context bridge', () => {
  test('projects all implemented Model Lab contracts into private Artanis context without authority', () => {
    const projection = projectArtanisModelLabContext(
      exampleArtanisModelLabContext(examples()),
      'private_loop',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisModelLabContextProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      agentId: 'agent_artanis',
      consumedContractRefs: [
        'contract.public.model_lab.benchmark_cloud',
        'contract.public.model_lab.evidence_graph',
        'contract.public.model_lab.model_artifact',
        'contract.public.model_lab.promotion_decision',
        'contract.public.model_lab.public_report',
        'contract.public.model_lab.retained_failure_loop',
        'contract.public.model_lab.training_run',
      ],
      missingContractRefs: [],
      publicForumSummaryReportRefs: ['report.public.model_lab_autopilot_v2'],
      publicPromotionClaimRefs: ['report.public.model_lab_autopilot_v2'],
      readiness: 'ready',
    })
    expect(projection.retainedFailureLoop?.retainedFailureCount).toBe(1)
    expect(projection.modelArtifacts).toHaveLength(1)
    expect(projection.trainingRuns).toHaveLength(1)
    expect(projection.evidenceGraph?.connected).toBe(true)
    expect(projection.benchmarkCloud?.promotionBlocked).toBe(false)
    expect(projection.promotionDecisionLedger?.claimState).toBe(
      'passed_not_deployed',
    )
    expect(projection.publicReport?.readiness).toBe('complete')
    expect(projection.operatorNextActions.map(action => action.kind)).toEqual([
      'inspect_retained_failure',
      'draft_public_forum_summary',
      'request_operator_promotion_review',
    ])
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(JSON.stringify(projection)).not.toContain('"trainingLaunchAllowed":true')
    expect(JSON.stringify(projection)).not.toContain('"evalExecutionAllowed":true')
    expect(JSON.stringify(projection)).not.toContain('"runtimePromotionAllowed":true')
    expect(artanisModelLabContextProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('uses only the public Model Lab report projection for Forum and public Artanis audiences', () => {
    const forumProjection = projectArtanisModelLabContext(
      contextRecord({
        privateEvidenceRefs: ['evidence.private.operator_model_lab_packet'],
      }),
      'public_forum',
      nowIso,
    )
    const publicArtanisProjection = projectArtanisModelLabContext(
      contextRecord({
        privateEvidenceRefs: ['evidence.private.operator_model_lab_packet'],
      }),
      'public_artanis',
      nowIso,
    )

    for (const projection of [forumProjection, publicArtanisProjection]) {
      expect(projection.publicReport?.reportRef).toBe(
        'report.public.model_lab_autopilot_v2',
      )
      expect(projection.retainedFailureLoop).toBeNull()
      expect(projection.modelArtifacts).toEqual([])
      expect(projection.trainingRuns).toEqual([])
      expect(projection.evidenceGraph).toBeNull()
      expect(projection.benchmarkCloud).toBeNull()
      expect(projection.promotionDecisionLedger).toBeNull()
      expect(projection.privateEvidenceRefs).toEqual([])
      expect(projection.operatorNextActions).toEqual([])
      expect(JSON.stringify(projection)).not.toContain('evidence.private')
      expect(artanisModelLabContextProjectionHasPrivateMaterial(projection))
        .toBe(false)
    }
  })

  test('turns missing contracts and missing evidence into blockers, not public promotion claims', () => {
    const base = examples()
    const partialSection = {
      ...base.publicReport.sections[0]!,
      caveatRefs: ['caveat.public.retained_failure_needs_rerun'],
      evidenceRefs: [],
      missingEvidenceRefs: ['missing.public.retained_failure_rerun'],
      readiness: 'partial' as const,
    }
    const partialReport = {
      ...base.publicReport,
      claimState: 'missing_evidence' as const,
      missingEvidenceRefs: ['missing.public.retained_failure_rerun'],
      readiness: 'partial' as const,
      sections: [partialSection, ...base.publicReport.sections.slice(1)],
    }
    const projection = projectArtanisModelLabContext(
      contextRecord({
        benchmarkCloud: null,
        modelArtifacts: [],
        publicReport: partialReport,
      }),
      'private_loop',
      nowIso,
    )

    expect(projection.readiness).toBe('blocked')
    expect(projection.missingContractRefs).toEqual([
      'contract.public.model_lab.benchmark_cloud',
      'contract.public.model_lab.model_artifact',
    ])
    expect(projection.missingEvidenceRefs).toEqual([
      'missing.public.retained_failure_rerun',
    ])
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.artanis.missing_benchmark_cloud',
        'blocker.public.artanis.missing_model_artifact',
      ]),
    )
    expect(projection.publicPromotionClaimRefs).toEqual([])
    expect(projection.operatorNextActions.map(action => action.kind)).toEqual(
      expect.arrayContaining([
        'request_missing_contracts',
        'request_missing_evidence',
        'inspect_retained_failure',
      ]),
    )
  })

  test('drafts an operator eval-rerun action from retained failures when eval evidence is absent', () => {
    const retainedOnly = {
      ...exampleOmniModelLabRetainedFailureLoop(),
      adapterValidations: [],
      attributions: [],
      candidateRecords: [],
      evalReruns: [],
      promotionGates: [],
      state: 'retained' as const,
    }
    const projection = projectArtanisModelLabContext(
      contextRecord({
        publicReport: {
          ...exampleOmniModelLabReport(),
          claimState: 'no_public_claim',
          readiness: 'complete',
        },
        retainedFailureLoop: retainedOnly,
      }),
      'private_loop',
      nowIso,
    )

    expect(projection.operatorNextActions.map(action => action.kind)).toEqual(
      expect.arrayContaining(['draft_eval_rerun']),
    )
    expect(
      projection.operatorNextActions.find(
        action => action.kind === 'draft_eval_rerun',
      )?.requiresOperatorApproval,
    ).toBe(true)
    expect(projection.publicPromotionClaimRefs).toEqual([])
  })

  test('rejects non-Artanis context, unsafe refs, and false authority in consumed Model Lab records', () => {
    const badArtifact = {
      ...exampleOmniModelArtifact(),
      authority: {
        ...exampleOmniModelArtifact().authority,
        noRuntimePromotion: false,
      },
    }

    expect(() =>
      projectArtanisModelLabContext(
        contextRecord({ agentId: 'agent_adjutant' }),
        'private_loop',
        nowIso,
      ),
    ).toThrow(ArtanisModelLabContextUnsafe)
    expect(() =>
      projectArtanisModelLabContext(
        contextRecord({ caveatRefs: ['raw_prompt.customer'] }),
        'private_loop',
        nowIso,
      ),
    ).toThrow(ArtanisModelLabContextUnsafe)
    expect(() =>
      projectArtanisModelLabContext(
        contextRecord({ modelArtifacts: [badArtifact] }),
        'private_loop',
        nowIso,
      ),
    ).toThrow(ArtanisModelLabContextUnsafe)
  })
})
