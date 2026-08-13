import { Effect, Schema as S } from 'effect'

import type { AuthKvStore } from './auth/auth-kv'
import { parseJsonWithSchema } from './json-boundary'
import { githubIdentityTokenKey } from './onboarding/github'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

export const GITHUB_SCM_AUTHORIZATION_PATH = '/api/pylon/github/authorize'
export const GITHUB_SCM_AUTHORIZATION_CALLBACK_PATH = '/github/callback'
export const GITHUB_SCM_AUTHORIZATION_SCOPES = [
  'contents:read-write',
  'pull-requests:read-write',
  'metadata:read',
] as const

const AUTHORIZATION_TTL_SECONDS = 10 * 60
const statePattern = /^github_scm_state_[0-9a-f]{32}$/
const selectionPattern = /^github_scm_selection_[0-9a-f]{32}$/
const grantPattern = /^github-scm-grant:[0-9a-f]{32}$/
const workspacePattern = /^one_workspace_[a-z0-9_-]{1,160}$/
const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/

const PendingAuthorization = S.Struct({
  schema: S.Literal('openagents.github_scm.pending_authorization.v1'),
  workspace: S.String,
  returnTo: S.String,
  createdAt: S.String,
})

const SelectableRepository = S.Struct({
  fullName: S.String,
  private: S.Boolean,
})

const RepositorySelection = S.Struct({
  schema: S.Literal('openagents.github_scm.repository_selection.v1'),
  workspace: S.String,
  returnTo: S.String,
  userId: S.String,
  accessToken: S.String,
  repositories: S.Array(SelectableRepository),
  createdAt: S.String,
})

const RepositoryGrant = S.Struct({
  schema: S.Literal('openagents.github_scm.repository_grant.v1'),
  userId: S.String,
  repository: S.String,
  workspace: S.String,
  createdAt: S.String,
})

type PendingAuthorization = typeof PendingAuthorization.Type
type RepositorySelection = typeof RepositorySelection.Type
type RepositoryGrant = typeof RepositoryGrant.Type

export type GitHubScmSelectableRepository = typeof SelectableRepository.Type

export type GitHubScmOAuthExchange = Readonly<{
  accessToken: string
  userId: string
  repositories: ReadonlyArray<GitHubScmSelectableRepository>
}>

export type GitHubScmAuthorizationCallbackResult = Readonly<{
  selectionRef: string
  repositories: ReadonlyArray<GitHubScmSelectableRepository>
}>

export type GitHubScmAuthorizationCompletion = Readonly<{
  grantReference: string
  repository: string
  returnTo: string
}>

export type GitHubScmRepositoryGrant = Readonly<{
  userId: string
  repository: string
}>

export class GitHubScmAuthorizationFailed extends S.TaggedErrorClass<GitHubScmAuthorizationFailed>()(
  'GitHubScmAuthorizationFailed',
  {
    reason: S.String,
  },
) {}

export type GitHubScmAuthorizationService = Readonly<{
  begin: (
    workspace: string,
    returnTo: string,
  ) => Effect.Effect<string, GitHubScmAuthorizationFailed>
  callback: (
    state: string,
    code: string,
  ) => Effect.Effect<
    GitHubScmAuthorizationCallbackResult,
    GitHubScmAuthorizationFailed
  >
  complete: (
    selectionRef: string,
    repository: string,
  ) => Effect.Effect<
    GitHubScmAuthorizationCompletion,
    GitHubScmAuthorizationFailed
  >
  readGrant: (
    grantReference: string,
  ) => Effect.Effect<GitHubScmRepositoryGrant | undefined, never>
}>

export type GitHubScmAuthorizationDependencies = Readonly<{
  storage: AuthKvStore
  authorizationUrl: (state: string) => string
  exchangeOAuthCode: (
    code: string,
  ) => Effect.Effect<GitHubScmOAuthExchange, unknown>
  verifyRepositoryAccess: (
    input: Readonly<{
      accessToken: string
      owner: string
      name: string
    }>,
  ) => Effect.Effect<
    Readonly<{ ok: true; fullName: string }> | Readonly<{ ok: false }>,
    unknown
  >
  nowIsoTimestamp?: () => string
  newUuid?: () => string
}>

const pendingKey = (state: string): string =>
  `github-scm:authorization:${state}`
const selectionKey = (selectionRef: string): string =>
  `github-scm:selection:${selectionRef}`
const selectionConsumedKey = (selectionRef: string): string =>
  `github-scm:selection-consumed:${selectionRef}`
const grantKey = (grantReference: string): string =>
  `github-scm:grant:${grantReference}`

const storageFailure = (reason: string) =>
  new GitHubScmAuthorizationFailed({ reason })

const validReturnTo = (value: string): boolean => {
  try {
    const url = new URL(value)
    return (
      (url.origin === 'https://one.openagents.com' ||
        url.origin === 'https://openagents.com') &&
      url.pathname === '/' &&
      url.username === '' &&
      url.password === '' &&
      url.search === '' &&
      url.hash === ''
    )
  } catch {
    return false
  }
}

const validRepository = (value: string): boolean =>
  githubFullNamePattern.test(value)

const readStored = <A>(
  storage: AuthKvStore,
  key: string,
  schema: S.Decoder<A>,
  reason: string,
): Effect.Effect<A | undefined, GitHubScmAuthorizationFailed> =>
  Effect.tryPromise({
    try: () => storage.get(key),
    catch: () => storageFailure(reason),
  }).pipe(
    Effect.flatMap(value => {
      if (value === null) {
        return Effect.sync((): A | undefined => undefined)
      }
      return Effect.try({
        try: () => parseJsonWithSchema(schema, value),
        catch: () => storageFailure(reason),
      })
    }),
  )

const putStored = (
  storage: AuthKvStore,
  key: string,
  value: unknown,
  reason: string,
  expirationTtl?: number,
): Effect.Effect<void, GitHubScmAuthorizationFailed> =>
  Effect.tryPromise({
    try: () =>
      storage.put(
        key,
        JSON.stringify(value),
        expirationTtl === undefined ? undefined : { expirationTtl },
      ),
    catch: () => storageFailure(reason),
  })

const deleteStored = (
  storage: AuthKvStore,
  key: string,
  reason: string,
): Effect.Effect<void, GitHubScmAuthorizationFailed> =>
  Effect.tryPromise({
    try: () => storage.delete(key),
    catch: () => storageFailure(reason),
  })

const consumeSelection = (
  storage: AuthKvStore,
  selectionRef: string,
): Effect.Effect<boolean, GitHubScmAuthorizationFailed> =>
  Effect.tryPromise({
    try: () =>
      storage.putIfAbsent(selectionConsumedKey(selectionRef), 'consumed', {
        expirationTtl: AUTHORIZATION_TTL_SECONDS,
      }),
    catch: () => storageFailure('selection_consume_failed'),
  })

export const makeGitHubScmAuthorizationService = (
  dependencies: GitHubScmAuthorizationDependencies,
): GitHubScmAuthorizationService => {
  const now = dependencies.nowIsoTimestamp ?? currentIsoTimestamp
  const newUuid = dependencies.newUuid ?? randomUuid

  return {
    begin: (workspace, returnTo) =>
      Effect.gen(function* () {
        if (!workspacePattern.test(workspace) || !validReturnTo(returnTo)) {
          return yield* storageFailure('invalid_authorization_request')
        }
        const state = `github_scm_state_${newUuid().replaceAll('-', '')}`
        if (!statePattern.test(state)) {
          return yield* storageFailure('invalid_generated_state')
        }
        const pending: PendingAuthorization = {
          schema: 'openagents.github_scm.pending_authorization.v1',
          workspace,
          returnTo,
          createdAt: now(),
        }
        yield* putStored(
          dependencies.storage,
          pendingKey(state),
          pending,
          'authorization_store_failed',
          AUTHORIZATION_TTL_SECONDS,
        )
        return dependencies.authorizationUrl(state)
      }),

    callback: (state, code) =>
      Effect.gen(function* () {
        if (!statePattern.test(state) || code.trim() === '') {
          return yield* storageFailure('invalid_oauth_callback')
        }
        const pending = yield* readStored(
          dependencies.storage,
          pendingKey(state),
          PendingAuthorization,
          'authorization_read_failed',
        )
        if (pending === undefined) {
          return yield* storageFailure('authorization_not_found')
        }
        yield* deleteStored(
          dependencies.storage,
          pendingKey(state),
          'authorization_consume_failed',
        )
        const exchange = yield* dependencies
          .exchangeOAuthCode(code)
          .pipe(Effect.mapError(() => storageFailure('oauth_exchange_failed')))
        const repositories = exchange.repositories
          .filter(repository => validRepository(repository.fullName))
          .slice(0, 100)
        if (
          exchange.accessToken.trim() === '' ||
          exchange.userId.trim() === '' ||
          repositories.length === 0
        ) {
          return yield* storageFailure('github_repository_list_empty')
        }
        const selectionRef = `github_scm_selection_${newUuid().replaceAll('-', '')}`
        if (!selectionPattern.test(selectionRef)) {
          return yield* storageFailure('invalid_generated_selection')
        }
        const selection: RepositorySelection = {
          schema: 'openagents.github_scm.repository_selection.v1',
          workspace: pending.workspace,
          returnTo: pending.returnTo,
          userId: exchange.userId,
          accessToken: exchange.accessToken,
          repositories,
          createdAt: now(),
        }
        yield* putStored(
          dependencies.storage,
          selectionKey(selectionRef),
          selection,
          'selection_store_failed',
          AUTHORIZATION_TTL_SECONDS,
        )
        return { selectionRef, repositories }
      }),

    complete: (selectionRef, repository) =>
      Effect.gen(function* () {
        if (
          !selectionPattern.test(selectionRef) ||
          !validRepository(repository)
        ) {
          return yield* storageFailure('invalid_repository_selection')
        }
        const wonConsumption = yield* consumeSelection(
          dependencies.storage,
          selectionRef,
        )
        if (!wonConsumption) {
          return yield* storageFailure('repository_selection_consumed')
        }
        const selection = yield* readStored(
          dependencies.storage,
          selectionKey(selectionRef),
          RepositorySelection,
          'selection_read_failed',
        )
        yield* deleteStored(
          dependencies.storage,
          selectionKey(selectionRef),
          'selection_delete_failed',
        )
        if (
          selection === undefined ||
          !selection.repositories.some(
            item => item.fullName.toLowerCase() === repository.toLowerCase(),
          )
        ) {
          return yield* storageFailure('repository_not_selectable')
        }
        const [owner, name] = repository.split('/')
        if (owner === undefined || name === undefined) {
          return yield* storageFailure('invalid_repository_selection')
        }
        const access = yield* dependencies
          .verifyRepositoryAccess({
            accessToken: selection.accessToken,
            owner,
            name,
          })
          .pipe(
            Effect.mapError(() =>
              storageFailure('repository_verification_failed'),
            ),
          )
        if (
          !access.ok ||
          access.fullName.toLowerCase() !== repository.toLowerCase()
        ) {
          return yield* storageFailure('repository_access_denied')
        }
        const grantReference = `github-scm-grant:${newUuid().replaceAll('-', '')}`
        if (!grantPattern.test(grantReference)) {
          return yield* storageFailure('invalid_generated_grant')
        }
        yield* Effect.tryPromise({
          try: () =>
            dependencies.storage.put(
              githubIdentityTokenKey(selection.userId),
              selection.accessToken,
            ),
          catch: () => storageFailure('github_identity_token_store_failed'),
        })
        const grant: RepositoryGrant = {
          schema: 'openagents.github_scm.repository_grant.v1',
          userId: selection.userId,
          repository: access.fullName,
          workspace: selection.workspace,
          createdAt: now(),
        }
        yield* putStored(
          dependencies.storage,
          grantKey(grantReference),
          grant,
          'grant_store_failed',
        )
        return {
          grantReference,
          repository: access.fullName,
          returnTo: selection.returnTo,
        }
      }),

    readGrant: grantReference => {
      if (!grantPattern.test(grantReference)) {
        return Effect.sync(
          (): GitHubScmRepositoryGrant | undefined => undefined,
        )
      }
      return readStored(
        dependencies.storage,
        grantKey(grantReference),
        RepositoryGrant,
        'grant_read_failed',
      ).pipe(
        Effect.map(grant =>
          grant === undefined
            ? undefined
            : { userId: grant.userId, repository: grant.repository },
        ),
        Effect.catch(() =>
          Effect.sync((): GitHubScmRepositoryGrant | undefined => undefined),
        ),
      )
    },
  }
}
