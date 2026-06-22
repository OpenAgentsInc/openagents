import { describe, expect, test } from 'vitest'

import { apiWorkerMcpContractImport } from './mcp-contract-import'

describe('API worker MCP contract import', () => {
  test('imports the shared Phase 0 contract without exposing a runtime transport', () => {
    expect(apiWorkerMcpContractImport).toMatchObject({
      surface: 'api_worker',
      schemaVersion: 'openagents.mcp.phase0.v1',
      packageName: '@openagentsinc/mcp-contract',
      authority: 'operator_read',
      outputSafety: 'operator',
      reservedTransportKind: 'streamable_http',
      runtimeTransportExposed: false,
    })
  })
})
