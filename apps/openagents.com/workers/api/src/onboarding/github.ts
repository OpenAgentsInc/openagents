import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  type MobileGitHubToolRequest,
  type OnboardingGitHubRepository,
} from './schema'

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

export class GitHubRepositoryToolFailed extends S.TaggedErrorClass<GitHubRepositoryToolFailed>()(
  'GitHubRepositoryToolFailed',
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

const GitHubContentFile = S.Struct({
  type: S.Literal('file'),
  name: S.String,
  path: S.String,
  sha: S.String,
  size: S.Number,
  encoding: S.String,
  content: S.String,
  html_url: S.NullOr(S.String),
})

const GitHubContentEntry = S.Struct({
  type: S.Literals(['file', 'dir', 'symlink', 'submodule']),
  name: S.String,
  path: S.String,
  sha: S.String,
  size: S.Number,
  html_url: S.NullOr(S.String),
})

const GitHubContentResponse = S.Union([GitHubContentFile, S.Array(GitHubContentEntry)])

const GitHubIssueApiItem = S.Struct({
  number: S.Number,
  title: S.String,
  state: S.String,
  html_url: S.String,
  body: S.NullOr(S.String),
  pull_request: S.optional(S.Unknown),
})

const GitHubIssueApiItems = S.Array(GitHubIssueApiItem)

const GitHubCreatedIssue = S.Struct({
  number: S.Number,
  title: S.String,
  state: S.String,
  html_url: S.String,
})

const GitHubGitRef = S.Struct({
  ref: S.String,
  object: S.Struct({ sha: S.String }),
})

const GitHubFileCommit = S.Struct({
  content: S.NullOr(
    S.Struct({
      path: S.String,
      sha: S.String,
      html_url: S.NullOr(S.String),
    }),
  ),
  commit: S.Struct({
    sha: S.String,
    html_url: S.NullOr(S.String),
  }),
})

const GitHubPullRequest = S.Struct({
  number: S.Number,
  title: S.String,
  state: S.String,
  draft: S.Boolean,
  html_url: S.String,
})

const githubApiRequest = (
  accessToken: string,
  path: string,
  init: RequestInit = {},
): Effect.Effect<unknown, GitHubRepositoryToolFailed> =>
  Effect.tryPromise({
    try: async () => {
      const response = await fetch(`https://api.github.com${path}`, {
        ...init,
        headers: {
          ...githubHeaders(accessToken),
          'content-type': 'application/json',
          ...init.headers,
        },
      })
      const payload = await response.json().catch(() => undefined)
      if (!response.ok) {
        const message =
          typeof payload === 'object' &&
          payload !== null &&
          'message' in payload &&
          typeof payload.message === 'string'
            ? payload.message
            : `GitHub returned HTTP ${response.status}.`
        throw new GitHubRepositoryToolFailed({
          reason: message,
          status: response.status,
        })
      }
      return payload
    },
    catch: (error) =>
      error instanceof GitHubRepositoryToolFailed
        ? error
        : new GitHubRepositoryToolFailed({
            reason: error instanceof Error ? error.message : String(error),
            status: 0,
          }),
  })

const repositoryPath = (owner: string, name: string): string =>
  `/repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`

const decodeBase64Utf8 = (value: string): string => {
  const binary = atob(value.replaceAll('\n', ''))
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0))
  return new TextDecoder().decode(bytes)
}

const encodeBase64Utf8 = (value: string): string => {
  const bytes = new TextEncoder().encode(value)
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += 32_768) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 32_768))
  }
  return btoa(binary)
}

const executeTool = (
  accessToken: string,
  input: MobileGitHubToolRequest,
): Effect.Effect<unknown, GitHubRepositoryToolFailed | S.SchemaError> =>
  Effect.gen(function* () {
    const repo = repositoryPath(input.owner, input.name)

    if (input.action === 'get_contents') {
      const path = (input.path ?? '').split('/').filter(Boolean).map(encodeURIComponent).join('/')
      const query = input.ref ? `?ref=${encodeURIComponent(input.ref)}` : ''
      const payload = yield* githubApiRequest(accessToken, `${repo}/contents/${path}${query}`)
      const decoded = yield* S.decodeUnknownEffect(GitHubContentResponse)(payload)
      if ('content' in decoded) {
        const content =
          decoded.encoding === 'base64' ? decodeBase64Utf8(decoded.content) : decoded.content
        return {
          type: 'file',
          name: decoded.name,
          path: decoded.path,
          sha: decoded.sha,
          size: decoded.size,
          content: content.slice(0, 200_000),
          truncated: content.length > 200_000,
          url: decoded.html_url,
        }
      }
      return {
        type: 'directory',
        entries: decoded.map((entry) => ({
          type: entry.type,
          name: entry.name,
          path: entry.path,
          sha: entry.sha,
          size: entry.size,
          url: entry.html_url,
        })),
      }
    }

    if (input.action === 'list_issues') {
      const payload = yield* githubApiRequest(
        accessToken,
        `${repo}/issues?state=${input.state ?? 'open'}&per_page=${input.limit ?? 30}&sort=updated`,
      )
      const issues = yield* S.decodeUnknownEffect(GitHubIssueApiItems)(payload)
      return {
        issues: issues
          .filter((issue) => issue.pull_request === undefined)
          .map((issue) => ({
            number: issue.number,
            title: issue.title,
            state: issue.state,
            body: issue.body,
            url: issue.html_url,
          })),
      }
    }

    if (input.action === 'create_issue') {
      const payload = yield* githubApiRequest(accessToken, `${repo}/issues`, {
        method: 'POST',
        body: JSON.stringify({ title: input.title, body: input.body ?? '' }),
      })
      const issue = yield* S.decodeUnknownEffect(GitHubCreatedIssue)(payload)
      return {
        number: issue.number,
        title: issue.title,
        state: issue.state,
        url: issue.html_url,
      }
    }

    if (input.action === 'create_branch') {
      if (input.branch === input.fromRef) {
        return yield* new GitHubRepositoryToolFailed({
          reason: 'The new branch must differ from its source.',
          status: 400,
        })
      }
      const repository = yield* getRepository(accessToken, input.owner, input.name).pipe(
        Effect.mapError(
          (error) =>
            new GitHubRepositoryToolFailed({
              reason:
                error instanceof GitHubRepositoryReadFailed
                  ? error.reason
                  : 'GitHub repository response was invalid.',
              status: error instanceof GitHubRepositoryReadFailed ? error.status : 502,
            }),
        ),
      )
      if (input.branch === repository.defaultBranch) {
        return yield* new GitHubRepositoryToolFailed({
          reason: 'The default branch cannot be created or overwritten.',
          status: 400,
        })
      }
      const sourcePayload = yield* githubApiRequest(
        accessToken,
        `${repo}/git/ref/heads/${encodeURIComponent(input.fromRef)}`,
      )
      const source = yield* S.decodeUnknownEffect(GitHubGitRef)(sourcePayload)
      const createdPayload = yield* githubApiRequest(accessToken, `${repo}/git/refs`, {
        method: 'POST',
        body: JSON.stringify({
          ref: `refs/heads/${input.branch}`,
          sha: source.object.sha,
        }),
      })
      const created = yield* S.decodeUnknownEffect(GitHubGitRef)(createdPayload)
      return { branch: created.ref.replace(/^refs\/heads\//, ''), sha: created.object.sha }
    }

    if (input.action === 'upsert_file') {
      if (/^\.github\/workflows(?:\/|$)/i.test(input.path)) {
        return yield* new GitHubRepositoryToolFailed({
          reason: 'GitHub Actions workflow files cannot be modified.',
          status: 403,
        })
      }
      const repository = yield* getRepository(accessToken, input.owner, input.name).pipe(
        Effect.mapError(
          (error) =>
            new GitHubRepositoryToolFailed({
              reason:
                error instanceof GitHubRepositoryReadFailed
                  ? error.reason
                  : 'GitHub repository response was invalid.',
              status: error instanceof GitHubRepositoryReadFailed ? error.status : 502,
            }),
        ),
      )
      if (input.branch === repository.defaultBranch) {
        return yield* new GitHubRepositoryToolFailed({
          reason: 'Files cannot be written directly to the default branch.',
          status: 403,
        })
      }
      const payload = yield* githubApiRequest(
        accessToken,
        `${repo}/contents/${input.path
          .split('/')
          .filter(Boolean)
          .map(encodeURIComponent)
          .join('/')}`,
        {
          method: 'PUT',
          body: JSON.stringify({
            message: input.message,
            content: encodeBase64Utf8(input.content),
            branch: input.branch,
            ...(input.sha ? { sha: input.sha } : {}),
          }),
        },
      )
      const committed = yield* S.decodeUnknownEffect(GitHubFileCommit)(payload)
      return {
        path: committed.content?.path ?? input.path,
        contentSha: committed.content?.sha ?? null,
        commitSha: committed.commit.sha,
        url: committed.content?.html_url ?? committed.commit.html_url,
      }
    }

    const payload = yield* githubApiRequest(accessToken, `${repo}/pulls`, {
      method: 'POST',
      body: JSON.stringify({
        title: input.title,
        body: input.body ?? '',
        head: input.head,
        base: input.base,
        draft: input.draft ?? false,
      }),
    })
    const pullRequest = yield* S.decodeUnknownEffect(GitHubPullRequest)(payload)
    return {
      number: pullRequest.number,
      title: pullRequest.title,
      state: pullRequest.state,
      draft: pullRequest.draft,
      url: pullRequest.html_url,
    }
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
    readonly executeTool: (
      accessToken: string,
      input: MobileGitHubToolRequest,
    ) => Effect.Effect<unknown, GitHubRepositoryToolFailed | S.SchemaError>
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
    executeTool: Effect.fn('GitHubRepositoryService.executeTool')(executeTool),
  })
}
