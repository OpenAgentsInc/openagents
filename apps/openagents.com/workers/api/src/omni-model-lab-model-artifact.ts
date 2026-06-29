import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniModelArtifactAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniModelArtifactAudience = typeof OmniModelArtifactAudience.Type

export const OmniModelArtifactKind = S.Literals([
  'adapter',
  'base_model',
  'embedding_model',
  'eval_harness',
  'fine_tune',
  'quantized_model',
  'router_policy',
])
export type OmniModelArtifactKind = typeof OmniModelArtifactKind.Type

export const OmniModelArtifactState = S.Literals([
  'approved',
  'archived',
  'blocked',
  'draft',
  'imported',
  'retained',
  'review_ready',
  'superseded',
  'validated',
])
export type OmniModelArtifactState = typeof OmniModelArtifactState.Type

export const OmniModelArtifactReadiness = S.Literals([
  'archived',
  'blocked',
  'missing_evidence',
  'retained',
  'reviewed',
  'validation_ready',
])
export type OmniModelArtifactReadiness =
  typeof OmniModelArtifactReadiness.Type

export const OmniModelArtifactRollbackPosture = S.Literals([
  'missing',
  'candidate',
  'ready',
  'verified',
])
export type OmniModelArtifactRollbackPosture =
  typeof OmniModelArtifactRollbackPosture.Type

export const OmniModelArtifactRightsState = S.Literals([
  'internal_only',
  'open',
  'redistributable',
  'restricted',
  'unknown',
])
export type OmniModelArtifactRightsState =
  typeof OmniModelArtifactRightsState.Type

export const OmniModelArtifactStorageState = S.Literals([
  'archived',
  'digest_only',
  'internal_ref',
  'metadata_only',
  'redacted_pointer',
])
export type OmniModelArtifactStorageState =
  typeof OmniModelArtifactStorageState.Type

export const OmniModelArtifactAuthorityBoundary = S.Literals([
  'read_only_model_artifact',
])
export type OmniModelArtifactAuthorityBoundary =
  typeof OmniModelArtifactAuthorityBoundary.Type

export class OmniModelArtifactAuthority extends S.Class<OmniModelArtifactAuthority>(
  'OmniModelArtifactAuthority',
)({
  authorityBoundary: OmniModelArtifactAuthorityBoundary,
  noAdapterInstall: S.Boolean,
  noModelTrainingStart: S.Boolean,
  noPayoutMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRawWeightCopy: S.Boolean,
  noRoutingMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class OmniModelArtifactDigestRecord extends S.Class<OmniModelArtifactDigestRecord>(
  'OmniModelArtifactDigestRecord',
)({
  algorithm: S.String,
  byteCount: S.NullOr(S.Number),
  digestRef: S.String,
  evidenceRefs: S.Array(S.String),
  noRawWeightCopy: S.Boolean,
}) {}

export class OmniModelArtifactRightsRecord extends S.Class<OmniModelArtifactRightsRecord>(
  'OmniModelArtifactRightsRecord',
)({
  caveatRefs: S.Array(S.String),
  licenseRefs: S.Array(S.String),
  redistributionAllowed: S.Boolean,
  rightsState: OmniModelArtifactRightsState,
  trainingReuseAllowed: S.Boolean,
}) {}

export class OmniModelArtifactSafetyRecord extends S.Class<OmniModelArtifactSafetyRecord>(
  'OmniModelArtifactSafetyRecord',
)({
  blockedReasonRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  riskLabelRefs: S.Array(S.String),
  safetyReviewRefs: S.Array(S.String),
}) {}

export class OmniModelArtifactRollbackRecord extends S.Class<OmniModelArtifactRollbackRecord>(
  'OmniModelArtifactRollbackRecord',
)({
  priorArtifactRefs: S.Array(S.String),
  rollbackPosture: OmniModelArtifactRollbackPosture,
  rollbackRefs: S.Array(S.String),
}) {}

export class OmniModelArtifactRecord extends S.Class<OmniModelArtifactRecord>(
  'OmniModelArtifactRecord',
)({
  adapterValidationRefs: S.Array(S.String),
  artifactDigests: S.Array(OmniModelArtifactDigestRecord),
  artifactRef: S.String,
  authority: OmniModelArtifactAuthority,
  benchmarkRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evalRefs: S.Array(S.String),
  familyRef: S.String,
  id: S.String,
  kind: OmniModelArtifactKind,
  modelLabLoopRefs: S.Array(S.String),
  providerRefs: S.Array(S.String),
  promotionGateRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  rights: OmniModelArtifactRightsRecord,
  rollback: OmniModelArtifactRollbackRecord,
  safety: OmniModelArtifactSafetyRecord,
  sourceRefs: S.Array(S.String),
  state: OmniModelArtifactState,
  storageRefs: S.Array(S.String),
  storageState: OmniModelArtifactStorageState,
  trainingRunRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OmniModelArtifactProjection extends S.Class<OmniModelArtifactProjection>(
  'OmniModelArtifactProjection',
)({
  adapterInstallAllowed: S.Boolean,
  adapterValidationRefs: S.Array(S.String),
  artifactDigests: S.Array(OmniModelArtifactDigestRecord),
  artifactRef: S.String,
  audience: OmniModelArtifactAudience,
  authority: OmniModelArtifactAuthority,
  benchmarkRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  digestCount: S.Number,
  evalRefs: S.Array(S.String),
  familyRef: S.String,
  id: S.String,
  kind: OmniModelArtifactKind,
  modelLabLoopRefs: S.Array(S.String),
  modelTrainingStartAllowed: S.Boolean,
  payoutMutationAllowed: S.Boolean,
  providerRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  rawWeightCopyAllowed: S.Boolean,
  readiness: OmniModelArtifactReadiness,
  readinessLabel: S.String,
  redistributionAllowed: S.Boolean,
  retainedFailureRefs: S.Array(S.String),
  rights: OmniModelArtifactRightsRecord,
  rollback: OmniModelArtifactRollbackRecord,
  routingMutationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  safety: OmniModelArtifactSafetyRecord,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  state: OmniModelArtifactState,
  stateLabel: S.String,
  storageRefs: S.Array(S.String),
  storageState: OmniModelArtifactStorageState,
  trainingReuseAllowed: S.Boolean,
  trainingRunRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OmniModelArtifactUnsafe extends S.TaggedErrorClass<OmniModelArtifactUnsafe>()(
  'OmniModelArtifactUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_MODEL_ARTIFACT_READ_ONLY_AUTHORITY:
  OmniModelArtifactAuthority = {
    authorityBoundary: 'read_only_model_artifact',
    noAdapterInstall: true,
    noModelTrainingStart: true,
    noPayoutMutation: true,
    noPublicClaimUpgrade: true,
    noRawWeightCopy: true,
    noRoutingMutation: true,
    noRuntimePromotion: true,
    noSettlementMutation: true,
  }

const stateLabelByState: Readonly<Record<OmniModelArtifactState, string>> = {
  approved: 'Approved for reviewed use',
  archived: 'Archived',
  blocked: 'Blocked',
  draft: 'Draft',
  imported: 'Imported evidence',
  retained: 'Retained for Model Lab',
  review_ready: 'Ready for review',
  superseded: 'Superseded',
  validated: 'Validated evidence',
}

const readinessLabelByReadiness:
  Readonly<Record<OmniModelArtifactReadiness, string>> = {
    archived: 'Archived',
    blocked: 'Blocked',
    missing_evidence: 'Missing evidence',
    retained: 'Retained',
    reviewed: 'Reviewed evidence',
    validation_ready: 'Validation ready',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeModelArtifactRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.(raw|private)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(adapter_validation\.private|artifact\.private|benchmark\.private|caveat\.private|digest\.private|eval\.private|family\.private|loop\.private|model\.private|provider\.|promotion_gate\.private|retained_failure\.private|rollback\.private|safety\.private|source\.|storage\.|training_run\.private)/i
const agentUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|digest\.private|eval\.private|model\.private|provider\.private|promotion_gate\.private|retained_failure\.private|rollback\.private|safety\.private|source\.private|storage\.private|training_run\.private)/i
const customerUnsafeRefPattern =
  /(artifact\.private|benchmark\.private|digest\.private|eval\.private|model\.private|provider\.private|promotion_gate\.private|retained_failure\.private|rollback\.private|safety\.private|source\.private|storage\.private|training_run\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeModelArtifactRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniModelArtifactUnsafe({
      reason: `${label} contains raw weights, private provider payloads, prompts, datasets, secrets, wallet/payment material, private repositories, raw logs, source archives, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniModelArtifactAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'agent') {
    return agentUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniModelArtifactAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const refForAudience = (
  label: string,
  ref: string,
  audience: OmniModelArtifactAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniModelArtifactAuthority,
): void => {
  if (
    authority.noAdapterInstall !== true ||
    authority.noModelTrainingStart !== true ||
    authority.noPayoutMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRawWeightCopy !== true ||
    authority.noRoutingMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new OmniModelArtifactUnsafe({
      reason:
        'Model artifacts are read-only evidence and cannot start training, install adapters, promote runtime behavior, mutate routes, copy raw weights, mutate payouts, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniModelArtifactUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertDigest = (digest: OmniModelArtifactDigestRecord): void => {
  assertSafeRefs('Artifact digest ref', [digest.digestRef])
  assertSafeRefs('Artifact digest evidence refs', digest.evidenceRefs)
  assertSafeRefs('Artifact digest algorithm', [digest.algorithm])

  if (digest.noRawWeightCopy !== true) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Artifact digests must explicitly deny raw weight copy.',
    })
  }

  if (!hasAny(digest.evidenceRefs)) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Artifact digests require evidence refs.',
    })
  }

  if (digest.byteCount !== null && digest.byteCount < 0) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Artifact digest byte count cannot be negative.',
    })
  }
}

const assertRights = (rights: OmniModelArtifactRightsRecord): void => {
  assertSafeRefs('Artifact rights caveat refs', rights.caveatRefs)
  assertSafeRefs('Artifact rights license refs', rights.licenseRefs)

  if (
    rights.redistributionAllowed &&
    rights.rightsState !== 'open' &&
    rights.rightsState !== 'redistributable'
  ) {
    throw new OmniModelArtifactUnsafe({
      reason:
        'Redistribution requires open or redistributable rights state.',
    })
  }

  if (rights.redistributionAllowed && !hasAny(rights.licenseRefs)) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Redistributable artifacts require license refs.',
    })
  }

  if (rights.rightsState === 'unknown' && rights.trainingReuseAllowed) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Unknown rights cannot allow training reuse.',
    })
  }
}

const assertSafety = (safety: OmniModelArtifactSafetyRecord): void => {
  assertSafeRefs('Artifact safety blocked reason refs', safety.blockedReasonRefs)
  assertSafeRefs('Artifact safety redaction policy refs', safety.redactionPolicyRefs)
  assertSafeRefs('Artifact safety risk label refs', safety.riskLabelRefs)
  assertSafeRefs('Artifact safety review refs', safety.safetyReviewRefs)

  if (!hasAny(safety.redactionPolicyRefs)) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Model artifacts require redaction policy refs.',
    })
  }
}

const assertRollback = (
  rollback: OmniModelArtifactRollbackRecord,
  state: OmniModelArtifactState,
): void => {
  assertSafeRefs('Artifact rollback refs', rollback.rollbackRefs)
  assertSafeRefs('Artifact prior artifact refs', rollback.priorArtifactRefs)

  if (
    state === 'approved' &&
    rollback.rollbackPosture !== 'ready' &&
    rollback.rollbackPosture !== 'verified'
  ) {
    throw new OmniModelArtifactUnsafe({
      reason:
        'Approved model artifacts require ready or verified rollback posture.',
    })
  }

  if (
    state === 'approved' &&
    (!hasAny(rollback.rollbackRefs) || !hasAny(rollback.priorArtifactRefs))
  ) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Approved model artifacts require rollback and prior artifact refs.',
    })
  }
}

const assertRecord = (record: OmniModelArtifactRecord): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('createdAtIso', record.createdAtIso)
  assertValidIso('updatedAtIso', record.updatedAtIso)

  assertSafeRefs('Artifact id', [record.id])
  assertSafeRefs('Artifact ref', [record.artifactRef])
  assertSafeRefs('Artifact family ref', [record.familyRef])
  assertSafeRefs('Artifact adapter validation refs', record.adapterValidationRefs)
  assertSafeRefs('Artifact benchmark refs', record.benchmarkRefs)
  assertSafeRefs('Artifact caveat refs', record.caveatRefs)
  assertSafeRefs('Artifact eval refs', record.evalRefs)
  assertSafeRefs('Artifact loop refs', record.modelLabLoopRefs)
  assertSafeRefs('Artifact provider refs', record.providerRefs)
  assertSafeRefs('Artifact promotion gate refs', record.promotionGateRefs)
  assertSafeRefs('Artifact retained failure refs', record.retainedFailureRefs)
  assertSafeRefs('Artifact source refs', record.sourceRefs)
  assertSafeRefs('Artifact storage refs', record.storageRefs)
  assertSafeRefs('Artifact training run refs', record.trainingRunRefs)

  record.artifactDigests.forEach(assertDigest)
  assertRights(record.rights)
  assertSafety(record.safety)
  assertRollback(record.rollback, record.state)

  if (!hasAny(record.artifactDigests)) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Model artifacts require at least one digest record.',
    })
  }

  if (!hasAny(record.sourceRefs)) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Model artifacts require source refs.',
    })
  }

  if (record.state === 'blocked' && !hasAny(record.safety.blockedReasonRefs)) {
    throw new OmniModelArtifactUnsafe({
      reason: 'Blocked model artifacts require blocked reason refs.',
    })
  }

  if (
    (record.state === 'validated' ||
      record.state === 'review_ready' ||
      record.state === 'approved') &&
    (!hasAny(record.evalRefs) || !hasAny(record.safety.safetyReviewRefs))
  ) {
    throw new OmniModelArtifactUnsafe({
      reason:
        'Validated, review-ready, and approved model artifacts require eval refs and safety review refs.',
    })
  }

  if (
    record.state === 'approved' &&
    (!hasAny(record.promotionGateRefs) || !hasAny(record.benchmarkRefs))
  ) {
    throw new OmniModelArtifactUnsafe({
      reason:
        'Approved model artifacts require promotion gate and benchmark refs.',
    })
  }
}

const readinessForRecord = (
  record: OmniModelArtifactRecord,
): OmniModelArtifactReadiness => {
  if (record.state === 'archived' || record.state === 'superseded') {
    return 'archived'
  }

  if (record.state === 'blocked') {
    return 'blocked'
  }

  if (
    !hasAny(record.artifactDigests) ||
    !hasAny(record.sourceRefs) ||
    !hasAny(record.safety.redactionPolicyRefs)
  ) {
    return 'missing_evidence'
  }

  if (record.state === 'approved' || record.state === 'review_ready') {
    return 'reviewed'
  }

  if (record.state === 'validated') {
    return 'validation_ready'
  }

  return 'retained'
}

const redactDigest = (
  digest: OmniModelArtifactDigestRecord,
  audience: OmniModelArtifactAudience,
): OmniModelArtifactDigestRecord => ({
  ...digest,
  digestRef: refForAudience(
    'Artifact digest ref',
    digest.digestRef,
    audience,
    'digest.redacted.model_artifact',
  ),
  evidenceRefs: refsForAudience(
    'Artifact digest evidence refs',
    digest.evidenceRefs,
    audience,
  ),
})

const rightsForAudience = (
  rights: OmniModelArtifactRightsRecord,
  audience: OmniModelArtifactAudience,
): OmniModelArtifactRightsRecord => ({
  ...rights,
  caveatRefs: refsForAudience(
    'Artifact rights caveat refs',
    rights.caveatRefs,
    audience,
  ),
  licenseRefs: refsForAudience(
    'Artifact rights license refs',
    rights.licenseRefs,
    audience,
  ),
})

const safetyForAudience = (
  safety: OmniModelArtifactSafetyRecord,
  audience: OmniModelArtifactAudience,
): OmniModelArtifactSafetyRecord => ({
  ...safety,
  blockedReasonRefs: refsForAudience(
    'Artifact safety blocked reason refs',
    safety.blockedReasonRefs,
    audience,
  ),
  redactionPolicyRefs: refsForAudience(
    'Artifact safety redaction policy refs',
    safety.redactionPolicyRefs,
    audience,
  ),
  riskLabelRefs: refsForAudience(
    'Artifact safety risk label refs',
    safety.riskLabelRefs,
    audience,
  ),
  safetyReviewRefs: refsForAudience(
    'Artifact safety review refs',
    safety.safetyReviewRefs,
    audience,
  ),
})

const rollbackForAudience = (
  rollback: OmniModelArtifactRollbackRecord,
  audience: OmniModelArtifactAudience,
): OmniModelArtifactRollbackRecord => ({
  ...rollback,
  priorArtifactRefs: refsForAudience(
    'Artifact prior artifact refs',
    rollback.priorArtifactRefs,
    audience,
  ),
  rollbackRefs: refsForAudience(
    'Artifact rollback refs',
    rollback.rollbackRefs,
    audience,
  ),
})

export const omniModelArtifactProjectionHasPrivateMaterial = (
  projection: OmniModelArtifactProjection,
): boolean =>
  unsafeModelArtifactRefPattern.test(JSON.stringify(projection)) ||
  rawTimestampPattern.test(JSON.stringify(projection))

export const projectOmniModelArtifact = (
  record: OmniModelArtifactRecord,
  audience: OmniModelArtifactAudience,
  nowIso: string,
): OmniModelArtifactProjection => {
  assertRecord(record)
  assertValidIso('nowIso', nowIso)

  const readiness = readinessForRecord(record)
  const projection: OmniModelArtifactProjection = {
    adapterInstallAllowed: false,
    adapterValidationRefs: refsForAudience(
      'Artifact adapter validation refs',
      record.adapterValidationRefs,
      audience,
    ),
    artifactDigests: record.artifactDigests.map(digest =>
      redactDigest(digest, audience),
    ),
    artifactRef: refForAudience(
      'Artifact ref',
      record.artifactRef,
      audience,
      'artifact.redacted.model_artifact',
    ),
    audience,
    authority: record.authority,
    benchmarkRefs: refsForAudience(
      'Artifact benchmark refs',
      record.benchmarkRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artifact caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    digestCount: record.artifactDigests.length,
    evalRefs: refsForAudience('Artifact eval refs', record.evalRefs, audience),
    familyRef: refForAudience(
      'Artifact family ref',
      record.familyRef,
      audience,
      'family.redacted.model_artifact',
    ),
    id: refForAudience('Artifact id', record.id, audience, 'artifact.redacted'),
    kind: record.kind,
    modelLabLoopRefs: refsForAudience(
      'Artifact loop refs',
      record.modelLabLoopRefs,
      audience,
    ),
    modelTrainingStartAllowed: false,
    payoutMutationAllowed: false,
    providerRefs: refsForAudience(
      'Artifact provider refs',
      record.providerRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    rawWeightCopyAllowed: false,
    readiness,
    readinessLabel: readinessLabelByReadiness[readiness],
    redistributionAllowed: record.rights.redistributionAllowed,
    retainedFailureRefs: refsForAudience(
      'Artifact retained failure refs',
      record.retainedFailureRefs,
      audience,
    ),
    rights: rightsForAudience(record.rights, audience),
    rollback: rollbackForAudience(record.rollback, audience),
    routingMutationAllowed: false,
    runtimePromotionAllowed: false,
    safety: safetyForAudience(record.safety, audience),
    settlementMutationAllowed: false,
    sourceRefs: refsForAudience(
      'Artifact source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    storageRefs: refsForAudience(
      'Artifact storage refs',
      record.storageRefs,
      audience,
    ),
    storageState: record.storageState,
    trainingReuseAllowed: record.rights.trainingReuseAllowed,
    trainingRunRefs: refsForAudience(
      'Artifact training run refs',
      record.trainingRunRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (omniModelArtifactProjectionHasPrivateMaterial(projection)) {
    throw new OmniModelArtifactUnsafe({
      reason:
        'Model artifact projection contains private model, provider, source, dataset, prompt, payment, wallet, or raw timestamp material.',
    })
  }

  return projection
}

export const exampleOmniModelArtifact = (): OmniModelArtifactRecord => ({
  adapterValidationRefs: ['adapter_validation.public.otect_safety_adapter'],
  artifactDigests: [
    {
      algorithm: 'sha256',
      byteCount: 428112,
      digestRef: 'digest.public.otect_adapter_sha256',
      evidenceRefs: ['evidence.public.digest_manifest'],
      noRawWeightCopy: true,
    },
  ],
  artifactRef: 'artifact.public.otect_layout_adapter_v1',
  authority: OMNI_MODEL_ARTIFACT_READ_ONLY_AUTHORITY,
  benchmarkRefs: ['benchmark.public.otect_revision_suite'],
  caveatRefs: ['caveat.public.model_lab_not_deployed'],
  createdAtIso: '2026-06-06T23:00:00.000Z',
  evalRefs: ['eval.public.otect_revision_regression_pass'],
  familyRef: 'family.public.site_design_adapter',
  id: 'model_artifact.public.otect_layout_adapter_v1',
  kind: 'adapter',
  modelLabLoopRefs: ['loop.public.otect_retained_failure_loop'],
  providerRefs: ['provider.public.psionic_lab'],
  promotionGateRefs: ['promotion_gate.public.otect_adapter_review'],
  retainedFailureRefs: ['retained_failure.public.otect_revision_images'],
  rights: {
    caveatRefs: ['caveat.public.internal_review_only'],
    licenseRefs: ['license.public.openagents_internal_artifact'],
    redistributionAllowed: false,
    rightsState: 'internal_only',
    trainingReuseAllowed: true,
  },
  rollback: {
    priorArtifactRefs: ['artifact.public.otect_layout_adapter_previous'],
    rollbackPosture: 'ready',
    rollbackRefs: ['rollback.public.otect_adapter_restore'],
  },
  safety: {
    blockedReasonRefs: [],
    redactionPolicyRefs: ['redaction.public.model_artifact_refs_only'],
    riskLabelRefs: ['risk.public.low_public_projection'],
    safetyReviewRefs: ['safety.public.operator_reviewed'],
  },
  sourceRefs: ['source.public.otect_revision_brief'],
  state: 'approved',
  storageRefs: ['storage.public.digest_manifest_only'],
  storageState: 'digest_only',
  trainingRunRefs: ['training_run.public.otect_adapter_tune'],
  updatedAtIso: '2026-06-06T23:25:00.000Z',
})
