import { describe, expect, test } from "bun:test"

import {
  APPLE_FM_BACKEND_KIND,
  appleFmPreferredOnPlatform,
  decisionFromBackendJson,
  disabledOnDeviceDeciderStatus,
  GPT_OSS_BACKEND_KIND,
  parseOnDeviceDeciderConfig,
  selectOnDeviceDeciderBackend,
} from "../src/shared/on-device-decider.js"

const fixedNow = "2026-06-29T00:00:00.000Z"

describe("openagents desktop on-device decider contract", () => {
  test("is off by default", () => {
    const config = parseOnDeviceDeciderConfig({})
    const status = disabledOnDeviceDeciderStatus({
      observedAt: fixedNow,
      platform: { arch: "arm64", platform: "darwin" },
    })

    expect(config.mode).toBe("off")
    expect(status.enabled).toBe(false)
    expect(status.available).toBe(false)
    expect(status.noSpend).toBe(true)
    expect(status.mainModelParityClaim).toBe(false)
  })

  test("selects Apple FM only for Apple platforms in auto mode", () => {
    expect(
      appleFmPreferredOnPlatform({ arch: "arm64", platform: "darwin" }),
    ).toBe(true)
    expect(appleFmPreferredOnPlatform({ arch: "arm64", platform: "ios" })).toBe(
      true,
    )
    expect(
      selectOnDeviceDeciderBackend("auto", {
        arch: "arm64",
        platform: "darwin",
      }),
    ).toBe(APPLE_FM_BACKEND_KIND)
    expect(
      selectOnDeviceDeciderBackend("auto", {
        arch: "x64",
        platform: "linux",
      }),
    ).toBe(GPT_OSS_BACKEND_KIND)
  })

  test("honors explicit opt-in backend selection", () => {
    expect(
      parseOnDeviceDeciderConfig({
        OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "1",
        OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_BACKEND: "gpt-oss",
      }).mode,
    ).toBe(GPT_OSS_BACKEND_KIND)
    expect(
      parseOnDeviceDeciderConfig({
        OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER: "apple_fm",
      }).mode,
    ).toBe(APPLE_FM_BACKEND_KIND)
  })

  test("sanitizes backend decisions to declared tool and model candidates", () => {
    const decision = decisionFromBackendJson({
      backendKind: APPLE_FM_BACKEND_KIND,
      modelCandidates: [{ id: "codex-local-small" }],
      observedAt: fixedNow,
      raw: {
        confidence: 2,
        reasonRefs: [
          "on_device_decider.reason.fast_local_match",
          "/Users/example/private",
        ],
        selectedModelId: "private-model",
        selectedToolNames: ["read_file", "shell", "read_file"],
      },
      toolCandidates: [
        { name: "read_file" },
        { name: "list_files" },
      ],
    })

    expect(decision.selectedToolNames).toEqual(["read_file"])
    expect(decision.selectedModelId).toBeNull()
    expect(decision.confidence).toBe(1)
    expect(decision.reasonRefs).toEqual([
      "on_device_decider.reason.fast_local_match",
    ])
    expect(decision.noSpend).toBe(true)
    expect(decision.mainModelParityClaim).toBe(false)
  })
})
