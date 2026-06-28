import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type CloudCodingAuth,
  type CloudCodingMeteringHook,
  type CloudCodingRuntimeAdapter,
  type CloudCodingSession,
  type CloudCodingSessionServiceDeps,
  CloudCodingAdapterError,
  MAX_CLOUD_CODING_TIMEOUT_SECONDS,
  admissibleLanesForTrustTier,
  cloudCodingSessionReceiptRef,
  decidePlacement,
  handleCloudCodingSessionGet,
  handleCloudCodingSessionLaunch,
  isCloudGceProvisioningArmed,
  isCloudCodingSessionsEnabled,
  makeCloudControlCloudCodingAdapter,
  makeLedgerCloudCodingMeteringHook,
  routeCloudCodingSessionRequest,
  stubCloudCodingAdapter,
} from './cloud-coding-session-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const authOk: CloudCodingAuth = async () => ({ accountRef: 'agent:test-user' })
const authOther: CloudCodingAuth = async () => ({ accountRef: 'agent:other' })
const authNone: CloudCodingAuth = async () => undefined

const baseDeps = (
  overrides: Partial<CloudCodingSessionServiceDeps> = {},
): CloudCodingSessionServiceDeps => ({
  authenticate: authOk,
  adapter: stubCloudCodingAdapter,
  enabled: true,
  newId: () => 'ccs_fixed',
  ...overrides,
})

const launchRequest = (body: unknown, init: RequestInit = {}): Request =>
  new Request('https://openagents.com/v1/cloud-coding-sessions', {
    body: JSON.stringify(body),
    method: 'POST',
    ...init,
  })

const getRequest = (sessionId: string): Request =>
  new Request(`https://openagents.com/v1/cloud-coding-sessions/${sessionId}`, {
    method: 'GET',
  })

const validBody = {
  objective: 'fix the failing test',
  repoRef: 'repo:openagents/openagents',
}

describe('cloud coding sessions feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isCloudCodingSessionsEnabled(undefined)).toBe(false)
    expect(isCloudCodingSessionsEnabled('')).toBe(false)
    expect(isCloudCodingSessionsEnabled('false')).toBe(false)
    expect(isCloudCodingSessionsEnabled('0')).toBe(false)
    expect(isCloudCodingSessionsEnabled('off')).toBe(false)
    expect(isCloudCodingSessionsEnabled('true')).toBe(true)
    expect(isCloudCodingSessionsEnabled('1')).toBe(true)
    expect(isCloudCodingSessionsEnabled('on')).toBe(true)
    expect(isCloudCodingSessionsEnabled('YES')).toBe(true)
  })

  test('arms live GCE provisioning only on the explicit live token', () => {
    expect(isCloudGceProvisioningArmed(undefined)).toBe(false)
    expect(isCloudGceProvisioningArmed('fake')).toBe(false)
    expect(isCloudGceProvisioningArmed('true')).toBe(false)
    expect(isCloudGceProvisioningArmed(' live ')).toBe(true)
  })
})

describe('placement policy (authority boundary)', () => {
  test('regulated repos are SHC-only', () => {
    expect(admissibleLanesForTrustTier('regulated')).toEqual(['cloud-shc'])
  })

  test('private and public repos accept either cloud lane', () => {
    expect(admissibleLanesForTrustTier('private')).toEqual([
      'cloud-gcp',
      'cloud-shc',
    ])
    expect(admissibleLanesForTrustTier('public')).toEqual([
      'cloud-gcp',
      'cloud-shc',
    ])
  })

  test('a regulated repo requesting cloud-gcp is refused', () => {
    const decision = decidePlacement({ lane: 'cloud-gcp', tier: 'regulated' })
    expect(decision.allowed).toBe(false)
    if (!decision.allowed) {
      expect(decision.admissibleLanes).toEqual(['cloud-shc'])
      expect(decision.requestedLane).toBe('cloud-gcp')
    }
  })

  test('a regulated repo on cloud-shc is allowed', () => {
    const decision = decidePlacement({ lane: 'cloud-shc', tier: 'regulated' })
    expect(decision.allowed).toBe(true)
    if (decision.allowed) {
      expect(decision.lane).toBe('cloud-shc')
    }
  })
})

describe('POST /v1/cloud-coding-sessions', () => {
  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ enabled: false }),
      ),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('cloud_coding_sessions_disabled')
  })

  test('fails closed with a typed not-armed error when no live GCE adapter is armed', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(launchRequest(validBody), {
        authenticate: authOk,
        enabled: true,
        newId: () => 'ccs_fixed',
      }),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('runtime_error')
    expect(body.reason).toBe('cloud_gce_provisioning_not_armed')
  })

  test('rejects a non-POST method with 405', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(
        new Request('https://openagents.com/v1/cloud-coding-sessions', {
          method: 'GET',
        }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(405)
  })

  test('rejects an unauthenticated request with 401 + WWW-Authenticate', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ authenticate: authNone }),
      ),
    )
    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBe('Bearer')
  })

  test('rejects invalid JSON with 400', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(
        new Request('https://openagents.com/v1/cloud-coding-sessions', {
          body: 'not json',
          method: 'POST',
        }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe(
      'invalid_json',
    )
  })

  test('rejects a request missing repoRef/objective with 400', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(launchRequest({ repoRef: '' }), baseDeps()),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe(
      'invalid_request',
    )
  })

  test('rejects a non-positive or over-ceiling timeout before placement', async () => {
    const zero = await run(
      handleCloudCodingSessionLaunch(
        launchRequest({ ...validBody, timeoutSeconds: 0 }),
        baseDeps(),
      ),
    )
    expect(zero.status).toBe(400)
    expect(((await zero.json()) as { error: string }).error).toBe(
      'invalid_timeout',
    )

    const over = await run(
      handleCloudCodingSessionLaunch(
        launchRequest({
          ...validBody,
          timeoutSeconds: MAX_CLOUD_CODING_TIMEOUT_SECONDS + 1,
        }),
        baseDeps(),
      ),
    )
    expect(over.status).toBe(400)
  })

  test('refuses a regulated repo on cloud-gcp with 403 before any dispatch', async () => {
    let launched = false
    const adapter: CloudCodingRuntimeAdapter = {
      ...stubCloudCodingAdapter,
      launch: input => {
        launched = true
        return stubCloudCodingAdapter.launch(input)
      },
    }
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest({
          ...validBody,
          lane: 'cloud-gcp',
          repoTrustTier: 'regulated',
        }),
        baseDeps({ adapter }),
      ),
    )
    expect(response.status).toBe(403)
    const body = (await response.json()) as {
      error: string
      admissibleLanes: ReadonlyArray<string>
    }
    expect(body.error).toBe('lane_not_admissible_for_trust_tier')
    expect(body.admissibleLanes).toEqual(['cloud-shc'])
    expect(launched).toBe(false)
  })

  test('launches a queued cloud session through the stub adapter with honest null refs', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(launchRequest(validBody), baseDeps()),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('cloud.coding_session')
    expect(body.id).toBe('ccs_fixed')
    expect(body.lane).toBe('cloud-gcp')
    expect(body.adapter).toBe('codex')
    expect(body.repo_trust_tier).toBe('private')
    expect(body.state).toBe('queued')
    // Honest: the stub provisions no VM and runs no edit.
    expect(body.placement_ref).toBeNull()
    expect(body.lease_refs).toEqual([])
    expect(body.artifact_ref).toBeNull()
    // Honest: the stub meters nothing.
    expect(body.metered).toBe(false)
    expect(body.receipt_ref).toBeNull()
  })

  test('honors an explicit cloud-shc lane on a regulated repo', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest({
          ...validBody,
          lane: 'cloud-shc',
          repoTrustTier: 'regulated',
        }),
        baseDeps(),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.lane).toBe('cloud-shc')
    expect(body.repo_trust_tier).toBe('regulated')
  })

  test('maps a runtime adapter failure to 502', async () => {
    const adapter: CloudCodingRuntimeAdapter = {
      ...stubCloudCodingAdapter,
      launch: () =>
        Effect.fail(
          new CloudCodingAdapterError({
            adapterId: 'x',
            reason: 'lease_unavailable',
          }),
        ),
    }
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ adapter }),
      ),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('runtime_error')
    expect(body.reason).toBe('lease_unavailable')
  })

  test('with live provisioning armed, posts to cloud placement and returns lease refs', async () => {
    let placementBody: Record<string, unknown> | undefined
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async (url, init) => {
        expect(url).toBe('https://cloud.openagents.test/v1/placement')
        expect(init?.method).toBe('POST')
        expect((init?.headers as Record<string, string>).Authorization).toBe(
          'Bearer secret-test-token',
        )
        placementBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        return Response.json({
          binding: {
            capacityClassId: 'gce-standard',
            caps: { gceLeaseRef: 'lease.gce.vm.ccs_fixed' },
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
          },
          externalRunId: 'run_gce_1',
          status: 'running',
        })
      },
      gceProvisioningArmed: true,
    })
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest({
          ...validBody,
          authGrantRef: 'grant.public.test',
          providerAccountRef: 'provider-account.public.test',
        }),
        baseDeps({ adapter }),
      ),
    )
    expect(response.status).toBe(200)
    expect(placementBody).toMatchObject({
      auth_grant_ref: 'grant.public.test',
      contract_version: 'openagents.codex_placement_assignment.v1',
      lane: 'cloud-gcp',
      owner_ref: 'agent:test-user',
      provider_account_ref: 'provider-account.public.test',
      repository: validBody.repoRef,
      run_id: 'ccs_fixed',
      wallet_authority: false,
    })
    const body = (await response.json()) as Record<string, unknown>
    expect(body.state).toBe('running')
    expect(body.placement_ref).toBe('placement.cloud-coding.run_gce_1')
    expect(body.lease_refs).toEqual([
      'placement.cloud-coding.run_gce_1',
      'cloud-run.run_gce_1',
      'cloud-runner.runner_gce_1',
      'cloud-capacity-class.gce-standard',
      'lease.gce.vm.ccs_fixed',
    ])
  })

  test('surfaces a live metering receipt ref when the hook reports metered', async () => {
    const meteringHook: CloudCodingMeteringHook = () =>
      Effect.succeed({
        metered: true,
        receiptRef: cloudCodingSessionReceiptRef('ccs_fixed'),
      })
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ meteringHook }),
      ),
    )
    const body = (await response.json()) as Record<string, unknown>
    expect(body.metered).toBe(true)
    expect(body.receipt_ref).toBe(
      cloudCodingSessionReceiptRef('ccs_fixed'),
    )
  })
})

describe('GET /v1/cloud-coding-sessions/:id', () => {
  const readyAdapter = (
    session: CloudCodingSession,
  ): CloudCodingRuntimeAdapter => ({
    ...stubCloudCodingAdapter,
    get: ({ accountRef, sessionId }) =>
      Effect.succeed(
        sessionId === session.sessionId && accountRef === session.accountRef
          ? session
          : undefined,
      ),
  })

  const sample: CloudCodingSession = {
    accountRef: 'agent:test-user',
    adapter: 'codex',
    artifactRef: null,
    createdAt: '2026-06-19T00:00:00.000Z',
    lane: 'cloud-gcp',
    leaseRefs: ['placement:abc'],
    placementRef: 'placement:abc',
    repoRef: 'repo:openagents/openagents',
    repoTrustTier: 'private',
    sessionId: 'ccs_fixed',
    state: 'running',
    timeoutSeconds: 1800,
  }

  test('is inert (404) when the flag is disabled', async () => {
    const response = await run(
      handleCloudCodingSessionGet(
        getRequest('ccs_fixed'),
        'ccs_fixed',
        baseDeps({ enabled: false }),
      ),
    )
    expect(response.status).toBe(404)
    expect(((await response.json()) as { error: string }).error).toBe(
      'cloud_coding_sessions_disabled',
    )
  })

  test('rejects an unauthenticated read with 401', async () => {
    const response = await run(
      handleCloudCodingSessionGet(
        getRequest('ccs_fixed'),
        'ccs_fixed',
        baseDeps({ authenticate: authNone }),
      ),
    )
    expect(response.status).toBe(401)
  })

  test('the stub adapter resolves every read to 404 (no persistence)', async () => {
    const response = await run(
      handleCloudCodingSessionGet(
        getRequest('ccs_fixed'),
        'ccs_fixed',
        baseDeps(),
      ),
    )
    expect(response.status).toBe(404)
  })

  test('resolves a session for the owning account', async () => {
    const response = await run(
      handleCloudCodingSessionGet(
        getRequest('ccs_fixed'),
        'ccs_fixed',
        baseDeps({ adapter: readyAdapter(sample) }),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.id).toBe('ccs_fixed')
    expect(body.state).toBe('running')
    expect(body.placement_ref).toBe('placement:abc')
  })

  test('cross-account isolation: another account gets 404, not the session', async () => {
    const response = await run(
      handleCloudCodingSessionGet(
        getRequest('ccs_fixed'),
        'ccs_fixed',
        baseDeps({ adapter: readyAdapter(sample), authenticate: authOther }),
      ),
    )
    expect(response.status).toBe(404)
  })
})

describe('routeCloudCodingSessionRequest dispatcher', () => {
  test('returns undefined for a non-matching path', () => {
    const effect = routeCloudCodingSessionRequest(
      new Request('https://openagents.com/v1/sandboxes', { method: 'POST' }),
      baseDeps(),
    )
    expect(effect).toBeUndefined()
  })

  test('routes the base path to launch', async () => {
    const effect = routeCloudCodingSessionRequest(
      launchRequest(validBody),
      baseDeps(),
    )
    expect(effect).toBeDefined()
    const response = await run(effect!)
    expect(response.status).toBe(200)
    expect(((await response.json()) as { object: string }).object).toBe(
      'cloud.coding_session',
    )
  })

  test('routes a /:id path to the lifecycle read', async () => {
    const effect = routeCloudCodingSessionRequest(
      getRequest('ccs_fixed'),
      baseDeps(),
    )
    expect(effect).toBeDefined()
    // Stub adapter => 404 (no persistence), proving it reached the GET handler.
    const response = await run(effect!)
    expect(response.status).toBe(404)
  })

  test('returns undefined for a trailing-slash-only or nested path', () => {
    expect(
      routeCloudCodingSessionRequest(
        new Request('https://openagents.com/v1/cloud-coding-sessions/', {
          method: 'GET',
        }),
        baseDeps(),
      ),
    ).toBeUndefined()
    expect(
      routeCloudCodingSessionRequest(
        new Request('https://openagents.com/v1/cloud-coding-sessions/a/b', {
          method: 'GET',
        }),
        baseDeps(),
      ),
    ).toBeUndefined()
  })
})

describe('makeLedgerCloudCodingMeteringHook', () => {
  test('reports metered:false at launch time (no metered usage yet)', async () => {
    const hook = makeLedgerCloudCodingMeteringHook({
      db: {} as never,
      priceUsd: () => 1,
      usdToMsat: usd => Math.ceil(usd * 1000),
    })
    const outcome = await run(
      hook({ accountRef: 'agent:x', lane: 'cloud-gcp', sessionId: 's1' }),
    )
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBeNull()
  })

  test('a zero-usd charge is metered with a receipt ref and no debit', async () => {
    const hook = makeLedgerCloudCodingMeteringHook({
      db: {} as never,
      priceUsd: () => 0,
      usdToMsat: usd => Math.ceil(usd * 1000),
    })
    const outcome = await run(
      hook({
        accountRef: 'agent:x',
        lane: 'cloud-gcp',
        sessionId: 's1',
        usage: { wallSeconds: 120 },
      }),
    )
    expect(outcome.metered).toBe(true)
    expect(outcome.receiptRef).toBe(cloudCodingSessionReceiptRef('s1'))
  })
})
