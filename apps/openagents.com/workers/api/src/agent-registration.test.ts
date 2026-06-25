import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type OpenAuthAgentLinkRecord,
  authenticateProgrammaticAgent,
  createProgrammaticAgentRegistration,
} from './agent-registration'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly registrations: Array<AgentRegistrationRecord> = []
  readonly links: Array<OpenAuthAgentLinkRecord> = []
  readonly lookups = new Map<string, AgentCredentialLookup>()
  readonly touches: Array<
    Readonly<{ credentialId: string; lastUsedAt: string }>
  > = []

  createAgentRegistration(record: AgentRegistrationRecord): Promise<void> {
    this.registrations.push(record)
    this.lookups.set(record.credential.tokenHash, {
      user: record.user,
      credentialId: record.credential.id,
      openauthUserId: record.credential.openauthUserId,
      profileMetadataJson: record.profile.metadataJson,
      tokenPrefix: record.credential.tokenPrefix,
    })

    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(this.lookups.get(tokenHash))
  }

  touchAgentCredential(
    credentialId: string,
    lastUsedAt: string,
  ): Promise<void> {
    this.touches.push({ credentialId, lastUsedAt })

    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }

  async linkOpenAuthAgent(record: OpenAuthAgentLinkRecord): Promise<void> {
    this.links.push(record)
    const registration = this.registrations.find(
      item =>
        item.user.id === record.agentUserId &&
        item.credential.id === record.agentCredentialId,
    )
    if (registration !== undefined) {
      this.lookups.set(registration.credential.tokenHash, {
        credentialId: registration.credential.id,
        openauthUserId: record.openauthUserId,
        profileMetadataJson: registration.profile.metadataJson,
        tokenPrefix: registration.credential.tokenPrefix,
        user: registration.user,
      })
    }
  }

  async listLinkedAgentsForOpenAuthUser(
    openauthUserId: string,
    _limit: number,
  ) {
    return this.links
      .filter(
        link =>
          link.openauthUserId === openauthUserId &&
          link.status === 'active' &&
          link.revokedAt === null,
      )
      .map(link => {
        const registration = this.registrations.find(
          item => item.user.id === link.agentUserId,
        )
        return {
          agentUserId: link.agentUserId,
          credentialId: link.agentCredentialId,
          displayName: registration?.user.displayName ?? link.agentUserId,
          linkKind: link.linkKind,
          openauthUserId: link.openauthUserId,
          tokenPrefix: registration?.credential.tokenPrefix ?? null,
        }
      })
  }
}

const makeUuidFactory = (values: ReadonlyArray<string>) => {
  const queue = Array.from(values)

  return (): string => {
    const value = queue.shift()

    if (value === undefined) {
      throw new Error('uuid factory exhausted')
    }

    return value
  }
}

describe('programmatic agent registration', () => {
  test('creates an agent user record and a hashed credential', async () => {
    const store = new MemoryAgentRegistrationStore()
    const createdAt = '2026-06-02T17:10:00.000Z'
    const token = 'oa_agent_test_secret'
    const registration = await createProgrammaticAgentRegistration(
      store,
      {
        displayName: 'SHC Runner',
        slug: 'shc-runner',
        externalId: 'shc-runner-1',
        metadata: { runtime: 'opencode' },
      },
      {
        now: () => createdAt,
        makeUuid: makeUuidFactory(['user-1', 'credential-1', 'identity-1']),
        makeToken: () => token,
      },
    )
    const record = store.registrations[0]

    expect(record?.user).toEqual({
      id: 'user_user-1',
      kind: 'agent',
      displayName: 'SHC Runner',
      primaryEmail: null,
      avatarUrl: null,
      status: 'active',
      createdAt,
      updatedAt: createdAt,
    })
    expect(record?.identity).toMatchObject({
      id: 'auth_identity_identity-1',
      provider: 'agent_programmatic',
      providerSubject: 'shc-runner-1',
      userId: 'user_user-1',
    })
    expect(record?.profile).toMatchObject({
      userId: 'user_user-1',
      slug: 'shc-runner',
      metadataJson: '{"runtime":"opencode"}',
    })
    expect(record?.credential.id).toBe('agent_credential_credential-1')
    expect(record?.credential.expiresAt).toBeNull()
    expect(record?.credential.tokenHash).toMatch(/^[a-f0-9]{64}$/)
    expect(record?.credential.tokenHash).not.toContain('test_secret')
    expect(registration.credential.token).toBe(token)
    expect(registration.credential.expiresAt).toBeNull()
  })

  test('authenticates the returned credential against the stored hash', async () => {
    const store = new MemoryAgentRegistrationStore()
    const createdAt = '2026-06-02T17:10:00.000Z'
    const lastUsedAt = '2026-06-02T17:11:00.000Z'
    const registration = await createProgrammaticAgentRegistration(
      store,
      { displayName: 'Deploy Agent' },
      {
        now: () => createdAt,
        makeUuid: makeUuidFactory(['user-2', 'credential-2', 'identity-2']),
        makeToken: () => 'oa_agent_deploy_secret',
      },
    )
    const session = await authenticateProgrammaticAgent(
      store,
      registration.credential.token,
      () => lastUsedAt,
    )

    expect(session?.user.id).toBe(registration.user.id)
    expect(session?.credential).toEqual({
      id: registration.credential.id,
      openauthUserId: null,
      profileMetadataJson: '{}',
      tokenPrefix: registration.credential.tokenPrefix,
      lastUsedAt,
    })
    expect(store.touches).toEqual([
      {
        credentialId: registration.credential.id,
        lastUsedAt,
      },
    ])
  })

  test('links one OpenAuth account to many owned agent credentials', async () => {
    const store = new MemoryAgentRegistrationStore()
    const first = await createProgrammaticAgentRegistration(
      store,
      { displayName: 'Owner Codex Pylon' },
      {
        makeToken: () => 'oa_agent_owner_1',
        makeUuid: makeUuidFactory(['user-4', 'credential-4', 'identity-4']),
        now: () => '2026-06-02T17:00:00.000Z',
      },
    )
    const second = await createProgrammaticAgentRegistration(
      store,
      { displayName: 'Other Owner Pylon' },
      {
        makeToken: () => 'oa_agent_owner_2',
        makeUuid: makeUuidFactory(['user-5', 'credential-5', 'identity-5']),
        now: () => '2026-06-02T17:00:00.000Z',
      },
    )

    await store.linkOpenAuthAgent({
      agentCredentialId: first.credential.id,
      agentUserId: first.user.id,
      createdAt: '2026-06-02T17:01:00.000Z',
      id: 'openauth_agent_link_1',
      linkKind: 'credential_anchor',
      openauthUserId: 'user_human_1',
      revokedAt: null,
      status: 'active',
      updatedAt: '2026-06-02T17:01:00.000Z',
    })
    await store.linkOpenAuthAgent({
      agentCredentialId: second.credential.id,
      agentUserId: second.user.id,
      createdAt: '2026-06-02T17:01:00.000Z',
      id: 'openauth_agent_link_2',
      linkKind: 'credential_anchor',
      openauthUserId: 'user_human_2',
      revokedAt: null,
      status: 'active',
      updatedAt: '2026-06-02T17:01:00.000Z',
    })

    await expect(
      store.listLinkedAgentsForOpenAuthUser('user_human_1', 100),
    ).resolves.toMatchObject([
      {
        agentUserId: first.user.id,
        displayName: 'Owner Codex Pylon',
        openauthUserId: 'user_human_1',
      },
    ])
  })

  test('does not authenticate expired credentials', async () => {
    class ExpiringMemoryAgentRegistrationStore extends MemoryAgentRegistrationStore {
      override findAgentByTokenHash(
        tokenHash: string,
        now: string,
      ): Promise<AgentCredentialLookup | undefined> {
        const registration = this.registrations.find(
          record => record.credential.tokenHash === tokenHash,
        )

        if (
          registration?.credential.expiresAt !== null &&
          registration?.credential.expiresAt !== undefined &&
          registration.credential.expiresAt <= now
        ) {
          return Promise.resolve(undefined)
        }

        return super.findAgentByTokenHash(tokenHash, now)
      }
    }

    const store = new ExpiringMemoryAgentRegistrationStore()
    const registration = await createProgrammaticAgentRegistration(
      store,
      { displayName: 'Expiring Agent' },
      {
        expiresAt: '2026-06-02T17:10:00.000Z',
        makeToken: () => 'oa_agent_expired_secret',
        makeUuid: makeUuidFactory(['user-3', 'credential-3', 'identity-3']),
        now: () => '2026-06-02T17:00:00.000Z',
      },
    )
    const session = await authenticateProgrammaticAgent(
      store,
      registration.credential.token,
      () => '2026-06-02T17:11:00.000Z',
    )

    expect(session).toBeUndefined()
    expect(store.touches).toEqual([])
  })
})
