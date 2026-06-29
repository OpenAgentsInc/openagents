import { Effect, Layer } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleProviderAccountsListEffect } from './provider-account-browser-routes'
import {
  ProviderAccountLifecycleService,
  type ProviderAccountLifecycleServiceShape,
} from './provider-account-service'

const unused = () => Effect.die('unused provider-account test service')

describe('provider account browser routes', () => {
  test('lists accounts through the lifecycle service layer', async () => {
    const calls: Array<string> = []
    const service: ProviderAccountLifecycleServiceShape = {
      disconnectProviderAccountForUser: () => unused(),
      issueProviderAccountGrant: () => unused(),
      listForUser: userId =>
        Effect.sync(() => {
          calls.push(userId)

          return { accounts: [], attempts: [] }
        }),
      recordDeviceLoginConnected: () => unused(),
      recordDeviceLoginFailed: () => unused(),
      recordProviderAccountHealth: () => unused(),
      refreshChatGptCodexDeviceLoginForUser: () => unused(),
      resolveProviderAccountGrant: () => unused(),
      startChatGptCodexDeviceLogin: () => unused(),
    }
    const layer = Layer.succeed(ProviderAccountLifecycleService, service)
    const dependencies = {
      appendRefreshedSessionCookies: (response: Response) => {
        response.headers.set('x-session-refreshed', 'true')

        return response
      },
      deleteStartedCodexDeviceLogin: () => async () => undefined,
      providerAuthSecretKey: (providerAccountRef: string) =>
        `provider-auth/${providerAccountRef}`,
      probeProviderApiKey: async () => ({
        health: 'healthy' as const,
        probeStatus: 200,
      }),
      readStartedCodexDeviceLogin: () => async () => undefined,
      requireBrowserSession: async () => ({ user: { userId: 'user_1' } }),
      storeConnectedCodexAuth: () => async () => 'codex-auth/provider_1',
      storeConnectedProviderApiKey: () => async () => undefined,
      storeStartedCodexDeviceLogin: () => async () => undefined,
    }
    const env = {
      AUTH_STORAGE: {},
      OPENAGENTS_DB: {},
    } as unknown as {
      AUTH_STORAGE: KVNamespace
      OPENAGENTS_DB: D1Database
    }
    const ctx = {} as ExecutionContext

    const response = await Effect.runPromise(
      handleProviderAccountsListEffect(
        new Request('https://openagents.com/api/provider-accounts'),
        env,
        ctx,
        dependencies,
      ).pipe(Effect.provide(layer)),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    expect(calls).toEqual(['user_1'])
    expect(await response.json()).toEqual({ accounts: [], attempts: [] })
  })
})
