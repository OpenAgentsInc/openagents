import { Effect } from "effect"
import {
  KhalaToolRuntimeError,
  type KhalaBrowserActionInput,
  type KhalaBrowserNavigateInput,
  type KhalaBrowserPageSnapshot,
  type KhalaBrowserReadDomResult,
  type KhalaBrowserReadInput,
  type KhalaBrowserReadTextResult,
  type KhalaBrowserScreenshotInput,
  type KhalaBrowserScreenshotResult,
  type KhalaBrowserService,
  type KhalaBrowserTypeInput,
  type KhalaBrowserWaitInput,
  type KhalaBrowserWaitResult,
} from "@openagentsinc/khala-tools"

type PlaywrightModule = typeof import("playwright")
type PlaywrightBrowser = Awaited<ReturnType<PlaywrightModule["chromium"]["launch"]>>
type PlaywrightPage = Awaited<ReturnType<PlaywrightBrowser["newPage"]>>

export function createPlaywrightKhalaBrowserService(): KhalaBrowserService {
  const host = new LazyPlaywrightBrowserHost()
  return {
    click: input => host.usePage(page => click(page, input)),
    marker: "khala.browser_service",
    navigate: input => host.usePage(page => navigate(page, input)),
    readDom: input => host.usePage(page => readDom(page, input)),
    readText: input => host.usePage(page => readText(page, input)),
    screenshot: input => host.usePage(page => screenshot(page, input)),
    typeText: input => host.usePage(page => typeText(page, input)),
    waitFor: input => host.usePage(page => waitFor(page, input)),
  }
}

class LazyPlaywrightBrowserHost {
  private browser: PlaywrightBrowser | null = null
  private page: PlaywrightPage | null = null

  usePage<A>(
    fn: (page: PlaywrightPage) => Promise<A>,
  ): Effect.Effect<A, KhalaToolRuntimeError> {
    return Effect.tryPromise({
      try: async () => fn(await this.getPage()),
      catch: error => new KhalaToolRuntimeError({
        code: "browser_unavailable",
        reason: error instanceof Error ? error.message : String(error),
      }),
    })
  }

  private async getPage(): Promise<PlaywrightPage> {
    if (this.page !== null) return this.page
    const playwright = await import("playwright")
    this.browser = await playwright.chromium.launch({ headless: true })
    this.page = await this.browser.newPage({ viewport: { width: 1280, height: 900 } })
    return this.page
  }
}

async function navigate(page: PlaywrightPage, input: KhalaBrowserNavigateInput): Promise<KhalaBrowserPageSnapshot> {
  await page.goto(input.url, { timeout: input.timeoutMs, waitUntil: "domcontentloaded" })
  return snapshot(page)
}

async function click(page: PlaywrightPage, input: KhalaBrowserActionInput): Promise<KhalaBrowserPageSnapshot> {
  await page.locator(input.selector).click({ timeout: input.timeoutMs })
  return snapshot(page)
}

async function typeText(page: PlaywrightPage, input: KhalaBrowserTypeInput): Promise<KhalaBrowserPageSnapshot> {
  await page.locator(input.selector).fill(input.text, { timeout: input.timeoutMs })
  return snapshot(page)
}

async function readText(page: PlaywrightPage, input: KhalaBrowserReadInput): Promise<KhalaBrowserReadTextResult> {
  const text = input.selector === undefined
    ? await page.locator("body").innerText({ timeout: 5_000 })
    : await page.locator(input.selector).innerText({ timeout: 5_000 })
  return {
    ...(await snapshot(page)),
    text,
  }
}

async function readDom(page: PlaywrightPage, input: KhalaBrowserReadInput): Promise<KhalaBrowserReadDomResult> {
  const html = input.selector === undefined
    ? await page.content()
    : await page.locator(input.selector).evaluate(element => element.outerHTML)
  return {
    ...(await snapshot(page)),
    html,
  }
}

async function waitFor(page: PlaywrightPage, input: KhalaBrowserWaitInput): Promise<KhalaBrowserWaitResult> {
  if (input.kind === "selector-visible") {
    await page.locator(input.selector ?? "").waitFor({ state: "visible", timeout: input.timeoutMs })
  } else if (input.kind === "text-visible") {
    await page.waitForFunction(
      value => document.body?.innerText.includes(String(value)) === true,
      input.value,
      { timeout: input.timeoutMs },
    )
  } else {
    await page.waitForURL(url => url.href.includes(input.value ?? ""), { timeout: input.timeoutMs })
  }
  return {
    ...(await snapshot(page)),
    met: true,
  }
}

async function screenshot(
  page: PlaywrightPage,
  _input: KhalaBrowserScreenshotInput,
): Promise<KhalaBrowserScreenshotResult> {
  const bytes = await page.screenshot({ type: "png" })
  const viewport = page.viewportSize()
  const dimensions = viewport === null ? {} : { height: viewport.height, width: viewport.width }
  return {
    ...(await snapshot(page)),
    bytes,
    mediaType: "image/png",
    ...dimensions,
  }
}

async function snapshot(page: PlaywrightPage): Promise<KhalaBrowserPageSnapshot> {
  return {
    timestampMs: Date.now(),
    title: await page.title(),
    url: page.url(),
  }
}
