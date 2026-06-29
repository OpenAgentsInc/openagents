import {
  OPENAGENTS_MCP_SCHEMA_VERSION,
  openAgentsMcpContractStatus,
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpContractStatus,
  type OpenAgentsMcpOutputSafetyClass,
  type OpenAgentsMcpTransportKind,
} from '@openagentsinc/mcp-contract'

export type ApiWorkerMcpContractImport = Readonly<{
  surface: 'api_worker'
  schemaVersion: OpenAgentsMcpContractStatus['schemaVersion']
  packageName: OpenAgentsMcpContractStatus['packageName']
  authority: OpenAgentsMcpAuthorityClass
  outputSafety: OpenAgentsMcpOutputSafetyClass
  reservedTransportKind: OpenAgentsMcpTransportKind
  runtimeTransportExposed: false
}>

export const apiWorkerMcpContractImport: ApiWorkerMcpContractImport = {
  surface: 'api_worker',
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  packageName: openAgentsMcpContractStatus.packageName,
  authority: 'operator_read',
  outputSafety: 'operator',
  reservedTransportKind: 'streamable_http',
  runtimeTransportExposed: false,
}
