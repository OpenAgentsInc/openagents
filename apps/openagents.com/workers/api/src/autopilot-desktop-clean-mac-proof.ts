// Autopilot Desktop clean-Mac proof fixture.
//
// Models the owner-run from-DMG proof needed before the desktop GUI and
// built-in compute promises can move beyond yellow. CI validates the proof
// contract only; live mode requires public-safe refs from a signed installer
// run on a clean Mac.

import { Schema as S } from 'effect'

export const AUTOPILOT_DESKTOP_CLEAN_MAC_PROOF_SCHEMA_VERSION =
  'openagents.autopilot_desktop_clean_mac_proof.v1' as const

export const AutopilotDesktopCleanMacProofMode = S.Literals([
  'ci_contract_only',
  'owner_clean_mac_from_dmg',
])
export type AutopilotDesktopCleanMacProofMode =
  typeof AutopilotDesktopCleanMacProofMode.Type

export const AutopilotDesktopCleanMacProofStepKind = S.Literals([
  'installer_signed',
  'rendered_window_captured',
  'production_presence_observed',
  'desktop_runtime_wiring_observed',
  'packaged_compute_ready',
  'metered_compute_session_recorded',
  'settled_bitcoin_receipt_captured',
  'owner_signoff_recorded',
])
export type AutopilotDesktopCleanMacProofStepKind =
  typeof AutopilotDesktopCleanMacProofStepKind.Type

export const AutopilotDesktopCleanMacProofStepState = S.Literals([
  'blocked',
  'passed',
  'planned_no_live_sessions',
])
export type AutopilotDesktopCleanMacProofStepState =
  typeof AutopilotDesktopCleanMacProofStepState.Type

export const AutopilotDesktopCleanMacProofStatus = S.Literals([
  'blocked',
  'ci_contract_ready',
  'clean_mac_proof_verified',
])
export type AutopilotDesktopCleanMacProofStatus =
  typeof AutopilotDesktopCleanMacProofStatus.Type

export class AutopilotDesktopCleanMacProofInput extends S.Class<AutopilotDesktopCleanMacProofInput>(
  'AutopilotDesktopCleanMacProofInput',
)({
  desktopRuntimeWiringRefs: S.Array(S.String),
  installerSignatureRefs: S.Array(S.String),
  meteredComputeSessionRefs: S.Array(S.String),
  mode: AutopilotDesktopCleanMacProofMode,
  nowIso: S.String,
  ownerSignoffRefs: S.Array(S.String),
  packagedComputeReadinessRefs: S.Array(S.String),
  productionPresenceRefs: S.Array(S.String),
  renderedWindowRefs: S.Array(S.String),
  settledBitcoinReceiptRefs: S.Array(S.String),
}) {}

export class AutopilotDesktopCleanMacProofStep extends S.Class<AutopilotDesktopCleanMacProofStep>(
  'AutopilotDesktopCleanMacProofStep',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  guardRefs: S.Array(S.String),
  kind: AutopilotDesktopCleanMacProofStepKind,
  state: AutopilotDesktopCleanMacProofStepState,
}) {}

export class AutopilotDesktopCleanMacProofProjection extends S.Class<AutopilotDesktopCleanMacProofProjection>(
  'AutopilotDesktopCleanMacProofProjection',
)({
  blockerRefs: S.Array(S.String),
  cleanMacProofVerified: S.Boolean,
  desktopGuiProofRefs: S.Array(S.String),
  desktopRuntimeWiringRefs: S.Array(S.String),
  mode: AutopilotDesktopCleanMacProofMode,
  packagedComputeProofRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
  redactionScanPassed: S.Boolean,
  schemaVersion: S.Literal(
    AUTOPILOT_DESKTOP_CLEAN_MAC_PROOF_SCHEMA_VERSION,
  ),
  status: AutopilotDesktopCleanMacProofStatus,
  steps: S.Array(AutopilotDesktopCleanMacProofStep),
}) {}

export class AutopilotDesktopCleanMacProofUnsafe extends S.TaggedErrorClass<AutopilotDesktopCleanMacProofUnsafe>()(
  'AutopilotDesktopCleanMacProofUnsafe',
  { reason: S.String },
) {}

const requiredStepKinds: ReadonlyArray<AutopilotDesktopCleanMacProofStepKind> =
  [
    'installer_signed',
    'rendered_window_captured',
    'production_presence_observed',
    'desktop_runtime_wiring_observed',
    'packaged_compute_ready',
    'metered_compute_session_recorded',
    'settled_bitcoin_receipt_captured',
    'owner_signoff_recorded',
  ]

const unsafeMaterialPattern =
  /(@|AIza[0-9A-Za-z_-]{10,}|api[_-]?key|bearer|raw[_-]?key|secret[^_-]|sk-[a-z0-9]|provider[_-]?(key|credential)|token[^_-]?(=|:)|\/Users\/|\/home\/)/i

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  refs.some(ref => ref.trim() !== '')

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const blockerIfMissing = (
  condition: boolean,
  ref: string,
): ReadonlyArray<string> => (condition ? [] : [ref])

const stepState = (
  mode: AutopilotDesktopCleanMacProofMode,
  blockers: ReadonlyArray<string>,
): AutopilotDesktopCleanMacProofStepState => {
  if (blockers.length > 0) return 'blocked'
  if (mode === 'ci_contract_only') return 'planned_no_live_sessions'
  return 'passed'
}

const stepFor = (
  kind: AutopilotDesktopCleanMacProofStepKind,
  evidenceRefs: ReadonlyArray<string>,
  guardRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
  mode: AutopilotDesktopCleanMacProofMode,
): AutopilotDesktopCleanMacProofStep =>
  new AutopilotDesktopCleanMacProofStep({
    blockerRefs: uniqueRefs(blockerRefs),
    evidenceRefs: uniqueRefs(evidenceRefs),
    guardRefs: uniqueRefs(guardRefs),
    kind,
    state: stepState(mode, blockerRefs),
  })

const commonBlockerRefs = (
  input: AutopilotDesktopCleanMacProofInput,
): ReadonlyArray<string> => {
  if (input.mode === 'ci_contract_only') return []

  return [
    ...blockerIfMissing(
      hasRefs(input.installerSignatureRefs),
      'blocker.autopilot_desktop_clean_mac_proof.installer_signature_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.renderedWindowRefs),
      'blocker.autopilot_desktop_clean_mac_proof.rendered_window_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.productionPresenceRefs),
      'blocker.autopilot_desktop_clean_mac_proof.production_presence_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.desktopRuntimeWiringRefs),
      'blocker.autopilot_desktop_clean_mac_proof.runtime_wiring_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.packagedComputeReadinessRefs),
      'blocker.autopilot_desktop_clean_mac_proof.packaged_compute_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.meteredComputeSessionRefs),
      'blocker.autopilot_desktop_clean_mac_proof.metered_compute_session_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.settledBitcoinReceiptRefs),
      'blocker.autopilot_desktop_clean_mac_proof.settled_bitcoin_receipt_missing',
    ),
    ...blockerIfMissing(
      hasRefs(input.ownerSignoffRefs),
      'blocker.autopilot_desktop_clean_mac_proof.owner_signoff_missing',
    ),
  ]
}

const statusFor = (
  input: AutopilotDesktopCleanMacProofInput,
  blockers: ReadonlyArray<string>,
): AutopilotDesktopCleanMacProofStatus => {
  if (blockers.length > 0) return 'blocked'
  if (input.mode === 'owner_clean_mac_from_dmg') {
    return 'clean_mac_proof_verified'
  }
  return 'ci_contract_ready'
}

export const autopilotDesktopCleanMacProofHasPrivateMaterial = (
  value: unknown,
): boolean => unsafeMaterialPattern.test(JSON.stringify(value))

export const planAutopilotDesktopCleanMacProof = (
  input: AutopilotDesktopCleanMacProofInput,
): AutopilotDesktopCleanMacProofProjection => {
  const blockerRefs = uniqueRefs(commonBlockerRefs(input))
  const isLive = input.mode === 'owner_clean_mac_from_dmg'

  const steps = [
    stepFor(
      'installer_signed',
      input.installerSignatureRefs,
      ['guard.autopilot_desktop_clean_mac_proof.notarized_dmg_required'],
      blockerIfMissing(
        !isLive || hasRefs(input.installerSignatureRefs),
        'blocker.autopilot_desktop_clean_mac_proof.installer_signature_missing',
      ),
      input.mode,
    ),
    stepFor(
      'rendered_window_captured',
      input.renderedWindowRefs,
      ['guard.autopilot_desktop_clean_mac_proof.clean_mac_window_screenshot'],
      blockerIfMissing(
        !isLive || hasRefs(input.renderedWindowRefs),
        'blocker.autopilot_desktop_clean_mac_proof.rendered_window_missing',
      ),
      input.mode,
    ),
    stepFor(
      'production_presence_observed',
      input.productionPresenceRefs,
      ['guard.autopilot_desktop_clean_mac_proof.production_pylon_stats_ref'],
      blockerIfMissing(
        !isLive || hasRefs(input.productionPresenceRefs),
        'blocker.autopilot_desktop_clean_mac_proof.production_presence_missing',
      ),
      input.mode,
    ),
    stepFor(
      'desktop_runtime_wiring_observed',
      input.desktopRuntimeWiringRefs,
      ['guard.autopilot_desktop_clean_mac_proof.live_runtime_refs_required'],
      blockerIfMissing(
        !isLive || hasRefs(input.desktopRuntimeWiringRefs),
        'blocker.autopilot_desktop_clean_mac_proof.runtime_wiring_missing',
      ),
      input.mode,
    ),
    stepFor(
      'packaged_compute_ready',
      input.packagedComputeReadinessRefs,
      ['guard.autopilot_desktop_clean_mac_proof.packaged_compute_secret_ref_only'],
      blockerIfMissing(
        !isLive || hasRefs(input.packagedComputeReadinessRefs),
        'blocker.autopilot_desktop_clean_mac_proof.packaged_compute_missing',
      ),
      input.mode,
    ),
    stepFor(
      'metered_compute_session_recorded',
      input.meteredComputeSessionRefs,
      ['guard.autopilot_desktop_clean_mac_proof.metering_ledger_ref_required'],
      blockerIfMissing(
        !isLive || hasRefs(input.meteredComputeSessionRefs),
        'blocker.autopilot_desktop_clean_mac_proof.metered_compute_session_missing',
      ),
      input.mode,
    ),
    stepFor(
      'settled_bitcoin_receipt_captured',
      input.settledBitcoinReceiptRefs,
      ['guard.autopilot_desktop_clean_mac_proof.settled_receipt_required'],
      blockerIfMissing(
        !isLive || hasRefs(input.settledBitcoinReceiptRefs),
        'blocker.autopilot_desktop_clean_mac_proof.settled_bitcoin_receipt_missing',
      ),
      input.mode,
    ),
    stepFor(
      'owner_signoff_recorded',
      input.ownerSignoffRefs,
      ['guard.autopilot_desktop_clean_mac_proof.promise_transition_owner_signed'],
      blockerIfMissing(
        !isLive || hasRefs(input.ownerSignoffRefs),
        'blocker.autopilot_desktop_clean_mac_proof.owner_signoff_missing',
      ),
      input.mode,
    ),
  ]

  const desktopGuiProofRefs = uniqueRefs([
    ...input.installerSignatureRefs,
    ...input.renderedWindowRefs,
    ...input.productionPresenceRefs,
    ...input.desktopRuntimeWiringRefs,
    ...input.settledBitcoinReceiptRefs,
    ...input.ownerSignoffRefs,
  ])

  const packagedComputeProofRefs = uniqueRefs([
    ...input.installerSignatureRefs,
    ...input.packagedComputeReadinessRefs,
    ...input.meteredComputeSessionRefs,
    ...input.ownerSignoffRefs,
  ])

  const projection = new AutopilotDesktopCleanMacProofProjection({
    blockerRefs,
    cleanMacProofVerified: isLive && blockerRefs.length === 0,
    desktopGuiProofRefs,
    desktopRuntimeWiringRefs: uniqueRefs(input.desktopRuntimeWiringRefs),
    mode: input.mode,
    packagedComputeProofRefs,
    proofBundleRefs: uniqueRefs([
      ...desktopGuiProofRefs,
      ...packagedComputeProofRefs,
    ]),
    redactionScanPassed: true,
    schemaVersion: AUTOPILOT_DESKTOP_CLEAN_MAC_PROOF_SCHEMA_VERSION,
    status: statusFor(input, blockerRefs),
    steps,
  })

  if (
    requiredStepKinds.some(
      kind => !projection.steps.some(step => step.kind === kind),
    )
  ) {
    throw new AutopilotDesktopCleanMacProofUnsafe({
      reason:
        'Autopilot Desktop clean-Mac proof projection is missing a required step.',
    })
  }

  if (autopilotDesktopCleanMacProofHasPrivateMaterial(projection)) {
    throw new AutopilotDesktopCleanMacProofUnsafe({
      reason:
        'Autopilot Desktop clean-Mac proof projection contains private material.',
    })
  }

  return projection
}
