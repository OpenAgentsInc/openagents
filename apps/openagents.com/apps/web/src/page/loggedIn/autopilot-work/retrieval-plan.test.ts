import { describe, expect, test } from 'vitest'

import {
  type ForgeRetrievalMode,
  type ForgeRetrievalPlanInput,
  projectForgeRetrievalPlan,
} from './retrieval-plan'

const baseInput = (
  overrides: Partial<ForgeRetrievalPlanInput> = {},
): ForgeRetrievalPlanInput => ({
  generatedAt: '2026-06-16T20:00:00.000Z',
  mode: 'exact',
  planRef: 'retrieval-plan.public.work_1',
  queryRefs: ['retrieval-query.public.work_1'],
  requestRef: 'retrieval-request.public.work_1',
  ...overrides,
})

describe('Forge retrieval plan projection', () => {
  test.each<ForgeRetrievalMode>([
    'exact',
    'structured',
    'semantic',
    'model_selected',
    'hybrid',
  ])('projects a ready refs-only %s retrieval plan', mode => {
    const plan = projectForgeRetrievalPlan(
      baseInput({
        candidates: [
          {
            candidateRef: `retrieval-candidate.public.${mode}`,
            provenanceRefs: [`retrieval-provenance.public.${mode}`],
            score: 0.82,
            sourceRef: `retrieval-source.public.${mode}`,
          },
        ],
        mode,
        sourceRefs: ['retrieval-source.public.seed'],
      }),
    )

    expect(plan).toMatchObject({
      blockerRefs: [],
      freshness: 'unknown',
      mode,
      omittedUnsafeRefCount: 0,
      planRef: 'retrieval-plan.public.work_1',
      queryRefs: ['retrieval-query.public.work_1'],
      requestRef: 'retrieval-request.public.work_1',
      status: 'ready',
    })
    expect(plan.candidates).toEqual([
      {
        blockerRefs: [],
        candidateRef: `retrieval-candidate.public.${mode}`,
        freshness: 'unknown',
        mode,
        provenanceRefs: [`retrieval-provenance.public.${mode}`],
        rank: null,
        score: 0.82,
        sourceRef: `retrieval-source.public.${mode}`,
      },
    ])
    expect(plan.resultSet).toEqual({
      selectedCandidateRefs: [`retrieval-candidate.public.${mode}`],
      skippedCandidateRefs: [],
      sourceRefs: [
        'retrieval-source.public.seed',
        `retrieval-source.public.${mode}`,
      ],
      totalSelected: 1,
      totalSkipped: 0,
    })
  })

  test('orders selected and skipped candidates deterministically', () => {
    const plan = projectForgeRetrievalPlan(
      baseInput({
        candidates: [
          {
            candidateRef: 'candidate.public.rank_2',
            rank: 2,
            score: 0.99,
            sourceRef: 'source.public.z',
          },
          {
            candidateRef: 'candidate.public.rank_1_lower_score',
            rank: 1,
            score: 0.5,
            sourceRef: 'source.public.a',
          },
          {
            candidateRef: 'candidate.public.rank_1_higher_score',
            rank: 1,
            score: 0.9,
            sourceRef: 'source.public.b',
          },
          {
            candidateRef: 'candidate.public.no_rank',
            score: 1,
            sourceRef: 'source.public.a',
          },
        ],
        skippedCandidates: [
          {
            candidateRef: 'candidate.public.skipped_z',
            reason: 'stale',
            sourceRef: 'source.public.z',
          },
          {
            candidateRef: 'candidate.public.skipped_a',
            reason: 'duplicate',
            sourceRef: 'source.public.a',
          },
          {
            candidateRef: 'candidate.public.skipped_b',
            reason: 'duplicate',
            sourceRef: 'source.public.b',
          },
        ],
      }),
    )

    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'candidate.public.rank_1_higher_score',
      'candidate.public.rank_1_lower_score',
      'candidate.public.rank_2',
      'candidate.public.no_rank',
    ])
    expect(plan.skippedCandidates.map(candidate => candidate.candidateRef)).toEqual(
      [
        'candidate.public.skipped_a',
        'candidate.public.skipped_b',
        'candidate.public.skipped_z',
      ],
    )
    expect(plan.resultSet.selectedCandidateRefs).toEqual([
      'candidate.public.rank_1_higher_score',
      'candidate.public.rank_1_lower_score',
      'candidate.public.rank_2',
      'candidate.public.no_rank',
    ])
    expect(plan.resultSet.skippedCandidateRefs).toEqual([
      'candidate.public.skipped_a',
      'candidate.public.skipped_b',
      'candidate.public.skipped_z',
    ])
    expect(plan.resultSet.sourceRefs).toEqual([
      'source.public.b',
      'source.public.a',
      'source.public.z',
    ])
  })

  test('reports blocked, stale, and empty retrieval-plan states', () => {
    const blocked = projectForgeRetrievalPlan(
      baseInput({
        blockerRefs: ['retrieval-blocker.public.index_unavailable'],
        queryRefs: [],
      }),
    )
    const stale = projectForgeRetrievalPlan(
      baseInput({
        candidates: [{ candidateRef: 'candidate.public.stale' }],
        freshness: 'stale',
      }),
    )
    const empty = projectForgeRetrievalPlan(baseInput())

    expect(blocked.status).toBe('blocked')
    expect(blocked.blockerRefs).toEqual([
      'retrieval-blocker.public.index_unavailable',
      'forge-retrieval-plan-blocker:retrieval-plan.public.work_1:missing-query-ref',
    ])
    expect(stale.status).toBe('stale')
    expect(empty.status).toBe('empty')
    expect(empty.blockerRefs).toEqual([])
  })

  test('omits unsafe private retrieval material before projection', () => {
    const plan = projectForgeRetrievalPlan(
      baseInput({
        blockerRefs: ['private repo content /Users/christopher/src/openagents'],
        candidates: [
          {
            blockerRefs: ['raw shell command $(cat ~/.ssh/id_rsa)'],
            candidateRef: '/Users/christopher/work/openagents/private.ts',
            provenanceRefs: ['diff --git a/private.ts b/private.ts'],
            sourceRef: 'source.public.safe',
          },
          {
            candidateRef: 'candidate.public.safe',
            provenanceRefs: [
              'retrieval-provenance.public.safe',
              'provider payload sk-private',
            ],
            sourceRef: 'raw file /Users/christopher/private.md',
          },
        ],
        planRef: 'diff --git a/plan b/plan',
        queryRefs: [
          'retrieval-query.public.safe',
          'raw prompt /Users/christopher/private.md',
        ],
        skippedCandidates: [
          {
            blockerRefs: ['token bearer secret'],
            candidateRef: 'candidate.public.skipped',
            reason: 'filtered_private',
            sourceRef: 'https://private.example.test/repo',
          },
        ],
        sourceRefs: [
          'retrieval-source.public.safe',
          '/Users/christopher/private/source',
        ],
      }),
    )
    const payload = JSON.stringify(plan)

    expect(plan.status).toBe('blocked')
    expect(plan.omittedUnsafeRefCount).toBe(11)
    expect(plan.planRef).toBe('unsafe-plan-ref-omitted')
    expect(plan.queryRefs).toEqual(['retrieval-query.public.safe'])
    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'candidate.public.safe',
    ])
    expect(plan.candidates[0]?.provenanceRefs).toEqual([
      'retrieval-provenance.public.safe',
    ])
    expect(plan.candidates[0]?.sourceRef).toBeNull()
    expect(plan.skippedCandidates).toEqual([
      {
        blockerRefs: [],
        candidateRef: 'candidate.public.skipped',
        reason: 'filtered_private',
        sourceRef: null,
      },
    ])
    expect(plan.blockerRefs).toContain(
      'forge-retrieval-plan-blocker:retrieval-request.public.work_1:unsafe-retrieval-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
    expect(payload).not.toContain('bearer secret')
    expect(payload).not.toContain('private.example')
  })
})
