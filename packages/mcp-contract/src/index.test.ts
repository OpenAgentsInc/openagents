import { describe, expect, test } from "bun:test"

import {
  decodeOpenAgentsMcpContractStatus,
  openAgentsMcpContractStatus,
} from "./index.js"

describe("@openagentsinc/mcp-contract", () => {
  test("exports a phase 0 status without exposing runtime transports", () => {
    expect(decodeOpenAgentsMcpContractStatus(openAgentsMcpContractStatus)).toEqual({
      schemaVersion: "openagents.mcp.phase0.v1",
      packageName: "@openagentsinc/mcp-contract",
      phase: "phase_0_contract_groundwork",
      runtimeTransportExposed: false,
    })
  })
})
