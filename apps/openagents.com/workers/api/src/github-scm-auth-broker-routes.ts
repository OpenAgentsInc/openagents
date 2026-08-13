import { Effect, Schema as S } from 'effect'

import { readAgentBearerToken } from './auth/bearer-token'
import {
  GITHUB_SCM_AUTHORIZATION_PATH,
  type GitHubScmAuthorizationService,
} from './github-scm-authorization'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

export const GITHUB_SCM_AUTH_BROKER_PATH = '/api/pylon/github/git-credentials'

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

type GitCredentialBrokerRequest = typeof GitCredentialBrokerRequest.Type

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
  authorization: GitHubScmAuthorizationService
  credentialTtlSeconds?: number
  nowEpochMillis?: () => number
}>

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const publicRefPattern = /^[A-Za-z0-9_.:/=@+-]{1,200}$/
const rawCredentialMaterialPattern =
  /(bearer\s+|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|password=|secret|token_value|credential_value|sk-[A-Za-z0-9_-]{16,})/i
const opaqueGrantPattern = /^github-scm-grant:[0-9a-f]{32}$/

export const githubScmAuthBrokerRepositoryRef = (
  owner: string,
  name: string,
): string => `repo.github/${owner}/${name}`

const noStoreBrokerJson = (value: unknown, init: ResponseInit = {}) =>
  noStoreJsonResponse(value, init)

const decodeBody = (value: unknown): GitCredentialBrokerRequest | undefined => {
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
    if (
      body.repositoryRef.toLowerCase() !== expectedRepositoryRef.toLowerCase()
    ) {
      return scopeDenied()
    }

    const grantReferences = body.authRefs.filter(reference =>
      opaqueGrantPattern.test(reference),
    )
    let tokenOwnerUserId = session.userId
    if (grantReferences.length > 0) {
      if (grantReferences.length !== 1) return scopeDenied()
      const grant = yield* deps.authorization.readGrant(grantReferences[0]!)
      if (
        grant === undefined ||
        grant.repository.toLowerCase() !== repository.fullName.toLowerCase()
      ) {
        return scopeDenied()
      }
      tokenOwnerUserId = grant.userId
    } else {
      const expectedGithubTokenRef = `github-identity:token:${session.userId}`
      if (!body.authRefs.includes(expectedGithubTokenRef)) {
        return scopeDenied()
      }
    }

    const accessToken = yield* deps
      .readGithubAccessToken(tokenOwnerUserId)
      .pipe(
        Effect.catch(() => Effect.sync((): string | undefined => undefined)),
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
        Effect.catch(() => Effect.succeed({ ok: false as const, status: 0 })),
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

const noStoreHtml = (html: string, status = 200) =>
  new Response(html, {
    status,
    headers: {
      'cache-control': 'no-store',
      'content-security-policy':
        "default-src 'none'; style-src 'unsafe-inline'; form-action https://openagents.com; base-uri 'none'; frame-ancestors 'none'",
      'content-type': 'text/html; charset=utf-8',
      'referrer-policy': 'no-referrer',
      'x-content-type-options': 'nosniff',
      'x-frame-options': 'DENY',
    },
  })

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const authorizationErrorPage = () =>
  noStoreHtml(
    '<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>GitHub connection failed</title><body><main><h1>GitHub connection failed</h1><p>No repository access was stored. Return to One and try again.</p></main></body></html>',
    400,
  )

const repositorySelectionPage = (
  selectionRef: string,
  repositories: ReadonlyArray<Readonly<{ fullName: string; private: boolean }>>,
) => {
  const options = repositories
    .map(
      repository =>
        `<option value="${escapeHtml(repository.fullName)}">${escapeHtml(repository.fullName)}${repository.private ? ' · private' : ' · public'}</option>`,
    )
    .join('')
  return noStoreHtml(`<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Connect a GitHub repository to One</title>
  <style>body{margin:0;background:#07111f;color:#edf4ff;font:16px/1.5 system-ui,sans-serif}main{max-width:42rem;margin:10vh auto;padding:2rem}section{background:#0d1b2d;border:1px solid #263b58;border-radius:16px;padding:2rem}label{display:block;font-weight:650;margin:1.5rem 0 .5rem}select,button{box-sizing:border-box;width:100%;font:inherit;border-radius:10px;padding:.85rem}select{background:#081422;color:#edf4ff;border:1px solid #47617e}button{margin-top:1.5rem;background:#6aa9ff;color:#03101f;border:0;font-weight:750;cursor:pointer}ul{padding-left:1.25rem;color:#b8c9dd}.note{color:#91a7bf;font-size:.925rem}</style>
</head>
<body><main><section>
  <h1>Connect a GitHub repository to One</h1>
  <p>Select the repository Sarah may use, then confirm the closed launch grant.</p>
  <form method="post" action="https://openagents.com${GITHUB_SCM_AUTHORIZATION_PATH}">
    <input type="hidden" name="selectionRef" value="${escapeHtml(selectionRef)}">
    <label for="repository">Repository</label>
    <select id="repository" name="repository" required>${options}</select>
    <p>One repository only, with:</p>
    <ul><li>Contents · read and write</li><li>Pull requests · read and write</li><li>Metadata · read</li></ul>
    <p class="note">GitHub credentials stay with the external SCM broker. One receives only an opaque repository-scoped grant reference.</p>
    <button type="submit">Confirm and return to One</button>
  </form>
</section></main></body></html>`)
}

export const handleGitHubScmAuthorizationCallback = (
  request: Request,
  authorization: GitHubScmAuthorizationService,
) =>
  Effect.gen(function* () {
    const url = new URL(request.url)
    const state = url.searchParams.get('state')?.trim() ?? ''
    const code = url.searchParams.get('code')?.trim() ?? ''
    if (
      url.searchParams.has('error') ||
      !state.startsWith('github_scm_state_') ||
      code === ''
    ) {
      return authorizationErrorPage()
    }
    const result = yield* authorization
      .callback(state, code)
      .pipe(Effect.catch(() => Effect.sync((): undefined => undefined)))
    return result === undefined
      ? authorizationErrorPage()
      : repositorySelectionPage(result.selectionRef, result.repositories)
  })

const routeAuthorizationRequest = (
  request: Request,
  authorization: GitHubScmAuthorizationService,
) => {
  if (request.method === 'GET') {
    const url = new URL(request.url)
    const workspace = url.searchParams.get('workspace')?.trim() ?? ''
    const returnTo = url.searchParams.get('returnTo')?.trim() ?? ''
    return authorization.begin(workspace, returnTo).pipe(
      Effect.map(
        authorizeUrl =>
          new Response(null, {
            status: 302,
            headers: { 'cache-control': 'no-store', location: authorizeUrl },
          }),
      ),
      Effect.catch(() =>
        Effect.succeed(
          noStoreBrokerJson(
            { error: 'github_scm_authorization_invalid' },
            { status: 400 },
          ),
        ),
      ),
    )
  }
  if (request.method === 'POST') {
    return Effect.tryPromise({
      try: () => request.formData(),
      catch: () =>
        new GitHubScmAuthBrokerDependencyFailed({
          reason: 'authorization_form_read_failed',
        }),
    }).pipe(
      Effect.catch(() => Effect.sync((): undefined => undefined)),
      Effect.flatMap(form => {
        if (form === undefined) return Effect.succeed(authorizationErrorPage())
        const selectionRef = form.get('selectionRef')
        const repository = form.get('repository')
        if (
          typeof selectionRef !== 'string' ||
          typeof repository !== 'string'
        ) {
          return Effect.succeed(authorizationErrorPage())
        }
        return authorization.complete(selectionRef, repository).pipe(
          Effect.map(completion => {
            const returnUrl = new URL(completion.returnTo)
            returnUrl.searchParams.set('repoRepository', completion.repository)
            returnUrl.searchParams.set(
              'repoGrantReference',
              completion.grantReference,
            )
            return new Response(null, {
              status: 303,
              headers: {
                'cache-control': 'no-store',
                location: returnUrl.toString(),
              },
            })
          }),
          Effect.catch(() => Effect.succeed(authorizationErrorPage())),
        )
      }),
    )
  }
  return Effect.succeed(methodNotAllowed(['GET', 'POST']))
}

export const routeGitHubScmAuthBrokerRequest = (
  request: Request,
  deps: GitHubScmAuthBrokerDependencies,
) => {
  const url = new URL(request.url)
  if (url.pathname === GITHUB_SCM_AUTHORIZATION_PATH) {
    return routeAuthorizationRequest(request, deps.authorization)
  }
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
