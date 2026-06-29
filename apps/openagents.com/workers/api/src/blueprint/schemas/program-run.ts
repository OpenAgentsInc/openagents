import { Schema as S } from 'effect'

export const BlueprintProgramRunAuthorityBoundary = S.Literals([
  'evidence_only',
])
export type BlueprintProgramRunAuthorityBoundary =
  typeof BlueprintProgramRunAuthorityBoundary.Type

export const BlueprintProgramRunRecord = S.Struct({
  actorRef: S.String,
  archivedAt: S.NullOr(S.String),
  authorityBoundary: BlueprintProgramRunAuthorityBoundary,
  confidence: S.Number,
  costRef: S.String,
  createdAt: S.String,
  directMutationDisabled: S.Boolean,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  idempotencyKey: S.String,
  inputSnapshotHash: S.String,
  latencyMs: S.Number,
  metadata: S.Record(S.String, S.Unknown),
  moduleVersionId: S.String,
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
  programSignatureId: S.String,
  programTypeId: S.String,
  purposeRef: S.String,
  receiptRefs: S.Array(S.String),
  routeRef: S.String,
  typedOutput: S.Record(S.String, S.Unknown),
  updatedAt: S.String,
})
export type BlueprintProgramRunRecord =
  typeof BlueprintProgramRunRecord.Type

export const blueprintProgramRunHasWriteAuthority = (
  run: BlueprintProgramRunRecord,
): boolean =>
  run.authorityBoundary !== 'evidence_only' ||
  !run.directMutationDisabled ||
  !run.noDeploy ||
  !run.noEmail ||
  !run.noSpend ||
  !run.noSourceMutation

export const blueprintProgramRunIsEvidenceOnly = (
  run: BlueprintProgramRunRecord,
): boolean => !blueprintProgramRunHasWriteAuthority(run)
