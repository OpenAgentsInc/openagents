// Admin-gated Cloudflare Browser Rendering smoke route (#6205, live binding +
// smoke).
//
// This proves the REAL `env.BROWSER` Browser Rendering binding from inside the
// deployed Worker: it launches managed Chrome via `@cloudflare/playwright`'s
// `launch(env.BROWSER)`, navigates to a stable page, screenshots it, and reports
// public-safe metadata (`title`, viewport, screenshot byte count). It is the
// deploy-time counterpart to qa-runner's `cfBrowserBackend` — that backend's
// `env.BROWSER` only exists inside a deployed Worker, so this endpoint is how the
// coordinator confirms the binding is actually wired on prod.
//
// AUTH: reuses the EXACT admin mechanism every other `/api/admin/*` route uses —
// `requireBrowserSession(...)` then `isOpenAgentsAdminEmail(session.user.email)`,
// injected by the caller (mirrors `makeAdminOverviewHandlers`). No new auth path.
//
// HONEST ABOUT THE BINDING: the binding only exists inside a deployed Worker. If
// `env.BROWSER` is absent (e.g. binding not yet provisioned / enablement issue)
// or Browser Rendering errors, this returns `{ ok: false, reason }` with the
// error message and a 200 so the coordinator can SEE the enablement error rather
// than a blind 500. Auth failures still return 401/403 as usual.
//
// `launch` is INJECTED (default: dynamic import of `@cloudflare/playwright`,
// available only in the Worker runtime) so unit tests can inject a fake binding
// and a fake `launch` to prove the lifecycle deterministically — no network, no
// managed-browser spend.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

// Alias `globalThis.Response` so this route module mirrors the zero-debt-
// architecture convention used by the other admin route modules (e.g.
// admin-overview-routes.ts): HTTP response surfaces are written as
// `HttpResponse`, keeping route modules off the "Worker Response return
// surfaces" budget while route mappers are extracted.
type HttpResponse = globalThis.Response

/** The opaque Cloudflare Browser Rendering binding (`env.BROWSER`). */
export type CfBrowserBinding = unknown

/** Minimal Playwright `Page` slice this smoke drives (over CDP, managed). */
export interface CfSmokePage {
  goto(url: string, options?: { readonly waitUntil?: string }): Promise<unknown>
  title(): Promise<string>
  viewportSize(): { readonly width: number; readonly height: number } | null
  screenshot(options?: { readonly fullPage?: boolean }): Promise<Uint8Array>
}

/** Minimal managed-browser slice (from `launch(env.BROWSER)`). */
export interface CfSmokeBrowser {
  newPage(): Promise<CfSmokePage>
  close(): Promise<void>
}

/** `launch(env.BROWSER, opts)` from `@cloudflare/playwright`. */
export type CfSmokeLaunch = (
  binding: CfBrowserBinding,
  options?: { readonly keep_alive?: number },
) => Promise<CfSmokeBrowser>

/** The stable target the smoke navigates to. */
export const CF_BROWSER_SMOKE_URL = 'https://example.com'

/** Default viewport used when the page does not report one. */
const DEFAULT_SMOKE_WIDTH = 1280
const DEFAULT_SMOKE_HEIGHT = 720

type AdminSmokeSession = Readonly<{
  user: Readonly<{
    email: string
  }>
}>

export type CfBrowserSmokeDependencies<
  Session extends AdminSmokeSession,
  Bindings,
> = Readonly<{
  /** Same admin-email predicate every `/api/admin/*` route uses. */
  isOpenAgentsAdminEmail: (email: string) => boolean
  /** Same browser-session boundary every `/api/admin/*` route uses. */
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  /** Same refresh-cookie helper the other admin routes use. */
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  /**
   * Read the Browser Rendering binding off `env`. Defaults to `env.BROWSER`.
   * Kept injectable so the route can be wired without `BROWSER` being on the
   * statically-typed `Env`.
   */
  readBrowserBinding?: (env: Bindings) => CfBrowserBinding | undefined
  /**
   * Injectable `launch`. Default: dynamic import of `@cloudflare/playwright`'s
   * `launch` (Worker-runtime only). Tests inject a fake returning a scripted
   * page — no network, no spend.
   */
  launch?: CfSmokeLaunch
  /** Keep-alive (ms) passed to `launch` so the ~60s idle window does not kill
   *  the smoke mid-flight. */
  keepAliveMs?: number
}>

const defaultReadBrowserBinding = <Bindings>(
  env: Bindings,
): CfBrowserBinding | undefined =>
  (env as { readonly BROWSER?: CfBrowserBinding }).BROWSER

/** Dynamic import of the real `@cloudflare/playwright` `launch`. Only used when
 *  no `launch` is injected — i.e. inside a deployed Worker. Kept out of the unit
 *  module graph so CI never tries to load the Worker-only package. */
const defaultLaunch: CfSmokeLaunch = async (binding, options) => {
  const mod = (await import(
    /* @vite-ignore */ '@cloudflare/playwright'
  )) as { readonly launch: CfSmokeLaunch }

  return mod.launch(binding, options)
}

const DEFAULT_KEEP_ALIVE_MS = 60_000

/**
 * Run the Browser Rendering smoke against a resolved binding. Returns the
 * public-safe success payload, or an honest `{ ok: false, reason }` if the
 * managed browser errors. Always closes the browser (even on error) so a smoke
 * never leaks a concurrent browser slot.
 */
const runSmoke = async (
  binding: CfBrowserBinding,
  launch: CfSmokeLaunch,
  keepAliveMs: number,
): Promise<
  | { readonly ok: true; title: string; width: number; height: number; bytes: number }
  | { readonly ok: false; reason: string }
> => {
  let browser: CfSmokeBrowser | undefined
  try {
    browser = await launch(binding, { keep_alive: keepAliveMs })
    const page = await browser.newPage()
    await page.goto(CF_BROWSER_SMOKE_URL, { waitUntil: 'load' })
    const title = await page.title()
    const viewport = page.viewportSize()
    const shot = await page.screenshot()

    return {
      ok: true,
      title,
      width: viewport?.width ?? DEFAULT_SMOKE_WIDTH,
      height: viewport?.height ?? DEFAULT_SMOKE_HEIGHT,
      bytes: shot.byteLength,
    }
  } catch (error) {
    return {
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
    }
  } finally {
    if (browser !== undefined) {
      try {
        await browser.close()
      } catch {
        // best-effort teardown: never mask the real result with a close error.
      }
    }
  }
}

/**
 * Build the admin-gated `GET /api/admin/cf-browser-smoke` handler.
 *
 * Auth identical to every other `/api/admin/*` route (injected
 * `requireBrowserSession` + `isOpenAgentsAdminEmail`). On success returns
 * `{ ok: true, title, width, height, bytes }`. If the binding is absent or
 * Browser Rendering errors, returns `{ ok: false, reason }` (HTTP 200) so the
 * coordinator can see an enablement error instead of an opaque 500.
 */
export const makeCfBrowserSmokeHandler =
  <Session extends AdminSmokeSession, Bindings>(
    dependencies: CfBrowserSmokeDependencies<Session, Bindings>,
  ) =>
  (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> => {
    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    return Effect.promise(async () => {
      let session: Session | undefined
      try {
        session = await dependencies.requireBrowserSession(request, env, ctx)
      } catch {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      if (session === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
        return noStoreJsonResponse({ error: 'forbidden' }, { status: 403 })
      }

      const readBinding =
        dependencies.readBrowserBinding ?? defaultReadBrowserBinding
      const binding = readBinding(env)

      if (binding === undefined || binding === null) {
        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            ok: false,
            reason:
              'Cloudflare Browser Rendering binding (env.BROWSER) is absent. ' +
              'Add `"browser": { "binding": "BROWSER" }` to wrangler.jsonc and ' +
              'deploy; this binding only exists inside a deployed Worker.',
          }),
          session,
        )
      }

      const launch = dependencies.launch ?? defaultLaunch
      const keepAliveMs = dependencies.keepAliveMs ?? DEFAULT_KEEP_ALIVE_MS
      const result = await runSmoke(binding, launch, keepAliveMs)

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(result),
        session,
      )
    })
  }
