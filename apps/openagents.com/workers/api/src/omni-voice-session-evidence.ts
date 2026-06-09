import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniVoiceSessionAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniVoiceSessionAudience = typeof OmniVoiceSessionAudience.Type

export const OmniVoiceProviderKind = S.Literals([
  'browser_microphone',
  'imported_recording',
  'local_model',
  'phone_bridge',
  'realtime_api',
  'unknown',
])
export type OmniVoiceProviderKind = typeof OmniVoiceProviderKind.Type

export const OmniVoiceCaptureState = S.Literals([
  'not_recorded',
  'recorded',
  'transcribed',
  'redacted',
  'discarded',
])
export type OmniVoiceCaptureState = typeof OmniVoiceCaptureState.Type

export const OmniVoiceTranscriptSpeakerRole = S.Literals([
  'agent',
  'operator',
  'system',
  'unknown',
  'user',
])
export type OmniVoiceTranscriptSpeakerRole =
  typeof OmniVoiceTranscriptSpeakerRole.Type

export const OmniVoiceCommandRouteKind = S.Literals([
  'coding_write',
  'crm_send',
  'customer_order',
  'forum_post',
  'payment',
  'provider_action',
  'public_claim',
  'pylon_setup',
  'runner_launch',
  'site_revision_feedback',
  'unknown',
])
export type OmniVoiceCommandRouteKind =
  typeof OmniVoiceCommandRouteKind.Type

export const OmniVoiceCommandProposalState = S.Literals([
  'approved',
  'blocked',
  'draft',
  'executed',
  'expired',
  'needs_approval',
  'proposed',
  'rejected',
])
export type OmniVoiceCommandProposalState =
  typeof OmniVoiceCommandProposalState.Type

export const OmniVoiceApprovalRequirement = S.Literals([
  'not_required',
  'operator_required',
  'customer_required',
  'admin_required',
  'legal_required',
])
export type OmniVoiceApprovalRequirement =
  typeof OmniVoiceApprovalRequirement.Type

export const OmniVoiceRiskLevel = S.Literals([
  'low',
  'medium',
  'high',
  'critical',
])
export type OmniVoiceRiskLevel = typeof OmniVoiceRiskLevel.Type

export const OmniVoiceSessionAuthorityBoundary = S.Literals([
  'read_only_voice_session_evidence',
])
export type OmniVoiceSessionAuthorityBoundary =
  typeof OmniVoiceSessionAuthorityBoundary.Type

export class OmniVoiceSessionAuthority extends S.Class<OmniVoiceSessionAuthority>(
  'OmniVoiceSessionAuthority',
)({
  authorityBoundary: OmniVoiceSessionAuthorityBoundary,
  noApprovalMutation: S.Boolean,
  noAudioCaptureMutation: S.Boolean,
  noCommandExecution: S.Boolean,
  noPaymentMutation: S.Boolean,
  noProposalMutation: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noTranscriptMutation: S.Boolean,
}) {}

export class OmniVoiceTranscriptSegment extends S.Class<OmniVoiceTranscriptSegment>(
  'OmniVoiceTranscriptSegment',
)({
  confidenceBps: S.Number,
  durationMillis: S.Number,
  evidenceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  segmentRef: S.String,
  sourceRefs: S.Array(S.String),
  speakerRole: OmniVoiceTranscriptSpeakerRole,
  startMillis: S.Number,
  textRef: S.String,
}) {}

export class OmniVoiceCommandProposal extends S.Class<OmniVoiceCommandProposal>(
  'OmniVoiceCommandProposal',
)({
  approvalReceiptRefs: S.Array(S.String),
  approvalRequirement: OmniVoiceApprovalRequirement,
  blockedReasonRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  confidenceBps: S.Number,
  evidenceRefs: S.Array(S.String),
  executionReceiptRefs: S.Array(S.String),
  expiresAtIso: S.NullOr(S.String),
  idempotencyKeyRef: S.String,
  proposalRef: S.String,
  receiptRefs: S.Array(S.String),
  riskLevel: OmniVoiceRiskLevel,
  routeKind: OmniVoiceCommandRouteKind,
  sourceRefs: S.Array(S.String),
  sourceSegmentRefs: S.Array(S.String),
  state: OmniVoiceCommandProposalState,
  summaryRef: S.String,
  titleRef: S.String,
}) {}

export class OmniVoiceSessionReportRecord extends S.Class<OmniVoiceSessionReportRecord>(
  'OmniVoiceSessionReportRecord',
)({
  approvalReceiptRefs: S.Array(S.String),
  authority: OmniVoiceSessionAuthority,
  captureState: OmniVoiceCaptureState,
  caveatRefs: S.Array(S.String),
  commandProposals: S.Array(OmniVoiceCommandProposal),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  languageRef: S.String,
  providerKind: OmniVoiceProviderKind,
  providerRef: S.String,
  receiptRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  sessionRef: S.String,
  sourceRefs: S.Array(S.String),
  transcriptSegments: S.Array(OmniVoiceTranscriptSegment),
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniVoiceTranscriptSegmentProjection extends S.Class<OmniVoiceTranscriptSegmentProjection>(
  'OmniVoiceTranscriptSegmentProjection',
)({
  confidenceBps: S.Number,
  durationMillis: S.Number,
  evidenceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  segmentRef: S.String,
  sourceRefs: S.Array(S.String),
  speakerRole: OmniVoiceTranscriptSpeakerRole,
  startMillis: S.Number,
  textRef: S.String,
}) {}

export class OmniVoiceCommandProposalProjection extends S.Class<OmniVoiceCommandProposalProjection>(
  'OmniVoiceCommandProposalProjection',
)({
  approvalReceiptRefs: S.Array(S.String),
  approvalRequirement: OmniVoiceApprovalRequirement,
  approvalRequired: S.Boolean,
  blockedReasonRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  confidenceBps: S.Number,
  evidenceRefs: S.Array(S.String),
  executionReceiptRefs: S.Array(S.String),
  expiresAtDisplay: S.NullOr(S.String),
  idempotencyKeyRef: S.String,
  proposalRef: S.String,
  receiptRefs: S.Array(S.String),
  riskLabel: S.String,
  riskLevel: OmniVoiceRiskLevel,
  routeKind: OmniVoiceCommandRouteKind,
  routeLabel: S.String,
  sourceRefs: S.Array(S.String),
  sourceSegmentRefs: S.Array(S.String),
  state: OmniVoiceCommandProposalState,
  stateLabel: S.String,
  summaryRef: S.String,
  titleRef: S.String,
}) {}

export class OmniVoiceSessionReportProjection extends S.Class<OmniVoiceSessionReportProjection>(
  'OmniVoiceSessionReportProjection',
)({
  approvalMutationAllowed: S.Boolean,
  approvalReceiptRefs: S.Array(S.String),
  approvedProposalCount: S.Number,
  audience: OmniVoiceSessionAudience,
  audioCaptureMutationAllowed: S.Boolean,
  authority: OmniVoiceSessionAuthority,
  blockedProposalCount: S.Number,
  captureState: OmniVoiceCaptureState,
  captureStateLabel: S.String,
  caveatRefs: S.Array(S.String),
  commandExecutionAllowed: S.Boolean,
  commandProposals: S.Array(OmniVoiceCommandProposalProjection),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  executedProposalCount: S.Number,
  id: S.String,
  languageRef: S.String,
  paymentMutationAllowed: S.Boolean,
  pendingApprovalCount: S.Number,
  proposalMutationAllowed: S.Boolean,
  providerKind: OmniVoiceProviderKind,
  providerMutationAllowed: S.Boolean,
  providerRef: S.String,
  publicClaimUpgradeAllowed: S.Boolean,
  receiptRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  sessionRef: S.String,
  sourceRefs: S.Array(S.String),
  transcriptMutationAllowed: S.Boolean,
  transcriptSegmentCount: S.Number,
  transcriptSegments: S.Array(OmniVoiceTranscriptSegmentProjection),
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

export class OmniVoiceSessionEvidenceUnsafe extends S.TaggedErrorClass<OmniVoiceSessionEvidenceUnsafe>()(
  'OmniVoiceSessionEvidenceUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY:
  OmniVoiceSessionAuthority = {
    authorityBoundary: 'read_only_voice_session_evidence',
    noApprovalMutation: true,
    noAudioCaptureMutation: true,
    noCommandExecution: true,
    noPaymentMutation: true,
    noProposalMutation: true,
    noProviderMutation: true,
    noPublicClaimUpgrade: true,
    noTranscriptMutation: true,
  }

const captureStateLabelByState: Readonly<
  Record<OmniVoiceCaptureState, string>
> = {
  discarded: 'Discarded',
  not_recorded: 'Not recorded',
  recorded: 'Recorded',
  redacted: 'Redacted',
  transcribed: 'Transcribed',
}

const proposalStateLabelByState: Readonly<
  Record<OmniVoiceCommandProposalState, string>
> = {
  approved: 'Approved',
  blocked: 'Blocked',
  draft: 'Draft',
  executed: 'Executed',
  expired: 'Expired',
  needs_approval: 'Needs approval',
  proposed: 'Proposed',
  rejected: 'Rejected',
}

const routeLabelByRoute: Readonly<Record<OmniVoiceCommandRouteKind, string>> =
  {
    coding_write: 'Coding write',
    crm_send: 'CRM send',
    customer_order: 'Customer order',
    forum_post: 'Forum post',
    payment: 'Payment',
    provider_action: 'Provider action',
    public_claim: 'Public claim',
    pylon_setup: 'Pylon setup',
    runner_launch: 'Runner launch',
    site_revision_feedback: 'Site revision feedback',
    unknown: 'Unknown route',
  }

const riskLabelByRisk: Readonly<Record<OmniVoiceRiskLevel, string>> = {
  critical: 'Critical risk',
  high: 'High risk',
  low: 'Low risk',
  medium: 'Medium risk',
}

const approvalRequiredRoutes =
  new Set<OmniVoiceCommandRouteKind>([
    'coding_write',
    'crm_send',
    'payment',
    'provider_action',
    'public_claim',
    'pylon_setup',
    'runner_launch',
  ])

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeVoiceRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|audio[_-]?bytes|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|contact[_-]?(email|name|phone)|cookie|customer[_-]?(email|name|phone|record|value)|email[_-]?(address|body)|full[_-]?(name|transcript)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|phone[_-]?number|preimage|private[_-]?(archive|audio|customer|key|source|transcript|wallet)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(audio|auth|connector|customer|email|invoice|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|transcript|voice|webhook)|recording[_-]?bytes|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|transcript[_-]?text|voice[_-]?payload|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicAudiencePattern =
  /(approval\.private|caveat\.private|evidence\.private|idempotency\.private|language\.private|proposal\.private|provider\.|receipt\.private|redaction\.private|segment\.private|session\.private|source\.private|summary\.private|text\.private|title\.private|workroom\.private)/i
const agentAudiencePattern =
  /(approval\.private|idempotency\.private|provider\.private|receipt\.private|segment\.private|session\.private|text\.private)/i
const customerAudiencePattern =
  /(approval\.private|idempotency\.private|provider\.private|receipt\.private|text\.private)/i

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
      unsafeVoiceRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: `${label} contains private name, contact info, provider payload, wallet, payment, raw voice/transcript, secret, private repo, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniVoiceSessionAudience,
): RegExp | null => {
  switch (audience) {
    case 'agent':
      return agentAudiencePattern
    case 'customer':
      return customerAudiencePattern
    case 'public':
      return publicAudiencePattern
    case 'operator':
    case 'team':
      return null
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniVoiceSessionAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniVoiceSessionAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniVoiceSessionAuthority,
): void => {
  if (
    authority.noApprovalMutation !== true ||
    authority.noAudioCaptureMutation !== true ||
    authority.noCommandExecution !== true ||
    authority.noPaymentMutation !== true ||
    authority.noProposalMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noTranscriptMutation !== true
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Voice session evidence is read-only and cannot capture audio, mutate transcripts, mutate proposals, approve, execute commands, spend, mutate providers, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string | null): void => {
  if (iso === null) {
    return
  }

  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertConfidenceBps = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0 || value > 10_000) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: `${label} must be an integer from 0 to 10000 basis points.`,
    })
  }
}

const assertTranscriptSegment = (
  segment: OmniVoiceTranscriptSegment,
): void => {
  assertConfidenceBps('Transcript segment confidenceBps', segment.confidenceBps)
  assertNonNegativeInteger('Transcript segment startMillis', segment.startMillis)
  assertNonNegativeInteger(
    'Transcript segment durationMillis',
    segment.durationMillis,
  )
  assertSafeRefs('Transcript segment refs', [
    segment.segmentRef,
    segment.textRef,
    ...segment.evidenceRefs,
    ...segment.redactionPolicyRefs,
    ...segment.sourceRefs,
  ])

  if (segment.sourceRefs.length === 0 || segment.evidenceRefs.length === 0) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Transcript segments require source refs and transcript evidence refs.',
    })
  }
}

const assertProposal = (
  proposal: OmniVoiceCommandProposal,
  segmentRefs: ReadonlySet<string>,
): void => {
  assertConfidenceBps('Voice command proposal confidenceBps', proposal.confidenceBps)
  assertValidIso('Voice command proposal expiresAtIso', proposal.expiresAtIso)
  assertSafeRefs('Voice command proposal refs', [
    proposal.idempotencyKeyRef,
    proposal.proposalRef,
    proposal.summaryRef,
    proposal.titleRef,
    ...proposal.approvalReceiptRefs,
    ...proposal.blockedReasonRefs,
    ...proposal.caveatRefs,
    ...proposal.evidenceRefs,
    ...proposal.executionReceiptRefs,
    ...proposal.receiptRefs,
    ...proposal.sourceRefs,
    ...proposal.sourceSegmentRefs,
  ])

  if (
    ['proposed', 'needs_approval', 'approved', 'executed'].includes(
      proposal.state,
    ) &&
    (proposal.sourceSegmentRefs.length === 0 || proposal.evidenceRefs.length === 0)
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Voice command proposals require source transcript segments and evidence refs before they can be proposed, approved, or executed.',
    })
  }

  const unknownSegmentRef = proposal.sourceSegmentRefs.find(
    ref => !segmentRefs.has(ref),
  )

  if (unknownSegmentRef !== undefined) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Voice command proposal sourceSegmentRefs must point at transcript segments in the same session report.',
    })
  }

  if (
    ['high', 'critical'].includes(proposal.riskLevel) &&
    proposal.approvalRequirement === 'not_required'
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: 'High and critical voice command proposals require approval.',
    })
  }

  if (
    approvalRequiredRoutes.has(proposal.routeKind) &&
    proposal.approvalRequirement === 'not_required'
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Voice proposals for writes, sends, payments, provider actions, public claims, pylon setup, and runner launches require approval.',
    })
  }

  if (
    ['approved', 'executed'].includes(proposal.state) &&
    proposal.approvalRequirement !== 'not_required' &&
    proposal.approvalReceiptRefs.length === 0
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: 'Approved or executed voice proposals require approval receipts.',
    })
  }

  if (
    proposal.state === 'executed' &&
    proposal.executionReceiptRefs.length === 0
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: 'Executed voice proposals require execution receipts.',
    })
  }

  if (proposal.state === 'blocked' && proposal.blockedReasonRefs.length === 0) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: 'Blocked voice proposals require blocked reason refs.',
    })
  }

  if (proposal.state === 'expired' && proposal.expiresAtIso === null) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason: 'Expired voice proposals require an expiry timestamp.',
    })
  }
}

const assertSessionReport = (record: OmniVoiceSessionReportRecord): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('Voice session createdAtIso', record.createdAtIso)
  assertValidIso('Voice session updatedAtIso', record.updatedAtIso)
  assertSafeRefs('Voice session refs', [
    record.id,
    record.languageRef,
    record.providerRef,
    record.sessionRef,
    record.workroomRef,
    ...record.approvalReceiptRefs,
    ...record.caveatRefs,
    ...record.evidenceRefs,
    ...record.receiptRefs,
    ...record.redactionPolicyRefs,
    ...record.sourceRefs,
  ])

  if (record.sourceRefs.length === 0 || record.redactionPolicyRefs.length === 0) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Voice session reports require source refs and redaction policy refs.',
    })
  }

  if (
    ['transcribed', 'redacted'].includes(record.captureState) &&
    record.transcriptSegments.length === 0
  ) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Transcribed or redacted voice sessions require transcript segments.',
    })
  }

  record.transcriptSegments.forEach(assertTranscriptSegment)

  const segmentRefs = new Set(
    record.transcriptSegments.map(segment => segment.segmentRef),
  )

  record.commandProposals.forEach(proposal =>
    assertProposal(proposal, segmentRefs),
  )
}

const durationLabel = (elapsedMs: number): string => {
  const minuteMs = 60_000
  const hourMs = 60 * minuteMs
  const dayMs = 24 * hourMs
  const durationMs = Math.max(0, elapsedMs)

  if (durationMs < minuteMs) {
    return 'less than 1 minute'
  }

  if (durationMs < hourMs) {
    const minutes = Math.floor(durationMs / minuteMs)

    return minutes === 1 ? '1 minute' : `${minutes} minutes`
  }

  if (durationMs < dayMs) {
    const hours = Math.floor(durationMs / hourMs)

    return hours === 1 ? '1 hour' : `${hours} hours`
  }

  const days = Math.floor(durationMs / dayMs)

  return days === 1 ? '1 day' : `${days} days`
}

const expiryDisplay = (expiresAtIso: string | null, nowIso: string): string | null => {
  if (expiresAtIso === null) {
    return null
  }

  const expiresAt = Date.parse(expiresAtIso)
  const now = Date.parse(nowIso)

  if (!Number.isFinite(expiresAt) || !Number.isFinite(now)) {
    return 'Recently'
  }

  return expiresAt <= now
    ? `Expired ${durationLabel(now - expiresAt)} ago`
    : `Expires in ${durationLabel(expiresAt - now)}`
}

const transcriptSegmentProjection = (
  segment: OmniVoiceTranscriptSegment,
  audience: OmniVoiceSessionAudience,
): OmniVoiceTranscriptSegmentProjection => ({
  confidenceBps: segment.confidenceBps,
  durationMillis: segment.durationMillis,
  evidenceRefs: refsForAudience(
    'Voice transcript evidence refs',
    segment.evidenceRefs,
    audience,
  ),
  redactionPolicyRefs: refsForAudience(
    'Voice transcript redaction refs',
    segment.redactionPolicyRefs,
    audience,
  ),
  segmentRef: primaryRefForAudience(
    'Voice transcript segment refs',
    segment.segmentRef,
    audience,
    'segment.redacted',
  ),
  sourceRefs: refsForAudience(
    'Voice transcript source refs',
    segment.sourceRefs,
    audience,
  ),
  speakerRole: segment.speakerRole,
  startMillis: segment.startMillis,
  textRef: primaryRefForAudience(
    'Voice transcript text refs',
    segment.textRef,
    audience,
    'text.redacted',
  ),
})

const proposalProjection = (
  proposal: OmniVoiceCommandProposal,
  audience: OmniVoiceSessionAudience,
  nowIso: string,
): OmniVoiceCommandProposalProjection => ({
  approvalReceiptRefs: refsForAudience(
    'Voice proposal approval receipt refs',
    proposal.approvalReceiptRefs,
    audience,
  ),
  approvalRequirement: proposal.approvalRequirement,
  approvalRequired: proposal.approvalRequirement !== 'not_required',
  blockedReasonRefs: refsForAudience(
    'Voice proposal blocked reason refs',
    proposal.blockedReasonRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Voice proposal caveat refs',
    proposal.caveatRefs,
    audience,
  ),
  confidenceBps: proposal.confidenceBps,
  evidenceRefs: refsForAudience(
    'Voice proposal evidence refs',
    proposal.evidenceRefs,
    audience,
  ),
  executionReceiptRefs: refsForAudience(
    'Voice proposal execution receipt refs',
    proposal.executionReceiptRefs,
    audience,
  ),
  expiresAtDisplay: expiryDisplay(proposal.expiresAtIso, nowIso),
  idempotencyKeyRef: primaryRefForAudience(
    'Voice proposal idempotency refs',
    proposal.idempotencyKeyRef,
    audience,
    'idempotency.redacted',
  ),
  proposalRef: primaryRefForAudience(
    'Voice proposal refs',
    proposal.proposalRef,
    audience,
    'proposal.redacted',
  ),
  receiptRefs: refsForAudience(
    'Voice proposal receipt refs',
    proposal.receiptRefs,
    audience,
  ),
  riskLabel: riskLabelByRisk[proposal.riskLevel],
  riskLevel: proposal.riskLevel,
  routeKind: proposal.routeKind,
  routeLabel: routeLabelByRoute[proposal.routeKind],
  sourceRefs: refsForAudience(
    'Voice proposal source refs',
    proposal.sourceRefs,
    audience,
  ),
  sourceSegmentRefs: refsForAudience(
    'Voice proposal source segment refs',
    proposal.sourceSegmentRefs,
    audience,
  ),
  state: proposal.state,
  stateLabel: proposalStateLabelByState[proposal.state],
  summaryRef: primaryRefForAudience(
    'Voice proposal summary refs',
    proposal.summaryRef,
    audience,
    'summary.redacted',
  ),
  titleRef: primaryRefForAudience(
    'Voice proposal title refs',
    proposal.titleRef,
    audience,
    'title.redacted',
  ),
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
  projection: OmniVoiceSessionReportProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    unsafeVoiceRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const projectOmniVoiceSessionReport = (
  record: OmniVoiceSessionReportRecord,
  audience: OmniVoiceSessionAudience,
  nowIso: string,
): OmniVoiceSessionReportProjection => {
  assertSessionReport(record)

  const commandProposals = record.commandProposals.map(proposal =>
    proposalProjection(proposal, audience, nowIso),
  )

  const projection: OmniVoiceSessionReportProjection = {
    approvalMutationAllowed: false,
    approvalReceiptRefs: refsForAudience(
      'Voice session approval receipt refs',
      record.approvalReceiptRefs,
      audience,
    ),
    approvedProposalCount: commandProposals.filter(
      proposal => proposal.state === 'approved',
    ).length,
    audience,
    audioCaptureMutationAllowed: false,
    authority: OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
    blockedProposalCount: commandProposals.filter(
      proposal => proposal.state === 'blocked',
    ).length,
    captureState: record.captureState,
    captureStateLabel: captureStateLabelByState[record.captureState],
    caveatRefs: refsForAudience(
      'Voice session caveat refs',
      record.caveatRefs,
      audience,
    ),
    commandExecutionAllowed: false,
    commandProposals,
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: refsForAudience(
      'Voice session evidence refs',
      record.evidenceRefs,
      audience,
    ),
    executedProposalCount: commandProposals.filter(
      proposal => proposal.state === 'executed',
    ).length,
    id: primaryRefForAudience(
      'Voice session id refs',
      record.id,
      audience,
      'voice_session_report.redacted',
    ),
    languageRef: primaryRefForAudience(
      'Voice session language refs',
      record.languageRef,
      audience,
      'language.redacted',
    ),
    paymentMutationAllowed: false,
    pendingApprovalCount: commandProposals.filter(
      proposal => proposal.state === 'needs_approval',
    ).length,
    proposalMutationAllowed: false,
    providerKind: record.providerKind,
    providerMutationAllowed: false,
    providerRef: primaryRefForAudience(
      'Voice session provider refs',
      record.providerRef,
      audience,
      'provider_ref.redacted',
    ),
    publicClaimUpgradeAllowed: false,
    receiptRefs: refsForAudience(
      'Voice session receipt refs',
      record.receiptRefs,
      audience,
    ),
    redactionPolicyRefs: refsForAudience(
      'Voice session redaction policy refs',
      record.redactionPolicyRefs,
      audience,
    ),
    sessionRef: primaryRefForAudience(
      'Voice session refs',
      record.sessionRef,
      audience,
      'session.redacted',
    ),
    sourceRefs: refsForAudience(
      'Voice session source refs',
      record.sourceRefs,
      audience,
    ),
    transcriptMutationAllowed: false,
    transcriptSegmentCount: record.transcriptSegments.length,
    transcriptSegments: record.transcriptSegments.map(segment =>
      transcriptSegmentProjection(segment, audience),
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRef: primaryRefForAudience(
      'Voice session workroom refs',
      record.workroomRef,
      audience,
      'workroom.redacted',
    ),
  }

  if (projectionHasPrivateMaterial(projection)) {
    throw new OmniVoiceSessionEvidenceUnsafe({
      reason:
        'Voice session projection contains private name, contact info, provider payload, wallet, payment, raw voice/transcript, secret, private repo, raw timestamp, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniVoiceTranscriptSegment = (
  overrides: Partial<OmniVoiceTranscriptSegment> = {},
): OmniVoiceTranscriptSegment => ({
  confidenceBps: 9200,
  durationMillis: 11_000,
  evidenceRefs: ['evidence.public.voice_segment_1_transcript'],
  redactionPolicyRefs: ['redaction.public.refs_only'],
  segmentRef: 'segment.public.voice_001',
  sourceRefs: ['source.public.browser_audio_summary'],
  speakerRole: 'user',
  startMillis: 0,
  textRef: 'text.public.request_revision_summary',
  ...overrides,
})

export const exampleOmniVoiceCommandProposal = (
  overrides: Partial<OmniVoiceCommandProposal> = {},
): OmniVoiceCommandProposal => ({
  approvalReceiptRefs: [],
  approvalRequirement: 'operator_required',
  blockedReasonRefs: [],
  caveatRefs: ['caveat.public.voice_is_evidence_only'],
  confidenceBps: 8900,
  evidenceRefs: ['evidence.public.voice_command_route'],
  executionReceiptRefs: [],
  expiresAtIso: '2026-06-06T23:30:00.000Z',
  idempotencyKeyRef: 'idempotency.public.voice_site_revision_feedback',
  proposalRef: 'proposal.public.voice_site_revision_feedback',
  receiptRefs: ['receipt.public.voice_session_report'],
  riskLevel: 'medium',
  routeKind: 'site_revision_feedback',
  sourceRefs: ['source.public.browser_audio_summary'],
  sourceSegmentRefs: ['segment.public.voice_001'],
  state: 'needs_approval',
  summaryRef: 'summary.public.voice_revision_feedback',
  titleRef: 'title.public.voice_revision_feedback',
  ...overrides,
})

export const exampleOmniVoiceSessionReport = (
  overrides: Partial<OmniVoiceSessionReportRecord> = {},
): OmniVoiceSessionReportRecord => ({
  approvalReceiptRefs: [],
  authority: OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
  captureState: 'transcribed',
  caveatRefs: ['caveat.public.voice_evidence_only'],
  commandProposals: [
    exampleOmniVoiceCommandProposal(),
    exampleOmniVoiceCommandProposal({
      approvalReceiptRefs: ['approval.public.operator_approved'],
      confidenceBps: 9400,
      evidenceRefs: ['evidence.public.forum_post_route'],
      executionReceiptRefs: ['execution.public.forum_post_created'],
      expiresAtIso: null,
      idempotencyKeyRef: 'idempotency.public.forum_post',
      proposalRef: 'proposal.public.forum_post',
      receiptRefs: ['receipt.public.forum_post_created'],
      riskLevel: 'low',
      routeKind: 'forum_post',
      state: 'executed',
      summaryRef: 'summary.public.forum_post',
      titleRef: 'title.public.forum_post',
    }),
  ],
  createdAtIso: '2026-06-06T22:00:00.000Z',
  evidenceRefs: ['evidence.public.voice_session_report'],
  id: 'voice_session.public.otec_revision_voice_1',
  languageRef: 'language.public.en',
  providerKind: 'realtime_api',
  providerRef: 'provider.public.openai_realtime_redacted',
  receiptRefs: ['receipt.public.voice_session_report'],
  redactionPolicyRefs: ['redaction.public.refs_only_transcript'],
  sessionRef: 'session.public.otec_voice_1',
  sourceRefs: ['source.public.browser_audio_summary'],
  transcriptSegments: [
    exampleOmniVoiceTranscriptSegment(),
    exampleOmniVoiceTranscriptSegment({
      confidenceBps: 8800,
      durationMillis: 8_000,
      evidenceRefs: ['evidence.public.voice_segment_2_transcript'],
      segmentRef: 'segment.public.voice_002',
      sourceRefs: ['source.public.browser_audio_summary'],
      speakerRole: 'agent',
      startMillis: 11_000,
      textRef: 'text.public.agent_confirmed_draft_only',
    }),
  ],
  updatedAtIso: '2026-06-06T22:25:00.000Z',
  workroomRef: 'workroom.public.otec_site_revision',
  ...overrides,
})
