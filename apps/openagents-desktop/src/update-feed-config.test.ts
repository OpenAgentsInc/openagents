import { describe, expect, test } from "vite-plus/test"

import { PRODUCTION_RELEASE_KEY_PIN } from "./update-contract.ts"
import {
  DESKTOP_UPDATE_FEED_BASE_URL_ENV,
  DESKTOP_UPDATE_FEED_STAGING_PIN_ENV,
  PRODUCTION_UPDATE_FEED_ORIGIN,
  resolveDesktopUpdateFeedConfig,
} from "./update-feed-config.ts"

const stagingPin = JSON.stringify({ alg: "ed25519", kid: "staging-e2e", x: "A".repeat(43) })

describe("Desktop update feed configuration (REL-FEED-01)", () => {
  test("no overrides resolves the production feed with the production pin, matching the host default", () => {
    for (const channel of ["stable", "rc"] as const) {
      expect(resolveDesktopUpdateFeedConfig({}, channel)).toEqual({
        ok: true,
        source: "production",
        // MUST stay byte-equal with update-staging-host.ts's default baseUrl.
        baseUrl: `https://updates.openagents.com/desktop/openagents/${channel}`,
        pin: PRODUCTION_RELEASE_KEY_PIN,
      })
    }
    expect(PRODUCTION_UPDATE_FEED_ORIGIN).toBe("https://updates.openagents.com")
  })

  test("empty and whitespace-only overrides are treated as unset", () => {
    const resolved = resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "   ",
      [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: "",
    }, "rc")
    expect(resolved).toMatchObject({ ok: true, source: "production" })
  })

  test("an https staging base override appends the fixed channel route and keeps the production pin", () => {
    const resolved = resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "https://oa-updates-staging.example.run.app/prefix/",
    }, "rc")
    expect(resolved).toEqual({
      ok: true,
      source: "staging_override",
      baseUrl: "https://oa-updates-staging.example.run.app/prefix/desktop/openagents/rc",
      pin: PRODUCTION_RELEASE_KEY_PIN,
    })
  })

  test("a staging pin applies only alongside a non-production base override", () => {
    const withOverride = resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "https://oa-updates-staging.example.run.app",
      [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: stagingPin,
    }, "rc")
    expect(withOverride).toMatchObject({
      ok: true,
      source: "staging_override",
      baseUrl: "https://oa-updates-staging.example.run.app/desktop/openagents/rc",
      pin: { kid: "staging-e2e" },
    })

    expect(resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: stagingPin,
    }, "rc")).toEqual({ ok: false, reason: "staging_pin_requires_base_override" })

    expect(resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "https://updates.openagents.com",
      [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: stagingPin,
    }, "rc")).toEqual({ ok: false, reason: "staging_pin_on_production_feed" })
  })

  test("plain http is admissible only toward loopback", () => {
    expect(resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "http://127.0.0.1:8791",
    }, "rc")).toMatchObject({
      ok: true,
      baseUrl: "http://127.0.0.1:8791/desktop/openagents/rc",
    })
    expect(resolveDesktopUpdateFeedConfig({
      [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "http://staging.example.com",
    }, "rc")).toEqual({ ok: false, reason: "feed_base_url_invalid" })
  })

  test("credentials, query, fragment, junk, and oversized base URLs are refused", () => {
    for (const raw of [
      "https://user:secret@staging.example.com",
      "https://staging.example.com/?channel=stable",
      "https://staging.example.com/#fragment",
      "not a url",
      "ftp://staging.example.com",
      `https://staging.example.com/${"a".repeat(1100)}`,
    ]) {
      expect(resolveDesktopUpdateFeedConfig({
        [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: raw,
      }, "rc")).toEqual({ ok: false, reason: "feed_base_url_invalid" })
    }
  })

  test("malformed staging pins and the reserved production kid are refused", () => {
    const base = { [DESKTOP_UPDATE_FEED_BASE_URL_ENV]: "https://staging.example.com" }
    for (const raw of [
      "not json",
      JSON.stringify({ alg: "rsa", kid: "staging", x: "A".repeat(43) }),
      JSON.stringify({ alg: "ed25519", kid: "staging" }),
      JSON.stringify({ alg: "ed25519", kid: "staging", x: "A".repeat(43), d: "PRIVATE" }),
    ]) {
      expect(resolveDesktopUpdateFeedConfig({
        ...base,
        [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: raw,
      }, "rc")).toEqual({ ok: false, reason: "staging_pin_invalid" })
    }
    expect(resolveDesktopUpdateFeedConfig({
      ...base,
      [DESKTOP_UPDATE_FEED_STAGING_PIN_ENV]: JSON.stringify({
        alg: "ed25519",
        kid: PRODUCTION_RELEASE_KEY_PIN.kid,
        x: "A".repeat(43),
      }),
    }, "rc")).toEqual({ ok: false, reason: "staging_pin_kid_reserved" })
  })
})
