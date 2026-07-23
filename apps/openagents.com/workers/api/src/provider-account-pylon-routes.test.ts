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

const makeAccount = (
  overrides: Partial<ProviderAccountRecord>,
): ProviderAccountRecord => ({
  accountLabel: 'Claude Work',
  authMode: 'api_key',
  connectedAt: '2026-06-25T12:00:00.000Z',
  createdAt: '2026-06-25T12:00:00.000Z',
  deletedAt: null,
  deniedAt: null,
  disconnectedAt: null,
  health: 'healthy',
  id: 'provider-account-claude-owner',
  lastStatusAt: '2026-06-25T12:00:00.000Z',
  metadataJson: null,
  planType: null,
  provider: 'anthropic_claude',
  providerAccountRef: 'provider_account_claude_owner',
  secretRef: 'provider-auth:provider_account_claude_owner',
  status: 'connected',
  teamId: null,
  updatedAt: '2026-06-25T12:00:00.000Z',
  userId: 'openauth-user-owner',
  ...overrides,
})

const makeGeminiGrant = (
  overrides: Partial<ProviderAccountAuthGrantRecord> = {},
): ProviderAccountAuthGrantRecord => ({
  id: 'provider_grant_gemini_owner',
  providerAccountId: 'provider_account_gemini_owner',
  userId: 'openauth-user-owner',
  teamId: null,
  threadId: 'thread.agent_computer.test',
  workroomId: 'workroom.agent_computer.test',
  runnerSessionId: 'turn.agent_computer.gemini.test',
  provider: 'google_gemini',
  providerAccountRef: 'provider_account_gemini_owner',
  providerSecretRef:
    'provider-account://google-gemini/worker-secret/GEMINI_API_KEY',
  grantRef: 'provider-auth-grant_gemini_owner',
  status: 'issued',
  requestedAction: 'agent_computer_gemini_turn',
  metadataJson: '{}',
  createdAt: '2026-07-23T12:00:00.000Z',
  updatedAt: '2026-07-23T12:00:00.000Z',
  expiresAt: '2099-07-23T12:05:00.000Z',
  usedAt: null,
  revokedAt: null,
  failedAt: null,
  ...overrides,
})

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

const makeGeminiMaterialHandlers = (input: {
  token: string
  repository: MemoryProviderAccountRepository
  ownerUserId?: string | null
  readSecret?: () => string | undefined
}) =>
  makeProviderAccountPylonHandlers({
    agentStore: () =>
      agentStoreFor(input.token, input.ownerUserId ?? 'openauth-user-owner'),
    deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
    makeProviderAccountRepository: () => input.repository,
    providerGrantRepository: () => input.repository,
    readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
    readGoogleGeminiSecretMaterial:
      input.readSecret ?? (() => 'gemini-secret-value'),
    readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
    storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
    storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
  })

const geminiMaterialRequest = (
  token: string | undefined,
  overrides: Record<string, unknown> = {},
): Request =>
  new Request(
    'https://openagents.com/api/pylon/provider-accounts/google-gemini/auth-material',
    {
      method: 'POST',
      headers: {
        ...(token === undefined
          ? {}
          : { authorization: `Bearer ${token}` }),
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        grantRef: 'provider-auth-grant_gemini_owner',
        kind: 'gemini_api_key',
        providerAccountRef: 'provider_account_gemini_owner',
        runnerSessionId: 'turn.agent_computer.gemini.test',
        secretRef:
          'provider-account://google-gemini/worker-secret/GEMINI_API_KEY',
        ...overrides,
      }),
    },
  )

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
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
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
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
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

  test('issues access-only Codex auth material from custody for linked Pylons', async () => {
    const token = 'oa_agent_auth_material_token'
    const calls: Array<{ ownerUserId: string; providerAccountRef: string }> = []
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({
      accountLabel: 'Codex Work',
      authMode: 'codex_device_auth',
      id: 'provider-account-codex-owner',
      provider: 'chatgpt_codex',
      providerAccountRef: 'provider_account_codex_owner',
      secretRef: 'codex-auth://provider_account_codex_owner',
    }))
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => repository,
      readConnectedCodexAuthMaterial: (_env, ownerUserId, providerAccountRef) => {
        calls.push({ ownerUserId, providerAccountRef })
        return Promise.resolve({
          authContentEnv: 'OPENCODE_AUTH_CONTENT',
          authContentJson: JSON.stringify({
            openai: {
              type: 'oauth',
              access: 'access-secret',
              expires: 1_800_000_000,
            },
          }),
        })
      },
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response = await handlers.handlePylonProviderCodexAuthMaterialApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/chatgpt-codex/auth-material',
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${token}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            providerAccountRef: 'provider_account_codex_owner',
          }),
        },
      ),
      env(),
    )
    const body = (await response.json()) as {
      authMaterial: {
        authContentEnv: string
        authContentJson: string
      }
      pylonLink: { owner: string; status: string }
      status: string
    }
    const authContent = JSON.parse(body.authMaterial.authContentJson) as {
      openai: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(repository.grants).toHaveLength(1)
    expect(repository.grants[0]?.status).toBe('used')
    expect(repository.grants[0]?.requestedAction).toBe(
      'pylon_local_codex_assignment',
    )
    expect(body.status).toBe('issued')
    expect(body.pylonLink).toEqual({ owner: 'openauth', status: 'linked' })
    expect(calls).toEqual([
      {
        ownerUserId: 'openauth-user-owner',
        providerAccountRef: 'provider_account_codex_owner',
      },
    ])
    expect(body.authMaterial.authContentEnv).toBe('OPENCODE_AUTH_CONTENT')
    expect(authContent.openai.access).toBe('access-secret')
    expect(authContent.openai.refresh).toBeUndefined()
    expect(JSON.stringify(body)).not.toContain('refresh-secret')
  })

  test('refuses custody auth material requests from unlinked Pylon agents', async () => {
    const token = 'oa_agent_unlinked_auth_material_token'
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, null),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => new MemoryProviderAccountRepository(),
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response = await handlers.handlePylonProviderCodexAuthMaterialApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/chatgpt-codex/auth-material',
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${token}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            providerAccountRef: 'provider_account_codex_owner',
          }),
        },
      ),
      env(),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(409)
    expect(body.error).toBe('pylon_agent_not_linked')
  })

  test('issues Claude auth material only for the linked owner account', async () => {
    const token = 'oa_agent_claude_auth_material_token'
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))
    const calls: Array<{ ownerUserId: string; providerAccountRef: string }> = []
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => repository,
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readConnectedClaudeAuthMaterial: (_env, ownerUserId, providerAccountRef) => {
        calls.push({ ownerUserId, providerAccountRef })
        return Promise.resolve({
          authContentEnv: 'CLAUDE_CODE_OAUTH_TOKEN',
          authContentValue: 'sk-ant-oat-claude-secret',
        })
      },
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response = await handlers.handlePylonProviderClaudeAuthMaterialApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/anthropic-claude/auth-material',
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${token}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            providerAccountRef: 'provider_account_claude_owner',
          }),
        },
      ),
      env(),
    )
    const body = (await response.json()) as {
      authMaterial: {
        authContentEnv: string
        authContentValue: string
      }
      pylonLink: { owner: string; status: string }
      status: string
    }

    expect(response.status).toBe(200)
    expect(body.status).toBe('issued')
    expect(body.pylonLink).toEqual({ owner: 'openauth', status: 'linked' })
    expect(calls).toEqual([
      {
        ownerUserId: 'openauth-user-owner',
        providerAccountRef: 'provider_account_claude_owner',
      },
    ])
    expect(body.authMaterial.authContentEnv).toBe('CLAUDE_CODE_OAUTH_TOKEN')
    expect(body.authMaterial.authContentValue).toBe('sk-ant-oat-claude-secret')
  })

  test('materializes Gemini secret once for the exact owner and turn grant', async () => {
    const token = 'oa_agent_gemini_material_token'
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(makeGeminiGrant())
    let secretReads = 0
    const handlers = makeGeminiMaterialHandlers({
      token,
      repository,
      readSecret: () => {
        secretReads += 1
        return 'gemini-secret-value'
      },
    })

    const response =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(token),
        env(),
      )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body).toEqual({
      schemaVersion: 'openagents.provider_secret_material.v1',
      grantRef: 'provider-auth-grant_gemini_owner',
      providerAccountRef: 'provider_account_gemini_owner',
      runnerSessionId: 'turn.agent_computer.gemini.test',
      secretRef:
        'provider-account://google-gemini/worker-secret/GEMINI_API_KEY',
      secretValue: 'gemini-secret-value',
    })
    expect(secretReads).toBe(1)
    expect(repository.grants[0]?.status).toBe('used')
    expect(repository.grants[0]?.usedAt).not.toBeNull()
  })

  test.each([
    [
      'owner',
      makeGeminiGrant({ userId: 'openauth-user-other' }),
      {},
    ],
    [
      'turn',
      makeGeminiGrant(),
      { runnerSessionId: 'turn.agent_computer.gemini.other' },
    ],
    [
      'grant',
      makeGeminiGrant(),
      { grantRef: 'provider-auth-grant_gemini_other' },
    ],
    [
      'account',
      makeGeminiGrant(),
      { providerAccountRef: 'provider_account_gemini_other' },
    ],
    [
      'secret',
      makeGeminiGrant(),
      {
        secretRef:
          'provider-account://google-gemini/worker-secret/OTHER_SECRET',
      },
    ],
    [
      'action',
      makeGeminiGrant({ requestedAction: 'probe_gemini_turn' }),
      {},
    ],
    ['kind', makeGeminiGrant(), { kind: 'probe_gemini_api_key' }],
  ])(
    'refuses Gemini material when the %s scope does not match',
    async (_scope, grant, overrides) => {
      const token = `oa_agent_gemini_wrong_${_scope}_token`
      const repository = new MemoryProviderAccountRepository()
      repository.grants.push(grant)
      let secretReads = 0
      const handlers = makeGeminiMaterialHandlers({
        token,
        repository,
        readSecret: () => {
          secretReads += 1
          return 'gemini-secret-value'
        },
      })

      const response =
        await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
          geminiMaterialRequest(token, overrides),
          env(),
        )
      const body = (await response.json()) as { error: string }

      expect(response.status).toBe(409)
      expect(response.headers.get('cache-control')).toBe('no-store')
      expect(body.error).toBe('provider_secret_material_scope_mismatch')
      expect(JSON.stringify(body)).not.toContain('gemini-secret-value')
      expect(secretReads).toBe(0)
      expect(repository.grants[0]?.status).toBe('issued')
    },
  )

  test('rejects a Gemini material request with a missing scope field', async () => {
    const token = 'oa_agent_gemini_missing_scope_token'
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(makeGeminiGrant())
    let secretReads = 0
    const handlers = makeGeminiMaterialHandlers({
      token,
      repository,
      readSecret: () => {
        secretReads += 1
        return 'gemini-secret-value'
      },
    })

    const response =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(token, { runnerSessionId: null }),
        env(),
      )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(400)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.error).toBe('provider_secret_material_request_invalid')
    expect(secretReads).toBe(0)
    expect(repository.grants[0]?.status).toBe('issued')
  })

  test('fails closed when Gemini secret material is missing', async () => {
    const token = 'oa_agent_gemini_missing_material_token'
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(makeGeminiGrant())
    const handlers = makeGeminiMaterialHandlers({
      token,
      repository,
      readSecret: () => undefined,
    })

    const response =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(token),
        env(),
      )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(409)
    expect(body.error).toBe('provider_secret_material_unavailable')
    expect(repository.grants[0]?.status).toBe('issued')
  })

  test('rejects replay after one Gemini material response', async () => {
    const token = 'oa_agent_gemini_replay_token'
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(makeGeminiGrant())
    const handlers = makeGeminiMaterialHandlers({ token, repository })

    const first =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(token),
        env(),
      )
    const replay =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(token),
        env(),
      )
    const replayBody = (await replay.json()) as { error: string }

    expect(first.status).toBe(200)
    expect(replay.status).toBe(409)
    expect(replayBody.error).toBe('provider_secret_material_unavailable')
    expect(JSON.stringify(replayBody)).not.toContain('gemini-secret-value')
  })

  test('rejects an expired Gemini grant without returning secret material', async () => {
    const token = 'oa_agent_gemini_expired_token'
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(
      makeGeminiGrant({ expiresAt: '2020-07-23T12:05:00.000Z' }),
    )
    const handlers = makeGeminiMaterialHandlers({ token, repository })

    const response =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(token),
        env(),
      )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(409)
    expect(body.error).toBe('provider_secret_material_unavailable')
    expect(JSON.stringify(body)).not.toContain('gemini-secret-value')
    expect(repository.grants[0]?.status).toBe('issued')
  })

  test('requires an Agent Computer bearer header for Gemini material', async () => {
    const token = 'oa_agent_gemini_header_token'
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(makeGeminiGrant())
    const handlers = makeGeminiMaterialHandlers({ token, repository })

    const response =
      await handlers.handlePylonProviderGoogleGeminiAuthMaterialApi(
        geminiMaterialRequest(undefined),
        env(),
      )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.error).toBe('unauthorized')
    expect(repository.grants[0]?.status).toBe('issued')
  })

  test('refuses Claude auth material for an account not owned by the linked user', async () => {
    const token = 'oa_agent_claude_wrong_owner_token'
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        userId: 'other-owner',
        providerAccountRef: 'provider_account_claude_other',
      }),
    )
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => repository,
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readConnectedClaudeAuthMaterial: () =>
        Promise.resolve({
          authContentEnv: 'CLAUDE_CODE_OAUTH_TOKEN',
          authContentValue: 'sk-ant-oat-should-not-read',
        }),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response = await handlers.handlePylonProviderClaudeAuthMaterialApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/anthropic-claude/auth-material',
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${token}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            providerAccountRef: 'provider_account_claude_other',
          }),
        },
      ),
      env(),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(409)
    expect(body.error).toBe('provider_account_auth_material_unavailable')
  })

  test('imports local Codex auth under the linked OpenAuth owner without echoing credentials', async () => {
    const token = 'oa_agent_import_token'
    const repository = new MemoryProviderAccountRepository()
    const connectedAuth: Array<{
      auth: { access?: string; refresh?: string; type?: string }
      providerAccountRef: string
    }> = []
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => repository,
      nowIso: () => '2026-06-25T12:00:00.000Z',
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => input => {
        connectedAuth.push(input)

        return Promise.resolve(`codex-auth://${input.providerAccountRef}`)
      },
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response = await handlers.handlePylonProviderLocalCodexAuthImportApi(
      new Request(
        'https://openagents.com/api/pylon/provider-accounts/chatgpt-codex/local-auth/import',
        {
          method: 'POST',
          headers: {
            authorization: `Bearer ${token}`,
            'content-type': 'application/json',
          },
          body: JSON.stringify({
            accountLabel: 'codex',
            createNew: true,
            auth: {
              type: 'oauth',
              access: 'access-secret',
              refresh: 'refresh-secret',
              expires: 1_800_000_000,
              accountId: 'account-fixture',
              idToken: 'id-secret',
            },
          }),
        },
      ),
      env(),
    )
    const body = (await response.json()) as {
      account: { providerAccountRef: string; status: string }
      attempt: { id: string; status: string }
      pylonLink: { owner: string; status: string }
    }
    const responseText = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body.account.status).toBe('connected')
    expect(body.attempt.status).toBe('connected')
    expect(body.pylonLink).toEqual({ owner: 'openauth', status: 'linked' })
    expect(repository.accounts[0]?.userId).toBe('openauth-user-owner')
    expect(repository.accounts[0]?.authMode).toBe('codex_device_auth')
    expect(repository.accounts[0]?.status).toBe('connected')
    expect(repository.attempts[0]?.method).toBe('codex_device_auth')
    expect(repository.attempts[0]?.source).toBe('pylon_local_codex_auth')
    expect(connectedAuth).toHaveLength(1)
    expect(connectedAuth[0]?.providerAccountRef).toBe(
      body.account.providerAccountRef,
    )
    expect(connectedAuth[0]?.auth).toMatchObject({
      type: 'oauth',
      access: 'access-secret',
      refresh: 'refresh-secret',
    })
    expect(responseText).not.toContain('access-secret')
    expect(responseText).not.toContain('refresh-secret')
    expect(responseText).not.toContain('id-secret')
  })

  test('imports local Claude auth under the linked OpenAuth owner without echoing credentials', async () => {
    const token = 'oa_agent_claude_import_token'
    const repository = new MemoryProviderAccountRepository()
    const connectedAuth: Array<{
      authContentValue: string
      providerAccountRef: string
    }> = []
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => repository,
      nowIso: () => '2026-06-25T12:00:00.000Z',
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedClaudeAuth: () => input => {
        connectedAuth.push(input)

        return Promise.resolve(
          `provider-account://anthropic-claude/user-oauth-token/${input.providerAccountRef}`,
        )
      },
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response =
      await handlers.handlePylonProviderLocalClaudeAuthImportApi(
        new Request(
          'https://openagents.com/api/pylon/provider-accounts/anthropic-claude/local-auth/import',
          {
            method: 'POST',
            headers: {
              authorization: `Bearer ${token}`,
              'content-type': 'application/json',
            },
            body: JSON.stringify({
              accountLabel: 'claude',
              createNew: true,
              authContentValue: 'sk-ant-oat-claude-secret',
            }),
          },
        ),
        env(),
      )
    const body = (await response.json()) as {
      account: { providerAccountRef: string; status: string }
      attempt: { id: string; status: string }
      pylonLink: { owner: string; status: string }
    }
    const responseText = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body.account.status).toBe('connected')
    expect(body.attempt.status).toBe('connected')
    expect(body.pylonLink).toEqual({ owner: 'openauth', status: 'linked' })
    expect(repository.accounts[0]?.userId).toBe('openauth-user-owner')
    expect(repository.accounts[0]?.provider).toBe('anthropic_claude')
    expect(repository.accounts[0]?.authMode).toBe('claude_local_auth')
    expect(repository.accounts[0]?.status).toBe('connected')
    expect(repository.attempts[0]?.method).toBe('claude_local_auth')
    expect(repository.attempts[0]?.source).toBe('pylon_local_claude_auth')
    expect(connectedAuth).toHaveLength(1)
    expect(connectedAuth[0]?.providerAccountRef).toBe(
      body.account.providerAccountRef,
    )
    expect(connectedAuth[0]?.authContentValue).toBe('sk-ant-oat-claude-secret')
    expect(responseText).not.toContain('sk-ant-oat-claude-secret')
  })

  test('rejects local Claude auth import missing the OAuth token', async () => {
    const token = 'oa_agent_claude_import_missing_token'
    const repository = new MemoryProviderAccountRepository()
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      makeProviderAccountRepository: () => repository,
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedClaudeAuth: () => () =>
        Promise.resolve('provider-account://anthropic-claude/user-oauth-token/unused'),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response =
      await handlers.handlePylonProviderLocalClaudeAuthImportApi(
        new Request(
          'https://openagents.com/api/pylon/provider-accounts/anthropic-claude/local-auth/import',
          {
            method: 'POST',
            headers: {
              authorization: `Bearer ${token}`,
              'content-type': 'application/json',
            },
            body: JSON.stringify({}),
          },
        ),
        env(),
      )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(400)
    expect(body.error).toBe('provider_local_claude_auth_import_failed')
    expect(repository.accounts).toHaveLength(0)
  })

  test('fails closed when Claude local auth import is not configured', async () => {
    const token = 'oa_agent_claude_import_not_configured'
    const handlers = makeProviderAccountPylonHandlers({
      agentStore: () => agentStoreFor(token, 'openauth-user-owner'),
      deleteStartedCodexDeviceLogin: () => () => Promise.resolve(),
      readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
      readStartedCodexDeviceLogin: () => () => Promise.resolve(undefined),
      storeConnectedCodexAuth: () => () => Promise.resolve('codex-auth://unused'),
      storeStartedCodexDeviceLogin: () => () => Promise.resolve(),
    })

    const response =
      await handlers.handlePylonProviderLocalClaudeAuthImportApi(
        new Request(
          'https://openagents.com/api/pylon/provider-accounts/anthropic-claude/local-auth/import',
          {
            method: 'POST',
            headers: {
              authorization: `Bearer ${token}`,
              'content-type': 'application/json',
            },
            body: JSON.stringify({ authContentValue: 'sk-ant-oat-unused' }),
          },
        ),
        env(),
      )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(501)
    expect(body.error).toBe('claude_local_auth_import_not_configured')
  })
})
