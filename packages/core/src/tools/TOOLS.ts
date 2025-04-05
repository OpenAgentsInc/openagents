/**
 * Central definition of available tools in the application
 */

/**
 * Tool definition interface
 */
export interface ToolDefinition {
  id: string;             // Unique identifier for the tool
  name: string;           // Human-readable name
  description: string;    // Explanation of the tool's purpose
  type: 'builtin' | 'mcp'; // Whether the tool is built-in or from MCP
  schema: any;            // JSON schema defining the tool's parameters
  serverIdentifier: string; // ID used when communicating with server
  supportsModels?: string[]; // Optional list of model IDs that support this tool
  providerId?: string;    // ID of the provider (MCP client) that provides this tool
  providerName?: string;  // Name of the provider (MCP client) that provides this tool
}

/**
 * Built-in shell command tool
 */
export const SHELL_COMMAND_TOOL: ToolDefinition = {
  id: 'shell_command',
  name: 'Shell Command',
  description: 'Execute shell commands on the local system',
  type: 'builtin',
  schema: {
    type: 'object',
    properties: {
      command: {
        type: 'string',
        description: 'The shell command to execute'
      }
    },
    required: ['command']
  },
  serverIdentifier: 'shell_command'
};

/**
 * List of all available tools
 * This will be extended dynamically with MCP tools at runtime
 */
export const TOOLS: ToolDefinition[] = [
  SHELL_COMMAND_TOOL
];

/**
 * Utility function to find a tool by ID
 */
export function findToolById(id: string): ToolDefinition | undefined {
  return TOOLS.find(tool => tool.id === id);
}

/**
 * Utility function to extend the tools list with MCP tools
 */
export function extendWithMCPTools(
  mcpTools: Record<string, any>, 
  mcpClients?: Record<string, { id: string; name: string; tools?: string[] }>
): ToolDefinition[] {
  // Create a copy of the built-in tools
  const allTools = [...TOOLS];

  // Add each MCP tool
  for (const [toolId, toolDef] of Object.entries(mcpTools)) {
    // Skip if a tool with this ID already exists
    if (allTools.some(tool => tool.id === toolId)) {
      continue;
    }

    // Determine which provider this tool belongs to
    let providerId = '';
    let providerName = '';

    if (mcpClients) {
      for (const [clientId, clientInfo] of Object.entries(mcpClients)) {
        // If client info has a list of its tools, check if this tool is in that list
        if (clientInfo.tools && clientInfo.tools.includes(toolId)) {
          providerId = clientId;
          providerName = clientInfo.name;
          break;
        }
      }
    }

    // Convert MCP tool to our ToolDefinition format
    allTools.push({
      id: toolId,
      name: toolDef.name || toolId,
      description: toolDef.description || `MCP tool: ${toolId}`,
      type: 'mcp',
      schema: toolDef.parameters || {},
      serverIdentifier: toolId,
      providerId,
      providerName
    });
  }

  return allTools;
}