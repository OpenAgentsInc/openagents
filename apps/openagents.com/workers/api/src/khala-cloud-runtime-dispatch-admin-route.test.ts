import { describe, expect, test } from 'vitest'

import type { CloudGcpDispatchOutcome } from './khala-cloud-runtime-dispatch'
import {
  handleCloudGcpRuntimeDispatchAdminRoute,
  KHALA_CLOUD_RUNTIME_DISPATCH_ADMIN_PATH,
  type CloudGcpRuntimeDispatchAdminRouteDeps,
  type CloudGcpRuntimeDispatchContext,
} from './khala-cloud-runtime-dispatch-admin-routes'

type Env = Readonly<{ tag: 'test-env' }>
const env: Env = { tag: 'test-env' }

const validBody = {
  commit: '7fd1a60b01f91b314f59955a4e4d4e80d8edf11d',
  ownerUserId: 'github:14167547',
  repo: 'octocat/Hello-World',
  threadId: 'thread.t1',
  turnId: 'turn.t1',
  workContextRef: 'work-context.agent-computer.wc1',
}

const post = (body: unknown): Request =>
  new Request(`https://openagents.com${KHALA_CLOUD_RUNTIME_DISPATCH_ADMIN_PATH}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const launchedOutcome: CloudGcpDispatchOutcome = {
  credentialId: 'agentcred.seam-a.cloud-runtime.1',
  outcome: 'launched',
  placementRef: 'placement.cloud-coding.run_gce_1',
  sessionId: 'ccs.turn_t1',
  tokenRevoked: false,
}

const makeDeps = (
  overrides: Partial<CloudGcpRuntimeDispatchAdminRouteDeps<Env>> = {},
  ctx: CloudGcpRuntimeDispatchContext = {
    armed: true,
    configured: true,
    run: () => Promise.resolve(launchedOutcome),
  },
): { deps: CloudGcpRuntimeDispatchAdminRouteDeps<Env>; runCalls: Array<unknown> } => {
  const runCalls: Array<unknown> = []
  const wrapped: CloudGcpRuntimeDispatchContext = ctx.configured
    ? {
        armed: ctx.armed,
        configured: true,
        run: admitted => {
          runCalls.push(admitted)
          return ctx.run(admitted)
        },
      }
    : ctx
  return {
    deps: {
      requireAdminApiToken: () => Promise.resolve(true),
      resolveContext: () => Promise.resolve(wrapped),
      ...overrides,
    },
    runCalls,
  }
}

describe('handleCloudGcpRuntimeDispatchAdminRoute', () => {
  test('rejects non-POST', async () => {
    const { deps } = makeDeps()
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(
      new Request(`https://openagents.com${KHALA_CLOUD_RUNTIME_DISPATCH_ADMIN_PATH}`),
      env,
      deps,
    )
    expect(res.status).toBe(405)
  })

  test('401 without admin bearer', async () => {
    const { deps, runCalls } = makeDeps({
      requireAdminApiToken: () => Promise.resolve(false),
    })
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(post(validBody), env, deps)
    expect(res.status).toBe(401)
    expect(runCalls).toHaveLength(0)
  })

  test('400 on malformed body (missing fields)', async () => {
    const { deps, runCalls } = makeDeps()
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(
      post({ ownerUserId: 'github:1' }),
      env,
      deps,
    )
    expect(res.status).toBe(400)
    const json = (await res.json()) as { error: string }
    expect(json.error).toContain('missing_fields')
    expect(runCalls).toHaveLength(0)
  })

  test('503 when not configured', async () => {
    const { deps, runCalls } = makeDeps({}, { configured: false })
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(post(validBody), env, deps)
    expect(res.status).toBe(503)
    expect(runCalls).toHaveLength(0)
  })

  test('FAIL-CLOSED: 409 not_armed never runs the dispatch', async () => {
    const { deps, runCalls } = makeDeps(
      {},
      { armed: false, configured: true, run: () => Promise.resolve(launchedOutcome) },
    )
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(post(validBody), env, deps)
    expect(res.status).toBe(409)
    const json = (await res.json()) as { ok: boolean; reason: string; armed: boolean }
    expect(json.ok).toBe(false)
    expect(json.armed).toBe(false)
    expect(json.reason).toBe('cloud_gcp_runtime_not_armed')
    expect(runCalls).toHaveLength(0)
  })

  test('armed: drives one dispatch and echoes the outcome (incl. credentialId)', async () => {
    const { deps, runCalls } = makeDeps()
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(post(validBody), env, deps)
    expect(res.status).toBe(200)
    const json = (await res.json()) as Record<string, unknown>
    expect(json.ok).toBe(true)
    expect(json.armed).toBe(true)
    expect(json.outcome).toBe('launched')
    expect(json.credentialId).toBe('agentcred.seam-a.cloud-runtime.1')
    expect(json.placementRef).toBe('placement.cloud-coding.run_gce_1')
    expect(json.tokenRevoked).toBe(false)
    expect(runCalls).toHaveLength(1)
    expect(runCalls[0]).toMatchObject({
      commit: validBody.commit,
      eventCount: 0,
      ownerUserId: validBody.ownerUserId,
      repo: validBody.repo,
      turnId: validBody.turnId,
    })
  })

  test('failed launch echoes ok:false + reason (200, dispatch already settled the turn)', async () => {
    const failedOutcome: CloudGcpDispatchOutcome = {
      credentialId: 'agentcred.seam-a.cloud-runtime.2',
      outcome: 'failed',
      reason: 'cloud_placement_effect_failed',
      tokenRevoked: true,
    }
    const { deps } = makeDeps(
      {},
      { armed: true, configured: true, run: () => Promise.resolve(failedOutcome) },
    )
    const res = await handleCloudGcpRuntimeDispatchAdminRoute(post(validBody), env, deps)
    expect(res.status).toBe(200)
    const json = (await res.json()) as Record<string, unknown>
    expect(json.ok).toBe(false)
    expect(json.outcome).toBe('failed')
    expect(json.tokenRevoked).toBe(true)
    expect(json.reason).toBe('cloud_placement_effect_failed')
  })

  test('passes optional fields (branch, objective, runtimeLane) through', async () => {
    const { deps, runCalls } = makeDeps()
    await handleCloudGcpRuntimeDispatchAdminRoute(
      post({
        ...validBody,
        branch: 'feature/x',
        eventCount: 3,
        objective: 'implement #1234',
        runtimeLane: 'hosted_khala',
      }),
      env,
      deps,
    )
    expect(runCalls[0]).toMatchObject({
      branch: 'feature/x',
      eventCount: 3,
      objective: 'implement #1234',
      runtimeLane: 'hosted_khala',
    })
  })
})
