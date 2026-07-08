import { type AuthKvStore, authKvStoreForEnv } from './auth/auth-kv'
import {
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  readAgentBearerToken as bearerTokenFromRequest,
} from './auth/bearer-token'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  optionalBoolean,
  optionalString,
  readJsonObject,
} from './json-boundary'
import { logWorkerRouteError, observedPromise } from './observability'
import {
  type CodexOAuthAuth,
  type DeleteStartedCodexDeviceLogin,
  type PollCodexDeviceLogin,
  type ProviderAccountRepository,
  type ReadStartedCodexDeviceLogin,
  type StartCodexDeviceLogin,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
  connectChatGptCodexLocalAuthForUser,
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
import { ProviderAccountCredentialMaterial } from './provider-account-errors'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

type ProviderAccountPylonBindings = Readonly<{
  AUTH_KV?: AuthKvStore | undefined
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

type ConnectedCodexAuthMaterial = Readonly<{
  authContentEnv: 'OPENCODE_AUTH_CONTENT'
  authContentJson: string
}>

type ConnectedClaudeAuthMaterial = Readonly<{
  authContentEnv: 'CLAUDE_CODE_OAUTH_TOKEN'
  authContentValue: string
}>

type ProviderAccountPylonDependencies<
  Bindings extends ProviderAccountPylonBindings,
> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  deleteStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => DeleteStartedCodexDeviceLogin
  makeProviderAccountRepository?: (db: D1Database) => ProviderAccountRepository
  nowIso?: () => string
  pollDeviceLogin?: PollCodexDeviceLogin
  readConnectedCodexAuthMaterial: (
    bindings: Bindings,
    ownerUserId: string,
    providerAccountRef: string,
  ) => Promise<ConnectedCodexAuthMaterial | undefined>
  readConnectedClaudeAuthMaterial?: (
    bindings: Bindings,
    ownerUserId: string,
    providerAccountRef: string,
  ) => Promise<ConnectedClaudeAuthMaterial | undefined>
  readStartedCodexDeviceLogin: (kv: AuthKvStore) => ReadStartedCodexDeviceLogin
  startDeviceLogin?: StartCodexDeviceLogin
  storeConnectedCodexAuth: (env: Bindings) => StoreConnectedCodexAuth
  storeStartedCodexDeviceLogin: (
    kv: AuthKvStore,
  ) => StoreStartedCodexDeviceLogin
}>

const requireAgent = async <Bindings extends ProviderAccountPylonBindings>(
  dependencies: ProviderAccountPylonDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Promise<ProgrammaticAgentSession | undefined> => {
  const token = bearerTokenFromRequest(request)
  if (token === undefined) return undefined
  return authenticateProgrammaticAgent(
    dependencies.agentStore(env),
    token,
    dependencies.nowIso ?? currentIsoTimestamp,
  )
}

const linkedOpenAuthOwnerUserId = (
  session: ProgrammaticAgentSession,
): string | undefined => {
  const owner = session.credential.openauthUserId
  return typeof owner === 'string' && owner.trim() !== '' ? owner : undefined
}

const pylonAgentNotLinkedResponse = () =>
  noStoreJsonResponse(
    {
      error: 'pylon_agent_not_linked',
      message:
        'This Pylon agent token is not linked to an OpenAuth account. Link the Pylon to your OpenAgents account before starting a Codex device login from Pylon.',
    },
    { status: 409 },
  )

const routeProviderAccountRepository = <
  Bindings extends ProviderAccountPylonBindings,
>(
  dependencies: ProviderAccountPylonDependencies<Bindings>,
  env: Bindings,
): ProviderAccountRepository =>
  (dependencies.makeProviderAccountRepository ?? makeD1ProviderAccountRepository)(
    openAgentsDatabase(env),
  )

const withPylonLinkMetadata = <T extends Record<string, unknown>>(
  result: T,
): T & { pylonLink: { owner: 'openauth'; status: 'linked' } } => ({
  ...result,
  pylonLink: {
    owner: 'openauth',
    status: 'linked',
  },
})

const finiteNumber = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const requiredSecretString = (
  record: Record<string, unknown>,
  key: string,
): string => {
  const value = record[key]
  if (typeof value !== 'string' || value.trim() === '') {
    throw new ProviderAccountCredentialMaterial({
      fieldName: `auth.${key}`,
      message: 'Codex local auth material is missing a required field.',
    })
  }
  return value
}

const optionalSecretString = (
  record: Record<string, unknown>,
  key: string,
): string | undefined => {
  const value = record[key]
  return typeof value === 'string' && value.trim() !== ''
    ? value
    : undefined
}

const codexOAuthAuthFromBody = (
  body: Record<string, unknown>,
): CodexOAuthAuth => {
  const auth =
    body.auth !== null && typeof body.auth === 'object' && !Array.isArray(body.auth)
      ? (body.auth as Record<string, unknown>)
      : {}
  const expires = finiteNumber(auth.expires)
  const accountId = optionalSecretString(auth, 'accountId')
  const idToken = optionalSecretString(auth, 'idToken')

  return {
    type: 'oauth',
    access: requiredSecretString(auth, 'access'),
    refresh: requiredSecretString(auth, 'refresh'),
    expires: expires ?? 0,
    ...(accountId === undefined ? {} : { accountId }),
    ...(idToken === undefined ? {} : { idToken }),
  }
}

export const makeProviderAccountPylonHandlers = <
  Bindings extends ProviderAccountPylonBindings,
>(
  dependencies: ProviderAccountPylonDependencies<Bindings>,
) => ({
  handlePylonProviderDeviceLoginStartApi: async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await requireAgent(dependencies, request, env)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const userId = linkedOpenAuthOwnerUserId(session)
    if (userId === undefined) {
      return pylonAgentNotLinkedResponse()
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const accountLabel = optionalString(body.accountLabel)
    const createNew = optionalBoolean(body.createNew)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const repository = routeProviderAccountRepository(dependencies, env)

    try {
      const result = await observedPromise(
        'ProviderAccountPylon.startChatGptCodexDeviceLogin',
        () =>
          startChatGptCodexDeviceLogin(
            repository,
            {
              userId,
              ...(accountLabel === undefined ? {} : { accountLabel }),
              ...(createNew === undefined ? {} : { createNew }),
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
            },
            dependencies.startDeviceLogin ?? (() => startOpenAiCodexDeviceLogin()),
            {
              storeStartedDeviceLogin: dependencies.storeStartedCodexDeviceLogin(
                authKvStoreForEnv(env),
              ),
            },
          ),
      )

      return noStoreJsonResponse(withPylonLinkMetadata(result), { status: 201 })
    } catch (error) {
      logWorkerRouteError('pylon_provider_device_login_start_failed', error, {
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

  handlePylonProviderDeviceLoginStatusApi: async (
    request: Request,
    env: Bindings,
    attemptId: string,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = await requireAgent(dependencies, request, env)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const userId = linkedOpenAuthOwnerUserId(session)
    if (userId === undefined) {
      return pylonAgentNotLinkedResponse()
    }

    const result = await refreshChatGptCodexDeviceLoginForUser(
      routeProviderAccountRepository(dependencies, env),
      {
        attemptId,
        userId,
      },
      dependencies.readStartedCodexDeviceLogin(authKvStoreForEnv(env)),
      dependencies.storeConnectedCodexAuth(env),
      dependencies.pollDeviceLogin ?? (secret => pollOpenAiCodexDeviceLogin(secret)),
      dependencies.deleteStartedCodexDeviceLogin(authKvStoreForEnv(env)),
    )

    if (result === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse(withPylonLinkMetadata(result))
  },

  handlePylonProviderCodexAuthMaterialApi: async (
    request: Request,
    env: Bindings,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await requireAgent(dependencies, request, env)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const userId = linkedOpenAuthOwnerUserId(session)
    if (userId === undefined) {
      return pylonAgentNotLinkedResponse()
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const providerAccountRef = optionalString(body.providerAccountRef)
    if (providerAccountRef === undefined) {
      return noStoreJsonResponse(
        {
          error: 'provider_account_ref_required',
          message:
            'A providerAccountRef is required before Pylon can request Codex auth material from custody.',
        },
        { status: 400 },
      )
    }

    try {
      const authMaterial = await observedPromise(
        'ProviderAccountPylon.readConnectedCodexAuthMaterial',
        () =>
          dependencies.readConnectedCodexAuthMaterial(
            env,
            userId,
            providerAccountRef,
          ),
      )

      if (authMaterial === undefined) {
        return noStoreJsonResponse(
          {
            error: 'provider_account_auth_material_unavailable',
            message:
              'Codex provider account auth material is unavailable in custody for this owner.',
          },
          { status: 409 },
        )
      }

      return noStoreJsonResponse(
        withPylonLinkMetadata({
          schema: 'openagents.pylon.provider_account.codex_auth_material.v1',
          status: 'issued',
          providerAccountRef,
          authMaterial,
        }),
      )
    } catch (error) {
      logWorkerRouteError('pylon_provider_codex_auth_material_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_account_auth_material_unavailable',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 409) },
      )
    }
  },

  handlePylonProviderClaudeAuthMaterialApi: async (
    request: Request,
    env: Bindings,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await requireAgent(dependencies, request, env)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const userId = linkedOpenAuthOwnerUserId(session)
    if (userId === undefined) {
      return pylonAgentNotLinkedResponse()
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const providerAccountRef = optionalString(body.providerAccountRef)
    if (providerAccountRef === undefined) {
      return noStoreJsonResponse(
        {
          error: 'provider_account_ref_required',
          message:
            'A providerAccountRef is required before Pylon can request Claude auth material from custody.',
        },
        { status: 400 },
      )
    }

    try {
      const account = await routeProviderAccountRepository(
        dependencies,
        env,
      ).findAccountByRef(userId, providerAccountRef)
      if (account === undefined || account.provider !== 'anthropic_claude') {
        return noStoreJsonResponse(
          {
            error: 'provider_account_auth_material_unavailable',
            message:
              'Claude provider account auth material is unavailable in custody for this owner.',
          },
          { status: 409 },
        )
      }

      const authMaterial = await observedPromise(
        'ProviderAccountPylon.readConnectedClaudeAuthMaterial',
        () =>
          dependencies.readConnectedClaudeAuthMaterial?.(
            env,
            userId,
            providerAccountRef,
          ) ?? Promise.resolve(undefined),
      )

      if (authMaterial === undefined) {
        return noStoreJsonResponse(
          {
            error: 'provider_account_auth_material_unavailable',
            message:
              'Claude provider account auth material is unavailable in custody for this owner.',
          },
          { status: 409 },
        )
      }

      return noStoreJsonResponse(
        withPylonLinkMetadata({
          schema: 'openagents.pylon.provider_account.claude_auth_material.v1',
          status: 'issued',
          providerAccountRef,
          authMaterial,
        }),
      )
    } catch (error) {
      logWorkerRouteError('pylon_provider_claude_auth_material_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_account_auth_material_unavailable',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 409) },
      )
    }
  },

  handlePylonProviderLocalCodexAuthImportApi: async (
    request: Request,
    env: Bindings,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await requireAgent(dependencies, request, env)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const userId = linkedOpenAuthOwnerUserId(session)
    if (userId === undefined) {
      return pylonAgentNotLinkedResponse()
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const accountLabel = optionalString(body.accountLabel)
    const createNew = optionalBoolean(body.createNew)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const repository = routeProviderAccountRepository(dependencies, env)

    try {
      const result = await observedPromise(
        'ProviderAccountPylon.connectChatGptCodexLocalAuthForUser',
        () =>
          connectChatGptCodexLocalAuthForUser(
            repository,
            {
              userId,
              auth: codexOAuthAuthFromBody(body),
              ...(accountLabel === undefined ? {} : { accountLabel }),
              ...(createNew === undefined ? {} : { createNew }),
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
            },
            dependencies.storeConnectedCodexAuth(env),
          ),
      )

      return noStoreJsonResponse(withPylonLinkMetadata(result), { status: 201 })
    } catch (error) {
      logWorkerRouteError('pylon_provider_local_codex_auth_import_failed', error, {
        createNew,
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_local_codex_auth_import_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 400) },
      )
    }
  },
})
