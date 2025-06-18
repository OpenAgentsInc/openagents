import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import { BrowserService, BrowserServiceLive } from "../src/Browser/index.js"

describe("BrowserService", () => {
  it.effect("should launch and close browser", () =>
    Effect.gen(function*() {
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

  it.effect("should create new page", () =>
    Effect.gen(function*() {
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
