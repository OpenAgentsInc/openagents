import { describe, expect, test } from "bun:test"

import {
  APPLE_FM_CAPABILITY,
  buildKhalaAppleFmInstallSmokeEvidence,
  buildKhalaAppleFmReadiness,
  sanitizePylonAppleFmStatus,
  type KhalaAppleFmReadiness,
} from "../src/shared/apple-fm-readiness.js"

describe("khala desktop Apple FM readiness", () => {
  const readyFromInstallReadiness = (): KhalaAppleFmReadiness =>
    buildKhalaAppleFmReadiness({
      platform: { platform: "darwin", arch: "arm64" },
      helperFound: true,
      helperExecutable: true,
      helperLaunchState: "running",
      pylonControlConfigured: true,
      pylonStatus: {
        available: true,
        status: "ready",
        backendKind: "apple_fm_bridge",
        profileId: "apple-fm-local",
        model: "apple-foundation-model",
        capability: APPLE_FM_CAPABILITY,
        advertisedCapabilities: [APPLE_FM_CAPABILITY],
        blockerRefs: [],
        supervisor: {
          health: "healthy",
          phase: "running",
          supervised: true,
          blockerRefs: [],
        },
      },
      observedAt: "2026-06-29T00:00:00.000Z",
    })

  test("does not advertise Apple FM capacity from hardware alone", () => {
    const readiness = buildKhalaAppleFmReadiness({
      platform: { platform: "darwin", arch: "arm64" },
      helperFound: true,
      helperExecutable: true,
      helperLaunchState: "running",
      pylonControlConfigured: false,
      observedAt: "2026-06-29T00:00:00.000Z",
    })

    expect(readiness.available).toBe(false)
    expect(readiness.state).toBe("running")
    expect(readiness.blockerRefs).toContain(
      "blocker.khala_desktop.apple_fm.pylon_control_unconfigured",
    )
  })

  test("reports ready only after live Pylon Apple FM status is ready", () => {
    const readiness = buildKhalaAppleFmReadiness({
      platform: { platform: "darwin", arch: "arm64" },
      helperFound: true,
      helperExecutable: true,
      helperLaunchState: "running",
      pylonControlConfigured: true,
      pylonStatus: {
        available: true,
        status: "ready",
        backendKind: "apple_fm_bridge",
        profileId: "apple-fm-local",
        model: "apple-foundation-model",
        capability: APPLE_FM_CAPABILITY,
        advertisedCapabilities: [APPLE_FM_CAPABILITY, "probe.blueprint.tool_menu"],
        blockerRefs: [],
      },
      observedAt: "2026-06-29T00:00:00.000Z",
    })

    expect(readiness.available).toBe(true)
    expect(readiness.state).toBe("ready")
    expect(readiness.provider).toBe("pylon-apple-fm-own-capacity")
    expect(readiness.demandSource).toBe("khala_apple_fm_delegation")
    expect(readiness.usageTruth).toBe("estimated")
  })

  test("fails closed when Pylon ready status is missing the Apple FM identity", () => {
    const readiness = buildKhalaAppleFmReadiness({
      platform: { platform: "darwin", arch: "arm64" },
      helperFound: true,
      helperExecutable: true,
      helperLaunchState: "running",
      pylonControlConfigured: true,
      pylonStatus: {
        available: true,
        status: "ready",
        advertisedCapabilities: [APPLE_FM_CAPABILITY],
        blockerRefs: [],
      },
      observedAt: "2026-06-29T00:00:00.000Z",
    })

    expect(readiness.available).toBe(false)
    expect(readiness.state).toBe("running")
    expect(readiness.blockerRefs).toContain(
      "blocker.khala_desktop.apple_fm.pylon_status_malformed",
    )
  })

  test("keeps Pylon status public-safe and omits loopback paths and tokens", () => {
    const status = sanitizePylonAppleFmStatus({
      available: false,
      status: "unreachable",
      baseUrl: "http://127.0.0.1:4716",
      helperPath: "/Users/example/app/apple-fm-bridge/foundation-bridge",
      controlToken: "secret-token",
      callbackUrl: "http://127.0.0.1/callback",
      blockerRefs: ["blocker.pylon.apple_fm.bridge_unreachable"],
      supervisor: {
        health: "recovering",
        phase: "backoff",
        supervised: true,
        blockerRefs: [],
      },
    })

    const json = JSON.stringify(status)
    expect(json).not.toContain("127.0.0.1")
    expect(json).not.toContain("/Users/example")
    expect(json).not.toContain("secret-token")
    expect(json).not.toContain("callback")
    expect(status.blockerRefs).toEqual(["blocker.pylon.apple_fm.bridge_unreachable"])
    expect(status.supervisor?.contentRedacted).toBe(true)
  })

  test("unsupported platforms fail closed", () => {
    const readiness = buildKhalaAppleFmReadiness({
      platform: { platform: "linux", arch: "x64" },
      helperFound: true,
      helperExecutable: true,
      observedAt: "2026-06-29T00:00:00.000Z",
    })

    expect(readiness.supported).toBe(false)
    expect(readiness.available).toBe(false)
    expect(readiness.state).toBe("not_supported")
  })

  test("from-install smoke evidence passes only with signed supervised local execution", () => {
    const evidence = buildKhalaAppleFmInstallSmokeEvidence({
      readiness: readyFromInstallReadiness(),
      installer: {
        artifactRef: "artifact.public.khala_desktop.signed_notarized.apple_fm.20260629",
        notarized: true,
        signed: true,
      },
      helper: {
        packaged: true,
        restartObserved: true,
        source: "packaged-resource",
        supervised: true,
      },
      session: {
        adapter: "apple_fm",
        cloudRunner: null,
        completed: true,
        executionMode: "local_bounded",
        lane: "local",
        networkAccessEnabled: false,
        resourceUsageReceiptRef: null,
        sandboxMode: "read-only",
        toolSuccess: true,
      },
      redaction: {
        bearerLeaked: false,
        callbackTokenLeaked: false,
        callbackUrlLeaked: false,
        fileContentLeaked: false,
        localPathLeaked: false,
        promptLeaked: false,
      },
      observedAt: "2026-06-29T00:00:00.000Z",
    })

    expect(evidence.ok).toBe(true)
    expect(evidence.state).toBe("passed")
    expect(evidence.blockerRefs).toEqual([])
    expect(evidence.checked).toMatchObject({
      boundedLocalSession: true,
      helperRestartObserved: true,
      noHostedCompute: true,
      packagedHelper: true,
      publicSafeRedaction: true,
      pylonReady: true,
      supervisedHelper: true,
    })
    expect(JSON.stringify(evidence)).not.toContain("127.0.0.1")
    expect(JSON.stringify(evidence)).not.toContain("/Users/")
  })

  test("from-install smoke evidence stays blocked without packaged supervision and redaction", () => {
    const evidence = buildKhalaAppleFmInstallSmokeEvidence({
      readiness: readyFromInstallReadiness(),
      installer: {
        artifactRef: "/Users/example/Khala.dmg",
        notarized: false,
        signed: true,
      },
      helper: {
        packaged: false,
        restartObserved: false,
        source: "source-build",
        supervised: false,
      },
      session: {
        adapter: "apple_fm",
        cloudRunner: "hosted",
        completed: true,
        executionMode: "local_bounded",
        lane: "local",
        networkAccessEnabled: false,
        resourceUsageReceiptRef: "receipt.public.hosted",
        sandboxMode: "read-only",
        toolSuccess: true,
      },
      redaction: {
        bearerLeaked: false,
        callbackTokenLeaked: true,
        callbackUrlLeaked: false,
        fileContentLeaked: false,
        localPathLeaked: true,
        promptLeaked: false,
      },
      observedAt: "2026-06-29T00:00:00.000Z",
    })

    expect(evidence.ok).toBe(false)
    expect(evidence.state).toBe("blocked")
    expect(evidence.artifactRef).toBeNull()
    expect(evidence.blockerRefs).toContain(
      "blocker.product_promises.local_apple_fm_helper_supervision_missing",
    )
    expect(evidence.blockerRefs).toContain(
      "blocker.product_promises.local_apple_fm_packaged_helper_missing",
    )
    expect(evidence.blockerRefs).toContain(
      "blocker.product_promises.local_apple_fm_redaction_proof_missing",
    )
    expect(evidence.blockerRefs).toContain(
      "blocker.product_promises.local_apple_fm_no_hosted_compute_proof_missing",
    )
  })
})
