import { describe, expect, test } from "bun:test"

import { classifyShipModeFromFingerprint } from "../src/coordinator/ship-mode-classify"

describe("ship mode fingerprint classifier", () => {
  test("returns ota when fingerprints match for JS-only changes", () => {
    expect(
      classifyShipModeFromFingerprint({
        previousRuntimeFingerprint: "runtime-1",
        nextRuntimeFingerprint: "runtime-1",
        changedPaths: ["src/screens/Home.tsx"],
      }),
    ).toEqual({
      mode: "ota",
      reason:
        "Runtime fingerprint runtime-1 is unchanged; JS-only changes can ship OTA.",
    })
  })

  test("returns rebuild when fingerprints differ", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-2",
      changedPaths: ["src/screens/Home.tsx"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("Runtime fingerprint changed")
  })

  test("returns rebuild for ios changes even when fingerprints match", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-1",
      changedPaths: ["ios/OpenAgents/AppDelegate.swift"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("ios/OpenAgents/AppDelegate.swift")
  })

  test("returns rebuild for android changes", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-1",
      changedPaths: ["android/app/build.gradle"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("android/app/build.gradle")
  })

  test("returns rebuild for Expo app config changes", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-1",
      changedPaths: ["app.config.ts"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("app.config.ts")
  })

  test("returns rebuild for package metadata changes", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-1",
      changedPaths: ["package.json"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("package.json")
  })

  test("returns rebuild for podspec changes", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-1",
      changedPaths: ["OpenAgents.podspec"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("OpenAgents.podspec")
  })

  test("returns rebuild for Babel config changes", () => {
    const result = classifyShipModeFromFingerprint({
      previousRuntimeFingerprint: "runtime-1",
      nextRuntimeFingerprint: "runtime-1",
      changedPaths: ["babel.config.js"],
    })

    expect(result.mode).toBe("rebuild")
    expect(result.reason).toContain("babel.config.js")
  })
})
