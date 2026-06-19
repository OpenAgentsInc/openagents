import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type SandboxAuth,
  type SandboxComputeServiceDeps,
  type SandboxMeteringHook,
  type SandboxRuntimeAdapter,
  MAX_SANDBOX_TTL_SECONDS,
  SandboxAdapterError,
  DEFAULT_SANDBOX_IMAGE,
  handleSandboxRequest,
  isSandboxComputeServiceEnabled,
  sandboxRentalReceiptRef,
  stubSandboxAdapter,
} from './sandbox-compute-service-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const authOk: SandboxAuth = async () => ({ accountRef: 'agent:test-user' })
const authNone: SandboxAuth = async () => undefined

const baseDeps = (
  overrides: Partial<SandboxComputeServiceDeps> = {},
): SandboxComputeServiceDeps => ({
  authenticate: authOk,
  enabled: true,
  newId: () => 'sbx_fixed',
  ...overrides,
})

const sandboxRequest = (body: unknown, init: RequestInit = {}): Request =>
  new Request('https://openagents.com/v1/sandboxes', {
    body: JSON.stringify(body),
    method: 'POST',
    ...init,
  })

describe('sandbox compute service feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isSandboxComputeServiceEnabled(undefined)).toBe(false)
    expect(isSandboxComputeServiceEnabled('')).toBe(false)
    expect(isSandboxComputeServiceEnabled('false')).toBe(false)
    expect(isSandboxComputeServiceEnabled('0')).toBe(false)
    expect(isSandboxComputeServiceEnabled('true')).toBe(true)
    expect(isSandboxComputeServiceEnabled('1')).toBe(true)
    expect(isSandboxComputeServiceEnabled('on')).toBe(true)
    expect(isSandboxComputeServiceEnabled('yes')).toBe(true)
  })
})

describe('POST /v1/sandboxes', () => {
  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ enabled: false })),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('sandbox_compute_service_disabled')
  })

  test('rejects an unauthenticated request with 401 + WWW-Authenticate', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ authenticate: authNone })),
    )
    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBe('Bearer')
  })

  test('rejects non-POST with 405', async () => {
    const response = await run(
      handleSandboxRequest(
        new Request('https://openagents.com/v1/sandboxes', { method: 'GET' }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(405)
  })

  test('rejects invalid JSON with 400', async () => {
    const response = await run(
      handleSandboxRequest(
        new Request('https://openagents.com/v1/sandboxes', {
          body: '{bad',
          method: 'POST',
        }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe('invalid_json')
  })

  test('accepts an empty body and applies image/ttl defaults', async () => {
    const response = await run(
      handleSandboxRequest(
        new Request('https://openagents.com/v1/sandboxes', { body: '', method: 'POST' }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('sandbox')
    expect(body.image).toBe(DEFAULT_SANDBOX_IMAGE)
    expect(body.status).toBe('provisioning')
    // Scaffold never returns a usable connection.
    expect(body.connection_ref).toBeNull()
  })

  test('rejects an over-ceiling TTL with 400 (abuse control) before provisioning', async () => {
    const response = await run(
      handleSandboxRequest(
        sandboxRequest({ ttlSeconds: MAX_SANDBOX_TTL_SECONDS + 1 }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string; maxTtlSeconds: number }
    expect(body.error).toBe('invalid_ttl')
    expect(body.maxTtlSeconds).toBe(MAX_SANDBOX_TTL_SECONDS)
  })

  test('rejects a non-positive TTL with 400', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({ ttlSeconds: 0 }), baseDeps()),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe('invalid_ttl')
  })

  test('stub metering reports metered:false / null receipt (honest, not live)', async () => {
    const response = await run(
      handleSandboxRequest(sandboxRequest({ image: 'custom' }), baseDeps()),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(false)
    expect(body.receipt_ref).toBeNull()
  })

  test('maps a runtime adapter failure to 502', async () => {
    const failing: SandboxRuntimeAdapter = {
      id: 'failing',
      provision: () =>
        Effect.fail(new SandboxAdapterError({ adapterId: 'failing', reason: 'no_capacity' })),
    }
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ adapter: failing })),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('runtime_error')
    expect(body.reason).toBe('no_capacity')
  })

  test('a live metering hook can project a receipt ref', async () => {
    const liveHook: SandboxMeteringHook = context =>
      Effect.succeed({
        metered: true,
        receiptRef: sandboxRentalReceiptRef(context.sandboxId),
      })
    const response = await run(
      handleSandboxRequest(sandboxRequest({}), baseDeps({ meteringHook: liveHook })),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(true)
    expect(body.receipt_ref).toBe('receipt.cloud.sandbox_compute.rental.sbx_fixed')
  })

  test('the stub adapter never returns a usable connection', async () => {
    const sandbox = await run(
      Effect.orDie(
        stubSandboxAdapter.provision({
          sandboxId: 's1',
          accountRef: 'agent:x',
          request: { image: 'i', ttlSeconds: 60, options: {} },
        }),
      ),
    )
    expect(sandbox.status).toBe('provisioning')
    expect(sandbox.connectionRef).toBeNull()
  })
})
