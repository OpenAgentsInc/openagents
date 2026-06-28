import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsWorkerConfigEnv,
  decodeOpenAgentsWorkerConfig,
  getOpenAgentsWorkerConfig,
  redactedValue,
} from './config'

const minimalEnv = (): OpenAgentsWorkerConfigEnv => ({
  GITHUB_CLIENT_ID: 'github-client',
  GITHUB_CLIENT_SECRET: 'github-secret',
  OPENAGENTS_APP_URL: 'https://openagents.com',
  OPENAUTH_CLIENT_ID: 'openauth-client',
  OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
})

describe('OpenAgentsWorkerConfig', () => {
  test('decodes a minimal valid env', async () => {
    const config = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig(minimalEnv()),
    )

    expect(config.app.origin).toBe('https://openagents.com')
    expect(config.github.clientId).toBe('github-client')
    expect(Redacted.isRedacted(config.github.clientSecret)).toBe(true)
    expect(config.email.resend).toBeUndefined()
    expect(config.artanis.scheduledRunnerEnabled).toBe(false)
    expect(config.exa.enabled).toBe(false)
    expect(config.exa.apiKey).toBeUndefined()
    expect(config.exa.assignmentRequestBudget).toBe(12)
    expect(config.exa.baseUrl).toBe('https://api.exa.ai')
    expect(config.exa.cacheTtlHours).toBe(24)
    expect(config.exa.dailyRequestBudget).toBe(200)
    expect(config.exa.defaultSearchType).toBe('auto')
    expect(config.exa.rateLimitBackoffMs).toBe(1000)
    expect(config.exa.retryLimit).toBe(2)
    expect(config.mdk.configured).toBe(false)
    expect(config.mdk.checkout).toMatchObject({
      checkoutPathBase: '/checkout',
      configured: false,
      credentialBindingRef: null,
      environment: 'sandbox',
      providerRef: 'provider.openagents.hosted_mdk.route',
      routeKind: 'fake_provider',
      routeUrl: undefined,
      webhookBindingRef: null,
      webhookSource: 'dashboard_standard_webhooks',
    })
    expect(config.runnerBackends).toEqual({
      automaticFailoverEnabled: false,
      cloudflareContainer: {
        allowedWorkloadTrusts: ['low', 'medium'],
        binding: {},
        configured: false,
        enabled: false,
        policyApproved: false,
        stagingSmokePassed: false,
      },
      gcloud: {
        referenceEnabled: false,
        sensitiveApproved: false,
      },
      policy: 'shc_primary_only',
    })
    expect(config.shc.dispatchMode).toBe('unconfigured')
  })

  test('decodes full env values and redacts secrets', async () => {
    const config = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig({
        ...minimalEnv(),
        ARTANIS_SCHEDULED_RUNNER_ENABLED: 'true',
        EXA_API_KEY: 'exa-secret',
        EXA_ASSIGNMENT_REQUEST_BUDGET: '9',
        EXA_BASE_URL: 'https://api.exa.ai/',
        EXA_CACHE_TTL_HOURS: '6',
        EXA_DAILY_REQUEST_BUDGET: '30',
        EXA_DEFAULT_NUM_RESULTS: '6',
        EXA_DEFAULT_SEARCH_TYPE: 'deep-lite',
        EXA_FRESHNESS_MAX_AGE_HOURS: '12',
        EXA_MAX_HIGHLIGHT_CHARACTERS: '900',
        EXA_MAX_TEXT_CHARACTERS: '4000',
        EXA_RATE_LIMIT_BACKOFF_MS: '250',
        EXA_REQUEST_TIMEOUT_MS: '10000',
        EXA_RETRY_LIMIT: '1',
        MDK_ACCESS_TOKEN: 'mdk-access-token',
        MDK_CHECKOUT_CONFIG_REF: 'config.mdk.checkout.route',
        MDK_CHECKOUT_CREDENTIAL_BINDING_REF:
          'credential_binding.mdk.checkout.route_binding',
        MDK_CHECKOUT_ENVIRONMENT: 'production',
        MDK_CHECKOUT_PATH_BASE: '/checkout/mdk',
        MDK_CHECKOUT_PROVIDER_REF: 'provider.mdk.checkout.route',
        MDK_CHECKOUT_ROUTE_KIND: 'self_hosted_mdkd_sidecar',
        MDK_CHECKOUT_ROUTE_SECRET: 'mdk-checkout-route-secret',
        MDK_CHECKOUT_ROUTE_URL: 'https://mdk-sidecar.openagents.com/api/mdk',
        MDK_CHECKOUT_WEBHOOK_BINDING_REF:
          'webhook_binding.mdk.checkout.standard_webhooks',
        MDK_CHECKOUT_WEBHOOK_SECRET: 'mdk-checkout-webhook-secret',
        MDK_CHECKOUT_WEBHOOK_SOURCE: 'daemon_invoice_hmac',
        MDK_MNEMONIC: 'mdk-mnemonic',
        MDK_WALLET_MNEMONIC: 'mdk-wallet-mnemonic',
        OPENAGENTS_ADMIN_API_TOKEN: 'admin-token',
        RESEND_API_KEY: 're_test',
        RESEND_FROM_EMAIL: 'OpenAgents <billing@openagents.com>',
        RESEND_REPLY_TO_EMAIL: 'support@openagents.com',
        RUNNER_AUTOMATIC_FAILOVER_ENABLED: 'false',
        RUNNER_BACKEND_POLICY:
          'shc_primary_cloudflare_container_backup_gcloud_reference',
        RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS: 'low,medium',
        RUNNER_CLOUDFLARE_CONTAINER_CLASS_NAME: 'OpenAgentsSiteRunnerContainer',
        RUNNER_CLOUDFLARE_CONTAINER_CONFIGURED: 'true',
        RUNNER_CLOUDFLARE_CONTAINER_DURABLE_OBJECT_BINDING:
          'SITE_RUNNER_CONTAINER',
        RUNNER_CLOUDFLARE_CONTAINER_ENABLED: 'true',
        RUNNER_CLOUDFLARE_CONTAINER_IMAGE_REF:
          './containers/site-runner/Dockerfile',
        RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE: 'lite',
        RUNNER_CLOUDFLARE_CONTAINER_MAX_INSTANCES: '2',
        RUNNER_CLOUDFLARE_CONTAINER_POLICY_APPROVED: 'true',
        RUNNER_CLOUDFLARE_CONTAINER_STAGING_SMOKE: 'true',
        RUNNER_GCLOUD_REFERENCE_ENABLED: 'true',
        RUNNER_GCLOUD_SENSITIVE_APPROVED: 'true',
        SHC_CONTROL_API_BEARER_TOKEN: 'shc-token',
        SHC_CONTROL_API_URL: 'https://shc.openagents.com/v1',
        SHC_DISPATCH_MODE: 'live',
        SHC_RUNNER_CALLBACK_TOKEN: 'runner-token',
      }),
    )

    expect(redactedValue(config.adminApiToken)).toBe('admin-token')
    expect(config.artanis.scheduledRunnerEnabled).toBe(true)
    expect(config.exa.enabled).toBe(true)
    expect(redactedValue(config.exa.apiKey)).toBe('exa-secret')
    expect(config.exa.assignmentRequestBudget).toBe(9)
    expect(config.exa.baseUrl).toBe('https://api.exa.ai')
    expect(config.exa.cacheTtlHours).toBe(6)
    expect(config.exa.dailyRequestBudget).toBe(30)
    expect(config.exa.defaultNumResults).toBe(6)
    expect(config.exa.defaultSearchType).toBe('deep-lite')
    expect(config.exa.freshnessMaxAgeHours).toBe(12)
    expect(config.exa.maxHighlightCharacters).toBe(900)
    expect(config.exa.maxTextCharacters).toBe(4000)
    expect(config.exa.rateLimitBackoffMs).toBe(250)
    expect(config.exa.requestTimeoutMs).toBe(10000)
    expect(config.exa.retryLimit).toBe(1)
    expect(redactedValue(config.email.resend?.apiKey)).toBe('re_test')
    expect(config.email.resend?.fromEmail).toBe(
      'OpenAgents <billing@openagents.com>',
    )
    expect(config.email.resend?.replyToEmail).toBe('support@openagents.com')
    expect(config.mdk.configured).toBe(true)
    expect(redactedValue(config.mdk.accessToken)).toBe('mdk-access-token')
    expect(config.mdk.checkout).toMatchObject({
      checkoutPathBase: '/checkout/mdk',
      configRef: 'config.mdk.checkout.route',
      configured: true,
      credentialBindingRef: 'credential_binding.mdk.checkout.route_binding',
      environment: 'production',
      providerRef: 'provider.mdk.checkout.route',
      routeKind: 'self_hosted_mdkd_sidecar',
      routeUrl: 'https://mdk-sidecar.openagents.com/api/mdk',
      webhookBindingRef: 'webhook_binding.mdk.checkout.standard_webhooks',
      webhookSource: 'daemon_invoice_hmac',
    })
    expect(redactedValue(config.mdk.checkout.routeSecret)).toBe(
      'mdk-checkout-route-secret',
    )
    expect(redactedValue(config.mdk.checkout.webhookSecret)).toBe(
      'mdk-checkout-webhook-secret',
    )
    expect(redactedValue(config.mdk.mnemonic)).toBe('mdk-mnemonic')
    expect(redactedValue(config.mdk.walletMnemonic)).toBe('mdk-wallet-mnemonic')
    expect(config.runnerBackends).toEqual({
      automaticFailoverEnabled: false,
      cloudflareContainer: {
        allowedWorkloadTrusts: ['low', 'medium'],
        binding: {
          className: 'OpenAgentsSiteRunnerContainer',
          durableObjectBinding: 'SITE_RUNNER_CONTAINER',
          imageRef: './containers/site-runner/Dockerfile',
          instanceType: 'lite',
          maxInstances: 2,
        },
        configured: true,
        enabled: true,
        policyApproved: true,
        stagingSmokePassed: true,
      },
      gcloud: {
        referenceEnabled: true,
        sensitiveApproved: true,
      },
      policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
    })
    expect(redactedValue(config.shc.controlApiBearerToken)).toBe('shc-token')
    expect(config.shc.controlApiUrl).toBe('https://shc.openagents.com/v1')
    expect(config.shc.dispatchMode).toBe('live')
    expect(redactedValue(config.shc.runnerCallbackToken)).toBe('runner-token')
  })

  test('fails when required config is missing', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          GITHUB_CLIENT_SECRET: '',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'GITHUB_CLIENT_SECRET',
    })
  })

  test('fails on malformed required and optional URLs', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          OPENAGENTS_APP_URL: 'not a url',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'OPENAGENTS_APP_URL',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          SHC_CONTROL_API_URL: 'bad shc url',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'SHC_CONTROL_API_URL',
    })
  })

  test('fails when live SHC dispatch is missing required settings', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          SHC_DISPATCH_MODE: 'live',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'SHC_CONTROL_API_URL',
    })
  })

  test('fails on malformed secret and email config values', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          RESEND_API_KEY: '',
          RESEND_FROM_EMAIL: 'OpenAgents <billing@openagents.com>',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'RESEND_API_KEY',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          RESEND_API_KEY: 're_test',
          RESEND_FROM_EMAIL: 'not-email',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'RESEND_FROM_EMAIL',
    })
  })

  test('fails on malformed SHC dispatch mode', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          SHC_DISPATCH_MODE: 'dry-run',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'SHC_DISPATCH_MODE',
    })
  })

  test('fails on malformed runner backend config', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          RUNNER_BACKEND_POLICY: 'containers_first',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'RUNNER_BACKEND_POLICY',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          RUNNER_CLOUDFLARE_CONTAINER_ENABLED: 'maybe',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'RUNNER_CLOUDFLARE_CONTAINER_ENABLED',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS: 'low,critical',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE: 'huge',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE',
    })
  })

  test('fails on malformed Exa config values', async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          EXA_DEFAULT_SEARCH_TYPE: 'livecrawl',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'EXA_DEFAULT_SEARCH_TYPE',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          EXA_DEFAULT_NUM_RESULTS: '0',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'EXA_DEFAULT_NUM_RESULTS',
    })

    await expect(
      Effect.runPromise(
        decodeOpenAgentsWorkerConfig({
          ...minimalEnv(),
          EXA_RETRY_LIMIT: '-1',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'OpenAgentsWorkerConfigError',
      field: 'EXA_RETRY_LIMIT',
    })
  })

  test('caches sync boundary config per env object', () => {
    const env = minimalEnv()
    const first = getOpenAgentsWorkerConfig(env)
    const second = getOpenAgentsWorkerConfig(env)

    expect(first).toBe(second)
  })
})
