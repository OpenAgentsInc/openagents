import { type AuthKvStore, authKvStoreForEnv } from './auth/auth-kv'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  optionalBoolean,
  optionalString,
  readJsonObject,
} from './json-boundary'
import { Effect, Layer } from 'effect'
import {
  type ProviderApiKeyProbe,
  type StoreConnectedProviderApiKey,
  connectProviderApiKeyAccount,
  providerApiKeyConnectPolicyForRouteSegment,
} from './provider-account-api-key'
import {
  type DeleteStartedCodexDeviceLogin,
  type ReadStartedCodexDeviceLogin,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
  disconnectProviderAccountForUser,
  issueProviderAccountGrant,
  makeD1ProviderAccountRepository,
  makeProviderAccountLifecycleLayer,
  pollOpenAiCodexDeviceLogin,
  ProviderAccountLifecycleService,
  type ProviderAccountError,
  refreshChatGptCodexDeviceLoginForUser,
  startChatGptCodexDeviceLogin,
  startOpenAiCodexDeviceLogin,
  providerAccountErrorFromUnknown,
} from './provider-accounts'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'
import { logWorkerRouteError, observedPromise } from './observability'
import { openAgentsDatabase } from './runtime'
import type { RouteEffect } from './http/route-effects'

type ProviderAccountBrowserEnv = Readonly<{
  AUTH_KV?: AuthKvStore | undefined
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

type ProviderAccountBrowserSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type ProviderAccountBrowserDependencies<
  Session extends ProviderAccountBrowserSession,
  RouteEnv extends ProviderAccountBrowserEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response
  deleteStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => DeleteStartedCodexDeviceLogin
  providerAuthSecretKey: (providerAccountRef: string) => string
  readStartedCodexDeviceLogin: (kv: AuthKvStore) => ReadStartedCodexDeviceLogin
  probeProviderApiKey: ProviderApiKeyProbe
  requireBrowserSession: (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  storeConnectedCodexAuth: (env: RouteEnv) => StoreConnectedCodexAuth
  storeConnectedProviderApiKey: (
    kv: AuthKvStore,
  ) => StoreConnectedProviderApiKey
  storeStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => StoreStartedCodexDeviceLogin
  providerAccountLifecycleLayer?: (
    env: RouteEnv,
  ) => Layer.Layer<ProviderAccountLifecycleService> | undefined
}>

const providerAccountLifecycleLayerForEnv = <
  Session extends ProviderAccountBrowserSession,
  RouteEnv extends ProviderAccountBrowserEnv,
>(
  dependencies: ProviderAccountBrowserDependencies<Session, RouteEnv>,
  env: RouteEnv,
) =>
  dependencies.providerAccountLifecycleLayer?.(env) ??
  makeProviderAccountLifecycleLayer({
    deleteStartedDeviceLogin: dependencies.deleteStartedCodexDeviceLogin(
      authKvStoreForEnv(env),
    ),
    pollDeviceLogin: secret => pollOpenAiCodexDeviceLogin(secret),
    readStartedDeviceLogin: dependencies.readStartedCodexDeviceLogin(
      authKvStoreForEnv(env),
    ),
    repository: makeD1ProviderAccountRepository(openAgentsDatabase(env)),
    startDeviceLogin: () => startOpenAiCodexDeviceLogin(),
    storeConnectedAuth: dependencies.storeConnectedCodexAuth(env),
    storeStartedDeviceLogin: dependencies.storeStartedCodexDeviceLogin(
      authKvStoreForEnv(env),
    ),
  })

export const handleProviderAccountsListEffect = <
  Session extends ProviderAccountBrowserSession,
  RouteEnv extends ProviderAccountBrowserEnv,
>(
  request: Request,
  env: RouteEnv,
  ctx: ExecutionContext,
  dependencies: ProviderAccountBrowserDependencies<Session, RouteEnv>,
): Effect.Effect<Response, never, ProviderAccountLifecycleService> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error =>
        providerAccountErrorFromUnknown('require_browser_session', error),
    })

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const service = yield* ProviderAccountLifecycleService
    const bundle = yield* service.listForUser(session.user.userId)
    const response = noStoreJsonResponse(bundle)

    return dependencies.appendRefreshedSessionCookies(response, session)
  }).pipe(
    Effect.catch((error: ProviderAccountError) => {
      logWorkerRouteError('provider_accounts_list_failed', error, {
        errorName: providerAccountRouteErrorName(error),
      })

      return Effect.succeed(
        noStoreJsonResponse(
          {
            error: 'provider_accounts_list_failed',
            message: providerAccountRouteErrorMessage(error),
          },
          { status: providerAccountRouteErrorStatus(error) },
        ),
      )
    }),
  )
}

export const makeProviderAccountBrowserHandlers = <
  Session extends ProviderAccountBrowserSession,
  RouteEnv extends ProviderAccountBrowserEnv,
>(
  dependencies: ProviderAccountBrowserDependencies<Session, RouteEnv>,
) => ({
  handleProviderAccountsListApi: (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ): RouteEffect =>
    handleProviderAccountsListEffect(request, env, ctx, dependencies).pipe(
      Effect.provide(providerAccountLifecycleLayerForEnv(dependencies, env)),
    ),

  handleProviderAccountDisconnectApi: async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
    providerAccountRef: string,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const account = await disconnectProviderAccountForUser(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      session.user.userId,
      providerAccountRef,
    )

    if (account === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    await authKvStoreForEnv(env).delete(
      dependencies.providerAuthSecretKey(providerAccountRef),
    )

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ account }),
      session,
    )
  },

  handleProviderAccountGrantIssueApi: async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
    providerAccountRef: string,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const requestedAction = optionalString(body.requestedAction)
    const threadId = optionalString(body.threadId)
    const workroomId = optionalString(body.workroomId)
    const runnerSessionId =
      optionalString(body.runnerSessionId) ?? optionalString(body.runId)

    try {
      const grant = await observedPromise(
        'ProviderAccountBrowser.issueProviderAccountGrant',
        () =>
          issueProviderAccountGrant(
            makeD1ProviderAccountRepository(openAgentsDatabase(env)),
            {
              providerAccountRef,
              userId: session.user.userId,
              ...(requestedAction === undefined ? {} : { requestedAction }),
              ...(threadId === undefined ? {} : { threadId }),
              ...(workroomId === undefined ? {} : { workroomId }),
              ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
            },
          ),
      )

      if (grant === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({ grant }, { status: 201 }),
        session,
      )
    } catch (error) {
      logWorkerRouteError('provider_account_grant_issue_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
        requestedAction,
        runnerSessionId,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_account_grant_issue_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error) },
      )
    }
  },

  handleProviderApiKeyConnectApi: async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
    providerRouteSegment: string,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const policy =
      providerApiKeyConnectPolicyForRouteSegment(providerRouteSegment)

    if (policy === undefined) {
      return noStoreJsonResponse(
        {
          error: 'provider_not_api_key_connectable',
          message:
            'This provider does not support API-key connect. Subscription-account connect is not offered.',
        },
        { status: 404 },
      )
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const accountLabel = optionalString(body.accountLabel)
    const providerAccountRef = optionalString(body.providerAccountRef)

    try {
      const result = await observedPromise(
        'ProviderAccountBrowser.connectProviderApiKeyAccount',
        () =>
          connectProviderApiKeyAccount(
            makeD1ProviderAccountRepository(openAgentsDatabase(env)),
            {
              userId: session.user.userId,
              provider: policy.provider,
              apiKey: typeof body.apiKey === 'string' ? body.apiKey : '',
              ...(accountLabel === undefined ? {} : { accountLabel }),
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
            },
            {
              probeApiKey: dependencies.probeProviderApiKey,
              storeConnectedApiKey: dependencies.storeConnectedProviderApiKey(
                authKvStoreForEnv(env),
              ),
            },
          ),
      )

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(result, { status: 201 }),
        session,
      )
    } catch (error) {
      logWorkerRouteError('provider_api_key_connect_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        provider: policy.provider,
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_api_key_connect_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error) },
      )
    }
  },

  handleProviderDeviceLoginStartApi: async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const accountLabel = optionalString(body.accountLabel)
    const createNew = optionalBoolean(body.createNew)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const repository = makeD1ProviderAccountRepository(openAgentsDatabase(env))

    try {
      const result = await observedPromise(
        'ProviderAccountBrowser.startChatGptCodexDeviceLogin',
        () =>
          startChatGptCodexDeviceLogin(
            repository,
            {
              userId: session.user.userId,
              ...(accountLabel === undefined ? {} : { accountLabel }),
              ...(createNew === undefined ? {} : { createNew }),
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
            },
            () => startOpenAiCodexDeviceLogin(),
            {
              storeStartedDeviceLogin: dependencies.storeStartedCodexDeviceLogin(
                authKvStoreForEnv(env),
              ),
            },
          ),
      )
      const response = noStoreJsonResponse(result, { status: 201 })

      return dependencies.appendRefreshedSessionCookies(response, session)
    } catch (error) {
      logWorkerRouteError('provider_device_login_start_failed', error, {
        createNew,
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_device_login_start_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 502) },
      )
    }
  },

  handleProviderDeviceLoginStatusApi: async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
    attemptId: string,
  ): Promise<Response> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = await dependencies.requireBrowserSession(request, env, ctx)

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const result = await refreshChatGptCodexDeviceLoginForUser(
      makeD1ProviderAccountRepository(openAgentsDatabase(env)),
      {
        attemptId,
        userId: session.user.userId,
      },
      dependencies.readStartedCodexDeviceLogin(authKvStoreForEnv(env)),
      dependencies.storeConnectedCodexAuth(env),
      secret => pollOpenAiCodexDeviceLogin(secret),
      dependencies.deleteStartedCodexDeviceLogin(authKvStoreForEnv(env)),
    )

    if (result === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(result),
      session,
    )
  },
})
