import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

// ---------------------------------------------------------------------------
// Source-authorized business objects (DE-9 / EPIC #5532)
//
// This module is the typed source-authority model for who/what may write a
// business object inside a workroom, plus the approval-gated write decision
// engine. It is a CONTRACT/PROJECTION-ONLY surface, exactly like the existing
// omni-crm-follow-up-workrooms and omni-support-project-ops-workrooms modules:
// it records authority bindings, proposed writes, approvals, write receipts,
// and closeout as public-safe refs only. It never performs a real business-
// object mutation, never settles, never sends, and never grants spend or
// provider-account authority by itself.
//
// The promise stays RED. Green requires a LIVE source-authorized,
// approval-gated workroom write with a closeout receipt and owner sign-off
// (see omni-workroom-business-object-delivery.ts for the flag-gated INERT
// delivery integration seam, and product-promises.ts for the gate).
// ---------------------------------------------------------------------------

/**
 * The class of business object a write targets. These are the operational
 * truth objects the source-authorized-business-objects promise speaks to:
 * contacts, companies, tasks, decisions, documents, approvals, artifacts,
 * receipts.
 */
export const OmniBusinessObjectKind = S.Literals([
  'approval',
  'artifact',
  'company',
  'contact',
  'decision',
  'document',
  'receipt',
  'task',
])
export type OmniBusinessObjectKind = typeof OmniBusinessObjectKind.Type

/**
 * The class of actor a source-authority binding authorizes. A binding names
 * WHO may write (the principal class) and WHAT source backs the write.
 */
export const OmniSourceAuthorityPrincipalKind = S.Literals([
  'agent_runtime',
  'authorized_organization',
  'authorized_user',
  'connector_source',
  'operator',
])
export type OmniSourceAuthorityPrincipalKind =
  typeof OmniSourceAuthorityPrincipalKind.Type

/**
 * The class of source backing a write. A source-authorized write must be
 * backed by a typed source ref, never inferred from chat text alone.
 */
export const OmniBusinessObjectSourceKind = S.Literals([
  'approval_decision',
  'connector_read',
  'uploaded_document',
  'verified_chat_extraction',
  'workroom_artifact',
])
export type OmniBusinessObjectSourceKind =
  typeof OmniBusinessObjectSourceKind.Type

/**
 * The write operation a proposed write performs against a business object.
 */
export const OmniBusinessObjectWriteOperation = S.Literals([
  'append',
  'create',
  'supersede',
  'update',
])
export type OmniBusinessObjectWriteOperation =
  typeof OmniBusinessObjectWriteOperation.Type

/**
 * Lifecycle of a single proposed write against a business object. A write is
 * a PROPOSAL until an authorized approver accepts it; only then may it be
 * applied, and an applied write is not "settled truth" until a closeout
 * receipt is recorded.
 */
export const OmniBusinessObjectWriteState = S.Literals([
  'applied',
  'approval_recorded',
  'approval_requested',
  'blocked',
  'closed',
  'proposed',
  'rejected',
])
export type OmniBusinessObjectWriteState =
  typeof OmniBusinessObjectWriteState.Type

export const OmniSourceAuthorityBoundary = S.Literals([
  'contract_projection_only',
])
export type OmniSourceAuthorityBoundary =
  typeof OmniSourceAuthorityBoundary.Type

/**
 * The fixed authority boundary every record in this module must carry. A
 * source-authority binding describes WHO MAY propose a write; it never grants
 * the runtime the ability to apply, send, settle, or spend on its own.
 */
export class OmniSourceAuthority extends S.Class<OmniSourceAuthority>(
  'OmniSourceAuthority',
)({
  authorityBoundary: OmniSourceAuthorityBoundary,
  noBusinessObjectMutationWithoutApproval: S.Boolean,
  noConnectorWritebackWithoutApproval: S.Boolean,
  noNotificationSend: S.Boolean,
  noSettlementImplication: S.Boolean,
  noSpendAuthority: S.Boolean,
}) {}

export const OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY: OmniSourceAuthority = {
  authorityBoundary: 'contract_projection_only',
  noBusinessObjectMutationWithoutApproval: true,
  noConnectorWritebackWithoutApproval: true,
  noNotificationSend: true,
  noSettlementImplication: true,
  noSpendAuthority: true,
}

export const omniSourceAuthorityIsContractOnly = (
  authority: OmniSourceAuthority,
): boolean =>
  authority.authorityBoundary === 'contract_projection_only' &&
  authority.noBusinessObjectMutationWithoutApproval &&
  authority.noConnectorWritebackWithoutApproval &&
  authority.noNotificationSend &&
  authority.noSettlementImplication &&
  authority.noSpendAuthority

/**
 * A typed authority binding: it names a principal class, the source class it
 * is allowed to back writes with, the business-object kinds it may target,
 * the write operations it may propose, and whether an approval is required
 * before a proposed write may be applied. `requiresApproval` defaults true
 * for the model; a binding that sets it false is only honored for low-risk
 * append/create operations on append-only kinds (see write-decision engine).
 */
export class OmniSourceAuthorityBinding extends S.Class<OmniSourceAuthorityBinding>(
  'OmniSourceAuthorityBinding',
)({
  allowedOperations: S.Array(OmniBusinessObjectWriteOperation),
  allowedSourceKinds: S.Array(OmniBusinessObjectSourceKind),
  authority: OmniSourceAuthority,
  businessObjectKinds: S.Array(OmniBusinessObjectKind),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  id: S.String,
  principalKind: OmniSourceAuthorityPrincipalKind,
  principalRef: S.String,
  requiresApproval: S.Boolean,
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniSourceAuthorityBindingProjection extends S.Class<OmniSourceAuthorityBindingProjection>(
  'OmniSourceAuthorityBindingProjection',
)({
  allowedOperations: S.Array(OmniBusinessObjectWriteOperation),
  allowedSourceKinds: S.Array(OmniBusinessObjectSourceKind),
  audience: OmniProjectionAudience,
  authority: OmniSourceAuthority,
  businessObjectKinds: S.Array(OmniBusinessObjectKind),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  id: S.String,
  principalKind: OmniSourceAuthorityPrincipalKind,
  principalRef: S.String,
  requiresApproval: S.Boolean,
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

/**
 * A single proposed write against a business object inside a workroom. It
 * carries the binding it claims authority under, the source ref backing it,
 * the approval refs once requested/recorded, the write-receipt ref once
 * applied, and the closeout ref once closed.
 */
export class OmniBusinessObjectWriteRecord extends S.Class<OmniBusinessObjectWriteRecord>(
  'OmniBusinessObjectWriteRecord',
)({
  appliedReceiptRefs: S.Array(S.String),
  approvalRefs: S.Array(S.String),
  authority: OmniSourceAuthority,
  blockerRefs: S.Array(S.String),
  bindingRef: S.String,
  businessObjectKind: OmniBusinessObjectKind,
  businessObjectRef: S.String,
  caveatRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  connectorReadReceiptRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  operation: OmniBusinessObjectWriteOperation,
  operatorDiagnosticRefs: S.Array(S.String),
  principalKind: OmniSourceAuthorityPrincipalKind,
  principalRef: S.String,
  proposedChangeRefs: S.Array(S.String),
  sourceKind: OmniBusinessObjectSourceKind,
  sourceRefs: S.Array(S.String),
  state: OmniBusinessObjectWriteState,
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniBusinessObjectWriteProjection extends S.Class<OmniBusinessObjectWriteProjection>(
  'OmniBusinessObjectWriteProjection',
)({
  appliedReceiptRefs: S.Array(S.String),
  approvalRecorded: S.Boolean,
  approvalRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: OmniSourceAuthority,
  blockerRefs: S.Array(S.String),
  bindingRef: S.String,
  businessObjectKind: OmniBusinessObjectKind,
  businessObjectRef: S.String,
  caveatRefs: S.Array(S.String),
  closeoutReady: S.Boolean,
  closeoutRefs: S.Array(S.String),
  connectorReadReceiptRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  mutationApplied: S.Boolean,
  operation: OmniBusinessObjectWriteOperation,
  operatorDiagnosticRefs: S.Array(S.String),
  principalKind: OmniSourceAuthorityPrincipalKind,
  principalRef: S.String,
  proposedChangeRefs: S.Array(S.String),
  sourceKind: OmniBusinessObjectSourceKind,
  sourceRefs: S.Array(S.String),
  state: OmniBusinessObjectWriteState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

/**
 * The decision the write-authority engine returns for a proposed write. This
 * is the core of the approval-gated write path: a write may only be applied
 * when the authority binding covers it AND (an approval is recorded OR the
 * write is a low-risk append/create whose binding waived approval).
 */
export class OmniBusinessObjectWriteDecision extends S.Class<OmniBusinessObjectWriteDecision>(
  'OmniBusinessObjectWriteDecision',
)({
  applyAllowed: S.Boolean,
  approvalRequired: S.Boolean,
  blockerRefs: S.Array(S.String),
  reasonRef: S.String,
}) {}

export class OmniSourceAuthorityUnsafe extends S.TaggedErrorClass<OmniSourceAuthorityUnsafe>()(
  'OmniSourceAuthorityUnsafe',
  { reason: S.String },
) {}

const stateRank: Readonly<Record<OmniBusinessObjectWriteState, number>> = {
  applied: 4,
  approval_recorded: 3,
  approval_requested: 2,
  blocked: -1,
  closed: 5,
  proposed: 1,
  rejected: -2,
}

const stateLabelByState: Readonly<
  Record<OmniBusinessObjectWriteState, string>
> = {
  applied: 'Applied',
  approval_recorded: 'Approval recorded',
  approval_requested: 'Approval requested',
  blocked: 'Blocked',
  closed: 'Closed',
  proposed: 'Proposed',
  rejected: 'Rejected',
}

export const omniBusinessObjectWriteStateAtLeast = (
  state: OmniBusinessObjectWriteState,
  threshold: OmniBusinessObjectWriteState,
): boolean => stateRank[state] >= stateRank[threshold]

/**
 * Append/create operations on append-only kinds are the only writes a binding
 * may waive approval for. Everything that updates or supersedes existing
 * business truth always requires an explicit approval ref.
 */
const APPROVAL_OPTIONAL_OPERATIONS: ReadonlySet<OmniBusinessObjectWriteOperation> =
  new Set<OmniBusinessObjectWriteOperation>(['append', 'create'])
const APPROVAL_OPTIONAL_KINDS: ReadonlySet<OmniBusinessObjectKind> =
  new Set<OmniBusinessObjectKind>(['artifact', 'receipt'])

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeSourceAuthorityRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|contact[_-]?(address|email|name|phone)|cookie|customer[_-]?(email|name|phone|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(contact|key)|provider[_-]?(account|grant|payload|token)|raw[_-]?(contact|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(approval\.|binding\.principal|company\.|contact\.|diagnostic\.operator|document\.|principal\.|proposed\.|source\.)/i
const customerUnsafeRefPattern =
  /(diagnostic\.operator|principal\.private|provider\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(diagnostic\.operator|provider\.private|source\.private)/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeSourceAuthorityRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniSourceAuthorityUnsafe({
      reason: `${label} contains raw email, private contact, customer, provider, connector payload, secret, wallet/payment, private repo, or raw timestamp material.`,
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

const bindingRefs = (
  binding: OmniSourceAuthorityBinding,
): ReadonlyArray<string> => [
  binding.id,
  binding.principalRef,
  binding.workroomRef,
  ...binding.caveatRefs,
]

const recordRefs = (
  record: OmniBusinessObjectWriteRecord,
): ReadonlyArray<string> => [
  record.id,
  record.bindingRef,
  record.businessObjectRef,
  record.principalRef,
  record.workroomRef,
  ...record.appliedReceiptRefs,
  ...record.approvalRefs,
  ...record.blockerRefs,
  ...record.caveatRefs,
  ...record.closeoutRefs,
  ...record.connectorReadReceiptRefs,
  ...record.evidenceRefs,
  ...record.operatorDiagnosticRefs,
  ...record.proposedChangeRefs,
  ...record.sourceRefs,
]

const assertBindingSafe = (binding: OmniSourceAuthorityBinding): void => {
  assertSafeRefs('source-authority binding refs', bindingRefs(binding))

  if (!omniSourceAuthorityIsContractOnly(binding.authority)) {
    throw new OmniSourceAuthorityUnsafe({
      reason:
        'Source-authority bindings must remain contract/projection-only and cannot grant unapproved business-object mutation, connector writeback, notification send, settlement, or spend authority.',
    })
  }

  if (binding.businessObjectKinds.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Source-authority bindings must name at least one business object kind.',
    })
  }

  if (binding.allowedOperations.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Source-authority bindings must name at least one write operation.',
    })
  }

  if (binding.allowedSourceKinds.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Source-authority bindings must name at least one source kind.',
    })
  }
}

const assertRecordSafe = (record: OmniBusinessObjectWriteRecord): void => {
  assertSafeRefs('business-object write refs', recordRefs(record))

  if (!omniSourceAuthorityIsContractOnly(record.authority)) {
    throw new OmniSourceAuthorityUnsafe({
      reason:
        'Business-object write records must remain contract/projection-only and cannot grant unapproved mutation, connector writeback, notification send, settlement, or spend authority.',
    })
  }

  if (record.sourceRefs.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason:
        'Business-object writes must carry at least one source ref; writes may not be inferred from chat text alone.',
    })
  }

  if (record.state === 'blocked' && record.blockerRefs.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Blocked business-object writes require blocker refs.',
    })
  }

  if (record.state === 'rejected' && record.approvalRefs.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Rejected business-object writes require approval (decision) refs.',
    })
  }

  if (
    (record.state === 'approval_requested' ||
      record.state === 'approval_recorded') &&
    record.approvalRefs.length === 0
  ) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Approval-requested writes require approval refs.',
    })
  }

  if (
    record.state === 'applied' &&
    record.appliedReceiptRefs.length === 0
  ) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Applied business-object writes require write-receipt refs.',
    })
  }

  if (
    record.sourceKind === 'connector_read' &&
    record.connectorReadReceiptRefs.length === 0
  ) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Connector read writes require connector read receipt refs.',
    })
  }

  if (record.state === 'closed' && record.closeoutRefs.length === 0) {
    throw new OmniSourceAuthorityUnsafe({
      reason: 'Closed business-object writes require closeout refs.',
    })
  }
}

/**
 * The approval-gated write-authority engine. Given a binding and a proposed
 * write, it decides whether the write may be applied. This is a pure decision:
 * it returns whether apply is allowed, whether approval is required, and any
 * typed blocker refs. It NEVER applies the write.
 *
 * Apply is allowed only when:
 *  - the binding's authority is contract-only (boundary preserved),
 *  - the binding covers the business-object kind, the operation, and the
 *    source kind the write claims,
 *  - the principal class and principal ref match the binding, and
 *  - an approval is recorded, OR the write is a low-risk append/create on an
 *    append-only kind whose binding explicitly waived approval.
 */
export const decideOmniBusinessObjectWrite = (
  binding: OmniSourceAuthorityBinding,
  record: OmniBusinessObjectWriteRecord,
): OmniBusinessObjectWriteDecision => {
  assertBindingSafe(binding)
  assertRecordSafe(record)

  const blockerRefs: Array<string> = []

  if (record.bindingRef !== binding.id) {
    blockerRefs.push('blocker.source_authority.binding_ref_mismatch')
  }

  if (record.workroomRef !== binding.workroomRef) {
    blockerRefs.push('blocker.source_authority.workroom_mismatch')
  }

  if (
    record.principalKind !== binding.principalKind ||
    record.principalRef !== binding.principalRef
  ) {
    blockerRefs.push('blocker.source_authority.principal_mismatch')
  }

  if (!binding.businessObjectKinds.includes(record.businessObjectKind)) {
    blockerRefs.push('blocker.source_authority.object_kind_not_authorized')
  }

  if (!binding.allowedOperations.includes(record.operation)) {
    blockerRefs.push('blocker.source_authority.operation_not_authorized')
  }

  if (!binding.allowedSourceKinds.includes(record.sourceKind)) {
    blockerRefs.push('blocker.source_authority.source_kind_not_authorized')
  }

  const writeIsLowRisk =
    APPROVAL_OPTIONAL_OPERATIONS.has(record.operation) &&
    APPROVAL_OPTIONAL_KINDS.has(record.businessObjectKind)

  const approvalRequired = binding.requiresApproval || !writeIsLowRisk

  const approvalRecorded =
    omniBusinessObjectWriteStateAtLeast(record.state, 'approval_recorded') &&
    record.approvalRefs.length > 0

  if (approvalRequired && !approvalRecorded) {
    blockerRefs.push('blocker.source_authority.approval_required')
  }

  if (record.state === 'rejected') {
    blockerRefs.push('blocker.source_authority.write_rejected')
  }

  if (record.state === 'blocked') {
    blockerRefs.push('blocker.source_authority.write_blocked')
  }

  const applyAllowed = blockerRefs.length === 0

  return new OmniBusinessObjectWriteDecision({
    applyAllowed,
    approvalRequired,
    blockerRefs: [...new Set(blockerRefs)].sort(),
    reasonRef: applyAllowed
      ? approvalRequired
        ? 'reason.source_authority.approved_write_applyable'
        : 'reason.source_authority.low_risk_write_applyable'
      : 'reason.source_authority.write_not_applyable',
  })
}

export const projectOmniSourceAuthorityBinding = (
  binding: OmniSourceAuthorityBinding,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniSourceAuthorityBindingProjection => {
  assertBindingSafe(binding)

  return new OmniSourceAuthorityBindingProjection({
    allowedOperations: binding.allowedOperations,
    allowedSourceKinds: binding.allowedSourceKinds,
    audience,
    authority: binding.authority,
    businessObjectKinds: binding.businessObjectKinds,
    caveatRefs: safeRefsForAudience(
      'binding caveat refs',
      binding.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      binding.createdAtIso,
      nowIso,
    ),
    id: safeRefForAudience('binding id', binding.id, audience),
    principalKind: binding.principalKind,
    principalRef:
      audience === 'public' || audience === 'agent'
        ? 'redacted'
        : safeRefForAudience(
          'binding principal ref',
          binding.principalRef,
          audience,
        ),
    requiresApproval: binding.requiresApproval,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      binding.updatedAtIso,
      nowIso,
    ),
    workroomRef:
      audience === 'public' || audience === 'agent'
        ? 'redacted'
        : safeRefForAudience(
          'binding workroom ref',
          binding.workroomRef,
          audience,
        ),
  })
}

export const projectOmniBusinessObjectWrite = (
  record: OmniBusinessObjectWriteRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniBusinessObjectWriteProjection => {
  assertRecordSafe(record)

  const publicOrAgent = audience === 'public' || audience === 'agent'
  const operatorOrPrivate = audience === 'operator' || audience === 'private'

  return new OmniBusinessObjectWriteProjection({
    appliedReceiptRefs: safeRefsForAudience(
      'applied receipt refs',
      record.appliedReceiptRefs,
      audience,
    ),
    approvalRecorded:
      omniBusinessObjectWriteStateAtLeast(record.state, 'approval_recorded') &&
      record.approvalRefs.length > 0,
    approvalRefs: publicOrAgent
      ? []
      : safeRefsForAudience('approval refs', record.approvalRefs, audience),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'blocker refs',
      record.blockerRefs,
      audience,
    ),
    bindingRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience('binding ref', record.bindingRef, audience),
    businessObjectKind: record.businessObjectKind,
    businessObjectRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience(
        'business object ref',
        record.businessObjectRef,
        audience,
      ),
    caveatRefs: safeRefsForAudience(
      'caveat refs',
      record.caveatRefs,
      audience,
    ),
    closeoutReady:
      record.state === 'closed' &&
      record.closeoutRefs.length > 0 &&
      record.appliedReceiptRefs.length > 0,
    closeoutRefs: publicOrAgent
      ? []
      : safeRefsForAudience('closeout refs', record.closeoutRefs, audience),
    connectorReadReceiptRefs: publicOrAgent
      ? []
      : safeRefsForAudience(
        'connector read receipt refs',
        record.connectorReadReceiptRefs,
        audience,
      ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefForAudience('write id', record.id, audience),
    mutationApplied:
      omniBusinessObjectWriteStateAtLeast(record.state, 'applied') &&
      record.appliedReceiptRefs.length > 0,
    operation: record.operation,
    operatorDiagnosticRefs: operatorOrPrivate
      ? safeRefsForAudience(
        'operator diagnostic refs',
        record.operatorDiagnosticRefs,
        audience,
      )
      : [],
    principalKind: record.principalKind,
    principalRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience('principal ref', record.principalRef, audience),
    proposedChangeRefs: publicOrAgent
      ? []
      : safeRefsForAudience(
        'proposed change refs',
        record.proposedChangeRefs,
        audience,
      ),
    sourceKind: record.sourceKind,
    sourceRefs: publicOrAgent
      ? []
      : safeRefsForAudience('source refs', record.sourceRefs, audience),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience('workroom ref', record.workroomRef, audience),
  })
}

const projectionText = (
  projection:
    | OmniSourceAuthorityBindingProjection
    | OmniBusinessObjectWriteProjection,
): string =>
  'businessObjectRef' in projection
    ? [
      projection.id,
      projection.bindingRef,
      projection.businessObjectRef,
      projection.principalRef,
      projection.workroomRef,
      ...projection.appliedReceiptRefs,
      ...projection.approvalRefs,
      ...projection.blockerRefs,
      ...projection.caveatRefs,
      ...projection.closeoutRefs,
      ...projection.connectorReadReceiptRefs,
      ...projection.evidenceRefs,
      ...projection.operatorDiagnosticRefs,
      ...projection.proposedChangeRefs,
      ...projection.sourceRefs,
    ].join(' ')
    : [
      projection.id,
      projection.principalRef,
      projection.workroomRef,
      ...projection.caveatRefs,
    ].join(' ')

export const omniSourceAuthorityProjectionHasPrivateMaterial = (
  projection:
    | OmniSourceAuthorityBindingProjection
    | OmniBusinessObjectWriteProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    unsafeSourceAuthorityRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE: OmniSourceAuthorityBinding = {
  allowedOperations: ['append', 'create', 'update'],
  allowedSourceKinds: [
    'approval_decision',
    'connector_read',
    'verified_chat_extraction',
  ],
  authority: OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY,
  businessObjectKinds: ['contact', 'company', 'task', 'decision'],
  caveatRefs: ['caveat.source_authority.proposals_until_approved'],
  createdAtIso: '2026-06-19T05:00:00.000Z',
  id: 'source_authority_binding.acme_crm_operator',
  principalKind: 'authorized_user',
  principalRef: 'principal.workroom_owner',
  requiresApproval: true,
  updatedAtIso: '2026-06-19T05:05:00.000Z',
  workroomRef: 'workroom.acme_delivery',
}

export const OMNI_BUSINESS_OBJECT_WRITE_FIXTURE: OmniBusinessObjectWriteRecord =
  {
    appliedReceiptRefs: ['receipt.business_object_write.contact_updated'],
    approvalRefs: ['approval.source_authority.owner_approved'],
    authority: OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY,
    blockerRefs: [],
    bindingRef: 'source_authority_binding.acme_crm_operator',
    businessObjectKind: 'contact',
    businessObjectRef: 'business_object.contact.acme_primary',
    caveatRefs: ['caveat.source_authority.proposal_until_approved'],
    closeoutRefs: ['closeout.business_object_write.contact_updated'],
    connectorReadReceiptRefs: [],
    createdAtIso: '2026-06-19T05:10:00.000Z',
    evidenceRefs: ['evidence.source_authority.write_summary'],
    id: 'business_object_write.acme_contact_1',
    operation: 'update',
    operatorDiagnosticRefs: ['diagnostic.operator.source_authority_route'],
    principalKind: 'authorized_user',
    principalRef: 'principal.workroom_owner',
    proposedChangeRefs: ['proposed_change.contact.title_updated'],
    sourceKind: 'verified_chat_extraction',
    sourceRefs: ['source.workroom.chat_extraction_summary'],
    state: 'closed',
    updatedAtIso: '2026-06-19T05:25:00.000Z',
    workroomRef: 'workroom.acme_delivery',
  }
