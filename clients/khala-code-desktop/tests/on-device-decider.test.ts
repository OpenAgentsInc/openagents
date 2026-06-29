import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_APPLE_FM_DECIDER_BACKEND_ID,
  KHALA_CODE_GPT_OSS_DECIDER_BACKEND_ID,
  KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION,
  normalizeKhalaCodeOnDeviceDeciderPlatform,
  selectKhalaCodeOnDeviceDecider,
} from "../src/shared/on-device-decider"

describe("Khala Code on-device decider selection", () => {
  test("is off by default and fails soft without selecting a backend", () => {
    const selection = selectKhalaCodeOnDeviceDecider({
      platform: "darwin",
      appleFmAvailable: true,
    })

    expect(selection).toEqual({
      status: "disabled",
      enabled: false,
      platform: "macos",
      backend: null,
      failSoft: true,
      blockerRefs: ["blocker.khala_code.on_device_decider.disabled"],
    })
  })

  test("normalizes host platform signals into the public contract", () => {
    expect(normalizeKhalaCodeOnDeviceDeciderPlatform("darwin")).toBe("macos")
    expect(normalizeKhalaCodeOnDeviceDeciderPlatform("macos")).toBe("macos")
    expect(normalizeKhalaCodeOnDeviceDeciderPlatform("ios")).toBe("ios")
    expect(normalizeKhalaCodeOnDeviceDeciderPlatform("linux")).toBe("linux")
    expect(normalizeKhalaCodeOnDeviceDeciderPlatform("win32")).toBe("windows")
    expect(normalizeKhalaCodeOnDeviceDeciderPlatform("freebsd")).toBe(
      "unsupported",
    )
  })

  test("selects Apple FM only for opted-in Mac and iOS hosts", () => {
    for (const platform of ["macos", "ios"] as const) {
      const selection = selectKhalaCodeOnDeviceDecider({
        enabled: true,
        platform,
        appleFmAvailable: true,
        gptOssAvailable: true,
      })

      expect(selection.status).toBe("ready")
      expect(selection.backend).toEqual({
        id: KHALA_CODE_APPLE_FM_DECIDER_BACKEND_ID,
        kind: "apple_fm",
        interfaceVersion: KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION,
      })
      expect(selection.blockerRefs).toEqual([])
    }
  })

  test("selects self-hosted GPT-OSS only for opted-in non-Mac desktop hosts", () => {
    for (const platform of ["linux", "windows"] as const) {
      const selection = selectKhalaCodeOnDeviceDecider({
        enabled: true,
        platform,
        appleFmAvailable: true,
        gptOssAvailable: true,
      })

      expect(selection.status).toBe("ready")
      expect(selection.backend).toEqual({
        id: KHALA_CODE_GPT_OSS_DECIDER_BACKEND_ID,
        kind: "self_hosted_gpt_oss",
        interfaceVersion: KHALA_CODE_ON_DEVICE_DECIDER_INTERFACE_VERSION,
      })
      expect(selection.blockerRefs).toEqual([])
    }
  })

  test("fails soft when the platform backend is unavailable", () => {
    expect(
      selectKhalaCodeOnDeviceDecider({
        enabled: true,
        platform: "macos",
        appleFmAvailable: false,
        gptOssAvailable: true,
      }),
    ).toMatchObject({
      status: "unavailable",
      backend: null,
      failSoft: true,
      blockerRefs: [
        "blocker.khala_code.on_device_decider.apple_fm_unavailable",
      ],
    })

    expect(
      selectKhalaCodeOnDeviceDecider({
        enabled: true,
        platform: "linux",
        gptOssAvailable: false,
      }),
    ).toMatchObject({
      status: "unavailable",
      backend: null,
      failSoft: true,
      blockerRefs: [
        "blocker.khala_code.on_device_decider.gpt_oss_unavailable",
      ],
    })
  })

  test("does not hard fail unsupported platforms", () => {
    expect(
      selectKhalaCodeOnDeviceDecider({
        enabled: true,
        platform: "freebsd",
      }),
    ).toEqual({
      status: "unavailable",
      enabled: true,
      platform: "unsupported",
      backend: null,
      failSoft: true,
      blockerRefs: [
        "blocker.khala_code.on_device_decider.unsupported_platform",
      ],
    })
  })
})
