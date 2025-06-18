import { Context, Effect, Layer } from "effect"
import puppeteer from "puppeteer"
import { BrowserError, PageError } from "./errors.js"
import type { Browser, BrowserOptions, Page } from "./types.js"

export class BrowserService extends Context.Tag("@openagentsinc/autotest/BrowserService")<
  BrowserService,
  {
    readonly launch: (options?: BrowserOptions) => Effect.Effect<Browser, BrowserError>
    readonly newPage: (browser: Browser) => Effect.Effect<Page, PageError>
    readonly close: (browser: Browser) => Effect.Effect<void, BrowserError>
    readonly closePage: (page: Page) => Effect.Effect<void, PageError>
  }
>() {}

export const BrowserServiceLive = Layer.succeed(
  BrowserService,
  BrowserService.of({
    launch: (options?: BrowserOptions) =>
      Effect.tryPromise({
        try: async () => {
          const defaultOptions: BrowserOptions = {
            headless: true,
            args: ["--no-sandbox", "--disable-setuid-sandbox"],
            ...options
          }
          const browser = await puppeteer.launch(defaultOptions)
          return { _tag: "Browser", instance: browser } as const
        },
        catch: (error) =>
          new BrowserError({
            message: `Failed to launch browser: ${error}`,
            cause: error
          })
      }),

    newPage: (browser: Browser) =>
      Effect.tryPromise({
        try: async () => {
          const page = await browser.instance.newPage()
          return { _tag: "Page", instance: page } as const
        },
        catch: (error) =>
          new PageError({
            message: `Failed to create new page: ${error}`,
            cause: error
          })
      }),

    close: (browser: Browser) =>
      Effect.tryPromise({
        try: () => browser.instance.close(),
        catch: (error) =>
          new BrowserError({
            message: `Failed to close browser: ${error}`,
            cause: error
          })
      }),

    closePage: (page: Page) =>
      Effect.tryPromise({
        try: () => page.instance.close(),
        catch: (error) =>
          new PageError({
            message: `Failed to close page: ${error}`,
            cause: error
          })
      })
  })
)
