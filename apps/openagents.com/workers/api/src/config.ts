import { Effect, Layer, Redacted, Schema as S } from 'effect'
import * as Context from 'effect/Context'

export type OpenAgentsWorkerConfigEnv = Readonly<{
  ARTANIS_SCHEDULED_RUNNER_ENABLED?: string | undefined
  EXA_API_KEY?: string | undefined
  EXA_BASE_URL?: string | undefined
  EXA_DEFAULT_NUM_RESULTS?: string | undefined
  EXA_DEFAULT_SEARCH_TYPE?: string | undefined
  EXA_FRESHNESS_MAX_AGE_HOURS?: string | undefined
  EXA_ASSIGNMENT_REQUEST_BUDGET?: string | undefined
  EXA_CACHE_TTL_HOURS?: string | undefined
  EXA_DAILY_REQUEST_BUDGET?: string | undefined
  EXA_MAX_HIGHLIGHT_CHARACTERS?: string | undefined
  EXA_MAX_TEXT_CHARACTERS?: string | undefined
  EXA_REQUEST_TIMEOUT_MS?: string | undefined
  EXA_RETRY_LIMIT?: string | undefined
  EXA_RATE_LIMIT_BACKOFF_MS?: string | undefined
  GITHUB_CLIENT_ID?: string | undefined
  GITHUB_CLIENT_SECRET?: string | undefined
  MDK_ACCESS_TOKEN?: string | undefined
  MDK_CHECKOUT_CONFIG_REF?: string | undefined
  MDK_CHECKOUT_CREDENTIAL_BINDING_REF?: string | undefined
  MDK_CHECKOUT_ENVIRONMENT?: string | undefined
  MDK_CHECKOUT_PATH_BASE?: string | undefined
  MDK_CHECKOUT_PROVIDER_REF?: string | undefined
  MDK_CHECKOUT_ROUTE_KIND?: string | undefined
  MDK_CHECKOUT_ROUTE_SECRET?: string | undefined
  MDK_CHECKOUT_ROUTE_URL?: string | undefined
  MDK_CHECKOUT_WEBHOOK_BINDING_REF?: string | undefined
  MDK_CHECKOUT_WEBHOOK_SECRET?: string | undefined
  MDK_CHECKOUT_WEBHOOK_SOURCE?: string | undefined
  MDK_WEBHOOK_SECRET?: string | undefined
  MDK_MNEMONIC?: string | undefined
  MDK_TIPS_BUFFER_ACCESS_TOKEN?: string | undefined
  MDK_TIPS_BUFFER_MNEMONIC?: string | undefined
  MDK_TIPS_BUFFER_SERVICE_TOKEN?: string | undefined
  MDK_TREASURY_ACCESS_TOKEN?: string | undefined
  MDK_TREASURY_MNEMONIC?: string | undefined
  MDK_TREASURY_SERVICE_TOKEN?: string | undefined
  MDK_WALLET_MNEMONIC?: string | undefined
  OPENAGENTS_ADMIN_API_TOKEN?: string | undefined
  OPENAGENTS_APP_URL?: string | undefined
  OPENAUTH_CLIENT_ID?: string | undefined
  OPENAUTH_ISSUER_URL?: string | undefined
  RESEND_API_KEY?: string | undefined
  RESEND_FROM_EMAIL?: string | undefined
  RESEND_REPLY_TO_EMAIL?: string | undefined
  RESEND_WEBHOOK_SECRET?: string | undefined
  RUNNER_AUTOMATIC_FAILOVER_ENABLED?: string | undefined
  RUNNER_BACKEND_POLICY?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_CLASS_NAME?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_CONFIGURED?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_DURABLE_OBJECT_BINDING?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_ENABLED?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_IMAGE_REF?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_MAX_INSTANCES?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_POLICY_APPROVED?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_STAGING_SMOKE?: string | undefined
  RUNNER_GCLOUD_REFERENCE_ENABLED?: string | undefined
  RUNNER_GCLOUD_SENSITIVE_APPROVED?: string | undefined
  SHC_CONTROL_API_BEARER_TOKEN?: string | undefined
  SHC_CONTROL_API_URL?: string | undefined
  SHC_DISPATCH_MODE?: string | undefined
  SHC_RUNNER_CALLBACK_TOKEN?: string | undefined
  TREASURY_DISPATCH_DAILY_SATS_CAP?: string | undefined
  TREASURY_DISPATCH_ENABLED?: string | undefined
  TREASURY_DISPATCH_LIQUIDITY_BUFFER_SATS?: string | undefined
  TREASURY_DISPATCH_PAYMENT_TIMEOUT_SECS?: string | undefined
  TREASURY_DISPATCH_PER_RUN_REWARD_CAP?: string | undefined
}>

export const OpenAgentsAppUrl = S.String.pipe(S.brand('OpenAgentsAppUrl'))
export type OpenAgentsAppUrl = typeof OpenAgentsAppUrl.Type

export const OpenAgentsAppOrigin = S.String.pipe(S.brand('OpenAgentsAppOrigin'))
export type OpenAgentsAppOrigin = typeof OpenAgentsAppOrigin.Type

export const OpenAuthIssuerUrl = S.String.pipe(S.brand('OpenAuthIssuerUrl'))
export type OpenAuthIssuerUrl = typeof OpenAuthIssuerUrl.Type

export const OpenAuthIssuerOrigin = S.String.pipe(
  S.brand('OpenAuthIssuerOrigin'),
)
export type OpenAuthIssuerOrigin = typeof OpenAuthIssuerOrigin.Type

export const GitHubClientId = S.String.pipe(S.brand('GitHubClientId'))
export type GitHubClientId = typeof GitHubClientId.Type

export const OpenAuthClientId = S.String.pipe(S.brand('OpenAuthClientId'))
export type OpenAuthClientId = typeof OpenAuthClientId.Type

export const ResendEmailSender = S.String.pipe(S.brand('ResendEmailSender'))
export type ResendEmailSender = typeof ResendEmailSender.Type

export const EmailAddress = S.String.pipe(S.brand('EmailAddress'))
export type EmailAddress = typeof EmailAddress.Type

export const ShcControlApiUrl = S.String.pipe(S.brand('ShcControlApiUrl'))
export type ShcControlApiUrl = typeof ShcControlApiUrl.Type

export const WorkerSecret = S.String.pipe(S.brand('WorkerSecret'))
export type WorkerSecret = typeof WorkerSecret.Type

export type ShcDispatchMode = 'live' | 'unconfigured'

export type RunnerBackendPolicy =
  | 'shc_primary_cloudflare_container_backup_gcloud_reference'
  | 'shc_primary_only'

export type RunnerWorkloadTrust = 'low' | 'medium' | 'sensitive'

export type CloudflareContainerInstanceType =
  | 'basic'
  | 'lite'
  | 'standard-1'
  | 'standard-2'
  | 'standard-3'
  | 'standard-4'

export type ResendEmailConfig = Readonly<{
  apiKey: Redacted.Redacted<WorkerSecret>
  fromEmail: ResendEmailSender
  replyToEmail?: EmailAddress | undefined
}>

export type MdkWorkerConfig = Readonly<{
  accessToken?: Redacted.Redacted<WorkerSecret> | undefined
  checkout: Readonly<{
    checkoutPathBase: string
    configRef: string
    configured: boolean
    credentialBindingRef: string | null
    environment: 'production' | 'sandbox'
    providerRef: string
    routeKind: MdkCheckoutRouteKind
    routeSecret?: Redacted.Redacted<WorkerSecret> | undefined
    routeUrl?: string | undefined
    webhookBindingRef: string | null
    webhookSecret?: Redacted.Redacted<WorkerSecret> | undefined
    webhookSource:
      | 'daemon_invoice_hmac'
      | 'dashboard_standard_webhooks'
      | 'sdk_node_control'
  }>
  configured: boolean
  mnemonic?: Redacted.Redacted<WorkerSecret> | undefined
  walletMnemonic?: Redacted.Redacted<WorkerSecret> | undefined
}>

export type MdkCheckoutRouteKind =
  | 'fake_provider'
  | 'hosted_platform'
  | 'self_hosted_mdkd_sidecar'

export const ExaBaseUrl = S.String.pipe(S.brand('ExaBaseUrl'))
export type ExaBaseUrl = typeof ExaBaseUrl.Type

export type ExaSearchType =
  | 'auto'
  | 'deep'
  | 'deep-lite'
  | 'deep-reasoning'
  | 'fast'
  | 'instant'

export type ExaConfig = Readonly<{
  apiKey?: Redacted.Redacted<WorkerSecret> | undefined
  assignmentRequestBudget: number
  baseUrl: ExaBaseUrl
  cacheTtlHours: number
  dailyRequestBudget: number
  defaultNumResults: number
  defaultSearchType: ExaSearchType
  enabled: boolean
  freshnessMaxAgeHours: number
  maxHighlightCharacters: number
  maxTextCharacters: number
  rateLimitBackoffMs: number
  requestTimeoutMs: number
  retryLimit: number
}>

export type RunnerBackendConfig = Readonly<{
  automaticFailoverEnabled: boolean
  cloudflareContainer: Readonly<{
    allowedWorkloadTrusts: ReadonlyArray<RunnerWorkloadTrust>
    binding: Readonly<{
      className?: string | undefined
      durableObjectBinding?: string | undefined
      imageRef?: string | undefined
      instanceType?: CloudflareContainerInstanceType | undefined
      maxInstances?: number | undefined
    }>
    configured: boolean
    enabled: boolean
    policyApproved: boolean
    stagingSmokePassed: boolean
  }>
  gcloud: Readonly<{
    referenceEnabled: boolean
    sensitiveApproved: boolean
  }>
  policy: RunnerBackendPolicy
}>

export type OpenAgentsWorkerConfigShape = Readonly<{
  adminApiToken?: Redacted.Redacted<WorkerSecret> | undefined
  app: Readonly<{
    origin: OpenAgentsAppOrigin
    url: OpenAgentsAppUrl
  }>
  artanis: Readonly<{
    scheduledRunnerEnabled: boolean
  }>
  email: Readonly<{
    resend?: ResendEmailConfig | undefined
    resendWebhookSecret?: Redacted.Redacted<WorkerSecret> | undefined
  }>
  exa: ExaConfig
  github: Readonly<{
    clientId: GitHubClientId
    clientSecret: Redacted.Redacted<WorkerSecret>
  }>
  mdk: MdkWorkerConfig
  openauth: Readonly<{
    clientId: OpenAuthClientId
    issuerOrigin: OpenAuthIssuerOrigin
    issuerUrl: OpenAuthIssuerUrl
  }>
  runnerBackends: RunnerBackendConfig
  shc: Readonly<{
    controlApiBearerToken?: Redacted.Redacted<WorkerSecret> | undefined
    controlApiUrl?: ShcControlApiUrl | undefined
    dispatchMode: ShcDispatchMode
    runnerCallbackToken?: Redacted.Redacted<WorkerSecret> | undefined
  }>
}>

export class OpenAgentsWorkerConfigError extends S.TaggedErrorClass<OpenAgentsWorkerConfigError>()(
  'OpenAgentsWorkerConfigError',
  {
    field: S.String,
    reason: S.String,
  },
) {}

export class OpenAgentsWorkerConfig extends Context.Service<
  OpenAgentsWorkerConfig,
  OpenAgentsWorkerConfigShape
>()('@openagents/OpenAgentsWorkerConfig') {
  static layer = (env: OpenAgentsWorkerConfigEnv) =>
    Layer.effect(OpenAgentsWorkerConfig, decodeOpenAgentsWorkerConfig(env))
}

const configCache = new WeakMap<object, OpenAgentsWorkerConfigShape>()

const trimmed = (value: string | undefined): string | undefined => {
  const next = value?.trim()

  return next === undefined || next === '' ? undefined : next
}

const requiredString = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<string, OpenAgentsWorkerConfigError> => {
  const value = trimmed(env[field])

  return value === undefined
    ? Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Required configuration value is missing.',
        }),
      )
    : Effect.succeed(value)
}

const optionalString = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): string | undefined => trimmed(env[field])

const emailAddressPattern = /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/

const extractAddress = (value: string): string =>
  value.match(/<([^<>]+)>$/u)?.[1]?.trim() ?? value

const validateEmailAddress = (
  field: keyof OpenAgentsWorkerConfigEnv,
  value: string,
): Effect.Effect<string, OpenAgentsWorkerConfigError> =>
  emailAddressPattern.test(extractAddress(value))
    ? Effect.succeed(value)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a valid email address.',
        }),
      )

const parseUrl = (
  field: keyof OpenAgentsWorkerConfigEnv,
  value: string,
): Effect.Effect<URL, OpenAgentsWorkerConfigError> =>
  Effect.try({
    catch: error =>
      new OpenAgentsWorkerConfigError({
        field,
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: () => new URL(value),
  })

const requiredUrl = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  Readonly<{ origin: string; url: string }>,
  OpenAgentsWorkerConfigError
> =>
  Effect.gen(function* () {
    const value = yield* requiredString(env, field)
    const url = yield* parseUrl(field, value)

    return {
      origin: url.origin,
      url: url.toString(),
    }
  })

const optionalUrl = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<string | undefined, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  return value === undefined
    ? Effect.sync(() => undefined)
    : Effect.map(parseUrl(field, value), url => url.toString())
}

const optionalBooleanFlag = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<boolean, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.succeed(false)
  }

  const normalized = value.toLowerCase()

  if (['1', 'true', 'yes', 'on'].includes(normalized)) {
    return Effect.succeed(true)
  }

  if (['0', 'false', 'no', 'off'].includes(normalized)) {
    return Effect.succeed(false)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field,
      reason: 'Expected a boolean flag value.',
    }),
  )
}

const redacted = (
  field: keyof OpenAgentsWorkerConfigEnv,
  value: string,
): Redacted.Redacted<WorkerSecret> =>
  Redacted.make(WorkerSecret.make(value), { label: field })

const requiredRedacted = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  Redacted.Redacted<WorkerSecret>,
  OpenAgentsWorkerConfigError
> => Effect.map(requiredString(env, field), value => redacted(field, value))

const optionalRedacted = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Redacted.Redacted<WorkerSecret> | undefined => {
  const value = optionalString(env, field)

  return value === undefined ? undefined : redacted(field, value)
}

const optionalPositiveInteger = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
  fallback: number,
): Effect.Effect<number, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.succeed(fallback)
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed > 0
    ? Effect.succeed(parsed)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a positive integer.',
        }),
      )
}

const optionalNonNegativeInteger = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
  fallback: number,
): Effect.Effect<number, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.succeed(fallback)
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed >= 0
    ? Effect.succeed(parsed)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a non-negative integer.',
        }),
      )
}

const optionalPositiveIntegerValue = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<number | undefined, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.sync((): number | undefined => undefined)
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed > 0
    ? Effect.succeed(parsed)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a positive integer.',
        }),
      )
}

const exaSearchType = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ExaSearchType, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'EXA_DEFAULT_SEARCH_TYPE') ?? 'auto'

  if (
    value === 'auto' ||
    value === 'fast' ||
    value === 'instant' ||
    value === 'deep-lite' ||
    value === 'deep' ||
    value === 'deep-reasoning'
  ) {
    return Effect.succeed(value)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'EXA_DEFAULT_SEARCH_TYPE',
      reason:
        'Expected auto, fast, instant, deep-lite, deep, or deep-reasoning.',
    }),
  )
}

const exaConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ExaConfig, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const apiKey = optionalRedacted(env, 'EXA_API_KEY')
    const baseUrl =
      (yield* optionalUrl(env, 'EXA_BASE_URL')) ?? 'https://api.exa.ai/'
    const normalizedBaseUrl = new URL(baseUrl).origin

    return {
      apiKey,
      assignmentRequestBudget: yield* optionalPositiveInteger(
        env,
        'EXA_ASSIGNMENT_REQUEST_BUDGET',
        12,
      ),
      baseUrl: ExaBaseUrl.make(normalizedBaseUrl),
      cacheTtlHours: yield* optionalPositiveInteger(
        env,
        'EXA_CACHE_TTL_HOURS',
        24,
      ),
      dailyRequestBudget: yield* optionalPositiveInteger(
        env,
        'EXA_DAILY_REQUEST_BUDGET',
        200,
      ),
      defaultNumResults: yield* optionalPositiveInteger(
        env,
        'EXA_DEFAULT_NUM_RESULTS',
        8,
      ),
      defaultSearchType: yield* exaSearchType(env),
      enabled: apiKey !== undefined,
      freshnessMaxAgeHours: yield* optionalNonNegativeInteger(
        env,
        'EXA_FRESHNESS_MAX_AGE_HOURS',
        24,
      ),
      maxHighlightCharacters: yield* optionalPositiveInteger(
        env,
        'EXA_MAX_HIGHLIGHT_CHARACTERS',
        1_200,
      ),
      maxTextCharacters: yield* optionalPositiveInteger(
        env,
        'EXA_MAX_TEXT_CHARACTERS',
        6_000,
      ),
      rateLimitBackoffMs: yield* optionalPositiveInteger(
        env,
        'EXA_RATE_LIMIT_BACKOFF_MS',
        1_000,
      ),
      requestTimeoutMs: yield* optionalPositiveInteger(
        env,
        'EXA_REQUEST_TIMEOUT_MS',
        25_000,
      ),
      retryLimit: yield* optionalNonNegativeInteger(env, 'EXA_RETRY_LIMIT', 2),
    }
  })

const shcDispatchMode = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ShcDispatchMode, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'SHC_DISPATCH_MODE')

  if (value === undefined || value === 'unconfigured') {
    return Effect.succeed('unconfigured')
  }

  if (value === 'live') {
    return Effect.succeed('live')
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'SHC_DISPATCH_MODE',
      reason: 'Expected "live" or "unconfigured".',
    }),
  )
}

const runnerBackendPolicy = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<RunnerBackendPolicy, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'RUNNER_BACKEND_POLICY')

  if (value === undefined || value === 'shc_primary_only') {
    return Effect.succeed('shc_primary_only')
  }

  if (value === 'shc_primary_cloudflare_container_backup_gcloud_reference') {
    return Effect.succeed(
      'shc_primary_cloudflare_container_backup_gcloud_reference',
    )
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'RUNNER_BACKEND_POLICY',
      reason:
        'Expected "shc_primary_only" or "shc_primary_cloudflare_container_backup_gcloud_reference".',
    }),
  )
}

const runnerWorkloadTrustValues: ReadonlyArray<RunnerWorkloadTrust> = [
  'low',
  'medium',
  'sensitive',
]

const isRunnerWorkloadTrust = (value: string): value is RunnerWorkloadTrust =>
  runnerWorkloadTrustValues.includes(value as RunnerWorkloadTrust)

const cloudflareContainerAllowedWorkloadTrusts = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  ReadonlyArray<RunnerWorkloadTrust>,
  OpenAgentsWorkerConfigError
> => {
  const value = optionalString(
    env,
    'RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS',
  )

  if (value === undefined) {
    return Effect.succeed(['low', 'medium'])
  }

  const trusts = value
    .split(',')
    .map(part => part.trim())
    .filter(part => part !== '')
  const invalid = trusts.find(trust => !isRunnerWorkloadTrust(trust))

  if (trusts.length === 0 || invalid !== undefined) {
    return Effect.fail(
      new OpenAgentsWorkerConfigError({
        field: 'RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS',
        reason: 'Expected comma-separated low, medium, or sensitive values.',
      }),
    )
  }

  return Effect.succeed([
    ...new Set(trusts),
  ] as ReadonlyArray<RunnerWorkloadTrust>)
}

const cloudflareContainerInstanceTypes: ReadonlyArray<CloudflareContainerInstanceType> =
  ['lite', 'basic', 'standard-1', 'standard-2', 'standard-3', 'standard-4']

const cloudflareContainerInstanceType = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  CloudflareContainerInstanceType | undefined,
  OpenAgentsWorkerConfigError
> => {
  const value = optionalString(env, 'RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE')

  if (value === undefined) {
    return Effect.sync(
      (): CloudflareContainerInstanceType | undefined => undefined,
    )
  }

  if (
    cloudflareContainerInstanceTypes.includes(
      value as CloudflareContainerInstanceType,
    )
  ) {
    return Effect.succeed(value as CloudflareContainerInstanceType)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE',
      reason:
        'Expected lite, basic, standard-1, standard-2, standard-3, or standard-4.',
    }),
  )
}

const runnerBackendConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<RunnerBackendConfig, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    return {
      automaticFailoverEnabled: yield* optionalBooleanFlag(
        env,
        'RUNNER_AUTOMATIC_FAILOVER_ENABLED',
      ),
      cloudflareContainer: {
        allowedWorkloadTrusts:
          yield* cloudflareContainerAllowedWorkloadTrusts(env),
        binding: {
          className: optionalString(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_CLASS_NAME',
          ),
          durableObjectBinding: optionalString(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_DURABLE_OBJECT_BINDING',
          ),
          imageRef: optionalString(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_IMAGE_REF',
          ),
          instanceType: yield* cloudflareContainerInstanceType(env),
          maxInstances: yield* optionalPositiveIntegerValue(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_MAX_INSTANCES',
          ),
        },
        configured: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_CONFIGURED',
        ),
        enabled: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_ENABLED',
        ),
        policyApproved: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_POLICY_APPROVED',
        ),
        stagingSmokePassed: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_STAGING_SMOKE',
        ),
      },
      gcloud: {
        referenceEnabled: yield* optionalBooleanFlag(
          env,
          'RUNNER_GCLOUD_REFERENCE_ENABLED',
        ),
        sensitiveApproved: yield* optionalBooleanFlag(
          env,
          'RUNNER_GCLOUD_SENSITIVE_APPROVED',
        ),
      },
      policy: yield* runnerBackendPolicy(env),
    }
  })

const resendConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ResendEmailConfig | undefined, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const hasAnyResendValue =
      optionalString(env, 'RESEND_API_KEY') !== undefined ||
      optionalString(env, 'RESEND_FROM_EMAIL') !== undefined ||
      optionalString(env, 'RESEND_REPLY_TO_EMAIL') !== undefined

    if (!hasAnyResendValue) {
      return undefined
    }

    const fromEmail = yield* Effect.flatMap(
      requiredString(env, 'RESEND_FROM_EMAIL'),
      value => validateEmailAddress('RESEND_FROM_EMAIL', value),
    )
    const replyToEmail = optionalString(env, 'RESEND_REPLY_TO_EMAIL')

    if (replyToEmail !== undefined) {
      yield* validateEmailAddress('RESEND_REPLY_TO_EMAIL', replyToEmail)
    }

    return {
      apiKey: yield* requiredRedacted(env, 'RESEND_API_KEY'),
      fromEmail: ResendEmailSender.make(fromEmail),
      replyToEmail:
        replyToEmail === undefined
          ? undefined
          : EmailAddress.make(replyToEmail),
    }
  })

const mdkCheckoutRouteKind = (
  env: OpenAgentsWorkerConfigEnv,
  routeUrl: string | undefined,
): Effect.Effect<MdkCheckoutRouteKind, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'MDK_CHECKOUT_ROUTE_KIND')

  if (value === undefined) {
    return Effect.succeed(
      routeUrl === undefined ? 'fake_provider' : 'hosted_platform',
    )
  }

  if (
    value === 'fake_provider' ||
    value === 'hosted_platform' ||
    value === 'self_hosted_mdkd_sidecar'
  ) {
    return Effect.succeed(value)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'MDK_CHECKOUT_ROUTE_KIND',
      reason:
        'Expected fake_provider, hosted_platform, or self_hosted_mdkd_sidecar.',
    }),
  )
}

const mdkConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<MdkWorkerConfig, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const accessToken = optionalRedacted(env, 'MDK_ACCESS_TOKEN')
    const checkoutRouteSecret =
      optionalRedacted(env, 'MDK_CHECKOUT_ROUTE_SECRET') ?? accessToken
    const checkoutRouteUrl = optionalString(env, 'MDK_CHECKOUT_ROUTE_URL')
    const routeKind = yield* mdkCheckoutRouteKind(env, checkoutRouteUrl)
    const checkoutWebhookSecret =
      optionalRedacted(env, 'MDK_CHECKOUT_WEBHOOK_SECRET') ??
      optionalRedacted(env, 'MDK_WEBHOOK_SECRET')
    const mnemonic = optionalRedacted(env, 'MDK_MNEMONIC')
    const walletMnemonic = optionalRedacted(env, 'MDK_WALLET_MNEMONIC')
    const checkoutEnvironment =
      optionalString(env, 'MDK_CHECKOUT_ENVIRONMENT') === 'production'
        ? 'production'
        : 'sandbox'
    const checkoutWebhookSourceInput = optionalString(
      env,
      'MDK_CHECKOUT_WEBHOOK_SOURCE',
    )
    const checkoutWebhookSource =
      checkoutWebhookSourceInput === 'daemon_invoice_hmac' ||
      checkoutWebhookSourceInput === 'sdk_node_control'
        ? checkoutWebhookSourceInput
        : 'dashboard_standard_webhooks'

    return {
      accessToken,
      checkout: {
        checkoutPathBase:
          optionalString(env, 'MDK_CHECKOUT_PATH_BASE') ?? '/checkout',
        configRef:
          optionalString(env, 'MDK_CHECKOUT_CONFIG_REF') ??
          'config.openagents.hosted_mdk.route',
        configured:
          checkoutRouteSecret !== undefined && checkoutRouteUrl !== undefined,
        credentialBindingRef:
          optionalString(env, 'MDK_CHECKOUT_CREDENTIAL_BINDING_REF') ??
          (checkoutRouteSecret === undefined
            ? null
            : 'credential_binding.openagents.hosted_mdk.route_binding'),
        environment: checkoutEnvironment,
        providerRef:
          optionalString(env, 'MDK_CHECKOUT_PROVIDER_REF') ??
          'provider.openagents.hosted_mdk.route',
        routeKind,
        routeSecret: checkoutRouteSecret,
        routeUrl: checkoutRouteUrl,
        webhookBindingRef:
          optionalString(env, 'MDK_CHECKOUT_WEBHOOK_BINDING_REF') ??
          (checkoutWebhookSecret === undefined
            ? null
            : `webhook_binding.openagents.hosted_mdk.${checkoutWebhookSource}`),
        webhookSecret: checkoutWebhookSecret,
        webhookSource: checkoutWebhookSource,
      },
      configured:
        accessToken !== undefined ||
        checkoutRouteSecret !== undefined ||
        checkoutRouteUrl !== undefined ||
        checkoutWebhookSecret !== undefined ||
        mnemonic !== undefined ||
        walletMnemonic !== undefined,
      mnemonic,
      walletMnemonic,
    }
  })

const validateLiveShc = (
  env: OpenAgentsWorkerConfigEnv,
  dispatchMode: ShcDispatchMode,
  controlApiUrl: string | undefined,
  controlApiBearerToken: Redacted.Redacted<string> | undefined,
): Effect.Effect<void, OpenAgentsWorkerConfigError> => {
  if (dispatchMode !== 'live') {
    return Effect.sync(() => undefined)
  }

  if (controlApiUrl === undefined) {
    return Effect.fail(
      new OpenAgentsWorkerConfigError({
        field: 'SHC_CONTROL_API_URL',
        reason:
          'SHC_CONTROL_API_URL is required when SHC_DISPATCH_MODE is live.',
      }),
    )
  }

  if (controlApiBearerToken === undefined) {
    return Effect.fail(
      new OpenAgentsWorkerConfigError({
        field: 'SHC_CONTROL_API_BEARER_TOKEN',
        reason:
          'SHC_CONTROL_API_BEARER_TOKEN is required when SHC_DISPATCH_MODE is live.',
      }),
    )
  }

  return Effect.sync(() => undefined)
}

export const decodeOpenAgentsWorkerConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<OpenAgentsWorkerConfigShape, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const app = yield* requiredUrl(env, 'OPENAGENTS_APP_URL')
    const issuer = yield* requiredUrl(env, 'OPENAUTH_ISSUER_URL')
    const dispatchMode = yield* shcDispatchMode(env)
    const controlApiUrl = yield* optionalUrl(env, 'SHC_CONTROL_API_URL')
    const controlApiBearerToken = optionalRedacted(
      env,
      'SHC_CONTROL_API_BEARER_TOKEN',
    )

    yield* validateLiveShc(
      env,
      dispatchMode,
      controlApiUrl,
      controlApiBearerToken,
    )

    return {
      adminApiToken: optionalRedacted(env, 'OPENAGENTS_ADMIN_API_TOKEN'),
      app: {
        origin: OpenAgentsAppOrigin.make(app.origin),
        url: OpenAgentsAppUrl.make(app.url),
      },
      artanis: {
        scheduledRunnerEnabled: yield* optionalBooleanFlag(
          env,
          'ARTANIS_SCHEDULED_RUNNER_ENABLED',
        ),
      },
      email: {
        resend: yield* resendConfig(env),
        resendWebhookSecret: optionalRedacted(env, 'RESEND_WEBHOOK_SECRET'),
      },
      exa: yield* exaConfig(env),
      github: {
        clientId: GitHubClientId.make(
          yield* requiredString(env, 'GITHUB_CLIENT_ID'),
        ),
        clientSecret: yield* requiredRedacted(env, 'GITHUB_CLIENT_SECRET'),
      },
      mdk: yield* mdkConfig(env),
      openauth: {
        clientId: OpenAuthClientId.make(
          yield* requiredString(env, 'OPENAUTH_CLIENT_ID'),
        ),
        issuerOrigin: OpenAuthIssuerOrigin.make(issuer.origin),
        issuerUrl: OpenAuthIssuerUrl.make(issuer.url),
      },
      runnerBackends: yield* runnerBackendConfig(env),
      shc: {
        controlApiBearerToken,
        controlApiUrl:
          controlApiUrl === undefined
            ? undefined
            : ShcControlApiUrl.make(controlApiUrl),
        dispatchMode,
        runnerCallbackToken: optionalRedacted(env, 'SHC_RUNNER_CALLBACK_TOKEN'),
      },
    }
  })

export const getOpenAgentsWorkerConfig = (
  env: OpenAgentsWorkerConfigEnv,
): OpenAgentsWorkerConfigShape => {
  const cached = configCache.get(env)

  if (cached !== undefined) {
    return cached
  }

  const config = Effect.runSync(decodeOpenAgentsWorkerConfig(env))
  configCache.set(env, config)

  return config
}

export const redactedValue = (
  value: Redacted.Redacted<string> | undefined,
): string | undefined =>
  value === undefined ? undefined : Redacted.value(value)
