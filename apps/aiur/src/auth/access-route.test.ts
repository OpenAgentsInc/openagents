import { describe, expect, test } from 'vitest'

import { handleAiurAccessRequest } from './access-route'
import { AIUR_ACCESS_COOKIE } from './cookies'
import type { AiurAuthClientLike } from './session'

const fakeUser = (userId: string) => ({
  userId,
  provider: 'github' as const,
  login: 'octocat',
  email: 'octocat@example.com',
  name: 'Octo Cat',
  avatarUrl: 'https://example.com/a.png',
})

const fakeClient = (userId: string | undefined): AiurAuthClientLike => ({
  verify: (async () => {
    if (userId === undefined) return { err: { message: 'invalid' } }
    return {
      err: undefined,
      subject: { type: 'user', properties: fakeUser(userId) },
      tokens: undefined,
    }
  }) as never,
})

describe('handleAiurAccessRequest', () => {
  test('signed_out with no cookie', async () => {
    const response = await handleAiurAccessRequest(
      new Request('https://aiur.openagents.com/api/aiur/access'),
      {},
      { client: fakeClient(undefined) },
    )
    expect(await response.json()).toEqual({ kind: 'signed_out' })
  })

  test('denied when session is valid but not allow-listed', async () => {
    const response = await handleAiurAccessRequest(
      new Request('https://aiur.openagents.com/api/aiur/access', {
        headers: { cookie: `${AIUR_ACCESS_COOKIE}=valid` },
      }),
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      { client: fakeClient('user_intruder') },
    )
    const body = (await response.json()) as { kind: string }
    expect(body.kind).toBe('denied')
  })

  test('owner when allow-listed', async () => {
    const response = await handleAiurAccessRequest(
      new Request('https://aiur.openagents.com/api/aiur/access', {
        headers: { cookie: `${AIUR_ACCESS_COOKIE}=valid` },
      }),
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      { client: fakeClient('user_owner') },
    )
    const body = (await response.json()) as { kind: string }
    expect(body.kind).toBe('owner')
  })
})
