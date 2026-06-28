import { Schema as S } from 'effect'

import {
  AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
  GYM_ENVIRONMENT_REGISTRY,
  compileGymExperiment,
  type CompiledGymExperiment,
  type GymExperiment,
} from './experiment'
import { exampleArtanisContinualLearningTemplateLedger } from '../../artanis-continual-learning-templates'

export { AGENTCL_REPO_REUSE_GYM_EXPERIMENT } from './experiment'

export const AGENTCL_EVAL_SCHEMA = 'openagents.gym.agentcl_eval.v0' as const
export const AGENTCL_REPO_REUSE_PLAN_SCHEMA =
  'openagents.gym.agentcl_repo_reuse_plan.v0' as const

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

export const runAgentClRepoReuseFixtureEval = (
  experiment: GymExperiment = AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
): Readonly<{
  compiled: CompiledGymExperiment
  plan: AgentClRepoReusePlan
  eval: AgentClEvalV0
}> => {
  const { compiled, plan } = buildAgentClRepoReusePlan(experiment)
  const sequentialRun = runAgentClSequentialLoop(plan)
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
    plan,
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
    }),
  }
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
