import { BlueprintAssignmentScope } from '@openagentsinc/sync-schema'
import { Schema as S } from 'effect'

import {
  BLUEPRINT_CONTRACT_EXPORT_SEED,
  BlueprintContractExportSeed,
  blueprintContractExportSeedHasCatalogs,
  blueprintContractExportSeedIsPrivateDataSafe,
} from './blueprint/exports/contract-export'
import {
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY,
  AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
} from './blueprint/fixtures/program-registry'
import {
  BlueprintProgramRegistryProjection,
  blueprintProgramRegistryProjectionIsSafe,
} from './blueprint/schemas/program-registry'

export const DEFAULT_PROBE_BLUEPRINT_PROGRAM_TYPE_ID =
  'program_type.autopilot.continue'
export const PROBE_BLUEPRINT_ACTION_SUBMISSION_POLICY_REF =
  'policy.blueprint.action_submission.proposals_only.v1'

export class ProbeBlueprintAssignmentScopeUnsafe extends S.TaggedErrorClass<ProbeBlueprintAssignmentScopeUnsafe>()(
  'ProbeBlueprintAssignmentScopeUnsafe',
  {
    reason: S.String,
  },
) {}

export const ProbeBlueprintRunnerCapabilitySupport = S.Struct({
  backendCapabilityRefs: S.Array(S.String),
  moduleVersionRefs: S.Array(S.String),
  programSignatureRefs: S.Array(S.String),
  programTypeRefs: S.Array(S.String),
  registryVersionRefs: S.Array(S.String),
  safeProjection: S.Boolean,
  toolRefs: S.Array(S.String),
})
export type ProbeBlueprintRunnerCapabilitySupport =
  typeof ProbeBlueprintRunnerCapabilitySupport.Type

const unsafeProbeBlueprintScopePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?(token|url)|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|log[_-]?line|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo|source)|provider[_-]?(grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|tool[_-]?log|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|tool[_-]?log|wallet)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const prefixed = (
  ref: string,
  prefixes: ReadonlyArray<string>,
): boolean => prefixes.some(prefix => ref.startsWith(prefix))

const assertRefPrefixes = (
  refs: ReadonlyArray<string>,
  label: string,
  prefixes: ReadonlyArray<string>,
): void => {
  const invalid = refs.find(ref => !prefixed(ref, prefixes))

  if (invalid !== undefined) {
    throw new ProbeBlueprintAssignmentScopeUnsafe({
      reason: `${label} must use one of these prefixes: ${prefixes.join(', ')}`,
    })
  }
}

const assertOptionalRefPrefixes = (
  refs: ReadonlyArray<string> | undefined,
  label: string,
  prefixes: ReadonlyArray<string>,
): void => assertRefPrefixes(refs ?? [], label, prefixes)

const assertKnownRefs = (
  refs: ReadonlyArray<string> | undefined,
  knownRefs: ReadonlySet<string>,
  label: string,
): void => {
  const missing = (refs ?? []).filter(ref => !knownRefs.has(ref))

  if (missing.length > 0) {
    throw new ProbeBlueprintAssignmentScopeUnsafe({
      reason: `${label} includes refs outside the inline Blueprint registry slice: ${missing.join(', ')}`,
    })
  }
}

const assertNoPrivateMaterial = (scope: BlueprintAssignmentScope): void => {
  if (unsafeProbeBlueprintScopePattern.test(JSON.stringify(scope))) {
    throw new ProbeBlueprintAssignmentScopeUnsafe({
      reason:
        'Probe Blueprint assignment scope contains raw prompt, callback, provider, source, wallet, customer, credential, or private-data-shaped material.',
    })
  }
}

const decodedRegistry = (
  registry: unknown,
) => S.decodeUnknownSync(BlueprintProgramRegistryProjection)(registry)

const decodedContractExport = (
  contractExport: unknown,
) => S.decodeUnknownSync(BlueprintContractExportSeed)(contractExport)

export const assertProbeBlueprintAssignmentScopeSafe = (
  scope: BlueprintAssignmentScope,
): BlueprintAssignmentScope => {
  assertNoPrivateMaterial(scope)
  assertRefPrefixes(
    [scope.registryVersionRef],
    'blueprint.registryVersionRef',
    ['blueprint_registry.'],
  )
  assertOptionalRefPrefixes(scope.programTypeRefs, 'blueprint.programTypeRefs', [
    'program_type.',
  ])
  assertOptionalRefPrefixes(
    scope.programSignatureRefs,
    'blueprint.programSignatureRefs',
    ['program_signature.'],
  )
  assertOptionalRefPrefixes(
    scope.moduleVersionRefs,
    'blueprint.moduleVersionRefs',
    ['module_version.'],
  )
  assertOptionalRefPrefixes(scope.contextPackRefs, 'blueprint.contextPackRefs', [
    'context_pack.',
  ])
  assertOptionalRefPrefixes(
    scope.sourceAuthorityRefs,
    'blueprint.sourceAuthorityRefs',
    ['source_authority.'],
  )
  assertOptionalRefPrefixes(scope.toolScopeRefs, 'blueprint.toolScopeRefs', [
    'tool.',
  ])
  assertOptionalRefPrefixes(scope.releaseGateRefs, 'blueprint.releaseGateRefs', [
    'release_gate.',
  ])
  assertOptionalRefPrefixes(
    scope.backendCapabilityRefs,
    'blueprint.backendCapabilityRefs',
    ['probe.backend.'],
  )

  if (
    scope.actionSubmissionPolicyRef !== undefined &&
    !scope.actionSubmissionPolicyRef.startsWith('policy.')
  ) {
    throw new ProbeBlueprintAssignmentScopeUnsafe({
      reason: 'blueprint.actionSubmissionPolicyRef must use policy.',
    })
  }

  if (
    scope.programRunPurposeRef !== undefined &&
    !scope.programRunPurposeRef.startsWith('purpose.')
  ) {
    throw new ProbeBlueprintAssignmentScopeUnsafe({
      reason: 'blueprint.programRunPurposeRef must use purpose.',
    })
  }

  if (scope.registry !== undefined) {
    const registry = decodedRegistry(scope.registry)

    if (!blueprintProgramRegistryProjectionIsSafe(registry)) {
      throw new ProbeBlueprintAssignmentScopeUnsafe({
        reason: 'blueprint.registry must be an operator-safe projection.',
      })
    }

    assertKnownRefs(
      scope.programTypeRefs,
      new Set(registry.programTypes.map(programType => programType.id)),
      'blueprint.programTypeRefs',
    )
    assertKnownRefs(
      scope.programSignatureRefs,
      new Set(registry.programSignatures.map(signature => signature.id)),
      'blueprint.programSignatureRefs',
    )
    assertKnownRefs(
      scope.moduleVersionRefs,
      new Set(registry.moduleVersions.map(moduleVersion => moduleVersion.id)),
      'blueprint.moduleVersionRefs',
    )
    assertKnownRefs(
      scope.releaseGateRefs,
      new Set(registry.releaseGates.map(gate => gate.id)),
      'blueprint.releaseGateRefs',
    )
  }

  if (scope.contractExport !== undefined) {
    const contractExport = decodedContractExport(scope.contractExport)

    if (
      !blueprintContractExportSeedHasCatalogs(contractExport) ||
      !blueprintContractExportSeedIsPrivateDataSafe(contractExport)
    ) {
      throw new ProbeBlueprintAssignmentScopeUnsafe({
        reason: 'blueprint.contractExport must be a private-data-safe catalog.',
      })
    }
  }

  return scope
}

export const probeBlueprintAssignmentScopeIsSafe = (
  scope: BlueprintAssignmentScope,
): boolean => {
  try {
    assertProbeBlueprintAssignmentScopeSafe(scope)

    return true
  } catch {
    return false
  }
}

export const buildProbeBlueprintAssignmentScope = (
  input: Readonly<{
    backendCapabilityRefs?: ReadonlyArray<string> | undefined
    contextPackRefs?: ReadonlyArray<string> | undefined
    includeContractExport?: boolean | undefined
    includeRegistry?: boolean | undefined
    programTypeId?: string | undefined
    sourceAuthorityRefs?: ReadonlyArray<string> | undefined
  }> = {},
): BlueprintAssignmentScope => {
  const registry = AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY
  const programTypeId =
    input.programTypeId ?? DEFAULT_PROBE_BLUEPRINT_PROGRAM_TYPE_ID
  const entry = registry.entries.find(
    candidate => candidate.programTypeId === programTypeId,
  )
  const programType = registry.programTypes.find(
    candidate => candidate.id === programTypeId,
  )

  if (entry === undefined || programType === undefined) {
    throw new ProbeBlueprintAssignmentScopeUnsafe({
      reason: `Blueprint program type is not present in the assignment registry: ${programTypeId}`,
    })
  }

  const signatures = registry.programSignatures.filter(signature =>
    entry.programSignatureIds.includes(signature.id),
  )
  const toolScopeRefs = uniqueRefs([
    ...programType.toolScopes.map(scope => scope.toolRef),
    ...signatures.flatMap(signature =>
      signature.toolScopes.map(scope => scope.toolRef),
    ),
  ])
  const scope = new BlueprintAssignmentScope({
    actionSubmissionPolicyRef: PROBE_BLUEPRINT_ACTION_SUBMISSION_POLICY_REF,
    ...(input.backendCapabilityRefs === undefined
      ? {}
      : { backendCapabilityRefs: uniqueRefs(input.backendCapabilityRefs) }),
    ...(input.contextPackRefs === undefined
      ? {}
      : { contextPackRefs: uniqueRefs(input.contextPackRefs) }),
    ...(input.includeContractExport === true
      ? { contractExport: BLUEPRINT_CONTRACT_EXPORT_SEED }
      : {}),
    moduleVersionRefs: entry.moduleVersionIds,
    programRunPurposeRef: programType.purposeRef,
    programSignatureRefs: entry.programSignatureIds,
    programTypeRefs: [programType.id],
    ...(input.includeRegistry === true ? { registry } : {}),
    registryVersionRef: AUTOPILOT_CONTINUATION_PROGRAM_REGISTRY_VERSION_REF,
    releaseGateRefs: entry.releaseGateIds,
    ...(input.sourceAuthorityRefs === undefined
      ? {}
      : { sourceAuthorityRefs: uniqueRefs(input.sourceAuthorityRefs) }),
    toolScopeRefs,
  })

  return assertProbeBlueprintAssignmentScopeSafe(scope)
}

const includesAll = (
  actual: ReadonlyArray<string>,
  expected: ReadonlyArray<string> | undefined,
): boolean =>
  (expected ?? []).every(ref => actual.includes(ref))

export const probeBlueprintCapabilitySupportCoversAssignmentScope = (
  support: ProbeBlueprintRunnerCapabilitySupport,
  scope: BlueprintAssignmentScope,
): boolean =>
  support.safeProjection &&
  support.registryVersionRefs.includes(scope.registryVersionRef) &&
  includesAll(support.programTypeRefs, scope.programTypeRefs) &&
  includesAll(support.programSignatureRefs, scope.programSignatureRefs) &&
  includesAll(support.moduleVersionRefs, scope.moduleVersionRefs) &&
  includesAll(support.toolRefs, scope.toolScopeRefs) &&
  includesAll(support.backendCapabilityRefs, scope.backendCapabilityRefs)
