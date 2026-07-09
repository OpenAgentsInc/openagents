import { type AuthKvStore, authKvStoreForEnv } from './auth/auth-kv'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { optionalBoolean, optionalString, readJsonObject } from './json-boundary'
import { logWorkerRouteError, observedPromise } from './observability'
import {
  type DeleteStartedCodexDeviceLogin,
  type ReadStartedCodexDeviceLogin,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
  disconnectProviderAccountForUser,
  filterMobileVisibleProviderAccountBundle,
  listProviderAccountsForUser,
  makeD1ProviderAccountRepository,
  pollOpenAiCodexDeviceLogin,
  refreshChatGptCodexDeviceLoginForUser,
  startChatGptCodexDeviceLogin,
  startOpenAiCodexDeviceLogin,
} from './provider-accounts'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'
import { openAgentsDatabase } from './runtime'

export const MOBILE_CODEX_ACCOUNTS_PATH = '/api/mobile/codex-accounts'
export const MOBILE_CODEX_DEVICE_LOGIN_START_PATH =
  '/api/mobile/codex-accounts/device-login/start'

type ProviderAccountMobileEnv = Readonly<{
  AUTH_KV?: AuthKvStore | undefined
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

type ProviderAccountMobileDependencies<Session, RouteEnv> = Readonly<{
  deleteConnectedCodexAuth: (
    env: RouteEnv,
    input: Readonly<{
      ownerUserId: string
      providerAccountRef: string
    }>,
  ) => Promise<boolean>
  deleteStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => DeleteStartedCodexDeviceLogin
  readStartedCodexDeviceLogin: (kv: AuthKvStore) => ReadStartedCodexDeviceLogin
  requireUserBearerSession: (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  storeConnectedCodexAuth: (env: RouteEnv) => StoreConnectedCodexAuth
  storeStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => StoreStartedCodexDeviceLogin
  userIdFromSession: (session: Session) => string
}>

export const makeProviderAccountMobileHandlers = <
  Session,
  RouteEnv extends ProviderAccountMobileEnv,
>(
  dependencies: ProviderAccountMobileDependencies<Session, RouteEnv>,
) => {
  const requireOwnerUserId = async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
  ): Promise<string | undefined> => {
    const session = await dependencies.requireUserBearerSession(
      request,
      env,
      ctx,
    )
    return session === undefined
      ? undefined
      : dependencies.userIdFromSession(session)
  }

  return {
    handleMobileCodexAccountsListApi: async (
      request: Request,
      env: RouteEnv,
      ctx: ExecutionContext,
    ) => {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const ownerUserId = await requireOwnerUserId(request, env, ctx)

      if (ownerUserId === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const bundle = await listProviderAccountsForUser(
        makeD1ProviderAccountRepository(openAgentsDatabase(env)),
        ownerUserId,
      )

      // Only project live accounts to the phone: connected accounts and
      // in-progress (non-expired) device logins. Disconnected/denied/expired/
      // unhealthy residue is never shown as connected (issue #8546).
      return noStoreJsonResponse(
        filterMobileVisibleProviderAccountBundle(bundle),
      )
    },

    handleMobileCodexDeviceLoginStartApi: async (
      request: Request,
      env: RouteEnv,
      ctx: ExecutionContext,
    ) => {
      if (request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      const ownerUserId = await requireOwnerUserId(request, env, ctx)

      if (ownerUserId === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const body = await readJsonObject(request).catch(
        (): Record<string, unknown> => ({}),
      )
      const accountLabel = optionalString(body.accountLabel)
      const createNew = optionalBoolean(body.createNew)
      const providerAccountRef = optionalString(body.providerAccountRef)

      try {
        const result = await observedPromise(
          'ProviderAccountMobile.startChatGptCodexDeviceLogin',
          () =>
            startChatGptCodexDeviceLogin(
              makeD1ProviderAccountRepository(openAgentsDatabase(env)),
              {
                userId: ownerUserId,
                ...(accountLabel === undefined ? {} : { accountLabel }),
                ...(createNew === undefined ? {} : { createNew }),
                ...(providerAccountRef === undefined
                  ? {}
                  : { providerAccountRef }),
              },
              () => startOpenAiCodexDeviceLogin(),
              {
                storeStartedDeviceLogin:
                  dependencies.storeStartedCodexDeviceLogin(
                    authKvStoreForEnv(env),
                  ),
              },
            ),
        )

        return noStoreJsonResponse(result, { status: 201 })
      } catch (error) {
        logWorkerRouteError('mobile_codex_device_login_start_failed', error, {
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

    handleMobileCodexDeviceLoginStatusApi: async (
      request: Request,
      env: RouteEnv,
      ctx: ExecutionContext,
      attemptId: string,
    ) => {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const ownerUserId = await requireOwnerUserId(request, env, ctx)

      if (ownerUserId === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      try {
        const result = await refreshChatGptCodexDeviceLoginForUser(
          makeD1ProviderAccountRepository(openAgentsDatabase(env)),
          {
            attemptId,
            userId: ownerUserId,
          },
          dependencies.readStartedCodexDeviceLogin(authKvStoreForEnv(env)),
          dependencies.storeConnectedCodexAuth(env),
          secret => pollOpenAiCodexDeviceLogin(secret),
          dependencies.deleteStartedCodexDeviceLogin(authKvStoreForEnv(env)),
        )

        return result === undefined
          ? noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
          : noStoreJsonResponse(result)
      } catch (error) {
        logWorkerRouteError('mobile_codex_device_login_status_failed', error, {
          attemptId,
          errorName: providerAccountRouteErrorName(error),
        })

        return noStoreJsonResponse(
          {
            error: 'provider_device_login_status_failed',
            message: providerAccountRouteErrorMessage(error),
          },
          { status: providerAccountRouteErrorStatus(error, 502) },
        )
      }
    },

    handleMobileCodexAccountDisconnectApi: async (
      request: Request,
      env: RouteEnv,
      ctx: ExecutionContext,
      providerAccountRef: string,
    ) => {
      if (request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      const ownerUserId = await requireOwnerUserId(request, env, ctx)

      if (ownerUserId === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      try {
        const account = await disconnectProviderAccountForUser(
          makeD1ProviderAccountRepository(openAgentsDatabase(env)),
          ownerUserId,
          providerAccountRef,
        )

        if (account === undefined) {
          return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
        }

        await dependencies.deleteConnectedCodexAuth(env, {
          ownerUserId,
          providerAccountRef,
        })

        return noStoreJsonResponse({ account })
      } catch (error) {
        logWorkerRouteError('mobile_codex_account_disconnect_failed', error, {
          errorName: providerAccountRouteErrorName(error),
          providerAccountRef,
        })

        return noStoreJsonResponse(
          {
            error: 'provider_account_disconnect_failed',
            message: providerAccountRouteErrorMessage(error),
          },
          { status: providerAccountRouteErrorStatus(error, 500) },
        )
      }
    },
  }
}
