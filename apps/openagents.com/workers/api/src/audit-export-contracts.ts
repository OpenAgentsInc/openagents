import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniDataClassification,
  OmniDataPolicyEnvelope,
  OmniDataPolicyProjection,
  OmniProjectionAudience,
  omniDataPolicyExportAllowed,
  omniDataPolicyProjectionDecision,
  projectOmniDataPolicyEnvelope,
} from './omni-data-classification'

export const OpenAgentsAuditExportScope = S.Literals([
  'artifact',
  'assignment',
  'billing_payment',
  'deployment',
  'email',
  'evidence_bundle',
  'forum_activity',
  'order',
  'public_claim',
  'receipt',
  'site',
  'site_revision',
  'site_version',
  'workroom',
])
export type OpenAgentsAuditExportScope =
  typeof OpenAgentsAuditExportScope.Type

export const OpenAgentsAuditExportDecision = S.Literals([
  'denied',
  'included',
  'omitted',
])
export type OpenAgentsAuditExportDecision =
  typeof OpenAgentsAuditExportDecision.Type

export class OpenAgentsAuditExportRequest extends S.Class<OpenAgentsAuditExportRequest>(
  'OpenAgentsAuditExportRequest',
)({
  approvedByRef: S.NullOr(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  exportPolicyRefs: S.Array(S.String),
  generatedAtIso: S.String,
  id: S.String,
  requestedScopeRefs: S.Array(S.String),
  requestedScopes: S.Array(OpenAgentsAuditExportScope),
  requesterRef: S.String,
  retentionPolicyRefs: S.Array(S.String),
}) {}

export class OpenAgentsAuditExportItem extends S.Class<OpenAgentsAuditExportItem>(
  'OpenAgentsAuditExportItem',
)({
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  dataPolicy: OmniDataPolicyEnvelope,
  evidenceRefs: S.Array(S.String),
  exportPolicyRefs: S.Array(S.String),
  itemRef: S.String,
  receiptRefs: S.Array(S.String),
  retentionPolicyRefs: S.Array(S.String),
  scope: OpenAgentsAuditExportScope,
  sourceRefs: S.Array(S.String),
}) {}

export class OpenAgentsAuditExportItemProjection extends S.Class<OpenAgentsAuditExportItemProjection>(
  'OpenAgentsAuditExportItemProjection',
)({
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dataPolicy: OmniDataPolicyProjection,
  evidenceRefs: S.Array(S.String),
  exportPolicyRefs: S.Array(S.String),
  itemRef: S.String,
  receiptRefs: S.Array(S.String),
  retentionPolicyRefs: S.Array(S.String),
  scope: OpenAgentsAuditExportScope,
  sourceRefs: S.Array(S.String),
}) {}

export class OpenAgentsAuditExportDenial extends S.Class<OpenAgentsAuditExportDenial>(
  'OpenAgentsAuditExportDenial',
)({
  caveatRefs: S.Array(S.String),
  dataPolicy: OmniDataPolicyProjection,
  denialRefs: S.Array(S.String),
  itemRef: S.String,
  scope: OpenAgentsAuditExportScope,
}) {}

export class OpenAgentsAuditExportBundle extends S.Class<OpenAgentsAuditExportBundle>(
  'OpenAgentsAuditExportBundle',
)({
  approvedByRef: S.NullOr(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  denialRefs: S.Array(S.String),
  denials: S.Array(OpenAgentsAuditExportDenial),
  deniedItemRefs: S.Array(S.String),
  exportPolicyRefs: S.Array(S.String),
  generatedAtIso: S.String,
  id: S.String,
  includedItemRefs: S.Array(S.String),
  includedItems: S.Array(OpenAgentsAuditExportItemProjection),
  omittedItemRefs: S.Array(S.String),
  requestedScopeRefs: S.Array(S.String),
  requestedScopes: S.Array(OpenAgentsAuditExportScope),
  requesterRef: S.String,
  retentionPolicyRefs: S.Array(S.String),
}) {}

export class OpenAgentsAuditExportBundleProjection extends S.Class<OpenAgentsAuditExportBundleProjection>(
  'OpenAgentsAuditExportBundleProjection',
)({
  approvedByRef: S.NullOr(S.String),
  audience: OmniProjectionAudience,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  denialRefs: S.Array(S.String),
  denials: S.Array(OpenAgentsAuditExportDenial),
  deniedItemCount: S.Number,
  deniedItemRefs: S.Array(S.String),
  exportPolicyRefs: S.Array(S.String),
  generatedAtDisplay: S.String,
  id: S.String,
  includedItemRefs: S.Array(S.String),
  includedItems: S.Array(OpenAgentsAuditExportItemProjection),
  itemCount: S.Number,
  omittedItemCount: S.Number,
  omittedItemRefs: S.Array(S.String),
  requestedScopeRefs: S.Array(S.String),
  requestedScopes: S.Array(OpenAgentsAuditExportScope),
  requesterRef: S.NullOr(S.String),
  retentionPolicyRefs: S.Array(S.String),
}) {}

export class OpenAgentsAuditExportUnsafe extends S.TaggedErrorClass<OpenAgentsAuditExportUnsafe>()(
  'OpenAgentsAuditExportUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeAuditExportRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const exportPolicyDenyPattern =
  /(export\.deny|export\.never|retention\.delete_requested)/i
const publicUnsafeRefPattern =
  /(approved_by|billing|email\.private|evidence\.operator|operator|payment|private|provider|requested_by|source\.private|workroom\.private)/i
const customerUnsafeRefPattern =
  /(approved_by|billing\.private|email\.private|evidence\.operator|operator|payment\.private|provider|source\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(billing\.private|email\.private|payment\.private|provider\.private|source\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const uniqueScopes = (
  scopes: ReadonlyArray<OpenAgentsAuditExportScope>,
): ReadonlyArray<OpenAgentsAuditExportScope> =>
  [...new Set(scopes)].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeAuditExportRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsAuditExportUnsafe({
      reason: `${label} contains private customer data, provider grants, raw provider payloads, raw emails, raw runner logs, raw source archives, private repo material, secrets, wallet/payment material, payout targets, or raw timestamps.`,
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

const safeNullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: typeof OmniProjectionAudience.Type,
): string | null =>
  ref === null
    ? null
    : safeRefsForAudience(label, [ref], audience)[0] ?? null

const projectionDecisionDenialRefs = (
  item: OpenAgentsAuditExportItem,
): ReadonlyArray<string> => {
  const decision = omniDataPolicyProjectionDecision(
    item.dataPolicy,
    item.dataPolicy.trustTier === 'blocked' ? 'private' : 'operator',
  )

  return [
    item.dataPolicy.trustTier === 'blocked'
      ? 'denial.audit_export.blocked_trust'
      : null,
    item.dataPolicy.dataClassification === 'secret_bearing' ||
        item.dataPolicy.dataClassification === 'deletion_retention_sensitive'
      ? 'denial.audit_export.classification_not_exportable'
      : null,
    item.dataPolicy.exportPolicyRefs.some(ref =>
        exportPolicyDenyPattern.test(ref)
      ) ||
        item.dataPolicy.retentionPolicyRefs.some(ref =>
          exportPolicyDenyPattern.test(ref)
        ) ||
        item.exportPolicyRefs.some(ref => exportPolicyDenyPattern.test(ref)) ||
        item.retentionPolicyRefs.some(ref =>
          exportPolicyDenyPattern.test(ref)
        )
      ? 'denial.audit_export.retention_or_export_policy'
      : null,
    decision === 'deny' ? 'denial.audit_export.projection_denied' : null,
  ].filter((ref): ref is string => ref !== null)
}

const exportDecisionForItem = (
  item: OpenAgentsAuditExportItem,
  audience: typeof OmniProjectionAudience.Type,
): OpenAgentsAuditExportDecision => {
  const denialRefs = projectionDecisionDenialRefs(item)

  if (denialRefs.length > 0) {
    return 'denied'
  }

  if (omniDataPolicyExportAllowed(item.dataPolicy, audience)) {
    return 'included'
  }

  return 'omitted'
}

const requestRefs = (
  request: OpenAgentsAuditExportRequest,
): ReadonlyArray<string> => [
  request.id,
  request.requesterRef,
  ...(request.approvedByRef === null ? [] : [request.approvedByRef]),
  ...request.caveatRefs,
  ...request.exportPolicyRefs,
  ...request.requestedScopeRefs,
  ...request.retentionPolicyRefs,
]

const itemRefs = (
  item: OpenAgentsAuditExportItem,
): ReadonlyArray<string> => [
  item.itemRef,
  ...item.caveatRefs,
  ...item.evidenceRefs,
  ...item.exportPolicyRefs,
  ...item.receiptRefs,
  ...item.retentionPolicyRefs,
  ...item.sourceRefs,
]

const assertRequestSafe = (request: OpenAgentsAuditExportRequest): void => {
  assertSafeRefs('audit export request refs', requestRefs(request))
}

const assertItemSafe = (item: OpenAgentsAuditExportItem): void => {
  assertSafeRefs('audit export item refs', itemRefs(item))
}

const projectItem = (
  item: OpenAgentsAuditExportItem,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsAuditExportItemProjection => {
  assertItemSafe(item)

  return {
    caveatRefs: safeRefsForAudience(
      'audit export item caveat refs',
      item.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      item.createdAtIso,
      nowIso,
    ),
    dataPolicy: projectOmniDataPolicyEnvelope(item.dataPolicy, audience),
    evidenceRefs: safeRefsForAudience(
      'audit export item evidence refs',
      item.evidenceRefs,
      audience,
    ),
    exportPolicyRefs: safeRefsForAudience(
      'audit export item export refs',
      item.exportPolicyRefs,
      audience,
    ),
    itemRef: safeRefForAudience(
      'audit export item ref',
      item.itemRef,
      audience,
    ),
    receiptRefs: safeRefsForAudience(
      'audit export item receipt refs',
      item.receiptRefs,
      audience,
    ),
    retentionPolicyRefs: safeRefsForAudience(
      'audit export item retention refs',
      item.retentionPolicyRefs,
      audience,
    ),
    scope: item.scope,
    sourceRefs: safeRefsForAudience(
      'audit export item source refs',
      item.sourceRefs,
      audience,
    ),
  }
}

const projectDenial = (
  item: OpenAgentsAuditExportItem,
  audience: typeof OmniProjectionAudience.Type,
): OpenAgentsAuditExportDenial => ({
  caveatRefs: safeRefsForAudience(
    'audit export denial caveat refs',
    item.caveatRefs,
    audience,
  ),
  dataPolicy: projectOmniDataPolicyEnvelope(item.dataPolicy, audience),
  denialRefs: uniqueRefs([
    ...projectionDecisionDenialRefs(item),
    'denial.audit_export.item_not_exported',
  ]),
  itemRef: safeRefForAudience(
    'audit export denied item ref',
    item.itemRef,
    audience,
  ),
  scope: item.scope,
})

const itemRefForAudience = (
  item: OpenAgentsAuditExportItem,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefForAudience('audit export item ref', item.itemRef, audience)

const exportItemsByDecision = (
  items: ReadonlyArray<OpenAgentsAuditExportItem>,
  audience: typeof OmniProjectionAudience.Type,
) => ({
  denied: items.filter(item => exportDecisionForItem(item, audience) === 'denied'),
  included: items.filter(item =>
    exportDecisionForItem(item, audience) === 'included'
  ),
  omitted: items.filter(item =>
    exportDecisionForItem(item, audience) === 'omitted'
  ),
})

export const buildOpenAgentsAuditExportBundle = (
  request: OpenAgentsAuditExportRequest,
  items: ReadonlyArray<OpenAgentsAuditExportItem>,
): OpenAgentsAuditExportBundle => {
  assertRequestSafe(request)
  items.forEach(assertItemSafe)

  const grouped = exportItemsByDecision(items, request.audience)
  const includedItems = grouped.included.map(item =>
    projectItem(item, request.audience, request.generatedAtIso)
  )
  const denials = grouped.denied.map(item =>
    projectDenial(item, request.audience)
  )
  const denialRefs = uniqueRefs(denials.flatMap(denial => denial.denialRefs))

  return {
    approvedByRef: request.approvedByRef,
    audience: request.audience,
    caveatRefs: uniqueRefs(request.caveatRefs),
    createdAtIso: request.createdAtIso,
    denialRefs,
    denials,
    deniedItemRefs: uniqueRefs(
      grouped.denied.map(item => itemRefForAudience(item, request.audience)),
    ),
    exportPolicyRefs: uniqueRefs(request.exportPolicyRefs),
    generatedAtIso: request.generatedAtIso,
    id: request.id,
    includedItemRefs: uniqueRefs(
      includedItems.map(item => item.itemRef),
    ),
    includedItems,
    omittedItemRefs: uniqueRefs(
      grouped.omitted.map(item => itemRefForAudience(item, request.audience)),
    ),
    requestedScopeRefs: uniqueRefs(request.requestedScopeRefs),
    requestedScopes: uniqueScopes(request.requestedScopes),
    requesterRef: request.requesterRef,
    retentionPolicyRefs: uniqueRefs(request.retentionPolicyRefs),
  }
}

export const projectOpenAgentsAuditExportBundle = (
  bundle: OpenAgentsAuditExportBundle,
  nowIso: string,
): OpenAgentsAuditExportBundleProjection => {
  const audience = bundle.audience
  const projection: OpenAgentsAuditExportBundleProjection = {
    approvedByRef:
      audience === 'team' || audience === 'operator' || audience === 'private'
        ? safeNullableRefForAudience(
            'audit export approved-by ref',
            bundle.approvedByRef,
            audience,
          )
        : null,
    audience,
    caveatRefs: safeRefsForAudience(
      'audit export caveat refs',
      bundle.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      bundle.createdAtIso,
      nowIso,
    ),
    denialRefs: safeRefsForAudience(
      'audit export denial refs',
      bundle.denialRefs,
      audience,
    ),
    denials: bundle.denials.map(denial => ({
      caveatRefs: safeRefsForAudience(
        'audit export denial caveat refs',
        denial.caveatRefs,
        audience,
      ),
      dataPolicy: denial.dataPolicy,
      denialRefs: safeRefsForAudience(
        'audit export denial refs',
        denial.denialRefs,
        audience,
      ),
      itemRef: safeRefForAudience(
        'audit export denied item ref',
        denial.itemRef,
        audience,
      ),
      scope: denial.scope,
    })),
    deniedItemCount: bundle.deniedItemRefs.length,
    deniedItemRefs: safeRefsForAudience(
      'audit export denied item refs',
      bundle.deniedItemRefs,
      audience,
    ),
    exportPolicyRefs: safeRefsForAudience(
      'audit export policy refs',
      bundle.exportPolicyRefs,
      audience,
    ),
    generatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      bundle.generatedAtIso,
      nowIso,
    ),
    id: safeRefForAudience('audit export id', bundle.id, audience),
    includedItemRefs: safeRefsForAudience(
      'audit export included item refs',
      bundle.includedItemRefs,
      audience,
    ),
    includedItems: bundle.includedItems,
    itemCount: bundle.includedItemRefs.length,
    omittedItemCount: bundle.omittedItemRefs.length,
    omittedItemRefs: safeRefsForAudience(
      'audit export omitted item refs',
      bundle.omittedItemRefs,
      audience,
    ),
    requestedScopeRefs: safeRefsForAudience(
      'audit export requested scope refs',
      bundle.requestedScopeRefs,
      audience,
    ),
    requestedScopes: bundle.requestedScopes,
    requesterRef:
      audience === 'operator' || audience === 'private'
        ? safeNullableRefForAudience(
            'audit export requester ref',
            bundle.requesterRef,
            audience,
          )
        : null,
    retentionPolicyRefs: safeRefsForAudience(
      'audit export retention refs',
      bundle.retentionPolicyRefs,
      audience,
    ),
  }

  if (openAgentsAuditExportProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsAuditExportUnsafe({
      reason: 'Audit export projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsAuditExportProjectionHasPrivateMaterial = (
  projection: OpenAgentsAuditExportBundleProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return unsafeAuditExportRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}

export const auditExportClassificationRequiresOperatorReview = (
  classification: typeof OmniDataClassification.Type,
): boolean =>
  classification === 'legal_sensitive' ||
  classification === 'payment_private' ||
  classification === 'provider_private' ||
  classification === 'secret_bearing' ||
  classification === 'deletion_retention_sensitive'
