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
    runtimeVersion?: { policy?: string }
    updates?: {
      enabled?: boolean
      url?: string
      requestHeaders?: Record<string, string>
    }
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

  test("OTA feed is the owned OpenAgents Updates server on the app's OWN channel", () => {
    // The owned oa-updates server (never EAS / Expo CDN) …
    expect(appConfig.expo.updates?.enabled).toBe(true)
    expect(appConfig.expo.updates?.url).toBe(
      "https://updates.openagents.com/openagents-mobile/manifest",
    )
    // … on a channel that belongs to THIS app.
    expect(appConfig.expo.updates?.requestHeaders?.["expo-channel-name"]).toBe(
      "openagents-production",
    )
    // Runtime compatibility is fingerprint-based, matching the publish path.
    expect(appConfig.expo.runtimeVersion?.policy).toBe("fingerprint")
  })

  test("OTA feed is NOT any legacy channel (khala / AutopilotRemoteControl / bare production)", () => {
    const serialized = JSON.stringify(appConfig.expo.updates).toLowerCase()
    expect(serialized).not.toContain("khala")
    expect(serialized).not.toContain("autopilot")
    expect(serialized).not.toContain("u.expo.dev")
    expect(
      appConfig.expo.updates?.requestHeaders?.["expo-channel-name"],
    ).not.toBe("production")
  })
})
