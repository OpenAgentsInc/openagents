/* eslint-disable @typescript-eslint/consistent-type-assertions */
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import worker from './index'
import {
  CHATGPT_CODEX_PROVIDER,
  CHATGPT_CODEX_VERIFICATION_URL,
  PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS,
  type ProviderAccountAuthGrantRecord,
  ProviderAccountClientRequestFailed,
  type ProviderAccountEventRecord,
  ProviderAccountLifecycleService,
  ProviderAccountNotConnectedHealthy,
  type ProviderAccountRecord,
  type ProviderAccountRepository,
  type ProviderConnectionAttemptRecord,
  ProviderGrantNotIssued,
  assertProviderAccountPublicProjection,
  buildRedactedOpenCodeMaterializationPlan,
  disconnectProviderAccountForUser,
  getDeviceLoginAttemptForUser,
  issueProviderAccountGrant,
  listProviderAccountsForUser,
  makeOpenAiCodexProviderClient,
  makeProviderAccountRepositoryService,
  pollOpenAiCodexDeviceLogin,
  recordDeviceLoginConnected,
  recordDeviceLoginFailed,
  recordProviderAccountHealth,
  refreshChatGptCodexDeviceLoginForUser,
  refreshOpenAiCodexOAuthAuth,
  resolveProviderAccountGrant,
  startChatGptCodexDeviceLogin,
  toPublicProviderAccount,
  toPublicProviderAccountEvent,
  toPublicProviderAccountGrant,
  toPublicProviderConnectionAttempt,
} from './provider-accounts'
import {
  makeProviderAccountLifecycleTestLayer,
  providerAccountTestIdFactory,
  providerAccountTestNow,
} from './test/service-fixtures'

class MemoryProviderAccountRepository implements ProviderAccountRepository {
  readonly accounts: Array<ProviderAccountRecord> = []
  readonly attempts: Array<ProviderConnectionAttemptRecord> = []
  readonly events: Array<ProviderAccountEventRecord> = []
  readonly grants: Array<ProviderAccountAuthGrantRecord> = []

  findAccountByRef(
    userId: string,
    providerAccountRef: string,
  ): Promise<ProviderAccountRecord | undefined> {
    return Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )
  }

  findAccountByProviderAccountRef(
    providerAccountRef: string,
  ): Promise<ProviderAccountRecord | undefined> {
    return Promise.resolve(
      this.accounts.find(
        account =>
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )
  }

  findReusableAccount(
    userId: string,
  ): Promise<ProviderAccountRecord | undefined> {
    return Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.status !== 'connected' &&
          account.deletedAt === null,
      ),
    )
  }

  listAccountsForUser(
    userId: string,
  ): Promise<ReadonlyArray<ProviderAccountRecord>> {
    return Promise.resolve(
      this.accounts.filter(
        account => account.userId === userId && account.deletedAt === null,
      ),
    )
  }

  listPendingAttemptsForUser(
    userId: string,
  ): Promise<ReadonlyArray<ProviderConnectionAttemptRecord>> {
    return Promise.resolve(
      this.attempts.filter(
        attempt => attempt.userId === userId && attempt.status === 'pending',
      ),
    )
  }

  findAttemptForUser(
    userId: string,
    attemptId: string,
  ): Promise<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined
  > {
    const attempt = this.attempts.find(
      candidate => candidate.userId === userId && candidate.id === attemptId,
    )
    const account = this.accounts.find(
      candidate => candidate.id === attempt?.providerAccountId,
    )

    return Promise.resolve(
      attempt === undefined || account === undefined
        ? undefined
        : { account, attempt },
    )
  }

  findAttemptById(attemptId: string): Promise<
    | Readonly<{
        account: ProviderAccountRecord
        attempt: ProviderConnectionAttemptRecord
      }>
    | undefined
  > {
    const attempt = this.attempts.find(candidate => candidate.id === attemptId)
    const account = this.accounts.find(
      candidate => candidate.id === attempt?.providerAccountId,
    )

    return Promise.resolve(
      attempt === undefined || account === undefined
        ? undefined
        : { account, attempt },
    )
  }

  saveStartedDeviceLogin(
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
    accountAlreadyExists: boolean,
  ): Promise<void> {
    if (accountAlreadyExists) {
      const index = this.accounts.findIndex(
        candidate => candidate.id === account.id,
      )

      this.accounts.splice(index, 1, account)
    } else {
      this.accounts.push(account)
    }

    this.attempts.push(attempt)
    this.events.push(event)

    return Promise.resolve()
  }

  recordConnectedAttempt(
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord> {
    this.replaceAccount(account)
    this.replaceAttempt(attempt)
    this.events.push(event)

    return Promise.resolve(account)
  }

  recordFailedAttempt(
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord> {
    this.replaceAccount(account)
    this.replaceAttempt(attempt)
    this.events.push(event)

    return Promise.resolve(account)
  }

  recordAccountHealth(
    providerAccountRef: string,
    account: ProviderAccountRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord | undefined> {
    const existing = this.accounts.find(
      candidate => candidate.providerAccountRef === providerAccountRef,
    )

    if (existing === undefined) {
      return Promise.resolve(undefined)
    }

    this.replaceAccount(account)
    this.events.push(event)

    return Promise.resolve(account)
  }

  createAuthGrant(
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountAuthGrantRecord> {
    this.grants.push(grant)
    this.events.push({ ...event, authGrantId: grant.id })

    return Promise.resolve(grant)
  }

  findGrantByRef(
    grantRef: string,
  ): Promise<ProviderAccountAuthGrantRecord | undefined> {
    return Promise.resolve(
      this.grants.find(grant => grant.grantRef === grantRef),
    )
  }

  markGrantUsed(
    grant: ProviderAccountAuthGrantRecord,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountAuthGrantRecord> {
    this.replaceGrant(grant)
    this.events.push({ ...event, authGrantId: grant.id })

    return Promise.resolve(grant)
  }

  disconnectAccount(
    userId: string,
    providerAccountRef: string,
    now: string,
    metadataJson: string,
    event: ProviderAccountEventRecord,
  ): Promise<ProviderAccountRecord | undefined> {
    const index = this.accounts.findIndex(
      account =>
        account.userId === userId &&
        account.providerAccountRef === providerAccountRef,
    )

    if (index < 0) {
      return Promise.resolve(undefined)
    }

    const previous = this.accounts[index]

    if (previous === undefined) {
      return Promise.resolve(undefined)
    }

    const updated: ProviderAccountRecord = {
      ...previous,
      disconnectedAt: now,
      health: 'requires_reauth',
      lastStatusAt: now,
      metadataJson,
      secretRef: null,
      status: 'disconnected',
      updatedAt: now,
    }

    this.accounts.splice(index, 1, updated)
    this.grants
      .filter(grant => grant.providerAccountId === previous.id)
      .forEach(grant => {
        this.grants.splice(this.grants.indexOf(grant), 1, {
          ...grant,
          status: 'revoked',
          revokedAt: now,
          updatedAt: now,
        })
      })
    this.events.push({ ...event, providerAccountId: previous.id })

    return Promise.resolve(updated)
  }

  private replaceAccount(account: ProviderAccountRecord): void {
    const index = this.accounts.findIndex(
      candidate => candidate.id === account.id,
    )

    if (index < 0) {
      this.accounts.push(account)
      return
    }

    this.accounts.splice(index, 1, account)
  }

  private replaceAttempt(attempt: ProviderConnectionAttemptRecord): void {
    const index = this.attempts.findIndex(
      candidate => candidate.id === attempt.id,
    )

    if (index < 0) {
      this.attempts.push(attempt)
      return
    }

    this.attempts.splice(index, 1, attempt)
  }

  private replaceGrant(grant: ProviderAccountAuthGrantRecord): void {
    const index = this.grants.findIndex(candidate => candidate.id === grant.id)

    if (index < 0) {
      this.grants.push(grant)
      return
    }

    this.grants.splice(index, 1, grant)
  }
}

const makeIdFactory = (values: ReadonlyArray<string>) => {
  const queue = Array.from(values)

  return (prefix: string): string => {
    const value = queue.shift()

    if (value === undefined) {
      throw new Error('id factory exhausted')
    }

    return `${prefix}_${value}`
  }
}

const makeAccount = (
  overrides: Partial<ProviderAccountRecord>,
): ProviderAccountRecord => ({
  id: 'provider_account_1',
  userId: 'github:1',
  teamId: null,
  provider: CHATGPT_CODEX_PROVIDER,
  authMode: 'chatgpt_device_code',
  status: 'connected',
  health: 'healthy',
  providerAccountRef: 'provider-account_1',
  secretRef: 'codex-auth://provider-account_1',
  accountLabel: 'Main ChatGPT',
  planType: 'plus',
  connectedAt: '2026-06-02T19:00:00.000Z',
  disconnectedAt: null,
  deniedAt: null,
  lastStatusAt: '2026-06-02T19:00:00.000Z',
  metadataJson: '{}',
  createdAt: '2026-06-02T19:00:00.000Z',
  updatedAt: '2026-06-02T19:00:00.000Z',
  deletedAt: null,
  ...overrides,
})

const makeAttempt = (
  overrides: Partial<ProviderConnectionAttemptRecord>,
): ProviderConnectionAttemptRecord => ({
  id: 'provider_attempt_1',
  providerAccountId: 'provider_account_1',
  userId: 'github:1',
  teamId: null,
  provider: CHATGPT_CODEX_PROVIDER,
  method: 'chatgpt_device_code',
  source: 'worker_device_code',
  loginRef: 'codex_login_1',
  verificationUrl: CHATGPT_CODEX_VERIFICATION_URL,
  userCode: 'ABCD-1234',
  status: 'pending',
  expiresAt: '2026-06-02T19:15:00.000Z',
  completedAt: null,
  failedAt: null,
  metadataJson: '{}',
  createdAt: '2026-06-02T19:00:00.000Z',
  updatedAt: '2026-06-02T19:00:00.000Z',
  ...overrides,
})

const makeGrant = (
  overrides: Partial<ProviderAccountAuthGrantRecord>,
): ProviderAccountAuthGrantRecord => ({
  id: 'provider_grant_1',
  providerAccountId: 'provider_account_1',
  userId: 'github:1',
  teamId: null,
  threadId: 'thread_1',
  workroomId: 'workroom_1',
  runnerSessionId: 'runner_session_1',
  provider: CHATGPT_CODEX_PROVIDER,
  providerAccountRef: 'provider-account_1',
  providerSecretRef: 'codex-auth://provider-account_1',
  grantRef: 'codex-auth-grant_1',
  status: 'issued',
  requestedAction: 'run_opencode',
  metadataJson: '{}',
  createdAt: '2026-06-02T19:00:00.000Z',
  updatedAt: '2026-06-02T19:00:00.000Z',
  expiresAt: '2026-06-02T21:00:00.000Z',
  usedAt: null,
  revokedAt: null,
  failedAt: null,
  ...overrides,
})

const makeEvent = (
  overrides: Partial<ProviderAccountEventRecord>,
): ProviderAccountEventRecord => ({
  id: 'provider_event_1',
  providerAccountId: 'provider_account_1',
  authGrantId: null,
  userId: 'github:1',
  teamId: null,
  threadId: null,
  workroomId: null,
  runnerSessionId: null,
  kind: 'login_connected',
  summary: 'ChatGPT/Codex account connected.',
  sourceRefsJson: '[]',
  evidenceRefsJson: '[]',
  targetRef: 'provider-account_1',
  metadataJson:
    '{"providerAccountRef":"provider-account_1","status":"connected"}',
  actorId: 'user_agent_broker',
  createdAt: '2026-06-02T19:00:00.000Z',
  ...overrides,
})

describe('provider account API route auth', () => {
  test('rejects provider account list without a browser session', async () => {
    const response = await worker.fetch(
      new Request('https://openagents.com/api/provider-accounts') as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_APP_URL: 'https://openagents.com',
        OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      } as never,
      {
        waitUntil: () => undefined,
        passThroughOnException: () => undefined,
      } as never,
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('rejects broker callback without service bearer auth', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/api/provider-accounts/chatgpt-codex/device-login/provider_attempt_1/connected',
        { method: 'POST' },
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_APP_URL: 'https://openagents.com',
        OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      } as never,
      {
        waitUntil: () => undefined,
        passThroughOnException: () => undefined,
      } as never,
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('rejects grant resolve without service bearer auth', async () => {
    const response = await worker.fetch(
      new Request(
        'https://openagents.com/api/provider-accounts/chatgpt-codex/grants/resolve',
        {
          body: JSON.stringify({ grantRef: 'codex-auth-grant_1' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ) as never,
      {
        ASSETS: {
          fetch: () => Response.json({ unused: true }),
        },
        OPENAGENTS_APP_URL: 'https://openagents.com',
        OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      } as never,
      {
        waitUntil: () => undefined,
        passThroughOnException: () => undefined,
      } as never,
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })
})

describe('provider account public projections', () => {
  const now = new Date('2026-06-02T19:05:00.000Z')

  test('names every public sync collection explicitly', () => {
    expect(Object.values(PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS)).toEqual([
      'provider_accounts_public',
      'provider_connection_attempts_public',
      'provider_account_events_public',
      'provider_account_grants_public',
      'runner_sessions_public',
    ])
  })

  test('projects logged-in account and grant data without secret refs', () => {
    const account = toPublicProviderAccount(makeAccount({}), [], now)
    const grant = toPublicProviderAccountGrant(makeGrant({}), now)
    const payload = JSON.stringify({ account, grant })

    expect(account.hasSecretRef).toBe(true)
    expect(grant.providerAccountRef).toBe('provider-account_1')
    expect(payload).not.toContain('providerSecretRef')
    expect(payload).not.toContain('secretRef')
    expect(payload).not.toContain('OPENCODE_AUTH_CONTENT')
    expect(payload).not.toContain('auth.json')
  })

  test('rejects credential-shaped account, attempt, grant, and event projections', () => {
    expect(() =>
      toPublicProviderAccount(
        makeAccount({ accountLabel: '{"refresh_token":"secret"}' }),
        [],
        now,
      ),
    ).toThrow(/credential material/)

    expect(() =>
      toPublicProviderConnectionAttempt(
        makeAttempt({
          verificationUrl:
            'https://auth.openai.com/codex/device?access_token=secret',
        }),
        makeAccount({}),
        now,
      ),
    ).toThrow(/credential material/)

    expect(() =>
      toPublicProviderAccountGrant(
        makeGrant({
          requestedAction: 'OPENCODE_AUTH_CONTENT={"openai":{"type":"oauth"}}',
        }),
        now,
      ),
    ).toThrow(/credential material/)

    expect(() =>
      toPublicProviderAccountEvent(
        makeEvent({
          metadataJson: '{"refresh_token":"secret"}',
        }),
      ),
    ).toThrow(/credential material/)

    expect(() =>
      assertProviderAccountPublicProjection(
        PROVIDER_ACCOUNT_PUBLIC_COLLECTIONS.events,
        toPublicProviderAccountEvent(
          makeEvent({
            summary:
              '{"openai":{"type":"oauth","refresh":"fake-refresh","access":"fake-access"}}',
          }),
        ),
      ),
    ).toThrow(/credential material/)
  })
})

describe('provider account service', () => {
  test('starts a ChatGPT/Codex device login with only public ceremony fields', async () => {
    const repository = new MemoryProviderAccountRepository()
    const now = new Date('2026-06-02T19:00:00.000Z')
    const storedDeviceLogins: Array<
      Readonly<{
        attemptId: string
        deviceAuthId: string
        userCode: string
        expiresAt: string
      }>
    > = []
    const result = await startChatGptCodexDeviceLogin(
      repository,
      {
        accountLabel: 'Main ChatGPT',
        createNew: true,
        userId: 'github:1',
      },
      () =>
        Promise.resolve({
          deviceAuthId: 'device_auth_1',
          expiresAt: '2026-06-02T19:15:00.000Z',
          intervalSeconds: 5,
          userCode: 'ABCD-1234',
          verificationUrl: CHATGPT_CODEX_VERIFICATION_URL,
        }),
      {
        makeId: makeIdFactory([
          'ref1',
          'account1',
          'attempt1',
          'login1',
          'event1',
        ]),
        now: () => now,
        storeStartedDeviceLogin: input => {
          storedDeviceLogins.push(input)

          return Promise.resolve()
        },
      },
    )

    expect(result.userCode).toBe('ABCD-1234')
    expect(result.verificationUrl).toBe(CHATGPT_CODEX_VERIFICATION_URL)
    expect(result.account.publicStatus).toBe('pending')
    expect(result.attempt.status).toBe('pending')
    expect(repository.accounts[0]?.secretRef).toBeNull()
    expect(storedDeviceLogins).toEqual([
      {
        attemptId: result.attempt.id,
        deviceAuthId: 'device_auth_1',
        expiresAt: '2026-06-02T19:15:00.000Z',
        userCode: 'ABCD-1234',
      },
    ])
    expect(JSON.stringify(result)).not.toContain('device_auth_id')
    expect(JSON.stringify(result)).not.toContain('device_auth_1')
  })

  test('refresh polls Codex device login and records a connected account without public token leakage', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        connectedAt: null,
        health: 'unknown',
        secretRef: null,
        status: 'pending',
      }),
    )
    repository.attempts.push(makeAttempt({}))
    const storedAuth: Array<Readonly<{ providerAccountRef: string }>> = []
    const deletedAttempts: Array<string> = []

    const result = await refreshChatGptCodexDeviceLoginForUser(
      repository,
      {
        attemptId: 'provider_attempt_1',
        userId: 'github:1',
      },
      () =>
        Promise.resolve({
          deviceAuthId: 'device_auth_1',
          userCode: 'ABCD-1234',
        }),
      input => {
        storedAuth.push({ providerAccountRef: input.providerAccountRef })

        return Promise.resolve(`codex-auth://${input.providerAccountRef}`)
      },
      () =>
        Promise.resolve({
          accountLabel: 'chris@openagents.com',
          auth: {
            access: 'fake-access-token',
            expires: 1_800_000_000_000,
            refresh: 'fake-refresh-token',
            type: 'oauth',
          },
          status: 'connected',
        }),
      attemptId => {
        deletedAttempts.push(attemptId)

        return Promise.resolve()
      },
      {
        makeId: makeIdFactory(['connected_event']),
        now: () => new Date('2026-06-02T19:05:00.000Z'),
      },
    )

    expect(result?.account.publicStatus).toBe('connected')
    expect(result?.account.hasSecretRef).toBe(true)
    expect(result?.attempt.status).toBe('connected')
    expect(repository.accounts[0]?.secretRef).toBe(
      'codex-auth://provider-account_1',
    )
    expect(repository.attempts[0]?.userCode).toBeNull()
    expect(storedAuth).toEqual([{ providerAccountRef: 'provider-account_1' }])
    expect(deletedAttempts).toEqual(['provider_attempt_1'])
    expect(JSON.stringify(result)).not.toContain('fake-access-token')
    expect(JSON.stringify(result)).not.toContain('fake-refresh-token')
  })

  test('OpenAI Codex poller exchanges device completion for OAuth auth content', async () => {
    const requests: Array<Readonly<{ url: string; body: string }>> = []
    const fetcher: typeof fetch = async (
      input: RequestInfo | URL,
      init?: RequestInit,
    ): Promise<Response> => {
      requests.push({
        url: String(input),
        body: String(init?.body ?? ''),
      })

      if (String(input).endsWith('/api/accounts/deviceauth/token')) {
        return Response.json({
          authorization_code: 'authorization-code-1',
          code_verifier: 'code-verifier-1',
        })
      }

      return Response.json({
        access_token: 'access-token-1',
        expires_in: 3600,
        id_token: 'id-token-1',
        refresh_token: 'refresh-token-1',
      })
    }

    const result = await pollOpenAiCodexDeviceLogin(
      {
        deviceAuthId: 'device-auth-1',
        userCode: 'ABCD-1234',
      },
      fetcher,
      new Date('2026-06-02T19:00:00.000Z'),
    )

    expect(result).toEqual({
      auth: {
        access: 'access-token-1',
        expires: 1_780_430_400_000,
        idToken: 'id-token-1',
        refresh: 'refresh-token-1',
        type: 'oauth',
      },
      accountLabel: undefined,
      status: 'connected',
    })
    expect(requests[0]).toEqual({
      url: 'https://auth.openai.com/api/accounts/deviceauth/token',
      body: '{"device_auth_id":"device-auth-1","user_code":"ABCD-1234"}',
    })
    expect(requests[1]).toEqual({
      url: 'https://auth.openai.com/oauth/token',
      body: 'grant_type=authorization_code&code=authorization-code-1&redirect_uri=https%3A%2F%2Fauth.openai.com%2Fdeviceauth%2Fcallback&client_id=app_EMoamEEZ73f0CkXaXp7hrann&code_verifier=code-verifier-1',
    })
  })

  test('OpenAI Codex refresh probe stores replacement OAuth auth content', async () => {
    const requests: Array<Readonly<{ url: string; body: string }>> = []
    const fetcher: typeof fetch = async (
      input: RequestInfo | URL,
      init?: RequestInit,
    ): Promise<Response> => {
      requests.push({
        url: String(input),
        body: String(init?.body ?? ''),
      })

      return Response.json({
        access_token: 'new-access-token-1',
        expires_in: 3600,
        refresh_token: 'new-refresh-token-1',
      })
    }

    const result = await refreshOpenAiCodexOAuthAuth(
      {
        access: 'old-access-token-1',
        accountId: 'account-1',
        expires: 0,
        idToken: 'old-id-token-1',
        refresh: 'old-refresh-token-1',
        type: 'oauth',
      },
      fetcher,
      new Date('2026-06-02T19:00:00.000Z'),
    )

    expect(result).toEqual({
      status: 'refreshed',
      auth: {
        access: 'new-access-token-1',
        accountId: 'account-1',
        expires: 1_780_430_400_000,
        idToken: 'old-id-token-1',
        refresh: 'new-refresh-token-1',
        type: 'oauth',
      },
    })
    expect(requests).toEqual([
      {
        url: 'https://auth.openai.com/oauth/token',
        body: 'grant_type=refresh_token&refresh_token=old-refresh-token-1&client_id=app_EMoamEEZ73f0CkXaXp7hrann',
      },
    ])
    expect(JSON.stringify(result)).not.toContain('old-refresh-token-1')
  })

  test('OpenAI Codex refresh probe classifies invalidated refresh tokens', async () => {
    const fetcher: typeof fetch = () =>
      Promise.resolve(
        Response.json(
          { error: { code: 'refresh_token_invalidated' } },
          { status: 401 },
        ),
      )

    const result = await refreshOpenAiCodexOAuthAuth(
      {
        access: 'old-access-token-1',
        expires: 0,
        refresh: 'old-refresh-token-1',
        type: 'oauth',
      },
      fetcher,
      new Date('2026-06-02T19:00:00.000Z'),
    )

    expect(result).toEqual({
      status: 'failed',
      code: 'refresh_token_invalidated',
      failureClass: 'token_invalidated',
      providerStatus: 401,
    })
  })

  test('projects expired pending attempts without exposing credential material', async () => {
    const repository = new MemoryProviderAccountRepository()
    const startedAt = new Date('2026-06-02T19:00:00.000Z')
    const result = await startChatGptCodexDeviceLogin(
      repository,
      { createNew: true, userId: 'github:1' },
      () =>
        Promise.resolve({
          deviceAuthId: 'device_auth_2',
          expiresAt: '2026-06-02T19:01:00.000Z',
          intervalSeconds: 5,
          userCode: 'WXYZ-9876',
          verificationUrl: CHATGPT_CODEX_VERIFICATION_URL,
        }),
      {
        makeId: makeIdFactory([
          'ref2',
          'account2',
          'attempt2',
          'login2',
          'event2',
        ]),
        now: () => startedAt,
      },
    )
    const status = await getDeviceLoginAttemptForUser(
      repository,
      'github:1',
      result.attempt.id,
      new Date('2026-06-02T19:02:00.000Z'),
    )

    expect(status?.attempt.status).toBe('expired')
    expect(status?.account.publicStatus).toBe('expired')
  })

  test('lists multiple provider accounts for the same OpenAgents user', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        id: 'provider_account_1',
        providerAccountRef: 'provider-account_1',
      }),
      makeAccount({
        id: 'provider_account_2',
        providerAccountRef: 'provider-account_2',
      }),
    )

    const bundle = await listProviderAccountsForUser(
      repository,
      'github:1',
      new Date('2026-06-02T19:05:00.000Z'),
    )

    expect(bundle.accounts.map(account => account.providerAccountRef)).toEqual([
      'provider-account_1',
      'provider-account_2',
    ])
  })

  test('createNew starts device login in a new provider account slot', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        id: 'provider_account_1',
        providerAccountRef: 'provider-account_1',
      }),
    )

    const result = await startChatGptCodexDeviceLogin(
      repository,
      { createNew: true, userId: 'github:1' },
      () =>
        Promise.resolve({
          deviceAuthId: 'device_auth_2',
          expiresAt: '2026-06-02T19:15:00.000Z',
          intervalSeconds: 5,
          userCode: 'WXYZ-9876',
          verificationUrl: CHATGPT_CODEX_VERIFICATION_URL,
        }),
      {
        makeId: makeIdFactory([
          'ref2',
          'account2',
          'attempt2',
          'login2',
          'event2',
        ]),
        now: () => new Date('2026-06-02T19:05:00.000Z'),
      },
    )

    expect(result.providerAccountRef).toBe('provider-account_ref_ref2')
    expect(
      repository.accounts.map(account => account.providerAccountRef),
    ).toEqual(['provider-account_1', 'provider-account_ref_ref2'])
  })

  test('disconnect clears usable secret refs and revokes issued grants', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))
    repository.grants.push(makeGrant({}))

    const account = await disconnectProviderAccountForUser(
      repository,
      'github:1',
      'provider-account_1',
      {
        makeId: makeIdFactory(['disconnect_event']),
        now: () => new Date('2026-06-02T19:10:00.000Z'),
      },
    )

    expect(account?.publicStatus).toBe('disconnected')
    expect(account?.hasSecretRef).toBe(false)
    expect(repository.accounts[0]?.secretRef).toBeNull()
    expect(repository.grants[0]?.status).toBe('revoked')
  })

  test('broker connected callback records account and attempt state idempotently', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        connectedAt: null,
        health: 'unknown',
        secretRef: null,
        status: 'pending',
      }),
    )
    repository.attempts.push(makeAttempt({}))

    const first = await recordDeviceLoginConnected(
      repository,
      {
        accountLabel: 'Primary ChatGPT',
        actorId: 'user_agent_broker',
        attemptId: 'provider_attempt_1',
        planType: 'pro',
        providerAccountRef: 'provider-account_1',
      },
      {
        makeId: makeIdFactory(['connected_event']),
        now: () => new Date('2026-06-02T19:05:00.000Z'),
      },
    )
    const second = await recordDeviceLoginConnected(
      repository,
      {
        actorId: 'user_agent_broker',
        attemptId: 'provider_attempt_1',
        providerAccountRef: 'provider-account_1',
      },
      {
        makeId: makeIdFactory(['unused_event']),
        now: () => new Date('2026-06-02T19:06:00.000Z'),
      },
    )

    expect(first?.account.publicStatus).toBe('connected')
    expect(first?.account.hasSecretRef).toBe(true)
    expect(first?.attempt.status).toBe('connected')
    expect(repository.attempts[0]?.userCode).toBeNull()
    expect(second?.account.publicStatus).toBe('connected')
    expect(repository.events.map(event => event.kind)).toEqual([
      'login_connected',
    ])
  })

  test('broker connected callback rejects stale and mismatched attempts', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        connectedAt: null,
        health: 'unknown',
        secretRef: null,
        status: 'pending',
      }),
    )
    repository.attempts.push(
      makeAttempt({ expiresAt: '2026-06-02T19:01:00.000Z' }),
    )

    await expect(
      recordDeviceLoginConnected(
        repository,
        {
          actorId: 'user_agent_broker',
          attemptId: 'provider_attempt_1',
        },
        {
          now: () => new Date('2026-06-02T19:02:00.000Z'),
        },
      ),
    ).rejects.toThrow(/expired/)

    await expect(
      recordDeviceLoginConnected(
        repository,
        {
          actorId: 'user_agent_broker',
          attemptId: 'provider_attempt_1',
          providerAccountRef: 'provider-account_wrong',
        },
        {
          now: () => new Date('2026-06-02T19:00:30.000Z'),
        },
      ),
    ).rejects.toThrow(/does not match/)
  })

  test('broker failed callback records denied state without credential leakage', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        connectedAt: null,
        health: 'unknown',
        secretRef: null,
        status: 'pending',
      }),
    )
    repository.attempts.push(makeAttempt({}))

    const result = await recordDeviceLoginFailed(
      repository,
      {
        actorId: 'user_agent_broker',
        attemptId: 'provider_attempt_1',
        reason: 'User denied login',
        status: 'denied',
      },
      {
        makeId: makeIdFactory(['failed_event']),
        now: () => new Date('2026-06-02T19:05:00.000Z'),
      },
    )

    expect(result?.account.publicStatus).toBe('denied')
    expect(result?.attempt.status).toBe('denied')
    expect(repository.events[0]?.kind).toBe('login_denied')
    expect(JSON.stringify(repository.events)).not.toContain('access_token')
  })

  test('broker health callback can mark requires reauth', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))

    const result = await recordProviderAccountHealth(
      repository,
      {
        actorId: 'user_agent_broker',
        health: 'requires_reauth',
        providerAccountRef: 'provider-account_1',
        reason: 'token_revoked',
      },
      {
        makeId: makeIdFactory(['health_event']),
        now: () => new Date('2026-06-02T19:20:00.000Z'),
      },
    )

    expect(result?.health).toBe('requires_reauth')
    expect(result?.publicStatus).toBe('unhealthy')
    expect(repository.events[0]?.kind).toBe('account_health_updated')
  })

  test('broker callbacks reject secret-shaped public metadata', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        connectedAt: null,
        health: 'unknown',
        secretRef: null,
        status: 'pending',
      }),
    )
    repository.attempts.push(makeAttempt({}))

    await expect(
      recordDeviceLoginConnected(
        repository,
        {
          accountLabel: '{"access_token":"secret"}',
          actorId: 'user_agent_broker',
          attemptId: 'provider_attempt_1',
        },
        {
          now: () => new Date('2026-06-02T19:05:00.000Z'),
        },
      ),
    ).rejects.toThrow(/credential-shaped/)
  })

  test('connected healthy account can issue a public grant', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))

    const grant = await issueProviderAccountGrant(
      repository,
      {
        providerAccountRef: 'provider-account_1',
        requestedAction: 'run_opencode',
        runnerSessionId: 'runner_session_1',
        threadId: 'thread_1',
        userId: 'github:1',
        workroomId: 'workroom_1',
      },
      {
        makeId: makeIdFactory(['grant_ref_1', 'grant_1', 'grant_event_1']),
        now: () => new Date('2026-06-02T19:00:00.000Z'),
      },
    )

    expect(grant?.status).toBe('issued')
    expect(grant?.grantRef).toBe('codex-auth-grant_grant_ref_grant_ref_1')
    expect(JSON.stringify(grant)).not.toContain('providerSecretRef')
    expect(repository.events[0]?.kind).toBe('auth_grant_issued')
  })

  test('grant issue rejects disconnected or reauth-required accounts', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(
      makeAccount({
        health: 'requires_reauth',
        status: 'unhealthy',
      }),
    )

    await expect(
      issueProviderAccountGrant(repository, {
        providerAccountRef: 'provider-account_1',
        userId: 'github:1',
      }),
    ).rejects.toThrow(/not connected and healthy/)

    await expect(
      issueProviderAccountGrant(repository, {
        providerAccountRef: 'provider-account_1',
        userId: 'github:1',
      }),
    ).rejects.toMatchObject({
      _tag: 'ProviderAccountNotConnectedHealthy',
      message: 'Provider account is not connected and healthy.',
    } satisfies Partial<ProviderAccountNotConnectedHealthy>)
  })

  test('fake SHC runner resolves a grant and receives only a redacted materialization plan', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(makeGrant({}))

    const resolved = await resolveProviderAccountGrant(
      repository,
      {
        actorId: 'user_agent_shc_runner',
        grantRef: 'codex-auth-grant_1',
        providerAccountRef: 'provider-account_1',
        runnerSessionId: 'runner_session_1',
      },
      {
        makeId: makeIdFactory(['used_event']),
        now: () => new Date('2026-06-02T19:10:00.000Z'),
      },
    )

    expect(resolved?.status).toBe('used')
    expect(resolved?.providerSecretRef).toBe('codex-auth://provider-account_1')
    expect(resolved?.materialization).toEqual(
      buildRedactedOpenCodeMaterializationPlan(
        'codex-auth://provider-account_1',
      ),
    )
    expect(JSON.stringify(resolved)).not.toContain('refresh_token')
    expect(JSON.stringify(resolved)).not.toContain('access_token')
    expect(repository.grants[0]?.status).toBe('used')
    await expect(
      resolveProviderAccountGrant(
        repository,
        {
          actorId: 'user_agent_shc_runner',
          grantRef: 'codex-auth-grant_1',
          runnerSessionId: 'runner_session_1',
        },
        {
          now: () => new Date('2026-06-02T19:11:00.000Z'),
        },
      ),
    ).rejects.toThrow(/not issued/)
    await expect(
      resolveProviderAccountGrant(
        repository,
        {
          actorId: 'user_agent_shc_runner',
          grantRef: 'codex-auth-grant_1',
          runnerSessionId: 'runner_session_1',
        },
        {
          now: () => new Date('2026-06-02T19:11:00.000Z'),
        },
      ),
    ).rejects.toMatchObject({
      _tag: 'ProviderGrantNotIssued',
      message: 'Grant is not issued.',
    } satisfies Partial<ProviderGrantNotIssued>)
  })

  test('grant resolve rejects expired and wrong-runner grants', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.grants.push(
      makeGrant({ expiresAt: '2026-06-02T19:01:00.000Z' }),
      makeGrant({
        id: 'provider_grant_2',
        grantRef: 'codex-auth-grant_2',
        runnerSessionId: 'runner_session_expected',
      }),
    )

    await expect(
      resolveProviderAccountGrant(
        repository,
        {
          actorId: 'user_agent_shc_runner',
          grantRef: 'codex-auth-grant_1',
        },
        {
          now: () => new Date('2026-06-02T19:02:00.000Z'),
        },
      ),
    ).rejects.toThrow(/expired/)

    await expect(
      resolveProviderAccountGrant(repository, {
        actorId: 'user_agent_shc_runner',
        grantRef: 'codex-auth-grant_2',
        runnerSessionId: 'runner_session_wrong',
      }),
    ).rejects.toThrow(/runner session/)
  })

  test('disconnect revokes outstanding issued grants', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))
    repository.grants.push(makeGrant({}))

    await disconnectProviderAccountForUser(
      repository,
      'github:1',
      'provider-account_1',
      {
        makeId: makeIdFactory(['disconnect_event_2']),
        now: () => new Date('2026-06-02T19:30:00.000Z'),
      },
    )

    expect(repository.grants[0]?.status).toBe('revoked')
    expect(repository.grants[0]?.revokedAt).toBe('2026-06-02T19:30:00.000Z')
  })

  test('repository service exposes persistence operations as Effects', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))
    const service = makeProviderAccountRepositoryService(repository)
    const accounts = await Effect.runPromise(
      service.listAccountsForUser('github:1'),
    )
    const account = await Effect.runPromise(
      service.findAccountByRef('github:1', 'provider-account_1'),
    )

    expect(accounts).toHaveLength(1)
    expect(account?.providerAccountRef).toBe('provider-account_1')
  })

  test('lifecycle service exposes grant authorization flow as Effects', async () => {
    const repository = new MemoryProviderAccountRepository()
    repository.accounts.push(makeAccount({}))
    const layer = makeProviderAccountLifecycleTestLayer({
      makeId: providerAccountTestIdFactory([
        'grant_ref',
        'grant',
        'grant_event',
      ]),
      now: providerAccountTestNow,
      repository,
    })

    const grant = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* ProviderAccountLifecycleService

        return yield* service.issueProviderAccountGrant({
          providerAccountRef: 'provider-account_1',
          runnerSessionId: 'runner_session_1',
          userId: 'github:1',
        })
      }).pipe(Effect.provide(layer)),
    )

    expect(grant?.grantRef).toBe('codex-auth-grant_grant_ref_grant_ref')
    expect(repository.grants[0]?.status).toBe('issued')
  })

  test('OpenAI Codex provider client reports start failures as tagged errors', async () => {
    const client = makeOpenAiCodexProviderClient(
      () => Promise.resolve(new Response('{}', { status: 503 })),
      () => new Date('2026-06-02T19:00:00.000Z'),
    )

    await expect(
      Effect.runPromise(client.startDeviceLogin()),
    ).rejects.toMatchObject({
      _tag: 'ProviderAccountClientRequestFailed',
      endpoint: 'deviceauth_usercode',
      message: 'ChatGPT/Codex device login start failed with 503.',
      status: 503,
    } satisfies Partial<ProviderAccountClientRequestFailed>)
  })

  test('OpenAI Codex provider client explains start rate limits', async () => {
    const client = makeOpenAiCodexProviderClient(
      () => Promise.resolve(new Response('{}', { status: 429 })),
      () => new Date('2026-06-02T19:00:00.000Z'),
    )

    await expect(
      Effect.runPromise(client.startDeviceLogin()),
    ).rejects.toMatchObject({
      _tag: 'ProviderAccountClientRequestFailed',
      endpoint: 'deviceauth_usercode',
      message:
        'OpenAI is rate limiting ChatGPT device login. Wait a minute, then try Reconnect ChatGPT again.',
      status: 429,
    } satisfies Partial<ProviderAccountClientRequestFailed>)
  })
})
