import {
  type BlueprintModuleVersion,
  blueprintModuleVersionCanSelfPromote,
  blueprintModuleVersionIsProduction,
  blueprintModuleVersionReleaseStateIsValid,
  blueprintModuleVersionRequiresOperatorPromotion,
} from '../schemas/module'
import type { BlueprintProgramSignature } from '../schemas/program'
import type {
  BlueprintContinuationReleaseGateResult,
  BlueprintContinuationReleaseTargetKind,
} from '../schemas/continuation-release-gate'
import {
  type BlueprintReleaseGate,
  blueprintReleaseGateCanPromote,
  blueprintReleaseGatePreservesRollbackEvidence,
} from '../schemas/release-gate'

export type BlueprintContinuationReleaseGateTarget =
  | Readonly<{
      _tag: 'ModuleVersion'
      moduleVersion: BlueprintModuleVersion
    }>
  | Readonly<{
      _tag: 'ProgramSignature'
      programSignature: BlueprintProgramSignature
    }>

export type EvaluateBlueprintContinuationReleaseGateInput = Readonly<{
  gate: BlueprintReleaseGate
  target: BlueprintContinuationReleaseGateTarget
}>

const targetKind = (
  target: BlueprintContinuationReleaseGateTarget,
): BlueprintContinuationReleaseTargetKind =>
  target._tag === 'ModuleVersion' ? 'module_version' : 'program_signature'

const targetRef = (
  target: BlueprintContinuationReleaseGateTarget,
): string =>
  target._tag === 'ModuleVersion'
    ? target.moduleVersion.id
    : target.programSignature.id

const targetIsContinuation = (
  target: BlueprintContinuationReleaseGateTarget,
): boolean => {
  if (target._tag === 'ProgramSignature') {
    return target.programSignature.supportsContinuation
  }

  return (
    target.moduleVersion.programSignatureId !== null &&
    target.moduleVersion.programSignatureId.startsWith(
      'program_signature.autopilot.',
    ) &&
    target.moduleVersion.programTypeId.startsWith('program_type.autopilot.')
  )
}

const targetFailureRefs = (
  target: BlueprintContinuationReleaseGateTarget,
): ReadonlyArray<string> => {
  if (!targetIsContinuation(target)) {
    return ['failure.continuation_release.target_not_continuation']
  }

  if (target._tag === 'ProgramSignature') {
    return []
  }

  const failures: string[] = []

  if (!blueprintModuleVersionReleaseStateIsValid(target.moduleVersion)) {
    failures.push('failure.continuation_release.module_state_invalid')
  }

  if (blueprintModuleVersionCanSelfPromote(target.moduleVersion)) {
    failures.push('failure.continuation_release.module_self_promotes')
  }

  if (blueprintModuleVersionIsProduction(target.moduleVersion)) {
    failures.push('failure.continuation_release.module_already_production')
  }

  if (!blueprintModuleVersionRequiresOperatorPromotion(target.moduleVersion)) {
    failures.push('failure.continuation_release.module_not_candidate')
  }

  return failures
}

const gateFailureRefs = (
  gate: BlueprintReleaseGate,
  target: BlueprintContinuationReleaseGateTarget,
): ReadonlyArray<string> => {
  const failures: string[] = []

  if (gate.targetKind !== targetKind(target)) {
    failures.push('failure.continuation_release.target_kind_mismatch')
  }

  if (gate.targetRef !== targetRef(target)) {
    failures.push('failure.continuation_release.target_ref_mismatch')
  }

  if (gate.fixtureRefs.length === 0) {
    failures.push('failure.continuation_release.fixture_refs_missing')
  }

  if (gate.fixturePassState !== 'passed') {
    failures.push('failure.continuation_release.fixtures_not_passed')
  }

  if (gate.policyState !== 'compliant') {
    failures.push('failure.continuation_release.policy_not_compliant')
  }

  if (gate.reviewState !== 'approved') {
    failures.push('failure.continuation_release.review_not_approved')
  }

  if (
    gate.decision !== 'approved' ||
    gate.decidedByRef === null ||
    !gate.decidedByRef.startsWith('operator.') ||
    gate.decisionReasonRef === null
  ) {
    failures.push('failure.continuation_release.operator_decision_missing')
  }

  if (gate.scorecardRef === null) {
    failures.push('failure.continuation_release.scorecard_missing')
  }

  if (gate.receiptRefs.length === 0) {
    failures.push('failure.continuation_release.receipts_missing')
  }

  if (!blueprintReleaseGatePreservesRollbackEvidence(gate)) {
    failures.push('failure.continuation_release.rollback_anchor_missing')
  }

  if (gate.selfPromotionAttempt) {
    failures.push('failure.continuation_release.self_promotion_attempt')
  }

  if (!blueprintReleaseGateCanPromote(gate)) {
    failures.push('failure.continuation_release.generic_gate_not_promotable')
  }

  return [...new Set(failures)]
}

export const evaluateBlueprintContinuationReleaseGate = (
  input: EvaluateBlueprintContinuationReleaseGateInput,
): BlueprintContinuationReleaseGateResult => {
  const failures = [
    ...targetFailureRefs(input.target),
    ...gateFailureRefs(input.gate, input.target),
  ]

  return {
    canPromote: failures.length === 0,
    failureRefs: failures,
    gateRef: input.gate.id,
    requiredFixtureRefs: input.gate.fixtureRefs,
    requiredReceiptRefs: input.gate.receiptRefs,
    targetKind: targetKind(input.target),
    targetRef: targetRef(input.target),
  }
}
