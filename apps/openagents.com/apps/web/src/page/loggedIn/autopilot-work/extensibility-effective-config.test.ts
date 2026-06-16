import { describe, expect, test } from 'vitest'

import {
  type ForgeExtensibilityEffectiveConfigInput,
  projectForgeExtensibilityEffectiveConfig,
} from './extensibility-effective-config'

const baseInput = (
  overrides: Partial<ForgeExtensibilityEffectiveConfigInput> = {},
): ForgeExtensibilityEffectiveConfigInput => ({
  configRef: 'extensibility-config.public.work_1',
  generatedAt: '2026-06-16T23:30:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge extensibility effective config projection', () => {
  test('projects mixed-domain effective states into counts and stable order', () => {
    const config = projectForgeExtensibilityEffectiveConfig(
      baseInput({
        entries: [
          {
            catalogRefs: ['skill-catalog.public.work_1'],
            configRefs: ['skill-config.public.default'],
            domain: 'skills',
            effectiveState: 'enabled',
            policyRefs: ['skill-policy.public.default'],
          },
          {
            catalogRefs: ['hook-catalog.public.work_1'],
            configRefs: ['hook-config.public.default'],
            domain: 'hooks',
            effectiveState: 'needs_trust',
            policyRefs: ['hook-policy.public.default'],
          },
          {
            catalogRefs: ['plugin-catalog.public.work_1'],
            configRefs: ['plugin-config.public.default'],
            domain: 'plugins',
            effectiveState: 'disabled',
          },
          {
            catalogRefs: ['mcp-catalog.public.work_1'],
            configRefs: ['mcp-config.public.default'],
            domain: 'mcp',
            effectiveState: 'needs_auth',
            policyRefs: ['mcp-policy.public.default'],
          },
          {
            blockerRefs: ['mcp-blocker.public.bad_config'],
            catalogRefs: ['mcp-catalog.public.broken'],
            configRefs: ['mcp-config.public.broken'],
            domain: 'mcp',
            effectiveState: 'blocked',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(config.domainCounts).toEqual({
      hooks: 1,
      mcp: 2,
      plugins: 1,
      skills: 1,
    })
    expect(config.stateCounts).toEqual({
      blocked: 1,
      disabled: 1,
      enabled: 1,
      needsAuth: 1,
      needsTrust: 1,
      pending: 0,
      total: 5,
    })
    expect(config.authority).toEqual({
      contextInjectionAuthority: false,
      hookExecutionAuthority: false,
      providerAccountAuthority: false,
      settlementAuthority: false,
      toolCallAuthority: false,
      workspaceWriteAuthority: false,
    })
    expect(config.entries.map(entry => `${entry.domain}:${entry.effectiveState}`)).toEqual(
      [
        'mcp:blocked',
        'mcp:needs_auth',
        'skills:enabled',
        'hooks:needs_trust',
        'plugins:disabled',
      ],
    )
    expect(config.status).toBe('blocked')
  })

  test('distinguishes empty, stale, and ready effective config states', () => {
    const empty = projectForgeExtensibilityEffectiveConfig(baseInput())
    const stale = projectForgeExtensibilityEffectiveConfig(
      baseInput({
        entries: [
          {
            configRefs: ['skill-config.public.default'],
            domain: 'skills',
            effectiveState: 'enabled',
          },
        ],
        freshness: 'stale',
      }),
    )
    const ready = projectForgeExtensibilityEffectiveConfig(
      baseInput({
        entries: [
          {
            configRefs: ['skill-config.public.default'],
            domain: 'skills',
            effectiveState: 'enabled',
          },
          {
            configRefs: ['plugin-config.public.default'],
            domain: 'plugins',
            effectiveState: 'disabled',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(empty.status).toBe('empty')
    expect(stale.status).toBe('stale')
    expect(ready.status).toBe('ready')
  })

  test('omits unsafe raw config and plugin material before projection', () => {
    const config = projectForgeExtensibilityEffectiveConfig(
      baseInput({
        entries: [
          {
            blockerRefs: [
              'config-blocker.public.safe',
              'diff --git a/private.json b/private.json',
            ],
            catalogRefs: [
              'plugin-catalog.public.safe',
              'raw plugin code /Users/christopher/private/plugin.ts',
            ],
            configRefs: [
              'plugin-config.public.safe',
              'raw config /Users/christopher/.openagents/private.json',
            ],
            domain: 'plugins',
            effectiveState: 'enabled',
            policyRefs: ['plugin-policy.public.safe', 'bearer token private'],
            sourceRefs: [
              'plugin-source.public.safe',
              'provider payload sk-private',
            ],
          },
          {
            catalogRefs: ['mcp-catalog.public.safe'],
            configRefs: ['/Users/christopher/private/mcp.json'],
            domain: 'mcp',
            effectiveState: 'blocked',
          },
        ],
      }),
    )
    const payload = JSON.stringify(config)

    expect(config.status).toBe('blocked')
    expect(config.omittedUnsafeRefCount).toBe(6)
    expect(config.entries).toEqual([
      {
        blockerRefs: [],
        catalogRefs: ['mcp-catalog.public.safe'],
        configRefs: [],
        domain: 'mcp',
        effectiveState: 'blocked',
        freshness: 'unknown',
        policyRefs: [],
        sourceRefs: [],
      },
      {
        blockerRefs: ['config-blocker.public.safe'],
        catalogRefs: ['plugin-catalog.public.safe'],
        configRefs: ['plugin-config.public.safe'],
        domain: 'plugins',
        effectiveState: 'enabled',
        freshness: 'unknown',
        policyRefs: ['plugin-policy.public.safe'],
        sourceRefs: ['plugin-source.public.safe'],
      },
    ])
    expect(config.blockerRefs).toContain(
      'forge-extensibility-config-blocker:extensibility-config.public.work_1:unsafe-extensibility-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw config')
    expect(payload).not.toContain('raw plugin code')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
    expect(payload).not.toContain('bearer token')
  })
})
