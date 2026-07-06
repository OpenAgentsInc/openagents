import { describe, expect, test } from 'vitest'

import { resolveAiurAccess } from './access'
import { AIUR_ACCESS_COOKIE } from './cookies'
import type { AiurAuthClientLike } from './session'

const fakeUser = (userId: string) => ({
  userId,
  provider: 'github' as const,
  githubId: '123',
  login: 'octocat',
  email: 'octocat@example.com',
  name: 'Octo Cat',
  avatarUrl: 'https://example.com/a.png',
})

const fakeClient = (
  behavior:
    | { ok: true; userId: string }
    | { ok: false },
): AiurAuthClientLike => ({
  verify: (async () => {
    if (!behavior.ok) {
      return { err: { message: 'invalid' } } as never
    }

    return {
      err: undefined,
      subject: { type: 'user', properties: fakeUser(behavior.userId) },
      tokens: undefined,
    } as never
  }) as never,
})

const requestWithCookie = (cookie: string | undefined): Request =>
  new Request('https://aiur.openagents.com/', {
    headers: cookie === undefined ? {} : { cookie },
  })

describe('resolveAiurAccess (fail-closed matrix)', () => {
  test('no session cookie => signed_out', async () => {
    const access = await resolveAiurAccess(
      requestWithCookie(undefined),
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      { client: fakeClient({ ok: true, userId: 'user_owner' }) },
    )
    expect(access.kind).toBe('signed_out')
  })

  test('invalid/expired session token => signed_out, never throws', async () => {
    const access = await resolveAiurAccess(
      requestWithCookie(`${AIUR_ACCESS_COOKIE}=garbage`),
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      { client: fakeClient({ ok: false }) },
    )
    expect(access.kind).toBe('signed_out')
  })

  test('valid session, but allowlist unset => denied (fail closed, never allow-all)', async () => {
    const access = await resolveAiurAccess(
      requestWithCookie(`${AIUR_ACCESS_COOKIE}=validtoken`),
      {},
      { client: fakeClient({ ok: true, userId: 'user_owner' }) },
    )
    expect(access.kind).toBe('denied')
  })

  test('valid session, non-owner user id => denied', async () => {
    const access = await resolveAiurAccess(
      requestWithCookie(`${AIUR_ACCESS_COOKIE}=validtoken`),
      { AIUR_OWNER_USER_IDS: 'user_owner' },
      { client: fakeClient({ ok: true, userId: 'user_intruder' }) },
    )
    expect(access.kind).toBe('denied')
    if (access.kind === 'denied') {
      expect(access.user.userId).toBe('user_intruder')
    }
  })

  test('valid session, owner user id in allowlist => owner', async () => {
    const access = await resolveAiurAccess(
      requestWithCookie(`${AIUR_ACCESS_COOKIE}=validtoken`),
      { AIUR_OWNER_USER_IDS: 'user_owner,user_second_owner' },
      { client: fakeClient({ ok: true, userId: 'user_owner' }) },
    )
    expect(access.kind).toBe('owner')
  })
})
