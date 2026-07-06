import { Effect, Schema as S } from 'effect'

import { readAgentBearerToken } from './auth/bearer-token'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

export const GITHUB_SCM_AUTH_BROKER_PATH =
  '/api/pylon/github/git-credentials'

export const GITHUB_SCM_AUTH_BROKER_REQUEST_SCHEMA =
  'openagents.pylon.git_credential_broker_request.v1'
export const GITHUB_SCM_AUTH_BROKER_HELPER_REF =
  'helper.pylon.scm_auth_broker.git_credential.v1'

export const GITHUB_SCM_AUTH_BROKER_DEFAULT_TTL_SECONDS = 5 * 60
export const GITHUB_SCM_AUTH_BROKER_MAX_TTL_SECONDS = 10 * 60

export class GitHubScmAuthBrokerDependencyFailed extends S.TaggedErrorClass<GitHubScmAuthBrokerDependencyFailed>()(
  'GitHubScmAuthBrokerDependencyFailed',
  {
    reason: S.String,
  },
) {}

const GitCredentialBrokerRequest = S.Struct({
  schema: S.Literal(GITHUB_SCM_AUTH_BROKER_REQUEST_SCHEMA),
  helperRef: S.Literal(GITHUB_SCM_AUTH_BROKER_HELPER_REF),
  repositoryRef: S.String,
  authRefs: S.Array(S.String),
  protocol: S.Literal('https'),
  host: S.String,
  path: S.String,
})

type GitCredentialBrokerRequest =
  typeof GitCredentialBrokerRequest.Type

export type GitHubScmAuthBrokerSession = Readonly<{
  userId: string
}>

export type GitHubScmAuthBrokerRepositoryAccess =
  | Readonly<{
      ok: true
      fullName: string
      private: boolean
    }>
  | Readonly<{
      ok: false
      status: number
    }>

export type GitHubScmAuthBrokerDependencies = Readonly<{
  authenticate: (
    request: Request,
  ) => Effect.Effect<GitHubScmAuthBrokerSession | undefined, unknown>
  readGithubAccessToken: (
    userId: string,
  ) => Effect.Effect<string | undefined, unknown>
  verifyRepositoryAccess: (
    input: Readonly<{
      accessToken: string
      owner: string
      name: string
    }>,
  ) => Effect.Effect<GitHubScmAuthBrokerRepositoryAccess, unknown>
  credentialTtlSeconds?: number
  nowEpochMillis?: () => number
}>

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const publicRefPattern = /^[A-Za-z0-9_.:/=@+-]{1,200}$/
const rawCredentialMaterialPattern =
  /(bearer\s+|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|password=|secret|token_value|credential_value|sk-[A-Za-z0-9_-]{16,})/i

export const githubScmAuthBrokerRepositoryRef = (
  owner: string,
  name: string,
): string => `repo.github/${owner}/${name}`

const noStoreBrokerJson = (
  value: unknown,
  init: ResponseInit = {},
) => noStoreJsonResponse(value, init)

const decodeBody = (
  value: unknown,
): GitCredentialBrokerRequest | undefined => {
  try {
    return S.decodeUnknownSync(GitCredentialBrokerRequest)(value)
  } catch {
    return undefined
  }
}

const parseGithubRepositoryPath = (
  path: string,
): Readonly<{ owner: string; name: string; fullName: string }> | undefined => {
  const normalized = `/${path.replace(/^\/+/, '')}`
  if (normalized.includes('..')) return undefined
  const match = /^\/([^/]+)\/([^/]+?)(?:\.git)?$/.exec(normalized)
  if (match === null) return undefined
  const owner = match[1]!
  const name = match[2]!
  const fullName = `${owner}/${name}`
  if (!githubFullNamePattern.test(fullName)) return undefined
  return { owner, name, fullName }
}

const safePublicRef = (value: string): boolean =>
  publicRefPattern.test(value) && !rawCredentialMaterialPattern.test(value)

const ttlSecondsFrom = (value: number | undefined): number => {
  if (
    value === undefined ||
    !Number.isInteger(value) ||
    value <= 0 ||
    value > GITHUB_SCM_AUTH_BROKER_MAX_TTL_SECONDS
  ) {
    return GITHUB_SCM_AUTH_BROKER_DEFAULT_TTL_SECONDS
  }
  return value
}

const responseExpiresAt = (deps: GitHubScmAuthBrokerDependencies): string =>
  epochMillisToIsoTimestamp(
    (deps.nowEpochMillis?.() ?? currentEpochMillis()) +
      ttlSecondsFrom(deps.credentialTtlSeconds) * 1000,
  )

const scopeDenied = () =>
  noStoreBrokerJson(
    {
      error: 'github_scm_scope_denied',
    },
    { status: 403 },
  )

const repositoryAccessDenied = (
  access: Extract<GitHubScmAuthBrokerRepositoryAccess, { ok: false }>,
) => {
  if (access.status === 401) {
    return noStoreBrokerJson(
      { error: 'github_identity_token_invalid' },
      { status: 401 },
    )
  }
  if (access.status === 0 || access.status >= 500) {
    return noStoreBrokerJson(
      { error: 'github_repository_verification_unavailable' },
      { status: 503 },
    )
  }
  return scopeDenied()
}

export const handleGitHubScmAuthBrokerRequest = (
  request: Request,
  deps: GitHubScmAuthBrokerDependencies,
) =>
  Effect.gen(function* () {
    const session = yield* deps
      .authenticate(request)
      .pipe(
        Effect.catch(() =>
          Effect.sync((): GitHubScmAuthBrokerSession | undefined => undefined),
        ),
      )
    if (session === undefined) {
      return noStoreBrokerJson(
        { error: 'unauthorized' },
        { headers: { 'www-authenticate': 'Bearer' }, status: 401 },
      )
    }

    const rawBody = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: () =>
        new GitHubScmAuthBrokerDependencyFailed({
          reason: 'invalid_json',
        }),
    }).pipe(Effect.catch(() => Effect.sync((): unknown => undefined)))
    if (rawBody === undefined) {
      return noStoreBrokerJson({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodeBody(rawBody)
    if (body === undefined) {
      return noStoreBrokerJson({ error: 'invalid_request' }, { status: 400 })
    }
    if (
      body.host.toLowerCase() !== 'github.com' ||
      body.authRefs.length === 0 ||
      body.authRefs.length > 8 ||
      !body.authRefs.every(safePublicRef) ||
      !safePublicRef(body.repositoryRef)
    ) {
      return scopeDenied()
    }

    const repository = parseGithubRepositoryPath(body.path)
    if (repository === undefined) {
      return scopeDenied()
    }

    const expectedRepositoryRef = githubScmAuthBrokerRepositoryRef(
      repository.owner,
      repository.name,
    )
    const expectedGithubTokenRef = `github-identity:token:${session.userId}`
    if (
      body.repositoryRef.toLowerCase() !== expectedRepositoryRef.toLowerCase() ||
      !body.authRefs.includes(expectedGithubTokenRef)
    ) {
      return scopeDenied()
    }

    const accessToken = yield* deps
      .readGithubAccessToken(session.userId)
      .pipe(
        Effect.catch(() =>
          Effect.sync((): string | undefined => undefined),
        ),
      )
    if (accessToken === undefined || accessToken.trim() === '') {
      return noStoreBrokerJson(
        { error: 'github_identity_token_missing' },
        { status: 409 },
      )
    }

    const access = yield* deps
      .verifyRepositoryAccess({
        accessToken,
        owner: repository.owner,
        name: repository.name,
      })
      .pipe(
        Effect.catch(() =>
          Effect.succeed({ ok: false as const, status: 0 }),
        ),
      )
    if (!access.ok) {
      return repositoryAccessDenied(access)
    }
    if (access.fullName.toLowerCase() !== repository.fullName.toLowerCase()) {
      return scopeDenied()
    }

    return noStoreBrokerJson({
      username: 'x-access-token',
      password: accessToken,
      expiresAt: responseExpiresAt(deps),
    })
  })

export const routeGitHubScmAuthBrokerRequest = (
  request: Request,
  deps: GitHubScmAuthBrokerDependencies,
) => {
  const url = new URL(request.url)
  if (url.pathname !== GITHUB_SCM_AUTH_BROKER_PATH) {
    return undefined
  }
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }
  if (readAgentBearerToken(request) === undefined) {
    return Effect.succeed(
      noStoreBrokerJson(
        { error: 'unauthorized' },
        { headers: { 'www-authenticate': 'Bearer' }, status: 401 },
      ),
    )
  }
  return handleGitHubScmAuthBrokerRequest(request, deps)
}
