import { describe, expect, test } from 'vitest'

import { buildForgeLiveRetrievalPlanInput } from './live-retrieval-adapters'
import { projectForgeRetrievalPlan } from './retrieval-plan'

const baseInput = {
  generatedAt: '2026-06-16T21:30:00.000Z',
  planRef: 'retrieval-plan.public.live_work_1',
  queryRefs: [
    'query.public.file_candidate',
    'query.public.doc',
    'query.public.diagnostic',
  ],
  requestRef: 'retrieval-request.public.live_work_1',
  workspaceBoundaryRefs: ['workspace-boundary.public.openagents.work_1'],
} as const

describe('Forge live retrieval adapters', () => {
  test('builds a bounded live file/docs/diagnostic retrieval plan deterministically', () => {
    const plan = projectForgeRetrievalPlan(
      buildForgeLiveRetrievalPlanInput({
        ...baseInput,
        freshness: 'fresh',
        sources: [
          {
            candidateRef: 'candidate.public.diagnostic_ts_error',
            exactRefs: ['query.public.diagnostic'],
            provenanceRefs: ['diagnostic.public.tsc.no_emit'],
            sourceKind: 'diagnostic',
            sourceRef: 'source.public.diagnostic.typecheck',
          },
          {
            candidateRef: 'candidate.public.docs_roadmap',
            exactRefs: ['query.public.doc'],
            provenanceRefs: ['docs.public.launch_roadmap'],
            sourceKind: 'documentation',
            sourceRef: 'source.public.docs.launch_roadmap',
          },
          {
            candidateRef: 'query.public.file_candidate',
            exactRefs: ['query.public.file_path'],
            provenanceRefs: ['file-index.public.autopilot_work'],
            sourceKind: 'file',
            sourceRef: 'source.public.file.autopilot_work',
          },
        ],
      }),
    )

    expect(plan.status).toBe('ready')
    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'query.public.file_candidate',
      'candidate.public.docs_roadmap',
      'candidate.public.diagnostic_ts_error',
    ])
    expect(plan.candidates.map(candidate => candidate.rank)).toEqual([1, 2, 3])
    expect(plan.candidates.map(candidate => candidate.provenanceRefs[0])).toEqual(
      [
        'retrieval-source-kind.file',
        'retrieval-source-kind.documentation',
        'retrieval-source-kind.diagnostic',
      ],
    )
    expect(plan.sourceRefs).toContain('workspace-boundary.public.openagents.work_1')
    expect(plan.sourceRefs).toContain('source.public.file.autopilot_work')
  })

  test('projects duplicate, stale, unsupported, missing-source, and low-score skips', () => {
    const plan = projectForgeRetrievalPlan(
      buildForgeLiveRetrievalPlanInput({
        ...baseInput,
        queryRefs: ['query.public.keep'],
        sources: [
          {
            candidateRef: 'candidate.public.keep',
            exactRefs: ['query.public.keep'],
            sourceKind: 'file',
            sourceRef: 'source.public.keep',
          },
          {
            candidateRef: 'candidate.public.keep',
            exactRefs: ['query.public.keep'],
            sourceKind: 'file',
            sourceRef: 'source.public.duplicate',
          },
          {
            candidateRef: 'candidate.public.stale',
            exactRefs: ['query.public.keep'],
            freshness: 'stale',
            sourceKind: 'documentation',
            sourceRef: 'source.public.stale',
          },
          {
            candidateRef: 'candidate.public.unsupported',
            exactRefs: ['query.public.keep'],
            sourceKind: 'unsupported',
            sourceRef: 'source.public.unsupported',
          },
          {
            candidateRef: 'candidate.public.missing_source',
            exactRefs: ['query.public.keep'],
            sourceKind: 'diagnostic',
          },
          {
            candidateRef: 'candidate.public.low_score',
            exactRefs: ['query.public.other'],
            sourceKind: 'file',
            sourceRef: 'source.public.low_score',
          },
        ],
      }),
    )

    expect(plan.status).toBe('ready')
    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'candidate.public.keep',
    ])
    expect(
      plan.skippedCandidates.map(candidate => [
        candidate.candidateRef,
        candidate.reason,
      ]),
    ).toEqual([
      ['candidate.public.keep', 'duplicate'],
      ['candidate.public.low_score', 'low_score'],
      ['candidate.public.missing_source', 'missing_source'],
      ['candidate.public.stale', 'stale'],
      ['candidate.public.unsupported', 'unsupported_mode'],
    ])
  })

  test('blocks live retrieval without workspace boundary or required provider evidence', () => {
    const semanticPlan = projectForgeRetrievalPlan(
      buildForgeLiveRetrievalPlanInput({
        ...baseInput,
        mode: 'semantic',
        providerEvidenceRefs: [],
        workspaceBoundaryRefs: [],
      }),
    )
    const semanticWithEvidence = projectForgeRetrievalPlan(
      buildForgeLiveRetrievalPlanInput({
        ...baseInput,
        mode: 'semantic',
        providerEvidenceRefs: ['provider-evidence.public.embedding_index'],
      }),
    )

    expect(semanticPlan.status).toBe('blocked')
    expect(semanticPlan.blockerRefs).toContain(
      'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:missing-workspace-boundary-ref',
    )
    expect(semanticPlan.blockerRefs).toContain(
      'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:missing-provider-evidence-ref',
    )
    expect(semanticWithEvidence.blockerRefs).not.toContain(
      'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:missing-provider-evidence-ref',
    )
  })

  test('does not perform keyword-only retrieval routing from prose query text', () => {
    const plan = projectForgeRetrievalPlan(
      buildForgeLiveRetrievalPlanInput({
        ...baseInput,
        queryRefs: ['query.public.unrelated_intent_ref'],
        sources: [
          {
            candidateRef: 'candidate.public.login_button',
            exactRefs: ['query.public.actual_login_ref'],
            provenanceRefs: ['docs.public.login'],
            sourceKind: 'documentation',
            sourceRef: 'source.public.login_docs',
          },
        ],
      }),
    )

    expect(plan.status).toBe('empty')
    expect(plan.candidates).toEqual([])
    expect(plan.skippedCandidates).toEqual([
      {
        blockerRefs: [],
        candidateRef: 'candidate.public.login_button',
        reason: 'low_score',
        sourceRef: 'source.public.login_docs',
      },
    ])
  })

  test('omits unsafe live adapter material before plan projection', () => {
    const planInput = buildForgeLiveRetrievalPlanInput({
      ...baseInput,
      queryRefs: ['query.public.safe', 'raw prompt /Users/christopher/private.md'],
      sources: [
        {
          candidateRef: 'candidate.public.safe',
          exactRefs: ['query.public.safe'],
          provenanceRefs: ['diff --git a/private.ts b/private.ts'],
          sourceKind: 'file',
          sourceRef: 'source.public.safe',
        },
        {
          candidateRef: '/Users/christopher/private/file.ts',
          exactRefs: ['query.public.safe'],
          sourceKind: 'file',
          sourceRef: 'source.public.private_candidate',
        },
        {
          candidateRef: 'candidate.public.source_omitted',
          exactRefs: ['query.public.safe'],
          sourceKind: 'documentation',
          sourceRef: '/Users/christopher/private/docs.md',
        },
      ],
    })
    const plan = projectForgeRetrievalPlan(planInput)
    const payload = JSON.stringify({ plan, planInput })

    expect(plan.status).toBe('blocked')
    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'candidate.public.safe',
    ])
    expect(plan.skippedCandidates).toEqual([
      {
        blockerRefs: [
          'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:unsafe-live-source-material-omitted',
        ],
        candidateRef: 'retrieval-candidate.filtered_private.2',
        reason: 'filtered_private',
        sourceRef: 'source.public.private_candidate',
      },
      {
        blockerRefs: [
          'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:unsafe-live-source-material-omitted',
        ],
        candidateRef: 'candidate.public.source_omitted',
        reason: 'missing_source',
        sourceRef: null,
      },
    ])
    expect(plan.blockerRefs).toContain(
      'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:unsafe-live-adapter-material-omitted',
    )
    expect(plan.blockerRefs).toContain(
      'forge-live-retrieval-adapter-blocker:retrieval-plan.public.live_work_1:unsafe-live-source-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw prompt')
  })
})
