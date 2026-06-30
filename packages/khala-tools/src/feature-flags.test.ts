import { describe, expect, test } from "bun:test"
import {
  defineKhalaFeatureRegistry,
  isKhalaFeatureEnabled,
  parseKhalaFeatureFlagArgs,
  type KhalaFeatureSpecType,
} from "./index.js"

const registry = defineKhalaFeatureRegistry([
  {
    defaultEnabled: false,
    description: "Gate a work-in-progress coding lane.",
    name: "lane-k",
    stage: "under-development",
  },
  {
    defaultEnabled: false,
    name: "browser-tools",
    stage: "experimental",
  },
  {
    defaultEnabled: true,
    name: "local-read",
    stage: "stable",
  },
  {
    defaultEnabled: false,
    name: "legacy-shell",
    stage: "deprecated",
  },
  {
    defaultEnabled: false,
    name: "old-router",
    stage: "removed",
  },
] as const satisfies ReadonlyArray<KhalaFeatureSpecType>)

type TestFeatureName = ReturnType<typeof registry.list>[number]["name"]

describe("Khala feature flags", () => {
  test("exposes a static typed registry with stages and defaults", () => {
    expect(registry.list()).toEqual([
      {
        defaultEnabled: false,
        description: "Gate a work-in-progress coding lane.",
        name: "lane-k",
        stage: "under-development",
      },
      { defaultEnabled: false, name: "browser-tools", stage: "experimental" },
      { defaultEnabled: true, name: "local-read", stage: "stable" },
      { defaultEnabled: false, name: "legacy-shell", stage: "deprecated" },
      { defaultEnabled: false, name: "old-router", stage: "removed" },
    ])

    const resolution = registry.resolve()
    expect(resolution.enabled).toEqual({
      "browser-tools": false,
      "lane-k": false,
      "legacy-shell": false,
      "local-read": true,
      "old-router": false,
    })
    expect(resolution.enabledFeatures).toEqual(["local-read"])
    expect(isKhalaFeatureEnabled(resolution, "local-read")).toBe(true)
  })

  test("applies [features] config table booleans over defaults", () => {
    const resolution = registry.resolve({
      config: {
        features: {
          "browser-tools": true,
          "local-read": false,
        },
      },
    })

    expect(resolution.enabledFeatures).toEqual(["browser-tools"])
    expect(resolution.disabledFeatures).toEqual(["lane-k", "local-read", "legacy-shell", "old-router"])
  })

  test("desugars ordered --enable/--disable arguments into runtime overrides", () => {
    const parsed = parseKhalaFeatureFlagArgs<TestFeatureName>([
      "--model",
      "openagents/khala",
      "--enable",
      "lane-k,browser-tools",
      "--disable=lane-k",
      "--enable=legacy-shell",
    ])

    expect(parsed.passthroughArgs).toEqual(["--model", "openagents/khala"])
    expect(parsed.overrides).toEqual([
      { enabled: true, name: "lane-k", source: "cli" },
      { enabled: true, name: "browser-tools", source: "cli" },
      { enabled: false, name: "lane-k", source: "cli" },
      { enabled: true, name: "legacy-shell", source: "cli" },
    ])

    const resolution = registry.resolve({
      config: { features: { "legacy-shell": false } },
      overrides: parsed.overrides,
    })
    expect(resolution.enabled).toMatchObject({
      "browser-tools": true,
      "lane-k": false,
      "legacy-shell": true,
      "local-read": true,
    })
  })

  test("rejects unknown, invalid, duplicate, and removed feature flips", () => {
    expect(() =>
      registry.resolve({ config: { features: { missing: true } as Partial<Record<TestFeatureName, boolean>> } }),
    ).toThrow("unknown_feature_flag")
    expect(() => parseKhalaFeatureFlagArgs(["--enable", "BadFlag"])).toThrow("invalid_feature_flag_name")
    expect(() => defineKhalaFeatureRegistry([
      { defaultEnabled: false, name: "same", stage: "experimental" },
      { defaultEnabled: true, name: "same", stage: "stable" },
    ])).toThrow("duplicate_feature_flag")
    expect(() => registry.resolve({ overrides: [{ enabled: true, name: "old-router", source: "cli" }] })).toThrow(
      "removed_feature_flag",
    )
  })
})
