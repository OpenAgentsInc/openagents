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
  type StoreConnectedClaudeAuth,
  type StoreConnectedCodexAuth,
  type StoreStartedCodexDeviceLogin,
  ANTHROPIC_CLAUDE_PROVIDER,
  CURSOR_PROVIDER,
  GOOGLE_GEMINI_PROVIDER,
  XAI_GROK_PROVIDER,
  connectChatGptCodexLocalAuthForUser,
  connectClaudeLocalAuthForUser,
  makeD1ProviderAccountRepository,
  pollOpenAiCodexDeviceLogin,
  refreshChatGptCodexDeviceLoginForUser,
  issueProviderAccountGrant,
  resolveProviderAccountGrant,
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
  CURSOR_API_KEY?: string | undefined
  GEMINI_API_KEY?: string | undefined
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
  XAI_API_KEY?: string | undefined
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
  providerGrantRepository?: (env: Bindings) => ProviderAccountRepository
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
  readGoogleGeminiSecretMaterial?: (
    bindings: Bindings,
  ) => string | undefined
  readCursorSecretMaterial?: (bindings: Bindings) => string | undefined
  readXaiSecretMaterial?: (bindings: Bindings) => string | undefined
  readStartedCodexDeviceLogin: (kv: AuthKvStore) => ReadStartedCodexDeviceLogin
  startDeviceLogin?: StartCodexDeviceLogin
  storeConnectedCodexAuth: (env: Bindings) => StoreConnectedCodexAuth
  /**
   * CX-5 (#8549): optional so existing (pre-CX-5) callers/tests that never
   * touch the Claude local-auth/import route keep compiling unchanged —
   * mirrors how `readConnectedClaudeAuthMaterial` was landed optional above.
   * `handlePylonProviderLocalClaudeAuthImportApi` fails closed with a typed
   * `claude_local_auth_import_not_configured` response when this is absent.
   */
  storeConnectedClaudeAuth?: (env: Bindings) => StoreConnectedClaudeAuth
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

const GOOGLE_GEMINI_SECRET_MATERIAL_KIND = 'gemini_api_key'
const GOOGLE_GEMINI_TURN_ACTION = 'agent_computer_gemini_turn'
const CURSOR_SECRET_MATERIAL_KIND = 'cursor_api_key'
const CURSOR_TURN_ACTION = 'agent_computer_cursor_turn'
const XAI_SECRET_MATERIAL_KIND = 'xai_api_key'
const XAI_GROK_TURN_ACTION = 'agent_computer_grok_turn'
const ANTHROPIC_CLAUDE_SECRET_MATERIAL_KIND =
  'claude_agent_anthropic_api_key'
const ANTHROPIC_CLAUDE_TURN_ACTION = 'agent_computer_claude_turn'

/** CX-5 (#8549): Claude's single-string bearer analogue of `requiredSecretString`. */
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
    const requestedAuthGrantRef = optionalString(body.authGrantRef)
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
      const grantRepository =
        dependencies.providerGrantRepository?.(env) ??
        routeProviderAccountRepository(dependencies, env)
      const issuedGrant =
        requestedAuthGrantRef === undefined
          ? await issueProviderAccountGrant(grantRepository, {
              providerAccountRef,
              requestedAction: 'pylon_local_codex_assignment',
              runnerSessionId: `pylon.${session.credential.id}`,
              userId,
            })
          : undefined
      const authGrantRef = requestedAuthGrantRef ?? issuedGrant?.grantRef
      if (authGrantRef === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }
      const candidateGrant = await grantRepository.findGrantByRef(authGrantRef)
      if (candidateGrant === undefined || candidateGrant.userId !== userId) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }
      await observedPromise(
        'ProviderAccountPylon.resolveProviderAccountGrantForMaterialization',
        () =>
          resolveProviderAccountGrant(grantRepository, {
            actorId: session.credential.id,
            grantRef: authGrantRef,
            providerAccountRef,
          }),
      )
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
    const grantRef = optionalString(body.grantRef)
    const kind = optionalString(body.kind)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const runnerSessionId = optionalString(body.runnerSessionId)
    const secretRef = optionalString(body.secretRef)

    if (
      grantRef === undefined ||
      kind === undefined ||
      providerAccountRef === undefined ||
      runnerSessionId === undefined ||
      secretRef === undefined
    ) {
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_request_invalid',
          message:
            'The Claude secret-material request must include all required scope references.',
        },
        { status: 400 },
      )
    }

    if (kind !== ANTHROPIC_CLAUDE_SECRET_MATERIAL_KIND) {
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_scope_mismatch',
          message: 'The Claude secret-material request scope does not match.',
        },
        { status: 409 },
      )
    }

    try {
      const grantRepository =
        dependencies.providerGrantRepository?.(env) ??
        routeProviderAccountRepository(dependencies, env)
      const candidateGrant = await grantRepository.findGrantByRef(grantRef)

      if (
        candidateGrant === undefined ||
        candidateGrant.userId !== userId ||
        candidateGrant.provider !== ANTHROPIC_CLAUDE_PROVIDER ||
        candidateGrant.providerAccountRef !== providerAccountRef ||
        candidateGrant.providerSecretRef !== secretRef ||
        candidateGrant.runnerSessionId !== runnerSessionId ||
        candidateGrant.requestedAction !== ANTHROPIC_CLAUDE_TURN_ACTION
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_scope_mismatch',
            message: 'The Claude secret-material request scope does not match.',
          },
          { status: 409 },
        )
      }

      const now = Date.parse((dependencies.nowIso ?? currentIsoTimestamp)())
      const grantExpiresAt = Date.parse(candidateGrant.expiresAt)
      if (
        candidateGrant.status !== 'issued' ||
        !Number.isFinite(now) ||
        !Number.isFinite(grantExpiresAt) ||
        grantExpiresAt <= now
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_unavailable',
            message: 'Claude secret material is unavailable.',
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
            error: 'provider_secret_material_unavailable',
            message: 'Claude secret material is unavailable.',
          },
          { status: 409 },
        )
      }

      const resolvedGrant = await observedPromise(
        'ProviderAccountPylon.resolveClaudeGrantForMaterialization',
        () =>
          resolveProviderAccountGrant(grantRepository, {
            actorId: session.credential.id,
            grantRef,
            providerAccountRef,
            runnerSessionId,
          }),
      )

      if (
        resolvedGrant === undefined ||
        resolvedGrant.ownerUserId !== userId ||
        resolvedGrant.provider !== ANTHROPIC_CLAUDE_PROVIDER ||
        resolvedGrant.providerAccountRef !== providerAccountRef ||
        resolvedGrant.providerSecretRef !== secretRef ||
        resolvedGrant.runnerSessionId !== runnerSessionId ||
        resolvedGrant.requestedAction !== ANTHROPIC_CLAUDE_TURN_ACTION ||
        !('kind' in resolvedGrant.materialization) ||
        resolvedGrant.materialization.kind !==
          ANTHROPIC_CLAUDE_SECRET_MATERIAL_KIND
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_scope_mismatch',
            message: 'The Claude secret-material request scope does not match.',
          },
          { status: 409 },
        )
      }

      return noStoreJsonResponse(
        withPylonLinkMetadata({
          schema: 'openagents.pylon.provider_account.claude_auth_material.v1',
          status: 'issued',
          grantRef,
          providerAccountRef,
          runnerSessionId,
          secretRef,
          authMaterial,
        }),
      )
    } catch (error) {
      logWorkerRouteError('pylon_provider_claude_auth_material_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        grantRef,
        providerAccountRef,
        runnerSessionId,
        secretRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_unavailable',
          message: 'Claude secret material is unavailable.',
        },
        { status: providerAccountRouteErrorStatus(error, 409) },
      )
    }
  },

  handlePylonProviderGoogleGeminiAuthMaterialApi: async (
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
    const grantRef = optionalString(body.grantRef)
    const kind = optionalString(body.kind)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const runnerSessionId = optionalString(body.runnerSessionId)
    const secretRef = optionalString(body.secretRef)

    if (
      grantRef === undefined ||
      kind === undefined ||
      providerAccountRef === undefined ||
      runnerSessionId === undefined ||
      secretRef === undefined
    ) {
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_request_invalid',
          message:
            'The Gemini secret-material request must include all required scope references.',
        },
        { status: 400 },
      )
    }

    if (kind !== GOOGLE_GEMINI_SECRET_MATERIAL_KIND) {
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_scope_mismatch',
          message: 'The Gemini secret-material request scope does not match.',
        },
        { status: 409 },
      )
    }

    try {
      const grantRepository =
        dependencies.providerGrantRepository?.(env) ??
        routeProviderAccountRepository(dependencies, env)
      const candidateGrant = await grantRepository.findGrantByRef(grantRef)

      if (
        candidateGrant === undefined ||
        candidateGrant.userId !== userId ||
        candidateGrant.provider !== GOOGLE_GEMINI_PROVIDER ||
        candidateGrant.providerAccountRef !== providerAccountRef ||
        candidateGrant.providerSecretRef !== secretRef ||
        candidateGrant.runnerSessionId !== runnerSessionId ||
        candidateGrant.requestedAction !== GOOGLE_GEMINI_TURN_ACTION
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_scope_mismatch',
            message: 'The Gemini secret-material request scope does not match.',
          },
          { status: 409 },
        )
      }

      const secretValue =
        dependencies.readGoogleGeminiSecretMaterial?.(env)?.trim()
      if (secretValue === undefined || secretValue === '') {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_unavailable',
            message: 'Gemini secret material is unavailable.',
          },
          { status: 409 },
        )
      }

      const resolvedGrant = await observedPromise(
        'ProviderAccountPylon.resolveGoogleGeminiGrantForMaterialization',
        () =>
          resolveProviderAccountGrant(grantRepository, {
            actorId: session.credential.id,
            grantRef,
            providerAccountRef,
            runnerSessionId,
          }),
      )

      if (
        resolvedGrant === undefined ||
        resolvedGrant.ownerUserId !== userId ||
        resolvedGrant.provider !== GOOGLE_GEMINI_PROVIDER ||
        resolvedGrant.providerAccountRef !== providerAccountRef ||
        resolvedGrant.providerSecretRef !== secretRef ||
        resolvedGrant.runnerSessionId !== runnerSessionId ||
        resolvedGrant.requestedAction !== GOOGLE_GEMINI_TURN_ACTION
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_scope_mismatch',
            message: 'The Gemini secret-material request scope does not match.',
          },
          { status: 409 },
        )
      }

      return noStoreJsonResponse({
        schemaVersion: 'openagents.provider_secret_material.v1',
        grantRef,
        providerAccountRef,
        runnerSessionId,
        secretRef,
        secretValue,
      })
    } catch (error) {
      logWorkerRouteError('pylon_provider_gemini_auth_material_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        grantRef,
        providerAccountRef,
        runnerSessionId,
        secretRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_unavailable',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error, 409) },
      )
    }
  },

  handlePylonProviderHarnessAuthMaterialApi: async (
    request: Request,
    env: Bindings,
    provider: typeof CURSOR_PROVIDER | typeof XAI_GROK_PROVIDER,
  ) => {
    if (request.method !== 'POST') return methodNotAllowed(['POST'])
    const session = await requireAgent(dependencies, request, env)
    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }
    const userId = linkedOpenAuthOwnerUserId(session)
    if (userId === undefined) return pylonAgentNotLinkedResponse()
    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const grantRef = optionalString(body.grantRef)
    const kind = optionalString(body.kind)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const runnerSessionId = optionalString(body.runnerSessionId)
    const secretRef = optionalString(body.secretRef)
    if (
      grantRef === undefined ||
      kind === undefined ||
      providerAccountRef === undefined ||
      runnerSessionId === undefined ||
      secretRef === undefined
    ) {
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_request_invalid',
          message:
            'The harness secret-material request must include all required scope references.',
        },
        { status: 400 },
      )
    }
    const expectedKind =
      provider === CURSOR_PROVIDER
        ? CURSOR_SECRET_MATERIAL_KIND
        : XAI_SECRET_MATERIAL_KIND
    const expectedAction =
      provider === CURSOR_PROVIDER ? CURSOR_TURN_ACTION : XAI_GROK_TURN_ACTION
    if (kind !== expectedKind) {
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_scope_mismatch',
          message: 'The harness secret-material request scope does not match.',
        },
        { status: 409 },
      )
    }
    try {
      const grantRepository =
        dependencies.providerGrantRepository?.(env) ??
        routeProviderAccountRepository(dependencies, env)
      const candidateGrant = await grantRepository.findGrantByRef(grantRef)
      if (
        candidateGrant === undefined ||
        candidateGrant.userId !== userId ||
        candidateGrant.provider !== provider ||
        candidateGrant.providerAccountRef !== providerAccountRef ||
        candidateGrant.providerSecretRef !== secretRef ||
        candidateGrant.runnerSessionId !== runnerSessionId ||
        candidateGrant.requestedAction !== expectedAction
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_scope_mismatch',
            message: 'The harness secret-material request scope does not match.',
          },
          { status: 409 },
        )
      }
      const secretValue = (
        provider === CURSOR_PROVIDER
          ? dependencies.readCursorSecretMaterial?.(env)
          : dependencies.readXaiSecretMaterial?.(env)
      )?.trim()
      if (secretValue === undefined || secretValue === '') {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_unavailable',
            message: 'Harness secret material is unavailable.',
          },
          { status: 409 },
        )
      }
      const resolvedGrant = await resolveProviderAccountGrant(grantRepository, {
        actorId: session.credential.id,
        grantRef,
        providerAccountRef,
        runnerSessionId,
      })
      if (
        resolvedGrant === undefined ||
        resolvedGrant.ownerUserId !== userId ||
        resolvedGrant.provider !== provider ||
        resolvedGrant.providerAccountRef !== providerAccountRef ||
        resolvedGrant.providerSecretRef !== secretRef ||
        resolvedGrant.runnerSessionId !== runnerSessionId ||
        resolvedGrant.requestedAction !== expectedAction
      ) {
        return noStoreJsonResponse(
          {
            error: 'provider_secret_material_scope_mismatch',
            message: 'The harness secret-material request scope does not match.',
          },
          { status: 409 },
        )
      }
      return noStoreJsonResponse({
        schemaVersion: 'openagents.provider_secret_material.v1',
        grantRef,
        providerAccountRef,
        runnerSessionId,
        secretRef,
        secretValue,
      })
    } catch (error) {
      logWorkerRouteError('pylon_provider_harness_auth_material_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        grantRef,
        provider,
        providerAccountRef,
        runnerSessionId,
      })
      return noStoreJsonResponse(
        {
          error: 'provider_secret_material_unavailable',
          message: 'Harness secret material is unavailable.',
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

  /**
   * CX-5 (#8549): the write-side counterpart to
   * `handlePylonProviderClaudeAuthMaterialApi` (the broker read CX-5's first
   * pass landed). Imports a `CLAUDE_CODE_OAUTH_TOKEN` the owner obtained
   * locally via `claude setup-token` — never a live `~/.claude` session read
   * or a `claude login` invocation from this route — mirroring
   * `handlePylonProviderLocalCodexAuthImportApi`'s local-auth/import shape
   * for Codex, generalized to Claude's single-secret credential.
   */
  handlePylonProviderLocalClaudeAuthImportApi: async (
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

    if (dependencies.storeConnectedClaudeAuth === undefined) {
      return noStoreJsonResponse(
        {
          error: 'claude_local_auth_import_not_configured',
          message:
            'Claude local auth import is not configured on this deployment.',
        },
        { status: 501 },
      )
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const accountLabel = optionalString(body.accountLabel)
    const createNew = optionalBoolean(body.createNew)
    const providerAccountRef = optionalString(body.providerAccountRef)
    const repository = routeProviderAccountRepository(dependencies, env)
    const storeConnectedClaudeAuth = dependencies.storeConnectedClaudeAuth

    try {
      const result = await observedPromise(
        'ProviderAccountPylon.connectClaudeLocalAuthForUser',
        () =>
          connectClaudeLocalAuthForUser(
            repository,
            {
              userId,
              authContentValue: requiredClaudeAuthContentValue(body),
              ...(accountLabel === undefined ? {} : { accountLabel }),
              ...(createNew === undefined ? {} : { createNew }),
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
            },
            storeConnectedClaudeAuth(env),
          ),
      )

      return noStoreJsonResponse(withPylonLinkMetadata(result), { status: 201 })
    } catch (error) {
      logWorkerRouteError('pylon_provider_local_claude_auth_import_failed', error, {
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
})
