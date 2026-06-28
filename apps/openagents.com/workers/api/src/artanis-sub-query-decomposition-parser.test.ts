import { describe, expect, test } from 'vitest'

import {
  ARTANIS_SUB_QUERY_DECOMPOSITION_SCHEMA,
  parseArtanisSubQueryDecomposition,
} from './artanis-sub-query-decomposition-parser'

const observedAt = '2026-06-28T18:20:00.000Z'

describe('Artanis sub-query decomposition parser (#6680)', () => {
  test('normalizes structured Blueprint-governed sub-query candidates into public refs', () => {
    const projection = parseArtanisSubQueryDecomposition({
      blueprintSignatureRefs: [
        'program_signature.frlm_conductor.v1',
        'program_signature.rlm_leaf_executor.v1',
      ],
      decomposition: {
        subQueries: [
          {
            evidenceRefs: ['evidence.operator.context_pack.public_status'],
            kind: 'collect_context',
            objective: 'Collect the public-safe context refs needed to answer the owner.',
            subQueryRef: 'subquery.artanis.rlm.collect_context.issue_6680',
          },
          {
            dependsOn: ['subquery.artanis.rlm.collect_context.issue_6680'],
            executor: 'pylon',
            kind: 'execute_leaf_query',
            objective: 'Ask a leaf executor to summarize the relevant public evidence.',
          },
          {
            kind: 'compose_answer',
            objective: 'Compose the final owner answer from sub-query results.',
          },
        ],
      },
      observedAt,
      rootObjective:
        'Explain the RLM decomposition plan for Artanis without relying on one bounded completion.',
      rootTaskRef: 'task.artanis.rlm.issue_6680',
    })

    expect(projection.schema).toBe(ARTANIS_SUB_QUERY_DECOMPOSITION_SCHEMA)
    expect(projection.canSchedule).toBe(true)
    expect(projection.blockerRefs).toEqual([])
    expect(projection.rootObjectiveDigestRef).toMatch(
      /^digest\.artanis\.root_objective\.[a-f0-9]{20}$/,
    )
    expect(projection.recursiveSubQueryRefs).toHaveLength(3)
    expect(projection.subQueries.map(subQuery => subQuery.kind)).toEqual([
      'collect_context',
      'execute_leaf_query',
      'compose_answer',
    ])
    expect(projection.subQueries[0]?.objectiveDigestRef).toMatch(
      /^digest\.artanis\.sub_query_objective\.[a-f0-9]{20}$/,
    )
    expect(projection.subQueries[1]?.executor).toBe('pylon')
    expect(projection.subQueries[1]?.dependsOn).toEqual([
      'subquery.artanis.rlm.collect_context.issue_6680',
    ])
    expect(JSON.stringify(projection)).not.toContain('Collect the public-safe context')
    expect(projection.authorityBoundary).toContain('does not dispatch workers')
    expect(projection.contentRedacted).toBe(true)
  })

  test('parses a bounded markdown decomposition with signature and dependency metadata', () => {
    const projection = parseArtanisSubQueryDecomposition({
      blueprintSignatureRefs: ['program_signature.frlm_conductor.v1'],
      decomposition: [
        '- collect_context: Read public issue and RLM architecture refs | ref=subquery.artanis.rlm.context.issue_6680 | evidence=evidence.issue.6680.public',
        '- inspect_blueprint_signature: Resolve the governing Blueprint signature | signature=program_signature.frlm_conductor.v1',
        '- execute_leaf_query: Run the leaf evidence summary | executor=pylon | depends=subquery.artanis.rlm.context.issue_6680',
        '- compose_answer: Compose the bounded public-safe response',
      ].join('\n'),
      observedAt,
      rootObjective: 'Build an Artanis RLM sub-query plan.',
      rootTaskRef: 'task.artanis.rlm.issue_6680',
    })

    expect(projection.canSchedule).toBe(true)
    expect(projection.subQueries).toHaveLength(4)
    expect(projection.subQueries[0]?.subQueryRef).toBe(
      'subquery.artanis.rlm.context.issue_6680',
    )
    expect(projection.subQueries[1]?.blueprintSignatureRef).toBe(
      'program_signature.frlm_conductor.v1',
    )
    expect(projection.subQueries[2]?.executor).toBe('pylon')
    expect(projection.evidenceRefs).toContain('evidence.issue.6680.public')
    expect(projection.releaseGateRefs).toEqual([
      'release_gate.artanis.sub_query_decomposition.public_refs_only',
      'release_gate.blueprint.signature_lookup.safe_projection',
    ])
  })

  test('blocks malformed or unsafe decomposition material instead of exposing it', () => {
    const projection = parseArtanisSubQueryDecomposition({
      blueprintSignatureRefs: ['program_signature.frlm_conductor.v1'],
      decomposition: {
        subQueries: [
          {
            evidenceRefs: ['evidence.safe.public'],
            kind: 'execute_leaf_query',
            objective: 'Use raw_prompt from /Users/operator/private/session.txt',
            subQueryRef: '/Users/operator/private/subquery.json',
          },
        ],
      },
      observedAt,
      rootObjective: 'Parse the owner task safely.',
      rootTaskRef: 'task.artanis.rlm.issue_6680',
    })

    expect(projection.canSchedule).toBe(false)
    expect(projection.subQueries).toEqual([])
    expect(projection.recursiveSubQueryRefs).toEqual([])
    expect(projection.blockerRefs).toContain(
      'blocker.artanis.sub_query_decomposition.unsafe_ref',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.artanis.sub_query_decomposition.plan_missing',
    )
    expect(JSON.stringify(projection)).not.toContain('/Users/operator')
    expect(JSON.stringify(projection)).not.toContain('raw_prompt')
  })

  test('blocks missing roots and non-parseable plans', () => {
    const projection = parseArtanisSubQueryDecomposition({
      decomposition: 'this is not a sub-query list',
      observedAt,
      rootObjective: 'Build an Artanis RLM plan.',
    })

    expect(projection.canSchedule).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.artanis.sub_query_decomposition.root_task_ref_missing',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.artanis.sub_query_decomposition.parse_failed',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.artanis.sub_query_decomposition.plan_missing',
    )
  })
})
