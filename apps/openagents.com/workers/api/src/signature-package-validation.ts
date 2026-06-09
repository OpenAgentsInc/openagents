import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const SignaturePackageValidationEndpoint =
  '/api/developer/signature-packages/validate'

export const SignaturePackageValidationStatus = S.Literals([
  'blocked',
  'invalid',
  'valid',
])
export type SignaturePackageValidationStatus =
  typeof SignaturePackageValidationStatus.Type

export class SignaturePackageManifest extends S.Class<SignaturePackageManifest>(
  'SignaturePackageManifest',
)({
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  displayName: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  jsonRenderBindingRefs: S.Array(S.String),
  packageRef: S.String,
  receiptRequirementRefs: S.Array(S.String),
  riskClassRef: S.String,
  schemaRefs: S.Array(S.String),
  selectorMetadataRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class SignaturePackageManifestProjection extends S.Class<SignaturePackageManifestProjection>(
  'SignaturePackageManifestProjection',
)({
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  displayName: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  fixtureRefs: S.Array(S.String),
  id: S.String,
  jsonRenderBindingRefs: S.Array(S.String),
  packageRef: S.String,
  receiptRequirementRefs: S.Array(S.String),
  riskClassRef: S.String,
  schemaRefs: S.Array(S.String),
  selectorMetadataRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class SignaturePackageValidationRequest extends S.Class<SignaturePackageValidationRequest>(
  'SignaturePackageValidationRequest',
)({
  manifest: SignaturePackageManifest,
  nowIso: S.String,
  validationRequestRef: S.String,
}) {}

export class SignaturePackageValidationResult extends S.Class<SignaturePackageValidationResult>(
  'SignaturePackageValidationResult',
)({
  audience: OmniProjectionAudience,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deploymentAllowed: S.Boolean,
  evidenceRequirementsPresent: S.Boolean,
  fixtureRefsPresent: S.Boolean,
  installAllowed: S.Boolean,
  jsonRenderBindingsPresent: S.Boolean,
  manifest: SignaturePackageManifestProjection,
  operatorDiagnosticRefs: S.Array(S.String),
  paymentMutationAllowed: S.Boolean,
  publicMarketplaceListingAllowed: S.Boolean,
  receiptRequirementsPresent: S.Boolean,
  riskClassPresent: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  schemaRefsPresent: S.Boolean,
  selectorMetadataPresent: S.Boolean,
  status: SignaturePackageValidationStatus,
  updatedAtDisplay: S.String,
  validationRequestRef: S.String,
  validationResultRef: S.String,
}) {}

export class SignaturePackageValidationUnsafe extends S.TaggedErrorClass<SignaturePackageValidationUnsafe>()(
  'SignaturePackageValidationUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeSignaturePackageRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|package[_-]?source[_-]?private|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|package|repo|source)|provider[_-]?(account|grant|payload|token)|raw[_-]?(document|email|fixture|invoice|package|payment|payload|prompt|provider|runner|run[_-]?log|schema|source|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(operator\.|package_source\.|provider\.|raw\.|source\.private|workroom\.private)/i
const customerUnsafeRefPattern =
  /(operator\.|package_source\.private|provider\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(operator\.|package_source\.private|provider\.private|source\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const safeRefSegment = (value: string): string =>
  value
    .trim()
    .replaceAll(/[^A-Za-z0-9_.-]+/g, '_')
    .replaceAll(/^_+|_+$/g, '')
    .slice(0, 96) || 'package'

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeSignaturePackageRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new SignaturePackageValidationUnsafe({
      reason: `${label} contains private package source, raw prompts, provider payloads, private repo refs, wallet/payment material, secrets, raw logs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const manifestRefs = (
  manifest: SignaturePackageManifest,
): ReadonlyArray<string> => [
  manifest.id,
  manifest.packageRef,
  manifest.versionRef,
  manifest.riskClassRef,
  ...manifest.caveatRefs,
  ...manifest.evidenceRequirementRefs,
  ...manifest.fixtureRefs,
  ...manifest.jsonRenderBindingRefs,
  ...manifest.receiptRequirementRefs,
  ...manifest.schemaRefs,
  ...manifest.selectorMetadataRefs,
  ...manifest.sourceRefs,
]

const assertManifestSafe = (
  manifest: SignaturePackageManifest,
): void => {
  assertSafeRefs('Signature package manifest refs', manifestRefs(manifest))

  if (manifest.displayName.trim() === '') {
    throw new SignaturePackageValidationUnsafe({
      reason: 'Signature package manifests require a display name.',
    })
  }
}

export const projectSignaturePackageManifest = (
  manifest: SignaturePackageManifest,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): SignaturePackageManifestProjection => {
  assertManifestSafe(manifest)

  return {
    audience,
    caveatRefs: safeRefsForAudience(
      'Signature package caveat refs',
      manifest.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      manifest.createdAtIso,
      nowIso,
    ),
    displayName: manifest.displayName.trim(),
    evidenceRequirementRefs: safeRefsForAudience(
      'Signature package evidence requirement refs',
      manifest.evidenceRequirementRefs,
      audience,
    ),
    fixtureRefs: safeRefsForAudience(
      'Signature package fixture refs',
      manifest.fixtureRefs,
      audience,
    ),
    id: safeRefForAudience('Signature package id', manifest.id, audience),
    jsonRenderBindingRefs: safeRefsForAudience(
      'Signature package json-render binding refs',
      manifest.jsonRenderBindingRefs,
      audience,
    ),
    packageRef: safeRefForAudience(
      'Signature package ref',
      manifest.packageRef,
      audience,
    ),
    receiptRequirementRefs: safeRefsForAudience(
      'Signature package receipt requirement refs',
      manifest.receiptRequirementRefs,
      audience,
    ),
    riskClassRef: safeRefForAudience(
      'Signature package risk class ref',
      manifest.riskClassRef,
      audience,
    ),
    schemaRefs: safeRefsForAudience(
      'Signature package schema refs',
      manifest.schemaRefs,
      audience,
    ),
    selectorMetadataRefs: safeRefsForAudience(
      'Signature package selector metadata refs',
      manifest.selectorMetadataRefs,
      audience,
    ),
    sourceRefs: audience === 'public' || audience === 'agent'
      ? []
      : safeRefsForAudience(
        'Signature package source refs',
        manifest.sourceRefs,
        audience,
      ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      manifest.updatedAtIso,
      nowIso,
    ),
    versionRef: safeRefForAudience(
      'Signature package version ref',
      manifest.versionRef,
      audience,
    ),
  }
}

const validationBlockerRefs = (
  manifest: SignaturePackageManifest,
): ReadonlyArray<string> => [
  ...(manifest.schemaRefs.length === 0
    ? ['blocker.signature_package.schema_refs_missing']
    : []),
  ...(manifest.fixtureRefs.length === 0
    ? ['blocker.signature_package.fixture_refs_missing']
    : []),
  ...(manifest.riskClassRef.trim() === ''
    ? ['blocker.signature_package.risk_class_missing']
    : []),
  ...(manifest.evidenceRequirementRefs.length === 0
    ? ['blocker.signature_package.evidence_requirements_missing']
    : []),
  ...(manifest.receiptRequirementRefs.length === 0
    ? ['blocker.signature_package.receipt_requirements_missing']
    : []),
  ...(manifest.selectorMetadataRefs.length === 0
    ? ['blocker.signature_package.selector_metadata_missing']
    : []),
  ...(manifest.jsonRenderBindingRefs.length === 0
    ? ['blocker.signature_package.json_render_bindings_missing']
    : []),
]

export const validateSignaturePackage = (
  request: SignaturePackageValidationRequest,
  audience: typeof OmniProjectionAudience.Type = 'agent',
): SignaturePackageValidationResult => {
  const manifest = S.decodeUnknownSync(SignaturePackageManifest)(
    request.manifest,
  )
  const projection = projectSignaturePackageManifest(
    manifest,
    audience,
    request.nowIso,
  )
  const blockerRefs = validationBlockerRefs(manifest)
  const status: SignaturePackageValidationStatus =
    blockerRefs.length === 0 ? 'valid' : 'invalid'
  const caveatRefs = uniqueRefs([
    ...manifest.caveatRefs,
    'caveat.signature_package.validation_read_only',
    'caveat.signature_package.no_install_or_promotion',
  ])

  return {
    audience,
    blockerRefs,
    caveatRefs: safeRefsForAudience(
      'Signature package validation caveat refs',
      caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      manifest.createdAtIso,
      request.nowIso,
    ),
    deploymentAllowed: false,
    evidenceRequirementsPresent: manifest.evidenceRequirementRefs.length > 0,
    fixtureRefsPresent: manifest.fixtureRefs.length > 0,
    installAllowed: false,
    jsonRenderBindingsPresent: manifest.jsonRenderBindingRefs.length > 0,
    manifest: projection,
    operatorDiagnosticRefs: audience === 'operator' || audience === 'private'
      ? [
        `operator.signature_package.validation.${
          status === 'valid' ? 'passed' : 'blocked'
        }`,
      ]
      : [],
    paymentMutationAllowed: false,
    publicMarketplaceListingAllowed: false,
    receiptRequirementsPresent: manifest.receiptRequirementRefs.length > 0,
    riskClassPresent: manifest.riskClassRef.trim() !== '',
    runtimePromotionAllowed: false,
    schemaRefsPresent: manifest.schemaRefs.length > 0,
    selectorMetadataPresent: manifest.selectorMetadataRefs.length > 0,
    status,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      manifest.updatedAtIso,
      request.nowIso,
    ),
    validationRequestRef: safeRefForAudience(
      'Signature package validation request ref',
      request.validationRequestRef,
      audience,
    ),
    validationResultRef: `validation_result.${
      safeRefSegment(request.validationRequestRef)
    }.${safeRefSegment(manifest.packageRef)}.${
      safeRefSegment(manifest.versionRef)
    }`,
  }
}

const projectionText = (
  projection:
    | SignaturePackageManifestProjection
    | SignaturePackageValidationResult,
): string =>
  'manifest' in projection
    ? [
      projection.validationRequestRef,
      projection.validationResultRef,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.operatorDiagnosticRefs,
      projection.manifest.id,
      projection.manifest.packageRef,
      projection.manifest.versionRef,
      projection.manifest.riskClassRef,
      ...projection.manifest.caveatRefs,
      ...projection.manifest.evidenceRequirementRefs,
      ...projection.manifest.fixtureRefs,
      ...projection.manifest.jsonRenderBindingRefs,
      ...projection.manifest.receiptRequirementRefs,
      ...projection.manifest.schemaRefs,
      ...projection.manifest.selectorMetadataRefs,
      ...projection.manifest.sourceRefs,
    ].join(' ')
    : [
      projection.id,
      projection.packageRef,
      projection.versionRef,
      projection.riskClassRef,
      ...projection.caveatRefs,
      ...projection.evidenceRequirementRefs,
      ...projection.fixtureRefs,
      ...projection.jsonRenderBindingRefs,
      ...projection.receiptRequirementRefs,
      ...projection.schemaRefs,
      ...projection.selectorMetadataRefs,
      ...projection.sourceRefs,
    ].join(' ')

export const signaturePackageProjectionHasPrivateMaterial = (
  projection:
    | SignaturePackageManifestProjection
    | SignaturePackageValidationResult,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeSignaturePackageRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE:
  SignaturePackageManifest = {
    caveatRefs: ['caveat.signature_package.review_required'],
    createdAtIso: '2026-06-07T08:00:00.000Z',
    displayName: 'Example Site Builder Signature',
    evidenceRequirementRefs: ['evidence_requirement.signature.demo_trace'],
    fixtureRefs: ['fixture.signature.demo_request'],
    id: 'signature_package.example_site_builder',
    jsonRenderBindingRefs: ['json_render.signature.site_card'],
    packageRef: 'package.signature.example_site_builder',
    receiptRequirementRefs: ['receipt_requirement.signature.validation'],
    riskClassRef: 'risk_class.signature.low',
    schemaRefs: ['schema.signature.package_manifest.v1'],
    selectorMetadataRefs: ['selector.signature.site_builder'],
    sourceRefs: ['source.signature.package_repo_public'],
    updatedAtIso: '2026-06-07T08:05:00.000Z',
    versionRef: 'version.signature.example_site_builder.v1',
  }

export const SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE:
  SignaturePackageValidationRequest = {
    manifest: SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
    nowIso: '2026-06-07T08:40:00.000Z',
    validationRequestRef: 'validation_request.signature.example_site_builder',
  }
