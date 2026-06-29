import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type OpenAuthAgentLinkRecord,
  createProgrammaticAgentRegistration,
  sha256Hex,
} from './agent-registration'
import type { RouteEffect } from './http/route-effects'
import { makePylonOpenAgentsAuthHandlers } from './pylon-openagents-auth-routes'

class MemoryAgentStore implements AgentRegistrationStore {
  readonly registrations: Array<AgentRegistrationRecord> = []
  readonly links: Array<OpenAuthAgentLinkRecord> = []

  createAgentRegistration(record: AgentRegistrationRecord): Promise<void> {
    this.registrations.push(record)
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
  ): Promise<AgentCredentialLookup | undefined> {
    const registration = this.registrations.find(
      record => record.credential.tokenHash === tokenHash,
    )
    if (registration === undefined) {
      return Promise.resolve(undefined)
    }
    return Promise.resolve({
      credentialId: registration.credential.id,
      openauthUserId: registration.credential.openauthUserId,
      profileMetadataJson: registration.profile.metadataJson,
      tokenPrefix: registration.credential.tokenPrefix,
      user: registration.user,
    })
  }

  touchAgentCredential(): Promise<void> {
    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(1)
  }

  linkOpenAuthAgent(record: OpenAuthAgentLinkRecord): Promise<void> {
    this.links.push(record)
    const index = this.registrations.findIndex(
      registration =>
        registration.user.id === record.agentUserId &&
        registration.credential.id === record.agentCredentialId,
    )
    if (index !== -1) {
      const registration = this.registrations[index]
      if (registration !== undefined) {
        this.registrations[index] = {
          ...registration,
          credential: {
            ...registration.credential,
            openauthUserId: record.openauthUserId,
          },
        }
      }
    }
    return Promise.resolve()
  }
}

class MemoryKv {
  readonly values = new Map<string, string>()

  get(key: string, type?: 'json') {
    const value = this.values.get(key)
    if (value === undefined) {
      return Promise.resolve(null)
    }
    return Promise.resolve(type === 'json' ? JSON.parse(value) : value)
  }

  put(key: string, value: string) {
    this.values.set(key, value)
    return Promise.resolve()
  }
}

const ctx = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as unknown as ExecutionContext

const runRoute = (effect: RouteEffect): Promise<Response> =>
  Effect.runPromise(effect)

describe('Pylon OpenAgents auth device-link routes', () => {
  test('starts, verifies, and polls an OpenAuth agent link without exposing the token', async () => {
    const store = new MemoryAgentStore()
    const registration = await createProgrammaticAgentRegistration(store, {
      displayName: 'Pylon CLI',
    })
    const kv = new MemoryKv()
    const handlers = makePylonOpenAgentsAuthHandlers({
      agentStore: () => store,
      appendRefreshedSessionCookies: response => response,
      makeId: () => 'attempt-1',
      nowIso: () => '2026-06-25T12:00:00.000Z',
      requireBrowserSession: () =>
        Promise.resolve({ user: { userId: 'openauth_user_1' } }),
    })
    const env = { AUTH_STORAGE: kv as unknown as KVNamespace }

    const start = await runRoute(
      handlers.handlePylonOpenAgentsAuthStartApi(
        new Request(
          'https://openagents.com/api/pylon/auth/openagents/device/start',
          {
            headers: {
              authorization: `Bearer ${registration.credential.token}`,
            },
            method: 'POST',
          },
        ),
        env,
      ),
    )
    const startBody = (await start.json()) as {
      attemptId: string
      status: string
      userCode: string
      verificationUrl: string
    }

    expect(start.status).toBe(201)
    expect(startBody).toMatchObject({
      attemptId: 'pylon_openauth_attempt-1',
      status: 'pending',
      // Derived from the attempt id's random suffix ("attempt-1"), NOT the
      // constant "pylon_openauth_" prefix (which used to yield "PYLO-NOPE").
      userCode: 'ATTE-MPT1',
    })
    expect(startBody.verificationUrl).toContain(
      '/api/pylon/auth/openagents/device/verify',
    )
    expect(JSON.stringify(startBody)).not.toContain(
      registration.credential.token,
    )

    const stored = [...kv.values.values()].join('\n')
    expect(stored).not.toContain(registration.credential.token)
    expect(stored).toContain(await sha256Hex(registration.credential.token))

    const verify = await runRoute(
      handlers.handlePylonOpenAgentsAuthVerifyApi(
        new Request(startBody.verificationUrl, { method: 'GET' }),
        env,
        ctx,
      ),
    )
    expect(verify.status).toBe(200)
    expect(await verify.text()).toContain('Pylon connected')
    expect(store.links).toHaveLength(1)
    expect(store.links[0]?.openauthUserId).toBe('openauth_user_1')

    const status = await runRoute(
      handlers.handlePylonOpenAgentsAuthStatusApi(
        new Request(
          `https://openagents.com/api/pylon/auth/openagents/device/${startBody.attemptId}`,
          {
            headers: {
              authorization: `Bearer ${registration.credential.token}`,
            },
            method: 'GET',
          },
        ),
        env,
        startBody.attemptId,
      ),
    )
    const statusBody = (await status.json()) as { status: string }

    expect(status.status).toBe(200)
    expect(statusBody.status).toBe('linked')
  })

  test('redirects unauthenticated browser verification through GitHub login', async () => {
    const store = new MemoryAgentStore()
    const registration = await createProgrammaticAgentRegistration(store, {
      displayName: 'Pylon CLI',
    })
    const kv = new MemoryKv()
    const handlers = makePylonOpenAgentsAuthHandlers({
      agentStore: () => store,
      appendRefreshedSessionCookies: response => response,
      makeId: () => 'attempt-1',
      nowIso: () => '2026-06-25T12:00:00.000Z',
      requireBrowserSession: () => Promise.resolve(undefined),
    })
    const env = { AUTH_STORAGE: kv as unknown as KVNamespace }

    const start = await runRoute(
      handlers.handlePylonOpenAgentsAuthStartApi(
        new Request(
          'https://openagents.com/api/pylon/auth/openagents/device/start',
          {
            headers: {
              authorization: `Bearer ${registration.credential.token}`,
            },
            method: 'POST',
          },
        ),
        env,
      ),
    )
    const startBody = (await start.json()) as { verificationUrl: string }
    const verify = await runRoute(
      handlers.handlePylonOpenAgentsAuthVerifyApi(
        new Request(startBody.verificationUrl, { method: 'GET' }),
        env,
        ctx,
      ),
    )

    expect(verify.status).toBe(302)
    expect(verify.headers.get('location')).toContain('/login/github?returnTo=')
  })
})
