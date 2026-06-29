import { Effect, Layer } from 'effect'
import { describe, expect, test } from 'vitest'

import { listProviderAccountsForBrowserSession } from './provider-account-browser-routes'
import {
  ProviderAccountLifecycleService,
  type ProviderAccountLifecycleServiceShape,
} from './provider-account-service'

const unimplemented = (operation: string) => () =>
  Effect.die(new Error(`${operation} was not expected in this test`))

describe('provider account browser route effects', () => {
  test('lists accounts through the lifecycle service layer', async () => {
    const calls: Array<string> = []
    const service = {
      disconnectProviderAccountForUser: unimplemented('disconnect'),
      issueProviderAccountGrant: unimplemented('issueGrant'),
      recordDeviceLoginConnected: unimplemented('recordConnected'),
      recordDeviceLoginFailed: unimplemented('recordFailed'),
      recordProviderAccountHealth: unimplemented('recordHealth'),
      refreshChatGptCodexDeviceLoginForUser: unimplemented('refreshDeviceLogin'),
      resolveProviderAccountGrant: unimplemented('resolveGrant'),
      startChatGptCodexDeviceLogin: unimplemented('startDeviceLogin'),
      listForUser: userId =>
        Effect.sync(() => {
          calls.push(userId)

          return {
            accounts: [],
            attempts: [],
          }
        }),
    } satisfies ProviderAccountLifecycleServiceShape
    const layer = Layer.succeed(ProviderAccountLifecycleService, service)
    const session = { user: { userId: 'github:effect-route-user' } }
    const response = await Effect.runPromise(
      listProviderAccountsForBrowserSession(session, response => {
        response.headers.set('set-cookie', 'session=refreshed')

        return response
      }).pipe(Effect.provide(layer)),
    )

    expect(calls).toEqual(['github:effect-route-user'])
    expect(response.headers.get('set-cookie')).toBe('session=refreshed')
    expect(await response.json()).toEqual({
      accounts: [],
      attempts: [],
    })
  })
})
