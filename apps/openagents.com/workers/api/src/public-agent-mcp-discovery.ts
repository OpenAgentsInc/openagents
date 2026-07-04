export const PUBLIC_MCP_PATH = '/api/agent-mcp'
export const PUBLIC_MCP_PROTOCOL_VERSION = '2025-06-18'
export const PUBLIC_MCP_UI_EXTENSION = 'io.modelcontextprotocol/ui'
export const PUBLIC_MCP_UI_MIME_TYPE = 'text/html;profile=mcp-app'
export const PUBLIC_MCP_SERVER_INFO = {
  name: 'openagents-public-mcp',
  title: 'OpenAgents',
  version: '1.0.0',
} as const

export const DEVELOPER_RESOURCES_URI = 'ui://openagents/developer-resources'

export const PUBLIC_DEVELOPER_RESOURCES = {
  website: 'https://openagents.com',
  agentOnboarding: 'https://openagents.com/AGENTS.md',
  llmsTxt: 'https://openagents.com/llms.txt',
  agentsMd: 'https://openagents.com/agents.md',
  capabilityManifest: 'https://openagents.com/.well-known/openagents.json',
  mcpManifest: 'https://openagents.com/.well-known/mcp.json',
  openApi: 'https://openagents.com/api/openapi.json',
  productPromises: 'https://openagents.com/docs/product-promises',
  productPromisesForum: 'https://openagents.com/forum/f/product-promises',
  sourceCode: 'https://github.com/OpenAgentsInc/openagents',
} as const

export type PublicMcpToolName =
  | 'openagents.get_developer_resources'
  | 'openagents.get_capability_manifest'

export type PublicMcpToolDescriptor = Readonly<{
  name: PublicMcpToolName
  title: string
  description: string
  uiResourceUri?: string
}>

export const PUBLIC_MCP_TOOLS: ReadonlyArray<PublicMcpToolDescriptor> = [
  {
    description:
      'Return the canonical OpenAgents developer resource links: agent onboarding doc, llms.txt, OpenAPI spec, capability manifest, product promises, and public source code. Renders an interactive card in MCP Apps-capable hosts.',
    name: 'openagents.get_developer_resources',
    title: 'Get OpenAgents developer resources',
    uiResourceUri: DEVELOPER_RESOURCES_URI,
  },
  {
    description:
      'Return the full OpenAgents public capability manifest (schemaVersion openagents.capabilities.v1): docs, auth modes, rate limits, and public API resources/actions.',
    name: 'openagents.get_capability_manifest',
    title: 'Get OpenAgents capability manifest',
  },
]
