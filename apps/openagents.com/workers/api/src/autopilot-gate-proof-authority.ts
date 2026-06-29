import { Schema as S } from 'effect'

import {
  PackAClaimRequirementResult,
  PackALedgerClaimKind,
  PackALedgerReceiptKind,
  PackAReceiptRecord,
  checkPackAClaimRequirements,
} from './autopilot-pack-a-ledger'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const AutopilotGateProofClaimKind = S.Literals([
  'autopilot_parent_closeout',
  'm9_live_rate_limit_rotation',
  'm10_overnight_unattended',
  'm14_mvp_exit_review',
  'pack_a_parent_closeout',
  'pack_a_smoke_receipt_authority',
  'w3_student_program_evaluation',
])
export type AutopilotGateProofClaimKind =
  typeof AutopilotGateProofClaimKind.Type

export const AutopilotGateDecisionKind = S.Literals([
  'blocked',
  'deferred',
  'ready_to_close',
])
export type AutopilotGateDecisionKind = typeof AutopilotGateDecisionKind.Type

export class AutopilotGateSmokeReceiptAuthority extends S.Class<AutopilotGateSmokeReceiptAuthority>(
  'AutopilotGateSmokeReceiptAuthority',
)({
  acceptedWorkAuthority: S.Literal(false),
  artifactRefs: S.Array(S.String),
  authorityRef: S.String,
  claimKind: AutopilotGateProofClaimKind,
  generatedAt: S.String,
  issueRefs: S.Array(S.String),
  payoutAuthority: S.Literal(false),
  publicClaimAuthority: S.Literal(false),
  receipt: PackAReceiptRecord,
  sourceCommitRefs: S.Array(S.String),
  verifierRefs: S.Array(S.String),
}) {}

export class AutopilotGateProofDecisionInput extends S.Class<AutopilotGateProofDecisionInput>(
  'AutopilotGateProofDecisionInput',
)({
  acceptedDeferredIssueRefs: S.Array(S.String),
  blockingIssueRefs: S.Array(S.String),
  claimKind: AutopilotGateProofClaimKind,
  closedIssueRefs: S.Array(S.String),
  generatedAt: S.String,
  liveEvidenceRefs: S.Array(S.String),
  receipts: S.Array(PackAReceiptRecord),
  requiredIssueRefs: S.Array(S.String),
  requiredLiveEvidenceRefs: S.Array(S.String),
  smokeAuthorities: S.Array(AutopilotGateSmokeReceiptAuthority),
  sourceCommitRefs: S.Array(S.String),
}) {}

export class AutopilotGateProofDecision extends S.Class<AutopilotGateProofDecision>(
  'AutopilotGateProofDecision',
)({
  blockerRefs: S.Array(S.String),
  claimKind: AutopilotGateProofClaimKind,
  closeAllowed: S.Boolean,
  deferredIssueRefs: S.Array(S.String),
  generatedAt: S.String,
  liveEvidenceRefs: S.Array(S.String),
  missingIssueRefs: S.Array(S.String),
  missingLiveEvidenceRefs: S.Array(S.String),
  missingReceiptKinds: S.Array(PackALedgerReceiptKind),
  receiptAuthorityRefs: S.Array(S.String),
  requiredIssueRefs: S.Array(S.String),
  requiredLiveEvidenceRefs: S.Array(S.String),
  requiredReceiptKinds: S.Array(PackALedgerReceiptKind),
  sourceCommitRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
  status: AutopilotGateDecisionKind,
}) {}

export class AutopilotGateProofUnsafe extends S.TaggedErrorClass<AutopilotGateProofUnsafe>()(
  'AutopilotGateProofUnsafe',
  {
    reason: S.String,
  },
) {}

const claimToLedgerRequirement: Partial<
  Record<AutopilotGateProofClaimKind, PackALedgerClaimKind>
> = {
  m9_live_rate_limit_rotation: 'm9_rate_limit_rotation',
  m10_overnight_unattended: 'm10_overnight_unattended',
  m14_mvp_exit_review: 'm14_exit_gate',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeGateProofPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer\s+|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/\s]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(home|key|material|mnemonic|path|payment|preimage|private|secret|seed)|webhook[_-]?secret|xprv)/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertIso = (label: string, value: string): void => {
  if (!Number.isFinite(Date.parse(value))) {
    throw new AutopilotGateProofUnsafe({
      reason: `${label} must be an ISO timestamp.`,
    })
  }
}

const assertPublicSafeValue = (label: string, value: unknown): void => {
  if (unsafeGateProofPattern.test(JSON.stringify(value) ?? '')) {
    throw new AutopilotGateProofUnsafe({
      reason: `${label} contains private, secret, provider, payment, wallet, raw prompt, raw source, or runner material.`,
    })
  }
}

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref => !safeRefPattern.test(ref) || unsafeGateProofPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new AutopilotGateProofUnsafe({
      reason: `${label} must contain stable public refs only.`,
    })
  }
}

const assertReceiptPublicSafe = (receipt: PackAReceiptRecord): void => {
  assertIso('receipt createdAt', receipt.createdAt)
  assertSafeRefs('receipt refs', [
    receipt.idempotencyKey,
    receipt.receiptRef,
    receipt.subjectRef,
    ...receipt.artifactRefs,
    ...receipt.previousReceiptRefs,
  ])
}

export const validateAutopilotGateSmokeReceiptAuthority = (
  authority: AutopilotGateSmokeReceiptAuthority,
): AutopilotGateSmokeReceiptAuthority => {
  assertIso('generatedAt', authority.generatedAt)
  assertPublicSafeValue('Gate smoke receipt authority', authority)
  assertReceiptPublicSafe(authority.receipt)
  assertSafeRefs('Gate smoke authority refs', [
    authority.authorityRef,
    ...authority.artifactRefs,
    ...authority.issueRefs,
    ...authority.sourceCommitRefs,
    ...authority.verifierRefs,
  ])

  if (
    authority.receipt.kind !== 'smoke_passed' &&
    authority.receipt.kind !== 'smoke_failed'
  ) {
    throw new AutopilotGateProofUnsafe({
      reason:
        'Gate smoke authority records must be backed by a smoke_passed or smoke_failed receipt.',
    })
  }

  if (
    authority.artifactRefs.length === 0 ||
    authority.verifierRefs.length === 0
  ) {
    throw new AutopilotGateProofUnsafe({
      reason:
        'Gate smoke authority records require artifact and verifier refs.',
    })
  }

  return new AutopilotGateSmokeReceiptAuthority({
    ...authority,
    artifactRefs: uniqueRefs(authority.artifactRefs),
    issueRefs: uniqueRefs(authority.issueRefs),
    sourceCommitRefs: uniqueRefs(authority.sourceCommitRefs),
    verifierRefs: uniqueRefs(authority.verifierRefs),
  })
}

const ledgerRequirementFor = (
  input: AutopilotGateProofDecisionInput,
): PackAClaimRequirementResult => {
  const ledgerClaimKind = claimToLedgerRequirement[input.claimKind]

  if (ledgerClaimKind === undefined) {
    return new PackAClaimRequirementResult({
      caveatRefs: [],
      claimKind: 'm14_exit_gate',
      missingReceiptKinds: [],
      ready: true,
      requiredReceiptKinds: [],
    })
  }

  return checkPackAClaimRequirements(ledgerClaimKind, input.receipts)
}

export const evaluateAutopilotGateProofDecision = (
  input: AutopilotGateProofDecisionInput,
): AutopilotGateProofDecision => {
  assertIso('generatedAt', input.generatedAt)
  assertPublicSafeValue('Gate proof decision input', input)
  input.receipts.forEach(assertReceiptPublicSafe)

  const smokeAuthorities = input.smokeAuthorities.map(
    validateAutopilotGateSmokeReceiptAuthority,
  )
  const ledgerRequirement = ledgerRequirementFor(input)
  const acceptedDeferred = new Set(input.acceptedDeferredIssueRefs)
  const closedIssues = new Set(input.closedIssueRefs)
  const liveEvidence = new Set(input.liveEvidenceRefs)
  const authorityClaimKinds = new Set(
    smokeAuthorities.map(authority => authority.claimKind),
  )
  const missingIssueRefs = uniqueRefs(input.requiredIssueRefs).filter(
    issueRef => !closedIssues.has(issueRef) && !acceptedDeferred.has(issueRef),
  )
  const missingLiveEvidenceRefs = uniqueRefs(
    input.requiredLiveEvidenceRefs,
  ).filter(evidenceRef => !liveEvidence.has(evidenceRef))
  const receiptAuthorityRefs = uniqueRefs(
    smokeAuthorities
      .filter(
        authority =>
          authority.claimKind === input.claimKind ||
          input.claimKind === 'pack_a_smoke_receipt_authority',
      )
      .map(authority => authority.authorityRef),
  )
  const missingSmokeAuthority =
    input.claimKind === 'm10_overnight_unattended' ||
    input.claimKind === 'm14_mvp_exit_review' ||
    input.claimKind === 'pack_a_smoke_receipt_authority'
      ? receiptAuthorityRefs.length === 0
      : false
  const blockerRefs = uniqueRefs([
    ...input.blockingIssueRefs.map(
      issueRef => `blocker.gate.issue_open.${issueRef}`,
    ),
    ...missingIssueRefs.map(
      issueRef => `blocker.gate.issue_required.${issueRef}`,
    ),
    ...missingLiveEvidenceRefs.map(
      evidenceRef => `blocker.gate.live_evidence_missing.${evidenceRef}`,
    ),
    ...ledgerRequirement.missingReceiptKinds.map(
      receiptKind => `blocker.gate.receipt_missing.${receiptKind}`,
    ),
    ...(missingSmokeAuthority
      ? ['blocker.gate.smoke_receipt_authority_missing']
      : []),
  ])
  const deferredIssueRefs = uniqueRefs(input.acceptedDeferredIssueRefs)
  const status =
    blockerRefs.length === 0
      ? 'ready_to_close'
      : deferredIssueRefs.length > 0 ||
          input.requiredLiveEvidenceRefs.length >
            input.liveEvidenceRefs.length ||
          !authorityClaimKinds.has(input.claimKind)
        ? 'deferred'
        : 'blocked'

  return new AutopilotGateProofDecision({
    blockerRefs,
    claimKind: input.claimKind,
    closeAllowed: status === 'ready_to_close',
    deferredIssueRefs,
    generatedAt: input.generatedAt,
    liveEvidenceRefs: uniqueRefs(input.liveEvidenceRefs),
    missingIssueRefs,
    missingLiveEvidenceRefs,
    missingReceiptKinds: ledgerRequirement.missingReceiptKinds,
    receiptAuthorityRefs,
    requiredIssueRefs: uniqueRefs(input.requiredIssueRefs),
    requiredLiveEvidenceRefs: uniqueRefs(input.requiredLiveEvidenceRefs),
    requiredReceiptKinds: ledgerRequirement.requiredReceiptKinds,
    sourceCommitRefs: uniqueRefs(input.sourceCommitRefs),
    staleness: liveAtReadStaleness([
      'gate.proof_receipt_appended',
      'gate.issue_status_changed',
      'gate.live_evidence_attached',
      'gate.deferred_boundary_changed',
    ]),
    status,
  })
}
