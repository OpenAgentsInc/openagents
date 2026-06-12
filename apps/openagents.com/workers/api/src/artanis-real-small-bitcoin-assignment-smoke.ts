import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisRealSmallBitcoinAssignmentSmokeState = S.Literals([
  'passed',
])
export type ArtanisRealSmallBitcoinAssignmentSmokeState =
  typeof ArtanisRealSmallBitcoinAssignmentSmokeState.Type

export class ArtanisRealSmallBitcoinAssignmentSmokeAuthority extends S.Class<ArtanisRealSmallBitcoinAssignmentSmokeAuthority>(
  'ArtanisRealSmallBitcoinAssignmentSmokeAuthority',
)({
  forumPostDeliveryAllowed: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisRealSmallBitcoinAssignmentSmokeRecord extends S.Class<ArtanisRealSmallBitcoinAssignmentSmokeRecord>(
  'ArtanisRealSmallBitcoinAssignmentSmokeRecord',
)({
  acceptedWorkRefs: S.Array(S.String),
  agentRef: S.String,
  amountRef: S.String,
  artifactProofRefs: S.Array(S.String),
  assignmentRef: S.String,
  authority: ArtanisRealSmallBitcoinAssignmentSmokeAuthority,
  createdAtIso: S.String,
  duplicateDispatchEvidenceRefs: S.Array(S.String),
  forumUpdateRefs: S.Array(S.String),
  moneyMovement: S.Literal('real_bitcoin'),
  paymentAuthorityReceiptRef: S.String,
  payoutAttemptRef: S.String,
  payoutIntentRef: S.String,
  payoutTargetApprovalRef: S.String,
  providerRef: S.String,
  pylonJobRef: S.String,
  pylonRef: S.String,
  receiptApiRouteRef: S.String,
  receiptPageRouteRef: S.String,
  reconciliationEventRef: S.String,
  settlementReceiptRef: S.String,
  smokeRef: S.String,
  state: ArtanisRealSmallBitcoinAssignmentSmokeState,
  updatedAtIso: S.String,
  walletReadinessRefs: S.Array(S.String),
}) {}

export class ArtanisRealSmallBitcoinAssignmentSmokeProjection extends S.Class<ArtanisRealSmallBitcoinAssignmentSmokeProjection>(
  'ArtanisRealSmallBitcoinAssignmentSmokeProjection',
)({
  acceptedWorkRefs: S.Array(S.String),
  agentRef: S.String,
  amountLabel: S.String,
  artifactProofRefs: S.Array(S.String),
  assignmentRef: S.String,
  audience: OmniProjectionAudience,
  duplicateDispatchEvidenceRefs: S.Array(S.String),
  forumUpdateRefs: S.Array(S.String),
  moneyMovement: S.Literal('real_bitcoin'),
  paymentAuthorityReceiptRef: S.String,
  payoutAttemptRef: S.String,
  payoutIntentRef: S.String,
  payoutTargetApprovalRef: S.String,
  providerRef: S.String,
  pylonJobRef: S.String,
  pylonRef: S.String,
  receiptApiRouteRef: S.String,
  receiptPageRouteRef: S.String,
  reconciliationEventRef: S.String,
  releaseCreationAllowedByThisRecord: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementReceiptRef: S.String,
  smokeRef: S.String,
  state: ArtanisRealSmallBitcoinAssignmentSmokeState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  walletReadinessRefs: S.Array(S.String),
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisRealSmallBitcoinAssignmentSmokeUnsafe extends S.TaggedErrorClass<ArtanisRealSmallBitcoinAssignmentSmokeUnsafe>()(
  'ArtanisRealSmallBitcoinAssignmentSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_REAL_SMALL_BITCOIN_ASSIGNMENT_SMOKE_NO_AUTHORITY:
  ArtanisRealSmallBitcoinAssignmentSmokeAuthority =
    new ArtanisRealSmallBitcoinAssignmentSmokeAuthority({
      forumPostDeliveryAllowed: false,
      releasePublicationAllowed: false,
      settlementMutationAllowed: false,
      walletSpendAllowed: false,
    })

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const unsafeMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|bearer|cookie|customer|email|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice\.(?!redacted)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|invoice|preimage|raw|secret)|payment\.(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|telemetry|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|dataset|email|invoice|log|model|node|payment|payload|payout|prompt|provider|record|release|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state))/i
const publicUnsafeRefPattern =
  /(^|[.:/_-])(operator|private|raw|secret)([.:/_-]|$)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const refsForAudience = (
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  const safe = uniqueRefs(refs)

  return audience === 'operator' || audience === 'private'
    ? safe
    : safe.filter(ref => !publicUnsafeRefPattern.test(ref))
}

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    containsProviderSecretMaterial(ref) ||
    unsafeMaterialPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason:
        `${label} contains private, secret, wallet, raw payment, raw command, customer, provider, payout target, or timestamp material.`,
    })
  }
}

const assertRequiredRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  if (uniqueRefs(refs).length === 0) {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason: `${label} requires at least one public-safe reference.`,
    })
  }

  assertSafeRefs(label, refs)
}

const assertNoAuthority = (
  authority: ArtanisRealSmallBitcoinAssignmentSmokeAuthority,
): void => {
  if (
    authority.forumPostDeliveryAllowed !== false ||
    authority.releasePublicationAllowed !== false ||
    authority.settlementMutationAllowed !== false ||
    authority.walletSpendAllowed !== false
  ) {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason:
        'Artanis real-assignment smoke evidence cannot publish releases, deliver Forum posts, spend wallet funds, or mutate settlement state.',
    })
  }
}

const assertRecordSafe = (
  record: ArtanisRealSmallBitcoinAssignmentSmokeRecord,
): void => {
  assertNoAuthority(record.authority)

  if (record.agentRef !== 'agent_artanis') {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason: 'Artanis real-assignment smoke must be administered by agent_artanis.',
    })
  }

  if (record.moneyMovement !== 'real_bitcoin' || record.state !== 'passed') {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason: 'Artanis real-assignment smoke must retain passed real-bitcoin movement evidence.',
    })
  }

  assertRequiredRefs('accepted work refs', record.acceptedWorkRefs)
  assertRequiredRefs('artifact proof refs', record.artifactProofRefs)
  assertRequiredRefs('duplicate-dispatch evidence refs', record.duplicateDispatchEvidenceRefs)
  assertRequiredRefs('Forum update refs', record.forumUpdateRefs)
  assertRequiredRefs('wallet readiness refs', record.walletReadinessRefs)
  assertSafeRefs('Artanis real-assignment smoke refs', [
    record.agentRef,
    record.amountRef,
    record.assignmentRef,
    record.paymentAuthorityReceiptRef,
    record.payoutAttemptRef,
    record.payoutIntentRef,
    record.payoutTargetApprovalRef,
    record.providerRef,
    record.pylonJobRef,
    record.pylonRef,
    record.receiptApiRouteRef,
    record.receiptPageRouteRef,
    record.reconciliationEventRef,
    record.settlementReceiptRef,
    record.smokeRef,
    ...record.acceptedWorkRefs,
    ...record.artifactProofRefs,
    ...record.duplicateDispatchEvidenceRefs,
    ...record.forumUpdateRefs,
    ...record.walletReadinessRefs,
  ])

  if (
    stringValues({ ...record, createdAtIso: 'redacted', updatedAtIso: 'redacted' })
      .some(value =>
        containsProviderSecretMaterial(value) ||
        unsafeMaterialPattern.test(value) ||
        rawTimestampPattern.test(value)
      )
  ) {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason:
        'Artanis real-assignment smoke records cannot expose private material or raw timestamps outside timestamp fields.',
    })
  }
}

const assertProjectionSafe = (
  projection: ArtanisRealSmallBitcoinAssignmentSmokeProjection,
): void => {
  const unsafe = stringValues(projection).find(value =>
    containsProviderSecretMaterial(value) ||
    unsafeMaterialPattern.test(value) ||
    rawTimestampPattern.test(value)
  )

  if (unsafe !== undefined) {
    throw new ArtanisRealSmallBitcoinAssignmentSmokeUnsafe({
      reason:
        'Artanis real-assignment smoke projection contains private, secret, wallet, raw payment, raw command, customer, provider, payout target, or timestamp material.',
    })
  }
}

export const issue438ArtanisRealSmallBitcoinAssignmentSmokeRecord = ():
  ArtanisRealSmallBitcoinAssignmentSmokeRecord =>
    new ArtanisRealSmallBitcoinAssignmentSmokeRecord({
      acceptedWorkRefs: ['accepted_work.public.issue_438_artanis_pylon_assignment'],
      agentRef: 'agent_artanis',
      amountRef: 'amount.bitcoin.1000_satoshis',
      artifactProofRefs: [
        'artifact.public.issue_438.artanis_assignment_proof_manifest',
        'proof.public.issue_438.pylon_assignment.accepted_work',
      ],
      assignmentRef: 'assignment.public.issue_438.issue_438_artanis_1780822221',
      authority: ARTANIS_REAL_SMALL_BITCOIN_ASSIGNMENT_SMOKE_NO_AUTHORITY,
      createdAtIso: '2026-06-07T08:50:21.000Z',
      duplicateDispatchEvidenceRefs: [
        'idempotency.public.issue_438.intent.insert_or_ignore',
        'idempotency.public.issue_438.payment_authority.no_duplicate_spend',
      ],
      forumUpdateRefs: [
        'forum.public.artanis.nexus_pylon.release_gate_pass.issue_438_artanis_1780822221',
      ],
      moneyMovement: 'real_bitcoin',
      paymentAuthorityReceiptRef:
        'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
      payoutAttemptRef:
        'payout_attempt.issue_438.issue_438_artanis_1780822221',
      payoutIntentRef: 'payout_intent.issue_438.issue_438_artanis_1780822221',
      payoutTargetApprovalRef:
        'approval.public.issue_438.issue_438_artanis_1780822221',
      providerRef: 'provider.public.mdk_agent_wallet',
      pylonJobRef: 'pylon_job.public.issue_438.issue_438_artanis_1780822221',
      pylonRef: 'pylon.public.issue_438_edge_wallet',
      receiptApiRouteRef:
        'route:/api/public/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
      receiptPageRouteRef:
        'route:/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
      reconciliationEventRef:
        'reconciliation.issue_438.issue_438_artanis_1780822221',
      settlementReceiptRef:
        'receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221',
      smokeRef: 'smoke.public.issue_438.artanis_real_assignment.issue_438_artanis_1780822221',
      state: 'passed',
      updatedAtIso: '2026-06-07T08:50:21.000Z',
      walletReadinessRefs: [
        'wallet_readiness.public.issue_438.pylon.receive_ready',
        'wallet_readiness.public.issue_438.treasury.minimum_satisfied',
      ],
    })

export const projectArtanisRealSmallBitcoinAssignmentSmoke = (
  record: ArtanisRealSmallBitcoinAssignmentSmokeRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisRealSmallBitcoinAssignmentSmokeProjection => {
  assertRecordSafe(record)

  const projection = new ArtanisRealSmallBitcoinAssignmentSmokeProjection({
    acceptedWorkRefs: refsForAudience(record.acceptedWorkRefs, audience),
    agentRef: record.agentRef,
    amountLabel: '0.00001000 bitcoin (1,000 satoshis)',
    artifactProofRefs: refsForAudience(record.artifactProofRefs, audience),
    assignmentRef: record.assignmentRef,
    audience,
    duplicateDispatchEvidenceRefs: refsForAudience(
      record.duplicateDispatchEvidenceRefs,
      audience,
    ),
    forumUpdateRefs: refsForAudience(record.forumUpdateRefs, audience),
    moneyMovement: record.moneyMovement,
    paymentAuthorityReceiptRef: record.paymentAuthorityReceiptRef,
    payoutAttemptRef: record.payoutAttemptRef,
    payoutIntentRef: record.payoutIntentRef,
    payoutTargetApprovalRef: record.payoutTargetApprovalRef,
    providerRef: record.providerRef,
    pylonJobRef: record.pylonJobRef,
    pylonRef: record.pylonRef,
    receiptApiRouteRef: record.receiptApiRouteRef,
    receiptPageRouteRef: record.receiptPageRouteRef,
    reconciliationEventRef: record.reconciliationEventRef,
    releaseCreationAllowedByThisRecord: false,
    releasePublicationAllowed: record.authority.releasePublicationAllowed,
    settlementMutationAllowed: record.authority.settlementMutationAllowed,
    settlementReceiptRef: record.settlementReceiptRef,
    smokeRef: record.smokeRef,
    state: record.state,
    stateLabel:
      'Artanis real small-bitcoin Pylon assignment smoke evidence is retained',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletReadinessRefs: refsForAudience(record.walletReadinessRefs, audience),
    walletSpendAllowed: record.authority.walletSpendAllowed,
  })

  assertProjectionSafe(projection)

  return projection
}
