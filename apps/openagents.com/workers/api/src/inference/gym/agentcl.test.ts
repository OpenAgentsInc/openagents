import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENTCL_EVAL_SCHEMA,
  AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
  AGENTCL_REPO_REUSE_PLAN_SCHEMA,
  AGENTCL_TASK_RUNNER_RESULT_SCHEMA,
  AGENTCL_VERTEX_RUNNER_PLAN_SCHEMA,
  AGENTCL_VERTEX_STRESS_REPORT_SCHEMA,
  assessAgentClLearningClaimGate,
  assessAgentClVertexRunnerCircuitBreaker,
  buildAgentClRepoReusePlan,
  buildAgentClVertexGeminiRunnerPlan,
  buildAgentClVertexStressBaselineReport,
  buildAgentClVertexStressExperiment,
  runAgentClVertexRunnerLoop,
  runAgentClRepoReuseFixtureEval,
  runAgentClSequentialLoop,
  runAgentClTaskRunner,
} from './agentcl'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
} from '../provider-adapter'

const vertexResult = (
  usage: InferenceResult['usage'],
): InferenceResult => ({
  content: 'public-safe AgentCL fixture outcome',
  finishReason: 'stop',
  servedModel: 'gemini-3.5-flash',
  usage,
})

const fakeVertexAdapter = (
  complete: (
    request: InferenceRequest,
  ) => Effect.Effect<InferenceResult, InferenceAdapterError>,
): InferenceProviderAdapter => ({
  complete,
  id: 'vertex-gemini',
  stream: () =>
    Effect.succeed([
      {
        contentDelta: '',
        finishReason: 'stop',
        servedModel: 'gemini-3.5-flash',
        usage: {
          completionTokens: 1,
          promptTokens: 1,
          totalTokens: 2,
        },
      },
    ]),
})

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
    expect(result.eval.taskRunner.schemaVersion).toBe(
      AGENTCL_TASK_RUNNER_RESULT_SCHEMA,
    )
    expect(result.eval.taskRunner.runnerConfigId).toBe(
      'gym:gym-agentcl-repo-reuse-two-pass-fixture-v0',
    )
    expect(result.eval.taskRunner.trajectoryEvaluations).toHaveLength(15)
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

  test('loads the AgentCL task sequence and evaluates trajectories through the active gym runner', () => {
    const result = runAgentClTaskRunner(AGENTCL_REPO_REUSE_GYM_EXPERIMENT)

    expect(result.fixtureRun.runSet.seamId).toBe('fixture')
    expect(result.fixtureRun.publicSafety).toEqual({
      safe: true,
      violations: [],
    })
    expect(result.taskRunner.loadedTaskRefs).toEqual(
      result.compiled.matrixConfig.workloads.flatMap(workload =>
        workload === 'agentcl-source-task'
          ? result.plan.sourceTasks.map(task => task.taskRef)
          : workload === 'agentcl-complex-task'
            ? result.plan.complexTasks.map(task => task.taskRef)
            : result.plan.heldOutTasks.map(task => task.taskRef),
      ),
    )
    expect(result.taskRunner.taskSequence).toHaveLength(
      result.sequentialRun.taskAttemptCount,
    )
    expect(
      result.taskRunner.taskSequence.map(entry => entry.sequenceIndex),
    ).toEqual(Array.from({ length: 15 }, (_, index) => index + 1))
    expect(
      result.taskRunner.taskSequence
        .filter(entry => entry.memoryAccess === 'read_write')
        .map(entry => entry.taskRef),
    ).not.toContain('agentcl.repo_reuse.held_out.mirrorcode_no_rag.v0')
    expect(
      result.taskRunner.trajectoryEvaluations.map(item => item.attemptRef),
    ).toEqual(
      result.sequentialRun.taskAttempts.map(attempt => attempt.attemptRef),
    )
    expect(
      result.taskRunner.trajectoryEvaluations.every(item =>
        item.telemetryRequestId.startsWith(
          `bench:${result.fixtureRun.runSet.configId}:`,
        ),
      ),
    ).toBe(true)
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

  test('builds the CL-5 owner-armed Vertex stress experiment with the mandated concurrency and model lane', () => {
    const experiment = buildAgentClVertexStressExperiment(
      'owner.approval.agentcl.vertex_stress.20260628',
    )
    const { compiled } = runAgentClRepoReuseFixtureEval(experiment)

    expect(experiment.id).toBe(
      'gym-agentcl-vertex-gemini35-flash-stress-cl5-v0',
    )
    expect(experiment.policy.fanout).toEqual({
      lanes: ['vertex-gemini'],
      mode: 'single',
      concurrency: 10,
    })
    expect(experiment.budget).toEqual({
      spendCapMsat: 0,
      maxBillableSamples: 10,
      seam: 'real',
      ownerApprovalRef: 'owner.approval.agentcl.vertex_stress.20260628',
    })
    expect(compiled.policySelection.fanout.lanes).toEqual(['vertex-gemini'])
    expect(compiled.policySelection.fanout.concurrency).toBe(10)
  })

  test('builds the CL-4 Vertex Gemini runner plan with route verification and fallback bans', () => {
    const plan = buildAgentClVertexGeminiRunnerPlan(
      'owner.approval.agentcl.vertex_stress.20260628',
    )

    expect(plan.schemaVersion).toBe(AGENTCL_VERTEX_RUNNER_PLAN_SCHEMA)
    expect(plan.issueRef).toBe('public.issue.6766')
    expect(plan.lane).toEqual({
      laneRef: 'vertex-gemini',
      model: 'gemini-3.5-flash',
      projectRef: 'project.openagentsgemini',
      forbiddenFallbackLaneRefs: ['glm-free', 'khala-free'],
      requiresPreScaleVertexProof: true,
      preScaleProofRef: 'proof.agentcl.vertex_gemini35_flash.pre_scale_routing',
    })
    expect(plan.parallelism).toEqual({
      plannedParallelSequences: 10,
      verifyRouteBeforeScaling: true,
    })
    expect(plan.budgetGuard).toEqual({
      spendCapUsdCents: 5000,
      abortOnEstimatedSpendAboveCap: true,
      abortOnConsecutiveBillingOrQuotaErrors: 3,
      trackedCapacityErrorRefs: [
        'billing_error',
        'quota_error',
        'http_429',
        'resource_exhausted',
      ],
    })
  })

  test('trips the CL-4 runner circuit breaker on quota streaks or spend above the cap', () => {
    expect(
      assessAgentClVertexRunnerCircuitBreaker({
        estimatedSpendUsdCents: 4999,
        consecutiveBillingOrQuotaErrors: 2,
      }),
    ).toEqual({ tripped: false, reason: 'none' })
    expect(
      assessAgentClVertexRunnerCircuitBreaker({
        estimatedSpendUsdCents: 1250,
        consecutiveBillingOrQuotaErrors: 3,
      }),
    ).toEqual({
      tripped: true,
      reason: 'consecutive_billing_or_quota_errors',
    })
    expect(
      assessAgentClVertexRunnerCircuitBreaker({
        estimatedSpendUsdCents: 5001,
        consecutiveBillingOrQuotaErrors: 0,
      }),
    ).toEqual({ tripped: true, reason: 'spend_cap_exceeded' })
  })

  test('keeps the AgentCL Vertex runner no-spend unless owner_armed_real is selected', async () => {
    let calls = 0
    const result = await Effect.runPromise(
      runAgentClVertexRunnerLoop({
        adapter: fakeVertexAdapter(() => {
          calls += 1
          return Effect.succeed(
            vertexResult({
              completionTokens: 1,
              promptTokens: 1,
              totalTokens: 2,
            }),
          )
        }),
        ownerApprovalRef: 'owner.approval.agentcl.vertex_stress.20260628',
        runMode: 'fixture_baseline',
      }),
    )

    expect(result.status).toBe('blocked_unarmed')
    expect(result.failureRefs).toContain(
      'blocker.gym.agentcl.vertex_runner_not_owner_armed_real',
    )
    expect(calls).toBe(0)
  })

  test('blocks AgentCL live execution when the supplied lane is a forbidden fallback', async () => {
    const fallbackAdapter: InferenceProviderAdapter = {
      ...fakeVertexAdapter(() =>
        Effect.succeed(
          vertexResult({
            completionTokens: 1,
            promptTokens: 1,
            totalTokens: 2,
          }),
        ),
      ),
      id: 'openrouter-khala-glm-fallback',
    }

    const result = await Effect.runPromise(
      runAgentClVertexRunnerLoop({
        adapter: fallbackAdapter,
        ownerApprovalRef: 'owner.approval.agentcl.vertex_stress.20260628',
        runMode: 'owner_armed_real',
      }),
    )

    expect(result.status).toBe('blocked_forbidden_fallback')
    expect(result.failureRefs).toContain(
      'blocker.gym.agentcl.vertex_runner_forbidden_fallback_lane',
    )
  })

  test('runs AgentCL live calls through Vertex only and aborts after measured spend crosses the $50 cap', async () => {
    let calls = 0
    const result = await Effect.runPromise(
      runAgentClVertexRunnerLoop({
        adapter: fakeVertexAdapter(request => {
          calls += 1
          expect(request.model).toBe('gemini-3.5-flash')
          expect(request.priority).toBe('internal_stress')
          return Effect.succeed(
            vertexResult({
              completionTokens: 166_666_668,
              promptTokens: 0,
              totalTokens: 166_666_668,
            }),
          )
        }),
        ownerApprovalRef: 'owner.approval.agentcl.vertex_stress.20260628',
        runMode: 'owner_armed_real',
      }),
    )

    expect(calls).toBe(1)
    expect(result.status).toBe('aborted_circuit_breaker')
    expect(result.report.budgetGuard.circuitBreakerTripped).toBe(true)
    expect(result.report.budgetGuard.circuitBreakerReason).toBe(
      'spend_cap_exceeded',
    )
    expect(result.report.budgetGuard.estimatedSpendUsdCents).toBeGreaterThan(
      5000,
    )
  })

  test('aborts AgentCL live execution after three consecutive billing or quota errors', async () => {
    let calls = 0
    const result = await Effect.runPromise(
      runAgentClVertexRunnerLoop({
        adapter: fakeVertexAdapter(() => {
          calls += 1
          return Effect.fail(
            new InferenceAdapterError({
              adapterId: 'vertex-gemini',
              httpStatus: 429,
              kind: 'quota_error',
              reason: 'Vertex Gemini returned HTTP 429: resource exhausted',
              retryable: true,
            }),
          )
        }),
        ownerApprovalRef: 'owner.approval.agentcl.vertex_stress.20260628',
        runMode: 'owner_armed_real',
      }),
    )

    expect(calls).toBe(3)
    expect(result.status).toBe('aborted_circuit_breaker')
    expect(result.report.budgetGuard.consecutiveBillingOrQuotaErrors).toBe(3)
    expect(result.report.budgetGuard.circuitBreakerReason).toBe(
      'consecutive_billing_or_quota_errors',
    )
    expect(result.report.capacityReport.http429Count).toBe(3)
  })

  test('emits the CL-5 baseline report with separate PG, SG, and GG curves plus capacity blockers', () => {
    const report = buildAgentClVertexStressBaselineReport()

    expect(report.schemaVersion).toBe(AGENTCL_VERTEX_STRESS_REPORT_SCHEMA)
    expect(report.issueRef).toBe('public.issue.6767')
    expect(report.routing).toEqual({
      laneRef: 'vertex-gemini',
      model: 'gemini-3.5-flash',
      projectRef: 'project.openagentsgemini',
      verifiedVertexBeforeScale: false,
      proofRefs: [],
    })
    expect(report.budgetGuard).toMatchObject({
      spendCapUsdCents: 5000,
      estimatedSpendUsdCents: 0,
      consecutiveBillingOrQuotaErrors: 0,
      circuitBreakerTripped: false,
      circuitBreakerReason: 'none',
    })
    expect(report.capacityReport).toMatchObject({
      plannedParallelSequences: 10,
      attemptedSequences: 0,
      completedSequences: 0,
      peakAcceptedParallelSequences: 0,
      http429Count: 0,
      resourceExhaustedCount: 0,
      capacityLimitHit: false,
    })
    expect(report.learningCurves.plasticityGain.map(point => point.pass)).toEqual(
      ['baseline', 'first_pass'],
    )
    expect(report.learningCurves.stabilityGain.map(point => point.pass)).toEqual([
      'first_pass',
      'frozen_second_pass',
    ])
    expect(
      report.learningCurves.generalizationGain.map(point => point.pass),
    ).toEqual(['baseline', 'held_out_pass'])
    expect(report.blockerRefs).toEqual([
      'blocker.gym.agentcl.vertex_routing_not_verified',
      'blocker.gym.agentcl.vertex_capacity_limit_not_hit',
      'blocker.gym.agentcl.fixture_baseline_not_live_stress',
    ])
    expect(report.decisionGrade).toBe(false)
    expect(report.publicClaimEligible).toBe(false)
  })

  test('records Vertex 429/resource-exhausted capacity evidence without tripping the budget guard', () => {
    const report = buildAgentClVertexStressBaselineReport({
      runMode: 'owner_armed_real',
      verifiedVertexBeforeScale: true,
      attemptedSequences: 10,
      completedSequences: 7,
      peakAcceptedParallelSequences: 7,
      http429Count: 2,
      resourceExhaustedCount: 1,
      estimatedSpendUsdCents: 1732,
    })

    expect(report.routing.verifiedVertexBeforeScale).toBe(true)
    expect(report.routing.proofRefs).toContain(
      'proof.agentcl.vertex_gemini35_flash.pre_scale_routing',
    )
    expect(report.capacityReport.capacityLimitHit).toBe(true)
    expect(report.capacityReport.http429Count).toBe(2)
    expect(report.capacityReport.resourceExhaustedCount).toBe(1)
    expect(report.budgetGuard.circuitBreakerTripped).toBe(false)
    expect(report.blockerRefs).toEqual([])
  })

  test('trips the CL-5 circuit breaker on three consecutive billing or quota errors before spend escapes the cap', () => {
    const report = buildAgentClVertexStressBaselineReport({
      runMode: 'owner_armed_real',
      verifiedVertexBeforeScale: true,
      consecutiveBillingOrQuotaErrors: 3,
      http429Count: 3,
      resourceExhaustedCount: 3,
      estimatedSpendUsdCents: 2140,
    })

    expect(report.budgetGuard.circuitBreakerTripped).toBe(true)
    expect(report.budgetGuard.circuitBreakerReason).toBe(
      'consecutive_billing_or_quota_errors',
    )
    expect(report.budgetGuard.estimatedSpendUsdCents).toBeLessThanOrEqual(5000)
  })
})
