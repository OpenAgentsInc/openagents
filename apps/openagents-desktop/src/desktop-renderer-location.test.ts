import { describe, expect, test } from "bun:test"
import {
  DesktopRendererScheme,
  desktopRendererEntryUrl,
  isTrustedDesktopRendererUrl,
} from "./desktop-renderer-location.ts"

describe("Desktop renderer location", () => {
  test("production stays on the privileged custom scheme", () => {
    expect(desktopRendererEntryUrl).toBe(`${DesktopRendererScheme}://renderer/index.html`)
  })

  test("trust is exact for the packaged renderer entry", () => {
    expect(isTrustedDesktopRendererUrl({ trustedEntryUrl: desktopRendererEntryUrl, value: desktopRendererEntryUrl })).toBe(true)
    expect(isTrustedDesktopRendererUrl({ trustedEntryUrl: desktopRendererEntryUrl, value: `${desktopRendererEntryUrl}#other` })).toBe(false)
  })
})
