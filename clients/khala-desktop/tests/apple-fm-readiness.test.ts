import { describe, expect, test } from "bun:test"

import {
  APPLE_FM_CAPABILITY,
  APPLE_FM_SAFE_TOOL_PROJECTION_CAPABILITY,
  buildKhalaAppleFmReadiness,
  sanitizePylonAppleFmStatus,
} from "../src/shared/apple-fm-readiness.js"

describe("khala desktop Apple FM readiness", () => {
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
        advertisedCapabilities: [
          APPLE_FM_CAPABILITY,
          APPLE_FM_SAFE_TOOL_PROJECTION_CAPABILITY,
        ],
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

  test("does not report ready without the safe Blueprint tool projection", () => {
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
      "blocker.khala_desktop.apple_fm.pylon_not_ready",
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
})
