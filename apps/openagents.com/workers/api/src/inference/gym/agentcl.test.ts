import { describe, expect, test } from 'vitest'

import {
  AGENTCL_EVAL_SCHEMA,
  AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
  AGENTCL_REPO_REUSE_PLAN_SCHEMA,
  assessAgentClLearningClaimGate,
  buildAgentClRepoReusePlan,
  runAgentClRepoReuseFixtureEval,
  runAgentClSequentialLoop,
} from './agentcl'

describe('AgentCL repo-reuse gym environment', () => {
  test('builds a public-safe two-pass compositional stream plan', () => {
    const { compiled, plan } = buildAgentClRepoReusePlan(
      AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
    )

    expect(plan.schemaVersion).toBe(AGENTCL_REPO_REUSE_PLAN_SCHEMA)
    expect(plan.environmentRef).toBe('agentcl-repo-reuse')
    expect(plan.streamKind).toBe('compositional')
    expect(plan.sourceTasks).toHaveLength(4)
    expect(plan.complexTasks).toHaveLength(2)
    expect(plan.heldOutTasks).toHaveLength(1)
    expect(plan.passes.map(pass => pass.pass)).toEqual([
      'baseline',
      'first_pass',
      'frozen_second_pass',
      'held_out_pass',
    ])
    expect(plan.passes.map(pass => pass.memoryAccess)).toEqual([
      'disabled',
      'read_write',
      'read_only_frozen',
      'read_only_frozen',
    ])
    expect(plan.publicSafetyBoundary).toEqual({
      publicTaskRefsOnly: true,
      rawPromptsStayOwnerPrivate: true,
      noTrainingOnHeldOut: true,
      reportPgSgGgSeparately: true,
      publicClaimEligible: false,
    })
    expect(compiled.policySelection.environment.acceptanceContractRef).toBe(
      'acceptance.gym.agentcl.report_only_no_claim.v0',
    )
  })

  test('emits agentcl_eval.v0 with separate plasticity, stability, and generalization gains', () => {
    const result = runAgentClRepoReuseFixtureEval(
      AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
    )

    expect(result.eval.schemaVersion).toBe(AGENTCL_EVAL_SCHEMA)
    expect(result.eval.baseline.acceptedOutcomeRate).toBe(0.45)
    expect(result.eval.firstPass.acceptedOutcomeRate).toBe(0.62)
    expect(result.eval.frozenSecondPass.acceptedOutcomeRate).toBe(0.58)
    expect(result.eval.heldOutBaseline.acceptedOutcomeRate).toBe(0.7)
    expect(result.eval.heldOutPass.acceptedOutcomeRate).toBe(0.66)
    expect(result.eval.plasticityGain).toBe(0.17)
    expect(result.eval.stabilityGain).toBe(-0.04)
    expect(result.eval.generalizationGain).toBe(-0.04)
    expect(result.eval.sequentialRun.taskAttemptCount).toBe(15)
    expect(result.eval.sequentialRun.memoryMutationCount).toBe(6)
    expect(result.eval.sequentialRun.templateLedgerRef).toBe(
      'ledger.public.artanis.continual_learning_templates',
    )
    expect(result.eval.claimDiscipline).toMatchObject({
      decisionGrade: false,
      publicClaimEligible: false,
      collapseGainsIntoOneNumber: false,
    })
    expect(result.eval.claimDiscipline.notes).toContain(
      'negative_stability_gain_requires_memory_review',
    )
    expect(result.eval.claimDiscipline.notes).toContain(
      'negative_generalization_gain_blocks_generalizes_claim',
    )
  })

  test('keeps held-out tasks out of read-write memory construction passes', () => {
    const { plan } = buildAgentClRepoReusePlan()
    const heldOutRefs = new Set(plan.heldOutTasks.map(task => task.taskRef))
    const memoryWritePasses = plan.passes.filter(
      pass => pass.memoryAccess === 'read_write',
    )

    expect(memoryWritePasses).toHaveLength(1)
    for (const pass of memoryWritePasses) {
      expect(pass.taskRefs.some(ref => heldOutRefs.has(ref))).toBe(false)
    }
  })

  test('runs tasks sequentially and mutates memory between write-enabled tasks', () => {
    const { plan } = buildAgentClRepoReusePlan()
    const run = runAgentClSequentialLoop(plan)
    const firstPassAttempts = run.taskAttempts.filter(
      attempt => attempt.pass === 'first_pass',
    )
    const firstPassMutations = run.memoryMutations.filter(
      mutation => mutation.pass === 'first_pass',
    )

    expect(firstPassAttempts).toHaveLength(6)
    expect(firstPassMutations).toHaveLength(6)
    expect(firstPassAttempts.map(attempt => attempt.stepIndex)).toEqual([
      7, 8, 9, 10, 11, 12,
    ])
    expect(firstPassAttempts.every(attempt => attempt.mutationRefs.length === 1))
      .toBe(true)
    expect(
      firstPassAttempts.every(
        attempt =>
          attempt.appliedTemplateRefs.length === 1 &&
          attempt.appliedTemplateRefs[0]?.startsWith(
            'template.public.artanis.continual_learning.',
          ) === true,
      ),
    ).toBe(true)
    expect(firstPassAttempts[1]?.memoryBeforeRefs).toContain(
      firstPassAttempts[0]?.mutationRefs[0],
    )
    expect(run.finalMemoryRefs).toContain(
      'mutation.public.agentcl.first_pass.step_12.agentcl.repo_reuse.complex.pg_sg_gg_report.v0',
    )
  })

  test('freezes memory for second-pass and held-out evaluation', () => {
    const { plan } = buildAgentClRepoReusePlan()
    const run = runAgentClSequentialLoop(plan)
    const readOnlyAttempts = run.taskAttempts.filter(
      attempt => attempt.memoryAccess === 'read_only_frozen',
    )

    expect(readOnlyAttempts).toHaveLength(3)
    expect(readOnlyAttempts.every(attempt => attempt.mutationRefs.length === 0))
      .toBe(true)
    expect(
      readOnlyAttempts.every(
        attempt =>
          JSON.stringify(attempt.memoryAfterRefs) ===
          JSON.stringify(attempt.memoryBeforeRefs),
      ),
    ).toBe(true)
    expect(
      run.memoryMutations.some(mutation =>
        mutation.taskRef.includes('held_out'),
      ),
    ).toBe(false)
  })

  test('requires separate PG, SG, and GG before memory-learning claims', () => {
    const result = runAgentClRepoReuseFixtureEval()
    const gate = assessAgentClLearningClaimGate({
      claimKind: 'continual_learning',
      eval: result.eval,
    })

    expect(gate.requiresSeparatePgSgGg).toBe(true)
    expect(gate.hasSeparatePlasticityGain).toBe(true)
    expect(gate.hasSeparateStabilityGain).toBe(true)
    expect(gate.hasSeparateGeneralizationGain).toBe(true)
    expect(gate.decisionGradeClaimAllowed).toBe(false)
    expect(gate.publicClaimAllowed).toBe(false)
    expect(gate.blockerRefs).toContain('blocker.gym.agentcl.not_decision_grade')
    expect(gate.blockerRefs).toContain(
      'blocker.gym.agentcl.public_claim_not_eligible',
    )
  })

  test('rejects a collapsed memory-improved metric as AgentCL evidence', () => {
    const result = runAgentClRepoReuseFixtureEval()
    const gate = assessAgentClLearningClaimGate({
      claimKind: 'memory_improvement',
      eval: result.eval,
      collapsedMemoryImprovementMetric: 0.12,
    })

    expect(gate.collapsedMemoryImprovementMetricAccepted).toBe(false)
    expect(gate.decisionGradeClaimAllowed).toBe(false)
    expect(gate.publicClaimAllowed).toBe(false)
    expect(gate.blockerRefs).toContain(
      'blocker.gym.agentcl.collapsed_memory_improvement_metric',
    )
  })
})
