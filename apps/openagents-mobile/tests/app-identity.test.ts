import { describe, expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { join } from "node:path"

/**
 * OpenAgents mobile (#8597) identity oracle — the issue's identity locks,
 * mechanically enforced so they cannot drift:
 *
 * 1. Display/product name: `OpenAgents`.
 * 2. iOS bundle identifier: `com.openagents.app`.
 * 3. Android package/application ID: `com.openagents.app`.
 * 4. The checked-in icon is an exact copy of the canonical Khala Code mobile
 *    icon (`clients/khala-mobile/assets/images/icon.png`), pinned by SHA-256.
 */

const appRoot = join(import.meta.dir, "..")

const appConfig = JSON.parse(
  readFileSync(join(appRoot, "app.json"), "utf8"),
) as {
  expo: {
    name: string
    icon: string
    ios: { bundleIdentifier: string }
    android: { package: string }
    updates?: unknown
  }
}

const PINNED_ICON_SHA256 =
  "0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce"

describe("contract openagents_mobile.identity.v1", () => {
  test("display name is exactly OpenAgents", () => {
    expect(appConfig.expo.name).toBe("OpenAgents")
  })

  test("iOS bundle identifier and Android application ID are exactly com.openagents.app", () => {
    expect(appConfig.expo.ios.bundleIdentifier).toBe("com.openagents.app")
    expect(appConfig.expo.android.package).toBe("com.openagents.app")
  })

  test("checked-in icon is the exact pinned OpenAgents icon (SHA-256)", () => {
    expect(appConfig.expo.icon).toBe("./assets/images/icon.png")
    const digest = createHash("sha256")
      .update(readFileSync(join(appRoot, "assets/images/icon.png")))
      .digest("hex")
    expect(digest).toBe(PINNED_ICON_SHA256)
  })

  test("no OTA update feed is configured yet (owned oa-updates channel lands later, never a legacy Khala feed)", () => {
    expect(appConfig.expo.updates).toBeUndefined()
  })
})
