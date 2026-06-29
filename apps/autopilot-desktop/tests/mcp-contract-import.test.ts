import { describe, expect, test } from "bun:test"

import { autopilotDesktopMcpContractImport } from "../src/mcp-contract-import"

describe("Autopilot Desktop MCP contract import", () => {
  test("imports the shared Phase 0 contract without exposing a runtime transport", () => {
    expect(autopilotDesktopMcpContractImport).toMatchObject({
      surface: "autopilot_desktop",
      schemaVersion: "openagents.mcp.phase0.v1",
      packageName: "@openagentsinc/mcp-contract",
      authority: "coding_session_control",
      outputSafety: "workspace_private",
      reservedTransportKind: "in_process",
      runtimeTransportExposed: false,
    })
  })
})
