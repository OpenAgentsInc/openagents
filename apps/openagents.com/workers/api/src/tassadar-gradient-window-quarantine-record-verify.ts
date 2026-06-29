import { Option, Schema as S } from 'effect'

import {
  TassadarGradientWindowPendingVerificationStages,
  TassadarGradientWindowQuarantineRecord,
  tassadarGradientWindowQuarantineRecordRef,
} from './tassadar-gradient-window-quarantine-record'

/**
 * Read-side quarantine record verifier for
 * training.public_gradient_windows.v1.
 *
 * The quarantine record builder (tassadar-gradient-window-quarantine-record.ts)
 * can BUILD a public-safe record from an admitted submission, and the promotion
 * lineage guard (tassadar-gradient-window-promotion-lineage.ts) checks a record
 * against the promotion receipt that descends from it. But a runtime or public
 * reader who dereferences a persisted quarantine record from a store or route
 * has neither the source submission nor the builder in hand — only the record
 * bytes. Nothing let such a reader confirm, without trusting the writer, that an
 * untrusted read-back record is actually a legitimate quarantine record that
 * still owes its full verification debt and grants residency only.
 *
 * This is the symmetric counterpart to the promoted-window receipt verifier
 * (tassadar-gradient-window-promotion-receipt-verify.ts): the receipt side of
 * the runtime had a read-side verifier, the quarantine side did not.
 *
 * `verifyTassadarGradientWindowQuarantineRecord` closes that gap. It is a pure,
 * TOTAL function over a single untrusted input: it decodes the record, and if it
 * fails to decode it returns an invalid decision rather than throwing, so it is
 * safe at the edge of a real store or route. It re-checks, on the read-back
 * record, the invariants the builder enforced at build time — the record ref
 * derives canonically from the window ref, the window ref is non-empty, the
 * required curated-data / construction / verification / psionic-H1 evidence the
 * window was admitted on is all still present, and the pending verification
 * stages are exactly the canonical recompute -> replicate -> canary -> promote
 * debt — plus a public-safety scan. The `quarantined` stage, the all-false /
 * residency-only authority, `compiledCoreUnchanged: true`, and `publicSafe: true`
 * literals are structurally guaranteed by the schema, so a record that violates
 * them fails to decode and is reported as unparsed.
 *
 * A valid quarantine record asserts only that the window holds quarantine
 * residency and still owes verification; `promotionEligible` is therefore always
 * false. Admission is not acceptance, and this verifier confers no promotion,
 * settlement, canonical-checkpoint, compiled-core-gradient, or direct-submission
 * authority.
 *
 * This advances blocker.product_promises.public_gradient_live_window_runtime_missing
 * by building the read-side validator a public quarantine store or route needs.
 * It does NOT clear that blocker: no live store persists these records, no route
 * serves them, and no public window has been accepted, promoted, paid, or
 * settled.
 */

export const TassadarGradientWindowQuarantineRecordVerificationSchemaVersion =
  'openagents.training.public_gradient_window.quarantine_record_verification.v1'
export type TassadarGradientWindowQuarantineRecordVerificationSchemaVersion =
  typeof TassadarGradientWindowQuarantineRecordVerificationSchemaVersion

const verificationBlocker = (suffix: string): string =>
  `blocker.public.tassadar_gradient_window.quarantine_record_verification.${suffix}`

export type TassadarGradientWindowQuarantineRecordVerification = Readonly<{
  invalidReasonRefs: ReadonlyArray<string>
  pendingVerificationStages: ReadonlyArray<string>
  promotionEligible: false
  publicSafe: true
  recordRef: string | null
  schemaVersion: TassadarGradientWindowQuarantineRecordVerificationSchemaVersion
  valid: boolean
  windowRef: string | null
}>

const decodeRecord = S.decodeUnknownOption(TassadarGradientWindowQuarantineRecord)

const unsafeRecordPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer|cookie|email[_-]?(address|body|raw)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage|secret)|preimage|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|mnemonic|path|private|seed))/i

const canonicalPendingStages = [
  ...TassadarGradientWindowPendingVerificationStages,
]

const arraysEqual = (
  a: ReadonlyArray<string>,
  b: ReadonlyArray<string>,
): boolean => a.length === b.length && a.every((value, index) => value === b[index])

const invalid = (
  invalidReasonRefs: ReadonlyArray<string>,
  refs: {
    pendingVerificationStages?: ReadonlyArray<string>
    recordRef?: string | null
    windowRef?: string | null
  } = {},
): TassadarGradientWindowQuarantineRecordVerification => ({
  invalidReasonRefs: [...new Set(invalidReasonRefs)].sort(),
  pendingVerificationStages: refs.pendingVerificationStages ?? [],
  promotionEligible: false,
  publicSafe: true,
  recordRef: refs.recordRef ?? null,
  schemaVersion:
    TassadarGradientWindowQuarantineRecordVerificationSchemaVersion,
  valid: false,
  windowRef: refs.windowRef ?? null,
})

/**
 * Verify that an untrusted, read-back quarantine record is a legitimate
 * residency-only quarantine record that still owes its full verification debt.
 *
 * Pure and total: never throws. An unparseable record, a non-canonical record
 * ref, an empty window ref, missing admission evidence, a tampered pending
 * verification stage list, or unsafe material all yield an invalid decision
 * carrying the reasons.
 */
export const verifyTassadarGradientWindowQuarantineRecord = (
  record: unknown,
): TassadarGradientWindowQuarantineRecordVerification => {
  const decoded = decodeRecord(record)
  if (Option.isNone(decoded)) {
    return invalid([verificationBlocker('quarantine_record_unparsed')])
  }

  const value = decoded.value
  const refs = {
    pendingVerificationStages: value.pendingVerificationStages,
    recordRef: value.recordRef,
    windowRef: value.windowRef,
  }
  const reasons: Array<string> = []

  if (value.windowRef.trim().length === 0) {
    reasons.push(verificationBlocker('window_ref_missing'))
  }
  if (
    value.recordRef !==
    tassadarGradientWindowQuarantineRecordRef(value.windowRef)
  ) {
    reasons.push(verificationBlocker('record_ref_mismatch'))
  }

  const evidence: ReadonlyArray<readonly [string, ReadonlyArray<string>]> = [
    ['curated_data_refs_missing', value.evidenceRefs.curatedDataRefs],
    [
      'construction_receipt_refs_missing',
      value.evidenceRefs.constructionReceiptRefs,
    ],
    [
      'psionic_h1_evidence_refs_missing',
      value.evidenceRefs.psionicH1EvidenceRefs,
    ],
    [
      'verification_receipt_refs_missing',
      value.evidenceRefs.verificationReceiptRefs,
    ],
  ]
  for (const [suffix, evidenceRefs] of evidence) {
    if (evidenceRefs.length === 0) {
      reasons.push(verificationBlocker(suffix))
    }
  }

  if (!arraysEqual(value.pendingVerificationStages, canonicalPendingStages)) {
    reasons.push(verificationBlocker('pending_verification_stages_tampered'))
  }

  if (unsafeRecordPattern.test(JSON.stringify(value))) {
    reasons.push(verificationBlocker('unsafe_material'))
  }

  if (reasons.length > 0) {
    return invalid(reasons, refs)
  }

  return {
    invalidReasonRefs: [],
    pendingVerificationStages: value.pendingVerificationStages,
    promotionEligible: false,
    publicSafe: true,
    recordRef: value.recordRef,
    schemaVersion:
      TassadarGradientWindowQuarantineRecordVerificationSchemaVersion,
    valid: true,
    windowRef: value.windowRef,
  }
}
