import { describe, expect, test } from 'vitest'

import {
  AgentClEvalSchemaVersion,
  buildAgentClRepoReuseStream,
  runAgentClTwoPassFixtureEval,
  summarizeAgentClEval,
} from './agentcl-eval'
import { AGENTCL_REPO_REUSE_GYM_EXPERIMENT } from './experiment'

describe('AgentCL repo-reuse compositional gym', () => {
  test('builds a public-safe stream with source-to-target reuse relations and a held-out task', () => {
    const stream = buildAgentClRepoReuseStream()

    expect(stream.environmentRef).toBe('agentcl-repo-reuse')
    expect(stream.streamKind).toBe('compositional')
    expect(stream.tasks.map(task => task.role)).toEqual([
      'source',
      'source',
      'source',
      'target',
      'target',
      'held_out',
    ])
    expect(stream.compositionalRelations).toHaveLength(2)
    expect(stream.heldOutTaskRefs).toEqual([
      'agentcl.repo_reuse.held_out.mirrorcode_no_rag_rule.v1',
    ])
    expect(stream.publicSafe).toBe(true)
    expect(stream.rawArtifactsIncluded).toBe(false)
  })

  test('runs a deterministic two-pass fixture and emits separate PG SG GG gains', () => {
    const evaluation = runAgentClTwoPassFixtureEval({
      generatedAt: '2026-06-27T12:00:00.000Z',
    })

    expect(evaluation.schemaVersion).toBe(AgentClEvalSchemaVersion)
    expect(evaluation.experimentId).toBe(AGENTCL_REPO_REUSE_GYM_EXPERIMENT.id)
    expect(evaluation.memoryConfigRef).toBe('pylon-tas-memory')
    expect(evaluation.aggregates).toEqual({
      baseline: 0.48,
      firstPass: 0.65,
      frozenSecondPass: 0.59,
      heldOutBaseline: 0.62,
      heldOutPass: 0.56,
    })
    expect(evaluation.gains).toEqual({
      plasticityGain: 0.17,
      stabilityGain: -0.06,
      generalizationGain: -0.06,
    })
    expect(summarizeAgentClEval(evaluation)).toEqual([
      'PG=0.1700',
      'SG=-0.0600',
      'GG=-0.0600',
    ])
    expect(evaluation.decisionGrade).toBe(false)
    expect(evaluation.publicSafe).toBe(true)
    expect(evaluation.rawArtifactsIncluded).toBe(false)
    expect(evaluation.authority).toEqual({
      publicClaimAllowed: false,
      runtimePromotionAllowed: false,
      payoutAllowed: false,
      settlementAllowed: false,
      providerMutationAllowed: false,
    })
  })

  test('makes held-out degradation visible instead of folding it into plasticity', () => {
    const pylonTas = runAgentClTwoPassFixtureEval({
      memoryConfigRef: 'pylon-tas-memory',
    })
    const omni = runAgentClTwoPassFixtureEval({
      memoryConfigRef: 'omni-retrieval',
    })

    expect(pylonTas.gains.plasticityGain).toBeGreaterThan(0)
    expect(pylonTas.gains.stabilityGain).toBeLessThan(0)
    expect(pylonTas.gains.generalizationGain).toBeLessThan(0)
    expect(omni.gains.plasticityGain).toBeGreaterThan(0)
    expect(omni.gains.generalizationGain).toBeLessThan(0)
  })
})
