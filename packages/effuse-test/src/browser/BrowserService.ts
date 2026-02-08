import * as ChildProcess from "node:child_process"
import * as Fs from "node:fs/promises"
import * as Net from "node:net"
import * as Os from "node:os"
import * as Path from "node:path"

import { Context, Effect, FiberRef, Layer, Scope } from "effect"

import { ProbeService } from "../effect/ProbeService.ts"
import { CurrentSpanId } from "../effect/span.ts"
import type { BrowserLaunchOptions, SpanId, TestEvent } from "../spec.ts"
import { TestContext } from "../runner/TestContext.ts"
import { connectCdp } from "./cdp.ts"
import type { Page, Viewport } from "./page.ts"

type ChromeTarget = {
  readonly id: string
  readonly webSocketDebuggerUrl: string
}

type ActivePage = {
  readonly page: Page
  readonly screenshot: (filePath: string) => Effect.Effect<void, unknown, ProbeService | TestContext>
  readonly htmlSnapshot: () => Effect.Effect<string, unknown, ProbeService | TestContext>
}

export class BrowserService extends Context.Tag("@openagentsinc/effuse-test/BrowserService")<
  BrowserService,
  {
    readonly withPage: <A, E, R>(
      f: (page: Page) => Effect.Effect<A, E, R>,
    ) => Effect.Effect<A, E, Scope.Scope | R>
    readonly captureFailureArtifacts: (options: {
      readonly screenshotPath: string
      readonly htmlPath: string
    }) => Effect.Effect<boolean, never, ProbeService | TestContext>
  }
>() {}

export type BrowserServiceOptions = BrowserLaunchOptions & {
  readonly chromePath?: string
}

export const BrowserServiceNone: Layer.Layer<BrowserService> = Layer.succeed(BrowserService, {
  withPage: () =>
    Effect.fail(
      new Error(
        "BrowserService is disabled (this run selected no browser tests). Add the `browser` tag to a test to enable Chromium.",
      ),
    ) as any,
  captureFailureArtifacts: () => Effect.succeed(false),
})

const asError = (u: unknown): { readonly name: string; readonly message: string } => {
  if (u instanceof Error) return { name: u.name, message: u.message }
  return { name: "UnknownError", message: String(u) }
}

const serviceSpan = <A, E, R>(
  name: string,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E, ProbeService | TestContext | R> =>
  Effect.gen(function* () {
    const ctx = yield* TestContext
    const probe = yield* ProbeService

    const parentSpanId = yield* FiberRef.get(CurrentSpanId)
    const spanId = crypto.randomUUID() as SpanId
    const start = Date.now()

    const started: TestEvent = {
      type: "span.started",
      runId: ctx.runId,
      ts: start,
      testId: ctx.testId,
      spanId,
      parentSpanId,
      name,
      kind: "service",
    }
    yield* probe.emit(started)

    return yield* Effect.locally(CurrentSpanId, spanId)(effect).pipe(
      Effect.tap(() =>
        probe.emit({
          type: "span.finished",
          runId: ctx.runId,
          ts: Date.now(),
          testId: ctx.testId,
          spanId,
          status: "passed",
          durationMs: Date.now() - start,
        }),
      ),
      Effect.tapError((error) =>
        probe.emit({
          type: "span.finished",
          runId: ctx.runId,
          ts: Date.now(),
          testId: ctx.testId,
          spanId,
          status: "failed",
          durationMs: Date.now() - start,
          error: asError(error),
        }),
      ),
    )
  })

const getFreePort = (): Promise<number> =>
  new Promise((resolve, reject) => {
    const server = Net.createServer()
    server.on("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const addr = server.address()
      if (addr && typeof addr !== "string") {
        const port = addr.port
        server.close(() => resolve(port))
      } else {
        server.close(() => reject(new Error("Failed to acquire free port")))
      }
    })
  })

const findChrome = async (explicit?: string): Promise<string> => {
  if (explicit) return explicit
  const envPath = process.env.EFFUSE_TEST_CHROME_PATH
  if (envPath) return envPath

  if (process.platform === "darwin") {
    const candidates = [
      "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "/Applications/Chromium.app/Contents/MacOS/Chromium",
      "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    ]
    for (const c of candidates) {
      try {
        await Fs.access(c)
        return c
      } catch {
        // continue
      }
    }
  }

  // Fall back to PATH resolution.
  const candidates = ["google-chrome", "chromium", "chromium-browser", "chrome"]
  for (const bin of candidates) {
    try {
      const which = await new Promise<string>((resolve, reject) => {
        ChildProcess.exec(`command -v ${bin}`, (err, stdout) => {
          if (err) reject(err)
          else resolve(stdout.trim())
        })
      })
      if (which) return which
    } catch {
      // continue
    }
  }

  throw new Error(
    "Could not find a Chromium/Chrome executable. Set EFFUSE_TEST_CHROME_PATH to an absolute path.",
  )
}

const fetchJson = async <A>(url: string, init?: RequestInit): Promise<A> => {
  const res = await fetch(url, init)
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} for ${url}`)
  }
  return (await res.json()) as A
}

const waitForHttpOk = async (url: string, timeoutMs: number): Promise<void> => {
  const start = Date.now()
  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      const res = await fetch(url, { redirect: "manual" })
      if (res.ok) return
    } catch {
      // ignore
    }
    if (Date.now() - start > timeoutMs) {
      throw new Error(`Timed out waiting for ${url}`)
    }
    await new Promise((r) => setTimeout(r, 100))
  }
}

export const BrowserServiceLive = (options: BrowserServiceOptions): Layer.Layer<BrowserService> =>
  Layer.scoped(
    BrowserService,
    Effect.gen(function* () {
      const chromePath = yield* Effect.promise(() => findChrome(options.chromePath))
      const port = yield* Effect.promise(getFreePort)
      const userDataDir = yield* Effect.promise(() => Fs.mkdtemp(Path.join(Os.tmpdir(), "effuse-test-chrome-")))

      const args: Array<string> = [
        `--remote-debugging-port=${port}`,
        "--no-first-run",
        "--no-default-browser-check",
        `--user-data-dir=${userDataDir}`,
        "--disable-background-networking",
        "--disable-background-timer-throttling",
        "--disable-client-side-phishing-detection",
        "--disable-default-apps",
        "--disable-hang-monitor",
        "--disable-popup-blocking",
        "--disable-prompt-on-repost",
        "--disable-sync",
        "--metrics-recording-only",
        "--no-sandbox",
      ]
      if (options.headless) args.push("--headless=new")

      const child = ChildProcess.spawn(chromePath, args, {
        stdio: "ignore",
        detached: true,
      })

      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          try {
            if (child.pid != null) process.kill(-child.pid, "SIGTERM")
            else child.kill("SIGTERM")
          } catch {
            // ignore
          }
        }),
      )
      yield* Effect.addFinalizer(() =>
        Effect.promise(() => Fs.rm(userDataDir, { recursive: true, force: true })).pipe(Effect.catchAll(() => Effect.void)),
      )

      yield* Effect.promise(() => waitForHttpOk(`http://127.0.0.1:${port}/json/version`, 30_000))

      let active: ActivePage | undefined

      const openPage = Effect.acquireRelease(
        Effect.gen(function* () {
          const target = yield* Effect.promise(() =>
            fetchJson<ChromeTarget>(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" }),
          )
          const session = yield* Effect.promise(() => connectCdp(target.webSocketDebuggerUrl))

          yield* Effect.promise(() => session.send("Page.enable"))
          yield* Effect.promise(() => session.send("Runtime.enable"))

          const evaluate = <A = unknown>(fnOrExpression: string | (() => unknown)) =>
            serviceSpan("page.evaluate", Effect.gen(function* () {
              const expression =
                typeof fnOrExpression === "string" ? fnOrExpression : `(${fnOrExpression.toString()})()`
              const res: any = yield* Effect.promise(() =>
                session.send("Runtime.evaluate", {
                  expression,
                  awaitPromise: true,
                  returnByValue: true,
                }),
              )
              if (res.exceptionDetails) {
                const details = res.exceptionDetails
                const description =
                  details.exception?.description ??
                  details.exception?.value ??
                  details.text ??
                  "Runtime.evaluate exception"
                throw new Error(description)
              }
              return res.result?.value as A
            }))

          const addInitScript = (source: string) =>
            serviceSpan("page.addInitScript", Effect.promise(() => session.send("Page.addScriptToEvaluateOnNewDocument", { source })))

          const setViewport = (viewport: Viewport) =>
            serviceSpan(
              "page.setViewport",
              Effect.promise(() =>
                session.send("Emulation.setDeviceMetricsOverride", {
                  width: viewport.width,
                  height: viewport.height,
                  deviceScaleFactor: viewport.deviceScaleFactor ?? 1,
                  mobile: false,
                }),
              ),
            )

          const goto = (url: string) =>
            serviceSpan(`page.goto ${url}`, Effect.gen(function* () {
              yield* Effect.promise(() => session.send("Page.navigate", { url }))
              yield* Effect.promise(() => session.waitForEvent("Page.loadEventFired", 30_000))
            }))

          const click = (selector: string) =>
            serviceSpan(`page.click ${selector}`, evaluate<void>(`(() => {
  const el = document.querySelector(${JSON.stringify(selector)});
  if (!el) throw new Error('Selector not found: ' + ${JSON.stringify(selector)});
  el.click();
})()`))

          const fill = (selector: string, value: string) =>
            serviceSpan(
              `page.fill ${selector}`,
              evaluate<void>(`(() => {
  const el = document.querySelector(${JSON.stringify(selector)});
  if (!el) throw new Error('Selector not found: ' + ${JSON.stringify(selector)});
  el.focus?.();
  el.value = ${JSON.stringify(value)};
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
})()`),
            )

          const type = (selector: string, text: string) =>
            serviceSpan(
              `page.type ${selector}`,
              evaluate<void>(`(() => {
  const el = document.querySelector(${JSON.stringify(selector)});
  if (!el) throw new Error('Selector not found: ' + ${JSON.stringify(selector)});
  el.focus?.();
  el.value = (el.value ?? '') + ${JSON.stringify(text)};
  el.dispatchEvent(new Event('input', { bubbles: true }));
})()`),
            )

          const waitForFunction = (fnOrExpression: string | (() => unknown), options?: { timeoutMs?: number; intervalMs?: number }) =>
            serviceSpan("page.waitForFunction", Effect.gen(function* () {
              const timeoutMs = options?.timeoutMs ?? 10_000
              const intervalMs = options?.intervalMs ?? 100
              const deadline = Date.now() + timeoutMs

              // eslint-disable-next-line no-constant-condition
              while (true) {
                const ok = yield* evaluate<boolean>(fnOrExpression as any)
                if (ok) return
                if (Date.now() > deadline) {
                  throw new Error("Timed out in waitForFunction")
                }
                yield* Effect.sleep(`${intervalMs} millis`)
              }
            }))

          const htmlSnapshot = () => evaluate<string>("document.documentElement.outerHTML")

          const screenshot = (filePath: string) =>
            serviceSpan(
              "page.screenshot",
              Effect.gen(function* () {
                const res: any = yield* Effect.promise(() =>
                  session.send("Page.captureScreenshot", { format: "png" }),
                )
                const bytes = Buffer.from(res.data, "base64")
                yield* Effect.promise(() => Fs.mkdir(Path.dirname(filePath), { recursive: true }))
                yield* Effect.promise(() => Fs.writeFile(filePath, bytes))
              }),
            )

          const close = Effect.promise(() => session.close()).pipe(Effect.catchAll(() => Effect.void))

          const page: Page = {
            addInitScript,
            setViewport,
            goto,
            click,
            fill,
            type,
            evaluate,
            waitForFunction,
            htmlSnapshot,
            screenshot,
            close,
          }

          active = { page, screenshot, htmlSnapshot }

          return page
        }),
        (_page) =>
          Effect.sync(() => {
            active = undefined
          }),
      )

      const withPage = <A, E, R>(f: (page: Page) => Effect.Effect<A, E, R>) =>
        Effect.scoped(
          Effect.acquireUseRelease(openPage, (page) => f(page), (page) => page.close),
        ) as Effect.Effect<A, E, Scope.Scope | R>

      const captureFailureArtifacts = (opts: { screenshotPath: string; htmlPath: string }) =>
        Effect.gen(function* () {
          if (!active) return false
          yield* active.screenshot(opts.screenshotPath).pipe(Effect.catchAll(() => Effect.void))
          const html = yield* active.htmlSnapshot().pipe(Effect.catchAll(() => Effect.succeed("<!doctype html><h1>htmlSnapshot failed</h1>")))
          yield* Effect.promise(() => Fs.mkdir(Path.dirname(opts.htmlPath), { recursive: true }))
          yield* Effect.promise(() => Fs.writeFile(opts.htmlPath, html, "utf8")).pipe(Effect.catchAll(() => Effect.void))
          return true
        })

      return BrowserService.of({ withPage, captureFailureArtifacts })
    }),
  )
