import { Effect } from "effect"
import { NavigationError } from "../Browser/errors.js"
import type { Page } from "../Browser/types.js"
import { InteractionError, WaitError } from "./errors.js"
import type { InteractionStep, NavigationOptions, WaitOptions } from "./types.js"

export const click = (page: Page, selector: string, timeout = 5000) =>
  Effect.tryPromise({
    try: async () => {
      await page.instance.waitForSelector(selector, { timeout })
      await page.instance.click(selector)
    },
    catch: (error) =>
      new InteractionError({
        action: "click",
        selector,
        message: `Failed to click element: ${error}`,
        cause: error
      })
  })

export const fill = (page: Page, selector: string, value: string, timeout = 5000) =>
  Effect.tryPromise({
    try: async () => {
      await page.instance.waitForSelector(selector, { timeout })
      await page.instance.type(selector, value)
    },
    catch: (error) =>
      new InteractionError({
        action: "fill",
        selector,
        message: `Failed to fill element: ${error}`,
        cause: error
      })
  })

export const select = (page: Page, selector: string, value: string, timeout = 5000) =>
  Effect.tryPromise({
    try: async () => {
      await page.instance.waitForSelector(selector, { timeout })
      await page.instance.select(selector, value)
    },
    catch: (error) =>
      new InteractionError({
        action: "select",
        selector,
        message: `Failed to select option: ${error}`,
        cause: error
      })
  })

export const navigate = (page: Page, url: string, options?: NavigationOptions) =>
  Effect.tryPromise({
    try: () =>
      page.instance.goto(url, {
        waitUntil: options?.waitUntil ?? "networkidle0",
        timeout: options?.timeout ?? 30000
      }),
    catch: (error) =>
      new NavigationError({
        url,
        message: `Failed to navigate: ${error}`,
        cause: error
      })
  })

export const waitForSelector = (page: Page, selector: string, options?: WaitOptions) =>
  Effect.tryPromise({
    try: () =>
      page.instance.waitForSelector(selector, {
        timeout: options?.timeout ?? 5000
      }),
    catch: (error) =>
      new WaitError({
        condition: `selector: ${selector}`,
        timeout: options?.timeout ?? 5000,
        message: `Timeout waiting for selector: ${error}`
      })
  })

export const waitForNavigation = (page: Page, options?: NavigationOptions) =>
  Effect.tryPromise({
    try: () =>
      page.instance.waitForNavigation({
        waitUntil: options?.waitUntil ?? "networkidle0",
        timeout: options?.timeout ?? 30000
      }),
    catch: (error) =>
      new WaitError({
        condition: "navigation",
        timeout: options?.timeout ?? 30000,
        message: `Timeout waiting for navigation: ${error}`
      })
  })

export const waitForLoadState = (page: Page, state: "load" | "domcontentloaded" | "networkidle" = "networkidle") =>
  Effect.tryPromise({
    try: async () => {
      // Puppeteer doesn't have waitForLoadState, so we use waitForNavigation
      if (state === "networkidle") {
        await page.instance.waitForNavigation({ waitUntil: "networkidle0" }).catch(() => {})
      } else {
        await page.instance.waitForNavigation({ waitUntil: state }).catch(() => {})
      }
    },
    catch: (error) =>
      new WaitError({
        condition: `load state: ${state}`,
        timeout: 30000,
        message: `Failed to wait for load state: ${error}`
      })
  })

export const performInteractions = (page: Page, steps: ReadonlyArray<InteractionStep>) =>
  Effect.forEach(steps, (step) =>
    Effect.gen(function*() {
      switch (step.action) {
        case "click":
          yield* click(page, step.selector!, step.timeout)
          break
        case "fill":
          yield* fill(page, step.selector!, step.value!, step.timeout)
          break
        case "select":
          yield* select(page, step.selector!, step.value!, step.timeout)
          break
        case "wait":
          yield* waitForSelector(page, step.selector!, {
            timeout: step.timeout !== undefined ? step.timeout : 5000
          })
          break
        case "navigate":
          yield* navigate(page, step.value!, {
            timeout: step.timeout !== undefined ? step.timeout : 30000
          })
          break
        default:
          yield* Effect.fail(
            new InteractionError({
              action: step.action,
              message: `Unknown interaction action: ${step.action}`
            })
          )
      }
    }).pipe(
      Effect.catchAll((error) =>
        Effect.fail(
          new InteractionError({
            action: step.action,
            selector: step.selector,
            message: `${step.action} failed: ${error}`,
            cause: error
          })
        )
      )
    ), { concurrency: 1 })
