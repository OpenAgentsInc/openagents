import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type CloudCodingAuth,
  type CloudCodingAdmissionGate,
  type CloudCodingMeteringHook,
  type CloudCodingRuntimeAdapter,
  type CloudCodingSession,
  type CloudCodingSessionServiceDeps,
  AGENT_COMPUTER_ISOLATION_POLICY_SCHEMA,
  CloudCodingAdapterError,
  MAX_CLOUD_CODING_TIMEOUT_SECONDS,
  admissibleLanesForTrustTier,
  allowCloudCodingAdmissionGate,
  cloudCodingSessionReceiptRef,
  configuredAgentComputerCapacitySnapshot,
  decideCloudCodingAdmission,
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
  admissionGate: allowCloudCodingAdmissionGate,
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

describe('agent computer admission policy', () => {
  const snapshot = {
    accountRef: 'agent:test-user',
    activeSessions: 0,
    agentComputerCapacityAvailable: true,
    availableBalanceMsat: 10_000,
    capacityRef: 'capacity.agent_computer.control_plane.armed',
    requestsInWindow: 0,
  }

  test('allows only when balance, per-user limits, and org capacity are all healthy', () => {
    const decision = decideCloudCodingAdmission({ snapshot })
    expect(decision.allowed).toBe(true)
    if (decision.allowed) {
      expect(decision.availableBalanceMsat).toBe(10_000)
      expect(decision.capacityRef).toBe(
        'capacity.agent_computer.control_plane.armed',
      )
    }
  })

  test('positive-credit gate refuses zero-balance users before capacity is consumed', () => {
    const decision = decideCloudCodingAdmission({
      snapshot: { ...snapshot, availableBalanceMsat: 0 },
    })
    expect(decision.allowed).toBe(false)
    if (!decision.allowed) {
      expect(decision.reason).toBe('insufficient_credit')
      expect(decision.statusCode).toBe(402)
    }
  })

  test('per-user concurrency and request-window caps produce typed rate_limited refusals', () => {
    const concurrent = decideCloudCodingAdmission({
      limits: { maxConcurrentSessions: 1, maxRequests: 10, windowSeconds: 60 },
      snapshot: { ...snapshot, activeSessions: 1 },
    })
    const requestWindow = decideCloudCodingAdmission({
      limits: { maxConcurrentSessions: 2, maxRequests: 1, windowSeconds: 60 },
      snapshot: { ...snapshot, requestsInWindow: 1 },
    })
    expect(concurrent.allowed).toBe(false)
    expect(requestWindow.allowed).toBe(false)
    if (!concurrent.allowed && !requestWindow.allowed) {
      expect(concurrent.reason).toBe('rate_limited')
      expect(requestWindow.reason).toBe('rate_limited')
      expect(concurrent.statusCode).toBe(429)
      expect(requestWindow.statusCode).toBe(429)
    }
  })

  test('org capacity unavailable is distinct from credit and rate refusals', () => {
    const decision = decideCloudCodingAdmission({
      snapshot: { ...snapshot, agentComputerCapacityAvailable: false },
    })
    expect(decision.allowed).toBe(false)
    if (!decision.allowed) {
      expect(decision.reason).toBe('org_capacity_unavailable')
      expect(decision.statusCode).toBe(503)
    }
  })

  test('configured control-plane readiness is public-safe and fail-closed', () => {
    expect(
      configuredAgentComputerCapacitySnapshot({
        baseUrl: 'https://cloud.openagents.test',
        bearerToken: 'token',
        gceProvisioningArmed: true,
      }),
    ).toEqual({
      available: true,
      availableSlots: 1,
      capacityRef: 'capacity.agent_computer.control_plane.armed',
    })
    expect(
      configuredAgentComputerCapacitySnapshot({
        baseUrl: 'https://cloud.openagents.test',
        bearerToken: '',
        gceProvisioningArmed: true,
      }),
    ).toEqual({
      available: false,
      availableSlots: 0,
      capacityRef: 'capacity.agent_computer.control_plane.unavailable',
    })
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
        admissionGate: allowCloudCodingAdmissionGate,
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

  test('rejects caller-supplied user Pylon selectors before admission or placement', async () => {
    let admitted = false
    let launched = false
    const admissionGate: CloudCodingAdmissionGate = context => {
      admitted = true
      return allowCloudCodingAdmissionGate(context)
    }
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
          pylonRef: 'pylon.user.somebody-else',
        }),
        baseDeps({ adapter, admissionGate }),
      ),
    )
    expect(response.status).toBe(400)
    expect(((await response.json()) as { error: string }).error).toBe(
      'user_pylon_capacity_not_admissible',
    )
    expect(admitted).toBe(false)
    expect(launched).toBe(false)
  })

  test('refuses insufficient_credit before cloud placement', async () => {
    let launched = false
    const admissionGate: CloudCodingAdmissionGate = () =>
      Effect.succeed(
        decideCloudCodingAdmission({
          snapshot: {
            accountRef: 'agent:test-user',
            activeSessions: 0,
            agentComputerCapacityAvailable: true,
            availableBalanceMsat: 0,
            capacityRef: 'capacity.agent_computer.control_plane.armed',
            requestsInWindow: 0,
          },
        }),
      )
    const adapter: CloudCodingRuntimeAdapter = {
      ...stubCloudCodingAdapter,
      launch: input => {
        launched = true
        return stubCloudCodingAdapter.launch(input)
      },
    }
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ adapter, admissionGate }),
      ),
    )
    expect(response.status).toBe(402)
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('insufficient_credit')
    expect(body.reason).toBe('insufficient_credit')
    expect(launched).toBe(false)
  })

  test('refuses rate_limited with rate headers before cloud placement', async () => {
    let launched = false
    const admissionGate: CloudCodingAdmissionGate = () =>
      Effect.succeed(
        decideCloudCodingAdmission({
          limits: { maxConcurrentSessions: 1, maxRequests: 1, windowSeconds: 60 },
          snapshot: {
            accountRef: 'agent:test-user',
            activeSessions: 1,
            agentComputerCapacityAvailable: true,
            availableBalanceMsat: 10_000,
            capacityRef: 'capacity.agent_computer.control_plane.armed',
            requestsInWindow: 0,
          },
        }),
      )
    const adapter: CloudCodingRuntimeAdapter = {
      ...stubCloudCodingAdapter,
      launch: input => {
        launched = true
        return stubCloudCodingAdapter.launch(input)
      },
    }
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ adapter, admissionGate }),
      ),
    )
    expect(response.status).toBe(429)
    expect(response.headers.get('ratelimit-limit')).toBe('1')
    expect(response.headers.get('ratelimit-policy')).toBe('1;w=60')
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('rate_limited')
    expect(body.reason).toBe('rate_limited')
    expect(launched).toBe(false)
  })

  test('refuses org_capacity_unavailable before cloud placement', async () => {
    let launched = false
    const admissionGate: CloudCodingAdmissionGate = () =>
      Effect.succeed(
        decideCloudCodingAdmission({
          snapshot: {
            accountRef: 'agent:test-user',
            activeSessions: 0,
            agentComputerCapacityAvailable: false,
            availableBalanceMsat: 10_000,
            capacityRef: 'capacity.agent_computer.control_plane.unavailable',
            requestsInWindow: 0,
          },
        }),
      )
    const adapter: CloudCodingRuntimeAdapter = {
      ...stubCloudCodingAdapter,
      launch: input => {
        launched = true
        return stubCloudCodingAdapter.launch(input)
      },
    }
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ adapter, admissionGate }),
      ),
    )
    expect(response.status).toBe(503)
    const body = (await response.json()) as {
      capacity_ref: string
      error: string
      reason: string
    }
    expect(body.error).toBe('org_capacity_unavailable')
    expect(body.reason).toBe('org_capacity_unavailable')
    expect(body.capacity_ref).toBe(
      'capacity.agent_computer.control_plane.unavailable',
    )
    expect(launched).toBe(false)
  })

  test('launches a queued cloud session through the stub adapter with honest null refs', async () => {
    const response = await run(
      handleCloudCodingSessionLaunch(launchRequest(validBody), baseDeps()),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.object).toBe('cloud.coding_session')
    expect(body.product_object).toBe('agent.computer_session')
    expect(body.id).toBe('ccs_fixed')
    expect(body.lane).toBe('cloud-gcp')
    expect(body.adapter).toBe('codex')
    expect(body.repo_trust_tier).toBe('private')
    expect(body.state).toBe('queued')
    // Honest: the stub provisions no VM and runs no edit.
    expect(body.placement_ref).toBeNull()
    expect(body.lease_refs).toEqual([])
    expect(body.work_context_ref).toBe('work-context.agent-computer.ccs_fixed')
    expect(body.agent_computer_ref).toBeNull()
    expect(body.agent_computer_state).toBe('requested')
    expect(body.lifecycle_receipt_refs).toEqual([])
    expect(body.resource_usage_receipt_refs).toEqual([])
    expect(body.agent_computer).toEqual({
      lifecycle_receipt_refs: [],
      ref: null,
      resource_usage_receipt_refs: [],
      state: 'requested',
      work_context_ref: 'work-context.agent-computer.ccs_fixed',
    })
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

  test('projects an explicit mobile work-context ref and forwards the isolation contract to placement', async () => {
    let placementBody: Record<string, unknown> | undefined
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async (_url, init) => {
        placementBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        return Response.json({
          binding: {
            capacityClassId: 'gce-standard',
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
            workContextRef: 'work-context.mobile.thread-1.repo-1',
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
          repoBindingRef: 'repo-binding.mobile.thread-1',
          threadRef: 'thread.mobile.1',
          workContextRef: 'work-context.mobile.thread-1.repo-1',
        }),
        baseDeps({ adapter }),
      ),
    )
    expect(response.status).toBe(200)
    expect(placementBody).toMatchObject({
      repo_binding_ref: 'repo-binding.mobile.thread-1',
      thread_ref: 'thread.mobile.1',
      timeout_seconds: 1800,
      work_context_ref: 'work-context.mobile.thread-1.repo-1',
    })
    expect(placementBody?.agent_computer_isolation_policy).toMatchObject({
      schema_version: AGENT_COMPUTER_ISOLATION_POLICY_SCHEMA,
      unit: 'one_firecracker_microvm_per_work_context',
      credentials: {
        credential_scanner_required: true,
        scm_broker_only: true,
      },
      lifecycle: {
        hard_timeout_seconds: 1800,
        microvm_destroy_required: true,
        scratch_wipe_required: true,
      },
      network: {
        no_inbound: true,
      },
      projection: {
        public_refs_only: true,
      },
    })
    const body = (await response.json()) as Record<string, unknown>
    expect(body.work_context_ref).toBe('work-context.mobile.thread-1.repo-1')
    expect(body.agent_computer_ref).toBe('agent-computer.run_gce_1')
  })

  test('Seam A: forwards work_context_b64 on cloud-gcp when the option is present', async () => {
    let placementBody: Record<string, unknown> | undefined
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async (_url, init) => {
        placementBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        return Response.json({
          binding: {
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
            workContextRef: 'work-context.agent-computer.wc1',
          },
          externalRunId: 'run_gce_1',
          status: 'running',
        })
      },
      gceProvisioningArmed: true,
    })
    await Effect.runPromise(
      adapter.launch({
        accountRef: 'agent:test-user',
        lane: 'cloud-gcp',
        request: {
          adapter: 'codex',
          lane: 'cloud-gcp',
          objective: 'seam-a',
          options: { workContextB64: 'eyJhIjoxfQ==' },
          repoRef: 'repo:openagents/openagents',
          repoTrustTier: 'private',
          timeoutSeconds: 1800,
          verify: [],
          workContextRef: 'work-context.agent-computer.wc1',
        },
        sessionId: 'ccs_fixed',
      }),
    )
    expect(placementBody?.work_context_b64).toBe('eyJhIjoxfQ==')
  })

  test('Seam A: accepts the real daemon shape (workContextRef only in the cloud.gce.provisioning event, none on binding)', async () => {
    // The live oa-codex-control RunnerBinding carries NO work_context_ref
    // field; it reports the bound ref only inside the cloud.gce.provisioning
    // event data. The adapter must still validate the placement (not refuse
    // with agent_computer_work_context_binding_missing).
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async () =>
        Response.json({
          binding: {
            externalRunId: 'run_gce_daemon',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_daemon',
            // NOTE: no workContextRef here — exactly like the real daemon.
          },
          events: [
            {
              dataJson: JSON.stringify({
                externalRunId: 'run_gce_daemon',
                lane: 'cloud-gcp',
                workContextRef: 'work-context.agent-computer.ccs_daemon',
              }),
              kind: 'cloud.gce.provisioning',
            },
          ],
          externalRunId: 'run_gce_daemon',
          status: 'provisioning',
        }),
      gceProvisioningArmed: true,
    })
    const session = await Effect.runPromise(
      adapter.launch({
        accountRef: 'agent:test-user',
        lane: 'cloud-gcp',
        request: {
          adapter: 'codex',
          lane: 'cloud-gcp',
          objective: 'seam-a',
          options: { workContextB64: 'eyJhIjoxfQ==' },
          repoRef: 'repo:openagents/openagents',
          repoTrustTier: 'private',
          timeoutSeconds: 1800,
          verify: [],
          workContextRef: 'work-context.agent-computer.ccs_daemon',
        },
        sessionId: 'ccs_daemon',
      }),
    )
    expect(session.workContextRef).toBe('work-context.agent-computer.ccs_daemon')
    expect(session.agentComputerState).toBeDefined()
  })

  test('Seam A: does NOT forward work_context_b64 on the cloud-shc lane', async () => {
    let placementBody: Record<string, unknown> | undefined
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async (_url, init) => {
        placementBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        return Response.json({
          binding: {
            externalRunId: 'run_shc_1',
            lane: 'cloud-shc',
            providerLane: 'shc',
            runnerId: 'runner_shc_1',
            workContextRef: 'work-context.agent-computer.wc1',
          },
          externalRunId: 'run_shc_1',
          status: 'running',
        })
      },
      gceProvisioningArmed: true,
    })
    await Effect.runPromise(
      adapter.launch({
        accountRef: 'agent:test-user',
        lane: 'cloud-shc',
        request: {
          adapter: 'codex',
          lane: 'cloud-shc',
          objective: 'seam-a',
          options: { workContextB64: 'eyJhIjoxfQ==' },
          repoRef: 'repo:openagents/openagents',
          repoTrustTier: 'private',
          timeoutSeconds: 1800,
          verify: [],
          workContextRef: 'work-context.agent-computer.wc1',
        },
        sessionId: 'ccs_fixed',
      }),
    )
    expect('work_context_b64' in (placementBody ?? {})).toBe(false)
  })

  test('Seam A: omits work_context_b64 when no option is supplied (Codex path)', async () => {
    let placementBody: Record<string, unknown> | undefined
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async (_url, init) => {
        placementBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        return Response.json({
          binding: {
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
            workContextRef: 'work-context.agent-computer.wc1',
          },
          externalRunId: 'run_gce_1',
          status: 'running',
        })
      },
      gceProvisioningArmed: true,
    })
    await Effect.runPromise(
      adapter.launch({
        accountRef: 'agent:test-user',
        lane: 'cloud-gcp',
        request: {
          adapter: 'codex',
          lane: 'cloud-gcp',
          objective: 'seam-a',
          options: {},
          repoRef: 'repo:openagents/openagents',
          repoTrustTier: 'private',
          timeoutSeconds: 1800,
          verify: [],
          workContextRef: 'work-context.agent-computer.wc1',
        },
        sessionId: 'ccs_fixed',
      }),
    )
    expect('work_context_b64' in (placementBody ?? {})).toBe(false)
  })

  test('fails closed when the control plane binds a placement to another work context', async () => {
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async () =>
        Response.json({
          binding: {
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
            workContextRef: 'work-context.mobile.someone-else',
          },
          externalRunId: 'run_gce_1',
          status: 'running',
        }),
      gceProvisioningArmed: true,
    })
    const response = await run(
      handleCloudCodingSessionLaunch(launchRequest(validBody), {
        ...baseDeps(),
        adapter,
      }),
    )
    expect(response.status).toBe(502)
    expect(((await response.json()) as { reason: string }).reason).toBe(
      'agent_computer_work_context_binding_mismatch',
    )
  })

  test('fails closed when a cleanup event lacks scratch-wipe or microVM-destroy evidence', async () => {
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: 'secret-test-token',
      fetch: async () =>
        Response.json({
          binding: {
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
            workContextRef: 'work-context.agent-computer.ccs_fixed',
          },
          events: [
            {
              dataJson: JSON.stringify({
                cleanupReceiptRef: 'sha256:cleanup-only',
              }),
              receiptRefs: ['sha256:cleanup-only'],
              type: 'cloud.gce.cleanup',
            },
          ],
          externalRunId: 'run_gce_1',
          status: 'running',
        }),
      gceProvisioningArmed: true,
    })
    const response = await run(
      handleCloudCodingSessionLaunch(launchRequest(validBody), {
        ...baseDeps(),
        adapter,
      }),
    )
    expect(response.status).toBe(502)
    expect(((await response.json()) as { reason: string }).reason).toBe(
      'agent_computer_reclaim_evidence_missing',
    )
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

  test('fails closed when live provisioning is armed but the control URL is absent', async () => {
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: '',
      bearerToken: 'secret-test-token',
      gceProvisioningArmed: true,
    })
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ adapter }),
      ),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).toBe('cloud_control_url_not_configured')
  })

  test('fails closed when live provisioning is armed but the control token is absent', async () => {
    const adapter = makeCloudControlCloudCodingAdapter({
      baseUrl: 'https://cloud.openagents.test',
      bearerToken: '',
      gceProvisioningArmed: true,
    })
    const response = await run(
      handleCloudCodingSessionLaunch(
        launchRequest(validBody),
        baseDeps({ adapter }),
      ),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { reason: string }
    expect(body.reason).toBe('cloud_control_token_not_configured')
  })

  test('with live provisioning armed, posts to cloud placement and projects agent computer receipts', async () => {
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
            externalRunId: 'run_gce_1',
            lane: 'cloud-gcp',
            providerLane: 'gcp',
            runnerId: 'runner_gce_1',
            workContextRef: 'work-context.agent-computer.ccs_fixed',
          },
          events: [
            {
              dataJson: JSON.stringify({
                instanceRef: 'gce-raw-instance-name',
                leaseRef: 'lease.gce.vm.ccs_fixed',
                provisionReceiptRef: 'sha256:provision',
              }),
              kind: 'placement',
              receiptRefs: ['sha256:provision'],
              type: 'cloud.gce.provisioned',
            },
            {
              dataJson: JSON.stringify({
                resourceUsageReceiptRef: 'sha256:usage',
              }),
              kind: 'receipt',
              receiptRefs: ['sha256:usage'],
              type: 'cloud.gce.resource_usage_receipt',
            },
            {
              dataJson: JSON.stringify({
                cleanupReceiptRef: 'sha256:cleanup',
                leaseRef: 'lease.gce.vm.ccs_fixed',
                microvmDestroyReceiptRef: 'sha256:microvm-destroy',
                scratchWipeReceiptRef: 'sha256:scratch-wipe',
              }),
              kind: 'cleanup',
              receiptRefs: ['sha256:cleanup'],
              type: 'cloud.gce.cleanup',
            },
          ],
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
      agent_computer_isolation_policy: {
        schema_version: AGENT_COMPUTER_ISOLATION_POLICY_SCHEMA,
      },
      lane: 'cloud-gcp',
      owner_ref: 'agent:test-user',
      provider_account_ref: 'provider-account.public.test',
      repository: validBody.repoRef,
      run_id: 'ccs_fixed',
      timeout_seconds: 1800,
      wallet_authority: false,
      work_context_ref: 'work-context.agent-computer.ccs_fixed',
    })
    const body = (await response.json()) as Record<string, unknown>
    expect(body.state).toBe('running')
    expect(body.placement_ref).toBe('placement.cloud-coding.run_gce_1')
    expect(body.agent_computer_ref).toBe('agent-computer.run_gce_1')
    expect(body.agent_computer_state).toBe('reclaimed')
    expect(body.lifecycle_receipt_refs).toEqual([
      'sha256:provision',
      'sha256:cleanup',
      'sha256:scratch-wipe',
      'sha256:microvm-destroy',
    ])
    expect(body.resource_usage_receipt_refs).toEqual(['sha256:usage'])
    expect(body.lease_refs).toEqual([
      'placement.cloud-coding.run_gce_1',
      'cloud-run.run_gce_1',
      'cloud-runner.runner_gce_1',
      'cloud-capacity-class.gce-standard',
      'lease.gce.vm.ccs_fixed',
    ])
    expect(JSON.stringify(body)).not.toContain('gce-raw-instance-name')
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
    lifecycleReceiptRefs: ['sha256:provision'],
    leaseRefs: ['placement:abc'],
    agentComputerRef: 'agent-computer.run_gce_1',
    agentComputerState: 'active',
    placementRef: 'placement:abc',
    repoRef: 'repo:openagents/openagents',
    repoTrustTier: 'private',
    resourceUsageReceiptRefs: ['sha256:usage'],
    sessionId: 'ccs_fixed',
    state: 'running',
    timeoutSeconds: 1800,
    workContextRef: 'work-context.mobile.thread-1.repo-1',
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
    expect(body.agent_computer_ref).toBe('agent-computer.run_gce_1')
    expect(body.work_context_ref).toBe('work-context.mobile.thread-1.repo-1')
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
      ledgerDb: {} as never,
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
      ledgerDb: {} as never,
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
