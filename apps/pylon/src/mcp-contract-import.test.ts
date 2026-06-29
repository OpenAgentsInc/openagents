import { describe, expect, it } from "bun:test"

import { pylonMcpContractImport } from "./mcp-contract-import.js"

describe("Pylon MCP contract import", () => {
  it("imports the shared Phase 0 contract without exposing a runtime transport", () => {
    expect(pylonMcpContractImport).toMatchObject({
      surface: "pylon",
      schemaVersion: "openagents.mcp.phase0.v1",
      packageName: "@openagentsinc/mcp-contract",
      authority: "local_node_control",
      outputSafety: "local_only",
      reservedTransportKind: "stdio",
      runtimeTransportExposed: false,
    })
  })
})
