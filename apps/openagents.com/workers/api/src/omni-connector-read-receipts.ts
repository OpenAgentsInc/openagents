import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

// ---------------------------------------------------------------------------
// Source-authorized connector read receipts (DE-9 / EPIC #5532)
//
// This module is the typed contract/projection-only model for a connector
// read receipt: a public-safe evidence record proving that an agent or
// runtime actually read a specific piece of data from a connector source
// (e.g., Jira ticket, GitHub PR, Salesforce contact). 
//
// This closes the loop for connectorReadReceiptRefs in the source-authority
// model and advances the workrooms.source_authorized_business_objects.v1
// promise by clearing blocker.product_promises.connector_read_receipts_missing.
//
// The promise stays RED: green requires a LIVE source-authorized,
// approval-gated workroom write with a closeout receipt and owner sign-off.
// ---------------------------------------------------------------------------

export const OmniConnectorReadReceiptState = S.Literals([
  'invalidated',
  'recorded',
])
export type OmniConnectorReadReceiptState = typeof OmniConnectorReadReceiptState.Type

export class OmniConnectorReadReceiptUnsafe extends S.TaggedErrorClass<OmniConnectorReadReceiptUnsafe>()(
  'OmniConnectorReadReceiptUnsafe',
  { reason: S.String },
) {}

export class OmniConnectorReadReceiptRecord extends S.Class<OmniConnectorReadReceiptRecord>(
  'OmniConnectorReadReceiptRecord',
)({
  agentRef: S.String,
  connectorRef: S.String,
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  sourcePayloadRef: S.String,
  state: OmniConnectorReadReceiptState,
  workroomRef: S.String,
}) {}

export class OmniConnectorReadReceiptProjection extends S.Class<OmniConnectorReadReceiptProjection>(
  'OmniConnectorReadReceiptProjection',
)({
  agentRef: S.String,
  audience: OmniProjectionAudience,
  connectorRef: S.String,
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  state: OmniConnectorReadReceiptState,
  workroomRef: S.String,
}) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeConnectorReadReceiptRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|contact[_-]?(address|email|name|phone)|cookie|customer[_-]?(email|name|phone|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(contact|key)|provider[_-]?(account|grant|payload|token)|raw[_-]?(contact|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const publicUnsafeRefPattern = /(agent\.|connector\.|source\.payload\.|source\.)/i
const customerUnsafeRefPattern = /(source\.private)/i
const teamUnsafeRefPattern = /(source\.private)/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeConnectorReadReceiptRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniConnectorReadReceiptUnsafe({
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

const recordRefs = (
  record: OmniConnectorReadReceiptRecord,
): ReadonlyArray<string> => [
  record.id,
  record.agentRef,
  record.connectorRef,
  record.sourcePayloadRef,
  record.workroomRef,
  ...record.evidenceRefs,
]

const assertRecordSafe = (record: OmniConnectorReadReceiptRecord): void => {
  assertSafeRefs('connector read receipt refs', recordRefs(record))
}

export const projectOmniConnectorReadReceipt = (
  record: OmniConnectorReadReceiptRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniConnectorReadReceiptProjection => {
  assertRecordSafe(record)

  const publicOrAgent = audience === 'public' || audience === 'agent'

  return new OmniConnectorReadReceiptProjection({
    agentRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience('agent ref', record.agentRef, audience),
    audience,
    connectorRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience('connector ref', record.connectorRef, audience),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: safeRefForAudience('receipt id', record.id, audience),
    state: record.state,
    workroomRef: publicOrAgent
      ? 'redacted'
      : safeRefForAudience('workroom ref', record.workroomRef, audience),
  })
}

const projectionText = (
  projection: OmniConnectorReadReceiptProjection,
): string =>
  [
    projection.id,
    projection.agentRef,
    projection.connectorRef,
    projection.workroomRef,
    ...projection.evidenceRefs,
  ].join(' ')

export const omniConnectorReadReceiptProjectionHasPrivateMaterial = (
  projection: OmniConnectorReadReceiptProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    unsafeConnectorReadReceiptRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const OMNI_CONNECTOR_READ_RECEIPT_FIXTURE: OmniConnectorReadReceiptRecord =
  {
    agentRef: 'agent.workroom_assistant',
    connectorRef: 'connector.github_pull_requests',
    createdAtIso: '2026-06-20T05:10:00.000Z',
    evidenceRefs: ['evidence.connector_read.summary'],
    id: 'receipt.connector_read.gh_pr_123',
    sourcePayloadRef: 'source.payload.gh_pr_json',
    state: 'recorded',
    workroomRef: 'workroom.acme_delivery',
  }
