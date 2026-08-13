import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeMemoryAuthKvStore } from './auth/auth-kv'
import {
  type GitHubScmAuthorizationDependencies,
  makeGitHubScmAuthorizationService,
} from './github-scm-authorization'
import { githubIdentityTokenKey } from './onboarding/github'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

const token = 'oauth-token-that-never-enters-one'
const uuids = [
  '11111111-1111-4111-8111-111111111111',
  '22222222-2222-4222-8222-222222222222',
  '33333333-3333-4333-8333-333333333333',
]

const makeFixture = (
  overrides: Partial<GitHubScmAuthorizationDependencies> = {},
) => {
  const storage = makeMemoryAuthKvStore()
  let uuidIndex = 0
  const service = makeGitHubScmAuthorizationService({
    storage,
    authorizationUrl: state =>
      `https://github.com/login/oauth/authorize?state=${encodeURIComponent(state)}`,
    exchangeOAuthCode: () =>
      Effect.succeed({
        accessToken: token,
        userId: 'github-user-123',
        repositories: [
          { fullName: 'OpenAgentsInc/one', private: true },
          { fullName: 'OpenAgentsInc/public-fixture', private: false },
        ],
      }),
    verifyRepositoryAccess: ({ owner, name }) =>
      Effect.succeed({ ok: true as const, fullName: `${owner}/${name}` }),
    nowIsoTimestamp: () => '2026-08-13T06:00:00.000Z',
    newUuid: () => uuids[uuidIndex++]!,
    ...overrides,
  })
  return { service, storage }
}

describe('GitHub SCM authorization authority', () => {
  test('mints an opaque exact-repository grant without returning the token', async () => {
    const { service, storage } = makeFixture()
    const authorizeUrl = await run(
      service.begin(
        'one_workspace_01kzvv22gk4gf9rrgzndfbpqzn',
        'https://one.openagents.com/',
      ),
    )
    expect(authorizeUrl).toContain(
      'state=github_scm_state_11111111111141118111111111111111',
    )
    expect(authorizeUrl).not.toContain(token)

    const callback = await run(
      service.callback(
        'github_scm_state_11111111111141118111111111111111',
        'single-use-code',
      ),
    )
    expect(callback.selectionRef).toBe(
      'github_scm_selection_22222222222242228222222222222222',
    )
    expect(JSON.stringify(callback)).not.toContain(token)

    const completion = await run(
      service.complete(callback.selectionRef, 'OpenAgentsInc/one'),
    )
    expect(completion).toEqual({
      grantReference: 'github-scm-grant:33333333333343338333333333333333',
      repository: 'OpenAgentsInc/one',
      returnTo: 'https://one.openagents.com/',
    })
    expect(JSON.stringify(completion)).not.toContain(token)
    await expect(
      run(service.readGrant(completion.grantReference)),
    ).resolves.toEqual({
      userId: 'github-user-123',
      repository: 'OpenAgentsInc/one',
    })
    await expect(
      storage.get(githubIdentityTokenKey('github-user-123')),
    ).resolves.toBe(token)
  })

  test('rejects a foreign return origin before creating OAuth state', async () => {
    const { service } = makeFixture()
    await expect(
      run(
        service.begin(
          'one_workspace_01kzvv22gk4gf9rrgzndfbpqzn',
          'https://evil.example/',
        ),
      ),
    ).rejects.toMatchObject({ reason: 'invalid_authorization_request' })
  })

  test('selection is one-time and cannot be replayed', async () => {
    const { service } = makeFixture()
    await run(
      service.begin(
        'one_workspace_01kzvv22gk4gf9rrgzndfbpqzn',
        'https://openagents.com/',
      ),
    )
    const callback = await run(
      service.callback(
        'github_scm_state_11111111111141118111111111111111',
        'single-use-code',
      ),
    )
    await run(service.complete(callback.selectionRef, 'OpenAgentsInc/one'))
    await expect(
      run(service.complete(callback.selectionRef, 'OpenAgentsInc/one')),
    ).rejects.toMatchObject({ reason: 'repository_selection_consumed' })
  })

  test('cannot retarget a selection to an unlisted repository', async () => {
    const { service } = makeFixture()
    await run(
      service.begin(
        'one_workspace_01kzvv22gk4gf9rrgzndfbpqzn',
        'https://one.openagents.com/',
      ),
    )
    const callback = await run(
      service.callback(
        'github_scm_state_11111111111141118111111111111111',
        'single-use-code',
      ),
    )
    await expect(
      run(service.complete(callback.selectionRef, 'OpenAgentsInc/not-listed')),
    ).rejects.toMatchObject({ reason: 'repository_not_selectable' })
  })
})
