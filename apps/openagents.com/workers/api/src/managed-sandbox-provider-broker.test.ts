import type {
  ManagedSandboxResource,
  ManagedSandboxTurn,
} from '@openagentsinc/managed-sandbox-contract'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { OpenAgentsWorkerEnv } from './bindings'
import type { BoxV1NativeStore } from './managed-sandbox-box-v1-routes'
import {
  makeManagedSandboxProviderBrokerRoutes,
  managedSandboxProviderBrokerPaths,
  mintManagedSandboxProviderCapability,
} from './managed-sandbox-provider-broker'

const nowMs = Date.parse('2026-07-19T20:00:00.000Z')
const ownerRef = 'owner.sbx09.live'
const tenantRef = 'tenant.sbx09.live'
const sandboxRef = 'sandbox.sbx09.live'
const turnRef = 'turn.sbx09.live.codex'
const capabilityRef = 'capability.sbx09.live.turn'

const resource = (state: 'active' | 'revoked' = 'active') =>
  ({
    schema: 'openagents.managed_sandbox.v1',
    sandboxRef,
    ownerRef,
    tenantRef,
    programRef: 'program.managed_agent_sandboxes',
    workUnitRef: 'work.sbx09.live',
    attachmentRef: 'attachment.sbx09.live',
    attachmentGeneration: 1,
    resourceGeneration: 1,
    version: 3,
    lastEventSequence: 2,
    target: {
      targetRef: 'target.openagents.google-cloud.managed-sandbox',
      targetClass: 'openagents_managed',
      provider: 'google_cloud',
      adapterRef: 'adapter.oa-codex-control.gce.v1',
      region: 'us-central1',
      isolation: 'gce_vm',
      dataPosture: 'openagents_managed_region',
    },
    imageDigest: `sha256:${'a'.repeat(64)}`,
    profileRef: 'profile.managed-sandbox.gce-e2-small-v1',
    lease: {
      leaseRef: 'lease.sbx09.live',
      state: 'active',
      issuedAt: '2026-07-19T19:55:00.000Z',
      expiresAt: '2026-07-19T20:10:00.000Z',
      ttlSeconds: 900,
      renewable: true,
    },
    budget: {
      currency: 'USD',
      maxCostMicros: 10_000,
      maxCpuMillis: 1_800_000,
      maxNetworkBytes: 20_000_000,
      maxArtifactBytes: 10_000_000,
      maxLifetimeSeconds: 900,
    },
    capabilities: [
      {
        capabilityRef,
        kind: 'agent_turn',
        state,
        expiresAt: '2026-07-19T20:10:00.000Z',
      },
    ],
    facts: {
      lifecycle: 'ready',
      leaseState: 'active',
      guestState: 'present',
      filesystemState: 'attached',
      ingressState: 'closed',
      runtimeState: 'running',
      acceptingWork: true,
      cleanupComplete: false,
    },
    createdAt: '2026-07-19T19:55:00.000Z',
    updatedAt: '2026-07-19T20:00:00.000Z',
  }) as ManagedSandboxResource

const turnFor = (provider: 'codex' | 'claude' = 'codex') =>
  ({
    schema: 'openagents.managed_sandbox_turn.v1',
    turnRef,
    sandboxRef,
    ownerRef,
    tenantRef,
    workUnitRef: 'work.sbx09.live',
    attachmentRef: 'attachment.sbx09.live',
    attachmentGeneration: 1,
    resourceGeneration: 1,
    turnSequence: 1,
    lastEventSequence: 1,
    commandRef: 'command.sbx09.live.codex',
    capabilityRef,
    promptDigest: `sha256:${'b'.repeat(64)}`,
    runtime: {
      provider,
      modelRef: `model.${provider}.default`,
      harnessRef:
        provider === 'codex'
          ? 'harness.openai.codex-sdk.v1'
          : 'harness.anthropic.claude-agent-sdk.v1',
    },
    status: 'running',
    createdAt: '2026-07-19T20:00:00.000Z',
    startedAt: '2026-07-19T20:00:00.000Z',
  }) as ManagedSandboxTurn

const env = {
  OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY: 's'.repeat(48),
  OA_MANAGED_SANDBOX_CODEX_MODEL: 'gpt-5.6',
  OA_MANAGED_SANDBOX_CLAUDE_MODEL: 'claude-sonnet-4-6',
  OA_MANAGED_SANDBOX_CLAUDE_LOCATION: 'us-east5',
  VERTEX_PROJECT_ID: 'openagentsgemini',
  OPENAI_API_KEY: 'provider-secret-never-returned',
} as unknown as OpenAgentsWorkerEnv

const store = (
  getResource: () => ManagedSandboxResource,
  provider: 'codex' | 'claude' = 'codex',
) =>
  ({
    inspect: () => Effect.succeed(getResource()),
    inspectTurn: () => Effect.succeed({ turn: turnFor(provider) }),
  }) as unknown as BoxV1NativeStore

const token = (provider: 'codex' | 'claude' = 'codex') =>
  Effect.runPromise(
    mintManagedSandboxProviderCapability(env, {
      actorRef: 'principal.sbx09.live',
      ownerRef,
      tenantRef,
      sandboxRef,
      turnRef,
      resourceGeneration: 1,
      capabilityRef,
      capabilityExpiresAt: '2026-07-19T20:10:00.000Z',
      provider,
      requestedModelRef: `model.${provider}.default`,
      nowMs,
    }),
  )

describe('managed-sandbox provider capability broker', () => {
  test('redeems one exact live generation without disclosing provider credentials', async () => {
    let upstreamBody: Record<string, unknown> | undefined
    let upstreamAuthorization: string | null = null
    const routes = makeManagedSandboxProviderBrokerRoutes({
      store: () => store(() => resource()),
      nowMs: () => nowMs + 1,
      fetchImpl: async (_url, init) => {
        upstreamBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        upstreamAuthorization = new Headers(init?.headers).get('authorization')
        return new Response('data: {"type":"response.completed"}\n\n', {
          status: 200,
          headers: {
            'content-type': 'text/event-stream',
            'x-request-id': 'provider-request-public-ref',
          },
        })
      },
    })
    const capability = await token()
    const effect = routes.route(
      new Request(
        `https://openagents.test${managedSandboxProviderBrokerPaths.openai}`,
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${capability}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            model: 'caller-cannot-change-model',
            input: 'Return the word READY.',
            stream: true,
          }),
        },
      ),
      env,
    )
    expect(effect).toBeDefined()
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')
    expect(await response.text()).toContain('response.completed')
    expect(upstreamBody?.['model']).toBe('gpt-5.6')
    expect(upstreamAuthorization).toBe('Bearer provider-secret-never-returned')
    expect(JSON.stringify(upstreamBody)).not.toContain(capability)
  })

  test('refuses the same signed token after capability revocation', async () => {
    const routes = makeManagedSandboxProviderBrokerRoutes({
      store: () => store(() => resource('revoked')),
      nowMs: () => nowMs + 1,
      fetchImpl: async () => {
        throw new Error('provider must not be called after revoke')
      },
    })
    const effect = routes.route(
      new Request(
        `https://openagents.test${managedSandboxProviderBrokerPaths.openai}`,
        {
          method: 'POST',
          headers: { authorization: `Bearer ${await token()}` },
          body: JSON.stringify({ input: 'forbidden' }),
        },
      ),
      env,
    )
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(403)
    expect(await response.json()).toEqual({ error: 'capability_revoked' })
  })

  test('maps a Claude Agent SDK request to the exact Vertex Anthropic model', async () => {
    let upstreamUrl = ''
    let upstreamBody: Record<string, unknown> | undefined
    let upstreamAuthorization: string | null = null
    const routes = makeManagedSandboxProviderBrokerRoutes({
      store: () => store(() => resource(), 'claude'),
      nowMs: () => nowMs + 1,
      vertexAccessToken: 'vertex-access-token-never-returned',
      fetchImpl: async (url, init) => {
        upstreamUrl = String(url)
        upstreamBody = JSON.parse(String(init?.body)) as Record<string, unknown>
        upstreamAuthorization = new Headers(init?.headers).get('authorization')
        return Response.json({
          type: 'message',
          content: [{ type: 'text', text: 'READY' }],
        })
      },
    })
    const capability = await token('claude')
    const effect = routes.route(
      new Request(
        `https://openagents.test${managedSandboxProviderBrokerPaths.anthropic}`,
        {
          method: 'POST',
          headers: { authorization: `Bearer ${capability}` },
          body: JSON.stringify({
            model: 'caller-cannot-change-model',
            max_tokens: 64,
            messages: [{ role: 'user', content: 'Return READY.' }],
          }),
        },
      ),
      env,
    )
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(200)
    expect(upstreamUrl).toContain(
      '/projects/openagentsgemini/locations/us-east5/publishers/anthropic/models/' +
        'claude-sonnet-4-6:rawPredict',
    )
    expect(upstreamBody?.['model']).toBeUndefined()
    expect(upstreamBody?.['anthropic_version']).toBe('vertex-2023-10-16')
    expect(upstreamAuthorization).toBe(
      'Bearer vertex-access-token-never-returned',
    )
  })

  test('rejects an expired signed capability before store or provider access', async () => {
    const capability = await token()
    const routes = makeManagedSandboxProviderBrokerRoutes({
      store: () => {
        throw new Error('expired token must not reach the store')
      },
      nowMs: () => nowMs + 15 * 60 * 1_000 + 1,
      fetchImpl: async () => {
        throw new Error('expired token must not reach the provider')
      },
    })
    const response = await Effect.runPromise(
      routes.route(
        new Request(
          `https://openagents.test${managedSandboxProviderBrokerPaths.openai}`,
          {
            method: 'POST',
            headers: { authorization: `Bearer ${capability}` },
            body: JSON.stringify({ input: 'forbidden' }),
          },
        ),
        env,
      )!,
    )
    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({ error: 'unauthorized' })
  })
})
