import { describe, expect, test } from 'vitest'

import {
  AgentClEvalSchemaVersion,
  AgentClEvalUnsafe,
  buildAgentClEval,
  evaluateContinualLearningClaim,
  type AgentClEval,
} from './agentcl-eval'

const validAgentClEval = (): AgentClEval => ({
  schemaVersion: AgentClEvalSchemaVersion,
  evalRef: 'eval.gym.agentcl.router_memory.v0',
  streamRef: 'stream.gym.agentcl.compositional_repo_tasks.v0',
  candidateRef: 'candidate.gym.khala.router_memory.v0',
  memorySystemRef: 'memory_system.gym.khala.repo_memory.v0',
  baseline: {
    phase: 'baseline',
    scoreBps: 4_000,
    taskCount: 20,
    reportRef: 'report.gym.agentcl.baseline',
    receiptRef: 'receipt.gym.agentcl.baseline',
  },
  firstPass: {
    phase: 'first_pass',
    scoreBps: 5_700,
    taskCount: 20,
    reportRef: 'report.gym.agentcl.first_pass',
    receiptRef: 'receipt.gym.agentcl.first_pass',
  },
  frozenSecondPass: {
    phase: 'frozen_second_pass',
    scoreBps: 5_300,
    taskCount: 20,
    reportRef: 'report.gym.agentcl.frozen_second_pass',
    receiptRef: 'receipt.gym.agentcl.frozen_second_pass',
  },
  heldOut: {
    phase: 'held_out',
    scoreBps: 3_900,
    taskCount: 20,
    reportRef: 'report.gym.agentcl.held_out',
    receiptRef: 'receipt.gym.agentcl.held_out',
  },
  gains: {
    plasticity: {
      kind: 'plasticity',
      gainBps: 1_700,
      evidenceRefs: ['evidence.gym.agentcl.pg'],
    },
    stability: {
      kind: 'stability',
      gainBps: -400,
      evidenceRefs: ['evidence.gym.agentcl.sg'],
    },
    generalization: {
      kind: 'generalization',
      gainBps: -100,
      evidenceRefs: ['evidence.gym.agentcl.gg'],
    },
  },
  caveatRefs: ['caveat.public.gym.agentcl.test'],
})

describe('AgentCL eval contract', () => {
  test('accepts baseline, first-pass, frozen second-pass, held-out, and separate gains', () => {
    const evalRecord = buildAgentClEval(validAgentClEval())

    expect(evalRecord.schemaVersion).toBe('openagents.gym.agentcl_eval.v0')
    expect(evalRecord.gains.plasticity.gainBps).toBe(1_700)
    expect(evalRecord.gains.stability.gainBps).toBe(-400)
    expect(evalRecord.gains.generalization.gainBps).toBe(-100)
    expect(evalRecord.caveatRefs).toContain(
      'caveat.public.gym.agentcl_eval.pg_sg_gg_reported_separately',
    )
  })

  test('rejects a gain that does not match its measured phase delta', () => {
    const evalRecord = validAgentClEval()

    expect(() =>
      buildAgentClEval({
        ...evalRecord,
        gains: {
          ...evalRecord.gains,
          plasticity: {
            ...evalRecord.gains.plasticity,
            gainBps: 900,
          },
        },
      }),
    ).toThrow(AgentClEvalUnsafe)
  })

  test('rejects gain records without separate evidence refs', () => {
    const evalRecord = validAgentClEval()

    expect(() =>
      buildAgentClEval({
        ...evalRecord,
        gains: {
          ...evalRecord.gains,
          generalization: {
            ...evalRecord.gains.generalization,
            evidenceRefs: [],
          },
        },
      }),
    ).toThrow(AgentClEvalUnsafe)
  })
})

describe('continual-learning claim discipline', () => {
  test('blocks a single memory-improved metric without AgentCL evidence', () => {
    const discipline = evaluateContinualLearningClaim({
      claimRef: 'claim.gym.khala.continually_learns',
      copy: 'Khala memory improves accuracy over time.',
      legacySingleMetricBps: 1_200,
      evidenceRefs: ['evidence.gym.memory_improved_accuracy'],
    })

    expect(discipline.ok).toBe(false)
    expect(discipline.evalRef).toBe(null)
    expect(discipline.blockerRefs).toEqual([
      'blocker.gym.agentcl_claim.agentcl_eval_v0_missing',
      'blocker.gym.agentcl_claim.single_memory_improved_metric_refused',
    ])
  })

  test('accepts a continual-learning claim only with AgentCL PG, SG, and GG evidence', () => {
    const discipline = evaluateContinualLearningClaim({
      claimRef: 'claim.gym.khala.continually_learns',
      copy: 'Khala memory reports Plasticity, Stability, and Generalization separately.',
      agentClEval: validAgentClEval(),
      evidenceRefs: ['evidence.gym.agentcl.claim'],
    })

    expect(discipline.ok).toBe(true)
    expect(discipline.evalRef).toBe('eval.gym.agentcl.router_memory.v0')
    expect(discipline.blockerRefs).toEqual([])
    expect(discipline.caveatRefs).toContain(
      'caveat.public.gym.continual_learning_claim.no_single_memory_improved_metric',
    )
  })
})
