import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  GITHUB_SCM_AUTH_BROKER_PATH,
  type GitHubScmAuthBrokerDependencies,
  githubScmAuthBrokerRepositoryRef,
  handleGitHubScmAuthorizationCallback,
  routeGitHubScmAuthBrokerRequest,
} from './github-scm-auth-broker-routes'
import {
  GITHUB_SCM_AUTHORIZATION_PATH,
  type GitHubScmAuthorizationService,
  type GitHubScmRepositoryGrant,
} from './github-scm-authorization'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

const userId = 'user_123'
const githubToken = 'oauth-token-for-user-123'
const opaqueGrantReference = 'github-scm-grant:0123456789abcdef0123456789abcdef'

const unusedAuthorization: GitHubScmAuthorizationService = {
  begin: () => Effect.die('unused'),
  callback: () => Effect.die('unused'),
  complete: () => Effect.die('unused'),
  readGrant: () =>
    Effect.sync((): GitHubScmRepositoryGrant | undefined => undefined),
}

const validBody = {
  schema: 'openagents.pylon.git_credential_broker_request.v1',
  helperRef: 'helper.pylon.scm_auth_broker.git_credential.v1',
  repositoryRef: githubScmAuthBrokerRepositoryRef(
    'OpenAgentsInc',
    'private-sum-fixture',
  ),
  authRefs: [`github-identity:token:${userId}`],
  protocol: 'https',
  host: 'github.com',
  path: '/OpenAgentsInc/private-sum-fixture.git',
}

const request = (body: unknown = validBody, init: RequestInit = {}): Request =>
  new Request(`https://openagents.com${GITHUB_SCM_AUTH_BROKER_PATH}`, {
    body: JSON.stringify(body),
    headers: { authorization: 'Bearer oa_agent_test' },
    method: 'POST',
    ...init,
  })

const baseDeps = (
  overrides: Partial<GitHubScmAuthBrokerDependencies> = {},
): GitHubScmAuthBrokerDependencies => ({
  authorization: unusedAuthorization,
  authenticate: () => Effect.succeed({ userId }),
  credentialTtlSeconds: 60,
  nowEpochMillis: () => Date.parse('2026-07-06T12:00:00.000Z'),
  readGithubAccessToken: () => Effect.succeed(githubToken),
  verifyRepositoryAccess: ({ owner, name }) =>
    Effect.succeed({
      ok: true as const,
      fullName: `${owner}/${name}`,
      private: true,
    }),
  ...overrides,
})

describe('GitHub SCM auth broker route', () => {
  test('falls through for unrelated paths', () => {
    expect(
      routeGitHubScmAuthBrokerRequest(
        new Request('https://openagents.com/api/elsewhere'),
        baseDeps(),
      ),
    ).toBeUndefined()
  })

  test('requires POST', async () => {
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        new Request(`https://openagents.com${GITHUB_SCM_AUTH_BROKER_PATH}`, {
          method: 'GET',
        }),
        baseDeps(),
      )!,
    )
    expect(response.status).toBe(405)
  })

  test('starts the browser OAuth leg for an allowed One origin', async () => {
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        new Request(
          `https://openagents.com${GITHUB_SCM_AUTHORIZATION_PATH}?workspace=one_workspace_example&returnTo=${encodeURIComponent('https://one.openagents.com/')}`,
        ),
        baseDeps({
          authorization: {
            ...unusedAuthorization,
            begin: () =>
              Effect.succeed(
                'https://github.com/login/oauth/authorize?state=safe-state',
              ),
          },
        }),
      )!,
    )
    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe(
      'https://github.com/login/oauth/authorize?state=safe-state',
    )
  })

  test('renders repository selection without credential material', async () => {
    const response = await run(
      handleGitHubScmAuthorizationCallback(
        new Request(
          'https://auth.openagents.com/github/callback?state=github_scm_state_example&code=provider-code',
        ),
        {
          ...unusedAuthorization,
          callback: () =>
            Effect.succeed({
              selectionRef:
                'github_scm_selection_0123456789abcdef0123456789abcdef',
              repositories: [{ fullName: 'OpenAgentsInc/one', private: true }],
            }),
        },
      ),
    )
    expect(response.status).toBe(200)
    const html = await response.text()
    expect(html).toContain('OpenAgentsInc/one')
    expect(html).toContain(GITHUB_SCM_AUTHORIZATION_PATH)
    expect(html).not.toContain(githubToken)
    expect(response.headers.get('cache-control')).toContain('no-store')
  })

  test('returns only repository and opaque grant to One after confirmation', async () => {
    const form = new FormData()
    form.set(
      'selectionRef',
      'github_scm_selection_0123456789abcdef0123456789abcdef',
    )
    form.set('repository', 'OpenAgentsInc/one')
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        new Request(`https://openagents.com${GITHUB_SCM_AUTHORIZATION_PATH}`, {
          method: 'POST',
          body: form,
        }),
        baseDeps({
          authorization: {
            ...unusedAuthorization,
            complete: () =>
              Effect.succeed({
                grantReference: opaqueGrantReference,
                repository: 'OpenAgentsInc/one',
                returnTo: 'https://one.openagents.com/',
              }),
          },
        }),
      )!,
    )
    expect(response.status).toBe(303)
    const location = response.headers.get('location') ?? ''
    expect(location).toContain('repoRepository=OpenAgentsInc%2Fone')
    expect(location).toContain(
      'repoGrantReference=github-scm-grant%3A0123456789abcdef0123456789abcdef',
    )
    expect(location).not.toContain(githubToken)
  })

  test('requires an executor agent bearer', async () => {
    let tokenRead = false
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        new Request(`https://openagents.com${GITHUB_SCM_AUTH_BROKER_PATH}`, {
          body: JSON.stringify(validBody),
          headers: { authorization: 'Bearer mobile_user_token' },
          method: 'POST',
        }),
        baseDeps({
          authenticate: () =>
            Effect.sync((): { userId: string } | undefined => undefined),
          readGithubAccessToken: () =>
            Effect.sync(() => {
              tokenRead = true
              return githubToken
            }),
        }),
      )!,
    )
    expect(response.status).toBe(401)
    expect(tokenRead).toBe(false)
  })

  test('serves a bounded Git credential for the authenticated user repo', async () => {
    const response = await run(
      routeGitHubScmAuthBrokerRequest(request(), baseDeps())!,
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')
    await expect(response.json()).resolves.toEqual({
      username: 'x-access-token',
      password: githubToken,
      expiresAt: '2026-07-06T12:01:00.000Z',
    })
  })

  test('resolves an opaque per-user grant independently of the executor owner', async () => {
    const grantUserId = 'github-user-456'
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request({ ...validBody, authRefs: [opaqueGrantReference] }),
        baseDeps({
          authenticate: () => Effect.succeed({ userId: 'executor-owner' }),
          authorization: {
            ...unusedAuthorization,
            readGrant: reference =>
              Effect.succeed(
                reference === opaqueGrantReference
                  ? {
                      userId: grantUserId,
                      repository: 'OpenAgentsInc/private-sum-fixture',
                    }
                  : undefined,
              ),
          },
          readGithubAccessToken: requestedUserId =>
            Effect.succeed(
              requestedUserId === grantUserId ? githubToken : undefined,
            ),
        }),
      )!,
    )
    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      password: githubToken,
    })
  })

  test('denies an opaque grant bound to another repository', async () => {
    let tokenRead = false
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request({ ...validBody, authRefs: [opaqueGrantReference] }),
        baseDeps({
          authorization: {
            ...unusedAuthorization,
            readGrant: () =>
              Effect.succeed({
                userId: 'github-user-456',
                repository: 'OpenAgentsInc/other-repo',
              }),
          },
          readGithubAccessToken: () =>
            Effect.sync(() => {
              tokenRead = true
              return githubToken
            }),
        }),
      )!,
    )
    expect(response.status).toBe(403)
    expect(tokenRead).toBe(false)
  })

  test('denies a mismatched repository ref before reading the stored token', async () => {
    let tokenRead = false
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request({
          ...validBody,
          repositoryRef: githubScmAuthBrokerRepositoryRef(
            'OpenAgentsInc',
            'other-repo',
          ),
        }),
        baseDeps({
          readGithubAccessToken: () =>
            Effect.sync(() => {
              tokenRead = true
              return githubToken
            }),
        }),
      )!,
    )
    expect(response.status).toBe(403)
    expect(tokenRead).toBe(false)
  })

  test('denies another user token ref before reading the stored token', async () => {
    let tokenRead = false
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request({
          ...validBody,
          authRefs: ['github-identity:token:other_user'],
        }),
        baseDeps({
          readGithubAccessToken: () =>
            Effect.sync(() => {
              tokenRead = true
              return githubToken
            }),
        }),
      )!,
    )
    expect(response.status).toBe(403)
    expect(tokenRead).toBe(false)
  })

  test('denies non-github scope without echoing token material', async () => {
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request({
          ...validBody,
          host: 'evil.example',
        }),
        baseDeps(),
      )!,
    )
    expect(response.status).toBe(403)
    const body = await response.text()
    expect(body).toContain('github_scm_scope_denied')
    expect(body).not.toContain(githubToken)
  })

  test('fails closed when the stored GitHub identity token is missing', async () => {
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request(),
        baseDeps({
          readGithubAccessToken: () =>
            Effect.sync((): string | undefined => undefined),
        }),
      )!,
    )
    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'github_identity_token_missing',
    })
  })

  test('denies repositories GitHub says the user cannot read', async () => {
    const response = await run(
      routeGitHubScmAuthBrokerRequest(
        request(),
        baseDeps({
          verifyRepositoryAccess: () =>
            Effect.succeed({ ok: false as const, status: 404 }),
        }),
      )!,
    )
    expect(response.status).toBe(403)
    const body = await response.text()
    expect(body).toContain('github_scm_scope_denied')
    expect(body).not.toContain(githubToken)
  })
})
