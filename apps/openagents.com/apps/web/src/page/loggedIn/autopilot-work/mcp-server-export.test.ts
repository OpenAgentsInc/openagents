import { describe, expect, test } from 'vitest'

import {
  buildForgeMcpServerExportInput,
  projectForgeMcpServerExport,
} from './mcp-server-export'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T23:10:00.000Z',
  snapshotRef: 'mcp-server-export-snapshot.public.work_1',
  versionRef: 'mcp-server-export-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge MCP server export projection', () => {
  test('projects public MCP server export evidence as refs-only non-authoritative state', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      entries: [
        {
          audienceRefs: ['audience.public.operator_agents'],
          authPolicyRefs: ['auth-policy.public.mcp_server.operator_only'],
          capabilityRefs: ['capability.public.mcp_server.read_status'],
          exportedPromptRefs: ['mcp-prompt.public.status_summary'],
          exportedResourceRefs: ['mcp-resource.public.run_status'],
          exportedToolRefs: ['mcp-tool.public.get_run_status'],
          freshness: 'fresh',
          invocationReceiptRefs: ['invocation-receipt.public.status_probe'],
          policyRefs: ['policy.public.mcp_server.read_only'],
          schemaRefs: ['schema.public.mcp_server.status.v1'],
          serverRef: 'mcp-server.public.operator',
          sourceRefs: ['source.public.extensibility_config'],
          state: 'exposed',
          transportRefs: ['transport.public.stdio_descriptor'],
          trustTierRefs: ['trust-tier.public.operator'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      disabled: 0,
      exposed: 1,
      internalOnly: 0,
      planned: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      credentialAuthority: false,
      deploymentAuthority: false,
      effectiveConfigMutationAuthority: false,
      fileReadAuthority: false,
      publicClaimAuthority: false,
      remoteInvocationAuthority: false,
      serverHostingAuthority: false,
      settingsWriteAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolExecutionAuthority: false,
      toolGrantAuthority: false,
      toolRoutingAuthority: false,
      transportExposureAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing MCP server export state as empty', () => {
    const view = projectForgeMcpServerExport({
      generatedAt: '2026-06-17T23:10:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale MCP server export evidence', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      entries: [
        {
          freshness: 'stale',
          policyRefs: ['policy.public.mcp_server.read_only'],
          schemaRefs: ['schema.public.mcp_server.status.v1'],
          serverRef: 'mcp-server.public.stale',
          state: 'internal_only',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-mcp-server-export-blocker:work.public.work_1:stale-mcp-server-evidence:mcp-server.public.stale',
    )
  })

  test('blocks exported surfaces without schema refs', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      entries: [
        {
          exportedToolRefs: ['mcp-tool.public.get_status'],
          freshness: 'fresh',
          policyRefs: ['policy.public.mcp_server.read_only'],
          serverRef: 'mcp-server.public.no_schema',
          state: 'internal_only',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-mcp-server-export-blocker:work.public.work_1:export-schema-ref-missing:mcp-server.public.no_schema',
    )
  })

  test('blocks exported surfaces without policy refs', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      entries: [
        {
          exportedToolRefs: ['mcp-tool.public.get_status'],
          freshness: 'fresh',
          schemaRefs: ['schema.public.mcp_server.status.v1'],
          serverRef: 'mcp-server.public.no_policy',
          state: 'internal_only',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-mcp-server-export-blocker:work.public.work_1:export-policy-ref-missing:mcp-server.public.no_policy',
    )
  })

  test('blocks remote exposure without auth audience and trust refs', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      entries: [
        {
          exportedToolRefs: ['mcp-tool.public.get_status'],
          freshness: 'fresh',
          policyRefs: ['policy.public.mcp_server.read_only'],
          schemaRefs: ['schema.public.mcp_server.status.v1'],
          serverRef: 'mcp-server.public.remote_no_auth',
          state: 'exposed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-mcp-server-export-blocker:work.public.work_1:remote-auth-ref-missing:mcp-server.public.remote_no_auth',
    )
  })

  test('blocks invocation receipts without exported capability refs', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          invocationReceiptRefs: ['invocation-receipt.public.status_probe'],
          policyRefs: ['policy.public.mcp_server.read_only'],
          schemaRefs: ['schema.public.mcp_server.status.v1'],
          serverRef: 'mcp-server.public.invocation_no_capability',
          state: 'internal_only',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-mcp-server-export-blocker:work.public.work_1:invocation-capability-ref-missing:mcp-server.public.invocation_no_capability',
    )
  })

  test('omits unsafe private MCP server material before projection', () => {
    const view = projectForgeMcpServerExport({
      ...baseInput,
      blockerRefs: ['mcp-server-blocker.public.safe', 'raw mcp /Users/christopher/mcp.log'],
      entries: [
        {
          audienceRefs: ['audience.public.safe'],
          authPolicyRefs: ['auth-policy.public.safe', 'bearer token private'],
          capabilityRefs: ['capability.public.safe'],
          exportedPromptRefs: ['mcp-prompt.public.safe', 'raw prompt sk-private'],
          exportedResourceRefs: ['mcp-resource.public.safe'],
          exportedToolRefs: ['mcp-tool.public.safe', 'raw tool /Users/christopher/tool.ts'],
          freshness: 'fresh',
          invocationReceiptRefs: ['invocation-receipt.public.safe'],
          policyRefs: ['policy.public.safe'],
          schemaRefs: ['schema.public.safe', 'raw schema /Users/christopher/schema.json'],
          serverRef: 'mcp-server.public.safe',
          sourceRefs: ['source.public.safe', 'private server content'],
          state: 'exposed',
          transportRefs: ['transport.public.safe', 'raw socket /tmp/mcp.sock'],
          trustTierRefs: ['trust-tier.public.safe'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.exportedToolRefs).toEqual(['mcp-tool.public.safe'])
    expect(view.entries[0]?.schemaRefs).toEqual(['schema.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-mcp-server-export-blocker:work.public.work_1:unsafe-mcp-server-export-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw mcp')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw tool')
    expect(payload).not.toContain('raw schema')
    expect(payload).not.toContain('raw socket')
    expect(payload).not.toContain('private server')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T23:11:00.000Z',
      mcpServerExport: {
        entries: [
          {
            capabilityRefs: ['capability.public.work_2'],
            freshness: 'fresh',
            policyRefs: ['policy.public.work_2'],
            schemaRefs: ['schema.public.work_2'],
            serverRef: 'mcp-server.public.work_2',
            state: 'internal_only',
          },
        ],
        snapshotRef: 'mcp-server-export-snapshot.public.work_2',
        versionRef: 'mcp-server-export-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeMcpServerExportInput(work)).toEqual({
      entries: [
        {
          capabilityRefs: ['capability.public.work_2'],
          freshness: 'fresh',
          policyRefs: ['policy.public.work_2'],
          schemaRefs: ['schema.public.work_2'],
          serverRef: 'mcp-server.public.work_2',
          state: 'internal_only',
        },
      ],
      generatedAt: '2026-06-17T23:11:00.000Z',
      snapshotRef: 'mcp-server-export-snapshot.public.work_2',
      versionRef: 'mcp-server-export-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
