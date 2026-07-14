/**
 * Legacy desktop lockout oracles (CUT-26, openagents#8706).
 *
 * The deprecated Electrobun desktop clients (khala-code-desktop /
 * autopilot-desktop) must not be sustainable as live coding surfaces from
 * this distribution/update authority post-cutover: every legacy serving
 * surface answers a typed 410 lockout document when the lockout is armed —
 * and the lockout is ARMED BY DEFAULT, disarmable only by the exact
 * documented archival value.
 */
import { describe, expect, test } from "vite-plus/test"

import {
  LEGACY_DESKTOP_LOCKOUT_SCHEMA_ID,
  LEGACY_DESKTOP_OTA_SURFACE,
  LEGACY_LOCKED_DESKTOP_PRODUCTS,
  isLegacyLockedDesktopProduct,
  legacyDesktopLockoutBody,
  resolveLegacyDesktopLockoutMode,
} from "./legacy-desktop-lockout.ts"
import { createUpdatesServer } from "./server.ts"

describe("lockout mode resolution (fail closed)", () => {
  test("armed by default and for every unrecognized value", () => {
    expect(resolveLegacyDesktopLockoutMode(undefined)).toBe("armed")
    expect(resolveLegacyDesktopLockoutMode("")).toBe("armed")
    expect(resolveLegacyDesktopLockoutMode("armed")).toBe("armed")
    expect(resolveLegacyDesktopLockoutMode("disarmed")).toBe("armed")
    expect(resolveLegacyDesktopLockoutMode("off")).toBe("armed")
    expect(resolveLegacyDesktopLockoutMode("0")).toBe("armed")
    expect(resolveLegacyDesktopLockoutMode("Disarmed-Historical-Read-Only")).toBe("armed")
  })

  test("disarmed only by the exact documented archival value", () => {
    expect(resolveLegacyDesktopLockoutMode("disarmed-historical-read-only")).toBe(
      "disarmed_historical_read_only",
    )
    expect(resolveLegacyDesktopLockoutMode(" disarmed-historical-read-only ")).toBe(
      "disarmed_historical_read_only",
    )
  })

  test("locked product set covers exactly the deprecated Electrobun apps", () => {
    expect([...LEGACY_LOCKED_DESKTOP_PRODUCTS].sort()).toEqual([
      "autopilot-desktop",
      "khala-code-desktop",
    ])
    expect(isLegacyLockedDesktopProduct("khala-code-desktop")).toBe(true)
    expect(isLegacyLockedDesktopProduct("autopilot-desktop")).toBe(true)
    expect(isLegacyLockedDesktopProduct("openagents-desktop")).toBe(false)
  })
})

const expectLockout = async (response: Response, subject: string) => {
  expect(response.status).toBe(410)
  expect(response.headers.get("cache-control")).toBe("no-store")
  const body = await response.json() as Record<string, unknown>
  expect(body.schema).toBe(LEGACY_DESKTOP_LOCKOUT_SCHEMA_ID)
  expect(body.subject).toBe(subject)
  expect(body.lockedOut).toBe(true)
  expect(body.reason).toBe("legacy_desktop_client_locked_out")
  expect(body.replacementApp).toBe("openagents-desktop")
  expect(body.policyRef).toBe("openagents#8706")
}

describe("server enforcement — armed (the default)", () => {
  const server = createUpdatesServer()

  // Historical rows may still be seeded (read-only data), but ARMED serving
  // refuses them: registration does not reopen the surface.
  server.registerDesktopUpdate(
    "stable",
    {
      version: "1.2.0",
      artifactUrl: "https://updates.openagents.test/assets/legacy",
      sha256: "legacy-sha",
    },
    "khala-code-desktop",
  )
  server.registerDesktopOtaFile("stable-macos-arm64-update.json", "/nonexistent/update.json")

  test("legacy product feed routes answer the typed 410 lockout", async () => {
    await expectLockout(
      await server.fetch(
        new Request("https://updates.openagents.test/desktop/khala-code-desktop/stable/feed.json"),
      ),
      "khala-code-desktop",
    )
    await expectLockout(
      await server.fetch(
        new Request("https://updates.openagents.test/desktop/autopilot-desktop/rc/feed.json"),
      ),
      "autopilot-desktop",
    )
  })

  test("the default-product feed route is locked out", async () => {
    await expectLockout(
      await server.fetch(
        new Request("https://updates.openagents.test/desktop/stable/feed.json"),
      ),
      "autopilot-desktop",
    )
  })

  test("the deprecated Khala Code updater poll URL is locked out", async () => {
    // This is the EXACT URL shape the frozen client's Electrobun updater
    // polls: release.baseUrl (…/desktop/khala-code-desktop) + flat
    // {channel}-{os}-{arch}-update.json.
    await expectLockout(
      await server.fetch(
        new Request(
          "https://updates.openagents.test/desktop/khala-code-desktop/stable-macos-arm64-update.json",
        ),
      ),
      "khala-code-desktop",
    )
  })

  test("flat Electrobun OTA files are locked out even when registered", async () => {
    await expectLockout(
      await server.fetch(
        new Request("https://updates.openagents.test/desktop/stable-macos-arm64-update.json"),
      ),
      LEGACY_DESKTOP_OTA_SURFACE,
    )
  })

  test("non-desktop and greenfield-desktop routes are unaffected", async () => {
    const pylonResponse = await server.fetch(
      new Request("https://updates.openagents.test/pylon/rc/darwin-arm64/feed.json"),
    )
    expect(pylonResponse.status).toBe(200)

    // Unpublished greenfield feed is a plain 404, never a lockout body.
    const greenfield = await server.fetch(
      new Request("https://updates.openagents.test/desktop/openagents-desktop/stable/feed.json"),
    )
    expect(greenfield.status).toBe(404)
  })
})

describe("server enforcement — explicit archival disarm", () => {
  test("historical read-only serving still works when disarmed", async () => {
    const server = createUpdatesServer({
      legacyDesktopLockout: "disarmed_historical_read_only",
    })
    server.registerDesktopUpdate(
      "stable",
      {
        version: "1.2.0",
        artifactUrl: "https://updates.openagents.test/assets/legacy",
        sha256: "legacy-sha",
      },
      "khala-code-desktop",
    )

    const response = await server.fetch(
      new Request("https://updates.openagents.test/desktop/khala-code-desktop/stable/feed.json"),
    )
    expect(response.status).toBe(200)
    expect(await response.json()).toEqual([
      {
        version: "1.2.0",
        artifactUrl: "https://updates.openagents.test/assets/legacy",
        sha256: "legacy-sha",
      },
    ])
  })

  test("lockout body shape is stable", () => {
    expect(legacyDesktopLockoutBody("khala-code-desktop")).toEqual({
      schema: LEGACY_DESKTOP_LOCKOUT_SCHEMA_ID,
      subject: "khala-code-desktop",
      lockedOut: true,
      reason: "legacy_desktop_client_locked_out",
      detail:
        "This deprecated desktop client is retired and is not a supported " +
        "coding surface. Its update/distribution feed is permanently closed. " +
        "Install OpenAgents Desktop instead.",
      replacementApp: "openagents-desktop",
      replacementFeedPath: "/desktop/openagents-desktop/stable/feed.json",
      policyRef: "openagents#8706",
    })
  })
})
