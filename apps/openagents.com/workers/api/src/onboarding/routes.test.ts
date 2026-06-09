import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOnboardingRoutes } from './routes'

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

class MissingTokenStorage implements KVNamespace {
  constructor(private readonly token: string | null = null) {}

  get(
    key: string,
    options?: Partial<KVNamespaceGetOptions<undefined>>,
  ): Promise<string | null>
  get(key: string, type: 'text'): Promise<string | null>
  get<ExpectedValue = unknown>(
    key: string,
    type: 'json',
  ): Promise<ExpectedValue | null>
  get(key: string, type: 'arrayBuffer'): Promise<ArrayBuffer | null>
  get(key: string, type: 'stream'): Promise<ReadableStream | null>
  get(
    key: string,
    options?: KVNamespaceGetOptions<'text'>,
  ): Promise<string | null>
  get<ExpectedValue = unknown>(
    key: string,
    options?: KVNamespaceGetOptions<'json'>,
  ): Promise<ExpectedValue | null>
  get(
    key: string,
    options?: KVNamespaceGetOptions<'arrayBuffer'>,
  ): Promise<ArrayBuffer | null>
  get(
    key: string,
    options?: KVNamespaceGetOptions<'stream'>,
  ): Promise<ReadableStream | null>
  get(key: Array<string>, type: 'text'): Promise<Map<string, string | null>>
  get<ExpectedValue = unknown>(
    key: Array<string>,
    type: 'json',
  ): Promise<Map<string, ExpectedValue | null>>
  get(
    key: Array<string>,
    options?: Partial<KVNamespaceGetOptions<undefined>>,
  ): Promise<Map<string, string | null>>
  get(
    key: Array<string>,
    options?: KVNamespaceGetOptions<'text'>,
  ): Promise<Map<string, string | null>>
  get<ExpectedValue = unknown>(
    key: Array<string>,
    options?: KVNamespaceGetOptions<'json'>,
  ): Promise<Map<string, ExpectedValue | null>>
  get(key: string | Array<string>): Promise<unknown> {
    if (Array.isArray(key)) {
      return Promise.resolve(new Map(key.map(item => [item, this.token])))
    }

    return Promise.resolve(this.token)
  }

  getWithMetadata<Metadata = unknown>(
    key: string,
    options?: Partial<KVNamespaceGetOptions<undefined>>,
  ): Promise<KVNamespaceGetWithMetadataResult<string, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: string,
    type: 'text',
  ): Promise<KVNamespaceGetWithMetadataResult<string, Metadata>>
  getWithMetadata<ExpectedValue = unknown, Metadata = unknown>(
    key: string,
    type: 'json',
  ): Promise<KVNamespaceGetWithMetadataResult<ExpectedValue, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: string,
    type: 'arrayBuffer',
  ): Promise<KVNamespaceGetWithMetadataResult<ArrayBuffer, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: string,
    type: 'stream',
  ): Promise<KVNamespaceGetWithMetadataResult<ReadableStream, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: string,
    options: KVNamespaceGetOptions<'text'>,
  ): Promise<KVNamespaceGetWithMetadataResult<string, Metadata>>
  getWithMetadata<ExpectedValue = unknown, Metadata = unknown>(
    key: string,
    options: KVNamespaceGetOptions<'json'>,
  ): Promise<KVNamespaceGetWithMetadataResult<ExpectedValue, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: string,
    options: KVNamespaceGetOptions<'arrayBuffer'>,
  ): Promise<KVNamespaceGetWithMetadataResult<ArrayBuffer, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: string,
    options: KVNamespaceGetOptions<'stream'>,
  ): Promise<KVNamespaceGetWithMetadataResult<ReadableStream, Metadata>>
  getWithMetadata<Metadata = unknown>(
    key: Array<string>,
    type: 'text',
  ): Promise<Map<string, KVNamespaceGetWithMetadataResult<string, Metadata>>>
  getWithMetadata<ExpectedValue = unknown, Metadata = unknown>(
    key: Array<string>,
    type: 'json',
  ): Promise<
    Map<string, KVNamespaceGetWithMetadataResult<ExpectedValue, Metadata>>
  >
  getWithMetadata<Metadata = unknown>(
    key: Array<string>,
    options?: Partial<KVNamespaceGetOptions<undefined>>,
  ): Promise<Map<string, KVNamespaceGetWithMetadataResult<string, Metadata>>>
  getWithMetadata<Metadata = unknown>(
    key: Array<string>,
    options?: KVNamespaceGetOptions<'text'>,
  ): Promise<Map<string, KVNamespaceGetWithMetadataResult<string, Metadata>>>
  getWithMetadata<ExpectedValue = unknown, Metadata = unknown>(
    key: Array<string>,
    options?: KVNamespaceGetOptions<'json'>,
  ): Promise<
    Map<string, KVNamespaceGetWithMetadataResult<ExpectedValue, Metadata>>
  >
  getWithMetadata(key: string | Array<string>): Promise<unknown> {
    const missing = {
      cacheStatus: null,
      metadata: null,
      value: null,
    }

    if (Array.isArray(key)) {
      return Promise.resolve(new Map(key.map(item => [item, missing])))
    }

    return Promise.resolve(missing)
  }

  delete(): Promise<void> {
    return Promise.resolve()
  }

  list<Metadata = unknown>(): Promise<KVNamespaceListResult<Metadata, string>> {
    return Promise.resolve({
      cacheStatus: null,
      keys: [],
      list_complete: true,
    })
  }

  put(): Promise<void> {
    return Promise.resolve()
  }
}

const tokenStorage = (token: string | null = null): KVNamespace =>
  new MissingTokenStorage(token)

const makeEnv = (
  hasCoreTeamAccess = true,
  githubToken: string | null = null,
) => ({
  AUTH_STORAGE: tokenStorage(githubToken),
  OPENAGENTS_DB: accessDb(hasCoreTeamAccess),
})

const makeRoutes = (session: TestSession | null) =>
  makeOnboardingRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
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
