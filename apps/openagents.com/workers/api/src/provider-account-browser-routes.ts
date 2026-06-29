import { Effect, Layer } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  RouteDependencyError,
  type RouteError,
  type RouteEffect,
} from './http/route-effects'
import {
  optionalBoolean,
  optionalString,
  readJsonObject,
} from './json-boundary'
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
  makeProviderAccountLifecycleService,
  pollOpenAiCodexDeviceLogin,
  ProviderAccountLifecycleService,
  type ProviderAccountError,
  refreshChatGptCodexDeviceLoginForUser,
  startChatGptCodexDeviceLogin,
  startOpenAiCodexDeviceLogin,
} from './provider-accounts'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'
import { logWorkerRouteError, observedPromise } from './observability'
import { openAgentsDatabase } from './runtime'

type ProviderAccountBrowserEnv = Readonly<{
  AUTH_STORAGE: KVNamespace
  OPENAGENTS_DB: D1Database
}>

type ProviderAccountBrowserSession = Readonly<{
  user: Readonly<{
    userId: string
  }>
}>

type ProviderAccountBrowserDependencies<
  Session extends ProviderAccountBrowserSession,
  Env extends ProviderAccountBrowserEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response
  deleteStartedCodexDeviceLogin: (
    kv: KVNamespace,
  ) => DeleteStartedCodexDeviceLogin
  providerAuthSecretKey: (providerAccountRef: string) => string
  readStartedCodexDeviceLogin: (kv: KVNamespace) => ReadStartedCodexDeviceLogin
  probeProviderApiKey: ProviderApiKeyProbe
  requireBrowserSession: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  storeConnectedCodexAuth: (kv: KVNamespace) => StoreConnectedCodexAuth
  storeConnectedProviderApiKey: (
    kv: KVNamespace,
  ) => StoreConnectedProviderApiKey
  storeStartedCodexDeviceLogin: (
    kv: KVNamespace,
  ) => StoreStartedCodexDeviceLogin
}>

type ProviderAccountsListRouteError = RouteError | ProviderAccountError

const requireBrowserSessionEffect = <
  Session extends ProviderAccountBrowserSession,
  Bindings extends ProviderAccountBrowserEnv,
>(
  dependencies: Pick<
    ProviderAccountBrowserDependencies<Session, Bindings>,
    'requireBrowserSession'
  >,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<Session | undefined, RouteError> =>
  Effect.tryPromise({
    try: () => dependencies.requireBrowserSession(request, env, ctx),
    catch: error =>
      new RouteDependencyError({
        operation: 'provider_accounts_require_browser_session',
        message: error instanceof Error ? error.message : String(error),
      }),
  })

export const listProviderAccountsForBrowserSession = <
  Session extends ProviderAccountBrowserSession,
>(
  session: Session,
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response,
) =>
  Effect.gen(function* () {
    const providerAccounts = yield* ProviderAccountLifecycleService
    const bundle = yield* providerAccounts.listForUser(session.user.userId)
    const response = noStoreJsonResponse(bundle)

    return appendRefreshedSessionCookies(response, session)
  })

const providerAccountsListRouteEffect = <
  Session extends ProviderAccountBrowserSession,
  Bindings extends ProviderAccountBrowserEnv,
>(
  input: Readonly<{
    dependencies: ProviderAccountBrowserDependencies<Session, Bindings>
    ctx: ExecutionContext
    env: Bindings
    request: Request
  }>,
) =>
  Effect.gen(function* () {
    if (input.request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = yield* requireBrowserSessionEffect(
      input.dependencies,
      input.request,
      input.env,
      input.ctx,
    )

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    return yield* listProviderAccountsForBrowserSession(
      session,
      input.dependencies.appendRefreshedSessionCookies,
    )
  })

const providerAccountsListErrorResponse = (
  error: ProviderAccountsListRouteError,
) =>
  noStoreJsonResponse(
    {
      error: 'provider_accounts_list_failed',
      errorName: providerAccountRouteErrorName(error),
      message: providerAccountRouteErrorMessage(error),
    },
    { status: providerAccountRouteErrorStatus(error, 500) },
  )

export const makeProviderAccountBrowserHandlers = <
  Session extends ProviderAccountBrowserSession,
  Env extends ProviderAccountBrowserEnv,
>(
  dependencies: ProviderAccountBrowserDependencies<Session, Env>,
) => ({
  handleProviderAccountsListApi: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): RouteEffect => {
    const repository = makeD1ProviderAccountRepository(openAgentsDatabase(env))
    const lifecycleLayer = Layer.succeed(
      ProviderAccountLifecycleService,
      makeProviderAccountLifecycleService({
        deleteStartedDeviceLogin: dependencies.deleteStartedCodexDeviceLogin(
          env.AUTH_STORAGE,
        ),
        pollDeviceLogin: secret => pollOpenAiCodexDeviceLogin(secret),
        readStartedDeviceLogin: dependencies.readStartedCodexDeviceLogin(
          env.AUTH_STORAGE,
        ),
        repository,
        startDeviceLogin: () => startOpenAiCodexDeviceLogin(),
        storeConnectedAuth: dependencies.storeConnectedCodexAuth(
          env.AUTH_STORAGE,
        ),
        storeStartedDeviceLogin: dependencies.storeStartedCodexDeviceLogin(
          env.AUTH_STORAGE,
        ),
      }),
    )

    return providerAccountsListRouteEffect({
      ctx,
      dependencies,
      env,
      request,
    }).pipe(
      Effect.provide(lifecycleLayer),
      Effect.catch(error =>
        Effect.succeed(providerAccountsListErrorResponse(error)),
      ),
    )
  },

  handleProviderAccountDisconnectApi: async (
    request: Request,
    env: Env,
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

    await env.AUTH_STORAGE.delete(
      dependencies.providerAuthSecretKey(providerAccountRef),
    )

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({ account }),
      session,
    )
  },

  handleProviderAccountGrantIssueApi: async (
    request: Request,
    env: Env,
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
    env: Env,
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
                env.AUTH_STORAGE,
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
    env: Env,
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
                env.AUTH_STORAGE,
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
    env: Env,
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
      dependencies.readStartedCodexDeviceLogin(env.AUTH_STORAGE),
      dependencies.storeConnectedCodexAuth(env.AUTH_STORAGE),
      secret => pollOpenAiCodexDeviceLogin(secret),
      dependencies.deleteStartedCodexDeviceLogin(env.AUTH_STORAGE),
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
