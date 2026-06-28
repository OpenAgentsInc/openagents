import { describe, expect, test } from "bun:test"
import {
  APPLE_FM_BRIDGE_DEFAULT_BASE_URL,
  APPLE_FM_DEFAULT_MODEL_ID,
  APPLE_FM_PACKAGED_HELPER_SUBPATH,
  KHALA_MACOS_APP_SUPPORT_PYLON_SUBPATH,
  KHALA_APPLE_FM_DEMAND_SOURCE,
  KHALA_APPLE_FM_TOKEN_PROVIDER,
  PYLON_PACKAGED_NODE_SUBPATH,
  buildKhalaMacosLaunchPlan,
  localAppleFmDemandAttribution,
  resolveAppleFmBaseUrl,
} from "./launch-plan"

describe("Khala macOS Apple FM launch plan", () => {
  test("uses the Probe Apple FM env var before the OpenAgents fallback", () => {
    expect(
      resolveAppleFmBaseUrl({
        PROBE_APPLE_FM_BASE_URL: "http://127.0.0.1:12000",
        OPENAGENTS_APPLE_FM_BASE_URL: "http://127.0.0.1:13000",
      }),
    ).toEqual({
      baseUrl: "http://127.0.0.1:12000",
      source: "PROBE_APPLE_FM_BASE_URL",
    })
  })

  test("falls back to the existing Apple FM loopback bridge contract", () => {
    expect(resolveAppleFmBaseUrl({})).toEqual({
      baseUrl: APPLE_FM_BRIDGE_DEFAULT_BASE_URL,
      source: "default",
    })
  })

  test("launches the embedded Pylon with Apple FM supervision for one-launch UX", () => {
    const plan = buildKhalaMacosLaunchPlan({
      resourcesDir: "/Applications/Khala.app/Contents/Resources",
      homeDir: "/Users/alice",
      appleFmBridgeReady: true,
      existingPylonReady: false,
      bunPath: "/Applications/Khala.app/Contents/MacOS/bun",
    })

    expect(plan.pylonMode).toBe("launch_embedded")
    expect(plan.pylonCommand).toEqual([
      "/Applications/Khala.app/Contents/MacOS/bun",
      `/Applications/Khala.app/Contents/Resources/${PYLON_PACKAGED_NODE_SUBPATH}`,
      "node",
    ])
    expect(plan.pylonHome).toBe(
      `/Users/alice/${KHALA_MACOS_APP_SUPPORT_PYLON_SUBPATH}`,
    )
    expect(plan.appleFmBridgePath).toBe(
      `/Applications/Khala.app/Contents/Resources/${APPLE_FM_PACKAGED_HELPER_SUBPATH}`,
    )
    expect(plan.childEnv.PYLON_APPLE_FM_SUPERVISE).toBe("1")
    expect(plan.childEnv.OPENAGENTS_APPLE_FM_BRIDGE_PATH).toBe(
      plan.appleFmBridgePath,
    )
    expect(plan.childEnv.PYLON_ASSIGNMENT_WORKER).toBe("1")
    expect(plan.blockerRefs).toEqual([])
  })

  test("connects to an existing Pylon instead of double-spawning", () => {
    const plan = buildKhalaMacosLaunchPlan({
      resourcesDir: "/r",
      homeDir: "/h",
      appleFmBridgeReady: true,
      existingPylonReady: true,
    })

    expect(plan.pylonMode).toBe("connect_existing")
    expect(plan.pylonCommand).toEqual([])
    expect(plan.capacityRefs).toContain("capacity.inference.apple_fm.ready=1")
    expect(plan.capacityRefs).toContain("capacity.inference.apple_fm.available=1")
  })

  test("advertises blocked Apple FM capacity honestly", () => {
    const plan = buildKhalaMacosLaunchPlan({
      resourcesDir: "/r",
      homeDir: "/h",
      appleFmBridgeReady: false,
      existingPylonReady: true,
    })

    expect(plan.capacityRefs).toContain("capacity.inference.apple_fm.ready=0")
    expect(plan.capacityRefs).toContain("capacity.inference.apple_fm.available=0")
    expect(plan.blockerRefs).toEqual([
      "blocker.khala_macos.apple_fm_bridge_unavailable",
    ])
  })

  test("marks local Apple FM work as own-capacity with estimated usage truth", () => {
    expect(localAppleFmDemandAttribution()).toEqual({
      provider: KHALA_APPLE_FM_TOKEN_PROVIDER,
      model: APPLE_FM_DEFAULT_MODEL_ID,
      backendKind: "apple_fm_bridge",
      demandKind: "own_capacity",
      demandSource: KHALA_APPLE_FM_DEMAND_SOURCE,
      usageTruth: "estimated",
      counterFamily: "khala_tokens_served",
    })
  })
})
