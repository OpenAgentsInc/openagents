import { Schema as S } from 'effect'

export const BlueprintContinuationReleaseTargetKind = S.Literals([
  'module_version',
  'program_signature',
])
export type BlueprintContinuationReleaseTargetKind =
  typeof BlueprintContinuationReleaseTargetKind.Type

export const BlueprintContinuationReleaseGateResult = S.Struct({
  canPromote: S.Boolean,
  failureRefs: S.Array(S.String),
  gateRef: S.String,
  requiredFixtureRefs: S.Array(S.String),
  requiredReceiptRefs: S.Array(S.String),
  targetKind: BlueprintContinuationReleaseTargetKind,
  targetRef: S.String,
})
export type BlueprintContinuationReleaseGateResult =
  typeof BlueprintContinuationReleaseGateResult.Type
