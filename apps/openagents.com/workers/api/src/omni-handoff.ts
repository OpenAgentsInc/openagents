import { Effect, Schema as S } from 'effect'

import { type OmniAcceptedOutcomeWorkKind } from './omni-accepted-outcome-contracts'
import {
  type CreateOmniEvidenceBundleInput,
  type OmniEvidenceBundleEntry,
  type OmniEvidenceBundleError,
  type OmniEvidenceBundleRecord,
  type OmniEvidenceBundlesRuntime,
  createOmniEvidenceBundle,
  customerOmniEvidenceBundleProjection,
  systemOmniEvidenceBundlesRuntime,
} from './omni-evidence-bundles'
import {
  type CreateOmniPublicProofBundleInput,
  type OmniPublicProofBundleError,
  type OmniPublicProofBundleRecord,
  type OmniPublicProofBundlesRuntime,
  createOmniPublicProofBundle,
  publicOmniProofBundleProjection,
  systemOmniPublicProofBundlesRuntime,
} from './omni-public-proof-bundles'

// On workroom completion/acceptance the handoff orchestration chains the
// internal evidence bundle into the customer-facing public proof bundle. The
// evidence bundle carries the full (possibly private) entry set; the proof
// bundle is the redacted, client-safe handoff. This module derives the proof
// bundle ONLY from public-safe evidence entries so raw prompts, run logs,
// provider material, and settlement/payout language never leak into the
// client-facing projection.

export const OmniHandoffWorkroomState = S.Literals([
  'accepted',
  'provisionally_accepted',
  'completed',
])
export type OmniHandoffWorkroomState = typeof OmniHandoffWorkroomState.Type

// Evidence entry kinds that are allowed to flow into the customer-facing proof
// bundle as source/artifact/receipt refs. Build logs, diffs, raw emails, and
// research briefs stay inside the evidence bundle even when an operator marks
// them public; the proof bundle is intentionally narrower than the evidence
// public projection.
const PROOF_SOURCE_KINDS = new Set([
  'research_brief',
  'source_commit',
  'exa_source_card',
])
const PROOF_ARTIFACT_KINDS = new Set(['deployment_url', 'screenshot'])
const PROOF_RECEIPT_KINDS = new Set(['receipt', 'redaction_report'])

export type OmniHandoffWorkroomContext = Readonly<{
  state: OmniHandoffWorkroomState
  workKind: OmniAcceptedOutcomeWorkKind
  workroomId: string
}>

export type OmniHandoffProofInputs = Readonly<{
  acceptanceStateRef: string
  economicsCaveatRef: string
  legalCaveatRef?: string | undefined
  privacyCaveatRef: string
  reviewStateRef: string
  // Extra customer-safe refs the caller wants carried into the proof bundle
  // (e.g. an explicit acceptance receipt). They are validated by the proof
  // bundle service before persistence.
  extraReceiptRefs?: ReadonlyArray<string> | undefined
}>

export type OmniHandoffInput = Readonly<{
  evidence: Readonly<{
    entries: ReadonlyArray<OmniEvidenceBundleEntry>
    idempotencyKey: string
    legalSensitive?: boolean | undefined
    metadata?: Readonly<Record<string, unknown>> | undefined
    sourceAuthorityCaveatRef?: string | undefined
    summaryRef: string
  }>
  proof: OmniHandoffProofInputs
  proofIdempotencyKey: string
  workroom: OmniHandoffWorkroomContext
}>

export type OmniHandoffResult = Readonly<{
  evidenceBundle: OmniEvidenceBundleRecord
  proofBundle: OmniPublicProofBundleRecord
  publicProofProjection: ReturnType<typeof publicOmniProofBundleProjection>
}>

export type OmniHandoffRuntime = Readonly<{
  evidenceRuntime: OmniEvidenceBundlesRuntime
  proofRuntime: OmniPublicProofBundlesRuntime
}>

export const systemOmniHandoffRuntime: OmniHandoffRuntime = {
  evidenceRuntime: systemOmniEvidenceBundlesRuntime,
  proofRuntime: systemOmniPublicProofBundlesRuntime,
}

export class OmniHandoffValidationError extends S.TaggedErrorClass<OmniHandoffValidationError>()(
  'OmniHandoffValidationError',
  { reason: S.String },
) {}

export type OmniHandoffError =
  | OmniHandoffValidationError
  | OmniEvidenceBundleError
  | OmniPublicProofBundleError

// An evidence entry is eligible to seed the customer-facing proof bundle only
// when it is public-safe, publicly visible, and not redacted away. This mirrors
// the publicly-visible entry rules in omni-evidence-bundles but is enforced
// independently here so the handoff cannot widen what reaches the client.
const entryIsCustomerHandoffSafe = (
  entry: OmniEvidenceBundleEntry,
): boolean =>
  entry.publicSafe &&
  entry.visibility === 'public' &&
  entry.redactionState !== 'private_only' &&
  entry.redactionState !== 'blocked'

const dedupeSorted = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].sort()

const proofRefsFromEvidence = (
  entries: ReadonlyArray<OmniEvidenceBundleEntry>,
): Readonly<{
  artifactRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}> => {
  const safe = entries.filter(entryIsCustomerHandoffSafe)

  return {
    artifactRefs: dedupeSorted(
      safe
        .filter(entry => PROOF_ARTIFACT_KINDS.has(entry.entryKind))
        .map(entry => entry.ref),
    ),
    receiptRefs: dedupeSorted(
      safe
        .filter(entry => PROOF_RECEIPT_KINDS.has(entry.entryKind))
        .map(entry => entry.ref),
    ),
    sourceRefs: dedupeSorted(
      safe
        .filter(entry => PROOF_SOURCE_KINDS.has(entry.entryKind))
        .map(entry => entry.ref),
    ),
  }
}

const assertWorkroomReadyForHandoff = (
  workroom: OmniHandoffWorkroomContext,
): void => {
  if (
    workroom.state !== 'accepted' &&
    workroom.state !== 'provisionally_accepted' &&
    workroom.state !== 'completed'
  ) {
    throw new OmniHandoffValidationError({
      reason:
        'handoff requires a completed, accepted, or provisionally accepted workroom.',
    })
  }
}

const evidenceCreateInput = (
  input: OmniHandoffInput,
): CreateOmniEvidenceBundleInput => ({
  entries: input.evidence.entries,
  idempotencyKey: input.evidence.idempotencyKey,
  legalSensitive: input.evidence.legalSensitive,
  metadata: input.evidence.metadata,
  sourceAuthorityCaveatRef: input.evidence.sourceAuthorityCaveatRef,
  status: 'ready',
  summaryRef: input.evidence.summaryRef,
  workKind: input.workroom.workKind,
  workroomId: input.workroom.workroomId,
})

const proofCreateInput = (
  input: OmniHandoffInput,
  evidence: OmniEvidenceBundleRecord,
): CreateOmniPublicProofBundleInput => {
  const refs = proofRefsFromEvidence(evidence.entries)

  return {
    acceptanceStateRef: input.proof.acceptanceStateRef,
    artifactRefs: refs.artifactRefs,
    economicsCaveatRef: input.proof.economicsCaveatRef,
    idempotencyKey: input.proofIdempotencyKey,
    legalCaveatRef: input.proof.legalCaveatRef,
    legalSensitive: evidence.legalSensitive,
    privacyCaveatRef: input.proof.privacyCaveatRef,
    receiptRefs: dedupeSorted([
      ...refs.receiptRefs,
      ...(input.proof.extraReceiptRefs ?? []),
    ]),
    reviewStateRef: input.proof.reviewStateRef,
    sourceRefs: refs.sourceRefs,
    status: 'ready',
    workKind: input.workroom.workKind,
    workroomId: input.workroom.workroomId,
  }
}

// Orchestration: given a completed/accepted workroom plus evidence inputs,
// create the (internal) evidence bundle, then derive and create the
// customer-safe public proof bundle from its public-safe entries, returning the
// public projection alongside both records. Redaction is enforced twice: the
// handoff only forwards public-safe public entries, and the proof bundle
// service rejects any residual provider/settlement/payout material.
export const runOmniWorkroomHandoff = (
  db: D1Database,
  input: OmniHandoffInput,
  runtime: OmniHandoffRuntime = systemOmniHandoffRuntime,
): Effect.Effect<OmniHandoffResult, OmniHandoffError> =>
  Effect.gen(function* () {
    assertWorkroomReadyForHandoff(input.workroom)

    const evidenceBundle = yield* createOmniEvidenceBundle(
      db,
      evidenceCreateInput(input),
      runtime.evidenceRuntime,
    )

    const proofBundle = yield* createOmniPublicProofBundle(
      db,
      proofCreateInput(input, evidenceBundle),
      runtime.proofRuntime,
    )

    return {
      evidenceBundle,
      proofBundle,
      publicProofProjection: publicOmniProofBundleProjection(proofBundle),
    }
  })

// Convenience customer-facing view of the chained handoff result. The evidence
// bundle is exposed through its customer-safe projection (never the operator
// projection) so callers cannot accidentally surface private entries.
export const customerOmniHandoffProjection = (result: OmniHandoffResult) => ({
  evidence: customerOmniEvidenceBundleProjection(result.evidenceBundle),
  proof: result.publicProofProjection,
})
