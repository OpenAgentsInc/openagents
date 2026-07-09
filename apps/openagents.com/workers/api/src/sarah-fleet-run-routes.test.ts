import { canonicalJson, fleetRunScope } from '@openagentsinc/khala-sync'
import {
  decodeFleetRunAuthorityStartRequest,
  FleetRunAuthorityError,
  FleetRunAuthorityObserveInput,
  FleetRunAuthorityRecord,
  FleetRunAuthorityStartInput,
  type FleetRunAuthorityRecord as FleetRunAuthorityRecordType,
  type FleetRunAuthorityRepositoryShape,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeSarahFleetRunRoutes,
  SARAH_FLEET_RUN_REQUEST_MAX_BYTES,
  SARAH_FLEET_RUNS_PATH,
  SARAH_FLEET_RUNS_ROUTE_REF,
  sarahFleetRunPolicyForMode,
  type SarahFleetRunAuthenticatedOwner,
  type SarahRelationshipMode,
} from './sarah-fleet-run-routes'

const FIXED_NOW = '2026-07-09T23:30:00.000Z'
const COMMIT = '3f34c65e3fc0e8914f7c112edcac85c6eeaac4f9'
const CONNECTION_STRING =
  'postgresql://operator:private-password@private-host/khala_sync'

const fleetRequest = (
  idempotencyKey: string,
  objective = 'Implement one bounded public issue.',
) => ({
  objective,
  repository: {
    owner: 'OpenAgentsInc',
    name: 'openagents',
    branch: 'main',
    commit: COMMIT,
  },
  verifier: { kind: 'command', command: 'bun test' },
  workSource: { kind: 'issue_list', issueRefs: ['#8637'] },
  workerPolicy: { workerKind: 'codex', targetPreference: 'owner_local' },
  targetConcurrency: 2,
  idempotencyKey,
})

const sha256Hex = async (value: unknown): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(canonicalJson(value)),
  )
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const makeFakeAuthority = (): FleetRunAuthorityRepositoryShape => {
  const recordsByOwnerAndIdempotency = new Map<
    string,
    FleetRunAuthorityRecordType
  >()
  const recordsByOwnerAndRun = new Map<string, FleetRunAuthorityRecordType>()

  const start: FleetRunAuthorityRepositoryShape['start'] = raw =>
    Effect.tryPromise({
      try: async () => {
        const decoded = S.decodeUnknownSync(FleetRunAuthorityStartInput)(raw, {
          onExcessProperty: 'error',
        })
        const request = decodeFleetRunAuthorityStartRequest(decoded.request)
        const idempotencyLookup = `${decoded.ownerUserId}:${request.idempotencyKey}`
        const fingerprint = await sha256Hex(request)
        const existing = recordsByOwnerAndIdempotency.get(idempotencyLookup)
        if (existing !== undefined) {
          if (existing.requestFingerprint !== fingerprint) {
            throw new FleetRunAuthorityError({
              kind: 'idempotency_conflict',
              reason: 'fixed conflict',
            })
          }
          return { duplicate: true, record: existing }
        }
        const runRef = `fleet_run.sarah.${(
          await sha256Hex({
            idempotencyKey: request.idempotencyKey,
            ownerUserId: decoded.ownerUserId,
          })
        ).slice(0, 20)}`
        const record = S.decodeUnknownSync(FleetRunAuthorityRecord)({
          schema: 'openagents.sarah.fleet_run_authority.v1',
          runRef,
          scope: fleetRunScope(runRef),
          ownerUserId: decoded.ownerUserId,
          requestFingerprint: fingerprint,
          status: 'pending_executor',
          request,
          createdAt: FIXED_NOW,
          updatedAt: FIXED_NOW,
        })
        recordsByOwnerAndIdempotency.set(idempotencyLookup, record)
        recordsByOwnerAndRun.set(`${decoded.ownerUserId}:${runRef}`, record)
        return { duplicate: false, record }
      },
      catch: error =>
        error instanceof FleetRunAuthorityError
          ? error
          : new FleetRunAuthorityError({
              kind: 'invalid_request',
              reason: 'fixed invalid request',
            }),
    })

  const observe: FleetRunAuthorityRepositoryShape['observe'] = raw =>
    Effect.try({
      try: () => {
        const decoded = S.decodeUnknownSync(FleetRunAuthorityObserveInput)(
          raw,
          { onExcessProperty: 'error' },
        )
        const record = recordsByOwnerAndRun.get(
          `${decoded.ownerUserId}:${decoded.runRef}`,
        )
        if (record === undefined) {
          throw new FleetRunAuthorityError({
            kind: 'run_not_found',
            reason: 'fixed not found',
          })
        }
        return { record }
      },
      catch: error =>
        error instanceof FleetRunAuthorityError
          ? error
          : new FleetRunAuthorityError({
              kind: 'invalid_request',
              reason: 'fixed invalid request',
            }),
    })

  const unsupported = () =>
    Effect.fail(
      new FleetRunAuthorityError({
        kind: 'invalid_request',
        reason: 'not available in this route fixture',
      }),
    )

  return {
    start,
    observe,
    claim: unsupported,
    acceptClaim: unsupported,
  }
}

type TestEnv = Readonly<{
  KHALA_SYNC_DB?: { connectionString: string } | undefined
}>

const policyModeForOwner = (
  owner: SarahFleetRunAuthenticatedOwner,
): SarahRelationshipMode => {
  if (owner.userId === 'user-prospect') {
    return 'prospect'
  }
  if (owner.userId === 'user-admin') {
    return 'administrator'
  }
  if (owner.userId === 'user-operator') {
    return 'operator'
  }
  return 'customer'
}

const makeHarness = (
  input: Readonly<{
    env?: TestEnv
    authenticationError?: Error
    factoryError?: Error
    policyError?: Error
  }> = {},
) => {
  const authority = makeFakeAuthority()
  const calls = { clients: 0, ended: 0 }
  const routes = makeSarahFleetRunRoutes<TestEnv>({
    authenticateOwner: request => {
      if (input.authenticationError !== undefined) {
        return Promise.reject(input.authenticationError)
      }
      const userId = request.headers.get('x-test-user')
      return Promise.resolve(
        userId === null
          ? undefined
          : {
              userId,
              email: `${userId}@example.com`,
              appendRefreshedSessionCookies: response => {
                response.headers.set('x-test-session-refreshed', '1')
                return response
              },
            },
      )
    },
    resolveRelationshipMode: owner =>
      input.policyError === undefined
        ? Promise.resolve(policyModeForOwner(owner))
        : Promise.reject(input.policyError),
    makeSqlClient: () => {
      calls.clients += 1
      if (input.factoryError !== undefined) {
        return Promise.reject(input.factoryError)
      }
      return Promise.resolve({
        sql: {} as SyncSql,
        end: () => {
          calls.ended += 1
          return Promise.resolve()
        },
      })
    },
    makeRepository: () => authority,
  })
  const env: TestEnv =
    'env' in input
      ? input.env ?? {}
      : { KHALA_SYNC_DB: { connectionString: CONNECTION_STRING } }
  const run = (request: Request) =>
    Effect.runPromise(routes.handle(request, env, {} as ExecutionContext))
  return { calls, run }
}

const post = (
  body: unknown,
  userId?: string,
  query = '',
): Request =>
  new Request(`https://openagents.com${SARAH_FLEET_RUNS_PATH}${query}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(userId === undefined ? {} : { 'x-test-user': userId }),
    },
    body: JSON.stringify(body),
  })

const observe = (runRef: string, userId: string, extraQuery = ''): Request =>
  new Request(
    `https://openagents.com${SARAH_FLEET_RUNS_PATH}?runRef=${encodeURIComponent(runRef)}${extraQuery}`,
    { headers: { 'x-test-user': userId } },
  )

describe('Sarah FleetRun relationship policy', () => {
  test('policy, not model or request input, owns posture, scope, and administrator tools', () => {
    expect(sarahFleetRunPolicyForMode('prospect')).toEqual({
      source: 'openagents_server_policy',
      relationshipMode: 'prospect',
      codingFleetStartAllowed: false,
      fleetObservationAllowed: false,
      retrievalScope: 'public_only',
      responsePosture: 'guided',
      uiDensity: 'standard',
      administratorToolsAllowed: false,
    })
    expect(sarahFleetRunPolicyForMode('operator')).toMatchObject({
      codingFleetStartAllowed: true,
      relationshipMode: 'operator',
      responsePosture: 'state_oriented',
      uiDensity: 'dense',
      administratorToolsAllowed: false,
    })
    expect(sarahFleetRunPolicyForMode('administrator')).toMatchObject({
      relationshipMode: 'administrator',
      administratorToolsAllowed: true,
    })
  })
})

describe('Sarah FleetRun authenticated route', () => {
  test('refuses unauthenticated and prospect-mode callers before storage', async () => {
    const unauthenticated = makeHarness()
    const missing = await unauthenticated.run(
      post(fleetRequest('unauthenticated-1')),
    )
    expect(missing.status).toBe(401)
    expect(await missing.json()).toMatchObject({
      error: { code: 'unauthenticated' },
      ok: false,
    })
    expect(unauthenticated.calls.clients).toBe(0)

    const prospect = makeHarness()
    const refused = await prospect.run(
      post(fleetRequest('prospect-mode-1'), 'user-prospect'),
    )
    expect(refused.status).toBe(403)
    expect(await refused.json()).toMatchObject({
      error: { code: 'relationship_not_authorized' },
      policy: {
        codingFleetStartAllowed: false,
        relationshipMode: 'prospect',
      },
    })
    expect(prospect.calls.clients).toBe(0)
  })

  test('distinguishes unavailable auth or relationship policy from unauthenticated', async () => {
    const authentication = makeHarness({
      authenticationError: new Error('private auth backend detail'),
    })
    const authenticationFailure = await authentication.run(
      post(fleetRequest('auth-unavailable-1'), 'user-operator'),
    )
    expect(authenticationFailure.status).toBe(503)
    expect(await authenticationFailure.json()).toMatchObject({
      error: { code: 'authentication_unavailable', retryable: true },
      ok: false,
    })
    expect(authentication.calls.clients).toBe(0)

    const policy = makeHarness({
      policyError: new Error('private policy backend detail'),
    })
    const policyFailure = await policy.run(
      post(fleetRequest('policy-unavailable-1'), 'user-operator'),
    )
    expect(policyFailure.status).toBe(503)
    expect(await policyFailure.json()).toMatchObject({
      error: { code: 'relationship_policy_unavailable', retryable: true },
      ok: false,
    })
    expect(policy.calls.clients).toBe(0)
  })

  test('creates and observes only a public-safe owner-authorized projection', async () => {
    const harness = makeHarness()
    const created = await harness.run(
      post(fleetRequest('operator-create-1'), 'user-operator'),
    )
    expect(created.status).toBe(200)
    expect(created.headers.get('x-test-session-refreshed')).toBe('1')
    const createdBody = (await created.json()) as {
      duplicate: boolean
      policy: Record<string, unknown>
      routeRef: string
      run: Record<string, unknown>
    }
    expect(createdBody.routeRef).toBe(SARAH_FLEET_RUNS_ROUTE_REF)
    expect(createdBody.duplicate).toBe(false)
    expect(createdBody.policy).toMatchObject({
      relationshipMode: 'operator',
      retrievalScope: 'owner_fleet_runs',
      responsePosture: 'state_oriented',
      uiDensity: 'dense',
    })
    expect(createdBody.run).toMatchObject({
      status: 'pending_executor',
      privateMaterialExcluded: true,
    })
    expect(createdBody.run).not.toHaveProperty('ownerUserId')
    expect(createdBody.run).not.toHaveProperty('requestFingerprint')
    expect(createdBody.run).not.toHaveProperty('idempotencyKey')
    expect(JSON.stringify(createdBody)).not.toContain('user-operator')

    const observed = await harness.run(
      observe(String(createdBody.run.runRef), 'user-operator'),
    )
    expect(observed.status).toBe(200)
    expect(await observed.json()).toMatchObject({
      ok: true,
      run: { runRef: createdBody.run.runRef },
    })
    expect(harness.calls.ended).toBe(2)
  })

  test('replays identical owner idempotency and returns a fixed changed-input conflict', async () => {
    const harness = makeHarness()
    const original = await harness.run(
      post(fleetRequest('idempotency-route-1'), 'user-customer'),
    )
    const replay = await harness.run(
      post(fleetRequest('idempotency-route-1'), 'user-customer'),
    )
    expect(original.status).toBe(200)
    expect(replay.status).toBe(200)
    const originalBody = (await original.json()) as {
      run: { runRef: string }
    }
    const replayBody = (await replay.json()) as {
      duplicate: boolean
      run: { runRef: string }
    }
    expect(replayBody.duplicate).toBe(true)
    expect(replayBody.run.runRef).toBe(originalBody.run.runRef)

    const conflict = await harness.run(
      post(
        fleetRequest(
          'idempotency-route-1',
          'Implement a different bounded public issue.',
        ),
        'user-customer',
      ),
    )
    expect(conflict.status).toBe(409)
    const conflictText = await conflict.text()
    expect(conflictText).toContain('idempotency_conflict')
    expect(conflictText).not.toContain('different bounded')
    expect(conflictText).not.toContain(CONNECTION_STRING)
  })

  test('cross-owner observation is indistinguishable from an absent run', async () => {
    const harness = makeHarness()
    const created = await harness.run(
      post(fleetRequest('owner-isolation-1'), 'user-owner-a'),
    )
    const body = (await created.json()) as { run: { runRef: string } }
    const foreign = await harness.run(
      observe(body.run.runRef, 'user-owner-b'),
    )
    expect(foreign.status).toBe(404)
    const foreignText = await foreign.text()
    expect(foreignText).toContain('run_not_found')
    expect(foreignText).not.toContain('user-owner-a')
    expect(foreignText).not.toContain(body.run.runRef)
  })

  test('rejects caller-supplied owner and relationship policy fields', async () => {
    const harness = makeHarness()
    const ownerOverride = await harness.run(
      post(
        { ...fleetRequest('owner-override-1'), ownerUserId: 'user-foreign' },
        'user-operator',
      ),
    )
    expect(ownerOverride.status).toBe(400)

    const policyOverride = await harness.run(
      post(
        {
          ...fleetRequest('policy-override-1'),
          relationshipMode: 'administrator',
        },
        'user-operator',
      ),
    )
    expect(policyOverride.status).toBe(400)

    const queryOverride = await harness.run(
      new Request(
        `https://openagents.com${SARAH_FLEET_RUNS_PATH}?runRef=fleet_run.sarah.${'a'.repeat(20)}&ownerUserId=user-foreign`,
        { headers: { 'x-test-user': 'user-operator' } },
      ),
    )
    expect(queryOverride.status).toBe(400)
  })

  test('fails closed with no configured Postgres authority and has no fallback', async () => {
    const harness = makeHarness({ env: {} })
    const response = await harness.run(
      post(fleetRequest('no-storage-1'), 'user-operator'),
    )
    expect(response.status).toBe(503)
    expect(await response.json()).toMatchObject({
      error: { code: 'storage_unavailable', retryable: true },
      ok: false,
    })
    expect(harness.calls.clients).toBe(0)
  })

  test('bounds request bytes and redacts storage failures', async () => {
    const oversized = makeHarness()
    const oversizedResponse = await oversized.run(
      post(
        {
          ...fleetRequest('oversized-body-1'),
          objective: 'x'.repeat(SARAH_FLEET_RUN_REQUEST_MAX_BYTES),
        },
        'user-operator',
      ),
    )
    expect(oversizedResponse.status).toBe(400)
    expect(oversized.calls.ended).toBe(1)

    const storage = makeHarness({
      factoryError: new Error(`connect failed at ${CONNECTION_STRING}`),
    })
    const storageResponse = await storage.run(
      post(fleetRequest('storage-failure-1'), 'user-operator'),
    )
    expect(storageResponse.status).toBe(503)
    const text = await storageResponse.text()
    expect(text).toContain('storage_unavailable')
    expect(text).not.toContain('private-host')
    expect(text).not.toContain('private-password')
  })

  test('rejects unsupported methods without authenticating or opening storage', async () => {
    const harness = makeHarness()
    const response = await harness.run(
      new Request(`https://openagents.com${SARAH_FLEET_RUNS_PATH}`, {
        method: 'DELETE',
      }),
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET, POST')
    expect(harness.calls.clients).toBe(0)
  })
})
