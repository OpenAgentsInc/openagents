import { describe, expect, it } from 'vitest'

import { makeBrowserSessionBoundary } from './session'

type TestUser = Readonly<{ userId: string }>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: {},
  waitUntil: () => undefined,
})

describe('browser session boundary', () => {
  it('returns undefined without persisting when verification fails', async () => {
    const persistedUsers: Array<TestUser> = []
    const boundary = makeBrowserSessionBoundary<
      TestUser,
      Record<string, never>
    >({
      persistUser: (_env, user) => {
        persistedUsers.push(user)

        return Promise.resolve()
      },
      verifySession: () => Promise.resolve(undefined),
    })

    await expect(
      boundary.requireBrowserSession(
        new Request('https://openagents.com/api/auth/session'),
        {},
        executionContext(),
      ),
    ).resolves.toBeUndefined()
    expect(persistedUsers).toEqual([])
  })

  it('persists and returns a verified browser session', async () => {
    const session = { user: { userId: 'github:1' } }
    const persistedUsers: Array<TestUser> = []
    const boundary = makeBrowserSessionBoundary<
      TestUser,
      Record<string, never>
    >({
      persistUser: (_env, user) => {
        persistedUsers.push(user)

        return Promise.resolve()
      },
      verifySession: () => Promise.resolve(session),
    })

    await expect(
      boundary.requireBrowserSession(
        new Request('https://openagents.com/api/auth/session'),
        {},
        executionContext(),
      ),
    ).resolves.toBe(session)
    expect(persistedUsers).toEqual([{ userId: 'github:1' }])
  })
})
