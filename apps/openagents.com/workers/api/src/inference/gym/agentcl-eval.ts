import { Schema as S } from 'effect'

import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import {
  AGENTCL_REPO_REUSE_GYM_EXPERIMENT,
  GYM_ENVIRONMENT_REGISTRY,
  type GymEnvironmentDefinition,
  type GymExperiment,
} from './experiment'

export const AgentClEvalSchemaVersion = 'openagents.gym.agentcl_eval.v0'
export const AgentClRepoReuseStreamSchemaVersion =
  'openagents.gym.agentcl_repo_reuse_stream.v1'

export const AgentClStreamKind = S.Literals(['naive', 'compositional'])
export type AgentClStreamKind = typeof AgentClStreamKind.Type

export const AgentClTaskRole = S.Literals(['source', 'target', 'held_out'])
export type AgentClTaskRole = typeof AgentClTaskRole.Type

export const AgentClPassKind = S.Literals([
  'baseline',
  'first_pass',
  'frozen_second_pass',
  'held_out_pass',
])
export type AgentClPassKind = typeof AgentClPassKind.Type

export const AgentClMemoryConfigRef = S.Literals([
  'memoryless-react-baseline',
  'pylon-tas-memory',
  'omni-retrieval',
])
export type AgentClMemoryConfigRef = typeof AgentClMemoryConfigRef.Type

const AgentClEvidenceOnlyAuthority = S.Struct({
  publicClaimAllowed: S.Literal(false),
  runtimePromotionAllowed: S.Literal(false),
  payoutAllowed: S.Literal(false),
  settlementAllowed: S.Literal(false),
  providerMutationAllowed: S.Literal(false),
})
type AgentClEvidenceOnlyAuthority = typeof AgentClEvidenceOnlyAuthority.Type

const evidenceOnlyAuthority: AgentClEvidenceOnlyAuthority = {
  publicClaimAllowed: false,
  runtimePromotionAllowed: false,
  payoutAllowed: false,
  settlementAllowed: false,
  providerMutationAllowed: false,
}

export const AgentClRepoReuseTask = S.Struct({
  taskRef: S.String,
  role: AgentClTaskRole,
  packageRef: S.String,
  promptSummaryRef: S.String,
  reusableSubSolutionRefs: S.Array(S.String),
  verifierRef: S.String,
  publicSafeSourceRefs: S.Array(S.String),
})
export type AgentClRepoReuseTask = typeof AgentClRepoReuseTask.Type

export const AgentClRepoReuseRelation = S.Struct({
  relationRef: S.String,
  sourceTaskRefs: S.Array(S.String),
  targetTaskRef: S.String,
  reusableSubSolutionRefs: S.Array(S.String),
})
export type AgentClRepoReuseRelation = typeof AgentClRepoReuseRelation.Type

export const AgentClRepoReuseStream = S.Struct({
  schemaVersion: S.Literal(AgentClRepoReuseStreamSchemaVersion),
  streamRef: S.String,
  streamKind: AgentClStreamKind,
  environmentRef: S.Literal('agentcl-repo-reuse'),
  taskSetRef: S.String,
  tasks: S.Array(AgentClRepoReuseTask),
  compositionalRelations: S.Array(AgentClRepoReuseRelation),
  heldOutTaskRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  publicSafe: S.Literal(true),
  rawArtifactsIncluded: S.Literal(false),
})
export type AgentClRepoReuseStream = typeof AgentClRepoReuseStream.Type

export const AgentClPassScore = S.Struct({
  taskRef: S.String,
  role: AgentClTaskRole,
  pass: AgentClPassKind,
  memoryConfigRef: AgentClMemoryConfigRef,
  score: S.Number.check(S.isBetween({ minimum: 0, maximum: 1 })),
  verifierRef: S.String,
})
export type AgentClPassScore = typeof AgentClPassScore.Type

export const AgentClEvalAggregate = S.Struct({
  baseline: S.Number,
  firstPass: S.Number,
  frozenSecondPass: S.Number,
  heldOutBaseline: S.Number,
  heldOutPass: S.Number,
})
export type AgentClEvalAggregate = typeof AgentClEvalAggregate.Type

export const AgentClEvalGains = S.Struct({
  plasticityGain: S.Number,
  stabilityGain: S.Number,
  generalizationGain: S.Number,
})
export type AgentClEvalGains = typeof AgentClEvalGains.Type

export const AgentClEval = S.Struct({
  schemaVersion: S.Literal(AgentClEvalSchemaVersion),
  evalRef: S.String,
  generatedAt: S.String,
  experimentId: S.String,
  environmentRef: S.Literal('agentcl-repo-reuse'),
  streamRef: S.String,
  streamKind: AgentClStreamKind,
  memoryConfigRef: AgentClMemoryConfigRef,
  baselinePassRef: S.String,
  firstPassRef: S.String,
  frozenSecondPassRef: S.String,
  heldOutPassRef: S.String,
  scores: S.Array(AgentClPassScore),
  aggregates: AgentClEvalAggregate,
  gains: AgentClEvalGains,
  decisionGrade: S.Literal(false),
  publicSafe: S.Literal(true),
  rawArtifactsIncluded: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  authority: AgentClEvidenceOnlyAuthority,
})
export type AgentClEval = typeof AgentClEval.Type

export class AgentClEvalError extends S.TaggedErrorClass<AgentClEvalError>()(
  'AgentClEvalError',
  {
    reason: S.Literals([
      'wrong_environment',
      'empty_stream',
      'missing_compositional_relation',
      'missing_required_score',
    ]),
    message: S.String,
  },
) {}

const decodeStream = S.decodeUnknownSync(AgentClRepoReuseStream)
const decodeEval = S.decodeUnknownSync(AgentClEval)

const roundGain = (value: number): number => Math.round(value * 10000) / 10000

const averageScore = (
  scores: ReadonlyArray<AgentClPassScore>,
  pass: AgentClPassKind,
  role: AgentClTaskRole,
): number => {
  const matchingScores = scores.filter(
    score => score.pass === pass && score.role === role,
  )
  if (matchingScores.length === 0) {
    throw new AgentClEvalError({
      reason: 'missing_required_score',
      message: `AgentCL eval cannot compute ${pass}/${role} without scored rows.`,
    })
  }
  return roundGain(
    matchingScores.reduce((sum, score) => sum + score.score, 0) /
      matchingScores.length,
  )
}

const taskRefsForRole = (
  tasks: ReadonlyArray<AgentClRepoReuseTask>,
  role: AgentClTaskRole,
): ReadonlyArray<string> =>
  tasks.filter(task => task.role === role).map(task => task.taskRef)

const assertStreamCanRun = (
  stream: AgentClRepoReuseStream,
): AgentClRepoReuseStream => {
  if (stream.tasks.length === 0) {
    throw new AgentClEvalError({
      reason: 'empty_stream',
      message: 'AgentCL repo-reuse stream must include public-safe tasks.',
    })
  }
  if (
    stream.streamKind === 'compositional' &&
    stream.compositionalRelations.length === 0
  ) {
    throw new AgentClEvalError({
      reason: 'missing_compositional_relation',
      message:
        'Compositional AgentCL streams must declare source-to-target reuse relations.',
    })
  }
  return stream
}

const scoreForTask = (
  task: AgentClRepoReuseTask,
  pass: AgentClPassKind,
  memoryConfigRef: AgentClMemoryConfigRef,
): number => {
  if (memoryConfigRef === 'memoryless-react-baseline') {
    return task.role === 'held_out' ? 0.62 : task.role === 'target' ? 0.48 : 0.7
  }
  if (memoryConfigRef === 'omni-retrieval') {
    if (task.role === 'target' && pass === 'first_pass') {
      return 0.59
    }
    if (task.role === 'target' && pass === 'frozen_second_pass') {
      return 0.54
    }
    if (task.role === 'held_out' && pass === 'held_out_pass') {
      return 0.58
    }
  }
  if (task.role === 'target' && pass === 'first_pass') {
    return 0.65
  }
  if (task.role === 'target' && pass === 'frozen_second_pass') {
    return 0.59
  }
  if (task.role === 'held_out' && pass === 'held_out_pass') {
    return 0.56
  }
  return task.role === 'held_out' ? 0.62 : task.role === 'target' ? 0.48 : 0.72
}

const passesForTaskRole = (
  role: AgentClTaskRole,
): ReadonlyArray<AgentClPassKind> => {
  if (role === 'held_out') {
    return ['baseline', 'held_out_pass']
  }
  if (role === 'target') {
    return ['baseline', 'first_pass', 'frozen_second_pass']
  }
  return ['baseline', 'first_pass']
}

const buildFixtureScores = (
  stream: AgentClRepoReuseStream,
  memoryConfigRef: AgentClMemoryConfigRef,
): ReadonlyArray<AgentClPassScore> =>
  stream.tasks.flatMap(task =>
    passesForTaskRole(task.role).map(pass => ({
      taskRef: task.taskRef,
      role: task.role,
      pass,
      memoryConfigRef,
      score: scoreForTask(task, pass, memoryConfigRef),
      verifierRef: task.verifierRef,
    })),
  )

const calculateGains = (
  scores: ReadonlyArray<AgentClPassScore>,
): Readonly<{ aggregates: AgentClEvalAggregate; gains: AgentClEvalGains }> => {
  const aggregates = {
    baseline: averageScore(scores, 'baseline', 'target'),
    firstPass: averageScore(scores, 'first_pass', 'target'),
    frozenSecondPass: averageScore(scores, 'frozen_second_pass', 'target'),
    heldOutBaseline: averageScore(scores, 'baseline', 'held_out'),
    heldOutPass: averageScore(scores, 'held_out_pass', 'held_out'),
  }
  return {
    aggregates,
    gains: {
      plasticityGain: roundGain(aggregates.firstPass - aggregates.baseline),
      stabilityGain: roundGain(
        aggregates.frozenSecondPass - aggregates.firstPass,
      ),
      generalizationGain: roundGain(
        aggregates.heldOutPass - aggregates.heldOutBaseline,
      ),
    },
  }
}

export const buildAgentClRepoReuseStream = (
  definition: GymEnvironmentDefinition =
    GYM_ENVIRONMENT_REGISTRY['agentcl-repo-reuse'],
): AgentClRepoReuseStream => {
  if (definition.ref !== 'agentcl-repo-reuse') {
    throw new AgentClEvalError({
      reason: 'wrong_environment',
      message: `AgentCL repo-reuse stream cannot be built from ${definition.ref}.`,
    })
  }

  const tasks: ReadonlyArray<AgentClRepoReuseTask> = [
    {
      taskRef: 'agentcl.repo_reuse.source.effect_schema_gym_contract.v1',
      role: 'source',
      packageRef: 'apps/openagents.com/workers/api',
      promptSummaryRef: 'prompt.public.agentcl.source.effect_schema_contract',
      reusableSubSolutionRefs: ['subsolution.effect_schema_public_eval_contract'],
      verifierRef: definition.verifier.ref,
      publicSafeSourceRefs: ['docs/research/agentcl/incorporation-synthesis.md'],
    },
    {
      taskRef: 'agentcl.repo_reuse.source.harbor_dispatch_receipt.v1',
      role: 'source',
      packageRef: 'apps/openagents.com/workers/api',
      promptSummaryRef: 'prompt.public.agentcl.source.harbor_dispatch_receipt',
      reusableSubSolutionRefs: ['subsolution.harbor_public_safety_boundary'],
      verifierRef: definition.verifier.ref,
      publicSafeSourceRefs: [
        'apps/openagents.com/workers/api/src/inference/gym/harbor-dispatch.ts',
      ],
    },
    {
      taskRef: 'agentcl.repo_reuse.source.tas_memory_fixture.v1',
      role: 'source',
      packageRef: 'apps/pylon',
      promptSummaryRef: 'prompt.public.agentcl.source.tas_memory_fixture',
      reusableSubSolutionRefs: ['subsolution.tas_memory_reference_only'],
      verifierRef: definition.verifier.ref,
      publicSafeSourceRefs: ['apps/pylon/tests/tas-session-memory.test.ts'],
    },
    {
      taskRef: 'agentcl.repo_reuse.target.two_pass_runner.v1',
      role: 'target',
      packageRef: 'apps/openagents.com/workers/api',
      promptSummaryRef: 'prompt.public.agentcl.target.two_pass_runner',
      reusableSubSolutionRefs: [
        'subsolution.effect_schema_public_eval_contract',
        'subsolution.harbor_public_safety_boundary',
      ],
      verifierRef: definition.verifier.ref,
      publicSafeSourceRefs: ['docs/research/agentcl/incorporation-synthesis.md'],
    },
    {
      taskRef: 'agentcl.repo_reuse.target.pylon_tas_memory_measurement.v1',
      role: 'target',
      packageRef: 'apps/pylon',
      promptSummaryRef: 'prompt.public.agentcl.target.pylon_tas_measurement',
      reusableSubSolutionRefs: [
        'subsolution.tas_memory_reference_only',
        'subsolution.effect_schema_public_eval_contract',
      ],
      verifierRef: definition.verifier.ref,
      publicSafeSourceRefs: ['apps/pylon/tests/tas-session-memory.test.ts'],
    },
    {
      taskRef: 'agentcl.repo_reuse.held_out.mirrorcode_no_rag_rule.v1',
      role: 'held_out',
      packageRef: 'apps/openagents.com/scripts/mirrorcode',
      promptSummaryRef: 'prompt.public.agentcl.held_out.mirrorcode_no_rag',
      reusableSubSolutionRefs: [],
      verifierRef: definition.verifier.ref,
      publicSafeSourceRefs: ['docs/research/agentcl/incorporation-synthesis.md'],
    },
  ]

  return decodeStream({
    schemaVersion: AgentClRepoReuseStreamSchemaVersion,
    streamRef: 'stream.gym.agentcl_repo_reuse.compositional.v1',
    streamKind: 'compositional',
    environmentRef: 'agentcl-repo-reuse',
    taskSetRef: definition.taskSet.ref,
    tasks,
    compositionalRelations: [
      {
        relationRef: 'relation.agentcl.repo_reuse.contract_and_harbor_to_runner',
        sourceTaskRefs: [
          'agentcl.repo_reuse.source.effect_schema_gym_contract.v1',
          'agentcl.repo_reuse.source.harbor_dispatch_receipt.v1',
        ],
        targetTaskRef: 'agentcl.repo_reuse.target.two_pass_runner.v1',
        reusableSubSolutionRefs: [
          'subsolution.effect_schema_public_eval_contract',
          'subsolution.harbor_public_safety_boundary',
        ],
      },
      {
        relationRef: 'relation.agentcl.repo_reuse.tas_to_measurement',
        sourceTaskRefs: [
          'agentcl.repo_reuse.source.tas_memory_fixture.v1',
          'agentcl.repo_reuse.source.effect_schema_gym_contract.v1',
        ],
        targetTaskRef: 'agentcl.repo_reuse.target.pylon_tas_memory_measurement.v1',
        reusableSubSolutionRefs: [
          'subsolution.tas_memory_reference_only',
          'subsolution.effect_schema_public_eval_contract',
        ],
      },
    ],
    heldOutTaskRefs: taskRefsForRole(tasks, 'held_out'),
    sourceRefs: [
      'docs/research/agentcl/incorporation-synthesis.md',
      'issue.public.openagents.6420',
    ],
    caveatRefs: [
      'caveat.gym.agentcl.fixture_scores_not_live_pylon_execution',
      'caveat.gym.agentcl.pg_sg_gg_not_tassadar_weight_training_metric',
    ],
    publicSafe: true,
    rawArtifactsIncluded: false,
  })
}

export const runAgentClTwoPassFixtureEval = (
  input: Readonly<{
    experiment?: GymExperiment
    stream?: AgentClRepoReuseStream
    memoryConfigRef?: AgentClMemoryConfigRef
    generatedAt?: string
  }> = {},
): AgentClEval => {
  const experiment = input.experiment ?? AGENTCL_REPO_REUSE_GYM_EXPERIMENT
  if (experiment.environment !== 'agentcl-repo-reuse') {
    throw new AgentClEvalError({
      reason: 'wrong_environment',
      message: `AgentCL two-pass runner requires agentcl-repo-reuse, got ${experiment.environment}.`,
    })
  }

  const stream = assertStreamCanRun(
    input.stream ?? buildAgentClRepoReuseStream(),
  )
  const memoryConfigRef = input.memoryConfigRef ?? 'pylon-tas-memory'
  const scores = buildFixtureScores(stream, memoryConfigRef)
  const { aggregates, gains } = calculateGains(scores)
  const evalSegment = publicRefSegment(
    `${stream.streamRef}.${memoryConfigRef}.${experiment.id}`,
    'agentcl_repo_reuse_eval',
  )

  return decodeEval({
    schemaVersion: AgentClEvalSchemaVersion,
    evalRef: `eval.gym.agentcl_repo_reuse.${evalSegment}`,
    generatedAt: input.generatedAt ?? '2026-06-27T00:00:00.000Z',
    experimentId: experiment.id,
    environmentRef: 'agentcl-repo-reuse',
    streamRef: stream.streamRef,
    streamKind: stream.streamKind,
    memoryConfigRef,
    baselinePassRef: `pass.gym.agentcl_repo_reuse.${evalSegment}.baseline`,
    firstPassRef: `pass.gym.agentcl_repo_reuse.${evalSegment}.first`,
    frozenSecondPassRef: `pass.gym.agentcl_repo_reuse.${evalSegment}.frozen_second`,
    heldOutPassRef: `pass.gym.agentcl_repo_reuse.${evalSegment}.held_out`,
    scores,
    aggregates,
    gains,
    decisionGrade: false,
    publicSafe: true,
    rawArtifactsIncluded: false,
    evidenceRefs: uniqueRefs([
      stream.streamRef,
      experiment.id,
      ...stream.sourceRefs,
      ...stream.compositionalRelations.map(relation => relation.relationRef),
    ]),
    blockerRefs: [],
    caveatRefs: uniqueRefs([
      ...stream.caveatRefs,
      'caveat.gym.agentcl.eval_fixture_only_until_harbor_pylon_dispatch_runs',
      'caveat.gym.agentcl.memory_is_reference_not_authority',
    ]),
    authority: evidenceOnlyAuthority,
  })
}

export const summarizeAgentClEval = (
  evaluation: AgentClEval,
): ReadonlyArray<string> =>
  [
    `PG=${evaluation.gains.plasticityGain.toFixed(4)}`,
    `SG=${evaluation.gains.stabilityGain.toFixed(4)}`,
    `GG=${evaluation.gains.generalizationGain.toFixed(4)}`,
  ]
