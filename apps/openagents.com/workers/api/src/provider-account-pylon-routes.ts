import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
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
  AUTH_STORAGE: KVNamespace
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

type ProviderAccountPylonDependencies<
  Bindings extends ProviderAccountPylonBindings,
> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  deleteStartedCodexDeviceLogin: (
    kv: KVNamespace,
  ) => DeleteStartedCodexDeviceLogin
  makeProviderAccountRepository?: (db: D1Database) => ProviderAccountRepository
  nowIso?: () => string
  pollDeviceLogin?: PollCodexDeviceLogin
  readStartedCodexDeviceLogin: (kv: KVNamespace) => ReadStartedCodexDeviceLogin
  startDeviceLogin?: StartCodexDeviceLogin
  storeConnectedCodexAuth: (env: Bindings) => StoreConnectedCodexAuth
  storeStartedCodexDeviceLogin: (
    kv: KVNamespace,
  ) => StoreStartedCodexDeviceLogin
}>

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')
  if (authorization === null) return undefined
  const [scheme, token] = authorization.split(' ')
  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
}

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
                env.AUTH_STORAGE,
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
      dependencies.readStartedCodexDeviceLogin(env.AUTH_STORAGE),
      dependencies.storeConnectedCodexAuth(env),
      dependencies.pollDeviceLogin ?? (secret => pollOpenAiCodexDeviceLogin(secret)),
      dependencies.deleteStartedCodexDeviceLogin(env.AUTH_STORAGE),
    )

    if (result === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse(withPylonLinkMetadata(result))
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
