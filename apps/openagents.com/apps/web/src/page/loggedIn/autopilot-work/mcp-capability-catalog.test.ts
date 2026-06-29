import { describe, expect, test } from 'vitest'

import {
  type ForgeMcpCapabilityCatalogInput,
  projectForgeMcpCapabilityCatalog,
} from './mcp-capability-catalog'

const baseInput = (
  overrides: Partial<ForgeMcpCapabilityCatalogInput> = {},
): ForgeMcpCapabilityCatalogInput => ({
  catalogRef: 'mcp-catalog.public.work_1',
  generatedAt: '2026-06-16T22:00:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge MCP capability catalog projection', () => {
  test('projects mixed MCP states into stable counts and sorted entries', () => {
    const catalog = projectForgeMcpCapabilityCatalog(
      baseInput({
        entries: [
          {
            capabilityRefs: ['mcp-capability.public.docs.search'],
            policyRefs: ['mcp-policy.public.readonly'],
            serverRef: 'mcp-server.public.docs',
            state: 'configured',
          },
          {
            capabilityRefs: ['mcp-capability.public.repo.index'],
            serverRef: 'mcp-server.public.repo',
            state: 'pending',
          },
          {
            authRefs: ['mcp-auth.public.github.required'],
            capabilityRefs: ['mcp-capability.public.github.issue_write'],
            serverRef: 'mcp-server.public.github',
            state: 'needs_auth',
          },
          {
            blockerRefs: ['mcp-blocker.public.filesystem.not_trusted'],
            capabilityRefs: ['mcp-capability.public.fs.read'],
            serverRef: 'mcp-server.public.filesystem',
            state: 'failed',
          },
          {
            capabilityRefs: ['mcp-capability.public.browser.capture'],
            serverRef: 'mcp-server.public.browser',
            state: 'disabled',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(catalog).toMatchObject({
      authority: {
        approvalBypassAuthority: false,
        providerAccountAuthority: false,
        settlementAuthority: false,
        toolCallAuthority: false,
        workspaceWriteAuthority: false,
      },
      counts: {
        configured: 1,
        disabled: 1,
        failed: 1,
        needsAuth: 1,
        pending: 1,
        total: 5,
      },
      freshness: 'fresh',
      omittedUnsafeRefCount: 0,
      publicSafe: true,
      status: 'blocked',
    })
    expect(catalog.entries.map(entry => entry.serverRef)).toEqual([
      'mcp-server.public.github',
      'mcp-server.public.filesystem',
      'mcp-server.public.repo',
      'mcp-server.public.docs',
      'mcp-server.public.browser',
    ])
    expect(catalog.blockerRefs).toEqual([
      'mcp-blocker.public.filesystem.not_trusted',
    ])
  })

  test('distinguishes empty, stale, and ready catalogs', () => {
    const empty = projectForgeMcpCapabilityCatalog(baseInput())
    const stale = projectForgeMcpCapabilityCatalog(
      baseInput({
        entries: [
          {
            capabilityRefs: ['mcp-capability.public.docs.search'],
            serverRef: 'mcp-server.public.docs',
            state: 'configured',
          },
        ],
        freshness: 'stale',
      }),
    )
    const ready = projectForgeMcpCapabilityCatalog(
      baseInput({
        entries: [
          {
            capabilityRefs: ['mcp-capability.public.docs.search'],
            serverRef: 'mcp-server.public.docs',
            state: 'configured',
          },
        ],
        freshness: 'fresh',
      }),
    )

    expect(empty.status).toBe('empty')
    expect(stale.status).toBe('stale')
    expect(ready.status).toBe('ready')
  })

  test('omits unsafe MCP refs and private material before projection', () => {
    const catalog = projectForgeMcpCapabilityCatalog(
      baseInput({
        entries: [
          {
            authRefs: ['mcp-auth.public.safe', 'bearer token private'],
            blockerRefs: [
              'mcp-blocker.public.safe',
              'diff --git a/private.json b/private.json',
            ],
            capabilityRefs: [
              'mcp-capability.public.safe',
              'raw tool schema /Users/christopher/private.json',
            ],
            policyRefs: [
              'mcp-policy.public.safe',
              'private server config /Users/christopher/.mcp.json',
            ],
            serverRef: 'mcp-server.public.safe',
            state: 'configured',
          },
          {
            capabilityRefs: ['mcp-capability.public.private_server'],
            serverRef: '/Users/christopher/.mcp/private-server.json',
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
        authRefs: ['mcp-auth.public.safe'],
        blockerRefs: ['mcp-blocker.public.safe'],
        capabilityRefs: ['mcp-capability.public.safe'],
        freshness: 'unknown',
        policyRefs: ['mcp-policy.public.safe'],
        serverRef: 'mcp-server.public.safe',
        state: 'configured',
      },
    ])
    expect(catalog.blockerRefs).toContain(
      'forge-mcp-capability-catalog-blocker:mcp-catalog.public.work_1:unsafe-mcp-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw tool schema')
    expect(payload).not.toContain('private server config')
    expect(payload).not.toContain('bearer token')
  })
})
