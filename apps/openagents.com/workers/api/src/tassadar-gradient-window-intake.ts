import { Option, Schema as S } from 'effect'

import {
  TassadarGradientWindowCandidate,
  type TassadarGradientWindowStage,
} from './tassadar-gradient-window-regime'

/**
 * Intake / admission controller for training.public_gradient_windows.v1.
 *
 * This is the FRONT DOOR of the (still not-live) public gradient-window
 * runtime. The regime gate (tassadar-gradient-window-regime.ts) evaluates a
 * window that has ALREADY been processed: it requires a full recompute,
 * replication, and canary receipt bundle, so it answers "may this window
 * promote?". Nothing, however, decided whether a freshly submitted candidate
 * may even ENTER quarantine and consume those verification resources. That
 * admission decision is what this module supplies.
 *
 * `admitTassadarGradientWindowToQuarantine` is a pure, deterministic function.
 * It accepts an untrusted submission, rejects anything malformed, unsafe, or
 * that disrespects the frozen compiled core, and otherwise admits the candidate
 * to quarantine. Admission grants QUARANTINE entry only: it grants no
 * promotion, settlement, canonical-checkpoint-mutation, compiled-core-gradient,
 * or direct-submission authority. It never throws on bad input — a hostile or
 * malformed submission yields a `rejected` decision, not an exception — so it is
 * safe to place at the edge of a real runtime.
 *
 * This advances blocker.product_promises.public_gradient_live_window_runtime_missing
 * by building the runtime's admission edge. It does NOT clear that blocker: no
 * live runtime yet receives real public submissions, and no public window has
 * been accepted, promoted, paid, or settled.
 */

export const TassadarGradientWindowIntakeSchemaVersion =
  'openagents.training.public_gradient_window.intake_admission.v1'
export type TassadarGradientWindowIntakeSchemaVersion =
  typeof TassadarGradientWindowIntakeSchemaVersion

const intakeBlocker = (suffix: string): string =>
  `blocker.public.tassadar_gradient_window.intake.${suffix}`

export type TassadarGradientWindowIntakeDecision = Readonly<{
  admitted: boolean
  authority: Readonly<{
    canonicalCheckpointMutationAllowed: false
    compiledCoreGradientMutationAllowed: false
    directSubmissionMutationAllowed: false
    promotionAllowed: false
    quarantineAdmissionGranted: boolean
    settlementMutationAllowed: false
  }>
  authorityBoundary: string
  publicSafe: true
  quarantineRecordRef: string | null
  rejectionReasonRefs: ReadonlyArray<string>
  schemaVersion: TassadarGradientWindowIntakeSchemaVersion
  stage: Extract<TassadarGradientWindowStage, 'quarantined'> | 'rejected'
  windowRef: string
}>

const decodeCandidate = S.decodeUnknownOption(TassadarGradientWindowCandidate)

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const unsafeIntakePattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer|cookie|email[_-]?(address|body|raw)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage|secret)|preimage|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|mnemonic|path|private|seed))/i

const scopeTargetsCompiledCore = (scope: string): boolean => {
  const normalized = scope.toLowerCase()

  return (
    normalized.includes('compiled_exact_core') ||
    normalized.includes('compiled_core') ||
    normalized.includes('exact_core') ||
    normalized.includes('frozen_core') ||
    normalized.includes('analytic_executor') ||
    normalized.includes('tassadar_alm_numeric_execute')
  )
}

const authorityBoundary =
  'Quarantine admission for one public gradient-window submission. It grants entry to quarantine recompute/replication/canary only; it grants no promotion, settlement, canonical-checkpoint mutation, compiled-core-gradient mutation, or direct-submission authority. Admission is not acceptance: a window can still be blocked by the regime gate.'

const reject = (
  windowRef: string,
  rejectionReasonRefs: ReadonlyArray<string>,
): TassadarGradientWindowIntakeDecision => ({
  admitted: false,
  authority: {
    canonicalCheckpointMutationAllowed: false,
    compiledCoreGradientMutationAllowed: false,
    directSubmissionMutationAllowed: false,
    promotionAllowed: false,
    quarantineAdmissionGranted: false,
    settlementMutationAllowed: false,
  },
  authorityBoundary,
  publicSafe: true,
  quarantineRecordRef: null,
  rejectionReasonRefs: [...new Set(rejectionReasonRefs)].sort(),
  schemaVersion: TassadarGradientWindowIntakeSchemaVersion,
  stage: 'rejected',
  windowRef,
})

export const admitTassadarGradientWindowToQuarantine = (
  submission: unknown,
): TassadarGradientWindowIntakeDecision => {
  const decoded = decodeCandidate(submission)
  if (Option.isNone(decoded)) {
    return reject('window.public.tassadar_gradient_window.unparsed', [
      intakeBlocker('malformed_submission'),
    ])
  }

  const candidate = decoded.value
  const windowRef = candidate.windowRef
  const reasons: Array<string> = []

  if (unsafeIntakePattern.test(JSON.stringify(candidate))) {
    return reject(safeSuffix(windowRef), [intakeBlocker('unsafe_material')])
  }

  const trainableCoreScope = candidate.trainableParameterScopes.some((scope: string) =>
    scopeTargetsCompiledCore(scope),
  )
  const frozenCoreScopePresent = candidate.frozenParameterScopes.some((scope: string) =>
    scopeTargetsCompiledCore(scope),
  )

  if (candidate.compiledCoreGradientTargeted || trainableCoreScope) {
    reasons.push(intakeBlocker('compiled_core_gradient_targeted'))
  }
  if (!frozenCoreScopePresent) {
    reasons.push(intakeBlocker('frozen_core_scope_missing'))
  }
  if (candidate.frozenCoreDigestBefore !== candidate.frozenCoreDigestAfter) {
    reasons.push(intakeBlocker('frozen_core_digest_changed'))
  }
  if (!candidate.gradientsFlowThroughTrace) {
    reasons.push(intakeBlocker('trace_not_forward_pass'))
  }
  if (candidate.psionicH1EvidenceRefs.length === 0) {
    reasons.push(intakeBlocker('psionic_h1_evidence_missing'))
  }
  if (candidate.curatedDataRefs.length === 0) {
    reasons.push(intakeBlocker('curated_data_refs_missing'))
  }
  if (candidate.constructionReceiptRefs.length === 0) {
    reasons.push(intakeBlocker('construction_substrate_missing'))
  }
  if (candidate.verificationReceiptRefs.length === 0) {
    reasons.push(intakeBlocker('verification_substrate_missing'))
  }
  if (candidate.contributorRef.trim().length === 0) {
    reasons.push(intakeBlocker('contributor_ref_missing'))
  }

  if (reasons.length > 0) {
    return reject(safeSuffix(windowRef), reasons)
  }

  return {
    admitted: true,
    authority: {
      canonicalCheckpointMutationAllowed: false,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      promotionAllowed: false,
      quarantineAdmissionGranted: true,
      settlementMutationAllowed: false,
    },
    authorityBoundary,
    publicSafe: true,
    quarantineRecordRef: `quarantine.public.tassadar_gradient_window.${safeSuffix(windowRef)}`,
    rejectionReasonRefs: [],
    schemaVersion: TassadarGradientWindowIntakeSchemaVersion,
    stage: 'quarantined',
    windowRef,
  }
}
