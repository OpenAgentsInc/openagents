import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  FRLM_CONDUCTOR_SIGNATURE_REF,
  FrlmConductor,
  FrlmConductorError,
  RLM_LEAF_EXECUTOR_SIGNATURE_REF,
  defaultArtanisFrlmPolicy,
  makeStaticFrlmPlanner,
  type FrlmConductorRunInput,
  type FrlmLeafExecutor,
  type FrlmSubQuery,
} from './artanis-frlm-conductor'

const subQueries: ReadonlyArray<FrlmSubQuery> = [
  {
    depth: 1,
    executor: 'local',
    input: 'Summarize operator memory relevant to the owner request.',
    purpose: 'memory-grounding',
    signatureRef: RLM_LEAF_EXECUTOR_SIGNATURE_REF,
    subQueryId: 'sq.memory',
  },
  {
    depth: 1,
    executor: 'codex',
    input: 'Inspect current Codex/Pylon backlog evidence refs.',
    purpose: 'backlog-grounding',
    signatureRef: RLM_LEAF_EXECUTOR_SIGNATURE_REF,
    subQueryId: 'sq.backlog',
  },
]

const input = (overrides?: {
  policy?: Partial<FrlmConductorRunInput['policy']>
}): FrlmConductorRunInput => {
  const policy = defaultArtanisFrlmPolicy({
    evidenceRefs: ['evidence.autonomous_ops_v1.signature_lookup.fixture'],
    maxSubQueries: 2,
    quorum: 2,
  })
  return {
    environment: {
      contextVars: {
        owner: 'owner:github:14167547',
        surface: 'artanis_operator',
      },
      fragments: [
        {
          fragmentRef: 'memory.fixture',
          text: 'Owner asked for the Khala burndown state.',
        },
      ],
    },
    objective:
      'Answer the owner with an unbounded, recursively composed Artanis status.',
    policy: { ...policy, ...(overrides?.policy ?? {}) },
    requestedAt: '2026-06-28T14:00:00.000Z',
    runId: 'run.frlm.fixture',
  }
}

const executor: FrlmLeafExecutor = subQuery =>
  Effect.succeed({
    evidenceRefs: [`evidence.${subQuery.subQueryId}.returned`],
    output: `grounded output for ${subQuery.purpose}`,
    subQueryId: subQuery.subQueryId,
    usage: { inputTokens: 10, outputTokens: 20, totalTokens: 30 },
  })

describe('FrlmConductor', () => {
  test('fans out governed subqueries and composes after quorum returns', async () => {
    const conductor = new FrlmConductor({
      compose: composition =>
        Effect.succeed(
          `composed:${composition.returns.map(item => item.subQueryId).join(',')}`,
        ),
      execute: executor,
      plan: makeStaticFrlmPlanner(subQueries),
    })

    const result = await Effect.runPromise(conductor.run(input()))

    expect(result.status).toBe('completed')
    expect(result.answer).toBe('composed:sq.memory,sq.backlog')
    expect(result.usage).toEqual({
      inputTokens: 20,
      outputTokens: 40,
      totalTokens: 60,
    })
    expect(result.events.map(item => item.kind)).toEqual([
      'Run.Init',
      'SubQuery.Submit',
      'SubQuery.Submit',
      'SubQuery.Return',
      'SubQuery.Return',
      'Run.Done',
    ])
    expect(result.events[0]?.refs).toContain(FRLM_CONDUCTOR_SIGNATURE_REF)
    expect(result.events.at(-1)?.refs).toContain(
      'evidence.frlm_conductor.quorum_met',
    )
    expect(result.compositionPrompt).toContain('SubQuery: sq.memory')
    expect(result.compositionPrompt).toContain(
      'Do not claim execution authority',
    )
  })

  test('blocks without composing when leaf returns do not meet quorum', async () => {
    const conductor = new FrlmConductor({
      compose: () => Effect.succeed('should not compose'),
      execute: subQuery =>
        subQuery.subQueryId === 'sq.backlog'
          ? Effect.fail(
              new FrlmConductorError({
                reason: 'codex executor unavailable',
              }),
            )
          : executor(subQuery, input()),
      plan: makeStaticFrlmPlanner(subQueries),
    })

    const result = await Effect.runPromise(conductor.run(input()))

    expect(result.status).toBe('blocked_quorum_not_met')
    expect(result.answer).toBeNull()
    expect(result.returns.map(item => item.subQueryId)).toEqual(['sq.memory'])
    expect(result.events.at(-1)?.refs).toContain(
      'blocker.frlm_conductor.quorum_not_met',
    )
    expect(result.events.map(item => item.summary).join('\n')).toContain(
      'codex executor unavailable',
    )
  })

  test('rejects subqueries outside the allowed Blueprint signatures', async () => {
    const conductor = new FrlmConductor({
      compose: () => Effect.succeed('unused'),
      execute: executor,
      plan: makeStaticFrlmPlanner([
        {
          ...subQueries[0]!,
          signatureRef: 'program_signature.unknown.v1',
        },
      ]),
    })

    await expect(Effect.runPromise(conductor.run(input()))).rejects.toMatchObject({
      reason: expect.stringContaining('ungoverned Blueprint signature'),
    })
  })
})
