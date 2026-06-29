import { describe, expect, test } from "bun:test"

import { validateExpoManifest } from "./manifest-validate"

const validManifest = {
  id: "update_2026_06_13_001",
  createdAt: "2026-06-13T12:34:56.000Z",
  runtimeVersion: "1.0.0",
  launchAsset: {
    key: "bundles/ios/main.js",
    contentType: "application/javascript",
    url: "https://updates.openagents.com/assets/launch_hash_abc123",
  },
  assets: [],
  metadata: {},
  extra: {},
}

describe("validateExpoManifest", () => {
  test("accepts a manifest with all required keys", () => {
    expect(validateExpoManifest(validManifest)).toEqual({
      ok: true,
      missing: [],
    })
  })

  test("reports missing top-level string keys", () => {
    const { id: _id, createdAt: _createdAt, ...manifest } = validManifest

    expect(validateExpoManifest(manifest)).toEqual({
      ok: false,
      missing: ["id", "createdAt"],
    })
  })

  test("reports blank runtimeVersion as invalid", () => {
    expect(
      validateExpoManifest({
        ...validManifest,
        runtimeVersion: " ",
      }),
    ).toEqual({
      ok: false,
      missing: ["runtimeVersion"],
    })
  })

  test("reports a missing launchAsset object", () => {
    const { launchAsset: _launchAsset, ...manifest } = validManifest

    expect(validateExpoManifest(manifest)).toEqual({
      ok: false,
      missing: ["launchAsset"],
    })
  })

  test("reports missing launchAsset fields", () => {
    expect(
      validateExpoManifest({
        ...validManifest,
        launchAsset: {
          key: "bundles/ios/main.js",
        },
      }),
    ).toEqual({
      ok: false,
      missing: ["launchAsset.contentType", "launchAsset.url"],
    })
  })

  test("requires assets to be an array", () => {
    expect(
      validateExpoManifest({
        ...validManifest,
        assets: {},
      }),
    ).toEqual({
      ok: false,
      missing: ["assets"],
    })
  })

  test("requires metadata and extra to be objects", () => {
    expect(
      validateExpoManifest({
        ...validManifest,
        metadata: [],
        extra: null,
      }),
    ).toEqual({
      ok: false,
      missing: ["metadata", "extra"],
    })
  })

  test("defensively rejects non-object input without throwing", () => {
    expect(validateExpoManifest(null)).toEqual({
      ok: false,
      missing: [
        "id",
        "createdAt",
        "runtimeVersion",
        "launchAsset",
        "assets",
        "metadata",
        "extra",
      ],
    })
  })
})
