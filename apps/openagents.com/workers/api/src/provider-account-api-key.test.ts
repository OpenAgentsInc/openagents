import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ApiKeyConnectProvider,
  PROVIDER_API_KEY_CONNECT_POLICIES,
  connectProviderApiKeyAccount,
  probeProviderApiKey,
  providerApiKeyConnectPolicyForProvider,
  providerApiKeyConnectPolicyForRouteSegment,
  providerApiKeyHealthFromStatus,
  providerApiKeyUserSecretRef,
  requireProviderApiKeyShape,
} from './provider-account-api-key'
import type {
  ProviderAccountAuthGrantRecord,
  ProviderAccountEventRecord,
  ProviderAccountRecord,
  ProviderAccountRepository,
  ProviderConnectionAttemptRecord,
} from './provider-account-domain'
import {
  ProviderApiKeyInvalid,
  ProviderApiKeyRejected,
} from './provider-account-errors'
import { makeProviderAccountRoutes } from './provider-account-routes'
import {
  providerAccountTestIdFactory,
  providerAccountTestNow,
} from './test/service-fixtures'

const FAKE_ANTHROPIC_KEY = 'test-anthropic-key-not-real-0001'
const FAKE_GEMINI_KEY = 'test-gemini-key-not-real-0001'
const FAKE_OPENROUTER_KEY = 'sk-or-test-key-not-real-0001'

class MemoryRepository implements ProviderAccountRepository {
  readonly accounts: Array<ProviderAccountRecord> = []
  readonly attempts: Array<ProviderConnectionAttemptRecord> = []
  readonly events: Array<ProviderAccountEventRecord> = []
  readonly grants: Array<ProviderAccountAuthGrantRecord> = []

  findAccountByRef(userId: string, providerAccountRef: string) {
    return Promise.resolve(
      this.accounts.find(
        account =>
          account.userId === userId &&
          account.providerAccountRef === providerAccountRef &&
          account.deletedAt === null,
      ),
    )
  }

  findAccountByProviderAccountRef(providerAccountRef: string) {
    return Promise.resolve(
      this.accounts.find(
        account => account.providerAccountRef === providerAccountRef,
      ),
    )
  }

  findReusableAccount() {
    return Promise.resolve(undefined)
  }

  listAccountsForUser(userId: string) {
    return Promise.resolve(
      this.accounts.filter(account => account.userId === userId),
    )
  }

  listPendingAttemptsForUser() {
    return Promise.resolve([])
  }

  findAttemptForUser() {
    return Promise.resolve(undefined)
  }

  findAttemptById() {
    return Promise.resolve(undefined)
  }

  saveStartedDeviceLogin(
    account: ProviderAccountRecord,
    attempt: ProviderConnectionAttemptRecord,
    event: ProviderAccountEventRecord,
    accountAlreadyExists: boolean,
  ) {
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

  recordConnectedAttempt(account: ProviderAccountRecord) {
    return Promise.resolve(account)
  }

  recordFailedAttempt(account: ProviderAccountRecord) {
    return Promise.resolve(account)
  }

  recordAccountHealth() {
    return Promise.resolve(undefined)
  }

  createAuthGrant(grant: ProviderAccountAuthGrantRecord) {
    this.grants.push(grant)

    return Promise.resolve(grant)
  }

  findGrantByRef() {
    return Promise.resolve(undefined)
  }

  markGrantUsed(grant: ProviderAccountAuthGrantRecord) {
    return Promise.resolve(grant)
  }

  disconnectAccount() {
    return Promise.resolve(undefined)
  }
}

const recordedKeys: Array<string> = []

const dependencies = (probeStatus: number) => ({
  probeApiKey: probeProviderApiKey((() =>
    Promise.resolve(new Response('{}', { status: probeStatus }))) as never),
  storeConnectedApiKey: (
    input: Readonly<{
      providerAccountRef: string
      provider: ApiKeyConnectProvider
      apiKey: string
    }>,
  ) => {
    recordedKeys.push(input.apiKey)

    return Promise.resolve()
  },
})

const options = () => ({
  makeId: providerAccountTestIdFactory(['1', '2', '3', '4', '5']),
  now: providerAccountTestNow,
})

describe('provider api key connect policy', () => {
  test('exposes exactly the ToS-cleared API-key BYOK providers', () => {
    expect(
      PROVIDER_API_KEY_CONNECT_POLICIES.map(policy => policy.provider),
    ).toEqual(['openrouter', 'anthropic_claude', 'google_gemini'])
    expect(
      providerApiKeyConnectPolicyForRouteSegment('openrouter')?.provider,
    ).toBe('openrouter')
    expect(
      providerApiKeyConnectPolicyForRouteSegment('anthropic')?.provider,
    ).toBe('anthropic_claude')
    expect(
      providerApiKeyConnectPolicyForRouteSegment('google-gemini')?.provider,
    ).toBe('google_gemini')
    expect(
      providerApiKeyConnectPolicyForRouteSegment('chatgpt-codex'),
    ).toBeUndefined()
  })

  test('builds public-safe per-user secret refs', () => {
    const anthropic = providerApiKeyConnectPolicyForProvider('anthropic_claude')

    expect(
      providerApiKeyUserSecretRef(anthropic, 'provider-account_ref_1'),
    ).toBe('provider-account://anthropic/user-api-key/provider-account_ref_1')
  })

  test('rejects empty, whitespace, and oversized key shapes without echoing them', () => {
    for (const value of [
      undefined,
      '',
      '   ',
      'short',
      'two words',
      `k${'a'.repeat(600)}`,
    ]) {
      try {
        requireProviderApiKeyShape(value)
        expect.unreachable('expected ProviderApiKeyInvalid')
      } catch (error) {
        expect(error).toBeInstanceOf(ProviderApiKeyInvalid)
        expect(JSON.stringify(error)).not.toContain('two words')
      }
    }
  })

  test('classifies probe statuses into account health', () => {
    expect(providerApiKeyHealthFromStatus(200)).toBe('healthy')
    expect(providerApiKeyHealthFromStatus(429)).toBe('healthy')
    expect(providerApiKeyHealthFromStatus(401)).toBe('requires_reauth')
    expect(providerApiKeyHealthFromStatus(403)).toBe('requires_reauth')
    expect(providerApiKeyHealthFromStatus(404)).toBe('unhealthy')
    expect(providerApiKeyHealthFromStatus(500)).toBe('unknown')
  })

  test('probes the provider endpoint with the key header and never throws', async () => {
    const calls: Array<Readonly<{ url: string; headers: Headers }>> = []
    const probe = probeProviderApiKey(((
      url: string,
      init: RequestInit,
    ): Promise<Response> => {
      calls.push({ url, headers: new Headers(init.headers) })

      return Promise.resolve(new Response('{}', { status: 200 }))
    }) as never)
    const anthropic = providerApiKeyConnectPolicyForProvider('anthropic_claude')

    await expect(probe(anthropic, FAKE_ANTHROPIC_KEY)).resolves.toEqual({
      health: 'healthy',
      probeStatus: 200,
    })
    expect(calls[0]?.url).toContain('https://api.anthropic.com/v1/models')
    expect(calls[0]?.headers.get('x-api-key')).toBe(FAKE_ANTHROPIC_KEY)

    const failingProbe = probeProviderApiKey((() =>
      Promise.reject(new Error('network down'))) as never)

    await expect(failingProbe(anthropic, FAKE_ANTHROPIC_KEY)).resolves.toEqual({
      health: 'unknown',
      probeStatus: undefined,
    })
  })
})

describe('connectProviderApiKeyAccount', () => {
  test('connects an Anthropic account with a healthy probed key and no key material in records', async () => {
    const repository = new MemoryRepository()
    const result = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'anthropic_claude',
        apiKey: FAKE_ANTHROPIC_KEY,
        accountLabel: 'Personal Anthropic key',
      },
      dependencies(200),
      options(),
    )

    expect(result.account).toMatchObject({
      provider: 'anthropic_claude',
      authMode: 'api_key',
      status: 'connected',
      publicStatus: 'connected',
      health: 'healthy',
      hasSecretRef: true,
      accountLabel: 'Personal Anthropic key',
    })
    expect(result.attempt).toMatchObject({
      method: 'provider_api_key',
      source: 'browser_api_key',
      status: 'connected',
    })
    expect(result.generatedAt).toBe('2026-06-02T19:00:00.000Z')
    expect(repository.accounts[0]?.secretRef).toBe(
      `provider-account://anthropic/user-api-key/${result.providerAccountRef}`,
    )
    expect(repository.events[0]?.summary).toContain('Anthropic Claude')
    expect(JSON.stringify(result)).not.toContain(FAKE_ANTHROPIC_KEY)
    expect(JSON.stringify(repository.accounts)).not.toContain(
      FAKE_ANTHROPIC_KEY,
    )
    expect(JSON.stringify(repository.events)).not.toContain(FAKE_ANTHROPIC_KEY)
  })

  test('connects a Gemini account with user-scoped secret ref', async () => {
    const repository = new MemoryRepository()
    const result = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'google_gemini',
        apiKey: FAKE_GEMINI_KEY,
      },
      dependencies(200),
      options(),
    )

    expect(result.account).toMatchObject({
      provider: 'google_gemini',
      authMode: 'api_key',
      status: 'connected',
      health: 'healthy',
    })
    expect(repository.accounts[0]?.secretRef).toBe(
      `provider-account://google-gemini/user-api-key/${result.providerAccountRef}`,
    )
  })

  test('connects an OpenRouter account with user-scoped secret ref and redacted records', async () => {
    const repository = new MemoryRepository()
    const result = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'openrouter',
        apiKey: FAKE_OPENROUTER_KEY,
        accountLabel: 'Khala BYOK',
      },
      dependencies(200),
      options(),
    )

    expect(result.account).toMatchObject({
      provider: 'openrouter',
      authMode: 'api_key',
      status: 'connected',
      health: 'healthy',
      accountLabel: 'Khala BYOK',
    })
    expect(repository.accounts[0]?.secretRef).toBe(
      `provider-account://openrouter/user-api-key/${result.providerAccountRef}`,
    )
    expect(repository.events[0]?.summary).toContain('OpenRouter')
    expect(JSON.stringify(result)).not.toContain(FAKE_OPENROUTER_KEY)
    expect(JSON.stringify(repository.accounts)).not.toContain(FAKE_OPENROUTER_KEY)
    expect(JSON.stringify(repository.attempts)).not.toContain(FAKE_OPENROUTER_KEY)
    expect(JSON.stringify(repository.events)).not.toContain(FAKE_OPENROUTER_KEY)
  })

  test('refuses keys the provider rejects and stores nothing', async () => {
    const repository = new MemoryRepository()
    const keysBefore = recordedKeys.length

    await expect(
      connectProviderApiKeyAccount(
        repository,
        {
          userId: 'user_1',
          provider: 'anthropic_claude',
          apiKey: FAKE_ANTHROPIC_KEY,
        },
        dependencies(401),
        options(),
      ),
    ).rejects.toBeInstanceOf(ProviderApiKeyRejected)
    expect(repository.accounts).toHaveLength(0)
    expect(repository.events).toHaveLength(0)
    expect(recordedKeys.length).toBe(keysBefore)
  })

  test('connects with unknown health when the provider is unreachable, leaving the account unleasable until a health check passes', async () => {
    const repository = new MemoryRepository()
    const result = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'anthropic_claude',
        apiKey: FAKE_ANTHROPIC_KEY,
      },
      dependencies(503),
      options(),
    )

    expect(result.account).toMatchObject({
      status: 'connected',
      health: 'unknown',
    })
  })

  test('rotates the key on an existing account when providerAccountRef is supplied', async () => {
    const repository = new MemoryRepository()
    const first = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'anthropic_claude',
        apiKey: FAKE_ANTHROPIC_KEY,
      },
      dependencies(200),
      options(),
    )
    const rotated = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'anthropic_claude',
        apiKey: `${FAKE_ANTHROPIC_KEY}-rotated`,
        providerAccountRef: first.providerAccountRef,
      },
      dependencies(200),
      {
        makeId: providerAccountTestIdFactory(['9']),
        now: providerAccountTestNow,
      },
    )

    expect(rotated.providerAccountRef).toBe(first.providerAccountRef)
    expect(repository.accounts).toHaveLength(1)
  })

  test('refuses cross-provider providerAccountRef reuse', async () => {
    const repository = new MemoryRepository()
    const first = await connectProviderApiKeyAccount(
      repository,
      {
        userId: 'user_1',
        provider: 'anthropic_claude',
        apiKey: FAKE_ANTHROPIC_KEY,
      },
      dependencies(200),
      options(),
    )

    await expect(
      connectProviderApiKeyAccount(
        repository,
        {
          userId: 'user_1',
          provider: 'google_gemini',
          apiKey: FAKE_GEMINI_KEY,
          providerAccountRef: first.providerAccountRef,
        },
        dependencies(200),
        options(),
      ),
    ).rejects.toMatchObject({ _tag: 'ProviderAccountRefMismatch' })
  })
})

describe('provider api key connect route dispatch', () => {
  const stub = (name: string, calls: Array<string>) => () => {
    calls.push(name)

    return Effect.succeed(new Response(null, { status: 204 }))
  }

  const makeRouter = (calls: Array<string>) =>
    makeProviderAccountRoutes<Record<string, never>>({
      handleGitHubWriteDisconnectApi: stub('githubWriteDisconnect', calls),
      handleProviderAccountDisconnectApi: stub('disconnect', calls),
      handleProviderAccountGrantIssueApi: stub('grantIssue', calls),
      handleProviderAccountGrantResolveApi: stub('grantResolve', calls),
      handleGoogleGeminiGrantResolveApi: stub('geminiGrantResolve', calls),
      handleGoogleGeminiBuiltinGrantApi: stub('geminiBuiltinGrant', calls),
      handleGoogleGeminiGenerateContentApi: stub('geminiGenerate', calls),
      handleProviderAccountHealthApi: stub('health', calls),
      handleProviderApiKeyConnectApi: (
        _request,
        _env,
        _ctx,
        providerRouteSegment,
      ) => {
        calls.push(`apiKeyConnect:${providerRouteSegment}`)

        return Effect.succeed(new Response(null, { status: 204 }))
      },
      handleProviderAccountPoolApi: stub('pool', calls),
      handleProviderAccountUsageApi: stub('usage', calls),
      handleProviderAccountsListApi: stub('list', calls),
      handleProviderDeviceLoginConnectedApi: stub('deviceConnected', calls),
      handleProviderDeviceLoginFailedApi: stub('deviceFailed', calls),
      handleProviderDeviceLoginStartApi: stub('deviceStart', calls),
      handleProviderDeviceLoginStatusApi: stub('deviceStatus', calls),
      handlePylonProviderDeviceLoginStartApi: stub('pylonDeviceStart', calls),
      handlePylonProviderDeviceLoginStatusApi: stub('pylonDeviceStatus', calls),
      handlePylonProviderCodexAuthMaterialApi: stub(
        'pylonCodexAuthMaterial',
        calls,
      ),
      handlePylonProviderLocalCodexAuthImportApi: stub(
        'pylonLocalCodexAuthImport',
        calls,
      ),
      handlePylonOpenAgentsAuthStartApi: stub(
        'pylonOpenAgentsAuthStart',
        calls,
      ),
      handlePylonOpenAgentsAuthStatusApi: stub(
        'pylonOpenAgentsAuthStatus',
        calls,
      ),
      handlePylonOpenAgentsAuthVerifyApi: stub(
        'pylonOpenAgentsAuthVerify',
        calls,
      ),
      handleKhalaCodeOpenAgentsAuthStartApi: stub(
        'khalaCodeOpenAgentsAuthStart',
        calls,
      ),
      handleKhalaCodeOpenAgentsAuthStatusApi: stub(
        'khalaCodeOpenAgentsAuthStatus',
        calls,
      ),
      handleKhalaCodeOpenAgentsAuthVerifyApi: stub(
        'khalaCodeOpenAgentsAuthVerify',
        calls,
      ),
      handleMobileCodexAccountDisconnectApi: stub(
        'mobileCodexDisconnect',
        calls,
      ),
      handleMobileCodexDeviceLoginStatusApi: stub(
        'mobileCodexDeviceStatus',
        calls,
      ),
    })

  const ctx = {
    passThroughOnException: () => undefined,
    waitUntil: () => undefined,
  } as unknown as ExecutionContext

  test('routes OpenRouter, Anthropic, and Google Gemini connect to the API-key handler', async () => {
    const calls: Array<string> = []
    const router = makeRouter(calls)

    for (const segment of ['openrouter', 'anthropic', 'google-gemini']) {
      const effect = router.routeProviderAccountRequest(
        new Request(
          `https://openagents.com/api/provider-accounts/${segment}/connect`,
          { method: 'POST' },
        ),
        {},
        ctx,
      )

      expect(effect).toBeDefined()

      if (effect !== undefined) {
        await Effect.runPromise(effect)
      }
    }

    expect(calls).toEqual([
      'apiKeyConnect:openrouter',
      'apiKeyConnect:anthropic',
      'apiKeyConnect:google-gemini',
    ])
  })

  test('does not expose a connect route for providers without an API-key lane', () => {
    const calls: Array<string> = []
    const router = makeRouter(calls)
    const effect = router.routeProviderAccountRequest(
      new Request(
        'https://openagents.com/api/provider-accounts/chatgpt-codex/connect',
        { method: 'POST' },
      ),
      {},
      ctx,
    )

    expect(effect).toBeUndefined()
    expect(calls).toEqual([])
  })

  test('routes Pylon bearer-token device-login paths to Pylon handlers', async () => {
    const calls: Array<string> = []
    const router = makeRouter(calls)

    for (const path of [
      '/api/pylon/provider-accounts/chatgpt-codex/device-login/start',
      '/api/pylon/provider-accounts/chatgpt-codex/device-login/provider_attempt_1',
      '/api/pylon/provider-accounts/chatgpt-codex/local-auth/import',
    ]) {
      const effect = router.routeProviderAccountRequest(
        new Request(`https://openagents.com${path}`, { method: 'POST' }),
        {},
        ctx,
      )

      expect(effect).toBeDefined()

      if (effect !== undefined) {
        await Effect.runPromise(effect)
      }
    }

    expect(calls).toEqual([
      'pylonDeviceStart',
      'pylonDeviceStatus',
      'pylonLocalCodexAuthImport',
    ])
  })

  test('routes Pylon OpenAgents auth device-link paths to auth handlers', async () => {
    const calls: Array<string> = []
    const router = makeRouter(calls)

    for (const path of [
      '/api/pylon/auth/openagents/device/start',
      '/api/pylon/auth/openagents/device/verify?attempt=pylon_openauth_1&code=ABCD-EFGH',
      '/api/pylon/auth/openagents/device/pylon_openauth_1',
    ]) {
      const effect = router.routeProviderAccountRequest(
        new Request(`https://openagents.com${path}`, { method: 'POST' }),
        {},
        ctx,
      )

      expect(effect).toBeDefined()

      if (effect !== undefined) {
        await Effect.runPromise(effect)
      }
    }

    expect(calls).toEqual([
      'pylonOpenAgentsAuthStart',
      'pylonOpenAgentsAuthVerify',
      'pylonOpenAgentsAuthStatus',
    ])
  })

  test('routes Khala Code OpenAgents auth device paths to auth handlers', async () => {
    const calls: Array<string> = []
    const router = makeRouter(calls)

    for (const path of [
      '/api/khala-code/auth/openagents/device/start',
      '/api/khala-code/auth/openagents/device/verify?attempt=khala_code_desktop_openauth_1&code=ABCD-EFGH',
      '/api/khala-code/auth/openagents/device/khala_code_desktop_openauth_1',
    ]) {
      const effect = router.routeProviderAccountRequest(
        new Request(`https://openagents.com${path}`, { method: 'POST' }),
        {},
        ctx,
      )

      expect(effect).toBeDefined()

      if (effect !== undefined) {
        await Effect.runPromise(effect)
      }
    }

    expect(calls).toEqual([
      'khalaCodeOpenAgentsAuthStart',
      'khalaCodeOpenAgentsAuthVerify',
      'khalaCodeOpenAgentsAuthStatus',
    ])
  })

  test('routes mobile Codex account poll and disconnect paths to mobile handlers', async () => {
    const calls: Array<string> = []
    const router = makeRouter(calls)

    for (const path of [
      '/api/mobile/codex-accounts/device-login/provider_attempt_1',
      '/api/mobile/codex-accounts/provider-account_1/disconnect',
    ]) {
      const effect = router.routeProviderAccountRequest(
        new Request(`https://openagents.com${path}`, { method: 'POST' }),
        {},
        ctx,
      )

      expect(effect).toBeDefined()

      if (effect !== undefined) {
        await Effect.runPromise(effect)
      }
    }

    expect(calls).toEqual([
      'mobileCodexDeviceStatus',
      'mobileCodexDisconnect',
    ])
  })
})
