import {
  OPENAGENTS_MCP_SCHEMA_VERSION,
  openAgentsMcpContractStatus,
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpContractStatus,
  type OpenAgentsMcpOutputSafetyClass,
  type OpenAgentsMcpTransportKind,
} from "@openagentsinc/mcp-contract"

export type AutopilotDesktopMcpContractImport = Readonly<{
  surface: "autopilot_desktop"
  schemaVersion: OpenAgentsMcpContractStatus["schemaVersion"]
  packageName: OpenAgentsMcpContractStatus["packageName"]
  authority: OpenAgentsMcpAuthorityClass
  outputSafety: OpenAgentsMcpOutputSafetyClass
  reservedTransportKind: OpenAgentsMcpTransportKind
  runtimeTransportExposed: false
}>

export const autopilotDesktopMcpContractImport: AutopilotDesktopMcpContractImport = {
  surface: "autopilot_desktop",
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  packageName: openAgentsMcpContractStatus.packageName,
  authority: "coding_session_control",
  outputSafety: "workspace_private",
  reservedTransportKind: "in_process",
  runtimeTransportExposed: false,
}
