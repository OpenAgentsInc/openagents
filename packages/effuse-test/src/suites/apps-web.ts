import { Effect, Scope } from "effect"
import * as Path from "node:path"

import { BrowserService } from "../browser/BrowserService.ts"
import type { ProbeService } from "../effect/ProbeService.ts"
import type { TestCase } from "../spec.ts"
import { TestContext } from "../runner/TestContext.ts"
import { assertEqual, assertTrue, step } from "../runner/Test.ts"
import { assertPngSnapshot, snapshotPathForStory } from "../runner/visualSnapshot.ts"

type AppsWebEnv = BrowserService | ProbeService | TestContext | Scope.Scope

export const appsWebSuite = (): ReadonlyArray<TestCase<AppsWebEnv>> => {
  const tests: Array<TestCase<AppsWebEnv>> = [
    {
      id: "apps-web.http.ssr-home",
      tags: ["e2e", "http", "apps/web"],
      timeoutMs: 60_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext

        const res = yield* step(
          "GET /",
          Effect.tryPromise({
            try: () => fetch(`${ctx.baseUrl}/`, { redirect: "manual" }),
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }),
        )

        yield* step(
          "assert status + content-type",
          Effect.gen(function* () {
            yield* assertEqual(res.status, 200, "Expected GET / to return 200")
            const ct = res.headers.get("content-type") ?? ""
            yield* assertTrue(ct.includes("text/html"), `Expected text/html content-type, got: ${ct}`)
          }),
        )

        const html = yield* step(
          "read body",
          Effect.tryPromise({
            try: () => res.text(),
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }),
        )

        yield* step(
          "assert SSR markers present",
          Effect.gen(function* () {
            yield* assertTrue(html.includes("data-effuse-shell"), "Expected SSR HTML to include data-effuse-shell")
            yield* assertTrue(html.includes("data-effuse-outlet"), "Expected SSR HTML to include data-effuse-outlet")
          }),
        )
      }),
    },
    {
      id: "apps-web.navigation.start-for-free",
      tags: ["e2e", "browser", "apps/web"],
      timeoutMs: 120_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))
            yield* step("click Start for free", page.click('a[href="/autopilot"]'))
            yield* step(
              "wait for /autopilot",
              page.waitForFunction("location.pathname === '/autopilot'", { timeoutMs: 15_000 }),
            )

            yield* step(
              "assert autopilot shell exists",
              Effect.gen(function* () {
                const shell = yield* page.evaluate<boolean>("!!document.querySelector('[data-autopilot-shell]')")
                yield* assertTrue(shell, "Expected [data-autopilot-shell] to exist on /autopilot")
              }),
            )
          }),
        )
      }),
    },
    {
      id: "apps-web.http.assets",
      tags: ["e2e", "http", "apps/web"],
      timeoutMs: 60_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext

        const css = yield* step(
          "GET /effuse-client.css",
          Effect.tryPromise({
            try: () => fetch(`${ctx.baseUrl}/effuse-client.css`, { redirect: "manual" }),
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }),
        )
        yield* step(
          "assert css ok",
          Effect.gen(function* () {
            yield* assertEqual(css.status, 200, "Expected GET /effuse-client.css to return 200")
            const ct = css.headers.get("content-type") ?? ""
            yield* assertTrue(ct.includes("text/css"), `Expected text/css content-type, got: ${ct}`)
            const body = yield* Effect.tryPromise({
              try: () => css.text(),
              catch: (error) => (error instanceof Error ? error : new Error(String(error))),
            })
            yield* assertTrue(body.length > 0, "Expected CSS body to be non-empty")
          }),
        )

        const js = yield* step(
          "GET /effuse-client.js",
          Effect.tryPromise({
            try: () => fetch(`${ctx.baseUrl}/effuse-client.js`, { redirect: "manual" }),
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }),
        )
        yield* step(
          "assert js ok",
          Effect.gen(function* () {
            yield* assertEqual(js.status, 200, "Expected GET /effuse-client.js to return 200")
            const ct = js.headers.get("content-type") ?? ""
            yield* assertTrue(ct.includes("javascript"), `Expected javascript content-type, got: ${ct}`)
            const body = yield* Effect.tryPromise({
              try: () => js.text(),
              catch: (error) => (error instanceof Error ? error : new Error(String(error))),
            })
            yield* assertTrue(body.length > 0, "Expected JS body to be non-empty")
          }),
        )
      }),
    },
    {
      id: "apps-web.navigation.back-forward",
      tags: ["e2e", "browser", "apps/web"],
      timeoutMs: 120_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            yield* step("init swap counter", page.addInitScript("window.__effuseSwapCount = 0"))

            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))
            yield* step(
              "pin shell node identity",
              page.evaluate("window.__effuseShell = document.querySelector('[data-effuse-shell]')"),
            )

            yield* step("click Log in", page.click('a[href=\"/login\"]'))
            yield* step(
              "wait for /login",
              page.waitForFunction("location.pathname === '/login'", { timeoutMs: 15_000 }),
            )

            const swapsAfterLogin = yield* step("read swaps after /login", page.evaluate<number>("window.__effuseSwapCount"))

            yield* step("history.back()", page.evaluate("history.back()"))
            yield* step("wait for /", page.waitForFunction("location.pathname === '/'"))

            yield* step(
              "assert back landed on home (no login input) + shell stable",
              Effect.gen(function* () {
                const emailInput = yield* page.evaluate<boolean>("!!document.querySelector('#login-email')")
                const shellStable = yield* page.evaluate<boolean>(
                  "window.__effuseShell === document.querySelector('[data-effuse-shell]')",
                )
                const swapsAfterBack = yield* page.evaluate<number>("window.__effuseSwapCount")

                yield* assertTrue(!emailInput, "Expected #login-email not to exist after navigating back to /")
                yield* assertTrue(shellStable, "Expected shell node identity to remain stable after history.back()")
                yield* assertTrue(swapsAfterBack >= swapsAfterLogin, "Expected swap count not to decrease after history.back()")
              }),
            )

            yield* step("history.forward()", page.evaluate("history.forward()"))
            yield* step("wait for /login", page.waitForFunction("location.pathname === '/login'"))

            yield* step(
              "assert forward landed on /login",
              Effect.gen(function* () {
                const emailInput = yield* page.evaluate<boolean>("!!document.querySelector('#login-email')")
                const swapsAfterForward = yield* page.evaluate<number>("window.__effuseSwapCount")
                yield* assertTrue(emailInput, "Expected #login-email to exist after navigating forward to /login")
                yield* assertTrue(
                  swapsAfterForward >= swapsAfterLogin,
                  "Expected swap count not to decrease after history.forward()",
                )
              }),
            )
          }),
        )
      }),
    },
    {
      id: "apps-web.hydration.strict-no-swap",
      tags: ["e2e", "browser", "apps/web"],
      timeoutMs: 120_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            yield* step("init swap counter", page.addInitScript("window.__effuseSwapCount = 0"))

            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))

            yield* step(
              "assert SSR shell/outlet present",
              Effect.gen(function* () {
                const shell = yield* page.evaluate<boolean>("!!document.querySelector('[data-effuse-shell]')")
                const outlet = yield* page.evaluate<boolean>("!!document.querySelector('[data-effuse-outlet]')")
                yield* assertTrue(shell && outlet, "Expected [data-effuse-shell] and [data-effuse-outlet] to exist")
              }),
            )

            yield* step(
              "assert strict hydration does not swap outlet",
              Effect.gen(function* () {
                const swaps = yield* page.evaluate<number>("window.__effuseSwapCount")
                yield* assertEqual(swaps, 0, "Expected __effuseSwapCount to be 0 after initial load")
              }),
            )

            yield* step(
              "pin shell node identity",
              page.evaluate("window.__effuseShell = document.querySelector('[data-effuse-shell]')"),
            )

            yield* step("click Log in", page.click('a[href=\"/login\"]'))
            yield* step(
              "wait for /login",
              page.waitForFunction("location.pathname === '/login'", { timeoutMs: 15_000 }),
            )

            yield* step(
              "assert navigation swapped outlet and preserved shell",
              Effect.gen(function* () {
                const swaps = yield* page.evaluate<number>("window.__effuseSwapCount")
                const shellStable = yield* page.evaluate<boolean>(
                  "window.__effuseShell === document.querySelector('[data-effuse-shell]')",
                )
                const emailInput = yield* page.evaluate<boolean>("!!document.querySelector('#login-email')")
                yield* assertTrue(swaps > 0, "Expected outlet swap count to increase after navigation")
                yield* assertTrue(shellStable, "Expected shell node identity to remain stable across navigation")
                yield* assertTrue(emailInput, "Expected login email input to exist after navigation")
              }),
            )
          }),
        )
      }),
    },
    {
      id: "apps-web.prod.autopilot.chat-send-shows-response-or-error",
      tags: ["e2e", "browser", "apps/web", "prod"],
      timeoutMs: 180_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            const prompt = `effuse-test:${Date.now()}:${Math.random().toString(16).slice(2)}`

            yield* step("goto /autopilot", page.goto(`${ctx.baseUrl}/autopilot`))
            yield* step(
              "wait for shell",
              page.waitForFunction("!!document.querySelector('[data-autopilot-shell]')", { timeoutMs: 20_000 }),
            )

            const initialAssistantCount = yield* step(
              "count initial assistant messages",
              page.evaluate<number>("document.querySelectorAll('[data-chat-role=\"assistant\"]').length"),
            )

            yield* step("type into chat input", page.fill('[data-autopilot-chat-input=\"1\"]', prompt))
            yield* step("click Send", page.click('[data-autopilot-chat-send=\"1\"]'))

            yield* step(
              "wait for user bubble to contain prompt",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"user\"]')).some(el => (el.textContent || '').includes(${JSON.stringify(
                  prompt,
                )}))`,
                { timeoutMs: 30_000 },
              ),
            )

            yield* step(
              "wait for assistant response OR visible error banner",
              page.waitForFunction(
                `(
                  !!document.querySelector('[data-autopilot-chat-error=\"1\"]')
                  || document.querySelectorAll('[data-chat-role=\"assistant\"]').length > ${initialAssistantCount}
                )`,
                { timeoutMs: 120_000 },
              ),
            )

            yield* step(
              "assert no silent stall",
              Effect.gen(function* () {
                const errorBanner = yield* page.evaluate<boolean>(
                  "!!document.querySelector('[data-autopilot-chat-error=\"1\"]')",
                )
                const assistantCount = yield* page.evaluate<number>(
                  "document.querySelectorAll('[data-chat-role=\"assistant\"]').length",
                )

                yield* assertTrue(
                  errorBanner || assistantCount > initialAssistantCount,
                  "Expected either an assistant message or a visible error banner after sending",
                )
              }),
            )
          }),
        )
      }),
    },
    {
      id: "apps-web.visual.storybook",
      tags: ["e2e", "browser", "apps/web", "visual", "storybook"],
      timeoutMs: 300_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        // Fetch story list from the running server (source of truth for visual suite coverage).
        const storiesRes = yield* step(
          "GET /__storybook/api/stories",
          Effect.tryPromise({
            try: () => fetch(`${ctx.baseUrl}/__storybook/api/stories`, { redirect: "manual" }),
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }),
        )

        const storiesJson = yield* step(
          "read stories json",
          Effect.tryPromise({
            try: async () => (await storiesRes.json()) as { readonly stories: ReadonlyArray<{ readonly id: string }> },
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
          }),
        )

        const storyIds = storiesJson.stories.map((s) => s.id).filter(Boolean)
        yield* step(
          "assert stories present",
          assertTrue(storyIds.length > 0, "Expected storybook API to return at least one story"),
        )

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            yield* step("set viewport", page.setViewport({ width: 1200, height: 800, deviceScaleFactor: 1 }))

            // Improve determinism: disable transitions/animations.
            yield* step(
              "inject reduce motion style",
              page.addInitScript(`
                (() => {
                  const style = document.createElement('style');
                  style.setAttribute('data-effuse-test', 'reduce-motion');
                  style.textContent = '*{animation:none!important;transition:none!important;caret-color:transparent!important}';
                  document.documentElement.appendChild(style);
                })();
              `),
            )

            for (const storyId of storyIds) {
              const url = `${ctx.baseUrl}/__storybook/canvas/${encodeURIComponent(storyId)}`
              const fileBase = storyId.replaceAll(/[^a-zA-Z0-9._-]+/g, "_")
              const actualPath = Path.join(ctx.artifactsDir, "storybook", `${fileBase}.png`)
              const diffPath = Path.join(ctx.artifactsDir, "storybook", `${fileBase}.diff.png`)
              const baselinePath = snapshotPathForStory(storyId)

              yield* step(`goto ${storyId}`, page.goto(url))
              yield* step(
                `wait for story ready ${storyId}`,
                page.waitForFunction("!!document.querySelector('[data-story-ready=\"1\"]')", { timeoutMs: 30_000 }),
              )
              yield* step(
                `wait for fonts ${storyId}`,
                page.evaluate("document.fonts ? document.fonts.ready.then(() => true) : true"),
              )

              yield* step(`screenshot ${storyId}`, page.screenshot(actualPath))
              yield* step(
                `compare snapshot ${storyId}`,
                assertPngSnapshot({
                  name: storyId,
                  actualPngPath: actualPath,
                  diffPngPath: diffPath,
                  baselinePngPath: baselinePath,
                }),
              )
            }
          }),
        )
      }),
    },
  ]

  const magicEmail = process.env.EFFUSE_TEST_MAGIC_EMAIL
  const magicCode = process.env.EFFUSE_TEST_MAGIC_CODE
  if (magicEmail && magicCode) {
    tests.push({
      id: "apps-web.prod.autopilot.login-via-chat",
      tags: ["e2e", "browser", "apps/web", "prod", "auth"],
      timeoutMs: 180_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            yield* step("goto /autopilot", page.goto(`${ctx.baseUrl}/autopilot`))
            yield* step(
              "wait for auth panel closed state",
              page.waitForFunction("!!document.querySelector('[data-autopilot-auth-step=\"closed\"]')", {
                timeoutMs: 30_000,
              }),
            )

            yield* step("click Verify email", page.click('[data-autopilot-auth-step=\"closed\"] button'))
            yield* step(
              "wait for email step",
              page.waitForFunction("!!document.querySelector('[data-autopilot-auth-step=\"email\"]')", {
                timeoutMs: 15_000,
              }),
            )

            yield* step(
              "fill email",
              page.fill('[data-autopilot-auth-step=\"email\"] input[name=\"email\"]', magicEmail),
            )
            yield* step("submit email", page.click('[data-autopilot-auth-step=\"email\"] button[type=\"submit\"]'))

            yield* step(
              "wait for code step",
              page.waitForFunction("!!document.querySelector('[data-autopilot-auth-step=\"code\"]')", {
                timeoutMs: 30_000,
              }),
            )

            yield* step(
              "fill code",
              page.fill('[data-autopilot-auth-step=\"code\"] input[name=\"code\"]', magicCode),
            )
            yield* step("submit code", page.click('[data-autopilot-auth-step=\"code\"] button[type=\"submit\"]'))

            yield* step(
              "wait for authed state",
              page.waitForFunction("!!document.querySelector('[data-autopilot-auth-step=\"authed\"]')", {
                timeoutMs: 30_000,
              }),
            )

            yield* step(
              "assert Signed in rendered",
              Effect.gen(function* () {
                const authed = yield* page.evaluate<boolean>(
                  "document.querySelector('[data-autopilot-auth-step=\"authed\"]')?.textContent?.includes('Signed in') ?? false",
                )
                yield* assertTrue(authed, "Expected auth panel to show Signed in after verification")
              }),
            )
          }),
        )
      }),
    })
  }

  return tests
}
