import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { BrowserService, BrowserServiceLive } from "../src/Browser/index.js"

// Skip browser tests in CI where Chrome may not be available
const isCI = process.env.CI === "true" || process.env.GITHUB_ACTIONS === "true"

describe.skip("BrowserService", () => {
  const testName1 = isCI ? "should launch and close browser (skipped in CI)" : "should launch and close browser"
  const testName2 = isCI ? "should create new page (skipped in CI)" : "should create new page"

  it.effect(testName1, () =>
    isCI
      ? Effect.void
      : Effect.gen(function*() {
        const service = yield* BrowserService

        // Launch browser
        const browser = yield* service.launch({ headless: true })
        expect(browser._tag).toBe("Browser")
        expect(browser.instance).toBeDefined()

        // Close browser
        yield* service.close(browser)
      }).pipe(
        Effect.provide(BrowserServiceLive),
        Effect.timeout("10 seconds")
      ))

  it.effect(testName2, () =>
    isCI
      ? Effect.void
      : Effect.gen(function*() {
        const service = yield* BrowserService

        const browser = yield* service.launch({ headless: true })
        const page = yield* service.newPage(browser)

        expect(page._tag).toBe("Page")
        expect(page.instance).toBeDefined()

        yield* service.closePage(page)
        yield* service.close(browser)
      }).pipe(
        Effect.provide(BrowserServiceLive),
        Effect.timeout("10 seconds")
      ))
})
