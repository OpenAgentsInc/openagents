import {
  type Browser,
  BrowserService,
  click,
  fill,
  navigate,
  type Page,
  ScreenshotService,
  waitForSelector
} from "@openagentsinc/autotest"
import { Effect } from "effect"
import type { PsionicApp } from "../core/app.js"

export interface PsionicTestContext {
  readonly app: PsionicApp
  readonly browser: Browser
  readonly page: Page
  readonly baseUrl: string
}

export const createPsionicTestContext = (app: PsionicApp) =>
  Effect.gen(function*() {
    const browserService = yield* BrowserService
    const browser = yield* browserService.launch({ headless: true })
    const page = yield* browserService.newPage(browser)
    const baseUrl = `http://localhost:${app.config.port ?? 3000}`

    return {
      app,
      browser,
      page,
      baseUrl
    } as PsionicTestContext
  })

export const closePsionicTestContext = (context: PsionicTestContext) =>
  Effect.gen(function*() {
    const browserService = yield* BrowserService
    yield* browserService.closePage(context.page)
    yield* browserService.close(context.browser)
  })

export const testPsionicApp = (app: PsionicApp) =>
  Effect.gen(function*() {
    const context = yield* createPsionicTestContext(app)

    return {
      visit: (path: string) => navigate(context.page, `${context.baseUrl}${path}`),

      expectHypermedia: (selector: string, timeout = 5000) => waitForSelector(context.page, selector, { timeout }),

      triggerHTMX: (selector: string) =>
        Effect.gen(function*() {
          // Wait for HTMX to be ready
          yield* Effect.tryPromise({
            try: () =>
              context.page.instance.waitForFunction(
                () => typeof (window as any).htmx !== "undefined",
                { timeout: 5000 }
              ),
            catch: (error) => new Error(`HTMX not loaded: ${error}`)
          })

          // Click the element
          yield* click(context.page, selector)

          // Wait for HTMX request to complete
          yield* Effect.tryPromise({
            try: () =>
              context.page.instance.waitForFunction(
                () => (window as any).htmx.find("body").classList.contains("htmx-request") === false,
                { timeout: 10000 }
              ),
            catch: (error) => new Error(`HTMX request timeout: ${error}`)
          })
        }),

      expectComponent: (componentName: string) =>
        waitForSelector(context.page, `[data-psionic-component="${componentName}"]`),

      captureScreenshot: (name: string) =>
        Effect.gen(function*() {
          const screenshotService = yield* ScreenshotService
          const screenshot = yield* screenshotService.capture({
            page: context.page,
            fullPage: true
          })
          const path = `.autotest/screenshots/psionic/${name}-${Date.now()}.png`
          yield* screenshotService.save(screenshot, path)
          return path
        }),

      fillForm: (formData: Record<string, string>) =>
        Effect.forEach(
          Object.entries(formData),
          ([selector, value]) => fill(context.page, selector, value),
          { concurrency: 1 }
        ),

      submitForm: (formSelector: string) =>
        Effect.gen(function*() {
          yield* Effect.tryPromise({
            try: () =>
              context.page.instance.evaluate((selector: string) => {
                const form = document.querySelector(selector) as HTMLFormElement
                if (form) {
                  form.submit()
                } else {
                  throw new Error(`Form not found: ${selector}`)
                }
              }, formSelector),
            catch: (error) => new Error(`Failed to submit form: ${error}`)
          })
        }),

      cleanup: () => closePsionicTestContext(context)
    }
  })

// Helper for testing component explorer
export const testComponentExplorer = (app: PsionicApp) =>
  Effect.gen(function*() {
    const test = yield* testPsionicApp(app)
    const componentsPath = app.config.componentsPath ?? "/components"

    yield* test.visit(componentsPath)
    yield* test.expectHypermedia(".psionic-component-explorer")

    return {
      ...test,
      selectStory: (storyName: string) => test.triggerHTMX(`[data-story="${storyName}"]`),

      expectStoryContent: (content: string) =>
        waitForSelector(test["page" as keyof typeof test] as Page, `.story-content:contains("${content}")`)
    }
  })

// Integration test helper
export const withPsionicTest = <R, E, A>(
  app: PsionicApp,
  test: (context: any) => Effect.Effect<A, E, R>
) =>
  Effect.gen(function*() {
    const testContext = yield* testPsionicApp(app)
    try {
      return yield* test(testContext)
    } finally {
      yield* testContext.cleanup()
    }
  })
