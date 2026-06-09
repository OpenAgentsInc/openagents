import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniDataPackageAudience = S.Literals([
  'public',
  'team',
  'operator',
])
export type OmniDataPackageAudience = typeof OmniDataPackageAudience.Type

export const OmniDataPackageState = S.Literals([
  'draft',
  'package_ready',
  'published',
  'reviewed',
  'revoked',
])
export type OmniDataPackageState = typeof OmniDataPackageState.Type

export const OmniDataPackageRightsState = S.Literals([
  'allowed',
  'restricted',
  'review_required',
  'revoked',
])
export type OmniDataPackageRightsState =
  typeof OmniDataPackageRightsState.Type

export const OmniDataPackageRedactionState = S.Literals([
  'blocked',
  'not_needed',
  'private_only',
  'redacted',
])
export type OmniDataPackageRedactionState =
  typeof OmniDataPackageRedactionState.Type

export const OmniDataPackageDigestAlgorithm = S.Literals([
  'blake3',
  'sha256',
])
export type OmniDataPackageDigestAlgorithm =
  typeof OmniDataPackageDigestAlgorithm.Type

export const OmniDataPackageArtifactKind = S.Literals([
  'artifact',
  'dataset',
  'file',
  'receipt',
  'schema',
  'source_bundle',
  'span',
  'table',
])
export type OmniDataPackageArtifactKind =
  typeof OmniDataPackageArtifactKind.Type

export const OmniDataPackageAuthorityBoundary = S.Literals([
  'read_only_data_package_export',
])
export type OmniDataPackageAuthorityBoundary =
  typeof OmniDataPackageAuthorityBoundary.Type

export class OmniDataPackageExportAuthority extends S.Class<OmniDataPackageExportAuthority>(
  'OmniDataPackageExportAuthority',
)({
  authorityBoundary: OmniDataPackageAuthorityBoundary,
  noDownloadMutation: S.Boolean,
  noFileHostingMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noReceiptMutation: S.Boolean,
  noRightsMutation: S.Boolean,
}) {}

export class OmniDataPackageArtifactDigest extends S.Class<OmniDataPackageArtifactDigest>(
  'OmniDataPackageArtifactDigest',
)({
  artifactKind: OmniDataPackageArtifactKind,
  artifactRef: S.String,
  digestAlgorithm: OmniDataPackageDigestAlgorithm,
  digestRef: S.String,
  sizeBytes: S.Number,
}) {}

export class OmniDataPackageSchemaManifest extends S.Class<OmniDataPackageSchemaManifest>(
  'OmniDataPackageSchemaManifest',
)({
  fieldRefs: S.Array(S.String),
  schemaRef: S.String,
  sensitiveFieldRefs: S.Array(S.String),
  versionRef: S.String,
}) {}

export class OmniDataPackageRightsManifest extends S.Class<OmniDataPackageRightsManifest>(
  'OmniDataPackageRightsManifest',
)({
  allowedAudienceRefs: S.Array(S.String),
  expiryRef: S.NullOr(S.String),
  licenseRefs: S.Array(S.String),
  rightsPolicyRef: S.String,
  rightsState: OmniDataPackageRightsState,
  usageCaveatRefs: S.Array(S.String),
}) {}

export class OmniDataPackageRedactionSummary extends S.Class<OmniDataPackageRedactionSummary>(
  'OmniDataPackageRedactionSummary',
)({
  blockedReasonRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  redactionState: OmniDataPackageRedactionState,
  removedFieldRefs: S.Array(S.String),
  retainedFieldRefs: S.Array(S.String),
  reviewerRefs: S.Array(S.String),
}) {}

export class OmniDataPackageProvenanceManifest extends S.Class<OmniDataPackageProvenanceManifest>(
  'OmniDataPackageProvenanceManifest',
)({
  generationRefs: S.Array(S.String),
  reviewRefs: S.Array(S.String),
  sourceBundleRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  spanRefs: S.Array(S.String),
}) {}

export class OmniDataPackageExportRecord extends S.Class<OmniDataPackageExportRecord>(
  'OmniDataPackageExportRecord',
)({
  artifactDigests: S.Array(OmniDataPackageArtifactDigest),
  authority: OmniDataPackageExportAuthority,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  id: S.String,
  packageRef: S.String,
  provenance: OmniDataPackageProvenanceManifest,
  receiptRefs: S.Array(S.String),
  redaction: OmniDataPackageRedactionSummary,
  reviewStateRef: S.NullOr(S.String),
  rights: OmniDataPackageRightsManifest,
  schema: OmniDataPackageSchemaManifest,
  state: OmniDataPackageState,
  titleRef: S.String,
  updatedAtIso: S.String,
}) {}

export class OmniDataPackageExportProjection extends S.Class<OmniDataPackageExportProjection>(
  'OmniDataPackageExportProjection',
)({
  artifactDigests: S.Array(OmniDataPackageArtifactDigest),
  audience: OmniDataPackageAudience,
  authority: OmniDataPackageExportAuthority,
  caveatRefs: S.Array(S.String),
  downloadMutationAllowed: S.Boolean,
  fileHostingMutationAllowed: S.Boolean,
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  packageRef: S.String,
  provenance: OmniDataPackageProvenanceManifest,
  publicClaimUpgradeAllowed: S.Boolean,
  publishedClaimAllowed: S.Boolean,
  readyForSharing: S.Boolean,
  receiptMutationAllowed: S.Boolean,
  receiptRefs: S.Array(S.String),
  redaction: OmniDataPackageRedactionSummary,
  reviewStateRef: S.NullOr(S.String),
  rights: OmniDataPackageRightsManifest,
  rightsMutationAllowed: S.Boolean,
  schema: OmniDataPackageSchemaManifest,
  state: OmniDataPackageState,
  stateLabel: S.String,
  titleRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class OmniDataPackageExportUnsafe extends S.TaggedErrorClass<OmniDataPackageExportUnsafe>()(
  'OmniDataPackageExportUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_DATA_PACKAGE_EXPORT_READ_ONLY_AUTHORITY:
  OmniDataPackageExportAuthority = {
    authorityBoundary: 'read_only_data_package_export',
    noDownloadMutation: true,
    noFileHostingMutation: true,
    noLiveWalletSpend: true,
    noPublicClaimUpgrade: true,
    noReceiptMutation: true,
    noRightsMutation: true,
  }

const stateLabelByState: Readonly<Record<OmniDataPackageState, string>> = {
  draft: 'Draft',
  package_ready: 'Package ready',
  published: 'Published',
  reviewed: 'Reviewed',
  revoked: 'Revoked',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeDataPackageRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|record|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|key|source|wallet)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(archive|auth|connector|customer|email|export|file|invoice|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|transcript|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?archive|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(artifact\.private|audience\.private|caveat\.private|digest\.private|field\.private|generation\.private|license\.private|package\.private|policy\.private|provenance\.private|receipt\.private|redaction\.private|review\.private|rights\.private|schema\.private|source\.private|span\.private|title\.private)/i
const teamUnsafeRefPattern =
  /(digest\.private|license\.private|receipt\.private|rights\.private|source\.private|span\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeDataPackageRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniDataPackageExportUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, raw source archive, raw customer/provider records, private repo, secret, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniDataPackageAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniDataPackageAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const nullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: OmniDataPackageAudience,
): string | null => {
  if (ref === null) {
    return null
  }

  return refsForAudience(label, [ref], audience)[0] ?? null
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniDataPackageAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new OmniDataPackageExportUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertArtifactDigest = (
  digest: OmniDataPackageArtifactDigest,
): void => {
  assertSafeRefs('Data package artifact digest refs', [
    digest.artifactRef,
    digest.digestRef,
  ])
  assertNonNegativeInteger('artifact digest sizeBytes', digest.sizeBytes)
}

const assertRights = (rights: OmniDataPackageRightsManifest): void => {
  assertSafeRefs('Data package rights refs', [
    rights.rightsPolicyRef,
    rights.expiryRef ?? '',
    ...rights.allowedAudienceRefs,
    ...rights.licenseRefs,
    ...rights.usageCaveatRefs,
  ])

  if (rights.licenseRefs.length === 0) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Data package rights manifest requires license refs.',
    })
  }

  if (rights.usageCaveatRefs.length === 0) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Data package rights manifest requires usage caveat refs.',
    })
  }
}

const assertSchema = (schema: OmniDataPackageSchemaManifest): void => {
  assertSafeRefs('Data package schema refs', [
    schema.schemaRef,
    schema.versionRef,
    ...schema.fieldRefs,
    ...schema.sensitiveFieldRefs,
  ])

  if (schema.fieldRefs.length === 0) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Data package schema manifest requires field refs.',
    })
  }
}

const assertRedaction = (
  redaction: OmniDataPackageRedactionSummary,
): void => {
  assertSafeRefs('Data package redaction refs', [
    ...redaction.blockedReasonRefs,
    ...redaction.redactionPolicyRefs,
    ...redaction.removedFieldRefs,
    ...redaction.retainedFieldRefs,
    ...redaction.reviewerRefs,
  ])

  if (
    redaction.redactionState === 'redacted' &&
    (redaction.redactionPolicyRefs.length === 0 ||
      redaction.removedFieldRefs.length === 0)
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason:
        'Redacted data packages require redaction policy refs and removed field refs.',
    })
  }

  if (
    redaction.redactionState === 'blocked' &&
    redaction.blockedReasonRefs.length === 0
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Blocked data packages require blocked reason refs.',
    })
  }
}

const assertProvenance = (
  provenance: OmniDataPackageProvenanceManifest,
): void => {
  assertSafeRefs('Data package provenance refs', [
    ...provenance.generationRefs,
    ...provenance.reviewRefs,
    ...provenance.sourceBundleRefs,
    ...provenance.sourceRefs,
    ...provenance.spanRefs,
  ])

  if (
    provenance.sourceBundleRefs.length === 0 ||
    provenance.sourceRefs.length === 0
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Data packages require source bundle and source refs.',
    })
  }
}

const assertRecord = (record: OmniDataPackageExportRecord): void => {
  if (
    record.authority.noDownloadMutation !== true ||
    record.authority.noFileHostingMutation !== true ||
    record.authority.noLiveWalletSpend !== true ||
    record.authority.noPublicClaimUpgrade !== true ||
    record.authority.noReceiptMutation !== true ||
    record.authority.noRightsMutation !== true
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason:
        'Data package exports must remain read-only and cannot mutate downloads, file hosting, wallets, public claims, receipts, or rights.',
    })
  }

  assertSafeRefs('Data package identity refs', [
    record.id,
    record.packageRef,
    record.reviewStateRef ?? '',
    record.titleRef,
  ])
  assertSafeRefs('Data package caveat refs', record.caveatRefs)
  assertSafeRefs('Data package receipt refs', record.receiptRefs)
  record.artifactDigests.forEach(assertArtifactDigest)
  assertRights(record.rights)
  assertSchema(record.schema)
  assertRedaction(record.redaction)
  assertProvenance(record.provenance)

  if (record.artifactDigests.length === 0) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Data packages require artifact digests.',
    })
  }

  if (
    record.state === 'published' &&
    (record.receiptRefs.length === 0 ||
      record.rights.rightsState !== 'allowed' ||
      ['blocked', 'private_only'].includes(record.redaction.redactionState))
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason:
        'Published data packages require receipt refs, allowed rights, and shareable redaction state.',
    })
  }

  if (
    record.state === 'reviewed' &&
    (record.reviewStateRef === null || record.provenance.reviewRefs.length === 0)
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Reviewed data packages require review state and review refs.',
    })
  }

  if (
    record.state === 'revoked' &&
    record.rights.rightsState !== 'revoked' &&
    record.redaction.blockedReasonRefs.length === 0
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason:
        'Revoked data packages require revoked rights or blocked reason refs.',
    })
  }

  if (
    record.rights.rightsState === 'revoked' &&
    record.state !== 'revoked'
  ) {
    throw new OmniDataPackageExportUnsafe({
      reason: 'Revoked rights require revoked package state.',
    })
  }
}

const artifactProjection = (
  digest: OmniDataPackageArtifactDigest,
  audience: OmniDataPackageAudience,
): OmniDataPackageArtifactDigest | null => {
  const artifactRef = refsForAudience(
    'Data package artifact refs',
    [digest.artifactRef],
    audience,
  )[0]
  const digestRef = refsForAudience(
    'Data package digest refs',
    [digest.digestRef],
    audience,
  )[0]

  if (artifactRef === undefined || digestRef === undefined) {
    return null
  }

  return { ...digest, artifactRef, digestRef }
}

const schemaProjection = (
  schema: OmniDataPackageSchemaManifest,
  audience: OmniDataPackageAudience,
): OmniDataPackageSchemaManifest => ({
  fieldRefs: refsForAudience('Data package schema fields', schema.fieldRefs, audience),
  schemaRef: primaryRefForAudience(
    'Data package schema refs',
    schema.schemaRef,
    audience,
    'schema.redacted',
  ),
  sensitiveFieldRefs: refsForAudience(
    'Data package sensitive schema fields',
    schema.sensitiveFieldRefs,
    audience,
  ),
  versionRef: primaryRefForAudience(
    'Data package schema version refs',
    schema.versionRef,
    audience,
    'schema_version.redacted',
  ),
})

const rightsProjection = (
  rights: OmniDataPackageRightsManifest,
  audience: OmniDataPackageAudience,
): OmniDataPackageRightsManifest => ({
  allowedAudienceRefs: refsForAudience(
    'Data package allowed audience refs',
    rights.allowedAudienceRefs,
    audience,
  ),
  expiryRef: nullableRefForAudience(
    'Data package rights expiry refs',
    rights.expiryRef,
    audience,
  ),
  licenseRefs: refsForAudience('Data package license refs', rights.licenseRefs, audience),
  rightsPolicyRef: primaryRefForAudience(
    'Data package rights policy refs',
    rights.rightsPolicyRef,
    audience,
    'rights_policy.redacted',
  ),
  rightsState: rights.rightsState,
  usageCaveatRefs: refsForAudience(
    'Data package usage caveat refs',
    rights.usageCaveatRefs,
    audience,
  ),
})

const redactionProjection = (
  redaction: OmniDataPackageRedactionSummary,
  audience: OmniDataPackageAudience,
): OmniDataPackageRedactionSummary => ({
  blockedReasonRefs: refsForAudience(
    'Data package blocked reason refs',
    redaction.blockedReasonRefs,
    audience,
  ),
  redactionPolicyRefs: refsForAudience(
    'Data package redaction policy refs',
    redaction.redactionPolicyRefs,
    audience,
  ),
  redactionState: redaction.redactionState,
  removedFieldRefs: refsForAudience(
    'Data package removed field refs',
    redaction.removedFieldRefs,
    audience,
  ),
  retainedFieldRefs: refsForAudience(
    'Data package retained field refs',
    redaction.retainedFieldRefs,
    audience,
  ),
  reviewerRefs: refsForAudience(
    'Data package reviewer refs',
    redaction.reviewerRefs,
    audience,
  ),
})

const provenanceProjection = (
  provenance: OmniDataPackageProvenanceManifest,
  audience: OmniDataPackageAudience,
): OmniDataPackageProvenanceManifest => ({
  generationRefs: refsForAudience(
    'Data package generation refs',
    provenance.generationRefs,
    audience,
  ),
  reviewRefs: refsForAudience('Data package review refs', provenance.reviewRefs, audience),
  sourceBundleRefs: refsForAudience(
    'Data package source bundle refs',
    provenance.sourceBundleRefs,
    audience,
  ),
  sourceRefs: refsForAudience('Data package source refs', provenance.sourceRefs, audience),
  spanRefs: refsForAudience('Data package span refs', provenance.spanRefs, audience),
})

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(item => [...stringValues(item)])
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => [...stringValues(item)])
  }

  return []
}

const projectionHasPrivateMaterial = (
  projection: OmniDataPackageExportProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeDataPackageRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectOmniDataPackageExport = (
  record: OmniDataPackageExportRecord,
  audience: OmniDataPackageAudience,
  nowIso: string,
): OmniDataPackageExportProjection => {
  assertRecord(record)

  const projection: OmniDataPackageExportProjection = {
    artifactDigests: record.artifactDigests
      .map(digest => artifactProjection(digest, audience))
      .filter((digest): digest is OmniDataPackageArtifactDigest =>
        digest !== null,
      ),
    audience,
    authority: OMNI_DATA_PACKAGE_EXPORT_READ_ONLY_AUTHORITY,
    caveatRefs: refsForAudience('Data package caveat refs', record.caveatRefs, audience),
    downloadMutationAllowed: false,
    fileHostingMutationAllowed: false,
    id: primaryRefForAudience(
      'Data package id refs',
      record.id,
      audience,
      'data_package.redacted',
    ),
    liveWalletSpendAllowed: false,
    packageRef: primaryRefForAudience(
      'Data package package refs',
      record.packageRef,
      audience,
      'package.redacted',
    ),
    provenance: provenanceProjection(record.provenance, audience),
    publicClaimUpgradeAllowed: false,
    publishedClaimAllowed:
      record.state === 'published' &&
      record.rights.rightsState === 'allowed' &&
      record.receiptRefs.length > 0,
    readyForSharing:
      ['package_ready', 'reviewed', 'published'].includes(record.state) &&
      record.rights.rightsState !== 'revoked' &&
      !['blocked', 'private_only'].includes(record.redaction.redactionState),
    receiptMutationAllowed: false,
    receiptRefs: refsForAudience(
      'Data package receipt refs',
      record.receiptRefs,
      audience,
    ),
    redaction: redactionProjection(record.redaction, audience),
    reviewStateRef: nullableRefForAudience(
      'Data package review state refs',
      record.reviewStateRef,
      audience,
    ),
    rights: rightsProjection(record.rights, audience),
    rightsMutationAllowed: false,
    schema: schemaProjection(record.schema, audience),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    titleRef: primaryRefForAudience(
      'Data package title refs',
      record.titleRef,
      audience,
      'title.redacted',
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (projectionHasPrivateMaterial(projection)) {
    throw new OmniDataPackageExportUnsafe({
      reason:
        'Data package export projection contains private customer, provider, wallet, payment, raw source archive, raw customer/provider records, private repo, secret, raw timestamp, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniDataPackageExport =
  (): OmniDataPackageExportRecord => ({
    artifactDigests: [
      {
        artifactKind: 'dataset',
        artifactRef: 'artifact.public.otec_research_dataset',
        digestAlgorithm: 'sha256',
        digestRef: 'digest.public.otec_research_dataset',
        sizeBytes: 120048,
      },
    ],
    authority: OMNI_DATA_PACKAGE_EXPORT_READ_ONLY_AUTHORITY,
    caveatRefs: ['caveat.public.data_package_is_redacted'],
    createdAtIso: '2026-06-06T22:00:00.000Z',
    id: 'data_package.public.otec_research_v1',
    packageRef: 'package.public.otec_research_v1',
    provenance: {
      generationRefs: ['generation.public.otec_research_export'],
      reviewRefs: ['review.public.operator_approved'],
      sourceBundleRefs: ['bundle.public.otec_research_sources'],
      sourceRefs: ['source.public.openagents_transcript_230'],
      spanRefs: ['span.public.transcript_230_intro'],
    },
    receiptRefs: ['receipt.public.otec_research_export'],
    redaction: {
      blockedReasonRefs: [],
      redactionPolicyRefs: ['policy.public.redacted_archive_only'],
      redactionState: 'redacted',
      removedFieldRefs: ['field.public.customer_notes_removed'],
      retainedFieldRefs: ['field.public.source_refs'],
      reviewerRefs: ['reviewer.public.operator_review'],
    },
    reviewStateRef: 'review.public.operator_approved',
    rights: {
      allowedAudienceRefs: ['audience.public.investor_review'],
      expiryRef: null,
      licenseRefs: ['license.public.openagents_docs'],
      rightsPolicyRef: 'rights.public.web_citation_allowed',
      rightsState: 'allowed',
      usageCaveatRefs: ['caveat.public.citation_required'],
    },
    schema: {
      fieldRefs: ['field.public.source_ref', 'field.public.span_ref'],
      schemaRef: 'schema.public.knowledge_source_package',
      sensitiveFieldRefs: ['field.public.customer_notes_removed'],
      versionRef: 'schema_version.public.knowledge_source_package.v1',
    },
    state: 'published',
    titleRef: 'title.public.otec_research_data_package',
    updatedAtIso: '2026-06-06T22:25:00.000Z',
  })
