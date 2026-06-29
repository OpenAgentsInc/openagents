import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisPylonProofTraceState = S.Literals([
  'blocked',
  'complete',
  'partial',
])
export type ArtanisPylonProofTraceState =
  typeof ArtanisPylonProofTraceState.Type

export const ArtanisPylonProofTracePylonEventKind = S.Literals([
  'artifact_proof_metadata',
  'assignment_acceptance',
  'assignment_progress',
  'payment_receipt',
  'settlement_status',
  'worker_closeout',
])
export type ArtanisPylonProofTracePylonEventKind =
  typeof ArtanisPylonProofTracePylonEventKind.Type

export const ArtanisPylonProofTraceMovementMode = S.Literals([
  'real_bitcoin',
  'simulation',
  'unknown',
])
export type ArtanisPylonProofTraceMovementMode =
  typeof ArtanisPylonProofTraceMovementMode.Type

export class ArtanisPylonProofTraceAuthority extends S.Class<ArtanisPylonProofTraceAuthority>(
  'ArtanisPylonProofTraceAuthority',
)({
  pylonMutationAllowed: S.Boolean,
  receiptMutationAllowed: S.Boolean,
  releasePublicationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisPylonProofTraceDispatchEvidence extends S.Class<ArtanisPylonProofTraceDispatchEvidence>(
  'ArtanisPylonProofTraceDispatchEvidence',
)({
  artanisRunRef: S.String,
  assignmentRef: S.String,
  evidenceRefs: S.Array(S.String),
  settlementIntentRef: S.NullOr(S.String),
}) {}

export class ArtanisPylonProofTracePylonEvent extends S.Class<ArtanisPylonProofTracePylonEvent>(
  'ArtanisPylonProofTracePylonEvent',
)({
  accepted: S.optionalKey(S.Boolean),
  assignmentRef: S.String,
  eventKind: ArtanisPylonProofTracePylonEventKind,
  evidenceRefs: S.Array(S.String),
  pylonRef: S.String,
  status: S.String,
}) {}

export class ArtanisPylonProofTraceReceiptEvidence extends S.Class<ArtanisPylonProofTraceReceiptEvidence>(
  'ArtanisPylonProofTraceReceiptEvidence',
)({
  assignmentRef: S.String,
  evidenceRefs: S.Array(S.String),
  movementMode: ArtanisPylonProofTraceMovementMode,
  pylonRef: S.NullOr(S.String),
  realBitcoinMoved: S.Boolean,
  receiptRef: S.String,
  settlementStateLabel: S.String,
  terminalSettlementObserved: S.Boolean,
}) {}

export class ArtanisPylonProofTraceRecord extends S.Class<ArtanisPylonProofTraceRecord>(
  'ArtanisPylonProofTraceRecord',
)({
  assignmentRef: S.String,
  createdAtIso: S.String,
  dispatch: ArtanisPylonProofTraceDispatchEvidence,
  pylonEvents: S.Array(ArtanisPylonProofTracePylonEvent),
  receipt: S.NullOr(ArtanisPylonProofTraceReceiptEvidence),
  releaseEvidenceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisPylonProofTraceProjection extends S.Class<ArtanisPylonProofTraceProjection>(
  'ArtanisPylonProofTraceProjection',
)({
  acceptedWorkObserved: S.Boolean,
  artifactProofObserved: S.Boolean,
  assignmentRef: S.String,
  audience: OmniProjectionAudience,
  authority: ArtanisPylonProofTraceAuthority,
  blockerRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dispatchObserved: S.Boolean,
  distinctPylonRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  missingEvidenceRefs: S.Array(S.String),
  paymentEvidenceObserved: S.Boolean,
  publicReceiptObserved: S.Boolean,
  realBitcoinMoved: S.Boolean,
  releaseEvidenceRefs: S.Array(S.String),
  sameAssignmentIdObserved: S.Boolean,
  settlementEvidenceObserved: S.Boolean,
  state: ArtanisPylonProofTraceState,
  stateLabel: S.String,
  terminalSettlementObserved: S.Boolean,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisPylonProofTraceUnsafe extends S.TaggedErrorClass<ArtanisPylonProofTraceUnsafe>()(
  'ArtanisPylonProofTraceUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_PYLON_PROOF_TRACE_NO_AUTHORITY:
  ArtanisPylonProofTraceAuthority =
    new ArtanisPylonProofTraceAuthority({
      pylonMutationAllowed: false,
      receiptMutationAllowed: false,
      releasePublicationAllowed: false,
      settlementMutationAllowed: false,
      walletSpendAllowed: false,
    })

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#{}-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|bolt11|bolt12|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)?|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw)|preimage|private[_-]?(archive|channel|customer|key|prompt|source|telemetry|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|payout|prompt|provider|record|release|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|spend))/i
const publicUnsafePattern =
  /(^|[.:/_-])(operator|private|raw|secret)([.:/_-]|$)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    containsProviderSecretMaterial(ref) ||
    unsafeRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisPylonProofTraceUnsafe({
      reason:
        `${label} contains private, secret, wallet, payment, raw timestamp, provider, payout target, customer, runner, or credential material.`,
    })
  }
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

const refsForAudience = (
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  const safe = uniqueRefs(refs)

  if (audience === 'operator' || audience === 'private') {
    return safe
  }

  return safe.filter(ref => !publicUnsafePattern.test(ref))
}

const eventRefs = (
  events: ReadonlyArray<ArtanisPylonProofTracePylonEvent>,
  kind: ArtanisPylonProofTracePylonEventKind,
): ReadonlyArray<string> =>
  uniqueRefs(
    events
      .filter(event => event.eventKind === kind)
      .flatMap(event => event.evidenceRefs),
  )

const requiredMissingEvidenceRefs = (
  checks: Readonly<Record<string, boolean>>,
): ReadonlyArray<string> =>
  Object.entries(checks).flatMap(([name, observed]) =>
    observed ? [] : [`missing.public.artanis_pylon_proof.${name}`]
  )

const stateLabelByState: Readonly<Record<ArtanisPylonProofTraceState, string>> = {
  blocked: 'Blocked',
  complete: 'Complete',
  partial: 'Partial',
}

export const projectArtanisPylonProofTrace = (
  record: ArtanisPylonProofTraceRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisPylonProofTraceProjection => {
  assertSafeRefs('Artanis/Pylon proof trace assignment ref', [
    record.assignmentRef,
  ])
  assertSafeRefs('Artanis/Pylon proof trace dispatch refs', [
    record.dispatch.artanisRunRef,
    record.dispatch.assignmentRef,
    ...(record.dispatch.settlementIntentRef === null
      ? []
      : [record.dispatch.settlementIntentRef]),
    ...record.dispatch.evidenceRefs,
  ])
  assertSafeRefs(
    'Artanis/Pylon proof trace event refs',
    record.pylonEvents.flatMap(event => [
      event.assignmentRef,
      event.pylonRef,
      event.status,
      ...event.evidenceRefs,
    ]),
  )
  assertSafeRefs(
    'Artanis/Pylon proof trace receipt refs',
    record.receipt === null
      ? []
      : [
      record.receipt.assignmentRef,
      ...(record.receipt.pylonRef === null ? [] : [record.receipt.pylonRef]),
      record.receipt.receiptRef,
      ...record.receipt.evidenceRefs,
    ],
  )
  assertSafeRefs(
    'Artanis/Pylon proof trace release refs',
    record.releaseEvidenceRefs,
  )

  const observedAssignmentRefs = [
    record.dispatch.assignmentRef,
    ...record.pylonEvents.map(event => event.assignmentRef),
    ...(record.receipt === null ? [] : [record.receipt.assignmentRef]),
  ]
  const assignmentRefs = uniqueRefs(observedAssignmentRefs)
  const sameAssignmentIdObserved =
    observedAssignmentRefs.length >= 3 &&
    observedAssignmentRefs.every(ref => ref === record.assignmentRef)

  const dispatchObserved =
    record.dispatch.assignmentRef === record.assignmentRef &&
    record.dispatch.evidenceRefs.length > 0
  const acceptedWorkObserved = record.pylonEvents.some(event =>
    event.assignmentRef === record.assignmentRef &&
    event.eventKind === 'assignment_acceptance' &&
    event.accepted === true &&
    event.status !== 'rejected'
  )
  const artifactProofObserved =
    eventRefs(record.pylonEvents, 'artifact_proof_metadata').length > 0
  const paymentEvidenceObserved =
    eventRefs(record.pylonEvents, 'payment_receipt').length > 0
  const settlementEvidenceObserved =
    eventRefs(record.pylonEvents, 'settlement_status').length > 0 ||
    record.receipt?.terminalSettlementObserved === true
  const publicReceiptObserved =
    record.receipt !== null &&
    record.receipt.assignmentRef === record.assignmentRef &&
    record.receipt.receiptRef.trim() !== ''
  const realBitcoinMoved =
    record.receipt?.movementMode === 'real_bitcoin' &&
    record.receipt.realBitcoinMoved
  const terminalSettlementObserved =
    record.receipt?.terminalSettlementObserved === true

  const missingEvidenceRefs = requiredMissingEvidenceRefs({
    accepted_work: acceptedWorkObserved,
    artifact_proof: artifactProofObserved,
    dispatch: dispatchObserved,
    payment_evidence: paymentEvidenceObserved,
    public_receipt: publicReceiptObserved,
    real_bitcoin_movement: realBitcoinMoved,
    same_assignment_id: sameAssignmentIdObserved,
    settlement_evidence: settlementEvidenceObserved,
    terminal_settlement: terminalSettlementObserved,
  })

  const assignmentMismatch =
    assignmentRefs.some(ref => ref !== record.assignmentRef) ||
    record.receipt?.assignmentRef !== undefined &&
      record.receipt.assignmentRef !== record.assignmentRef

  const blockerRefs = uniqueRefs([
    ...missingEvidenceRefs,
    ...(assignmentMismatch
      ? ['blocker.public.artanis_pylon_proof.assignment_ref_mismatch']
      : []),
    ...(record.receipt !== null &&
        record.receipt.movementMode === 'simulation'
      ? ['blocker.public.artanis_pylon_proof.simulation_only_receipt']
      : []),
  ])

  const complete =
    sameAssignmentIdObserved &&
    dispatchObserved &&
    acceptedWorkObserved &&
    artifactProofObserved &&
    paymentEvidenceObserved &&
    settlementEvidenceObserved &&
    publicReceiptObserved &&
    realBitcoinMoved &&
    terminalSettlementObserved &&
    !assignmentMismatch

  const state: ArtanisPylonProofTraceState = complete
    ? 'complete'
    : assignmentMismatch || !dispatchObserved
      ? 'blocked'
      : 'partial'

  const pylonRefs = uniqueRefs([
    ...record.pylonEvents.map(event => event.pylonRef),
    ...(record.receipt?.pylonRef === null || record.receipt?.pylonRef === undefined
      ? []
      : [record.receipt.pylonRef]),
  ])
  const evidenceRefs = refsForAudience(
    [
      record.assignmentRef,
      record.dispatch.artanisRunRef,
      ...(record.dispatch.settlementIntentRef === null
        ? []
        : [record.dispatch.settlementIntentRef]),
      ...record.dispatch.evidenceRefs,
      ...record.pylonEvents.flatMap(event => event.evidenceRefs),
      ...(record.receipt === null
        ? []
        : [record.receipt.receiptRef, ...record.receipt.evidenceRefs]),
    ],
    audience,
  )

  return new ArtanisPylonProofTraceProjection({
    acceptedWorkObserved,
    artifactProofObserved,
    assignmentRef: record.assignmentRef,
    audience,
    authority: ARTANIS_PYLON_PROOF_TRACE_NO_AUTHORITY,
    blockerRefs: refsForAudience(blockerRefs, audience),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    dispatchObserved,
    distinctPylonRefs: refsForAudience(pylonRefs, audience),
    evidenceRefs,
    missingEvidenceRefs: refsForAudience(missingEvidenceRefs, audience),
    paymentEvidenceObserved,
    publicReceiptObserved,
    realBitcoinMoved,
    releaseEvidenceRefs: refsForAudience(record.releaseEvidenceRefs, audience),
    sameAssignmentIdObserved,
    settlementEvidenceObserved,
    state,
    stateLabel: stateLabelByState[state],
    terminalSettlementObserved,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  })
}

export const artanisPylonProofTraceHasNoAuthority = (
  authority: ArtanisPylonProofTraceAuthority,
): boolean =>
  !authority.pylonMutationAllowed &&
  !authority.receiptMutationAllowed &&
  !authority.releasePublicationAllowed &&
  !authority.settlementMutationAllowed &&
  !authority.walletSpendAllowed

export const artanisPylonProofTraceProjectionHasPrivateMaterial = (
  projection: ArtanisPylonProofTraceProjection,
): boolean => {
  const values = stringValues(projection)

  return values.some(value =>
    containsProviderSecretMaterial(value) ||
    unsafeRefPattern.test(value) ||
    rawTimestampPattern.test(value)
  )
}

export const ARTANIS_PYLON_PROOF_TRACE_FIXTURES:
  ReadonlyArray<ArtanisPylonProofTraceRecord> = [
    new ArtanisPylonProofTraceRecord({
      assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
      createdAtIso: '2026-06-07T22:20:00.000Z',
      dispatch: new ArtanisPylonProofTraceDispatchEvidence({
        artanisRunRef: 'run.public.artanis.bootstrap.20260607.trace_001',
        assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
        evidenceRefs: [
          'event.public.artanis.dispatch.assignment_proof_trace_complete_001',
        ],
        settlementIntentRef:
          'settlement_intent.public.artanis.proof_trace.complete_001',
      }),
      pylonEvents: [
        new ArtanisPylonProofTracePylonEvent({
          accepted: true,
          assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
          eventKind: 'assignment_acceptance',
          evidenceRefs: [
            'accepted_work.public.pylon.proof_trace.complete_001',
          ],
          pylonRef: 'pylon.public.edge.trace_alpha',
          status: 'accepted',
        }),
        new ArtanisPylonProofTracePylonEvent({
          assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
          eventKind: 'artifact_proof_metadata',
          evidenceRefs: [
            'artifact.public.pylon.proof_trace.complete_001',
            'proof.public.pylon.proof_trace.complete_001',
          ],
          pylonRef: 'pylon.public.edge.trace_alpha',
          status: 'submitted',
        }),
        new ArtanisPylonProofTracePylonEvent({
          assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
          eventKind: 'payment_receipt',
          evidenceRefs: [
            'payment.redacted.pylon.proof_trace.complete_001',
            'settlement.public.pylon.proof_trace.complete_001',
          ],
          pylonRef: 'pylon.public.edge.trace_alpha',
          status: 'reported',
        }),
        new ArtanisPylonProofTracePylonEvent({
          assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
          eventKind: 'settlement_status',
          evidenceRefs: [
            'receipt.nexus_pylon.settlement.assignment_public_artanis_proof_trace_complete_001',
          ],
          pylonRef: 'pylon.public.edge.trace_alpha',
          status: 'settled',
        }),
      ],
      receipt: new ArtanisPylonProofTraceReceiptEvidence({
        assignmentRef: 'assignment.public.artanis.proof_trace.complete_001',
        evidenceRefs: [
          'route:/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_artanis_proof_trace_complete_001',
        ],
        movementMode: 'real_bitcoin',
        pylonRef: 'pylon.public.edge.trace_alpha',
        realBitcoinMoved: true,
        receiptRef:
          'receipt.nexus_pylon.settlement.assignment_public_artanis_proof_trace_complete_001',
        settlementStateLabel: 'Settled',
        terminalSettlementObserved: true,
      }),
      releaseEvidenceRefs: ['release.public.pylon.v0_2_4.installable'],
      updatedAtIso: '2026-06-07T22:50:00.000Z',
    }),
  ]
