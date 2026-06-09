import { Schema as S } from 'effect'

import {
  blueprintModuleVersionIsProduction,
  blueprintModuleVersionRequiresOperatorPromotion,
  BlueprintModuleVersion as BlueprintModuleVersionSchema,
  type BlueprintModuleVersion,
} from './module'
import {
  blueprintProgramTypeRequiresApproval,
  BlueprintProgramFamily,
  BlueprintProgramRiskClass,
  BlueprintProgramSignature as BlueprintProgramSignatureSchema,
  type BlueprintProgramSignature,
  BlueprintProgramStatus,
  BlueprintProgramType as BlueprintProgramTypeSchema,
  type BlueprintProgramType,
} from './program'
import {
  BlueprintProgramRunAuthorityBoundary,
  type BlueprintProgramRunRecord,
} from './program-run'
import {
  blueprintReleaseGateCanPromote,
  BlueprintReleaseGate as BlueprintReleaseGateSchema,
  type BlueprintReleaseGate,
} from './release-gate'

export const BlueprintProgramPromotionState = S.Literals([
  'blocked',
  'candidate',
  'deprecated',
  'draft',
  'production',
  'promotable',
  'review_pending',
  'rolled_back',
])
export type BlueprintProgramPromotionState =
  typeof BlueprintProgramPromotionState.Type

export const BlueprintProgramRegistryAudience = S.Literals(['operator'])
export type BlueprintProgramRegistryAudience =
  typeof BlueprintProgramRegistryAudience.Type

export const BlueprintProgramRegistryMethod = S.Literals(['GET'])
export type BlueprintProgramRegistryMethod =
  typeof BlueprintProgramRegistryMethod.Type

export const BlueprintProgramRegistryEntry = S.Struct({
  approvalRequired: S.Boolean,
  directMutationAllowed: S.Boolean,
  evidenceRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  family: BlueprintProgramFamily,
  id: S.String,
  moduleVersionIds: S.Array(S.String),
  programSignatureIds: S.Array(S.String),
  programTypeId: S.String,
  promotionState: BlueprintProgramPromotionState,
  receiptRefs: S.Array(S.String),
  releaseGateIds: S.Array(S.String),
  riskClass: BlueprintProgramRiskClass,
  runIds: S.Array(S.String),
  safeProjection: S.Boolean,
  status: BlueprintProgramStatus,
})
export type BlueprintProgramRegistryEntry =
  typeof BlueprintProgramRegistryEntry.Type

export const BlueprintProgramRunDetailProjection = S.Struct({
  actorRef: S.String,
  authorityBoundary: BlueprintProgramRunAuthorityBoundary,
  confidence: S.Number,
  costRef: S.String,
  createdAt: S.String,
  directMutationDisabled: S.Boolean,
  evidenceRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  id: S.String,
  latencyMs: S.Number,
  moduleVersionId: S.String,
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
  programSignatureId: S.String,
  programTypeId: S.String,
  promotionState: BlueprintProgramPromotionState,
  purposeRef: S.String,
  receiptRefs: S.Array(S.String),
  routeRef: S.String,
  safeProjection: S.Boolean,
  updatedAt: S.String,
})
export type BlueprintProgramRunDetailProjection =
  typeof BlueprintProgramRunDetailProjection.Type

export const BlueprintProgramRegistryProjection = S.Struct({
  entries: S.Array(BlueprintProgramRegistryEntry),
  moduleVersions: S.Array(BlueprintModuleVersionSchema),
  policyRef: S.String,
  programSignatures: S.Array(BlueprintProgramSignatureSchema),
  programTypes: S.Array(BlueprintProgramTypeSchema),
  releaseGates: S.Array(BlueprintReleaseGateSchema),
  runDetails: S.Array(BlueprintProgramRunDetailProjection),
  safeProjection: S.Boolean,
})
export type BlueprintProgramRegistryProjection =
  typeof BlueprintProgramRegistryProjection.Type

export const BlueprintProgramRegistryApiSeed = S.Struct({
  audience: BlueprintProgramRegistryAudience,
  method: BlueprintProgramRegistryMethod,
  path: S.String,
  projection: BlueprintProgramRegistryProjection,
  safeProjectionPolicyRef: S.String,
})
export type BlueprintProgramRegistryApiSeed =
  typeof BlueprintProgramRegistryApiSeed.Type

export type BlueprintProgramRegistryRecords = Readonly<{
  moduleVersions: ReadonlyArray<BlueprintModuleVersion>
  programSignatures: ReadonlyArray<BlueprintProgramSignature>
  programTypes: ReadonlyArray<BlueprintProgramType>
  releaseGates: ReadonlyArray<BlueprintReleaseGate>
  runs?: ReadonlyArray<BlueprintProgramRunRecord> | undefined
}>

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== null && ref !== undefined))]

const failureRefsFromRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  refs.filter(ref => ref.includes('failure') || ref.includes('error'))

export const blueprintProgramPromotionState = (
  moduleVersions: ReadonlyArray<BlueprintModuleVersion>,
  releaseGates: ReadonlyArray<BlueprintReleaseGate>,
): BlueprintProgramPromotionState => {
  if (
    moduleVersions.some(
      moduleVersion =>
        moduleVersion.status === 'deprecated' ||
        moduleVersion.releaseState === 'deprecated',
    )
  ) {
    return 'deprecated'
  }

  if (
    moduleVersions.some(
      moduleVersion =>
        moduleVersion.status === 'rolled_back' ||
        moduleVersion.releaseState === 'rolled_back',
    )
  ) {
    return 'rolled_back'
  }

  if (moduleVersions.some(blueprintModuleVersionIsProduction)) {
    return 'production'
  }

  if (releaseGates.some(blueprintReleaseGateCanPromote)) {
    return 'promotable'
  }

  if (
    releaseGates.some(
      gate =>
        gate.fixturePassState === 'blocked' ||
        gate.fixturePassState === 'failed' ||
        gate.policyState === 'blocked' ||
        gate.reviewState === 'rejected' ||
        gate.decision === 'blocked' ||
        gate.decision === 'rejected' ||
        gate.selfPromotionAttempt,
    )
  ) {
    return 'blocked'
  }

  if (
    releaseGates.some(
      gate =>
        gate.fixturePassState === 'draft' ||
        gate.policyState === 'not_checked' ||
        gate.reviewState === 'not_requested' ||
        gate.reviewState === 'pending',
    )
  ) {
    return 'review_pending'
  }

  if (moduleVersions.some(blueprintModuleVersionRequiresOperatorPromotion)) {
    return 'candidate'
  }

  return 'draft'
}

export const blueprintProgramRunDetailProjection = (
  run: BlueprintProgramRunRecord,
  records: Pick<
    BlueprintProgramRegistryRecords,
    'moduleVersions' | 'releaseGates'
  >,
): BlueprintProgramRunDetailProjection => {
  const moduleVersions = records.moduleVersions.filter(
    moduleVersion => moduleVersion.programTypeId === run.programTypeId,
  )
  const releaseGates = records.releaseGates.filter(
    gate =>
      gate.targetRef === run.programSignatureId ||
      gate.targetRef === run.moduleVersionId ||
      gate.targetRef === run.programTypeId,
  )
  const refs = uniqueRefs([...run.evidenceRefs, ...run.receiptRefs])

  return {
    actorRef: run.actorRef,
    authorityBoundary: run.authorityBoundary,
    confidence: run.confidence,
    costRef: run.costRef,
    createdAt: run.createdAt,
    directMutationDisabled: run.directMutationDisabled,
    evidenceRefs: [...run.evidenceRefs],
    failureRefs: failureRefsFromRefs(refs),
    id: run.id,
    latencyMs: run.latencyMs,
    moduleVersionId: run.moduleVersionId,
    noDeploy: run.noDeploy,
    noEmail: run.noEmail,
    noSourceMutation: run.noSourceMutation,
    noSpend: run.noSpend,
    programSignatureId: run.programSignatureId,
    programTypeId: run.programTypeId,
    promotionState: blueprintProgramPromotionState(moduleVersions, releaseGates),
    purposeRef: run.purposeRef,
    receiptRefs: [...run.receiptRefs],
    routeRef: run.routeRef,
    safeProjection: true,
    updatedAt: run.updatedAt,
  }
}

export const blueprintProgramRegistryEntryFromRecords = (
  programType: BlueprintProgramType,
  records: BlueprintProgramRegistryRecords,
): BlueprintProgramRegistryEntry => {
  const programSignatures = records.programSignatures.filter(
    signature => signature.programTypeId === programType.id,
  )
  const moduleVersions = records.moduleVersions.filter(
    moduleVersion => moduleVersion.programTypeId === programType.id,
  )
  const signatureIds = programSignatures.map(signature => signature.id)
  const moduleVersionIds = moduleVersions.map(moduleVersion => moduleVersion.id)
  const releaseGates = records.releaseGates.filter(
    gate =>
      gate.targetRef === programType.id ||
      signatureIds.includes(gate.targetRef) ||
      moduleVersionIds.includes(gate.targetRef),
  )
  const runs = (records.runs ?? []).filter(
    run => run.programTypeId === programType.id,
  )
  const evidenceRefs = uniqueRefs([
    ...programType.evidenceRequirements.map(
      requirement => requirement.descriptionRef,
    ),
    ...moduleVersions.flatMap(
      moduleVersion => moduleVersion.provenance.trainingDataRefs,
    ),
    ...releaseGates.flatMap(gate => gate.fixtureRefs),
    ...releaseGates.flatMap(gate => gate.receiptRefs),
    ...runs.flatMap(run => run.evidenceRefs),
  ])
  const receiptRefs = uniqueRefs([
    ...programType.receiptRequirements.map(
      requirement => requirement.receiptRef,
    ),
    ...releaseGates.flatMap(gate => gate.receiptRefs),
    ...runs.flatMap(run => run.receiptRefs),
  ])
  const failureRefs = uniqueRefs([
    ...moduleVersions.flatMap(
      moduleVersion => moduleVersion.provenance.retainedFailureRefs,
    ),
    ...failureRefsFromRefs(evidenceRefs),
    ...failureRefsFromRefs(receiptRefs),
  ])

  return {
    approvalRequired: blueprintProgramTypeRequiresApproval(programType),
    directMutationAllowed: programType.directMutationAllowed,
    evidenceRefs,
    failureRefs,
    family: programType.family,
    id: `program_registry_entry.${programType.id}`,
    moduleVersionIds,
    programSignatureIds: signatureIds,
    programTypeId: programType.id,
    promotionState: blueprintProgramPromotionState(
      moduleVersions,
      releaseGates,
    ),
    receiptRefs,
    releaseGateIds: releaseGates.map(gate => gate.id),
    riskClass: programType.riskClass,
    runIds: runs.map(run => run.id),
    safeProjection: true,
    status: programType.status,
  }
}

export const blueprintProgramRegistryProjection = (
  records: BlueprintProgramRegistryRecords,
): BlueprintProgramRegistryProjection => ({
  entries: records.programTypes.map(programType =>
    blueprintProgramRegistryEntryFromRecords(programType, records),
  ),
  moduleVersions: [...records.moduleVersions],
  policyRef: 'policy.blueprint.operator_safe_registry_projection.v1',
  programSignatures: [...records.programSignatures],
  programTypes: [...records.programTypes],
  releaseGates: [...records.releaseGates],
  runDetails: [...(records.runs ?? [])].map(run =>
    blueprintProgramRunDetailProjection(run, records),
  ),
  safeProjection: true,
})

export const blueprintProgramRegistryProjectionIsSafe = (
  projection: BlueprintProgramRegistryProjection,
): boolean =>
  projection.safeProjection &&
  projection.entries.every(entry => entry.safeProjection) &&
  projection.runDetails.every(
    run =>
      run.safeProjection &&
      run.authorityBoundary === 'evidence_only' &&
      run.directMutationDisabled &&
      run.noDeploy &&
      run.noEmail &&
      run.noSourceMutation &&
      run.noSpend,
  )
