import { describe, expect, test } from 'vitest'

import {
  PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_VERSION,
  projectProviderAccountTelemetryPrivacy,
} from './provider-account-telemetry-privacy'

describe('provider account telemetry privacy projection', () => {
  const base = {
    generatedAt: '2026-06-11T16:05:00.000Z',
    observedAt: '2026-06-11T16:04:30.000Z',
    projectionRef: 'provider-account-telemetry.pack-b.aggregate',
    redactionFixtureRefs: ['fixture.provider-account.telemetry-redaction.pack-b'],
    sharingPolicy: 'approved_users_only' as const,
    sourceRefs: ['provider-account-source:account-health'],
    staleAfterMs: 60_000,
    supportBundleRefs: ['support-bundle:provider-account:redacted'],
    telemetryMode: 'aggregate' as const,
  }

  test('projects aggregate provider account telemetry with freshness metadata', () => {
    const projection = projectProviderAccountTelemetryPrivacy({
      ...base,
      caveatRefs: ['telemetry-caveat:aggregate-only'],
      debugBundleRefs: ['debug-bundle:provider-account:redacted'],
      metrics: [
        {
          counter: 4,
          kind: 'account_health',
          metricRef: 'metric:provider-account-health:healthy',
          provider: 'anthropic_claude',
          providerAccountClass: 'connected',
          status: 'healthy',
        },
        {
          durationMs: 1_200,
          kind: 'rate_limit',
          metricRef: 'metric:provider-account-rate-limit:reset-pending',
          provider: 'anthropic_claude',
          providerAccountClass: 'leased',
          status: 'reset_pending',
        },
        {
          counter: 1,
          kind: 'reconnect',
          metricRef: 'metric:provider-account-reconnect:needed',
          provider: 'anthropic_claude',
          providerAccountClass: 'blocked',
          status: 'requires_reauth',
        },
      ],
    })

    expect(projection).toEqual({
      generatedAt: '2026-06-11T16:05:00.000Z',
      telemetryVersion: PROVIDER_ACCOUNT_TELEMETRY_PRIVACY_VERSION,
      projectionRef: 'provider-account-telemetry.pack-b.aggregate',
      telemetryMode: 'aggregate',
      sharingPolicy: 'approved_users_only',
      status: 'ready',
      freshness: 'fresh',
      observedAt: '2026-06-11T16:04:30.000Z',
      staleAt: '2026-06-11T16:05:30.000Z',
      ageMs: 30_000,
      metricRefs: [
        'metric:provider-account-health:healthy',
        'metric:provider-account-rate-limit:reset-pending',
        'metric:provider-account-reconnect:needed',
      ],
      metrics: [
        {
          caveatRefs: [],
          counter: 4,
          durationMs: undefined,
          kind: 'account_health',
          metricRef: 'metric:provider-account-health:healthy',
          provider: 'anthropic_claude',
          providerAccountClass: 'connected',
          status: 'healthy',
          valueKind: 'counter',
        },
        {
          caveatRefs: [],
          counter: undefined,
          durationMs: 1_200,
          kind: 'rate_limit',
          metricRef: 'metric:provider-account-rate-limit:reset-pending',
          provider: 'anthropic_claude',
          providerAccountClass: 'leased',
          status: 'reset_pending',
          valueKind: 'duration',
        },
        {
          caveatRefs: [],
          counter: 1,
          durationMs: undefined,
          kind: 'reconnect',
          metricRef: 'metric:provider-account-reconnect:needed',
          provider: 'anthropic_claude',
          providerAccountClass: 'blocked',
          status: 'requires_reauth',
          valueKind: 'counter',
        },
      ],
      redactionFixtureRefs: ['fixture.provider-account.telemetry-redaction.pack-b'],
      caveatRefs: ['telemetry-caveat:aggregate-only'],
      sourceRefs: ['provider-account-source:account-health'],
      debugBundleRefs: ['debug-bundle:provider-account:redacted'],
      supportBundleRefs: ['support-bundle:provider-account:redacted'],
      blockerRefs: [],
    })
  })

  test('keeps local-only telemetry ref-only and disabled telemetry empty', () => {
    const metrics = [
      {
        counter: 2,
        kind: 'provider_routing' as const,
        metricRef: 'metric:provider-routing:candidate-count',
        provider: 'mixed' as const,
        providerAccountClass: 'aggregate' as const,
        status: 'route_candidate' as const,
      },
    ]

    const localOnly = projectProviderAccountTelemetryPrivacy({
      ...base,
      metrics,
      projectionRef: 'provider-account-telemetry.pack-b.local-only',
      sharingPolicy: 'local_only',
      telemetryMode: 'local_only',
    })
    const disabled = projectProviderAccountTelemetryPrivacy({
      ...base,
      metrics,
      projectionRef: 'provider-account-telemetry.pack-b.opt-out',
      sharingPolicy: 'opt_out',
      telemetryMode: 'off',
    })

    expect(localOnly).toMatchObject({
      status: 'local_only',
      metricRefs: ['metric:provider-routing:candidate-count'],
      metrics: [],
    })
    expect(disabled).toMatchObject({
      status: 'disabled',
      metricRefs: ['metric:provider-routing:candidate-count'],
      metrics: [],
    })
  })

  test('marks stale account telemetry without dropping safe caveats', () => {
    const projection = projectProviderAccountTelemetryPrivacy({
      ...base,
      generatedAt: '2026-06-11T16:07:00.000Z',
      metrics: [
        {
          caveatRefs: ['metric-caveat:rate-limit-window'],
          kind: 'reset_hint',
          metricRef: 'metric:provider-account-reset-hint:window',
          provider: 'google_gemini',
          providerAccountClass: 'candidate',
          status: 'reset_pending',
        },
      ],
    })

    expect(projection).toMatchObject({
      freshness: 'stale',
      ageMs: 150_000,
      status: 'ready',
    })
    expect(projection.metrics[0]?.caveatRefs).toEqual([
      'metric-caveat:rate-limit-window',
    ])
  })

  test('blocks health and rate-limit telemetry when redaction fixtures are missing', () => {
    const projection = projectProviderAccountTelemetryPrivacy({
      ...base,
      metrics: [
        {
          counter: 1,
          kind: 'low_credit',
          metricRef: 'metric:provider-account-low-credit',
          provider: 'chatgpt_codex',
          providerAccountClass: 'leased',
          status: 'low_credit',
        },
        {
          kind: 'cooldown',
          metricRef: 'metric:provider-account-cooldown',
          provider: 'chatgpt_codex',
          providerAccountClass: 'blocked',
          status: 'cooling_down',
        },
      ],
      redactionFixtureRefs: [],
    })

    expect(projection).toMatchObject({
      status: 'blocked',
      blockerRefs: [
        'provider-account-telemetry-blocker:provider-account-telemetry.pack-b.aggregate:missing-redaction-fixture:low_credit',
        'provider-account-telemetry-blocker:provider-account-telemetry.pack-b.aggregate:missing-redaction-fixture:cooldown',
      ],
    })
  })

  test('rejects credentials, raw prompts, private repo data, and raw provider responses', () => {
    expect(() =>
      projectProviderAccountTelemetryPrivacy({
        ...base,
        metrics: [
          {
            kind: 'account_health',
            metricRef: 'metric:ANTHROPIC_API_KEY=secret',
            provider: 'anthropic_claude',
            providerAccountClass: 'connected',
            status: 'healthy',
          },
        ],
      }),
    ).toThrow(/provider credential material/)

    expect(() =>
      projectProviderAccountTelemetryPrivacy({
        ...base,
        debugBundleRefs: ['debug-bundle:raw prompt: summarize private code'],
        metrics: [],
      }),
    ).toThrow(/private telemetry material/)

    expect(() =>
      projectProviderAccountTelemetryPrivacy({
        ...base,
        metrics: [],
        sourceRefs: ['git@github.com:OpenAgentsInc/private-repo.git'],
      }),
    ).toThrow(/private telemetry material/)

    expect(() =>
      projectProviderAccountTelemetryPrivacy({
        ...base,
        metrics: [
          {
            kind: 'provider_routing',
            metricRef: 'metric:raw provider response: choices',
            provider: 'google_gemini',
            providerAccountClass: 'candidate',
            status: 'route_candidate',
          },
        ],
      }),
    ).toThrow(/private telemetry material/)
  })
})
