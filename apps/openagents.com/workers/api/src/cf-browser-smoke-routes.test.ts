import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CF_BROWSER_SMOKE_URL,
  type CfSmokeBrowser,
  type CfSmokeLaunch,
  type CfSmokePage,
  makeCfBrowserSmokeHandler,
} from './cf-browser-smoke-routes'

const executionContext = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as unknown as ExecutionContext

type Session = Readonly<{ user: Readonly<{ email: string }> }>

const adminSession: Session = { user: { email: 'chris@openagents.com' } }

const adminDeps = {
  isOpenAgentsAdminEmail: (email: string) => email === 'chris@openagents.com',
  requireBrowserSession: async (): Promise<Session | undefined> => adminSession,
  appendRefreshedSessionCookies: (response: Response) => response,
}

/** A scripted page + browser that never touches the network. */
const makeFakeLaunch = (
  options: Readonly<{
    title?: string
    width?: number
    height?: number
    bytes?: number
    onClose?: () => void
    onGoto?: (url: string) => void
  }> = {},
): { launch: CfSmokeLaunch; closed: () => boolean } => {
  let closed = false
  const page: CfSmokePage = {
    goto: async url => {
      options.onGoto?.(url)
      return undefined
    },
    title: async () => options.title ?? 'Example Domain',
    viewportSize: () =>
      options.width !== undefined && options.height !== undefined
        ? { width: options.width, height: options.height }
        : { width: 1280, height: 720 },
    screenshot: async () => new Uint8Array(options.bytes ?? 256),
  }
  const browser: CfSmokeBrowser = {
    newPage: async () => page,
    close: async () => {
      closed = true
      options.onClose?.()
    },
  }
  const launch: CfSmokeLaunch = async () => browser
  return { launch, closed: () => closed }
}

describe('admin-gated CF Browser Rendering smoke (#6205)', () => {
  test('with a FAKE env.BROWSER + injected launch returns { ok: true, ... }', async () => {
    let navigatedTo = ''
    const fake = makeFakeLaunch({
      title: 'Example Domain',
      width: 1280,
      height: 720,
      bytes: 512,
      onGoto: url => {
        navigatedTo = url
      },
    })

    const handler = makeCfBrowserSmokeHandler<Session, { BROWSER: unknown }>({
      ...adminDeps,
      launch: fake.launch,
    })

    const response = await Effect.runPromise(
      handler(
        new Request('https://openagents.com/api/admin/cf-browser-smoke'),
        { BROWSER: { fake: true } },
        executionContext,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.ok).toBe(true)
    expect(body.title).toBe('Example Domain')
    expect(body.width).toBe(1280)
    expect(body.height).toBe(720)
    expect(body.bytes).toBe(512)
    expect(navigatedTo).toBe(CF_BROWSER_SMOKE_URL)
    // The managed browser must always be closed (no leaked concurrent slot).
    expect(fake.closed()).toBe(true)
  })

  test('with NO env.BROWSER binding returns { ok: false, reason }', async () => {
    const handler = makeCfBrowserSmokeHandler<Session, Record<string, unknown>>({
      ...adminDeps,
      // launch must never be called when the binding is absent.
      launch: async () => {
        throw new Error('launch should not be called without a binding')
      },
    })

    const response = await Effect.runPromise(
      handler(
        new Request('https://openagents.com/api/admin/cf-browser-smoke'),
        {},
        executionContext,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.ok).toBe(false)
    expect(typeof body.reason).toBe('string')
    expect(body.reason as string).toContain('env.BROWSER')
  })

  test('when Browser Rendering errors returns honest { ok: false, reason } and still closes', async () => {
    let closed = false
    const launch: CfSmokeLaunch = async () => ({
      newPage: async () => {
        throw new Error('browser-rendering-not-enabled')
      },
      close: async () => {
        closed = true
      },
    })

    const handler = makeCfBrowserSmokeHandler<Session, { BROWSER: unknown }>({
      ...adminDeps,
      launch,
    })

    const response = await Effect.runPromise(
      handler(
        new Request('https://openagents.com/api/admin/cf-browser-smoke'),
        { BROWSER: { fake: true } },
        executionContext,
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.ok).toBe(false)
    expect(body.reason).toBe('browser-rendering-not-enabled')
    expect(closed).toBe(true)
  })

  test('rejects a non-admin session with 403', async () => {
    const fake = makeFakeLaunch()
    const handler = makeCfBrowserSmokeHandler<Session, { BROWSER: unknown }>({
      ...adminDeps,
      requireBrowserSession: async () => ({
        user: { email: 'not-admin@example.com' },
      }),
      launch: fake.launch,
    })

    const response = await Effect.runPromise(
      handler(
        new Request('https://openagents.com/api/admin/cf-browser-smoke'),
        { BROWSER: { fake: true } },
        executionContext,
      ),
    )

    expect(response.status).toBe(403)
  })

  test('rejects an absent session with 401', async () => {
    const handler = makeCfBrowserSmokeHandler<Session, { BROWSER: unknown }>({
      ...adminDeps,
      requireBrowserSession: async () => undefined,
    })

    const response = await Effect.runPromise(
      handler(
        new Request('https://openagents.com/api/admin/cf-browser-smoke'),
        { BROWSER: { fake: true } },
        executionContext,
      ),
    )

    expect(response.status).toBe(401)
  })

  test('rejects a non-GET method with 405', async () => {
    const handler = makeCfBrowserSmokeHandler<Session, { BROWSER: unknown }>(
      adminDeps,
    )

    const response = await Effect.runPromise(
      handler(
        new Request('https://openagents.com/api/admin/cf-browser-smoke', {
          method: 'POST',
        }),
        { BROWSER: { fake: true } },
        executionContext,
      ),
    )

    expect(response.status).toBe(405)
  })
})
