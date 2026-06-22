import {
  OPENAGENTS_MCP_SCHEMA_VERSION,
  openAgentsMcpContractStatus,
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpContractStatus,
  type OpenAgentsMcpOutputSafetyClass,
  type OpenAgentsMcpTransportKind,
} from '@openagentsinc/mcp-contract'

export type AutopilotWebMcpContractImport = Readonly<{
  surface: 'autopilot_web'
  schemaVersion: OpenAgentsMcpContractStatus['schemaVersion']
  packageName: OpenAgentsMcpContractStatus['packageName']
  authority: OpenAgentsMcpAuthorityClass
  outputSafety: OpenAgentsMcpOutputSafetyClass
  reservedTransportKind: OpenAgentsMcpTransportKind
  runtimeTransportExposed: false
}>

export const autopilotWebMcpContractImport: AutopilotWebMcpContractImport = {
  surface: 'autopilot_web',
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  packageName: openAgentsMcpContractStatus.packageName,
  authority: 'public_read',
  outputSafety: 'public',
  reservedTransportKind: 'bridge_proxy',
  runtimeTransportExposed: false,
}
