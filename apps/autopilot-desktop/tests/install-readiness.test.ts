import { describe, expect, test } from "bun:test"
import { projectInstallReadiness } from "../src/shared/install-readiness"
import type { BuiltInAgentReadinessResponse } from "../src/shared/rpc"

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
      autoUpdateDisabledReason: null,
    })

    expect(readiness.ok).toBe(true)
    expect(readiness.highestRoiAction).toBe("Go online")
    expect(readiness.userApiKeyRequired).toBe(false)
    expect(readiness.blockerRefs).toEqual([])
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
  })
})
