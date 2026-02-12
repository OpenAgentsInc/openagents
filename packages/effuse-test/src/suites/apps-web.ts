import { Effect, Scope } from "effect"
import * as Path from "node:path"
import * as Fs from "node:fs/promises"

import { BrowserService } from "../browser/BrowserService.ts"
import { EffuseTestConfig } from "../config/EffuseTestConfig.ts"
import type { ProbeService } from "../effect/ProbeService.ts"
import type { TestCase } from "../spec.ts"
import { TestContext } from "../runner/TestContext.ts"
import { assertEqual, assertTrue, step } from "../runner/Test.ts"
import { assertPngSnapshot, snapshotPathForStory } from "../runner/visualSnapshot.ts"

type AppsWebEnv =
  | BrowserService
  | ProbeService
  | TestContext
  | EffuseTestConfig
  | Scope.Scope

export const appsWebSuite = (): Effect.Effect<
  ReadonlyArray<TestCase<AppsWebEnv>>,
  never,
  EffuseTestConfig
> =>
  Effect.gen(function* () {
    const config = yield* EffuseTestConfig
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
            const reqId = res.headers.get("x-oa-request-id") ?? ""
            yield* assertTrue(reqId.length > 0, "Expected x-oa-request-id response header to be present")
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
      id: "apps-web.prod.http.prelaunch-off",
      tags: ["e2e", "http", "apps/web", "prod"],
      timeoutMs: 60_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext

        const res = yield* step(
          "GET / (prod)",
          Effect.tryPromise({
            try: () => fetch(`${ctx.baseUrl}/`, { redirect: "manual" }),
            catch: (error) => (error instanceof Error ? error : new Error(String(error))),
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
          "assert prelaunch is off markers present",
          Effect.gen(function* () {
            // Prelaunch mode must be stable on initial SSR.
            yield* assertTrue(
              html.includes('meta name="oa-prelaunch" content="0"'),
              "Expected SSR HTML to include <meta name=\"oa-prelaunch\" content=\"0\"> in prod",
            )
            yield* assertTrue(
              !html.includes('data-prelaunch-countdown="1"'),
              "Expected SSR HTML to not include prelaunch countdown markers in prod",
            )
            yield* assertTrue(
              html.includes('data-oa-open-chat-pane="1"'),
              "Expected SSR HTML to include home chat CTA markers in prod",
            )
          }),
        )
      }),
    },
    {
      id: "apps-web.prod.http.legacy-routes-redirect-home",
      tags: ["e2e", "http", "apps/web", "prod"],
      timeoutMs: 60_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext

        const assertRedirectHome = (pathname: string) =>
          Effect.gen(function* () {
            const res = yield* step(
              `GET ${pathname} (prod)`,
              Effect.tryPromise({
                try: () => fetch(`${ctx.baseUrl}${pathname}`, { redirect: "manual" }),
                catch: (error) => (error instanceof Error ? error : new Error(String(error))),
              }),
            )

            yield* step(
              `assert ${pathname} redirected to /`,
              Effect.gen(function* () {
                yield* assertTrue(
                  res.status === 302 || res.status === 301,
                  `Expected redirect status for ${pathname}, got ${res.status}`,
                )
                const loc = res.headers.get("location") ?? ""
                yield* assertTrue(
                  loc === "/" || loc.startsWith("/?"),
                  `Expected Location to be / (or /?*), got: ${loc}`,
                )
              }),
            )
          })

        // Legacy paths: all user flows are now on the homepage.
        yield* assertRedirectHome("/login")
        yield* assertRedirectHome("/autopilot")
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
            yield* step(
              "click Start for free (opens chat pane on same page)",
              page.click("[data-oa-open-chat-pane] a, [data-oa-open-chat-pane]"),
            )
            yield* step(
              "wait for chat pane to open with 'Autopilot initialized. Enter your email address to begin.'",
              page.waitForFunction(
                `!!document.querySelector('[data-pane-id="home-chat"]') && document.querySelector('[data-pane-id="home-chat"]')?.querySelector('[data-oa-pane-content]')?.textContent?.includes('Autopilot initialized. Enter your email address to begin.')`,
                { timeoutMs: 15_000 },
              ),
            )

            yield* step(
              "assert still on home (no navigation)",
              Effect.gen(function* () {
                const pathname = yield* page.evaluate<string>("location.pathname")
                yield* assertEqual(pathname, "/", "Expected to remain on / after opening chat pane")
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
            const reqId = css.headers.get("x-oa-request-id") ?? ""
            yield* assertTrue(reqId.length > 0, "Expected x-oa-request-id on CSS response")
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
            const reqId = js.headers.get("x-oa-request-id") ?? ""
            yield* assertTrue(reqId.length > 0, "Expected x-oa-request-id on JS response")
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
      id: "apps-web.http.l402.paywalls-settlements-deployments",
      tags: ["e2e", "http", "apps/web", "l402"],
      timeoutMs: 90_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext

        const assertEndpoint = (path: string) =>
          Effect.gen(function* () {
            const res = yield* step(
              `GET ${path}`,
              Effect.tryPromise({
                try: () => fetch(`${ctx.baseUrl}${path}`, { redirect: "manual" }),
                catch: (error) => (error instanceof Error ? error : new Error(String(error))),
              }),
            )

            yield* step(
              `assert ${path} status and request correlation`,
              Effect.gen(function* () {
                yield* assertTrue(
                  res.status === 200 || res.status === 401,
                  `Expected ${path} to return 200 or 401, got ${res.status}`,
                )
                const reqId = res.headers.get("x-oa-request-id") ?? ""
                yield* assertTrue(reqId.length > 0, `Expected x-oa-request-id on ${path}`)
              }),
            )

            const body = yield* step(
              `read json ${path}`,
              Effect.tryPromise({
                try: async () => (await res.json()) as Record<string, unknown>,
                catch: (error) => (error instanceof Error ? error : new Error(String(error))),
              }),
            )

            yield* step(
              `assert ${path} deterministic shape`,
              Effect.gen(function* () {
                yield* assertTrue(typeof body.ok === "boolean", `Expected ${path} response to include boolean ok field`)
                if (res.status === 401) {
                  yield* assertTrue(
                    typeof body.error === "string" && body.error.length > 0,
                    `Expected ${path} unauthorized response to include error`,
                  )
                }
              }),
            )
          })

        yield* assertEndpoint("/api/lightning/paywalls?limit=1")
        yield* assertEndpoint("/api/lightning/settlements?limit=1")
        yield* assertEndpoint("/api/lightning/deployments?limit=1")
        yield* assertEndpoint("/api/lightning/deployments/events?limit=1")
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

            yield* step("open chat pane", page.click("[data-oa-open-chat-pane] a, [data-oa-open-chat-pane]"))
            yield* step(
              "wait for chat pane",
              page.waitForFunction("!!document.querySelector('[data-pane-id=\"home-chat\"]')", { timeoutMs: 15_000 }),
            )

            yield* step(
              "assert no outlet swap and shell stable after opening chat pane",
              Effect.gen(function* () {
                const swaps = yield* page.evaluate<number>("window.__effuseSwapCount")
                const shellStable = yield* page.evaluate<boolean>(
                  "window.__effuseShell === document.querySelector('[data-effuse-shell]')",
                )
                const pathname = yield* page.evaluate<string>("location.pathname")
                yield* assertEqual(pathname, "/", "Expected to remain on / after opening chat pane")
                yield* assertEqual(swaps, 0, "Expected __effuseSwapCount to remain 0 after opening chat pane")
                yield* assertTrue(shellStable, "Expected shell node identity to remain stable")
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

  const e2eSecret = config.e2eBypassSecret
  if (e2eSecret && e2eSecret.length > 0) {
    tests.push({
      id: "apps-web.prod.autopilot.e2e-login",
      tags: ["e2e", "browser", "apps/web", "prod", "auth"],
      timeoutMs: 120_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            // Ensure same-origin so cookies are set correctly.
            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))

            const seed = `effuse-test:${ctx.runId}:${ctx.testId}`
            const loginRes = yield* step(
              "POST /api/auth/e2e/login",
              page.evaluate<{ readonly ok: boolean; readonly userId: string }>(`(() => {
  return fetch('/api/auth/e2e/login', {
    method: 'POST',
    cache: 'no-store',
    credentials: 'include',
    headers: {
      'content-type': 'application/json',
      'authorization': 'Bearer ' + ${JSON.stringify(e2eSecret)}
    },
    body: JSON.stringify({ seed: ${JSON.stringify(seed)} })
  }).then(async (res) => {
    const json = await res.json().catch(() => null);
    if (!res.ok || !json || json.ok !== true) {
      throw new Error('e2e login failed: HTTP ' + res.status + ' ' + JSON.stringify(json));
    }
    return json;
  });
})()`),
            )

            yield* step(
              "assert session is authed",
              Effect.gen(function* () {
                const session = yield* page.evaluate<{ readonly ok: boolean; readonly userId: string | null }>(`(() => {
  return fetch('/api/auth/session', { method: 'GET', cache: 'no-store', credentials: 'include' })
    .then(res => res.json())
    .catch(() => null);
})()`)
                yield* assertTrue(session?.ok === true, "Expected /api/auth/session ok === true after e2e login")
                yield* assertTrue(
                  typeof session?.userId === "string" && session.userId.length > 0,
                  "Expected /api/auth/session to include userId after e2e login",
                )
                yield* assertEqual(
                  session.userId,
                  loginRes.userId,
                  "Expected /api/auth/session userId to match /api/auth/e2e/login response",
                )
              }),
            )
          }),
        )
      }),
    })

    tests.push({
      id: "apps-web.prod.autopilot.chat-send-shows-response-or-error",
      tags: ["e2e", "browser", "apps/web", "prod"],
      timeoutMs: 180_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            // Login via E2E bypass first (prod gating requires auth).
            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))
            const seed = `effuse-test:${ctx.runId}:${ctx.testId}:chat`
            yield* step(
              "e2e login",
              page.evaluate(`(() => {
  return fetch('/api/auth/e2e/login', {
    method: 'POST',
    cache: 'no-store',
    credentials: 'include',
    headers: {
      'content-type': 'application/json',
      'authorization': 'Bearer ' + ${JSON.stringify(e2eSecret)}
    },
    body: JSON.stringify({ seed: ${JSON.stringify(seed)} })
  }).then(async (res) => {
    const json = await res.json().catch(() => null);
    if (!res.ok || !json || json.ok !== true) {
      throw new Error('e2e login failed: HTTP ' + res.status + ' ' + JSON.stringify(json));
    }
    return json;
  });
})()`),
            )

            const prompt = `effuse-test:${Date.now()}:${Math.random().toString(16).slice(2)}`

            yield* step(
              "open chat pane on home",
              Effect.gen(function* () {
                yield* page.click("[data-oa-open-chat-pane] a, [data-oa-open-chat-pane]")
                yield* page.waitForFunction("!!document.querySelector('[data-pane-id=\"home-chat\"]')", { timeoutMs: 15_000 })
                yield* page.waitForFunction(
                  "!!document.querySelector('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"authed\"]')",
                  { timeoutMs: 30_000 },
                )
                yield* page.waitForFunction(
                  `(() => {
  const el = document.querySelector('[data-oa-home-chat-controls=\"1\"]');
  const t = (el?.textContent || '').toLowerCase();
  return t.includes('thread:') && !t.includes('(loading');
})()`,
                  { timeoutMs: 30_000 },
                )
              }),
            )

            const initialAssistantCount = yield* step(
              "count initial assistant messages",
              page.evaluate<number>("document.querySelectorAll('[data-chat-role=\"assistant\"]').length"),
            )

            yield* step("type into chat input", page.fill('[data-oa-home-chat-input=\"1\"]', prompt))
            yield* step("click Send", page.click('[data-oa-home-chat-send=\"1\"]'))

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
              "wait for assistant response OR visible error banner (and status settles)",
              page.waitForFunction(
                `(
                  (
                    document.querySelector('[data-oa-home-chat-root=\"1\"]')?.getAttribute('data-oa-home-chat-status') === 'ready'
                    || document.querySelector('[data-oa-home-chat-root=\"1\"]')?.getAttribute('data-oa-home-chat-status') === 'error'
                  )
                  && (
                    !!document.querySelector('[data-oa-home-chat-error=\"1\"]')
                    || document.querySelectorAll('[data-chat-role=\"assistant\"]').length > ${initialAssistantCount}
                  )
                )`,
                { timeoutMs: 120_000 },
              ),
            )

            yield* step(
              "assert no silent stall",
              Effect.gen(function* () {
                const errorBanner = yield* page.evaluate<boolean>(
                  "!!document.querySelector('[data-oa-home-chat-error=\"1\"]')",
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
    })

    tests.push({
      id: "apps-web.prod.autopilot.dse-canary-recap-shows-debug-card-and-trace",
      tags: ["e2e", "browser", "apps/web", "prod", "dse"],
      timeoutMs: 180_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            // Login via E2E bypass first (prod gating requires auth).
            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))
            const seed = `effuse-test:${ctx.runId}:${ctx.testId}:dse`
            yield* step(
              "e2e login",
              page.evaluate(`(() => {
  return fetch('/api/auth/e2e/login', {
    method: 'POST',
    cache: 'no-store',
    credentials: 'include',
    headers: {
      'content-type': 'application/json',
      'authorization': 'Bearer ' + ${JSON.stringify(e2eSecret)}
    },
    body: JSON.stringify({ seed: ${JSON.stringify(seed)} })
  }).then(async (res) => {
    const json = await res.json().catch(() => null);
    if (!res.ok || !json || json.ok !== true) {
      throw new Error('e2e login failed: HTTP ' + res.status + ' ' + JSON.stringify(json));
    }
    return json;
  });
})()`),
            )

            yield* step(
              "open chat pane on home (authed) and wait for DSE controls",
              Effect.gen(function* () {
                yield* page.click("[data-oa-open-chat-pane] a, [data-oa-open-chat-pane]")
                yield* page.waitForFunction("!!document.querySelector('[data-pane-id=\"home-chat\"]')", { timeoutMs: 15_000 })
                yield* page.waitForFunction(
                  "!!document.querySelector('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"authed\"]')",
                  { timeoutMs: 30_000 },
                )
                yield* page.waitForFunction("!!document.querySelector('[data-oa-home-dse-recap=\"1\"]')", {
                  timeoutMs: 30_000,
                })
              }),
            )

            yield* step(
              "enable deterministic DSE recap stub mode in UI",
              page.evaluate(`(() => { (window as any).__OA_E2E_MODE = 'stub'; })()`),
            )

            yield* step(
              "select strategy=rlm_lite.v1",
              page.evaluate(`(() => {
  const el = document.querySelector('select[data-oa-home-dse-strategy="1"]');
  if (!el) throw new Error('missing strategy select');
  (el as HTMLSelectElement).value = 'rlm_lite.v1';
  el.dispatchEvent(new Event('change', { bubbles: true }));
})()`),
            )

            yield* step(
              "select budget=small",
              page.evaluate(`(() => {
  const el = document.querySelector('select[data-oa-home-dse-budget="1"]');
  if (!el) throw new Error('missing budget select');
  (el as HTMLSelectElement).value = 'small';
  el.dispatchEvent(new Event('change', { bubbles: true }));
})()`),
            )

            yield* step("click Run recap (canary)", page.click('[data-oa-home-dse-recap=\"1\"]'))

            yield* step(
              "wait for DSE signature card to appear",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-dse-signature-details=\"1\"]')).some(el => (el.textContent || '').includes('@openagents/autopilot/canary/RecapThread.v1'))`,
                { timeoutMs: 60_000 },
              ),
            )

            yield* step(
              "expand recap signature card (collapsible UI)",
              page.evaluate(`(() => {
  const cards = Array.from(document.querySelectorAll('[data-dse-signature-details=\"1\"]'));
  const card = cards.find(el => (el.textContent || '').includes('@openagents/autopilot/canary/RecapThread.v1')) as HTMLDetailsElement | undefined;
  if (!card) throw new Error('missing recap signature card');
  if (!card.open) {
    const summary = card.querySelector('[data-dse-signature-summary=\"1\"]') as HTMLElement | null;
    if (!summary) throw new Error('missing recap signature summary');
    summary.click();
  }
  if (!card.open) throw new Error('failed to expand recap signature card');
})()`),
            )

            yield* step(
              "assert expanded DSE card shows strategy + iteration counters",
              Effect.gen(function* () {
                const ok = yield* page.evaluate<boolean>(`(() => {
  const cards = Array.from(document.querySelectorAll('[data-dse-signature-details=\"1\"]'));
  const card = cards.find(el => (el.textContent || '').includes('@openagents/autopilot/canary/RecapThread.v1')) as HTMLDetailsElement | undefined;
  if (!card) return false;
  if (!card.open) return false;
  const t = (card.textContent || '');
  return t.includes('strategyId') && t.includes('rlm_lite.v1') && (t.includes('rlmIterations=') || t.includes('subLmCalls='));
})()`)
                yield* assertTrue(
                  ok,
                  "Expected DSE card to include strategyId=rlm_lite.v1 and rlmIterations/subLmCalls counters",
                )
              }),
            )

            const traceHref = yield* step(
              "read trace href",
              page.evaluate<string | null>(`(() => {
  const cards = Array.from(document.querySelectorAll('[data-dse-signature-details=\"1\"]'));
  const card = cards.find(el => (el.textContent || '').includes('@openagents/autopilot/canary/RecapThread.v1')) as HTMLDetailsElement | undefined;
  if (!card || !card.open) return null;
  const el = card.querySelector('[data-dse-open-trace=\"1\"]') as HTMLAnchorElement | null;
  if (!el) return null;
  const rects = el.getClientRects();
  if (!rects || rects.length === 0) return null;
  return el.getAttribute('href') || null;
})()`),
            )

            yield* step(
              "assert trace link exists",
              assertTrue(typeof traceHref === "string" && traceHref.length > 0, "Expected trace link href"),
            )

            yield* step(
              "fetch trace and assert events",
              Effect.gen(function* () {
                const trace = yield* page.evaluate<any>(`(() => {
  const href = ${JSON.stringify(traceHref)};
  if (!href) return null;
  return fetch(href, { method: 'GET', cache: 'no-store', credentials: 'include' })
    .then(res => res.text())
    .then(text => {
      try { return JSON.parse(text); } catch { return { _parseError: true, text }; }
    })
    .catch(() => null);
})()`)
                yield* assertTrue(trace && trace.format === "openagents.dse.rlm_trace", "Expected trace.format === openagents.dse.rlm_trace")
                const events = Array.isArray(trace?.events) ? trace.events : []
                const hasIter = events.some((e: any) => e && typeof e === "object" && e._tag === "Iteration")
                const hasFinal = events.some((e: any) => e && typeof e === "object" && e._tag === "Final")
                yield* assertTrue(hasIter, "Expected at least one Iteration event in trace.events")
                yield* assertTrue(hasFinal, "Expected at least one Final event in trace.events")
              }),
            )
          }),
        )
      }),
    })

    tests.push({
      id: "apps-web.prod.autopilot.bootstrap-initial-conversation",
      tags: ["e2e", "browser", "apps/web", "prod"],
      timeoutMs: 240_000,
      steps: Effect.gen(function* () {
        const ctx = yield* TestContext
        const browser = yield* BrowserService

        yield* browser.withPage((page) =>
          Effect.gen(function* () {
            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))

            // New user per-run: different seed => different userId => new default thread.
            const seed = `effuse-test:${ctx.runId}:${ctx.testId}:${Date.now()}:${Math.random().toString(16).slice(2)}`
            yield* step(
              "e2e login (new user)",
              page.evaluate(`(() => {
  return fetch('/api/auth/e2e/login', {
    method: 'POST',
    cache: 'no-store',
    credentials: 'include',
    headers: {
      'content-type': 'application/json',
      'authorization': 'Bearer ' + ${JSON.stringify(e2eSecret)}
    },
    body: JSON.stringify({ seed: ${JSON.stringify(seed)} })
  }).then(async (res) => {
    const json = await res.json().catch(() => null);
    if (!res.ok || !json || json.ok !== true) {
      throw new Error('e2e login failed: HTTP ' + res.status + ' ' + JSON.stringify(json));
    }
    return json;
  });
})()`),
            )

            yield* step(
              "open chat pane on home (authed)",
              Effect.gen(function* () {
                yield* page.click("[data-oa-open-chat-pane] a, [data-oa-open-chat-pane]")
                yield* page.waitForFunction("!!document.querySelector('[data-pane-id=\"home-chat\"]')", { timeoutMs: 15_000 })
                yield* page.waitForFunction(
                  "!!document.querySelector('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"authed\"]')",
                  { timeoutMs: 30_000 },
                )
              }),
            )

            // Wait for the first welcome assistant message.
            yield* step(
              "wait for welcome message (or error)",
              page.waitForFunction(
                `(
                  Array.from(document.querySelectorAll('[data-chat-role=\"assistant\"]')).some(el => (el.textContent || '').includes('Autopilot online.'))
                  || !!document.querySelector('[data-oa-home-chat-error=\"1\"]')
                )`,
                { timeoutMs: 30_000 },
              ),
            )

            yield* step(
              "fail fast if error banner is visible",
              Effect.gen(function* () {
                const errorText = yield* page.evaluate<string>(`(() => {
  const el = document.querySelector('[data-oa-home-chat-error=\"1\"]');
  return el ? ((el.textContent || '').trim()) : '';
})()`)
                if (errorText) {
                  throw new Error(`Autopilot chat error banner: ${errorText}`)
                }
              }),
            )

            const assistantCount0 = yield* step(
              "count assistant messages (baseline)",
              page.evaluate<number>("document.querySelectorAll('[data-chat-role=\"assistant\"]').length"),
            )

            const transcript0 = yield* step(
              "read initial transcript",
              page.evaluate(`(() => {
  return Array.from(document.querySelectorAll('[data-chat-role]')).map((el) => ({
    role: el.getAttribute('data-chat-role'),
    text: (el.textContent || '').trim(),
  }));
})()`),
            )

            // Turn 1: user gives handle.
            const handle = "Bobo"
            yield* step("type handle", page.fill('[data-oa-home-chat-input=\"1\"]', handle))
            yield* step("click Send (handle)", page.click('[data-oa-home-chat-send=\"1\"]'))
            yield* step(
              "wait for user bubble handle",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"user\"]')).some(el => (el.textContent || '').trim() === ${JSON.stringify(
                  handle,
                )})`,
                { timeoutMs: 30_000 },
              ),
            )
            yield* step(
              "wait for new assistant message (after handle)",
              page.waitForFunction(
                `document.querySelectorAll('[data-chat-role=\"assistant\"]').length > ${assistantCount0}`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for assistant to acknowledge handle",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"assistant\"]')).some(el => {
  const t = (el.textContent || '').toLowerCase();
  return t.includes(${JSON.stringify(handle.toLowerCase())}) && !t.includes('is that ok') && !t.includes('is that okay');
})`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for Send button (not busy)",
              page.waitForFunction("!!document.querySelector('[data-oa-home-chat-send=\"1\"]')", {
                timeoutMs: 120_000,
              }),
            )

            const assistantCount1 = yield* step(
              "count assistant messages (after handle)",
              page.evaluate<number>("document.querySelectorAll('[data-chat-role=\"assistant\"]').length"),
            )

            // Turn 2: user gives agent name.
            const agentName = "Autopilot"
            yield* step("type agent name", page.fill('[data-oa-home-chat-input=\"1\"]', agentName))
            yield* step("click Send (agent name)", page.click('[data-oa-home-chat-send=\"1\"]'))
            yield* step(
              "wait for user bubble agent name",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"user\"]')).some(el => (el.textContent || '').trim() === ${JSON.stringify(
                  agentName,
                )})`,
                { timeoutMs: 30_000 },
              ),
            )
            yield* step(
              "wait for new assistant message (after agent name)",
              page.waitForFunction(
                `document.querySelectorAll('[data-chat-role=\"assistant\"]').length > ${assistantCount1}`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for assistant to ask vibe",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"assistant\"]')).some(el => {
  const t = (el.textContent || '').toLowerCase();
  return t.includes('vibe');
})`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for Send button (not busy)",
              page.waitForFunction("!!document.querySelector('[data-oa-home-chat-send=\"1\"]')", {
                timeoutMs: 120_000,
              }),
            )

            const assistantCount2 = yield* step(
              "count assistant messages (after agent name)",
              page.evaluate<number>("document.querySelectorAll('[data-chat-role=\"assistant\"]').length"),
            )

            // Turn 3: user gives vibe.
            const vibe = "calm, direct, pragmatic"
            yield* step("type vibe", page.fill('[data-oa-home-chat-input=\"1\"]', vibe))
            yield* step("click Send (vibe)", page.click('[data-oa-home-chat-send=\"1\"]'))
            yield* step(
              "wait for user bubble vibe",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"user\"]')).some(el => (el.textContent || '').trim() === ${JSON.stringify(
                  vibe,
                )})`,
                { timeoutMs: 30_000 },
              ),
            )
            yield* step(
              "wait for new assistant message (after vibe)",
              page.waitForFunction(
                `document.querySelectorAll('[data-chat-role=\"assistant\"]').length > ${assistantCount2}`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for assistant to ask boundaries/preferences",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"assistant\"]')).some(el => {
  const t = (el.textContent || '').toLowerCase();
  return t.includes('boundaries') || t.includes('preferences');
})`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for Send button (not busy)",
              page.waitForFunction("!!document.querySelector('[data-oa-home-chat-send=\"1\"]')", {
                timeoutMs: 120_000,
              }),
            )

            const assistantCount3 = yield* step(
              "count assistant messages (after vibe)",
              page.evaluate<number>("document.querySelectorAll('[data-chat-role=\"assistant\"]').length"),
            )

            // Turn 4: user gives boundaries (none).
            const boundaries = "none"
            yield* step("type boundaries", page.fill('[data-oa-home-chat-input=\"1\"]', boundaries))
            yield* step("click Send (boundaries)", page.click('[data-oa-home-chat-send=\"1\"]'))
            yield* step(
              "wait for user bubble boundaries",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"user\"]')).some(el => (el.textContent || '').trim() === ${JSON.stringify(
                  boundaries,
                )})`,
                { timeoutMs: 30_000 },
              ),
            )
            yield* step(
              "wait for new assistant message (after boundaries)",
              page.waitForFunction(
                `document.querySelectorAll('[data-chat-role=\"assistant\"]').length > ${assistantCount3}`,
                { timeoutMs: 120_000 },
              ),
            )
            yield* step(
              "wait for assistant to ask what to do first",
              page.waitForFunction(
                `Array.from(document.querySelectorAll('[data-chat-role=\"assistant\"]')).some(el => {
  const t = (el.textContent || '').toLowerCase();
  return t.includes('what would you like to do first');
})`,
                { timeoutMs: 120_000 },
              ),
            )

            const transcript2 = yield* step(
              "read transcript after bootstrap turns",
              page.evaluate(`(() => {
  return Array.from(document.querySelectorAll('[data-chat-role]')).map((el) => ({
    role: el.getAttribute('data-chat-role'),
    text: (el.textContent || '').trim(),
  }));
})()`),
            )

            const outPath = Path.join(ctx.artifactsDir, "bootstrap-transcript.json")
            yield* step(
              "write transcript artifact",
              Effect.tryPromise({
                try: () => Fs.writeFile(outPath, JSON.stringify({ initial: transcript0, after: transcript2 }, null, 2), "utf8"),
                catch: (error) => (error instanceof Error ? error : new Error(String(error))),
              }),
            )
          }),
        )
      }),
    })
  }

  const magicEmail = config.magicEmail
  const magicCode = config.magicCode
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
            yield* step("goto /", page.goto(`${ctx.baseUrl}/`))

            yield* step(
              "open chat pane (email step)",
              Effect.gen(function* () {
                yield* page.click("[data-oa-open-chat-pane] a, [data-oa-open-chat-pane]")
                yield* page.waitForFunction("!!document.querySelector('[data-pane-id=\"home-chat\"]')", { timeoutMs: 15_000 })
                yield* page.waitForFunction(
                  "!!document.querySelector('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"email\"]')",
                  { timeoutMs: 15_000 },
                )
              }),
            )

            yield* step("fill email", page.fill('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"email\"] input[name=\"email\"]', magicEmail))
            yield* step("submit email", page.click('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"email\"] [data-oa-home-chat-send=\"1\"]'))

            yield* step(
              "wait for code step",
              page.waitForFunction("!!document.querySelector('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"code\"]')", {
                timeoutMs: 30_000,
              }),
            )

            yield* step("fill code", page.fill('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"code\"] input[name=\"code\"]', magicCode))
            yield* step("submit code", page.click('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"code\"] [data-oa-home-chat-send=\"1\"]'))

            yield* step(
              "wait for authed step",
              page.waitForFunction("!!document.querySelector('[data-oa-home-chat-form=\"1\"][data-oa-home-chat-step=\"authed\"]')", {
                timeoutMs: 30_000,
              }),
            )

            yield* step(
              "assert identity pill rendered",
              Effect.gen(function* () {
                const authed = yield* page.evaluate<boolean>(`(() => {
  const el = document.querySelector('[data-oa-home-identity-card]');
  const t = (el?.textContent || '').toLowerCase();
  return !!el && t.includes(${JSON.stringify(magicEmail.toLowerCase())});
})()`)
                yield* assertTrue(authed, "Expected home chat identity pill to include the authed email")
              }),
            )
          }),
        )
      }),
    })
  }

    return tests
  })
