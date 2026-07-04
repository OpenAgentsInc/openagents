import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type OpenAuthAgentLinkRecord,
} from './agent-registration'
import type { RouteEffect } from './http/route-effects'
import { makeKhalaCodeOpenAgentsAuthHandlers } from './khala-code-openagents-auth-routes'

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

describe('Khala Code OpenAgents desktop auth device routes', () => {
  // Oracle for khala_code.chat.khala_lane_connect_button.v1
  test('starts, verifies, and polls a signed-in desktop token without exposing it before poll', async () => {
    const store = new MemoryAgentStore()
    const kv = new MemoryKv()
    const ids = [
      'attempt-1',
      'poll-1',
      'user-1',
      'identity-1',
      'credential-1',
      'link-1',
    ]
    const handlers = makeKhalaCodeOpenAgentsAuthHandlers({
      agentStore: () => store,
      appendRefreshedSessionCookies: response => response,
      makeId: () => ids.shift() ?? 'fallback',
      nowIso: () => '2026-07-04T12:00:00.000Z',
      requireBrowserSession: () =>
        Promise.resolve({ user: { userId: 'openauth_user_1' } }),
    })
    const env = { AUTH_STORAGE: kv as unknown as KVNamespace }

    const start = await runRoute(
      handlers.handleKhalaCodeOpenAgentsAuthStartApi(
        new Request(
          'https://openagents.com/api/khala-code/auth/openagents/device/start',
          { method: 'POST' },
        ),
        env,
      ),
    )
    const startBody = (await start.json()) as {
      attemptId: string
      pollSecret: string
      status: string
      userCode: string
      verificationUrl: string
    }

    expect(start.status).toBe(201)
    expect(startBody).toMatchObject({
      attemptId: 'khala_code_desktop_openauth_attempt-1',
      status: 'pending',
      userCode: 'ATTE-MPT1',
    })
    expect(startBody.pollSecret).toBe('khala_code_desktop_poll_poll-1')
    expect(startBody.verificationUrl).toContain(
      '/api/khala-code/auth/openagents/device/verify',
    )
    expect(JSON.stringify(startBody)).not.toContain('oa_agent_')

    const pendingStored = [...kv.values.values()].join('\n')
    expect(pendingStored).not.toContain(startBody.pollSecret)
    expect(pendingStored).not.toContain('oa_agent_')

    const verify = await runRoute(
      handlers.handleKhalaCodeOpenAgentsAuthVerifyApi(
        new Request(startBody.verificationUrl, { method: 'GET' }),
        env,
        ctx,
      ),
    )
    expect(verify.status).toBe(200)
    expect(await verify.text()).toContain('Khala Code connected')
    expect(store.registrations[0]?.user.displayName).toBe('Khala Code Desktop')
    expect(store.links).toHaveLength(1)
    expect(store.links[0]?.openauthUserId).toBe('openauth_user_1')

    const unauthorizedPoll = await runRoute(
      handlers.handleKhalaCodeOpenAgentsAuthStatusApi(
        new Request(
          `https://openagents.com/api/khala-code/auth/openagents/device/${startBody.attemptId}`,
          {
            headers: {
              'x-openagents-device-secret': 'khala_code_desktop_poll_wrong',
            },
            method: 'GET',
          },
        ),
        env,
        startBody.attemptId,
      ),
    )
    expect(unauthorizedPoll.status).toBe(404)

    const poll = await runRoute(
      handlers.handleKhalaCodeOpenAgentsAuthStatusApi(
        new Request(
          `https://openagents.com/api/khala-code/auth/openagents/device/${startBody.attemptId}`,
          {
            headers: {
              'x-openagents-device-secret': startBody.pollSecret,
            },
            method: 'GET',
          },
        ),
        env,
        startBody.attemptId,
      ),
    )
    const pollBody = (await poll.json()) as {
      agentToken: string
      linkedAgent: { tokenPrefix: string }
      status: string
    }

    expect(poll.status).toBe(200)
    expect(pollBody.status).toBe('linked')
    expect(pollBody.agentToken).toMatch(/^oa_agent_/)
    expect(pollBody.linkedAgent.tokenPrefix).toBe(pollBody.agentToken.slice(0, 20))
  })

  test('redirects unauthenticated browser verification through GitHub login', async () => {
    const store = new MemoryAgentStore()
    const kv = new MemoryKv()
    const handlers = makeKhalaCodeOpenAgentsAuthHandlers({
      agentStore: () => store,
      appendRefreshedSessionCookies: response => response,
      makeId: () => 'attempt-1',
      nowIso: () => '2026-07-04T12:00:00.000Z',
      requireBrowserSession: () => Promise.resolve(undefined),
    })
    const env = { AUTH_STORAGE: kv as unknown as KVNamespace }

    const start = await runRoute(
      handlers.handleKhalaCodeOpenAgentsAuthStartApi(
        new Request(
          'https://openagents.com/api/khala-code/auth/openagents/device/start',
          { method: 'POST' },
        ),
        env,
      ),
    )
    const startBody = (await start.json()) as { verificationUrl: string }
    const verify = await runRoute(
      handlers.handleKhalaCodeOpenAgentsAuthVerifyApi(
        new Request(startBody.verificationUrl, { method: 'GET' }),
        env,
        ctx,
      ),
    )

    expect(verify.status).toBe(302)
    expect(verify.headers.get('location')).toContain('/login/github?returnTo=')
  })
})
