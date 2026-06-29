/**
 * Public-safe acceptance gate for the signed-installer Apple FM smoke in #7022.
 *
 * This is deliberately a pure structural gate. It does not run the helper,
 * inspect prompts, read local files, or perform signing/notarization. Operators
 * feed it public-safe smoke facts after a from-install run on supported Apple
 * Silicon, and it reports exactly which product-promise blockers remain.
 */

export const APPLE_FM_FROM_INSTALL_SMOKE_SCHEMA =
  "openagents.autopilot.apple_fm.from_install_smoke.v0.1" as const

export const LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER =
  "blocker.product_promises.local_apple_fm_signed_installer_recut_missing" as const

export const LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER =
  "blocker.product_promises.local_apple_fm_helper_supervision_missing" as const

export type AppleFmFromInstallSmokeInput = {
  readonly supportedAppleSiliconHost: boolean
  readonly signedInstaller: boolean
  readonly notarizedInstaller: boolean
  readonly packagedHelperVerified: boolean
  readonly helperLaunchedFromInstall: boolean
  readonly helperReadinessReady: boolean
  readonly helperRestartObserved: boolean
  readonly boundedLocalSessionCompleted: boolean
  readonly localModeNoHostedPrompt: boolean
  readonly redactionPassed: boolean
  readonly cleanShutdownObserved: boolean
  readonly publicEvidenceRef?: string | null
}

export type AppleFmFromInstallSmokeGate = {
  readonly schema: typeof APPLE_FM_FROM_INSTALL_SMOKE_SCHEMA
  readonly ok: boolean
  readonly clearedBlockerRefs: ReadonlyArray<
    | typeof LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER
    | typeof LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER
  >
  readonly remainingBlockerRefs: ReadonlyArray<
    | typeof LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER
    | typeof LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER
  >
  readonly evidenceRefs: ReadonlyArray<string>
  readonly missingChecks: ReadonlyArray<string>
}

const publicEvidenceRefPattern =
  /^(docs\/|route:|https:\/\/github\.com\/OpenAgentsInc\/openagents\/|issue:#)[A-Za-z0-9._~:/?#[\]@!$&'()*+,;=%-]+$/

const cleanPublicEvidenceRefs = (
  ref: string | null | undefined,
): ReadonlyArray<string> => {
  const value = ref?.trim()
  if (value === undefined || value.length === 0) return []
  return publicEvidenceRefPattern.test(value) ? [value] : []
}

export function evaluateAppleFmFromInstallSmokeGate(
  input: AppleFmFromInstallSmokeInput,
): AppleFmFromInstallSmokeGate {
  const evidenceRefs = cleanPublicEvidenceRefs(input.publicEvidenceRef)
  const signedInstallerChecks = [
    input.supportedAppleSiliconHost,
    input.signedInstaller,
    input.notarizedInstaller,
    input.packagedHelperVerified,
    evidenceRefs.length > 0,
  ]
  const helperSupervisionChecks = [
    input.helperLaunchedFromInstall,
    input.helperReadinessReady,
    input.helperRestartObserved,
    input.boundedLocalSessionCompleted,
    input.localModeNoHostedPrompt,
    input.redactionPassed,
    input.cleanShutdownObserved,
    evidenceRefs.length > 0,
  ]
  const signedInstallerOk = signedInstallerChecks.every(Boolean)
  const helperSupervisionOk = helperSupervisionChecks.every(Boolean)

  const remainingBlockerRefs = [
    ...(!signedInstallerOk ? [LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER] : []),
    ...(!helperSupervisionOk ? [LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER] : []),
  ]
  const clearedBlockerRefs = [
    ...(signedInstallerOk ? [LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER] : []),
    ...(helperSupervisionOk ? [LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER] : []),
  ]
  const missingChecks = [
    ...(!input.supportedAppleSiliconHost ? ["supported_apple_silicon_host"] : []),
    ...(!input.signedInstaller ? ["signed_installer"] : []),
    ...(!input.notarizedInstaller ? ["notarized_installer"] : []),
    ...(!input.packagedHelperVerified ? ["packaged_helper_verified"] : []),
    ...(!input.helperLaunchedFromInstall ? ["helper_launched_from_install"] : []),
    ...(!input.helperReadinessReady ? ["helper_readiness_ready"] : []),
    ...(!input.helperRestartObserved ? ["helper_restart_observed"] : []),
    ...(!input.boundedLocalSessionCompleted
      ? ["bounded_local_session_completed"]
      : []),
    ...(!input.localModeNoHostedPrompt ? ["local_mode_no_hosted_prompt"] : []),
    ...(!input.redactionPassed ? ["redaction_passed"] : []),
    ...(!input.cleanShutdownObserved ? ["clean_shutdown_observed"] : []),
    ...(evidenceRefs.length === 0 ? ["public_safe_evidence_ref"] : []),
  ]

  return {
    schema: APPLE_FM_FROM_INSTALL_SMOKE_SCHEMA,
    ok: remainingBlockerRefs.length === 0,
    clearedBlockerRefs,
    remainingBlockerRefs,
    evidenceRefs,
    missingChecks,
  }
}
