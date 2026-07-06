import type { AuthKvStore } from './auth/auth-kv'
import type { AutopilotTokenUsage } from '@openagentsinc/sync-schema'
import { Schema as S } from 'effect'

import {
  executeBuiltinComputeAgentGrant,
  makeD1BuiltinComputeAgentStore,
} from './builtin-compute-agent-grant'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { inferenceEntitlementsMirrorForEnv } from './inference-entitlements-store'
import {
  optionalBoolean,
  optionalString,
  parseJsonRecord,
  readJsonObject,
} from './json-boundary'
import { logWorkerRouteError, observedPromise } from './observability'
import {
  providerAccountRouteErrorMessage,
  providerAccountRouteErrorName,
  providerAccountRouteErrorStatus,
} from './provider-account-route-errors'
import {
  addMilliseconds,
  makeD1ProviderAccountRepository,
  recordDeviceLoginConnected,
  recordDeviceLoginFailed,
  recordProviderAccountHealth,
  resolveProviderAccountGrant,
  systemProviderAccountRuntime,
} from './provider-accounts'
import { openAgentsDatabase, scheduleBackgroundWork } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import { extractAutopilotTokenUsage } from './token-usage'

type ProviderAccountServiceEnv = Readonly<{
  AUTH_KV?: AuthKvStore | undefined
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  GEMINI_API_KEY?: string
  OPENAGENTS_DB: D1Database
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

type ProviderServiceActor = Readonly<{
  user: Readonly<{
    id: string
  }>
}>

type ConnectedCodexAuthMaterial = Readonly<{
  authContentEnv: 'OPENCODE_AUTH_CONTENT'
  authContentJson: string
}>

class ProviderAccountGrantSerializationError extends S.TaggedErrorClass<ProviderAccountGrantSerializationError>()(
  'ProviderAccountGrantSerializationError',
  {
    message: S.String,
    providerAccountRef: S.String,
  },
) {}

type ProviderAccountServiceDependencies<RouteEnv extends ProviderAccountServiceEnv> =
  Readonly<{
    readConnectedCodexAuthMaterial: (
      bindings: RouteEnv,
      ownerUserId: string,
      providerAccountRef: string,
    ) => Promise<ConnectedCodexAuthMaterial | undefined>
    requireProviderServiceActor: (
      request: Request,
      env: RouteEnv,
    ) => Promise<ProviderServiceActor | undefined>
  }>

const optionalFailedStatus = (
  value: unknown,
): 'denied' | 'expired' | 'failed' | undefined =>
  value === 'denied' || value === 'expired' || value === 'failed'
    ? value
    : undefined

const requiredProviderHealth = (
  value: unknown,
): 'healthy' | 'unhealthy' | 'requires_reauth' | undefined =>
  value === 'healthy' || value === 'unhealthy' || value === 'requires_reauth'
    ? value
    : undefined

const runnerResolvedGrantJson = (
  grant: Awaited<ReturnType<typeof resolveProviderAccountGrant>>,
) => {
  if (grant === undefined) {
    return undefined
  }

  const expiresAt = Date.parse(grant.expiresAt)

  if (!Number.isFinite(expiresAt)) {
    throw new ProviderAccountGrantSerializationError({
      message: 'Resolved grant expiry is invalid.',
      providerAccountRef: grant.providerAccountRef,
    })
  }

  return {
    grantRef: grant.grantRef,
    provider: grant.provider,
    providerAccountRef: grant.providerAccountRef,
    providerSecretRef: grant.providerSecretRef,
    ...(grant.requestedAction === undefined
      ? {}
      : { requestedAction: grant.requestedAction }),
    ...(grant.runnerSessionId === undefined
      ? {}
      : { runnerSessionId: grant.runnerSessionId }),
    expiresAt,
    status: 'issued',
    materialization: grant.materialization,
  }
}

const GOOGLE_GEMINI_SECRET_REF =
  'provider-account://google-gemini/worker-secret/GEMINI_API_KEY'
const GOOGLE_GEMINI_PROVIDER_ACCOUNT_REF =
  'provider-account_google_gemini_worker_secret'

const googleGeminiGrantJson = (
  input: Readonly<{
    grantRef: string
    providerAccountRef?: string | undefined
    runnerSessionId?: string | undefined
    now: Date
  }>,
) => {
  const providerAccountRef =
    input.providerAccountRef ?? GOOGLE_GEMINI_PROVIDER_ACCOUNT_REF
  const expiresAt = addMilliseconds(input.now, 1000 * 60 * 60 * 2)

  return {
    grantRef: input.grantRef,
    provider: 'google_gemini',
    providerAccountRef,
    providerSecretRef: GOOGLE_GEMINI_SECRET_REF,
    ...(input.runnerSessionId === undefined
      ? {}
      : { runnerSessionId: input.runnerSessionId }),
    expiresAt,
    status: 'issued',
    materialization: {
      kind: 'probe_gemini_api_key',
      provider: 'google_gemini',
      providerSecretRef: GOOGLE_GEMINI_SECRET_REF,
      target: {
        kind: 'env',
        name: 'GOOGLE_GENERATIVE_AI_API_KEY',
      },
      homeIsolation: 'per_run',
      scrubAfterCloseout: true,
    },
  }
}

const googleGeminiEndpoint = (model: string): string =>
  `https://generativelanguage.googleapis.com/v1beta/models/${encodeURIComponent(
    decodeURIComponent(model),
  )}:streamGenerateContent?alt=sse`

const safeRefPattern = /^[A-Za-z0-9:._-]{1,180}$/

const optionalSafeRefHeader = (
  request: Request,
  name: string,
): string | undefined => {
  const value = request.headers.get(name)?.trim()

  return value !== undefined && safeRefPattern.test(value) ? value : undefined
}

type ProviderUsageResponse = Readonly<{
  ok: boolean
  status: number
  text: () => Promise<string>
}>

const responseStatusLabel = (
  response: Pick<ProviderUsageResponse, 'ok'>,
): 'failed' | 'succeeded' => (response.ok ? 'succeeded' : 'failed')

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const geminiUsageRecordsFromText = (
  text: string,
): ReadonlyArray<Record<string, unknown>> => {
  const direct = parseJsonRecord(text)
  const sseRecords = text
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => line.startsWith('data:'))
    .map(line => line.slice('data:'.length).trim())
    .filter(data => data !== '' && data !== '[DONE]')
    .flatMap(data => {
      const parsed = parseJsonRecord(data)

      return parsed === undefined ? [] : [parsed]
    })

  return direct === undefined ? sseRecords : [direct, ...sseRecords]
}

const googleGeminiTokenUsageFromText = (
  model: string,
  text: string,
): AutopilotTokenUsage | undefined =>
  [...geminiUsageRecordsFromText(text)]
    .reverse()
    .map(record =>
      extractAutopilotTokenUsage({
        ...record,
        model,
        provider: 'google_gemini',
      }),
    )
    .find((usage): usage is AutopilotTokenUsage => usage !== undefined)

const insertGeminiTokenUsageEvent = async <
  RouteEnv extends ProviderAccountServiceEnv,
>(
  input: Readonly<{
    actor: ProviderServiceActor
    bodyHash: string
    env: RouteEnv
    model: string
    request: Request
    response: ProviderUsageResponse
    usage: AutopilotTokenUsage
  }>,
): Promise<void> => {
  const requestIdempotencyKey =
    input.request.headers.get('idempotency-key')?.trim() ||
    input.request.headers.get('x-openagents-idempotency-key')?.trim() ||
    `body:${input.bodyHash}`
  const eventHash = await sha256Hex(
    `omega:google_gemini:${input.actor.user.id}:${input.model}:${requestIdempotencyKey}`,
  )
  const eventId = `token_event_omega_gemini_${eventHash.slice(0, 32)}`
  const idempotencyKey = `omega:google_gemini:${eventHash}`
  const observedAt = currentIsoTimestamp()
  const safeMetadataJson = JSON.stringify({
    providerHttpStatus: input.response.status,
    providerRequestStatus: responseStatusLabel(input.response),
  })

  await openAgentsDatabase(input.env)
    .prepare(
      `INSERT OR IGNORE INTO token_usage_events (
        id,
        idempotency_key,
        observed_at,
        ingested_at,
        producer_system,
        source_route,
        actor_user_id,
        actor_team_id,
        account_ref,
        anonymized_source_ref,
        run_ref,
        session_ref,
        task_ref,
        repository_ref,
        provider,
        model,
        backend_profile,
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_write_5m_tokens,
        cache_write_1h_tokens,
        total_tokens,
        usage_truth,
        cost_amount,
        currency,
        leaderboard_eligible,
        privacy_opt_out,
        safe_metadata_json
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      eventId,
      idempotencyKey,
      observedAt,
      observedAt,
      'omega',
      'omega_provider_broker',
      input.actor.user.id,
      null,
      // Attribution: the omega Gemini broker always serves the worker-secret
      // Gemini provider account, so this ledger row carries that account ref
      // (the same ref the broker hands out in googleGeminiGrantJson).
      GOOGLE_GEMINI_PROVIDER_ACCOUNT_REF,
      `omega-gemini:${eventHash.slice(0, 24)}`,
      optionalSafeRefHeader(input.request, 'x-openagents-run-ref') ?? null,
      optionalSafeRefHeader(input.request, 'x-openagents-session-ref') ?? null,
      optionalSafeRefHeader(input.request, 'x-openagents-task-ref') ?? null,
      optionalSafeRefHeader(input.request, 'x-openagents-repository-ref') ??
        null,
      'google_gemini',
      input.model,
      'worker_secret_gemini_api_key',
      input.usage.inputTokens,
      input.usage.outputTokens,
      input.usage.reasoningTokens,
      input.usage.cacheReadTokens,
      input.usage.cacheWrite5mTokens,
      input.usage.cacheWrite1hTokens,
      input.usage.totalTokens,
      'exact',
      null,
      null,
      1,
      0,
      safeMetadataJson,
    )
    .run()
}

const recordGoogleGeminiTokenUsage = async <
  RouteEnv extends ProviderAccountServiceEnv,
>(
  input: Readonly<{
    actor: ProviderServiceActor
    body: string
    env: RouteEnv
    model: string
    request: Request
    response: ProviderUsageResponse
  }>,
): Promise<void> => {
  const responseText = await input.response.text()
  const usage = googleGeminiTokenUsageFromText(input.model, responseText)

  if (usage === undefined) {
    return
  }

  await insertGeminiTokenUsageEvent({
    actor: input.actor,
    bodyHash: await sha256Hex(input.body),
    env: input.env,
    model: input.model,
    request: input.request,
    response: input.response,
    usage,
  })
}

export const makeProviderAccountServiceHandlers = <
  RouteEnv extends ProviderAccountServiceEnv,
>(
  dependencies: ProviderAccountServiceDependencies<RouteEnv>,
) => ({
  handleProviderDeviceLoginConnectedApi: async (
    request: Request,
    env: RouteEnv,
    attemptId: string,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const providerAccountRef = optionalString(body.providerAccountRef)
    const accountLabel = optionalString(body.accountLabel)
    const planType = optionalString(body.planType)
    const secretRef = optionalString(body.secretRef)

    try {
      const result = await observedPromise(
        'ProviderAccountService.recordDeviceLoginConnected',
        () =>
          recordDeviceLoginConnected(
            makeD1ProviderAccountRepository(openAgentsDatabase(env)),
            {
              actorId: actor.user.id,
              attemptId,
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
              ...(accountLabel === undefined ? {} : { accountLabel }),
              ...(planType === undefined ? {} : { planType }),
              ...(secretRef === undefined ? {} : { secretRef }),
            },
          ),
      )

      if (result === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return noStoreJsonResponse(result)
    } catch (error) {
      logWorkerRouteError('provider_device_login_connected_failed', error, {
        attemptId,
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_device_login_connected_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error) },
      )
    }
  },

  handleProviderDeviceLoginFailedApi: async (
    request: Request,
    env: RouteEnv,
    attemptId: string,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const providerAccountRef = optionalString(body.providerAccountRef)
    const status = optionalFailedStatus(body.status)
    const reason = optionalString(body.reason)

    try {
      const result = await observedPromise(
        'ProviderAccountService.recordDeviceLoginFailed',
        () =>
          recordDeviceLoginFailed(
            makeD1ProviderAccountRepository(openAgentsDatabase(env)),
            {
              actorId: actor.user.id,
              attemptId,
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
              ...(status === undefined ? {} : { status }),
              ...(reason === undefined ? {} : { reason }),
            },
          ),
      )

      if (result === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return noStoreJsonResponse(result)
    } catch (error) {
      logWorkerRouteError(
        'provider_device_login_failed_callback_failed',
        error,
        {
          attemptId,
          errorName: providerAccountRouteErrorName(error),
          providerAccountRef,
        },
      )

      return noStoreJsonResponse(
        {
          error: 'provider_device_login_failed_callback_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error) },
      )
    }
  },

  handleProviderAccountHealthApi: async (
    request: Request,
    env: RouteEnv,
    providerAccountRef: string,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const health = requiredProviderHealth(body.health)
    const reason = optionalString(body.reason)

    if (health === undefined) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: 'health is required' },
        { status: 400 },
      )
    }

    try {
      const account = await observedPromise(
        'ProviderAccountService.recordProviderAccountHealth',
        () =>
          recordProviderAccountHealth(
            makeD1ProviderAccountRepository(openAgentsDatabase(env)),
            {
              actorId: actor.user.id,
              health,
              providerAccountRef,
              ...(reason === undefined ? {} : { reason }),
            },
          ),
      )

      if (account === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      return noStoreJsonResponse({ account })
    } catch (error) {
      logWorkerRouteError('provider_account_health_callback_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        providerAccountRef,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_account_health_callback_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error) },
      )
    }
  },

  handleProviderAccountGrantResolveApi: async (
    request: Request,
    env: RouteEnv,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const grantRef =
      optionalString(body.authGrantRef) ?? optionalString(body.grantRef)

    if (grantRef === undefined) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: 'grantRef is required' },
        { status: 400 },
      )
    }

    const providerAccountRef = optionalString(body.providerAccountRef)
    const runnerSessionId =
      optionalString(body.runnerSessionId) ?? optionalString(body.runId)
    const includeAuthMaterial =
      optionalBoolean(body.includeAuthMaterial) === true

    try {
      const grant = await observedPromise(
        'ProviderAccountService.resolveProviderAccountGrant',
        () =>
          resolveProviderAccountGrant(
            makeD1ProviderAccountRepository(openAgentsDatabase(env)),
            {
              actorId: actor.user.id,
              grantRef,
              ...(providerAccountRef === undefined
                ? {}
                : { providerAccountRef }),
              ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
            },
          ),
      )

      if (grant === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }

      const authMaterial = includeAuthMaterial
        ? await observedPromise(
            'ProviderAccountService.readConnectedCodexAuthMaterial',
            () =>
              dependencies.readConnectedCodexAuthMaterial(
                env,
                grant.ownerUserId,
                grant.providerAccountRef,
              ),
          )
        : undefined

      if (includeAuthMaterial && authMaterial === undefined) {
        return noStoreJsonResponse(
          {
            error: 'provider_account_auth_material_unavailable',
            message:
              'ChatGPT/Codex account material is unavailable. Reconnect ChatGPT in Settings -> Connections.',
          },
          { status: 409 },
        )
      }

      return noStoreJsonResponse({
        grant: runnerResolvedGrantJson(grant),
        ...(authMaterial === undefined ? {} : { authMaterial }),
        status: 'resolved',
      })
    } catch (error) {
      logWorkerRouteError('provider_account_grant_resolve_failed', error, {
        errorName: providerAccountRouteErrorName(error),
        grantRef,
        providerAccountRef,
        runnerSessionId,
      })

      return noStoreJsonResponse(
        {
          error: 'provider_account_grant_resolve_failed',
          message: providerAccountRouteErrorMessage(error),
        },
        { status: providerAccountRouteErrorStatus(error) },
      )
    }
  },

  handleGoogleGeminiGrantResolveApi: async (request: Request, env: RouteEnv) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const grantRef =
      optionalString(body.authGrantRef) ?? optionalString(body.grantRef)

    if (grantRef === undefined) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: 'grantRef is required' },
        { status: 400 },
      )
    }

    const providerAccountRef = optionalString(body.providerAccountRef)
    const runnerSessionId =
      optionalString(body.runnerSessionId) ?? optionalString(body.runId)

    if (env.GEMINI_API_KEY === undefined || env.GEMINI_API_KEY.trim() === '') {
      return noStoreJsonResponse(
        {
          error: 'provider_account_auth_material_unavailable',
          message: 'Gemini API key material is unavailable.',
        },
        { status: 409 },
      )
    }

    return noStoreJsonResponse(
      googleGeminiGrantJson({
        grantRef,
        now: systemProviderAccountRuntime.now(),
        ...(providerAccountRef === undefined ? {} : { providerAccountRef }),
        ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
      }),
    )
  },

  // Keyless, quota-gated hosted-compute grant for a no-key user's built-in
  // agent. Unlike handleGoogleGeminiGrantResolveApi (which resolves an existing
  // provider-account grantRef), this route mints a free-tier grant for the
  // authenticated agent's user WITHOUT requiring a prior grant. It is
  // COST/SECURITY-SENSITIVE: it gates access to the shared hosted Gemini key.
  //
  // - Inert by default: if GEMINI_API_KEY is not configured, it grants nothing
  //   and returns a clean hosted_compute_not_configured error (503).
  // - Over the per-user free-tier daily budget -> builtin_agent_quota_exhausted
  //   (429) with remaining/reset info; grants nothing.
  // - The response carries only the redacted secret-ref materialization, never
  //   the raw key.
  handleGoogleGeminiBuiltinGrantApi: async (request: Request, env: RouteEnv) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const hostedKeyConfigured =
      env.GEMINI_API_KEY !== undefined && env.GEMINI_API_KEY.trim() !== ''

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const providerAccountRef = optionalString(body.providerAccountRef)
    const runnerSessionId =
      optionalString(body.runnerSessionId) ?? optionalString(body.runId)

    const result = await executeBuiltinComputeAgentGrant({
      hostedKeyConfigured,
      session: { user: { id: actor.user.id } },
      store: makeD1BuiltinComputeAgentStore(
        openAgentsDatabase(env),
        // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
        inferenceEntitlementsMirrorForEnv(env),
      ),
      ...(providerAccountRef === undefined ? {} : { providerAccountRef }),
      ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
    })

    if (result.kind === 'not_configured') {
      return noStoreJsonResponse(
        {
          error: 'hosted_compute_not_configured',
          message:
            'Built-in hosted compute is not provisioned. No grant was issued.',
        },
        { status: 503 },
      )
    }

    if (result.kind === 'quota_exhausted') {
      return noStoreJsonResponse(
        {
          dailyTokenCeiling: result.dailyTokenCeiling,
          error: 'builtin_agent_quota_exhausted',
          message:
            'Built-in hosted-compute free daily limit reached. No grant was issued.',
          resetsAt: result.resetsAt,
          sessionsRemaining: result.sessionsRemaining,
        },
        { status: 429 },
      )
    }

    return noStoreJsonResponse({ grant: result.grant, status: 'issued' })
  },

  handleGoogleGeminiGenerateContentApi: async (
    request: Request,
    env: RouteEnv,
    ctx: ExecutionContext,
    model: string,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const actor = await dependencies.requireProviderServiceActor(request, env)

    if (actor === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const apiKey = env.GEMINI_API_KEY

    if (apiKey === undefined || apiKey.trim() === '') {
      return noStoreJsonResponse(
        {
          error: 'provider_account_auth_material_unavailable',
          message: 'Gemini API key material is unavailable.',
        },
        { status: 409 },
      )
    }

    const body = await request.text()
    const response = await fetch(googleGeminiEndpoint(model), {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'x-goog-api-key': apiKey,
      },
      body,
    })
    const responseForUsage = response.clone()
    scheduleBackgroundWork(
      ctx,
      recordGoogleGeminiTokenUsage({
        actor,
        body,
        env,
        model,
        request,
        response: responseForUsage,
      }).catch(error =>
        logWorkerRouteError('google_gemini_token_usage_record_failed', error, {
          actorId: actor.user.id,
          model,
        }),
      ),
    )
    const headers = new Headers()
    headers.set('cache-control', 'no-store')
    headers.set(
      'content-type',
      response.headers.get('content-type') ?? 'text/event-stream',
    )

    return new Response(response.body, {
      headers,
      status: response.status,
      statusText: response.statusText,
    })
  },
})
