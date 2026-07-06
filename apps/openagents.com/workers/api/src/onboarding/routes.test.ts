import type { AuthKvStore } from '../auth/auth-kv'
import { Effect, Layer } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  GitHubRepositoryListFailed,
  GitHubRepositoryReadFailed,
  GitHubRepositoryService,
} from './github'
import { makeOnboardingRoutes } from './routes'
import type { OnboardingGitHubRepository } from './schema'

type TestSession = Readonly<{ user: Readonly<{ userId: string }> }>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

const rejectedD1 = (reason: string) => Promise.reject(new Error(reason))

type TestUserRow = Record<string, string | number | null>

class AccessDbStore {
  user: TestUserRow = {
    onboarding_billing_skipped_at: null,
    onboarding_completed_at: null,
    onboarding_goal: null,
    onboarding_repository_default_branch: null,
    onboarding_repository_description: null,
    onboarding_repository_full_name: null,
    onboarding_repository_html_url: null,
    onboarding_repository_id: null,
    onboarding_repository_name: null,
    onboarding_repository_owner: null,
    onboarding_repository_private: null,
    onboarding_repository_provider: null,
    onboarding_repository_selected_at: null,
    onboarding_repository_skipped_at: null,
    onboarding_step: 'repository',
    onboarding_updated_at: null,
    updated_at: '2026-06-04T12:00:00.000Z',
  }
}

class AccessStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly hasCoreTeamAccess: boolean,
    private readonly store: AccessDbStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM team_memberships')) {
      return Promise.resolve(
        this.hasCoreTeamAccess ? JSON.parse('{"allowed":1}') : null,
      )
    }

    if (this.query.includes('FROM users')) {
      return Promise.resolve(this.store.user as T)
    }

    return rejectedD1(`Unexpected D1 first: ${this.query}`)
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('UPDATE users')) {
      const [
        repositoryId,
        repositoryOwner,
        repositoryName,
        repositoryFullName,
        repositoryPrivate,
        repositoryDefaultBranch,
        repositoryHtmlUrl,
        repositoryDescription,
        repositorySelectedAt,
        onboardingUpdatedAt,
        updatedAt,
      ] = this.values

      this.store.user = {
        ...this.store.user,
        onboarding_repository_default_branch: repositoryDefaultBranch as string,
        onboarding_repository_description: repositoryDescription as
          | string
          | null,
        onboarding_repository_full_name: repositoryFullName as string,
        onboarding_repository_html_url: repositoryHtmlUrl as string,
        onboarding_repository_id: repositoryId as string,
        onboarding_repository_name: repositoryName as string,
        onboarding_repository_owner: repositoryOwner as string,
        onboarding_repository_private: repositoryPrivate as number,
        onboarding_repository_provider: 'github',
        onboarding_repository_selected_at: repositorySelectedAt as string,
        onboarding_repository_skipped_at: null,
        onboarding_step: this.query.includes("onboarding_step = 'goal'")
          ? 'goal'
          : (this.store.user.onboarding_step as string),
        onboarding_updated_at: onboardingUpdatedAt as string,
        updated_at: updatedAt as string,
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return rejectedD1(`Unexpected D1 run: ${this.query}`)
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return rejectedD1(`Unexpected D1 all: ${this.query}`)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return rejectedD1(`Unexpected D1 raw: ${this.query}`)
  }
}

const accessDb = (hasCoreTeamAccess: boolean): D1Database => {
  const store = new AccessDbStore()

  return {
    batch: () => Promise.reject(new Error('D1 batch should not be used')),
    dump: () => Promise.reject(new Error('D1 dump should not be used')),
    exec: () => Promise.reject(new Error('D1 exec should not be used')),
    prepare: query => new AccessStatement(query, hasCoreTeamAccess, store),
    withSession: () => {
      throw new Error('D1 session should not be used')
    },
  }
}

// CFG-3 (#8518): the github identity token lives in the auth KV store
// (Postgres KvStore in production; a fixed-value fake here).
const tokenStorage = (token: string | null = null): AuthKvStore => ({
  get: ((_key: string) => Promise.resolve(token)) as AuthKvStore['get'],
  put: () => Promise.resolve(),
  delete: () => Promise.resolve(),
  listPrefix: () => Promise.resolve([]),
})

const makeEnv = (
  hasCoreTeamAccess = true,
  githubToken: string | null = null,
) => ({
  AUTH_KV: tokenStorage(githubToken),
  OPENAGENTS_DB: accessDb(hasCoreTeamAccess),
})

const makeRoutes = (
  session: TestSession | null,
  mobileSession: TestSession | null = null,
  githubRepositoryServiceLayer?: Layer.Layer<GitHubRepositoryService>,
) =>
  makeOnboardingRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
    requireUserBearerSession: () => Promise.resolve(mobileSession ?? undefined),
    ...(githubRepositoryServiceLayer === undefined
      ? {}
      : { githubRepositoryServiceLayer }),
  })

const runRoute = (
  session: TestSession | null,
  request: Request,
  hasCoreTeamAccess = true,
  githubToken: string | null = null,
): Promise<Response> => {
  const route = makeRoutes(session).routeOnboardingRequest(
    request,
    makeEnv(hasCoreTeamAccess, githubToken),
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

const fakeRepository = (owner: string, name: string): OnboardingGitHubRepository => ({
  id: `${owner}/${name}`,
  provider: 'github',
  owner,
  name,
  fullName: `${owner}/${name}`,
  private: false,
  defaultBranch: 'main',
  htmlUrl: `https://github.com/${owner}/${name}`,
  description: null,
})

type FakeGitHubServiceOptions = Readonly<{
  repositories?: ReadonlyArray<OnboardingGitHubRepository>
  hasNextPage?: boolean
  unauthorized?: boolean
  notFound?: boolean
}>

const fakeGitHubRepositoryServiceLayer = (
  options: FakeGitHubServiceOptions = {},
): Layer.Layer<GitHubRepositoryService> =>
  Layer.succeed(GitHubRepositoryService, {
    getRepository: (_accessToken, owner, name) => {
      if (options.unauthorized) {
        return Effect.fail(
          new GitHubRepositoryReadFailed({ reason: 'unauthorized', status: 401 }),
        )
      }

      if (options.notFound) {
        return Effect.fail(
          new GitHubRepositoryReadFailed({ reason: 'not found', status: 404 }),
        )
      }

      return Effect.succeed(fakeRepository(owner, name))
    },
    listRepositories: () => Effect.succeed(options.repositories ?? []),
    listRepositoriesPage: (_accessToken, { page, perPage }) => {
      if (options.unauthorized) {
        return Effect.fail(
          new GitHubRepositoryListFailed({ reason: 'unauthorized', status: 401 }),
        )
      }

      return Effect.succeed({
        repositories: options.repositories ?? [],
        page,
        perPage,
        hasNextPage: options.hasNextPage ?? false,
      })
    },
  })

const runMobileRoute = (
  mobileSession: TestSession | null,
  request: Request,
  githubToken: string | null = null,
  githubOptions?: FakeGitHubServiceOptions,
): Promise<Response> => {
  const route = makeRoutes(
    null,
    mobileSession,
    fakeGitHubRepositoryServiceLayer(githubOptions),
  ).routeOnboardingRequest(request, makeEnv(true, githubToken), executionContext())

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

describe('onboarding API routes', () => {
  test('returns unauthorized without a browser session', async () => {
    const response = await runRoute(
      null,
      new Request('https://openagents.com/api/onboarding'),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('reports missing GitHub repository token without failing the list route', async () => {
    const response = await runRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/onboarding/repositories'),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    await expect(response.json()).resolves.toEqual({
      repositories: [],
      tokenStatus: 'missing',
    })
  })

  test('allows authenticated users without Core Team access through customer intake', async () => {
    const response = await runRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/onboarding/repositories'),
      false,
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      repositories: [],
      tokenStatus: 'missing',
    })
  })

  test('rejects list repository selection when the GitHub token is missing', async () => {
    const response = await runRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/onboarding/repository/select', {
        body: JSON.stringify({ repositoryId: 'repo_1' }),
        method: 'POST',
      }),
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'github_token_missing',
    })
  })

  test('accepts manually typed public repository refs without a GitHub token', async () => {
    const response = await runRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/onboarding/repository/select', {
        body: JSON.stringify({ owner: 'OpenAgentsInc', name: 'typed-repo' }),
        method: 'POST',
      }),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      onboarding: {
        repository: {
          _tag: 'RepositorySelected',
          repository: {
            defaultBranch: 'main',
            fullName: 'OpenAgentsInc/typed-repo',
            htmlUrl: 'https://github.com/OpenAgentsInc/typed-repo',
            id: 'OpenAgentsInc/typed-repo',
            private: false,
          },
        },
        step: 'goal',
      },
    })
  })
})

describe('mobile-bearer repo API (MM-B1, #8471)', () => {
  test('GET /api/mobile/repos rejects a request with no mobile bearer session', async () => {
    const response = await runMobileRoute(
      null,
      new Request('https://openagents.com/api/mobile/repos'),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('GET /api/mobile/repos/{owner}/{name} rejects a request with no mobile bearer session', async () => {
    const response = await runMobileRoute(
      null,
      new Request(
        'https://openagents.com/api/mobile/repos/OpenAgentsInc/openagents',
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('GET /api/mobile/repos returns a typed failure when the GitHub token is missing', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/mobile/repos'),
      null,
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'github_token_missing',
    })
  })

  test('GET /api/mobile/repos/{owner}/{name} returns a typed failure when the GitHub token is missing', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request(
        'https://openagents.com/api/mobile/repos/OpenAgentsInc/openagents',
      ),
      null,
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'github_token_missing',
    })
  })

  test('GET /api/mobile/repos returns a typed failure when GitHub rejects the stored token', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/mobile/repos'),
      'expired-token',
      { unauthorized: true },
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({
      error: 'github_token_expired',
    })
  })

  test('GET /api/mobile/repos/{owner}/{name} returns a typed failure when GitHub rejects the stored token', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request(
        'https://openagents.com/api/mobile/repos/OpenAgentsInc/openagents',
      ),
      'expired-token',
      { unauthorized: true },
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({
      error: 'github_token_expired',
    })
  })

  test('GET /api/mobile/repos/{owner}/{name} returns 404 when GitHub reports the repo is not found', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request(
        'https://openagents.com/api/mobile/repos/OpenAgentsInc/does-not-exist',
      ),
      'valid-token',
      { notFound: true },
    )

    expect(response.status).toBe(404)
    await expect(response.json()).resolves.toEqual({
      error: 'repository_not_found',
      repositoryId: 'OpenAgentsInc/does-not-exist',
    })
  })

  test('GET /api/mobile/repos lists repositories with default paging when the token is available', async () => {
    const repositories = [fakeRepository('OpenAgentsInc', 'openagents')]
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/mobile/repos'),
      'valid-token',
      { repositories, hasNextPage: true },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      repositories,
      page: 1,
      perPage: 100,
      hasNextPage: true,
    })
  })

  test('GET /api/mobile/repos forwards page/perPage query params to the GitHub page fetch', async () => {
    const repositories = [fakeRepository('OpenAgentsInc', 'openagents')]
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/mobile/repos?page=2&perPage=10'),
      'valid-token',
      { repositories, hasNextPage: false },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      repositories,
      page: 2,
      perPage: 10,
      hasNextPage: false,
    })
  })

  test('GET /api/mobile/repos rejects an out-of-range perPage', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request('https://openagents.com/api/mobile/repos?perPage=500'),
      'valid-token',
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'bad_request',
    })
  })

  test('GET /api/mobile/repos/{owner}/{name} fetches a single repository when the token is available', async () => {
    const response = await runMobileRoute(
      { user: { userId: 'github:1' } },
      new Request(
        'https://openagents.com/api/mobile/repos/OpenAgentsInc/openagents',
      ),
      'valid-token',
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      repository: fakeRepository('OpenAgentsInc', 'openagents'),
    })
  })
})
