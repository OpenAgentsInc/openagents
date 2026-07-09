import { type AuthKvStore, authKvStoreForEnv } from './auth/auth-kv'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { optionalBoolean, optionalString, readJsonObject } from './json-boundary'
import { logWorkerRouteError, observedPromise } from './observability'
import {
  ANTHROPIC_CLAUDE_PROVIDER,
  CHATGPT_CODEX_PROVIDER,
  type DeleteStartedCodexDeviceLogin,
  type ReadStartedCodexDeviceLogin,
  type StoreConnectedClaudeAuth,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
  connectClaudeLocalAuthForUser,
  disconnectProviderAccountForUser,
  filterMobileProviderAccountBundleForProvider,
  listProviderAccountsForUser,
  makeD1ProviderAccountRepository,
  pollOpenAiCodexDeviceLogin,
  refreshChatGptCodexDeviceLoginForUser,
  startChatGptCodexDeviceLogin,
  startOpenAiCodexDeviceLogin,
} from './provider-accounts'
import { ProviderAccountCredentialMaterial } from './provider-account-errors'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'
import { openAgentsDatabase } from './runtime'

export const MOBILE_CODEX_ACCOUNTS_PATH = '/api/mobile/codex-accounts'
export const MOBILE_CODEX_DEVICE_LOGIN_START_PATH =
  '/api/mobile/codex-accounts/device-login/start'

/** CX-5 (#8549): mobile Claude accounts list (provider-scoped). */
export const MOBILE_CLAUDE_ACCOUNTS_PATH = '/api/mobile/claude-accounts'
/** CX-5 (#8549): paste-token import of a CLAUDE_CODE_OAUTH_TOKEN under mobile bearer. */
export const MOBILE_CLAUDE_LOCAL_AUTH_IMPORT_PATH =
  '/api/mobile/claude-accounts/local-auth/import'

type ProviderAccountMobileEnv = Readonly<{
  AUTH_KV?: AuthKvStore | undefined
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

type ProviderAccountMobileDependencies<Session, RouteEnv> = Readonly<{
  deleteConnectedClaudeAuth: (
    env: RouteEnv,
    input: Readonly<{
      ownerUserId: string
      providerAccountRef: string
    }>,
  ) => Promise<boolean>
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
  storeConnectedClaudeAuth: (env: RouteEnv) => StoreConnectedClaudeAuth
  storeConnectedCodexAuth: (env: RouteEnv) => StoreConnectedCodexAuth
  storeStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => StoreStartedCodexDeviceLogin
  userIdFromSession: (session: Session) => string
}>

const requiredClaudeAuthContentValue = (
  record: Record<string, unknown>,
): string => {
  const value = record.authContentValue
  if (typeof value !== 'string' || value.trim() === '') {
    throw new ProviderAccountCredentialMaterial({
      fieldName: 'authContentValue',
      message: 'Claude local auth material is missing a required field.',
    })
  }
  return value
}

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

      // Only project live Codex accounts to the phone: connected accounts and
      // in-progress (non-expired) device logins. Disconnected/denied/expired/
      // unhealthy residue is never shown as connected (issue #8546). CX-5 also
      // scopes this list to chatgpt_codex so Claude rows never render here.
      return noStoreJsonResponse(
        filterMobileProviderAccountBundleForProvider(
          bundle,
          CHATGPT_CODEX_PROVIDER,
        ),
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
        const repository = makeD1ProviderAccountRepository(
          openAgentsDatabase(env),
        )
        const existing = await repository.findAccountByRef(
          ownerUserId,
          providerAccountRef,
        )
        if (
          existing === undefined ||
          existing.provider !== CHATGPT_CODEX_PROVIDER
        ) {
          return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
        }

        const account = await disconnectProviderAccountForUser(
          repository,
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

    /**
     * CX-5 (#8549): mobile-bearer list of the owner's Claude subscription
     * accounts. Provider-scoped so Codex rows never appear under Claude.
     */
    handleMobileClaudeAccountsListApi: async (
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

      return noStoreJsonResponse(
        filterMobileProviderAccountBundleForProvider(
          bundle,
          ANTHROPIC_CLAUDE_PROVIDER,
        ),
      )
    },

    /**
     * CX-5 (#8549): paste-token Connect Claude flow. The owner runs
     * `claude setup-token` on their computer, pastes the long-lived
     * `CLAUDE_CODE_OAUTH_TOKEN` into Settings, and this route stores it under
     * custody for the Agent Computer broker to materialize later. Not a
     * device-login poll — Claude Code has no automatable device-code loop.
     */
    handleMobileClaudeLocalAuthImportApi: async (
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
      const createNew = optionalBoolean(body.createNew) ?? true
      const providerAccountRef = optionalString(body.providerAccountRef)

      try {
        const authContentValue = requiredClaudeAuthContentValue(body)
        const result = await observedPromise(
          'ProviderAccountMobile.connectClaudeLocalAuthForUser',
          () =>
            connectClaudeLocalAuthForUser(
              makeD1ProviderAccountRepository(openAgentsDatabase(env)),
              {
                userId: ownerUserId,
                authContentValue,
                createNew,
                ...(accountLabel === undefined ? {} : { accountLabel }),
                ...(providerAccountRef === undefined
                  ? {}
                  : { providerAccountRef }),
              },
              dependencies.storeConnectedClaudeAuth(env),
            ),
        )

        // Never echo the raw OAuth token back to the phone.
        return noStoreJsonResponse(result, { status: 201 })
      } catch (error) {
        logWorkerRouteError('mobile_claude_local_auth_import_failed', error, {
          createNew,
          errorName: providerAccountRouteErrorName(error),
          providerAccountRef,
        })

        return noStoreJsonResponse(
          {
            error: 'provider_local_claude_auth_import_failed',
            message: providerAccountRouteErrorMessage(error),
          },
          { status: providerAccountRouteErrorStatus(error, 400) },
        )
      }
    },

    handleMobileClaudeAccountDisconnectApi: async (
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
        const repository = makeD1ProviderAccountRepository(
          openAgentsDatabase(env),
        )
        const existing = await repository.findAccountByRef(
          ownerUserId,
          providerAccountRef,
        )
        if (
          existing === undefined ||
          existing.provider !== ANTHROPIC_CLAUDE_PROVIDER
        ) {
          return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
        }

        const account = await disconnectProviderAccountForUser(
          repository,
          ownerUserId,
          providerAccountRef,
        )

        if (account === undefined) {
          return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
        }

        await dependencies.deleteConnectedClaudeAuth(env, {
          ownerUserId,
          providerAccountRef,
        })

        return noStoreJsonResponse({ account })
      } catch (error) {
        logWorkerRouteError('mobile_claude_account_disconnect_failed', error, {
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
