import { describe, expect, test } from 'vitest'

import { projectForgeExactRetrievalPlan } from './exact-retrieval-adapters'

const baseInput = {
  generatedAt: '2026-06-16T21:00:00.000Z',
  planRef: 'retrieval-plan.public.exact_work_1',
  queryRefs: ['query.public.file_candidate', 'query.public.doc', 'query.public.repo'],
  requestRef: 'retrieval-request.public.exact_work_1',
} as const

describe('Forge exact retrieval adapters', () => {
  test('ranks file, documentation, and repository fixtures deterministically', () => {
    const plan = projectForgeExactRetrievalPlan({
      ...baseInput,
      fixtures: [
        {
          candidateRef: 'candidate.public.repo_openagents',
          exactRefs: ['query.public.repo'],
          provenanceRefs: ['provenance.public.repo_index'],
          sourceKind: 'repository',
          sourceRef: 'source.public.repo_openagents',
        },
        {
          candidateRef: 'query.public.file_candidate',
          exactRefs: ['query.public.file_path'],
          provenanceRefs: ['provenance.public.file_index'],
          sourceKind: 'file',
          sourceRef: 'source.public.file_progress',
        },
        {
          candidateRef: 'candidate.public.docs_roadmap',
          exactRefs: ['query.public.doc'],
          provenanceRefs: ['provenance.public.docs_index'],
          sourceKind: 'documentation',
          sourceRef: 'source.public.docs_roadmap',
        },
      ],
    })

    expect(plan.status).toBe('ready')
    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'query.public.file_candidate',
      'candidate.public.docs_roadmap',
      'candidate.public.repo_openagents',
    ])
    expect(plan.candidates.map(candidate => candidate.rank)).toEqual([1, 2, 3])
    expect(plan.candidates.map(candidate => candidate.score)).toEqual([
      1,
      0.85,
      0.85,
    ])
    expect(plan.candidates.map(candidate => candidate.provenanceRefs[0])).toEqual(
      [
        'retrieval-source-kind.file',
        'retrieval-source-kind.documentation',
        'retrieval-source-kind.repository',
      ],
    )
    expect(plan.resultSet).toMatchObject({
      selectedCandidateRefs: [
        'query.public.file_candidate',
        'candidate.public.docs_roadmap',
        'candidate.public.repo_openagents',
      ],
      skippedCandidateRefs: [],
      totalSelected: 3,
      totalSkipped: 0,
    })
  })

  test('projects duplicate, filtered, unsupported, missing-source, and low-score skips', () => {
    const plan = projectForgeExactRetrievalPlan({
      ...baseInput,
      queryRefs: ['query.public.keep'],
      fixtures: [
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
          candidateRef: 'candidate.public.filtered',
          exactRefs: ['query.public.keep'],
          skipReason: 'filtered_private',
          sourceKind: 'documentation',
          sourceRef: 'source.public.filtered',
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
          sourceKind: 'repository',
        },
        {
          candidateRef: 'candidate.public.low_score',
          exactRefs: ['query.public.other'],
          sourceKind: 'file',
          sourceRef: 'source.public.low_score',
        },
      ],
    })

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
      ['candidate.public.filtered', 'filtered_private'],
      ['candidate.public.low_score', 'low_score'],
      ['candidate.public.missing_source', 'missing_source'],
      ['candidate.public.unsupported', 'unsupported_mode'],
    ])
    expect(plan.resultSet).toMatchObject({
      skippedCandidateRefs: [
        'candidate.public.keep',
        'candidate.public.filtered',
        'candidate.public.low_score',
        'candidate.public.missing_source',
        'candidate.public.unsupported',
      ],
      totalSelected: 1,
      totalSkipped: 5,
    })
  })

  test('omits unsafe adapter refs without leaking private retrieval material', () => {
    const plan = projectForgeExactRetrievalPlan({
      ...baseInput,
      queryRefs: ['query.public.safe', 'raw prompt /Users/christopher/private.md'],
      fixtures: [
        {
          candidateRef: 'candidate.public.safe',
          exactRefs: ['query.public.safe'],
          provenanceRefs: ['diff --git a/private.ts b/private.ts'],
          sourceKind: 'file',
          sourceRef: 'source.public.safe',
        },
        {
          candidateRef: 'candidate.public.filtered',
          exactRefs: ['query.public.safe'],
          skipReason: 'filtered_private',
          sourceKind: 'documentation',
          sourceRef: '/Users/christopher/private/docs.md',
        },
        {
          candidateRef: '/Users/christopher/private/file.ts',
          exactRefs: ['query.public.safe'],
          sourceKind: 'file',
          sourceRef: 'source.public.private_candidate',
        },
      ],
    })
    const payload = JSON.stringify(plan)

    expect(plan.status).toBe('blocked')
    expect(plan.omittedUnsafeRefCount).toBe(4)
    expect(plan.candidates.map(candidate => candidate.candidateRef)).toEqual([
      'candidate.public.safe',
    ])
    expect(plan.candidates[0]?.provenanceRefs).toEqual([
      'retrieval-source-kind.file',
    ])
    expect(plan.skippedCandidates).toEqual([
      {
        blockerRefs: [],
        candidateRef: 'candidate.public.filtered',
        reason: 'filtered_private',
        sourceRef: null,
      },
    ])
    expect(plan.blockerRefs).toContain(
      'forge-retrieval-plan-blocker:retrieval-plan.public.exact_work_1:unsafe-retrieval-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw prompt')
  })
})
