import { describe, expect, test } from 'vitest'

import {
  type ForgeExtensibilityExecutionReceiptsInput,
  projectForgeExtensibilityExecutionReceipts,
} from './extensibility-execution-receipts'

const baseInput = (
  overrides: Partial<ForgeExtensibilityExecutionReceiptsInput> = {},
): ForgeExtensibilityExecutionReceiptsInput => ({
  config: {
    configRef: 'extensibility-config.public.work_1',
    entries: [
      {
        catalogRefs: ['mcp-catalog.public.work_1'],
        configRefs: ['mcp-config.public.filesystem'],
        domain: 'mcp',
        effectiveState: 'enabled',
        policyRefs: ['mcp-policy.public.workspace_read'],
        sourceRefs: ['mcp-source.public.filesystem'],
      },
      {
        catalogRefs: ['skill-catalog.public.work_1'],
        configRefs: ['skill-config.public.context_summary'],
        domain: 'skills',
        effectiveState: 'enabled',
        policyRefs: ['skill-policy.public.disclosure_required'],
        sourceRefs: ['skill-source.public.context_summary'],
      },
    ],
    freshness: 'fresh',
    generatedAt: '2026-06-17T00:00:00.000Z',
    workOrderRef: 'work_1',
  },
  generatedAt: '2026-06-17T00:05:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

const receiptByRef = (
  view: ReturnType<typeof projectForgeExtensibilityExecutionReceipts>,
  requestRef: string,
) => {
  const receipt = view.receipts.find(item => item.requestRef === requestRef)

  expect(receipt).toBeDefined()

  return receipt!
}

describe('Forge extensibility execution request receipts', () => {
  test('projects callable MCP and explicit skill-disclosure receipts without authority', () => {
    const view = projectForgeExtensibilityExecutionReceipts(
      baseInput({
        requests: [
          {
            configRefs: ['mcp-config.public.filesystem'],
            domain: 'mcp',
            policyRefs: ['mcp-policy.public.workspace_read'],
            providerAccountRefs: ['provider-account.public.local_mcp'],
            requestKind: 'mcp_tool_call',
            requestRef: 'extensibility-request.public.mcp_files',
            targetRef: 'mcp-source.public.filesystem',
          },
          {
            configRefs: ['skill-config.public.context_summary'],
            domain: 'skills',
            explicitDisclosure: true,
            policyRefs: ['skill-policy.public.disclosure_required'],
            requestKind: 'skill_body_disclosure',
            requestRef: 'extensibility-request.public.skill_context',
            targetRef: 'skill-source.public.context_summary',
          },
        ],
      }),
    )

    expect(view.status).toBe('ready')
    const mcp = receiptByRef(view, 'extensibility-request.public.mcp_files')
    const skill = receiptByRef(view, 'extensibility-request.public.skill_context')

    expect(mcp.outcome).toBe('callable')
    expect(mcp.authority).toEqual({
      contextInjectionAuthority: false,
      hookExecutionAuthority: false,
      mcpNetworkCallAuthority: false,
      pluginActivationAuthority: false,
      settingsWriteAuthority: false,
      skillBodyLoaded: false,
      workspaceWriteAuthority: false,
    })
    expect(mcp.providerAccountRefs).toEqual(['provider-account.public.local_mcp'])
    expect(skill.outcome).toBe('callable')
    expect(skill.authority.skillBodyLoaded).toBe(false)
    expect(skill.blockerRefs).toEqual([])
  })

  test('projects disabled, needs-auth, needs-trust, failed, and pending states', () => {
    const view = projectForgeExtensibilityExecutionReceipts(
      baseInput({
        config: {
          configRef: 'extensibility-config.public.work_1',
          entries: [
            {
              configRefs: ['plugin-config.public.disabled'],
              domain: 'plugins',
              effectiveState: 'disabled',
              policyRefs: ['plugin-policy.public.default'],
            },
            {
              configRefs: ['mcp-config.public.needs_auth'],
              domain: 'mcp',
              effectiveState: 'needs_auth',
              policyRefs: ['mcp-policy.public.auth_required'],
            },
            {
              configRefs: ['hook-config.public.needs_trust'],
              domain: 'hooks',
              effectiveState: 'needs_trust',
              policyRefs: ['hook-policy.public.workspace_trust_required'],
            },
            {
              configRefs: ['skill-config.public.pending'],
              domain: 'skills',
              effectiveState: 'pending',
            },
            {
              configRefs: ['mcp-config.public.configured_but_failed'],
              domain: 'mcp',
              effectiveState: 'enabled',
              policyRefs: ['mcp-policy.public.default'],
            },
          ],
          generatedAt: '2026-06-17T00:00:00.000Z',
          workOrderRef: 'work_1',
        },
        requests: [
          {
            configRefs: ['plugin-config.public.disabled'],
            domain: 'plugins',
            requestKind: 'plugin_activation',
            requestRef: 'extensibility-request.public.plugin_disabled',
            targetRef: 'plugin-config.public.disabled',
          },
          {
            configRefs: ['mcp-config.public.needs_auth'],
            domain: 'mcp',
            requestKind: 'mcp_resource_read',
            requestRef: 'extensibility-request.public.mcp_auth',
            targetRef: 'mcp-config.public.needs_auth',
          },
          {
            configRefs: ['hook-config.public.needs_trust'],
            domain: 'hooks',
            requestKind: 'hook_enablement',
            requestRef: 'extensibility-request.public.hook_trust',
            targetRef: 'hook-config.public.needs_trust',
          },
          {
            configRefs: ['skill-config.public.pending'],
            domain: 'skills',
            requestKind: 'skill_body_disclosure',
            requestRef: 'extensibility-request.public.skill_pending',
            targetRef: 'skill-config.public.pending',
          },
          {
            configRefs: ['mcp-config.public.configured_but_failed'],
            domain: 'mcp',
            failureRefs: ['mcp-failure.public.timeout'],
            observedState: 'failed',
            requestKind: 'mcp_tool_call',
            requestRef: 'extensibility-request.public.mcp_failed',
            targetRef: 'mcp-config.public.configured_but_failed',
          },
        ],
      }),
    )

    expect(receiptByRef(view, 'extensibility-request.public.plugin_disabled').outcome).toBe(
      'disabled',
    )
    expect(receiptByRef(view, 'extensibility-request.public.mcp_auth').outcome).toBe(
      'needs_auth',
    )
    expect(receiptByRef(view, 'extensibility-request.public.hook_trust').outcome).toBe(
      'needs_trust',
    )
    expect(receiptByRef(view, 'extensibility-request.public.skill_pending').outcome).toBe(
      'pending',
    )
    expect(receiptByRef(view, 'extensibility-request.public.mcp_failed')).toMatchObject({
      failureRefs: ['mcp-failure.public.timeout'],
      outcome: 'failed',
    })
  })

  test('blocks callable config when policy, workspace trust, provider, or explicit disclosure refs are missing', () => {
    const view = projectForgeExtensibilityExecutionReceipts(
      baseInput({
        config: {
          configRef: 'extensibility-config.public.work_1',
          entries: [
            {
              configRefs: ['mcp-config.public.filesystem'],
              domain: 'mcp',
              effectiveState: 'enabled',
              policyRefs: ['mcp-policy.public.workspace_read'],
            },
            {
              configRefs: ['hook-config.public.precommit'],
              domain: 'hooks',
              effectiveState: 'enabled',
              policyRefs: ['hook-policy.public.default'],
            },
            {
              configRefs: ['skill-config.public.context_summary'],
              domain: 'skills',
              effectiveState: 'enabled',
              policyRefs: ['skill-policy.public.disclosure_required'],
            },
          ],
          generatedAt: '2026-06-17T00:00:00.000Z',
          workOrderRef: 'work_1',
        },
        requests: [
          {
            configRefs: ['mcp-config.public.filesystem'],
            domain: 'mcp',
            requestKind: 'mcp_tool_call',
            requestRef: 'extensibility-request.public.mcp_missing_provider',
            targetRef: 'mcp-config.public.filesystem',
          },
          {
            configRefs: ['hook-config.public.precommit'],
            domain: 'hooks',
            requestKind: 'hook_enablement',
            requestRef: 'extensibility-request.public.hook_missing_trust',
            targetRef: 'hook-config.public.precommit',
          },
          {
            configRefs: ['skill-config.public.context_summary'],
            domain: 'skills',
            requestKind: 'skill_body_disclosure',
            requestRef: 'extensibility-request.public.skill_not_explicit',
            targetRef: 'skill-config.public.context_summary',
          },
        ],
      }),
    )

    const mcp = receiptByRef(
      view,
      'extensibility-request.public.mcp_missing_provider',
    )
    const hook = receiptByRef(
      view,
      'extensibility-request.public.hook_missing_trust',
    )
    const skill = receiptByRef(
      view,
      'extensibility-request.public.skill_not_explicit',
    )

    expect(mcp.outcome).toBe('blocked')
    expect(mcp.blockerRefs).toContain(
      'forge-extensibility-execution-blocker:extensibility-request.public.mcp_missing_provider:missing-provider-account-ref',
    )
    expect(hook.outcome).toBe('blocked')
    expect(hook.blockerRefs).toContain(
      'forge-extensibility-execution-blocker:extensibility-request.public.hook_missing_trust:missing-workspace-trust-ref',
    )
    expect(skill.outcome).toBe('blocked')
    expect(skill.blockerRefs).toContain(
      'forge-extensibility-execution-blocker:extensibility-request.public.skill_not_explicit:skill-body-disclosure-not-explicit',
    )
  })

  test('omits unsafe private execution request material before projection', () => {
    const view = projectForgeExtensibilityExecutionReceipts(
      baseInput({
        requests: [
          {
            actorRef: 'actor.public.safe',
            blockerRefs: ['raw shell command $(cat ~/.ssh/id_rsa)'],
            configRefs: [
              'skill-config.public.context_summary',
              'raw config /Users/christopher/private.json',
            ],
            domain: 'skills',
            explicitDisclosure: true,
            policyRefs: [
              'skill-policy.public.disclosure_required',
              'bearer token private',
            ],
            requestKind: 'skill_body_disclosure',
            requestRef: 'extensibility-request.public.skill_context',
            sourceRefs: ['skill-source.public.safe', 'skill body raw text'],
            targetRef: '/Users/christopher/private/skill.md',
          },
        ],
      }),
    )
    const payload = JSON.stringify(view)
    const receipt = receiptByRef(view, 'extensibility-request.public.skill_context')

    expect(view.status).toBe('blocked')
    expect(receipt.targetRef).toBe('unsafe-target-ref-omitted')
    expect(receipt.omittedUnsafeRefCount).toBe(5)
    expect(receipt.sourceRefs).toEqual([
      'skill-source.public.safe',
      'skill-source.public.context_summary',
    ])
    expect(receipt.blockerRefs).toContain(
      'forge-extensibility-execution-blocker:extensibility-request.public.skill_context:unsafe-extensibility-request-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('raw config')
    expect(payload).not.toContain('skill body raw text')
    expect(payload).not.toContain('bearer token')
  })
})
