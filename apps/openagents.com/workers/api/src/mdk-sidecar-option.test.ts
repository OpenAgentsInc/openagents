import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsWorkerConfigEnv,
  decodeOpenAgentsWorkerConfig,
} from './config'
import type { OpenAgentsMdkSidecarOptionInput } from './mdk-sidecar-option'
import {
  OpenAgentsMdkSidecarOptionProjection,
  OpenAgentsMdkSidecarOptionUnsafe,
  openAgentsMdkSidecarOptionHasPrivateMaterial,
  planOpenAgentsMdkSidecarOption,
} from './mdk-sidecar-option'

const baseAuth = {
  checkoutControlAuthRef: 'credential_binding.mdkd.checkout_control',
  emergencyPauseRef: 'control.mdkd.emergency_pause',
  payoutControlAuthRef: 'credential_binding.mdkd.payout_control',
  readOnlyStatusAuthRef: 'credential_binding.mdkd.read_only_status',
  webhookVerificationRef: 'webhook_binding.mdkd.daemon_hmac',
} as const

const baseInput = (
  overrides: Partial<OpenAgentsMdkSidecarOptionInput> = {},
): OpenAgentsMdkSidecarOptionInput => ({
  auth: baseAuth,
  checkoutRouteConfigured: true,
  emergencyPause: 'inactive',
  healthCheckedRef: 'health.mdkd.sidecar.latest',
  healthStatus: 'healthy',
  mdkdVersionRef: 'mdkd.version.9ffea5f',
  mode: 'self_hosted_mdkd_sidecar',
  observabilityRefs: [
    'observability.mdkd.health',
    'observability.mdkd.reconciliation_lag',
    'observability.mdkd.wallet_readiness_bucket',
  ],
  routeBindingRef: 'service_binding.mdkd.checkout_route',
  runtime: 'shc_node_service',
  serviceRef: 'service.mdkd.openagents.sidecar',
  storageRefs: [
    'storage.mdkd.disk.redacted',
    'storage.mdkd.sqlite.redacted',
    'storage.mdkd.vss.redacted',
  ],
  walletReadinessRef: 'wallet_readiness.mdkd.minimum_satisfied',
  ...overrides,
})

const minimalConfigEnv = (): OpenAgentsWorkerConfigEnv => ({
  GITHUB_CLIENT_ID: 'github-client',
  GITHUB_CLIENT_SECRET: 'github-secret',
  OPENAGENTS_APP_URL: 'https://openagents.com',
  OPENAUTH_CLIENT_ID: 'openauth-client',
  OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
})

describe('OpenAgents MDK sidecar option', () => {
  test('marks a self-hosted mdkd sidecar ready without moving payout authority', () => {
    const projection = planOpenAgentsMdkSidecarOption(baseInput())

    expect(S.decodeUnknownSync(OpenAgentsMdkSidecarOptionProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.readinessStatus).toBe('sidecar_ready')
    expect(projection.checkoutCreationAllowed).toBe(true)
    expect(projection.checkoutStatusLookupAllowed).toBe(true)
    expect(projection.nativeRuntimeInWorker).toBe(false)
    expect(projection.workerCompatibilityPreserved).toBe(true)
    expect(projection.payoutAuthorityOwner).toBe('nexus_treasury_policy')
    expect(projection.payoutDispatchAllowed).toBe(false)
    expect(projection.authTierRefs).toEqual([
      'auth_tier.mdkd.checkout_control',
      'auth_tier.mdkd.emergency_pause',
      'auth_tier.mdkd.payout_control',
      'auth_tier.mdkd.read_only_status',
      'auth_tier.mdkd.webhook_verification',
    ])
    expect(openAgentsMdkSidecarOptionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('distinguishes fake provider, hosted platform, and self-hosted sidecar modes', () => {
    const fake = planOpenAgentsMdkSidecarOption(baseInput({
      auth: {
        checkoutControlAuthRef: null,
        emergencyPauseRef: null,
        payoutControlAuthRef: null,
        readOnlyStatusAuthRef: null,
        webhookVerificationRef: null,
      },
      checkoutRouteConfigured: false,
      healthStatus: 'unknown',
      mdkdVersionRef: null,
      mode: 'fake_provider',
      routeBindingRef: null,
      storageRefs: [],
      walletReadinessRef: null,
    }))
    const hosted = planOpenAgentsMdkSidecarOption(baseInput({
      auth: {
        ...baseAuth,
        payoutControlAuthRef: null,
        readOnlyStatusAuthRef: null,
      },
      mdkdVersionRef: null,
      mode: 'hosted_platform',
      runtime: 'cloudflare_vpc_service',
      storageRefs: [],
      walletReadinessRef: null,
    }))

    expect(fake.readinessStatus).toBe('fake_provider_only')
    expect(fake.checkoutCreationAllowed).toBe(false)
    expect(hosted.readinessStatus).toBe('hosted_platform_ready')
    expect(hosted.checkoutCreationAllowed).toBe(true)
    expect(hosted.payoutDispatchAllowed).toBe(false)
  })

  test('blocks sidecar readiness when route, auth, storage, pause, or health is missing', () => {
    const missingRoute = planOpenAgentsMdkSidecarOption(baseInput({
      checkoutRouteConfigured: false,
      routeBindingRef: null,
    }))
    const missingAuth = planOpenAgentsMdkSidecarOption(baseInput({
      auth: {
        ...baseAuth,
        payoutControlAuthRef: null,
      },
    }))
    const missingStorage = planOpenAgentsMdkSidecarOption(baseInput({
      mdkdVersionRef: null,
      storageRefs: [],
    }))
    const paused = planOpenAgentsMdkSidecarOption(baseInput({
      emergencyPause: 'active',
    }))
    const unreachable = planOpenAgentsMdkSidecarOption(baseInput({
      healthStatus: 'unreachable',
    }))

    expect(missingRoute.readinessStatus).toBe('blocked_missing_route')
    expect(missingAuth.readinessStatus).toBe('blocked_missing_auth')
    expect(missingStorage.readinessStatus).toBe('blocked_missing_storage')
    expect(paused.readinessStatus).toBe('blocked_emergency_pause')
    expect(unreachable.readinessStatus).toBe('blocked_unhealthy')
  })

  test('rejects raw secrets, wallet paths, invoices, and payment material', () => {
    expect(() =>
      planOpenAgentsMdkSidecarOption(baseInput({
        routeBindingRef: 'authorization: basic abcdefghijklmnop',
      })),
    ).toThrow(OpenAgentsMdkSidecarOptionUnsafe)
    expect(() =>
      planOpenAgentsMdkSidecarOption(baseInput({
        storageRefs: ['/var/lib/mdkd/bitcoin/mdkd.sqlite'],
      })),
    ).toThrow(OpenAgentsMdkSidecarOptionUnsafe)
    expect(() =>
      planOpenAgentsMdkSidecarOption(baseInput({
        walletReadinessRef: 'lnbc10n1rawinvoice',
      })),
    ).toThrow(OpenAgentsMdkSidecarOptionUnsafe)
    expect(() =>
      planOpenAgentsMdkSidecarOption(baseInput({
        auth: {
          ...baseAuth,
          checkoutControlAuthRef: 'MDK_ACCESS_TOKEN=secret',
        },
      })),
    ).toThrow(OpenAgentsMdkSidecarOptionUnsafe)
  })

  test('decodes checkout route kind in Worker config', async () => {
    const fake = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig(minimalConfigEnv()),
    )
    const sidecar = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig({
        ...minimalConfigEnv(),
        MDK_CHECKOUT_ROUTE_KIND: 'self_hosted_mdkd_sidecar',
        MDK_CHECKOUT_ROUTE_SECRET: 'route-secret',
        MDK_CHECKOUT_ROUTE_URL: 'https://mdkd-sidecar.openagents.internal/api/mdk',
      }),
    )

    expect(fake.mdk.checkout.routeKind).toBe('fake_provider')
    expect(sidecar.mdk.checkout.routeKind).toBe('self_hosted_mdkd_sidecar')
    await expect(
      Effect.runPromise(decodeOpenAgentsWorkerConfig({
        ...minimalConfigEnv(),
        MDK_CHECKOUT_ROUTE_KIND: 'native_worker',
      })),
    ).rejects.toMatchObject({
      field: 'MDK_CHECKOUT_ROUTE_KIND',
      _tag: 'OpenAgentsWorkerConfigError',
    })
  })
})
