import { describe, expect, test } from 'vitest'

import { autopilotWebMcpContractImport } from './mcp-contract-import'

describe('Autopilot web MCP contract import', () => {
  test('imports the shared Phase 0 contract without exposing a runtime transport', () => {
    expect(autopilotWebMcpContractImport).toMatchObject({
      surface: 'autopilot_web',
      schemaVersion: 'openagents.mcp.phase0.v1',
      packageName: '@openagentsinc/mcp-contract',
      authority: 'public_read',
      outputSafety: 'public',
      reservedTransportKind: 'bridge_proxy',
      runtimeTransportExposed: false,
    })
  })
})
