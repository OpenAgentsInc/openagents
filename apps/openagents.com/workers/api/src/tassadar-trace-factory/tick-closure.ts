/**
 * Tick closure v0.1 — the factory's acceptance predicate (issue #4748).
 *
 * A factory work unit counts only when intent, execution, state delta,
 * and evaluation ALL close (agent Kenobi's tetrahedron criterion,
 * independently re-derived by the external analysis). Closed ticks ARE
 * training records: the distillation dataset is the byproduct of
 * operation, not a separate pipeline. A tick whose evaluation rejected
 * the work is still closed — closure is about the loop completing, not
 * about the answer being yes — but only a closed, verified,
 * corpus-admitted tick may mint a training record reference.
 */
import type { TassadarValidationTier } from './trace-record'

export const TASSADAR_TICK_CLOSURE_CONTRACT_VERSION = 'tick_closure.v0.1'

export type TassadarTickIntent = Readonly<{
  assignmentRef: string
  familyId: string
  inputSeed: string
  declaredStepCount: number
}>

export type TassadarTickExecution = Readonly<{
  fullTraceDigest: string
  executorHash: string
  stepCount: number
}>

export type TassadarTickStateDelta = Readonly<{
  recordId: string
  admittedTo: 'corpus' | 'quarantine'
  tokenCount: number
}>

export type TassadarTickEvaluation = Readonly<{
  verdictRef: string
  outcome: 'verified' | 'rejected'
  tier: TassadarValidationTier
}>

export type TassadarTickFaces = Readonly<{
  intent: TassadarTickIntent | null
  execution: TassadarTickExecution | null
  stateDelta: TassadarTickStateDelta | null
  evaluation: TassadarTickEvaluation | null
}>

export type TassadarTickFaceName =
  | 'intent'
  | 'execution'
  | 'state_delta'
  | 'evaluation'

export type TassadarClosedTick = Readonly<{
  contractVersion: typeof TASSADAR_TICK_CLOSURE_CONTRACT_VERSION
  intent: TassadarTickIntent
  execution: TassadarTickExecution
  stateDelta: TassadarTickStateDelta
  evaluation: TassadarTickEvaluation
}>

export type TassadarTickClosureResult =
  | Readonly<{ closed: true; tick: TassadarClosedTick }>
  | Readonly<{ closed: false; openFaces: ReadonlyArray<TassadarTickFaceName> }>

/** The tetrahedron predicate: all four faces or no tick. */
export const closeTick = (faces: TassadarTickFaces): TassadarTickClosureResult => {
  const openFaces: Array<TassadarTickFaceName> = []
  if (faces.intent === null) openFaces.push('intent')
  if (faces.execution === null) openFaces.push('execution')
  if (faces.stateDelta === null) openFaces.push('state_delta')
  if (faces.evaluation === null) openFaces.push('evaluation')
  if (
    faces.intent === null ||
    faces.execution === null ||
    faces.stateDelta === null ||
    faces.evaluation === null
  ) {
    return { closed: false, openFaces }
  }

  return {
    closed: true,
    tick: {
      contractVersion: TASSADAR_TICK_CLOSURE_CONTRACT_VERSION,
      evaluation: faces.evaluation,
      execution: faces.execution,
      intent: faces.intent,
      stateDelta: faces.stateDelta,
    },
  }
}

export type TassadarTrainingRecordRefResult =
  | Readonly<{ ok: true; trainingRecordRef: string }>
  | Readonly<{
      ok: false
      reason: 'evaluation_rejected' | 'not_admitted_to_corpus'
    }>

/**
 * Closed ticks are training records — but only when the evaluation
 * verified the work and the state delta admitted it to the corpus.
 * Everything else is an operational receipt, never training data.
 */
export const trainingRecordRefFromClosedTick = (
  tick: TassadarClosedTick,
): TassadarTrainingRecordRefResult => {
  if (tick.evaluation.outcome !== 'verified') {
    return { ok: false, reason: 'evaluation_rejected' }
  }
  if (tick.stateDelta.admittedTo !== 'corpus') {
    return { ok: false, reason: 'not_admitted_to_corpus' }
  }

  return {
    ok: true,
    trainingRecordRef: `training_record.${tick.stateDelta.recordId}.${tick.evaluation.verdictRef}`,
  }
}
