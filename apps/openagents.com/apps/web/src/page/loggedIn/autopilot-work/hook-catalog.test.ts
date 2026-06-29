import { describe, expect, test } from 'vitest'

import {
  type ForgeHookCatalogInput,
  projectForgeHookCatalog,
} from './hook-catalog'

const baseInput = (
  overrides: Partial<ForgeHookCatalogInput> = {},
): ForgeHookCatalogInput => ({
  catalogRef: 'hook-catalog.public.work_1',
  generatedAt: '2026-06-16T23:00:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge hook catalog projection', () => {
  test('projects mixed hook states into stable counts and sorted entries', () => {
    const catalog = projectForgeHookCatalog(
      baseInput({
        entries: [
          {
            descriptorRef: 'hook-descriptor.public.format',
            doctorRefs: ['hook-doctor.public.format.ok'],
            eventRefs: ['hook-event.public.before_commit'],
            hookRef: 'hook.format',
            policyRefs: ['hook-policy.public.trusted_format'],
            state: 'configured',
            workspaceTrustRefs: ['workspace-trust.public.openagents'],
          },
          {
            descriptorRef: 'hook-descriptor.public.lint',
            hookRef: 'hook.lint',
            state: 'configured',
          },
          {
            descriptorRef: 'hook-descriptor.public.release',
            hookRef: 'hook.release',
            state: 'pending',
          },
          {
            blockerRefs: ['hook-blocker.public.script_missing'],
            descriptorRef: 'hook-descriptor.public.test',
            hookRef: 'hook.test',
            state: 'failed',
          },
          {
            descriptorRef: 'hook-descriptor.public.legacy',
            hookRef: 'hook.legacy',
            state: 'disabled',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(catalog.counts).toEqual({
      configured: 1,
      disabled: 1,
      failed: 1,
      needsTrust: 1,
      pending: 1,
      total: 5,
    })
    expect(catalog.authority).toEqual({
      hookExecutionAuthority: false,
      providerAccountAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      workspaceWriteAuthority: false,
    })
    expect(catalog.executionGate).toEqual({
      disabledByDefault: true,
      hookExecutionAuthority: false,
      policyRefsRequired: true,
      shellExecutionAuthority: false,
      workspaceTrustRequired: true,
    })
    expect(catalog.entries.map(entry => entry.hookRef)).toEqual([
      'hook.lint',
      'hook.test',
      'hook.release',
      'hook.format',
      'hook.legacy',
    ])
    expect(catalog.entries[0]?.state).toBe('needs_trust')
    expect(catalog.entries[0]?.execution).toEqual({
      executionAllowed: false,
      policySatisfied: false,
      workspaceTrustSatisfied: false,
    })
    expect(catalog.entries[3]?.execution).toEqual({
      executionAllowed: false,
      policySatisfied: true,
      workspaceTrustSatisfied: true,
    })
    expect(catalog.status).toBe('blocked')
  })

  test('distinguishes empty, stale, and ready hook catalogs', () => {
    const empty = projectForgeHookCatalog(baseInput())
    const stale = projectForgeHookCatalog(
      baseInput({
        entries: [
          {
            descriptorRef: 'hook-descriptor.public.format',
            hookRef: 'hook.format',
            policyRefs: ['hook-policy.public.format'],
            state: 'configured',
            workspaceTrustRefs: ['workspace-trust.public.openagents'],
          },
        ],
        freshness: 'stale',
      }),
    )
    const ready = projectForgeHookCatalog(
      baseInput({
        entries: [
          {
            descriptorRef: 'hook-descriptor.public.format',
            hookRef: 'hook.format',
            policyRefs: ['hook-policy.public.format'],
            state: 'configured',
            workspaceTrustRefs: ['workspace-trust.public.openagents'],
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(empty.status).toBe('empty')
    expect(stale.status).toBe('stale')
    expect(ready.status).toBe('ready')
  })

  test('omits unsafe hook scripts and private material before projection', () => {
    const catalog = projectForgeHookCatalog(
      baseInput({
        entries: [
          {
            blockerRefs: [
              'hook-blocker.public.safe',
              'diff --git a/private.sh b/private.sh',
            ],
            descriptorRef: 'hook-descriptor.public.safe',
            doctorRefs: [
              'hook-doctor.public.safe',
              'raw hook script /Users/christopher/private.sh',
            ],
            eventRefs: ['hook-event.public.safe', 'shell command $(rm -rf /)'],
            hookRef: 'hook.safe',
            policyRefs: [
              'hook-policy.public.safe',
              'provider payload sk-private',
            ],
            state: 'configured',
            workspaceTrustRefs: [
              'workspace-trust.public.safe',
              '/Users/christopher/private/workspace',
            ],
          },
          {
            descriptorRef: '/Users/christopher/private/hook.json',
            hookRef: 'hook.private',
            state: 'failed',
          },
        ],
      }),
    )
    const payload = JSON.stringify(catalog)

    expect(catalog.status).toBe('blocked')
    expect(catalog.omittedUnsafeRefCount).toBe(6)
    expect(catalog.entries).toEqual([
      {
        blockerRefs: ['hook-blocker.public.safe'],
        descriptorRef: 'hook-descriptor.public.safe',
        doctorRefs: ['hook-doctor.public.safe'],
        eventRefs: ['hook-event.public.safe'],
        execution: {
          executionAllowed: false,
          policySatisfied: true,
          workspaceTrustSatisfied: true,
        },
        freshness: 'unknown',
        hookRef: 'hook.safe',
        policyRefs: ['hook-policy.public.safe'],
        state: 'configured',
        workspaceTrustRefs: ['workspace-trust.public.safe'],
      },
    ])
    expect(catalog.blockerRefs).toContain(
      'forge-hook-catalog-blocker:hook-catalog.public.work_1:unsafe-hook-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw hook script')
    expect(payload).not.toContain('shell command')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
  })
})
