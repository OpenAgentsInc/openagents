import { describe, expect, test } from 'vitest'

import type {
  AgentRegistrationStore,
  ProgrammaticAgentSession,
} from './agent-registration'
import { sha256Hex } from './agent-registration'
import { makeProviderAccountPylonHandlers } from './provider-account-pylon-routes'
import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountEventRecord,
  ProviderAccountRecord,
  ProviderAccountRepository,
  ProviderConnectionAttemptRecord,
  StartedCodexDeviceLoginSecret,
} from './provider-accounts'

class MemoryProviderAccountRepository implements ProviderAccountRepository {
  accounts: Array<ProviderAccountRecord> = []
  attempts: Array<ProviderConnectionAttemptRecord> = []
  events: Array<ProviderAccountEventRecord> = []
  grants: Array<ProviderAccountAuthGrantRecord> = []

  findAccountByRef = (
    userId: string,
    providerAccountRef: string,
  ): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )

  findAccountByProviderAccountRef = (
    providerAccountRef: string,
  ): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(
      this.accounts.find(
        account =>
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )

  findReusableAccount = (
    userId: string,
  ): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.status !== 'connected' &&
          account.deletedAt === null,
      ),
    )

  listAccountsForUser = (
    userId: string,
  ): Promise<ReadonlyArray<ProviderAccountRecord>> =>
    Promise.resolve(this.accounts.filter(account => account.userId === userId))

  listPendingAttemptsForUser = (
    userId: string,
  ): Promise<ReadonlyArray<ProviderConnectionAttemptRecord>> =>
    Promise.resolve(
      this.attempts.filter(
        attempt => attempt.userId === userId && attempt.status === 'pending',
      ),
    )

  findAttemptForUser = (
    userId: string,
    attemptId: string,
  ): Promise<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined
  > =>
    Promise.resolve(
      this.recordForAttempt(
        this.attempts.find(
          attempt => attempt.userId === userId && attempt.id === attemptId,
        ),
      ),
    )

  findAttemptById = (attemptId: string) =>
    Promise.resolve(
      this.recordForAttempt(
        this.attempts.find(attempt => attempt.id === attemptId),
      ),
    )

  saveStartedDeviceLogin = (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
    accountAlreadyExists: boolean,
  ): Promise<void> => {
    if (accountAlreadyExists) {
      this.accounts = this.accounts.map(candidate =>
        candidate.id === account.id ? account : candidate,
      )
    } else {
      this.accounts.push(account)
    }

    this.attempts.push(attempt)
    this.events.push(event)
    return Promise.resolve()
  }

  recordConnectedAttempt = (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord> => {
    this.accounts = this.accounts.map(candidate =>
      candidate.id === account.id ? account : candidate,
    )
    this.attempts = this.attempts.map(candidate =>
      candidate.id === attempt.id ? attempt : candidate,
    )
    this.events.push(event)
    return Promise.resolve(account)
  }

  recordFailedAttempt = (
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord> => {
    this.accounts = this.accounts.map(candidate =>
      candidate.id === account.id ? account : candidate,
    )
    this.attempts = this.attempts.map(candidate =>
      candidate.id === attempt.id ? attempt : candidate,
    )
    this.events.push(event)
    return Promise.resolve(account)
  }

  recordAccountHealth = (
    providerAccountRef: string,
    account: ProviderAccountRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord | undefined> => {
    const existing = this.accounts.find(
      candidate => candidate.providerAccountRef === providerAccountRef,
    )
    if (existing === undefined) return Promise.resolve(undefined)
    this.accounts = this.accounts.map(candidate =>
      candidate.providerAccountRef === providerAccountRef ? account : candidate,
    )
    this.events.push(event)
    return Promise.resolve(account)
  }

  createAuthGrant = (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountAuthGrantRecord> => {
    this.grants.push(grant)
    this.events.push(event)
    return Promise.resolve(grant)
  }

  findGrantByRef = (
    grantRef: string,
  ): Promise<ProviderAccountAuthGrantRecord | undefined> =>
    Promise.resolve(this.grants.find(grant => grant.grantRef === grantRef))

  markGrantUsed = (
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountAuthGrantRecord> => {
    this.grants = this.grants.map(candidate =>
      candidate.id === grant.id ? grant : candidate,
    )
    this.events.push(event)
    return Promise.resolve(grant)
  }

  disconnectAccount = (): Promise<ProviderAccountRecord | undefined> =>
    Promise.resolve(undefined)

  private recordForAttempt = (
    attempt: ProviderConnectionAttemptRecord | undefined,
  ):
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined => {
    if (attempt === undefined) return undefined
    const account = this.accounts.find(
      candidate => candidate.id === attempt.providerAccountId,
    )
    return account === undefined ? undefined : { account, attempt }
  }
}

const env = (): { AUTH_STORAGE: KVNamespace; OPENAGENTS_DB: D1Database } =>
  ({
    AUTH_STORAGE: {} as KVNamespace,
    OPENAGENTS_DB: {} as D1Database,
  }) as { AUTH_STORAGE: KVNamespace; OPENAGENTS_DB: D1Database }

const linkedSession = (
  tokenHash: string,
  openauthUserId: string | null,
): ProgrammaticAgentSession => ({
  credential: {
    id: 'credential-pylon',
    lastUsedAt: '2026-06-25T12:00:00.000Z',
    openauthUserId,
    profileMetadataJson: '{}',
    tokenPrefix: 'oa_agent_test',
  },
  user: {
    avatarUrl: null,
    createdAt: '2026-06-25T12:00:00.000Z',
    displayName: 'Pylon Agent',
    id: `agent-${tokenHash.slice(0, 8)}`,
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-25T12:00:00.000Z',
  },
})

const agentStoreFor = (
  token: string,
  openauthUserId: string | null,
): AgentRegistrationStore => {
  let expectedHash = ''
  return {
    createAgentRegistration: () => Promise.resolve(),
    findAgentByTokenHash: async tokenHash => {
      expectedHash = expectedHash === '' ? await sha256Hex(token) : expectedHash
      if (tokenHash !== expectedHash) return undefined
      const session = linkedSession(tokenHash, openauthUserId)
      return {
        credentialId: session.credential.id,
        openauthUserId,
        profileMetadataJson: session.credential.profileMetadataJson,
        tokenPrefix: session.credential.tokenPrefix,
        user: session.user,
      }
    },
    linkOpenAuthAgent: () => Promise.resolve(),
    listLinkedAgentsForOpenAuthUser: () => Promise.resolve([]),
    touchAgentCredential: () => Promise.resolve(),
    updateAgentDisplayName: () => Promise.resolve(0),
  }
}

describe('provider account Pylon device-login routes', () => {
  test('starts and completes Codex device login under the linked OpenAuth owner', async () => {
    const token = 'oa_agent_test_token'
    const repository = new MemoryProviderAccountRepository()
    const started = new Map<string, StartedCodexDeviceLoginSecret>()
    const connectedAuth: Array<unknown> = []
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => attemptId => {
        started.delete(attemptId)
        return Promise.resolve()
      },
      makeProviderAccountRepository: () => repository,
      nowIso: () => '2026-06-25T12:00:00.000Z',
      pollDeviceLogin: () =>
        Promise.resolve({
          status: 'connected',
          accountLabel: 'Codex Work',
          planType: 'pro',
          auth: {
            type: 'oauth',
            access: 'access-secret',
            refresh: 'refresh-secret',
            expires: 1_800_000_000,
          },
        }),
      readStartedCodexDeviceLogin: () => attemptId =>
        Promise.resolve(started.get(attemptId)),
      startDeviceLogin: () =>
        Promise.resolve({
          deviceAuthId: 'device-auth-id',
          expiresAt: '2099-06-25T12:10:00.000Z',
          intervalSeconds: 5,
          userCode: 'ABCD-EFGH',
          verificationUrl: 'https://auth.openai.com/device',
        }),
      storeConnectedCodexAuth: () => input => {
        connectedAuth.push(input)
        return Promise.resolve(`codex-auth://${input.providerAccountRef}`)
      },
      storeStartedCodexDeviceLogin: () => input => {
        started.set(input.attemptId, {
          deviceAuthId: input.deviceAuthId,
          userCode: input.userCode,
        })
        return Promise.resolve()
      },
    })

    const startResponse = await handlers.handlePylonProviderDeviceLoginStartApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/chatgpt-codex/device-login/start',
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${token}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({ accountLabel: 'Codex Work', createNew: true }),
        },
      ),
      env(),
    )
    const startBody = (await startResponse.json()) as {
      attempt: { id: string; status: string }
      pylonLink: { owner: string; status: string }
      userCode: string
    }

    expect(startResponse.status).toBe(201)
    expect(startBody.userCode).toBe('ABCD-EFGH')
    expect(startBody.pylonLink).toEqual({ owner: 'openauth', status: 'linked' })
    expect(repository.accounts[0]?.userId).toBe('openauth-user-owner')
    expect(repository.accounts[0]?.status).toBe('pending')
    expect(JSON.stringify(startBody)).not.toContain('access-secret')
    expect(JSON.stringify(startBody)).not.toContain('refresh-secret')

    const pollResponse = await handlers.handlePylonProviderDeviceLoginStatusApi(
      new Request(
        `https://openagents.com/api/pylon/provider-accounts/chatgpt-codex/device-login/${startBody.attempt.id}`,
        {
          method: 'GET',
          headers: { authorization: `Bearer ${token}` },
        },
      ),
      env(),
      startBody.attempt.id,
    )
    const pollBody = (await pollResponse.json()) as {
      account: { status: string; planType: string | null }
      attempt: { status: string }
      pylonLink: { owner: string; status: string }
    }

    expect(pollResponse.status).toBe(200)
    expect(pollBody.account.status).toBe('connected')
    expect(pollBody.account.planType).toBe('pro')
    expect(pollBody.attempt.status).toBe('connected')
    expect(pollBody.pylonLink).toEqual({ owner: 'openauth', status: 'linked' })
    expect(connectedAuth).toHaveLength(1)
    expect(started.has(startBody.attempt.id)).toBe(false)
    expect(JSON.stringify(pollBody)).not.toContain('access-secret')
    expect(JSON.stringify(pollBody)).not.toContain('refresh-secret')
  })

  test('refuses unlinked Pylon agent tokens instead of attaching to the agent user', async () => {
    const token = 'oa_agent_unlinked_token'
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, null),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => new MemoryProviderAccountRepository(),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response = await handlers.handlePylonProviderDeviceLoginStartApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/chatgpt-codex/device-login/start',
        {
          method: 'POST',
          headers: { authorization: `Bearer ${token}` },
        },
      ),
      env(),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(409)
    expect(body.error).toBe('pylon_agent_not_linked')
  })
})
