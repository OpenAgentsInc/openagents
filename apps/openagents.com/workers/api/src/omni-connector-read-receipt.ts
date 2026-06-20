import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

// ---------------------------------------------------------------------------
// Connector Read Receipts
//
// A pure, verifiable record that a connector read happened, providing the
// required source-authority backing for business-object writes. It proves
// WHICH connector was read, WHAT query/resource was accessed, WHEN it was read,
// HOW MANY records were returned, and WHICH workroom the read is scoped to.
// ---------------------------------------------------------------------------

export const OmniConnectorKind = S.Literals([
  'github',
  'hubspot',
  'linear',
  'notion',
  'slack',
])
export type OmniConnectorKind = typeof OmniConnectorKind.Type

export class OmniConnectorReadReceipt extends S.Class<OmniConnectorReadReceipt>(
  'OmniConnectorReadReceipt',
)({
  connectorKind: OmniConnectorKind,
  connectorRef: S.String,
  id: S.String,
  line: S.String,
  queryRef: S.String,
  readAtIso: S.String,
  recordCount: S.Number,
  workroomRef: S.String,
}) {}

export class OmniConnectorReadReceiptProjection extends S.Class<OmniConnectorReadReceiptProjection>(
  'OmniConnectorReadReceiptProjection',
)({
  audience: OmniProjectionAudience,
  connectorKind: OmniConnectorKind,
  connectorRef: S.String,
  id: S.String,
  line: S.String,
  queryRef: S.String,
  readAtDisplay: S.String,
  recordCount: S.Number,
  workroomRef: S.String,
}) {}

export type BuildOmniConnectorReadReceiptInput = Readonly<{
  connectorKind: OmniConnectorKind
  connectorRef: string
  id: string
  queryRef: string
  readAtIso: string
  recordCount: number
  workroomRef: string
}>

export const buildOmniConnectorReadReceipt = (
  input: BuildOmniConnectorReadReceiptInput,
): OmniConnectorReadReceipt => {
  const line = `Connector ${input.connectorKind} (${input.connectorRef}) read ${input.recordCount} records for query ${input.queryRef} into workroom ${input.workroomRef} at ${input.readAtIso}.`

  return new OmniConnectorReadReceipt({
    ...input,
    line,
  })
}

export const validateOmniConnectorReadReceipt = (
  receipt: OmniConnectorReadReceipt,
): boolean => {
  const expectedLine = `Connector ${receipt.connectorKind} (${receipt.connectorRef}) read ${receipt.recordCount} records for query ${receipt.queryRef} into workroom ${receipt.workroomRef} at ${receipt.readAtIso}.`
  return receipt.line === expectedLine && receipt.recordCount >= 0
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return /(connector\.|query\.|workroom\.)/i
  }
  if (audience === 'customer' || audience === 'team') {
    return /(connector\.private|query\.private)/i
  }
  return null
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (!safeRefPattern.test(ref)) {
    return `${label.replaceAll(' ', '_')}.redacted`
  }
  const pattern = audienceUnsafePattern(audience)
  if (pattern !== null && pattern.test(ref)) {
    return `${label.replaceAll(' ', '_')}.redacted`
  }
  return ref
}

export const projectOmniConnectorReadReceipt = (
  receipt: OmniConnectorReadReceipt,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OmniConnectorReadReceiptProjection => {
  const isPublicOrAgent = audience === 'public' || audience === 'agent'
  return new OmniConnectorReadReceiptProjection({
    audience,
    connectorKind: receipt.connectorKind,
    connectorRef: isPublicOrAgent 
      ? 'redacted' 
      : safeRefForAudience('connector ref', receipt.connectorRef, audience),
    id: safeRefForAudience('receipt id', receipt.id, audience),
    line: isPublicOrAgent 
      ? 'redacted' 
      : receipt.line, // The canonical line contains potentially sensitive refs
    queryRef: isPublicOrAgent 
      ? 'redacted' 
      : safeRefForAudience('query ref', receipt.queryRef, audience),
    readAtDisplay: friendlyBlueprintMissionBriefingTime(receipt.readAtIso, nowIso),
    recordCount: receipt.recordCount,
    workroomRef: isPublicOrAgent 
      ? 'redacted' 
      : safeRefForAudience('workroom ref', receipt.workroomRef, audience),
  })
}
