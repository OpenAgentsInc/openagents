import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import { type Env, handleProgrammaticAgentRegistration } from './index'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly registrations: Array<AgentRegistrationRecord> = []

  createAgentRegistration(record: AgentRegistrationRecord): Promise<void> {
    this.registrations.push(record)

    return Promise.resolve()
  }

  findAgentByTokenHash(
    _tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(undefined)
  }

  touchAgentCredential(
    _credentialId: string,
    _lastUsedAt: string,
  ): Promise<void> {
    return Promise.resolve()
  }
}

describe('programmatic agent registration route', () => {
  test('public registration returns an immediately usable agent token', async () => {
    const store = new MemoryAgentRegistrationStore()
    const response = await handleProgrammaticAgentRegistration(
      new Request('https://openagents.com/api/agents/register', {
        body: JSON.stringify({
          displayName: 'Self Service Forum Agent',
          externalId: 'self-service-forum-agent-1',
          metadata: { purpose: 'forum_posting' },
          slug: 'self-service-forum-agent',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      {} as Env,
      store,
    )
    const body = (await response.json()) as {
      credential: { token: string; tokenPrefix: string }
      user: { status: string }
    }
    const record = store.registrations[0]

    expect(response.status).toBe(201)
    expect(body.user.status).toBe('active')
    expect(body.credential.token).toMatch(/^oa_agent_/)
    expect(record?.credential.status).toBe('active')
    expect(record?.credential.tokenPrefix).toBe(body.credential.tokenPrefix)
    expect(record?.credential.tokenHash).toBe(
      await sha256Hex(body.credential.token),
    )
  })
})
