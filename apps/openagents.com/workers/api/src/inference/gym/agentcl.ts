import { Schema as S } from 'effect'

import {
  AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
  GYM_ENVIRONMENT_REGISTRY,
  compileGymExperiment,
  type CompiledGymExperiment,
  type GymExperiment,
} from './experiment'

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

const roundGain = (value: number): number => Math.round(value * 1000) / 1000

const agentClTaskRefs = (
  tasks: ReadonlyArray<AgentClRepoReuseTask>,
): ReadonlyArray<string> => tasks.map(task => task.taskRef)

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
  const baseline: AgentClPassScore = {
    pass: 'baseline',
    taskRole: 'complex',
    taskCount: plan.complexTasks.length,
    acceptedOutcomeRate: 0.45,
  }
  const firstPass: AgentClPassScore = {
    pass: 'first_pass',
    taskRole: 'complex',
    taskCount: plan.complexTasks.length,
    acceptedOutcomeRate: 0.62,
  }
  const frozenSecondPass: AgentClPassScore = {
    pass: 'frozen_second_pass',
    taskRole: 'complex',
    taskCount: plan.complexTasks.length,
    acceptedOutcomeRate: 0.58,
  }
  const heldOutBaseline: AgentClPassScore = {
    pass: 'baseline',
    taskRole: 'held_out',
    taskCount: plan.heldOutTasks.length,
    acceptedOutcomeRate: 0.7,
  }
  const heldOutPass: AgentClPassScore = {
    pass: 'held_out_pass',
    taskRole: 'held_out',
    taskCount: plan.heldOutTasks.length,
    acceptedOutcomeRate: 0.66,
  }

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
