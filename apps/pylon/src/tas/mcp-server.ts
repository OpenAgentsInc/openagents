export type McpToolHandlerKind =
  | "browser"
  | "desktop"
  | "repository"
  | "session"
  | "status"
  | (string & {})

export type McpToolContract = {
  readonly name: string
  readonly handlerKind: McpToolHandlerKind
  readonly readOnly: boolean
}

export type McpToolRegistry = {
  readonly tools: Map<string, McpToolContract>
}

export type McpToolsListResponse = {
  readonly tools: readonly McpListedTool[]
}

export type McpListedTool = {
  readonly name: string
  readonly handlerKind: McpToolHandlerKind
  readonly readOnly: boolean
}

export type McpToolCallRequest = {
  readonly name: string
  readonly args: unknown
}

export type McpToolRouteDescriptor = {
  readonly name: string
  readonly handlerKind: McpToolHandlerKind
  readonly readOnly: boolean
  readonly args: unknown
}

export type McpToolCallResult =
  | {
      readonly ok: true
      readonly route: McpToolRouteDescriptor
    }
  | {
      readonly ok: false
      readonly error: McpToolCallError
    }

export type McpToolCallError = {
  readonly code: "unknown_tool"
  readonly message: string
}

export function createMcpToolRegistry(
  initialTools: readonly McpToolContract[] = [],
): McpToolRegistry {
  const registry: McpToolRegistry = {
    tools: new Map(),
  }

  for (const tool of initialTools) {
    registerTool(registry, tool)
  }

  return registry
}

export function registerTool(
  registry: McpToolRegistry,
  tool: McpToolContract,
): McpToolContract {
  if (registry.tools.has(tool.name)) {
    throw new Error(`MCP tool already registered: ${tool.name}`)
  }

  registry.tools.set(tool.name, tool)
  return tool
}

export function handleToolsList(
  registry: McpToolRegistry,
): McpToolsListResponse {
  return {
    tools: Array.from(registry.tools.values(), (tool) => ({
      name: tool.name,
      handlerKind: tool.handlerKind,
      readOnly: tool.readOnly,
    })),
  }
}

export function dispatchToolCall(
  registry: McpToolRegistry,
  request: McpToolCallRequest,
): McpToolCallResult {
  const tool = registry.tools.get(request.name)

  if (!tool) {
    return {
      ok: false,
      error: {
        code: "unknown_tool",
        message: `Unknown MCP tool: ${request.name}`,
      },
    }
  }

  return {
    ok: true,
    route: {
      name: tool.name,
      handlerKind: tool.handlerKind,
      readOnly: tool.readOnly,
      args: request.args,
    },
  }
}
