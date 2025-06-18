import type { Browser as PuppeteerBrowser, Page as PuppeteerPage, PuppeteerLaunchOptions } from "puppeteer"

export interface Browser {
  readonly _tag: "Browser"
  readonly instance: PuppeteerBrowser
}

export interface Page {
  readonly _tag: "Page"
  readonly instance: PuppeteerPage
}

export interface BrowserOptions extends PuppeteerLaunchOptions {
  readonly headless?: boolean
  readonly slowMo?: number
  readonly devtools?: boolean
}
