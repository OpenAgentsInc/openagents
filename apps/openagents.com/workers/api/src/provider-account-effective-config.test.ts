import { describe, expect, test } from 'vitest'

import {
  PROVIDER_ACCOUNT_BUDGET_VERSION,
  PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_VERSION,
  evaluateProviderAccountBudgetEvents,
  resolveProviderAccountEffectiveConfig,
} from './provider-account-effective-config'

describe('provider account effective config', () => {
  const generatedAt = '2026-06-11T14:00:00.000Z'

  test('resolves explicit precedence into safe config refs and value tags', () => {
    const projection = resolveProviderAccountEffectiveConfig({
      decisionKind: 'provider',
      decisionRef: 'provider-decision.non-codex.live-readiness',
      generatedAt,
      layers: [
        {
          layer: 'environment',
          settings: [
            {
              configRef: 'config.env.provider.allowlist.codex',
              key: 'provider.allowlist',
              value: ['chatgpt_codex'],
            },
            {
              configRef: 'config.env.routing.codex-only',
              key: 'routing.mode',
              value: 'codex_only',
            },
          ],
        },
        {
          layer: 'team',
          settings: [
            {
              caveatRefs: ['caveat.provider_peers.tos_review_required'],
              configRef: 'config.team.provider.allowlist.pack-b',
              key: 'provider.allowlist',
              value: ['chatgpt_codex', 'anthropic_claude'],
            },
          ],
        },
        {
          layer: 'runtime',
          settings: [
            {
              configRef: 'config.runtime.routing.provider-peers',
              key: 'routing.mode',
              value: 'provider_peers',
            },
          ],
        },
      ],
      requiredKeys: ['provider.allowlist', 'routing.mode'],
      snapshotRef: 'provider-account-effective-config.pack-b.provider',
    })

    expect(projection).toEqual({
      generatedAt,
      configVersion: PROVIDER_ACCOUNT_EFFECTIVE_CONFIG_VERSION,
      decisionKind: 'provider',
      decisionRef: 'provider-decision.non-codex.live-readiness',
      effectiveConfigRef: 'provider-account-effective-config.pack-b.provider',
      status: 'resolved',
      settings: [
        {
          caveatRefs: ['caveat.provider_peers.tos_review_required'],
          configRef: 'config.team.provider.allowlist.pack-b',
          key: 'provider.allowlist',
          sourceLayer: 'team',
          valueTag: 'provider.allowlist:2',
        },
        {
          caveatRefs: [],
          configRef: 'config.runtime.routing.provider-peers',
          key: 'routing.mode',
          sourceLayer: 'runtime',
          valueTag: 'routing.mode:provider_peers',
        },
      ],
      blockerRefs: [],
      denialReasonRef: null,
    })
    expect(JSON.stringify(projection)).not.toContain('OPENAI_API_KEY')
  })

  test('blocks missing required settings instead of falling back silently', () => {
    const projection = resolveProviderAccountEffectiveConfig({
      decisionKind: 'telemetry',
      decisionRef: 'provider-decision.telemetry.pack-b',
      generatedAt,
      layers: [
        {
          layer: 'default',
          settings: [
            {
              configRef: 'config.default.telemetry.aggregate',
              key: 'telemetry.mode',
              value: 'aggregate',
            },
          ],
        },
      ],
      requiredKeys: ['telemetry.mode', 'retention.class'],
    })

    expect(projection).toMatchObject({
      status: 'blocked',
      blockerRefs: [
        'provider-account-config-blocker:provider-decision.telemetry.pack-b:missing:retention.class',
      ],
      denialReasonRef:
        'provider-account-config-denial:provider-decision.telemetry.pack-b',
    })
  })

  test('blocks invalid setting values with typed blocker refs', () => {
    const projection = resolveProviderAccountEffectiveConfig({
      decisionKind: 'approval',
      decisionRef: 'provider-decision.approval.pack-b',
      generatedAt,
      layers: [
        {
          layer: 'user',
          settings: [
            {
              configRef: 'config.user.approval.invalid',
              key: 'approval.mode',
              value: 'prompt_maybe',
            },
          ],
        },
      ],
      requiredKeys: ['approval.mode'],
    })

    expect(projection.status).toBe('blocked')
    expect(projection.settings[0]).toMatchObject({
      sourceLayer: 'user',
      valueTag: 'invalid',
    })
    expect(projection.blockerRefs).toEqual([
      'provider-account-config-blocker:provider-decision.approval.pack-b:invalid:approval.mode',
    ])
  })

  test('rejects raw secret material in refs, caveats, and values', () => {
    expect(() =>
      resolveProviderAccountEffectiveConfig({
        decisionKind: 'routing',
        decisionRef: 'provider-decision.routing.pack-b',
        generatedAt,
        layers: [
          {
            layer: 'environment',
            settings: [
              {
                configRef: 'config.env.routing',
                key: 'routing.mode',
                value: 'ANTHROPIC_API_KEY=secret',
              },
            ],
          },
        ],
        requiredKeys: ['routing.mode'],
      }),
    ).toThrow(/provider credential material/)

    expect(() =>
      resolveProviderAccountEffectiveConfig({
        decisionKind: 'budget',
        decisionRef: 'provider-decision.budget.pack-b',
        generatedAt,
        layers: [
          {
            layer: 'team',
            settings: [
              {
                caveatRefs: ['caveat.raw.OPENAI_API_KEY=secret'],
                configRef: 'config.team.budget.pack-b',
                key: 'budget.maxCents',
                value: 2500,
              },
            ],
          },
        ],
        requiredKeys: ['budget.maxCents'],
      }),
    ).toThrow(/provider credential material/)
  })
})

describe('evaluateProviderAccountBudgetEvents', () => {
  const ACCOUNT_REF = 'provider-account_chatgpt_codex_alpha'

  test('emits no events when no budget is configured', () => {
    expect(
      evaluateProviderAccountBudgetEvents({
        budgets: [],
        usageByAccountRef: [
          {
            providerAccountRef: ACCOUNT_REF,
            totalTokens: 9_999_999,
            windowTokens: 9_999_999,
          },
        ],
      }),
    ).toEqual([])
  })

  test('emits no events when usage is within budget', () => {
    expect(
      evaluateProviderAccountBudgetEvents({
        budgets: [
          {
            providerAccountRef: ACCOUNT_REF,
            maxTotalTokens: 1000,
            maxWindowTokens: 500,
          },
        ],
        usageByAccountRef: [
          {
            providerAccountRef: ACCOUNT_REF,
            totalTokens: 1000,
            windowTokens: 500,
          },
        ],
      }),
    ).toEqual([])
  })

  test('emits advisory-only over-budget events for each exceeded field', () => {
    const events = evaluateProviderAccountBudgetEvents({
      budgets: [
        {
          providerAccountRef: ACCOUNT_REF,
          maxTotalTokens: 1000,
          maxWindowTokens: 500,
          windowLabel: '7d',
        },
      ],
      usageByAccountRef: [
        {
          providerAccountRef: ACCOUNT_REF,
          totalTokens: 1500,
          windowTokens: 800,
        },
      ],
    })

    expect(events).toHaveLength(2)
    expect(events.map(event => event.field)).toEqual([
      'budget.totalTokens',
      'budget.windowTokens',
    ])
    expect(events.every(event => event.authority === 'advisory_event_only')).toBe(
      true,
    )
    expect(events.every(event => event.budgetVersion === PROVIDER_ACCOUNT_BUDGET_VERSION)).toBe(
      true,
    )
    expect(events[0]?.overBy).toBe(500)
    expect(events[1]?.windowLabel).toBe('7d')
  })

  test('ignores budgets with no matching observed usage', () => {
    expect(
      evaluateProviderAccountBudgetEvents({
        budgets: [
          { providerAccountRef: ACCOUNT_REF, maxTotalTokens: 1 },
        ],
        usageByAccountRef: [],
      }),
    ).toEqual([])
  })
})
