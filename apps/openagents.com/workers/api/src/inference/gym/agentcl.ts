import { Schema as S } from 'effect'

import {
  AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
  GYM_ENVIRONMENT_REGISTRY,
  compileGymExperiment,
  type CompiledGymExperiment,
  type GymExperiment,
  type GymFixtureRunResult,
  runGymFixtureExperiment,
} from './experiment'
import { exampleArtanisContinualLearningTemplateLedger } from '../../artanis-continual-learning-templates'
import type { BenchmarkWorkload } from '../benchmark'

export { AGENTCL_REPO_REUSE_GYM_EXPERIMENT } from './experiment'

export const AGENTCL_EVAL_SCHEMA = 'openagents.gym.agentcl_eval.v0' as const
export const AGENTCL_REPO_REUSE_PLAN_SCHEMA =
  'openagents.gym.agentcl_repo_reuse_plan.v0' as const
export const AGENTCL_VERTEX_STRESS_REPORT_SCHEMA =
  'openagents.gym.agentcl_vertex_stress_report.v0' as const
export const AGENTCL_TASK_RUNNER_RESULT_SCHEMA =
  'openagents.gym.agentcl_task_runner_result.v0' as const
export const AGENTCL_VERTEX_RUNNER_PLAN_SCHEMA =
  'openagents.gym.agentcl_vertex_runner_plan.v0' as const

export const AgentClStreamKind = S.Literals(['naive', 'compositional'])
export type AgentClStreamKind = typeof AgentClStreamKind.Type

export const AgentClTaskRole = S.Literals(['source', 'complex', 'held_out'])
export type AgentClTaskRole = typeof AgentClTaskRole.Type

export const AgentClPassKind = S.Literals([
  'baseline',
  'first_pass',
  'frozen_second_pass',
  'held_out_pass',
])
export type AgentClPassKind = typeof AgentClPassKind.Type

export const AgentClMemoryAccess = S.Literals([
  'disabled',
  'read_write',
  'read_only_frozen',
])
export type AgentClMemoryAccess = typeof AgentClMemoryAccess.Type

export const AgentClMemoryMutation = S.Struct({
  mutationRef: S.String,
  pass: AgentClPassKind,
  taskRef: S.String,
  stepIndex: S.Number,
  templateRef: S.String,
  memoryBeforeRefs: S.Array(S.String),
  memoryAfterRefs: S.Array(S.String),
  feedbackRef: S.String,
})
export type AgentClMemoryMutation = typeof AgentClMemoryMutation.Type

export const AgentClTaskAttempt = S.Struct({
  attemptRef: S.String,
  pass: AgentClPassKind,
  taskRef: S.String,
  taskRole: AgentClTaskRole,
  stepIndex: S.Number,
  memoryAccess: AgentClMemoryAccess,
  memoryBeforeRefs: S.Array(S.String),
  memoryAfterRefs: S.Array(S.String),
  appliedTemplateRefs: S.Array(S.String),
  mutationRefs: S.Array(S.String),
  acceptedOutcome: S.Boolean,
  acceptedOutcomeScore: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 1 }),
  ),
  feedbackRef: S.String,
  stateCarryForwardRef: S.String,
})
export type AgentClTaskAttempt = typeof AgentClTaskAttempt.Type

export const AgentClSequentialRun = S.Struct({
  runRef: S.String,
  taskAttemptCount: S.Number,
  memoryMutationCount: S.Number,
  initialMemoryRefs: S.Array(S.String),
  finalMemoryRefs: S.Array(S.String),
  templateLedgerRef: S.String,
  taskAttempts: S.Array(AgentClTaskAttempt),
  memoryMutations: S.Array(AgentClMemoryMutation),
})
export type AgentClSequentialRun = typeof AgentClSequentialRun.Type

export const AgentClRepoReuseTask = S.Struct({
  taskRef: S.String,
  role: AgentClTaskRole,
  packageRef: S.String,
  publicObjectiveRef: S.String,
  reusableSolutionRefs: S.Array(S.String),
  expectedReuseFromTaskRefs: S.Array(S.String),
})
export type AgentClRepoReuseTask = typeof AgentClRepoReuseTask.Type

export const AgentClRepoReusePassPlan = S.Struct({
  pass: AgentClPassKind,
  memoryAccess: AgentClMemoryAccess,
  taskRefs: S.Array(S.String),
  dispatchSurfaceRef: S.String,
  harborDispatchProfileRef: S.String,
})
export type AgentClRepoReusePassPlan =
  typeof AgentClRepoReusePassPlan.Type

export const AgentClTaskSequenceEntry = S.Struct({
  sequenceIndex: S.Number,
  taskRef: S.String,
  taskRole: AgentClTaskRole,
  workload: S.String,
  pass: AgentClPassKind,
  memoryAccess: AgentClMemoryAccess,
  sourceTaskSetRef: S.String,
  verifierRef: S.String,
})
export type AgentClTaskSequenceEntry =
  typeof AgentClTaskSequenceEntry.Type

export const AgentClTrajectoryEvaluation = S.Struct({
  trajectoryRef: S.String,
  attemptRef: S.String,
  taskRef: S.String,
  taskRole: AgentClTaskRole,
  workload: S.String,
  pass: AgentClPassKind,
  acceptedOutcome: S.Boolean,
  acceptedOutcomeScore: S.Number.check(
    S.isBetween({ minimum: 0, maximum: 1 }),
  ),
  scalarReward: S.Number.check(S.isBetween({ minimum: 0, maximum: 1 })),
  executedVerdict: S.String,
  benchmarkCellId: S.String,
  benchmarkSampleIndex: S.Number,
  telemetryRequestId: S.String,
  memoryMutationRefs: S.Array(S.String),
})
export type AgentClTrajectoryEvaluation =
  typeof AgentClTrajectoryEvaluation.Type

export const AgentClTaskRunnerResult = S.Struct({
  schemaVersion: S.Literal(AGENTCL_TASK_RUNNER_RESULT_SCHEMA),
  experimentId: S.String,
  taskSetRef: S.String,
  verifierRef: S.String,
  acceptanceContractRef: S.String,
  runnerConfigId: S.String,
  seamId: S.String,
  seamCanSpend: S.Boolean,
  loadedTaskRefs: S.Array(S.String),
  taskSequence: S.Array(AgentClTaskSequenceEntry),
  trajectoryEvaluations: S.Array(AgentClTrajectoryEvaluation),
  publicSafety: S.Struct({
    safe: S.Boolean,
    violations: S.Array(S.String),
  }),
})
export type AgentClTaskRunnerResult = typeof AgentClTaskRunnerResult.Type

export const AgentClRepoReusePlan = S.Struct({
  schemaVersion: S.Literal(AGENTCL_REPO_REUSE_PLAN_SCHEMA),
  environmentRef: S.Literal('agentcl-repo-reuse'),
  experimentId: S.String,
  streamKind: AgentClStreamKind,
  sourceTasks: S.Array(AgentClRepoReuseTask),
  complexTasks: S.Array(AgentClRepoReuseTask),
  heldOutTasks: S.Array(AgentClRepoReuseTask),
  passes: S.Array(AgentClRepoReusePassPlan),
  memorySystemsUnderTest: S.Array(S.String),
  publicSafetyBoundary: S.Struct({
    publicTaskRefsOnly: S.Literal(true),
    rawPromptsStayOwnerPrivate: S.Literal(true),
    noTrainingOnHeldOut: S.Literal(true),
    reportPgSgGgSeparately: S.Literal(true),
    publicClaimEligible: S.Literal(false),
  }),
})
export type AgentClRepoReusePlan = typeof AgentClRepoReusePlan.Type

export const AgentClPassScore = S.Struct({
  pass: AgentClPassKind,
  taskRole: AgentClTaskRole,
  taskCount: S.Number,
  acceptedOutcomeRate: S.Number.check(S.isBetween({ minimum: 0, maximum: 1 })),
})
export type AgentClPassScore = typeof AgentClPassScore.Type

export const AgentClEvalV0 = S.Struct({
  schemaVersion: S.Literal(AGENTCL_EVAL_SCHEMA),
  environmentRef: S.Literal('agentcl-repo-reuse'),
  experimentId: S.String,
  streamKind: AgentClStreamKind,
  memorySystemsUnderTest: S.Array(S.String),
  baseline: AgentClPassScore,
  firstPass: AgentClPassScore,
  frozenSecondPass: AgentClPassScore,
  heldOutBaseline: AgentClPassScore,
  heldOutPass: AgentClPassScore,
  plasticityGain: S.Number,
  stabilityGain: S.Number,
  generalizationGain: S.Number,
  sequentialRun: AgentClSequentialRun,
  claimDiscipline: S.Struct({
    decisionGrade: S.Literal(false),
    publicClaimEligible: S.Literal(false),
    collapseGainsIntoOneNumber: S.Literal(false),
    notes: S.Array(S.String),
  }),
  proofRefs: S.Array(S.String),
  taskRunner: AgentClTaskRunnerResult,
})
export type AgentClEvalV0 = typeof AgentClEvalV0.Type

export const AgentClLearningClaimKind = S.Literals([
  'continual_learning',
  'memory_improvement',
])
export type AgentClLearningClaimKind = typeof AgentClLearningClaimKind.Type

export const AgentClLearningClaimGate = S.Struct({
  schemaVersion: S.Literal('openagents.gym.agentcl_learning_claim_gate.v0'),
  claimKind: AgentClLearningClaimKind,
  evidenceSchemaVersion: S.Literal(AGENTCL_EVAL_SCHEMA),
  requiresSeparatePgSgGg: S.Literal(true),
  hasSeparatePlasticityGain: S.Boolean,
  hasSeparateStabilityGain: S.Boolean,
  hasSeparateGeneralizationGain: S.Boolean,
  collapsedMemoryImprovementMetricAccepted: S.Literal(false),
  decisionGradeClaimAllowed: S.Boolean,
  publicClaimAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
})
export type AgentClLearningClaimGate =
  typeof AgentClLearningClaimGate.Type

export const AgentClVertexStressRunMode = S.Literals([
  'fixture_baseline',
  'owner_armed_real',
])
export type AgentClVertexStressRunMode =
  typeof AgentClVertexStressRunMode.Type

export const AgentClVertexStressCircuitBreakerReason = S.Literals([
  'none',
  'spend_cap_exceeded',
  'consecutive_billing_or_quota_errors',
])
export type AgentClVertexStressCircuitBreakerReason =
  typeof AgentClVertexStressCircuitBreakerReason.Type

export const AgentClVertexRunnerPlanV0 = S.Struct({
  schemaVersion: S.Literal(AGENTCL_VERTEX_RUNNER_PLAN_SCHEMA),
  issueRef: S.Literal('public.issue.6766'),
  lane: S.Struct({
    laneRef: S.Literal('vertex-gemini'),
    model: S.Literal('gemini-3.5-flash'),
    projectRef: S.Literal('project.openagentsgemini'),
    forbiddenFallbackLaneRefs: S.Array(S.Literals(['glm-free', 'khala-free'])),
    requiresPreScaleVertexProof: S.Literal(true),
    preScaleProofRef: S.Literal(
      'proof.agentcl.vertex_gemini35_flash.pre_scale_routing',
    ),
  }),
  parallelism: S.Struct({
    plannedParallelSequences: S.Literal(10),
    verifyRouteBeforeScaling: S.Literal(true),
  }),
  budgetGuard: S.Struct({
    spendCapUsdCents: S.Literal(5000),
    abortOnEstimatedSpendAboveCap: S.Literal(true),
    abortOnConsecutiveBillingOrQuotaErrors: S.Literal(3),
    trackedCapacityErrorRefs: S.Array(
      S.Literals([
        'billing_error',
        'quota_error',
        'http_429',
        'resource_exhausted',
      ]),
    ),
  }),
  ownerApprovalRef: S.String,
  publicSafety: S.Struct({
    rawPromptsStayOwnerPrivate: S.Literal(true),
    noProviderPayloadsInPublicReport: S.Literal(true),
    noSpendWithoutOwnerApproval: S.Literal(true),
  }),
  reportRefs: S.Array(S.String),
})
export type AgentClVertexRunnerPlanV0 =
  typeof AgentClVertexRunnerPlanV0.Type

export const AgentClCurvePoint = S.Struct({
  pass: AgentClPassKind,
  taskRole: AgentClTaskRole,
  acceptedOutcomeRate: S.Number.check(S.isBetween({ minimum: 0, maximum: 1 })),
})
export type AgentClCurvePoint = typeof AgentClCurvePoint.Type

export const AgentClVertexStressReportV0 = S.Struct({
  schemaVersion: S.Literal(AGENTCL_VERTEX_STRESS_REPORT_SCHEMA),
  issueRef: S.Literal('public.issue.6767'),
  experimentId: S.String,
  runMode: AgentClVertexStressRunMode,
  routing: S.Struct({
    laneRef: S.Literal('vertex-gemini'),
    model: S.Literal('gemini-3.5-flash'),
    projectRef: S.Literal('project.openagentsgemini'),
    verifiedVertexBeforeScale: S.Boolean,
    proofRefs: S.Array(S.String),
  }),
  budgetGuard: S.Struct({
    spendCapUsdCents: S.Literal(5000),
    estimatedSpendUsdCents: S.Number.check(
      S.isBetween({ minimum: 0, maximum: 5000 }),
    ),
    consecutiveBillingOrQuotaErrors: S.Number.check(
      S.isBetween({ minimum: 0, maximum: 3 }),
    ),
    circuitBreakerTripped: S.Boolean,
    circuitBreakerReason: AgentClVertexStressCircuitBreakerReason,
  }),
  capacityReport: S.Struct({
    plannedParallelSequences: S.Literal(10),
    attemptedSequences: S.Number.check(S.isBetween({ minimum: 0, maximum: 10 })),
    completedSequences: S.Number.check(S.isBetween({ minimum: 0, maximum: 10 })),
    peakAcceptedParallelSequences: S.Number.check(
      S.isBetween({ minimum: 0, maximum: 10 }),
    ),
    http429Count: S.Number.check(S.isBetween({ minimum: 0, maximum: 10_000 })),
    resourceExhaustedCount: S.Number.check(
      S.isBetween({ minimum: 0, maximum: 10_000 }),
    ),
    capacityLimitHit: S.Boolean,
  }),
  learningCurves: S.Struct({
    plasticityGain: S.Array(AgentClCurvePoint),
    stabilityGain: S.Array(AgentClCurvePoint),
    generalizationGain: S.Array(AgentClCurvePoint),
  }),
  eval: AgentClEvalV0,
  decisionGrade: S.Literal(false),
  publicClaimEligible: S.Literal(false),
  blockerRefs: S.Array(S.String),
  reportRefs: S.Array(S.String),
})
export type AgentClVertexStressReportV0 =
  typeof AgentClVertexStressReportV0.Type

export type AgentClLearningClaimEvidence = Readonly<{
  plasticityGain?: number
  stabilityGain?: number
  generalizationGain?: number
  claimDiscipline?: Readonly<{
    decisionGrade?: boolean
    publicClaimEligible?: boolean
  }>
}>

const decodePlan = S.decodeUnknownSync(AgentClRepoReusePlan)
const decodeEval = S.decodeUnknownSync(AgentClEvalV0)
const decodeLearningClaimGate = S.decodeUnknownSync(AgentClLearningClaimGate)
const decodeSequentialRun = S.decodeUnknownSync(AgentClSequentialRun)
const decodeVertexRunnerPlan = S.decodeUnknownSync(AgentClVertexRunnerPlanV0)
const decodeVertexStressReport = S.decodeUnknownSync(AgentClVertexStressReportV0)
const decodeTaskRunnerResult = S.decodeUnknownSync(AgentClTaskRunnerResult)

const roundGain = (value: number): number => Math.round(value * 1000) / 1000

const agentClTaskRefs = (
  tasks: ReadonlyArray<AgentClRepoReuseTask>,
): ReadonlyArray<string> => tasks.map(task => task.taskRef)

const agentClTaskByRef = (
  plan: AgentClRepoReusePlan,
): Readonly<Record<string, AgentClRepoReuseTask>> =>
  [...plan.sourceTasks, ...plan.complexTasks, ...plan.heldOutTasks].reduce<
    Record<string, AgentClRepoReuseTask>
  >((acc, task) => {
    acc[task.taskRef] = task
    return acc
  }, {})

const scoreByAttemptRef: Readonly<Record<string, number>> = {
  'baseline:agentcl.repo_reuse.source.effect_schema_contract.v0': 0.46,
  'baseline:agentcl.repo_reuse.source.harbor_public_receipt.v0': 0.48,
  'baseline:agentcl.repo_reuse.source.tas_memory_ref.v0': 0.43,
  'baseline:agentcl.repo_reuse.source.omni_retrieval_ref.v0': 0.44,
  'baseline:agentcl.repo_reuse.complex.two_pass_runner.v0': 0.4,
  'baseline:agentcl.repo_reuse.complex.pg_sg_gg_report.v0': 0.5,
  'first_pass:agentcl.repo_reuse.source.effect_schema_contract.v0': 0.54,
  'first_pass:agentcl.repo_reuse.source.harbor_public_receipt.v0': 0.57,
  'first_pass:agentcl.repo_reuse.source.tas_memory_ref.v0': 0.59,
  'first_pass:agentcl.repo_reuse.source.omni_retrieval_ref.v0': 0.61,
  'first_pass:agentcl.repo_reuse.complex.two_pass_runner.v0': 0.6,
  'first_pass:agentcl.repo_reuse.complex.pg_sg_gg_report.v0': 0.64,
  'frozen_second_pass:agentcl.repo_reuse.complex.two_pass_runner.v0': 0.56,
  'frozen_second_pass:agentcl.repo_reuse.complex.pg_sg_gg_report.v0': 0.6,
  'held_out_pass:agentcl.repo_reuse.held_out.mirrorcode_no_rag.v0': 0.66,
  'held_out_baseline:agentcl.repo_reuse.held_out.mirrorcode_no_rag.v0': 0.7,
}

const memoryConsolidationTemplateRef = (
  task: AgentClRepoReuseTask,
): string => {
  const ledger = exampleArtanisContinualLearningTemplateLedger()
  const kind =
    task.role === 'source'
      ? 'dataset_curation'
      : task.role === 'complex'
        ? 'dspy_gepa_optimization'
        : 'benchmark_eval_rerun'
  const template = ledger.templates.find(candidate => candidate.kind === kind)
  return (
    template?.templateRef ??
    'template.public.artanis.continual_learning.regression_analysis'
  )
}

const scoreForAttempt = (
  pass: AgentClPassKind | 'held_out_baseline',
  taskRef: string,
): number => scoreByAttemptRef[`${pass}:${taskRef}`] ?? 0

const passScore = (
  attempts: ReadonlyArray<AgentClTaskAttempt>,
  pass: AgentClPassKind,
  taskRole: AgentClTaskRole,
): AgentClPassScore => {
  const scoredAttempts = attempts.filter(
    attempt => attempt.pass === pass && attempt.taskRole === taskRole,
  )
  const acceptedOutcomeRate = roundGain(
    scoredAttempts.reduce(
      (total, attempt) => total + attempt.acceptedOutcomeScore,
      0,
    ) / scoredAttempts.length,
  )
  return {
    pass,
    taskRole,
    taskCount: scoredAttempts.length,
    acceptedOutcomeRate,
  }
}

const heldOutBaselineScore = (
  plan: AgentClRepoReusePlan,
): AgentClPassScore => ({
  pass: 'baseline',
  taskRole: 'held_out',
  taskCount: plan.heldOutTasks.length,
  acceptedOutcomeRate: roundGain(
    plan.heldOutTasks.reduce(
      (total, task) => total + scoreForAttempt('held_out_baseline', task.taskRef),
      0,
    ) / plan.heldOutTasks.length,
  ),
})

const workloadForAgentClTaskRole = (
  role: AgentClTaskRole,
): BenchmarkWorkload => {
  switch (role) {
    case 'source':
      return 'agentcl-source-task'
    case 'complex':
      return 'agentcl-complex-task'
    case 'held_out':
      return 'agentcl-held-out-task'
  }
}

const taskSequenceForPlan = (
  plan: AgentClRepoReusePlan,
  compiled: CompiledGymExperiment,
): ReadonlyArray<AgentClTaskSequenceEntry> => {
  const tasksByRef = agentClTaskByRef(plan)
  const publishedTaskRefs = new Set(
    GYM_ENVIRONMENT_REGISTRY['agentcl-repo-reuse'].taskSet.publicSafeTaskRefs,
  )
  const entries: Array<AgentClTaskSequenceEntry> = []

  for (const pass of plan.passes) {
    for (const taskRef of pass.taskRefs) {
      const task = tasksByRef[taskRef]
      if (task === undefined || !publishedTaskRefs.has(taskRef)) {
        continue
      }
      entries.push({
        sequenceIndex: entries.length + 1,
        taskRef,
        taskRole: task.role,
        workload: workloadForAgentClTaskRole(task.role),
        pass: pass.pass,
        memoryAccess: pass.memoryAccess,
        sourceTaskSetRef: compiled.policySelection.environment.taskSetRef,
        verifierRef: compiled.policySelection.environment.verifierRef,
      })
    }
  }

  return entries
}

const evaluateAgentClTrajectories = (
  input: Readonly<{
    sequence: ReadonlyArray<AgentClTaskSequenceEntry>
    sequentialRun: AgentClSequentialRun
    fixtureRun: GymFixtureRunResult
  }>,
): ReadonlyArray<AgentClTrajectoryEvaluation> => {
  const runPoolsByWorkload = input.fixtureRun.runSet.runs.reduce<
    Record<string, typeof input.fixtureRun.runSet.runs>
  >((acc, run) => {
    if (run.record === null) {
      return acc
    }
    acc[run.cell.workload] = [...(acc[run.cell.workload] ?? []), run]
    return acc
  }, {})
  const usedByWorkload: Record<string, number> = {}

  return input.sequentialRun.taskAttempts.flatMap(attempt => {
    const sequenceEntry = input.sequence.find(
      entry =>
        entry.sequenceIndex === attempt.stepIndex &&
        entry.taskRef === attempt.taskRef &&
        entry.pass === attempt.pass,
    )
    if (sequenceEntry === undefined) {
      return []
    }
    const pool = runPoolsByWorkload[sequenceEntry.workload] ?? []
    const nextIndex = usedByWorkload[sequenceEntry.workload] ?? 0
    const run = pool[nextIndex % Math.max(pool.length, 1)]
    usedByWorkload[sequenceEntry.workload] = nextIndex + 1
    if (run === undefined || run.record === null) {
      return []
    }
    const scalarReward =
      typeof run.record.scalarReward === 'number' ? run.record.scalarReward : 0
    return [
      {
        trajectoryRef:
          `trajectory.public.agentcl.${attempt.pass}.` +
          `step_${attempt.stepIndex}.${attempt.taskRef}`,
        attemptRef: attempt.attemptRef,
        taskRef: attempt.taskRef,
        taskRole: attempt.taskRole,
        workload: sequenceEntry.workload,
        pass: attempt.pass,
        acceptedOutcome: attempt.acceptedOutcome,
        acceptedOutcomeScore: attempt.acceptedOutcomeScore,
        scalarReward,
        executedVerdict: run.record.executedVerdict,
        benchmarkCellId: run.cellId,
        benchmarkSampleIndex: run.sampleIndex,
        telemetryRequestId: run.record.requestId,
        memoryMutationRefs: attempt.mutationRefs,
      },
    ]
  })
}

export const runAgentClSequentialLoop = (
  plan: AgentClRepoReusePlan,
): AgentClSequentialRun => {
  const tasksByRef = agentClTaskByRef(plan)
  const templateLedger = exampleArtanisContinualLearningTemplateLedger()
  const initialMemoryRefs: ReadonlyArray<string> = [
    'memory.public.agentcl.seed.empty.v0',
  ]
  const seed = {
    memoryRefs: initialMemoryRefs,
    taskAttempts: [] as Array<AgentClTaskAttempt>,
    memoryMutations: [] as Array<AgentClMemoryMutation>,
    stepIndex: 0,
  }
  const result = plan.passes.reduce((state, pass) => {
    return pass.taskRefs.reduce((innerState, taskRef) => {
      const task = tasksByRef[taskRef]
      if (task === undefined) {
        return innerState
      }
      const stepIndex = innerState.stepIndex + 1
      const memoryBeforeRefs = innerState.memoryRefs
      const score = scoreForAttempt(pass.pass, task.taskRef)
      const feedbackRef = `feedback.public.agentcl.${pass.pass}.${task.taskRef}`
      const stateCarryForwardRef = `state.public.agentcl.${pass.pass}.step_${stepIndex}`
      const shouldMutate = pass.memoryAccess === 'read_write'
      const templateRef = memoryConsolidationTemplateRef(task)
      const mutationRef = `mutation.public.agentcl.${pass.pass}.step_${stepIndex}.${task.taskRef}`
      const memoryAfterRefs = shouldMutate
        ? [
            ...memoryBeforeRefs,
            ...task.reusableSolutionRefs,
            mutationRef,
          ]
        : memoryBeforeRefs
      const mutationRefs = shouldMutate ? [mutationRef] : []
      const appliedTemplateRefs = shouldMutate ? [templateRef] : []
      const attempt: AgentClTaskAttempt = {
        attemptRef: `attempt.public.agentcl.${pass.pass}.step_${stepIndex}.${task.taskRef}`,
        pass: pass.pass,
        taskRef: task.taskRef,
        taskRole: task.role,
        stepIndex,
        memoryAccess: pass.memoryAccess,
        memoryBeforeRefs,
        memoryAfterRefs,
        appliedTemplateRefs,
        mutationRefs,
        acceptedOutcome: score >= 0.5,
        acceptedOutcomeScore: score,
        feedbackRef,
        stateCarryForwardRef,
      }
      const mutation: ReadonlyArray<AgentClMemoryMutation> = shouldMutate
        ? [
            {
              mutationRef,
              pass: pass.pass,
              taskRef: task.taskRef,
              stepIndex,
              templateRef,
              memoryBeforeRefs,
              memoryAfterRefs,
              feedbackRef,
            },
          ]
        : []
      return {
        memoryRefs: memoryAfterRefs,
        taskAttempts: [...innerState.taskAttempts, attempt],
        memoryMutations: [...innerState.memoryMutations, ...mutation],
        stepIndex,
      }
    }, state)
  }, seed)

  return decodeSequentialRun({
    runRef: `run.public.agentcl.sequential.${plan.experimentId}.v0`,
    taskAttemptCount: result.taskAttempts.length,
    memoryMutationCount: result.memoryMutations.length,
    initialMemoryRefs,
    finalMemoryRefs: result.memoryRefs,
    templateLedgerRef: templateLedger.ledgerRef,
    taskAttempts: result.taskAttempts,
    memoryMutations: result.memoryMutations,
  })
}

const SOURCE_TASKS: ReadonlyArray<AgentClRepoReuseTask> = [
  {
    taskRef: 'agentcl.repo_reuse.source.effect_schema_contract.v0',
    role: 'source',
    packageRef: 'apps/openagents.com/workers/api/src/inference/gym',
    publicObjectiveRef: 'public.issue.6420.source.agentcl_eval_contract',
    reusableSolutionRefs: ['solution.agentcl.effect_schema.pg_sg_gg.v0'],
    expectedReuseFromTaskRefs: [],
  },
  {
    taskRef: 'agentcl.repo_reuse.source.harbor_public_receipt.v0',
    role: 'source',
    packageRef: 'apps/openagents.com/workers/api/src/inference/gym',
    publicObjectiveRef: 'public.issue.6420.source.harbor_dispatch_receipts',
    reusableSolutionRefs: ['solution.agentcl.harbor_dispatch_profile.v0'],
    expectedReuseFromTaskRefs: [],
  },
  {
    taskRef: 'agentcl.repo_reuse.source.tas_memory_ref.v0',
    role: 'source',
    packageRef: 'apps/pylon/src/tas',
    publicObjectiveRef: 'public.issue.6420.source.tas_memory_public_refs',
    reusableSolutionRefs: ['solution.agentcl.memory_reference_only.v0'],
    expectedReuseFromTaskRefs: [],
  },
  {
    taskRef: 'agentcl.repo_reuse.source.omni_retrieval_ref.v0',
    role: 'source',
    packageRef: 'apps/openagents.com/workers/api/src/inference',
    publicObjectiveRef: 'public.issue.6420.source.omni_retrieval_public_refs',
    reusableSolutionRefs: ['solution.agentcl.retrieve_but_verify.v0'],
    expectedReuseFromTaskRefs: [],
  },
]

const COMPLEX_TASKS: ReadonlyArray<AgentClRepoReuseTask> = [
  {
    taskRef: 'agentcl.repo_reuse.complex.two_pass_runner.v0',
    role: 'complex',
    packageRef: 'apps/openagents.com/workers/api/src/inference/gym',
    publicObjectiveRef: 'public.issue.6420.complex.two_pass_runner',
    reusableSolutionRefs: ['solution.agentcl.pass_plan.composition.v0'],
    expectedReuseFromTaskRefs: SOURCE_TASKS.map(task => task.taskRef),
  },
  {
    taskRef: 'agentcl.repo_reuse.complex.pg_sg_gg_report.v0',
    role: 'complex',
    packageRef: 'apps/openagents.com/workers/api/src/inference/gym',
    publicObjectiveRef: 'public.issue.6420.complex.pg_sg_gg_report',
    reusableSolutionRefs: ['solution.agentcl.separate_gain_report.v0'],
    expectedReuseFromTaskRefs: [
      'agentcl.repo_reuse.source.effect_schema_contract.v0',
      'agentcl.repo_reuse.source.harbor_public_receipt.v0',
    ],
  },
]

const HELD_OUT_TASKS: ReadonlyArray<AgentClRepoReuseTask> = [
  {
    taskRef: 'agentcl.repo_reuse.held_out.mirrorcode_no_rag.v0',
    role: 'held_out',
    packageRef: 'apps/openagents.com/scripts/mirrorcode',
    publicObjectiveRef: 'public.issue.6420.held_out.mirrorcode_no_rag',
    reusableSolutionRefs: [],
    expectedReuseFromTaskRefs: [],
  },
]

export const buildAgentClRepoReusePlan = (
  experiment: GymExperiment = AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
): Readonly<{
  compiled: CompiledGymExperiment
  plan: AgentClRepoReusePlan
}> => {
  const compiled = compileGymExperiment(experiment)
  const definition = GYM_ENVIRONMENT_REGISTRY['agentcl-repo-reuse']
  const passBase = {
    dispatchSurfaceRef: 'gym.harbor_dispatch.public_safe_plan.v0',
    harborDispatchProfileRef: 'harbor.dispatch.agentcl.repo_reuse.no_spend.v0',
  }
  return {
    compiled,
    plan: decodePlan({
      schemaVersion: AGENTCL_REPO_REUSE_PLAN_SCHEMA,
      environmentRef: 'agentcl-repo-reuse',
      experimentId: experiment.id,
      streamKind: 'compositional',
      sourceTasks: SOURCE_TASKS,
      complexTasks: COMPLEX_TASKS,
      heldOutTasks: HELD_OUT_TASKS,
      passes: [
        {
          ...passBase,
          pass: 'baseline',
          memoryAccess: 'disabled',
          taskRefs: [
            ...agentClTaskRefs(SOURCE_TASKS),
            ...agentClTaskRefs(COMPLEX_TASKS),
          ],
        },
        {
          ...passBase,
          pass: 'first_pass',
          memoryAccess: 'read_write',
          taskRefs: [
            ...agentClTaskRefs(SOURCE_TASKS),
            ...agentClTaskRefs(COMPLEX_TASKS),
          ],
        },
        {
          ...passBase,
          pass: 'frozen_second_pass',
          memoryAccess: 'read_only_frozen',
          taskRefs: agentClTaskRefs(COMPLEX_TASKS),
        },
        {
          ...passBase,
          pass: 'held_out_pass',
          memoryAccess: 'read_only_frozen',
          taskRefs: agentClTaskRefs(HELD_OUT_TASKS),
        },
      ],
      memorySystemsUnderTest: experiment.policy.modules.moduleRefs,
      publicSafetyBoundary: {
        publicTaskRefsOnly: true,
        rawPromptsStayOwnerPrivate: true,
        noTrainingOnHeldOut: true,
        reportPgSgGgSeparately: true,
        publicClaimEligible: definition.acceptance.publicClaimEligible,
      },
    }),
  }
}

export const runAgentClTaskRunner = (
  experiment: GymExperiment = AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
): Readonly<{
  compiled: CompiledGymExperiment
  fixtureRun: GymFixtureRunResult
  plan: AgentClRepoReusePlan
  sequentialRun: AgentClSequentialRun
  taskRunner: AgentClTaskRunnerResult
}> => {
  const { compiled, plan } = buildAgentClRepoReusePlan(experiment)
  const fixtureRun = runGymFixtureExperiment(experiment)
  const sequentialRun = runAgentClSequentialLoop(plan)
  const taskSequence = taskSequenceForPlan(plan, compiled)
  const trajectoryEvaluations = evaluateAgentClTrajectories({
    sequence: taskSequence,
    sequentialRun,
    fixtureRun,
  })

  return {
    compiled,
    fixtureRun,
    plan,
    sequentialRun,
    taskRunner: decodeTaskRunnerResult({
      schemaVersion: AGENTCL_TASK_RUNNER_RESULT_SCHEMA,
      experimentId: experiment.id,
      taskSetRef: fixtureRun.compiled.policySelection.environment.taskSetRef,
      verifierRef: fixtureRun.compiled.policySelection.environment.verifierRef,
      acceptanceContractRef:
        fixtureRun.compiled.policySelection.environment.acceptanceContractRef,
      runnerConfigId: fixtureRun.runSet.configId,
      seamId: fixtureRun.runSet.seamId,
      seamCanSpend: fixtureRun.runSet.seamCanSpend,
      loadedTaskRefs:
        GYM_ENVIRONMENT_REGISTRY['agentcl-repo-reuse'].taskSet
          .publicSafeTaskRefs,
      taskSequence,
      trajectoryEvaluations,
      publicSafety: fixtureRun.publicSafety,
    }),
  }
}

export const runAgentClRepoReuseFixtureEval = (
  experiment: GymExperiment = AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
): Readonly<{
  compiled: CompiledGymExperiment
  fixtureRun: GymFixtureRunResult
  plan: AgentClRepoReusePlan
  taskRunner: AgentClTaskRunnerResult
  eval: AgentClEvalV0
}> => {
  const { compiled, fixtureRun, plan, sequentialRun, taskRunner } =
    runAgentClTaskRunner(experiment)
  const baseline = passScore(sequentialRun.taskAttempts, 'baseline', 'complex')
  const firstPass = passScore(
    sequentialRun.taskAttempts,
    'first_pass',
    'complex',
  )
  const frozenSecondPass = passScore(
    sequentialRun.taskAttempts,
    'frozen_second_pass',
    'complex',
  )
  const heldOutBaseline = heldOutBaselineScore(plan)
  const heldOutPass = passScore(
    sequentialRun.taskAttempts,
    'held_out_pass',
    'held_out',
  )

  const plasticityGain = roundGain(
    firstPass.acceptedOutcomeRate - baseline.acceptedOutcomeRate,
  )
  const stabilityGain = roundGain(
    frozenSecondPass.acceptedOutcomeRate - firstPass.acceptedOutcomeRate,
  )
  const generalizationGain = roundGain(
    heldOutPass.acceptedOutcomeRate - heldOutBaseline.acceptedOutcomeRate,
  )

  return {
    compiled,
    fixtureRun,
    plan,
    taskRunner,
    eval: decodeEval({
      schemaVersion: AGENTCL_EVAL_SCHEMA,
      environmentRef: 'agentcl-repo-reuse',
      experimentId: experiment.id,
      streamKind: plan.streamKind,
      memorySystemsUnderTest: plan.memorySystemsUnderTest,
      baseline,
      firstPass,
      frozenSecondPass,
      heldOutBaseline,
      heldOutPass,
      plasticityGain,
      stabilityGain,
      generalizationGain,
      sequentialRun,
      claimDiscipline: {
        decisionGrade: false,
        publicClaimEligible: false,
        collapseGainsIntoOneNumber: false,
        notes: [
          'fixture_only_first_measurement_not_a_product_claim',
          stabilityGain < 0
            ? 'negative_stability_gain_requires_memory_review'
            : 'non_negative_stability_gain',
          generalizationGain < 0
            ? 'negative_generalization_gain_blocks_generalizes_claim'
            : 'non_negative_generalization_gain',
        ],
      },
      proofRefs: [
        compiled.policySelection.environment.taskSetRef,
        compiled.policySelection.environment.verifierRef,
        compiled.policySelection.environment.acceptanceContractRef,
      ],
      taskRunner,
    }),
  }
}

export const buildAgentClVertexStressExperiment = (
  ownerApprovalRef: string,
): GymExperiment => {
  const runnerPlan = buildAgentClVertexGeminiRunnerPlan(ownerApprovalRef)

  return {
    ...AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
    id: 'gym-agentcl-vertex-gemini35-flash-stress-cl5-v0',
    policy: {
      ...AGENTCL_REPO_REUSE_GYM_EXPERIMENT.policy,
      fanout: {
        lanes: [runnerPlan.lane.laneRef],
        mode: 'single',
        concurrency: runnerPlan.parallelism.plannedParallelSequences,
      },
      sampling: {
        ...AGENTCL_REPO_REUSE_GYM_EXPERIMENT.policy.sampling,
        maxTokens: 8192,
        transport: 'streaming',
      },
    },
    budget: {
      spendCapMsat: 0,
      maxBillableSamples: runnerPlan.parallelism.plannedParallelSequences,
      seam: 'real',
      ownerApprovalRef,
    },
  }
}

export const buildAgentClVertexGeminiRunnerPlan = (
  ownerApprovalRef: string,
): AgentClVertexRunnerPlanV0 =>
  decodeVertexRunnerPlan({
    schemaVersion: AGENTCL_VERTEX_RUNNER_PLAN_SCHEMA,
    issueRef: 'public.issue.6766',
    lane: {
      laneRef: 'vertex-gemini',
      model: 'gemini-3.5-flash',
      projectRef: 'project.openagentsgemini',
      forbiddenFallbackLaneRefs: ['glm-free', 'khala-free'],
      requiresPreScaleVertexProof: true,
      preScaleProofRef: 'proof.agentcl.vertex_gemini35_flash.pre_scale_routing',
    },
    parallelism: {
      plannedParallelSequences: 10,
      verifyRouteBeforeScaling: true,
    },
    budgetGuard: {
      spendCapUsdCents: 5000,
      abortOnEstimatedSpendAboveCap: true,
      abortOnConsecutiveBillingOrQuotaErrors: 3,
      trackedCapacityErrorRefs: [
        'billing_error',
        'quota_error',
        'http_429',
        'resource_exhausted',
      ],
    },
    ownerApprovalRef,
    publicSafety: {
      rawPromptsStayOwnerPrivate: true,
      noProviderPayloadsInPublicReport: true,
      noSpendWithoutOwnerApproval: true,
    },
    reportRefs: [
      'public.issue.6766',
      'route.gym.agentcl.vertex_gemini35_flash',
      'cap.usd.agentcl.vertex_stress.50',
    ],
  })

export const assessAgentClVertexRunnerCircuitBreaker = (
  input: Readonly<{
    estimatedSpendUsdCents: number
    consecutiveBillingOrQuotaErrors: number
  }>,
): Readonly<{
  tripped: boolean
  reason: AgentClVertexStressCircuitBreakerReason
}> => {
  const runnerPlan = buildAgentClVertexGeminiRunnerPlan(
    'owner.approval.agentcl.vertex_stress.required',
  )
  const reason =
    input.estimatedSpendUsdCents > runnerPlan.budgetGuard.spendCapUsdCents
      ? 'spend_cap_exceeded'
      : input.consecutiveBillingOrQuotaErrors >=
          runnerPlan.budgetGuard.abortOnConsecutiveBillingOrQuotaErrors
        ? 'consecutive_billing_or_quota_errors'
        : 'none'

  return {
    tripped: reason !== 'none',
    reason,
  }
}

export const buildAgentClVertexStressBaselineReport = (
  input: Readonly<{
    eval?: AgentClEvalV0
    runMode?: AgentClVertexStressRunMode
    verifiedVertexBeforeScale?: boolean
    attemptedSequences?: number
    completedSequences?: number
    peakAcceptedParallelSequences?: number
    http429Count?: number
    resourceExhaustedCount?: number
    estimatedSpendUsdCents?: number
    consecutiveBillingOrQuotaErrors?: number
  }> = {},
): AgentClVertexStressReportV0 => {
  const stressExperiment = buildAgentClVertexStressExperiment(
    'owner.approval.agentcl.vertex_stress.required',
  )
  const evalResult =
    input.eval ?? runAgentClRepoReuseFixtureEval(stressExperiment).eval
  const verifiedVertexBeforeScale = input.verifiedVertexBeforeScale ?? false
  const http429Count = input.http429Count ?? 0
  const resourceExhaustedCount = input.resourceExhaustedCount ?? 0
  const consecutiveBillingOrQuotaErrors =
    input.consecutiveBillingOrQuotaErrors ?? 0
  const estimatedSpendUsdCents = input.estimatedSpendUsdCents ?? 0
  const circuitBreaker = assessAgentClVertexRunnerCircuitBreaker({
    estimatedSpendUsdCents,
    consecutiveBillingOrQuotaErrors,
  })

  return decodeVertexStressReport({
    schemaVersion: AGENTCL_VERTEX_STRESS_REPORT_SCHEMA,
    issueRef: 'public.issue.6767',
    experimentId: stressExperiment.id,
    runMode: input.runMode ?? 'fixture_baseline',
    routing: {
      laneRef: 'vertex-gemini',
      model: 'gemini-3.5-flash',
      projectRef: 'project.openagentsgemini',
      verifiedVertexBeforeScale,
      proofRefs: verifiedVertexBeforeScale
        ? ['proof.agentcl.vertex_gemini35_flash.pre_scale_routing']
        : [],
    },
    budgetGuard: {
      spendCapUsdCents: 5000,
      estimatedSpendUsdCents,
      consecutiveBillingOrQuotaErrors,
      circuitBreakerTripped: circuitBreaker.tripped,
      circuitBreakerReason: circuitBreaker.reason,
    },
    capacityReport: {
      plannedParallelSequences: 10,
      attemptedSequences: input.attemptedSequences ?? 0,
      completedSequences: input.completedSequences ?? 0,
      peakAcceptedParallelSequences: input.peakAcceptedParallelSequences ?? 0,
      http429Count,
      resourceExhaustedCount,
      capacityLimitHit: http429Count > 0 || resourceExhaustedCount > 0,
    },
    learningCurves: {
      plasticityGain: [evalResult.baseline, evalResult.firstPass],
      stabilityGain: [evalResult.firstPass, evalResult.frozenSecondPass],
      generalizationGain: [evalResult.heldOutBaseline, evalResult.heldOutPass],
    },
    eval: evalResult,
    decisionGrade: false,
    publicClaimEligible: false,
    blockerRefs: [
      ...(verifiedVertexBeforeScale
        ? []
        : ['blocker.gym.agentcl.vertex_routing_not_verified']),
      ...(http429Count > 0 || resourceExhaustedCount > 0
        ? []
        : ['blocker.gym.agentcl.vertex_capacity_limit_not_hit']),
      ...(input.runMode === 'owner_armed_real'
        ? []
        : ['blocker.gym.agentcl.fixture_baseline_not_live_stress']),
    ],
    reportRefs: [
      'public.issue.6767',
      'route.gym.agentcl.vertex_gemini35_flash',
      'cap.usd.agentcl.vertex_stress.50',
    ],
  })
}

export const assessAgentClLearningClaimGate = (
  input: Readonly<{
    claimKind: AgentClLearningClaimKind
    eval?: AgentClLearningClaimEvidence
    collapsedMemoryImprovementMetric?: number
  }>,
): AgentClLearningClaimGate => {
  const hasSeparatePlasticityGain = input.eval?.plasticityGain !== undefined
  const hasSeparateStabilityGain = input.eval?.stabilityGain !== undefined
  const hasSeparateGeneralizationGain =
    input.eval?.generalizationGain !== undefined
  const hasAllSeparateGains =
    hasSeparatePlasticityGain &&
    hasSeparateStabilityGain &&
    hasSeparateGeneralizationGain
  const decisionGradeEvidence =
    input.eval?.claimDiscipline?.decisionGrade === true
  const publicClaimEvidence =
    input.eval?.claimDiscipline?.publicClaimEligible === true
  const blockerRefs = [
    ...(hasSeparatePlasticityGain
      ? []
      : ['blocker.gym.agentcl.missing_plasticity_gain']),
    ...(hasSeparateStabilityGain
      ? []
      : ['blocker.gym.agentcl.missing_stability_gain']),
    ...(hasSeparateGeneralizationGain
      ? []
      : ['blocker.gym.agentcl.missing_generalization_gain']),
    ...(input.collapsedMemoryImprovementMetric === undefined
      ? []
      : ['blocker.gym.agentcl.collapsed_memory_improvement_metric']),
    ...(decisionGradeEvidence
      ? []
      : ['blocker.gym.agentcl.not_decision_grade']),
    ...(publicClaimEvidence
      ? []
      : ['blocker.gym.agentcl.public_claim_not_eligible']),
  ]

  return decodeLearningClaimGate({
    schemaVersion: 'openagents.gym.agentcl_learning_claim_gate.v0',
    claimKind: input.claimKind,
    evidenceSchemaVersion: AGENTCL_EVAL_SCHEMA,
    requiresSeparatePgSgGg: true,
    hasSeparatePlasticityGain,
    hasSeparateStabilityGain,
    hasSeparateGeneralizationGain,
    collapsedMemoryImprovementMetricAccepted: false,
    decisionGradeClaimAllowed:
      hasAllSeparateGains &&
      input.collapsedMemoryImprovementMetric === undefined &&
      decisionGradeEvidence,
    publicClaimAllowed:
      hasAllSeparateGains &&
      input.collapsedMemoryImprovementMetric === undefined &&
      decisionGradeEvidence &&
      publicClaimEvidence,
    blockerRefs,
  })
}
