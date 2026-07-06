import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { type OnboardingGitHubRepository } from './schema'

const GitHubRepositoryOwner = S.Struct({
  login: S.String,
})

const GitHubRepositoryApiItem = S.Struct({
  id: S.Union([S.Number, S.String]),
  name: S.String,
  full_name: S.String,
  private: S.Boolean,
  owner: GitHubRepositoryOwner,
  default_branch: S.String,
  html_url: S.String,
  description: S.NullOr(S.String),
})

const GitHubRepositoryApiItems = S.Array(GitHubRepositoryApiItem)

type GitHubRepositoryApiItem = typeof GitHubRepositoryApiItem.Type

export class GitHubRepositoryListFailed extends S.TaggedErrorClass<GitHubRepositoryListFailed>()(
  'GitHubRepositoryListFailed',
  {
    reason: S.String,
    status: S.Number,
  },
) {}

export class GitHubRepositoryReadFailed extends S.TaggedErrorClass<GitHubRepositoryReadFailed>()(
  'GitHubRepositoryReadFailed',
  {
    reason: S.String,
    status: S.Number,
  },
) {}

export const githubIdentityTokenKey = (userId: string): string =>
  `github-identity:token:${userId}`

const githubHeaders = (accessToken: string): Record<string, string> => ({
  accept: 'application/vnd.github+json',
  authorization: `Bearer ${accessToken}`,
  'user-agent': 'OpenAgents',
  'x-github-api-version': '2022-11-28',
})

const repositoryFromApi = (
  repository: GitHubRepositoryApiItem,
): OnboardingGitHubRepository => ({
  id: String(repository.id),
  provider: 'github',
  owner: repository.owner.login,
  name: repository.name,
  fullName: repository.full_name,
  private: repository.private,
  defaultBranch: repository.default_branch,
  htmlUrl: repository.html_url,
  description: repository.description,
})

export const GITHUB_REPOSITORY_DEFAULT_PER_PAGE = 100
export const GITHUB_REPOSITORY_MAX_PER_PAGE = 100

export type GitHubRepositoryPage = Readonly<{
  repositories: ReadonlyArray<OnboardingGitHubRepository>
  page: number
  perPage: number
  hasNextPage: boolean
}>

/**
 * GitHub's `Link` response header carries `rel="next"` when another page
 * exists (RFC 8288). Parsing it (rather than guessing from result count) is
 * the correct signal: a short final page can still equal `perPage` when the
 * account happens to have exactly that many repositories.
 */
const hasNextPageLink = (linkHeader: string | null): boolean =>
  linkHeader !== null &&
  linkHeader.split(',').some(part => part.includes('rel="next"'))

const listRepositoriesPage = (
  accessToken: string,
  input: Readonly<{ page: number; perPage: number }>,
): Effect.Effect<
  GitHubRepositoryPage,
  GitHubRepositoryListFailed | S.SchemaError
> =>
  Effect.gen(function* () {
    const page = Math.max(1, Math.trunc(input.page))
    const perPage = Math.min(
      GITHUB_REPOSITORY_MAX_PER_PAGE,
      Math.max(1, Math.trunc(input.perPage)),
    )

    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `https://api.github.com/user/repos?per_page=${perPage}&page=${page}&sort=updated&affiliation=owner,collaborator,organization_member`,
          {
            headers: githubHeaders(accessToken),
          },
        ),
      catch: error =>
        new GitHubRepositoryListFailed({
          reason: error instanceof Error ? error.message : String(error),
          status: 0,
        }),
    })

    if (!response.ok) {
      return yield* new GitHubRepositoryListFailed({
        reason: `GitHub returned HTTP ${response.status}.`,
        status: response.status,
      })
    }

    const hasNextPage = hasNextPageLink(response.headers.get('link'))
    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error =>
        new GitHubRepositoryListFailed({
          reason: error instanceof Error ? error.message : String(error),
          status: 0,
        }),
    })
    const repositories = yield* S.decodeUnknownEffect(GitHubRepositoryApiItems)(
      payload,
    )

    return {
      repositories: repositories.map(repositoryFromApi),
      page,
      perPage,
      hasNextPage,
    }
  })

const listRepositories = (
  accessToken: string,
): Effect.Effect<
  ReadonlyArray<OnboardingGitHubRepository>,
  GitHubRepositoryListFailed | S.SchemaError
> =>
  listRepositoriesPage(accessToken, {
    page: 1,
    perPage: GITHUB_REPOSITORY_DEFAULT_PER_PAGE,
  }).pipe(Effect.map(result => result.repositories))

const getRepository = (
  accessToken: string,
  owner: string,
  name: string,
): Effect.Effect<
  OnboardingGitHubRepository,
  GitHubRepositoryReadFailed | S.SchemaError
> =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`,
          {
            headers: githubHeaders(accessToken),
          },
        ),
      catch: error =>
        new GitHubRepositoryReadFailed({
          reason: error instanceof Error ? error.message : String(error),
          status: 0,
        }),
    })

    if (!response.ok) {
      return yield* new GitHubRepositoryReadFailed({
        reason: `GitHub returned HTTP ${response.status}.`,
        status: response.status,
      })
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error =>
        new GitHubRepositoryReadFailed({
          reason: error instanceof Error ? error.message : String(error),
          status: 0,
        }),
    })
    const repository = yield* S.decodeUnknownEffect(GitHubRepositoryApiItem)(
      payload,
    )

    return repositoryFromApi(repository)
  })

export class GitHubRepositoryService extends Context.Service<
  GitHubRepositoryService,
  {
    readonly getRepository: (
      accessToken: string,
      owner: string,
      name: string,
    ) => Effect.Effect<
      OnboardingGitHubRepository,
      GitHubRepositoryReadFailed | S.SchemaError
    >
    readonly listRepositories: (
      accessToken: string,
    ) => Effect.Effect<
      ReadonlyArray<OnboardingGitHubRepository>,
      GitHubRepositoryListFailed | S.SchemaError
    >
    readonly listRepositoriesPage: (
      accessToken: string,
      input: Readonly<{ page: number; perPage: number }>,
    ) => Effect.Effect<
      GitHubRepositoryPage,
      GitHubRepositoryListFailed | S.SchemaError
    >
  }
>()('@openagentsinc/autopilot-omega/GitHubRepositoryService') {
  static readonly layer = Layer.succeed(GitHubRepositoryService, {
    getRepository: Effect.fn('GitHubRepositoryService.getRepository')(
      getRepository,
    ),
    listRepositories: Effect.fn('GitHubRepositoryService.listRepositories')(
      listRepositories,
    ),
    listRepositoriesPage: Effect.fn(
      'GitHubRepositoryService.listRepositoriesPage',
    )(listRepositoriesPage),
  })
}
