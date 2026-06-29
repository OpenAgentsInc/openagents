import {
  OPENAGENTS_MCP_SCHEMA_VERSION,
  openAgentsMcpContractStatus,
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpContractStatus,
  type OpenAgentsMcpOutputSafetyClass,
  type OpenAgentsMcpTransportKind,
} from "@openagentsinc/mcp-contract"

export type PylonMcpContractImport = Readonly<{
  surface: "pylon"
  schemaVersion: OpenAgentsMcpContractStatus["schemaVersion"]
  packageName: OpenAgentsMcpContractStatus["packageName"]
  authority: OpenAgentsMcpAuthorityClass
  outputSafety: OpenAgentsMcpOutputSafetyClass
  reservedTransportKind: OpenAgentsMcpTransportKind
  runtimeTransportExposed: false
}>

export const pylonMcpContractImport: PylonMcpContractImport = {
  surface: "pylon",
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  packageName: openAgentsMcpContractStatus.packageName,
  authority: "local_node_control",
  outputSafety: "local_only",
  reservedTransportKind: "stdio",
  runtimeTransportExposed: false,
}
