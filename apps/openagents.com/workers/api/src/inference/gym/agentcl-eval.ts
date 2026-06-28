import { Schema as S } from 'effect'

import { assertPylonGepaMetricCallPublicRefs } from '../../pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from '../../public-ref-format'

export const AgentClEvalSchemaVersion = 'openagents.gym.agentcl_eval.v0'

export const AgentClPhase = S.Literals([
  'baseline',
  'first_pass',
  'frozen_second_pass',
  'held_out',
])
export type AgentClPhase = typeof AgentClPhase.Type

export const AgentClGainKind = S.Literals([
  'plasticity',
  'stability',
  'generalization',
])
export type AgentClGainKind = typeof AgentClGainKind.Type

export const AgentClEvalPhaseScore = S.Struct({
  phase: AgentClPhase,
  scoreBps: S.Number,
  taskCount: S.Number,
  reportRef: S.String,
  receiptRef: S.String,
})
export type AgentClEvalPhaseScore = typeof AgentClEvalPhaseScore.Type

export const AgentClEvalGain = S.Struct({
  kind: AgentClGainKind,
  gainBps: S.Number,
  evidenceRefs: S.Array(S.String),
})
export type AgentClEvalGain = typeof AgentClEvalGain.Type

export const AgentClEval = S.Struct({
  schemaVersion: S.Literal(AgentClEvalSchemaVersion),
  evalRef: S.String,
  streamRef: S.String,
  candidateRef: S.String,
  memorySystemRef: S.String,
  baseline: AgentClEvalPhaseScore,
  firstPass: AgentClEvalPhaseScore,
  frozenSecondPass: AgentClEvalPhaseScore,
  heldOut: AgentClEvalPhaseScore,
  gains: S.Struct({
    plasticity: AgentClEvalGain,
    stability: AgentClEvalGain,
    generalization: AgentClEvalGain,
  }),
  caveatRefs: S.Array(S.String),
})
export type AgentClEval = typeof AgentClEval.Type

export const ContinualLearningClaim = S.Struct({
  claimRef: S.String,
  copy: S.String,
  agentClEval: S.optional(AgentClEval),
  legacySingleMetricBps: S.optional(S.Number),
  evidenceRefs: S.optional(S.Array(S.String)),
})
export type ContinualLearningClaim = typeof ContinualLearningClaim.Type

export type AgentClClaimDiscipline = Readonly<{
  ok: boolean
  evalRef: string | null
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
}>

export class AgentClEvalUnsafe extends S.TaggedErrorClass<AgentClEvalUnsafe>()(
  'AgentClEvalUnsafe',
  {
    reason: S.String,
  },
) {}

const AGENTCL_EVAL_CAVEATS = [
  'caveat.public.gym.agentcl_eval.non_parametric_memory_only',
  'caveat.public.gym.agentcl_eval.pg_sg_gg_reported_separately',
  'caveat.public.gym.agentcl_eval.held_out_not_memory_context',
] as const

const AGENTCL_CLAIM_CAVEATS = [
  'caveat.public.gym.continual_learning_claim.requires_agentcl_eval_v0',
  'caveat.public.gym.continual_learning_claim.no_single_memory_improved_metric',
] as const

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  try {
    assertPylonGepaMetricCallPublicRefs(label, normalized)
  } catch (error) {
    throw new AgentClEvalUnsafe({
      reason:
        error instanceof Error
          ? error.message
          : `${label} contains unsafe refs.`,
    })
  }
  return normalized
}

const assertFiniteBps = (label: string, value: number): void => {
  if (!Number.isFinite(value)) {
    throw new AgentClEvalUnsafe({ reason: `${label} must be finite.` })
  }
}

const assertPhase = (
  expected: AgentClPhase,
  phaseScore: AgentClEvalPhaseScore,
): void => {
  if (phaseScore.phase !== expected) {
    throw new AgentClEvalUnsafe({
      reason: `${expected} phase evidence was provided as ${phaseScore.phase}.`,
    })
  }
  assertFiniteBps(`${expected}.scoreBps`, phaseScore.scoreBps)
  if (!Number.isInteger(phaseScore.taskCount) || phaseScore.taskCount <= 0) {
    throw new AgentClEvalUnsafe({
      reason: `${expected}.taskCount must be a positive integer.`,
    })
  }
}

const assertGain = (
  expected: AgentClGainKind,
  gain: AgentClEvalGain,
  expectedGainBps: number,
): void => {
  if (gain.kind !== expected) {
    throw new AgentClEvalUnsafe({
      reason: `${expected} gain evidence was provided as ${gain.kind}.`,
    })
  }
  assertFiniteBps(`${expected}.gainBps`, gain.gainBps)
  if (gain.gainBps !== expectedGainBps) {
    throw new AgentClEvalUnsafe({
      reason: `${expected} gain must equal the measured phase delta.`,
    })
  }
  if (gain.evidenceRefs.length === 0) {
    throw new AgentClEvalUnsafe({
      reason: `${expected} gain requires separate evidence refs.`,
    })
  }
}

export const buildAgentClEval = (raw: unknown): AgentClEval => {
  let input: AgentClEval
  try {
    input = S.decodeUnknownSync(AgentClEval)(raw)
  } catch (error) {
    throw new AgentClEvalUnsafe({
      reason: error instanceof Error ? error.message : String(error),
    })
  }

  assertPhase('baseline', input.baseline)
  assertPhase('first_pass', input.firstPass)
  assertPhase('frozen_second_pass', input.frozenSecondPass)
  assertPhase('held_out', input.heldOut)
  assertGain(
    'plasticity',
    input.gains.plasticity,
    input.firstPass.scoreBps - input.baseline.scoreBps,
  )
  assertGain(
    'stability',
    input.gains.stability,
    input.frozenSecondPass.scoreBps - input.firstPass.scoreBps,
  )
  assertGain(
    'generalization',
    input.gains.generalization,
    input.heldOut.scoreBps - input.baseline.scoreBps,
  )

  const evalRef =
    input.evalRef === ''
      ? `eval.gym.agentcl.${publicRefSegment(
          [input.streamRef, input.candidateRef, input.memorySystemRef].join(
            '.',
          ),
          'unnamed',
        )}`
      : input.evalRef

  assertPublicRefs('AgentCL eval public refs', [
    evalRef,
    input.streamRef,
    input.candidateRef,
    input.memorySystemRef,
    input.baseline.reportRef,
    input.baseline.receiptRef,
    input.firstPass.reportRef,
    input.firstPass.receiptRef,
    input.frozenSecondPass.reportRef,
    input.frozenSecondPass.receiptRef,
    input.heldOut.reportRef,
    input.heldOut.receiptRef,
    ...input.gains.plasticity.evidenceRefs,
    ...input.gains.stability.evidenceRefs,
    ...input.gains.generalization.evidenceRefs,
    ...input.caveatRefs,
  ])

  return {
    ...input,
    evalRef,
    caveatRefs: uniqueRefs([...AGENTCL_EVAL_CAVEATS, ...input.caveatRefs]),
    gains: {
      plasticity: {
        ...input.gains.plasticity,
        evidenceRefs: uniqueRefs(input.gains.plasticity.evidenceRefs),
      },
      stability: {
        ...input.gains.stability,
        evidenceRefs: uniqueRefs(input.gains.stability.evidenceRefs),
      },
      generalization: {
        ...input.gains.generalization,
        evidenceRefs: uniqueRefs(input.gains.generalization.evidenceRefs),
      },
    },
  }
}

export const evaluateContinualLearningClaim = (
  claim: ContinualLearningClaim,
): AgentClClaimDiscipline => {
  const blockerRefs = uniqueRefs([
    ...(claim.agentClEval === undefined
      ? ['blocker.gym.agentcl_claim.agentcl_eval_v0_missing']
      : []),
    ...(claim.legacySingleMetricBps !== undefined
      ? ['blocker.gym.agentcl_claim.single_memory_improved_metric_refused']
      : []),
  ])

  if (claim.agentClEval === undefined) {
    return {
      ok: false,
      evalRef: null,
      blockerRefs,
      caveatRefs: [...AGENTCL_CLAIM_CAVEATS],
    }
  }

  const agentClEval = buildAgentClEval(claim.agentClEval)
  assertPublicRefs('AgentCL continual-learning claim refs', [
    claim.claimRef,
    ...(claim.evidenceRefs ?? []),
    agentClEval.evalRef,
  ])

  return {
    ok: blockerRefs.length === 0,
    evalRef: agentClEval.evalRef,
    blockerRefs,
    caveatRefs: uniqueRefs([...AGENTCL_CLAIM_CAVEATS, ...agentClEval.caveatRefs]),
  }
}
