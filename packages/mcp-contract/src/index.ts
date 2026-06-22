import { Schema as S } from "effect"

export const OpenAgentsMcpSchemaVersion = S.Literal("openagents.mcp.phase0.v1")
export type OpenAgentsMcpSchemaVersion = typeof OpenAgentsMcpSchemaVersion.Type

export const OPENAGENTS_MCP_SCHEMA_VERSION: OpenAgentsMcpSchemaVersion =
  "openagents.mcp.phase0.v1"

export const OpenAgentsMcpContractStatus = S.Struct({
  schemaVersion: OpenAgentsMcpSchemaVersion,
  packageName: S.Literal("@openagentsinc/mcp-contract"),
  phase: S.Literal("phase_0_contract_groundwork"),
  runtimeTransportExposed: S.Boolean,
})
export type OpenAgentsMcpContractStatus = typeof OpenAgentsMcpContractStatus.Type

export const openAgentsMcpContractStatus: OpenAgentsMcpContractStatus = {
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  packageName: "@openagentsinc/mcp-contract",
  phase: "phase_0_contract_groundwork",
  runtimeTransportExposed: false,
}

export const decodeOpenAgentsMcpContractStatus = S.decodeUnknownSync(
  OpenAgentsMcpContractStatus,
)
