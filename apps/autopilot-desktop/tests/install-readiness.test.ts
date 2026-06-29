import { describe, expect, test } from "bun:test"
import { projectInstallReadiness } from "../src/shared/install-readiness"
import type {
  AppleFmReadinessResponse,
  BuiltInAgentReadinessResponse,
} from "../src/shared/rpc"

const readyBuiltInAgent: BuiltInAgentReadinessResponse = {
  ok: true,
  fetchedAt: "2026-06-15T00:00:00.000Z",
  sourceUrl: "desktop:builtin-agent-readiness",
  enabled: true,
  localPylonReady: true,
  hostedComputeConfigured: true,
  userApiKeyRequired: false,
  lane: "cloud-gcp",
  modelSet: "openagents-hosted-gemini",
  maxSessionSeconds: 600,
  dailySessionCap: 3,
  dailySessionsUsed: 0,
  meteringLabel: "3 sessions/day · 600s/session · openagents-hosted-gemini",
  worktreePathPresent: true,
  blockerRefs: [],
}

const readyAppleFm: AppleFmReadinessResponse = {
  ok: true,
  fetchedAt: "2026-06-15T00:00:00.000Z",
  sourceUrl: "desktop:apple-fm-readiness",
  localPylonReady: true,
  available: true,
  status: "ready",
  backendKind: "apple_fm_bridge",
  profileId: "apple-fm-local",
  model: "apple-foundation-model",
  capability: "probe.backend.apple_fm_bridge",
  advertisedCapabilities: ["probe.backend.apple_fm_bridge"],
  baseUrl: "http://127.0.0.1:11435",
  platform: "darwin-arm64",
  version: "fake-bridge",
  unavailableReason: null,
  message: null,
  blockerRefs: [],
}

describe("projectInstallReadiness (#5064)", () => {
  test("fresh ready install points users at Go online", () => {
    const readiness = projectInstallReadiness({
      fetchedAt: "2026-06-15T00:00:00.000Z",
      platform: "darwin",
      arch: "arm64",
      runtime: "packaged",
      nodeLaunchStatus: "online",
      pylonHomePresent: true,
      controlTokenPresent: true,
      builtInAgentReadiness: readyBuiltInAgent,
      appleFmReadiness: readyAppleFm,
      autoUpdateDisabledReason: null,
    })

    expect(readiness.ok).toBe(true)
    expect(readiness.highestRoiAction).toBe("Go online")
    expect(readiness.userApiKeyRequired).toBe(false)
    expect(readiness.appleFmReady).toBe(true)
    expect(readiness.blockerRefs).toEqual([])
    expect(readiness.items.find(item => item.id === "local-apple-fm")).toMatchObject({
      status: "ready",
    })
  })

  test("launching local node is explicit waiting state", () => {
    const readiness = projectInstallReadiness({
      fetchedAt: "2026-06-15T00:00:00.000Z",
      platform: "darwin",
      arch: "arm64",
      runtime: "packaged",
      nodeLaunchStatus: "launching",
      pylonHomePresent: false,
      controlTokenPresent: false,
      builtInAgentReadiness: {
        ...readyBuiltInAgent,
        ok: false,
        localPylonReady: false,
        blockerRefs: ["blocker.autopilot.builtin_agent.local_pylon_offline"],
      },
      appleFmReadiness: {
        ...readyAppleFm,
        ok: false,
        localPylonReady: false,
        available: false,
        status: "unreachable",
        blockerRefs: ["blocker.autopilot.apple_fm.local_pylon_offline"],
      },
      autoUpdateDisabledReason: null,
    })

    expect(readiness.ok).toBe(false)
    expect(readiness.highestRoiAction).toBe("Wait for local node")
    expect(readiness.blockerRefs).toContain(
      "blocker.autopilot.install.local_pylon_launching",
    )
    expect(readiness.blockerRefs).toContain(
      "blocker.autopilot.builtin_agent.local_pylon_offline",
    )
  })

  test("hosted compute missing is a concrete blocker", () => {
    const readiness = projectInstallReadiness({
      fetchedAt: "2026-06-15T00:00:00.000Z",
      platform: "darwin",
      arch: "arm64",
      runtime: "source",
      nodeLaunchStatus: "adopted",
      pylonHomePresent: true,
      controlTokenPresent: true,
      builtInAgentReadiness: {
        ...readyBuiltInAgent,
        ok: false,
        hostedComputeConfigured: false,
        blockerRefs: [
          "blocker.autopilot.builtin_agent.hosted_compute_unconfigured",
        ],
      },
      appleFmReadiness: {
        ...readyAppleFm,
        ok: false,
        available: false,
        status: "unreachable",
        unavailableReason: "bridge_unreachable",
        message: "bridge offline",
        blockerRefs: ["blocker.pylon.apple_fm.bridge_unreachable"],
      },
      autoUpdateDisabledReason: "AUTOPILOT_DISABLE_AUTOUPDATE is set",
    })

    expect(readiness.ok).toBe(false)
    expect(readiness.highestRoiAction).toBe(
      "Install the hosted-compute desktop recut",
    )
    expect(readiness.autoUpdateEnabled).toBe(false)
    expect(readiness.blockerRefs).toContain(
      "blocker.autopilot.builtin_agent.hosted_compute_unconfigured",
    )
    expect(readiness.blockerRefs).not.toContain(
      "blocker.autopilot.install.autoupdate_disabled",
    )
    expect(readiness.blockerRefs).not.toContain(
      "blocker.pylon.apple_fm.bridge_unreachable",
    )
    expect(readiness.items.find(item => item.id === "local-apple-fm")).toMatchObject({
      status: "attention",
      blockerRef: "blocker.pylon.apple_fm.bridge_unreachable",
    })
  })

  test("darwin arm64 alone does not mark local Apple FM ready", () => {
    const readiness = projectInstallReadiness({
      fetchedAt: "2026-06-15T00:00:00.000Z",
      platform: "darwin",
      arch: "arm64",
      runtime: "packaged",
      nodeLaunchStatus: "online",
      pylonHomePresent: true,
      controlTokenPresent: true,
      builtInAgentReadiness: readyBuiltInAgent,
      appleFmReadiness: {
        ...readyAppleFm,
        ok: false,
        available: false,
        status: "unavailable",
        unavailableReason: "apple_intelligence_disabled",
        message: "Apple Intelligence is disabled.",
        blockerRefs: ["blocker.pylon.apple_fm.apple_intelligence_disabled"],
      },
      autoUpdateDisabledReason: null,
    })

    expect(readiness.appleFmReady).toBe(false)
    expect(readiness.ok).toBe(true)
    expect(readiness.items.find(item => item.id === "local-apple-fm")).toMatchObject({
      status: "attention",
      blockerRef: "blocker.pylon.apple_fm.apple_intelligence_disabled",
    })
  })
})
