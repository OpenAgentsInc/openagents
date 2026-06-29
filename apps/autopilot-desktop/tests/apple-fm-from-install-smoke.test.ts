import { describe, expect, test } from "bun:test"
import {
  LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER,
  LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER,
  evaluateAppleFmFromInstallSmokeGate,
  type AppleFmFromInstallSmokeInput,
} from "../src/shared/apple-fm-from-install-smoke"

const passingInput: AppleFmFromInstallSmokeInput = {
  supportedAppleSiliconHost: true,
  signedInstaller: true,
  notarizedInstaller: true,
  packagedHelperVerified: true,
  helperLaunchedFromInstall: true,
  helperReadinessReady: true,
  helperRestartObserved: true,
  boundedLocalSessionCompleted: true,
  localModeNoHostedPrompt: true,
  redactionPassed: true,
  cleanShutdownObserved: true,
  publicEvidenceRef:
    "docs/apple-fm/2026-06-29-local-apple-fm-signed-installer-smoke-gate.md",
}

describe("Apple FM from-install smoke gate", () => {
  test("clears both #7022 blockers only with signed installer and supervised helper evidence", () => {
    const gate = evaluateAppleFmFromInstallSmokeGate(passingInput)

    expect(gate.ok).toBe(true)
    expect(gate.remainingBlockerRefs).toEqual([])
    expect(gate.clearedBlockerRefs).toEqual([
      LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER,
      LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER,
    ])
  })

  test("keeps the promise yellow when only source-level packaging evidence exists", () => {
    const gate = evaluateAppleFmFromInstallSmokeGate({
      ...passingInput,
      signedInstaller: false,
      notarizedInstaller: false,
      helperLaunchedFromInstall: false,
      helperRestartObserved: false,
      cleanShutdownObserved: false,
      publicEvidenceRef: null,
    })

    expect(gate.ok).toBe(false)
    expect(gate.remainingBlockerRefs).toEqual([
      LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER,
      LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER,
    ])
    expect(gate.missingChecks).toEqual([
      "signed_installer",
      "notarized_installer",
      "helper_launched_from_install",
      "helper_restart_observed",
      "clean_shutdown_observed",
      "public_safe_evidence_ref",
    ])
  })

  test("requires local mode to avoid hosted prompt fallback before clearing supervision", () => {
    const gate = evaluateAppleFmFromInstallSmokeGate({
      ...passingInput,
      localModeNoHostedPrompt: false,
    })

    expect(gate.ok).toBe(false)
    expect(gate.clearedBlockerRefs).toEqual([
      LOCAL_APPLE_FM_SIGNED_INSTALLER_BLOCKER,
    ])
    expect(gate.remainingBlockerRefs).toEqual([
      LOCAL_APPLE_FM_HELPER_SUPERVISION_BLOCKER,
    ])
    expect(gate.missingChecks).toContain("local_mode_no_hosted_prompt")
  })

  test("drops non-public evidence refs instead of echoing local paths", () => {
    const gate = evaluateAppleFmFromInstallSmokeGate({
      ...passingInput,
      publicEvidenceRef: "/private/tmp/raw-smoke.log",
    })

    expect(gate.ok).toBe(false)
    expect(gate.evidenceRefs).toEqual([])
    expect(gate.missingChecks).toContain("public_safe_evidence_ref")
  })
})
