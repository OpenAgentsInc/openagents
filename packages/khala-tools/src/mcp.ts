import {
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpLifecycleStatus,
  isOpenAgentsMcpHighRiskAuthority,
} from "@openagentsinc/mcp-contract"
import { Effect } from "effect"
import { createGlobTool } from "./glob.js"
import {
  executeKhalaTool,
  khalaToolError,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaJsonSchema,
  type KhalaToolDefinition,
  type KhalaToolRegistry,
  type KhalaToolResult,
  type KhalaToolServices,
  type RegisteredKhalaTool,
} from "./index.js"
import { createGrepTool } from "./grep.js"
import { createLsTool } from "./ls.js"
import { createReadTool } from "./read.js"

export const KHALA_MCP_PROTOCOL_VERSION = "2025-06-18"

export type KhalaMcpJsonPrimitive = string | number | boolean | null
export type KhalaMcpJsonValue =
  | KhalaMcpJsonPrimitive
  | readonly KhalaMcpJsonValue[]
  | { readonly [key: string]: KhalaMcpJsonValue }

export type KhalaMcpRequest = Readonly<{
  id?: string | number | null
  jsonrpc?: unknown
  method?: unknown
  params?: unknown
}>

export type KhalaMcpResponse<Result extends Readonly<Record<string, unknown>> = Readonly<Record<string, unknown>>> =
  Readonly<{
    error?: Readonly<{
      code: number
      message: string
    }>
    id: string
    jsonrpc: "2.0"
    result?: Result
  }>

export type KhalaMcpToolContent = Readonly<{
  data?: string
  mimeType?: string
  text?: string
  type: string
}>

export type KhalaMcpToolCallResult = Readonly<{
  content: readonly KhalaMcpToolContent[]
  isError?: boolean
}>

export type KhalaMcpToolDefinition = Readonly<{
  annotations: Readonly<{
    khalaAuthority?: string
    lifecycleStatus?: KhalaMcpServerLifecycle
    policyScoped: boolean
    readOnlyHint: boolean
    sourceLabel: string
    sourceServerRef?: string
  }>
  description: string
  inputSchema: KhalaJsonSchema
  name: string
}>

export type KhalaMcpServerLifecycle =
  | "configured"
  | "pending"
  | "disabled"
  | "failed"
  | "needs_auth"
  | "connected"
  | "blocked_by_policy"

export type KhalaMcpExternalTool = Readonly<{
  description?: string
  inputSchema?: KhalaJsonSchema
  name: string
}>

export type KhalaMcpExternalServerConfig = Readonly<{
  displayName: string
  lifecycleStatus: KhalaMcpServerLifecycle | OpenAgentsMcpLifecycleStatus
  requestedAuthorities: ReadonlyArray<OpenAgentsMcpAuthorityClass>
  serverRef: string
  sourceLabel?: string
}>

export type KhalaMcpExternalServerProjection = Readonly<{
  displayName: string
  lifecycleStatus: KhalaMcpServerLifecycle
  policyAllowed: boolean
  requestedAuthorities: ReadonlyArray<OpenAgentsMcpAuthorityClass>
  serverRef: string
  sourceLabel: string
}>

export type KhalaMcpExternalToolProjection = Readonly<{
  description: string
  inputSchema: KhalaJsonSchema
  name: string
  originalName: string
  serverRef: string
  sourceLabel: string
}>

export type KhalaMcpClientPolicy = Readonly<{
  allowedAuthorities: ReadonlyArray<OpenAgentsMcpAuthorityClass>
  denyHighRisk?: boolean
}>

export type KhalaMcpExternalTransport = Readonly<{
  callTool: (
    server: KhalaMcpExternalServerProjection,
    name: string,
    args: Readonly<Record<string, unknown>>,
  ) => Promise<KhalaMcpToolCallResult>
  listTools: (server: KhalaMcpExternalServerProjection) => Promise<ReadonlyArray<KhalaMcpExternalTool>>
}>

export type KhalaMcpClient = Readonly<{
  callTool: (name: string, args?: Readonly<Record<string, unknown>>) => Promise<KhalaMcpToolCallResult>
  listServers: () => ReadonlyArray<KhalaMcpExternalServerProjection>
  listTools: () => Promise<ReadonlyArray<KhalaMcpExternalToolProjection>>
}>

export type KhalaMcpServerOptions = Readonly<{
  policy?: KhalaMcpClientPolicy
  registry?: KhalaToolRegistry
  services?: KhalaToolServices
  serverName?: string
  serverVersion?: string
}>

const defaultPublicMcpAuthorityPolicy: KhalaMcpClientPolicy = {
  allowedAuthorities: ["public_read", "workspace_read"],
  denyHighRisk: true,
}

export function createKhalaPublicMcpToolRegistry(): KhalaToolRegistry {
  return makeKhalaToolRegistry([
    createReadTool(),
    createLsTool(),
    createGlobTool(),
    createGrepTool(),
  ])
}

export function makeKhalaMcpClient(input: {
  readonly builtInToolNames: ReadonlyArray<string>
  readonly policy?: KhalaMcpClientPolicy
  readonly servers: ReadonlyArray<KhalaMcpExternalServerConfig>
  readonly transport: KhalaMcpExternalTransport
}): KhalaMcpClient {
  const builtIns = new Set(input.builtInToolNames)
  const policy = input.policy ?? defaultPublicMcpAuthorityPolicy
  const servers = input.servers.map(server => projectMcpServer(server, policy))

  return {
    callTool: async (name, args = {}) => {
      const listed = await listExternalMcpTools(servers, builtIns, input.transport)
      const tool = listed.find(candidate => candidate.name === name)
      if (tool === undefined) {
        return {
          content: [{ text: `unknown MCP tool: ${name}`, type: "text" }],
          isError: true,
        }
      }
      const server = servers.find(candidate => candidate.serverRef === tool.serverRef)
      if (server === undefined || !serverCanConnect(server)) {
        return {
          content: [{ text: `MCP server unavailable: ${tool.serverRef}`, type: "text" }],
          isError: true,
        }
      }
      return input.transport.callTool(server, tool.originalName, args)
    },
    listServers: () => servers,
    listTools: () => listExternalMcpTools(servers, builtIns, input.transport),
  }
}

export async function handleKhalaMcpRequest(
  request: KhalaMcpRequest,
  options: KhalaMcpServerOptions = {},
): Promise<KhalaMcpResponse> {
  const id =
    typeof request.id === "string" || typeof request.id === "number"
      ? String(request.id)
      : "null"
  if (request.jsonrpc !== "2.0" || typeof request.method !== "string") {
    return jsonRpcError(id, -32600, "invalid JSON-RPC request")
  }

  if (request.method === "initialize") {
    return jsonRpcResult(id, {
      capabilities: { tools: { listChanged: false } },
      protocolVersion: KHALA_MCP_PROTOCOL_VERSION,
      serverInfo: {
        name: options.serverName ?? "openagents-khala-mcp",
        title: "OpenAgents Khala MCP",
        version: options.serverVersion ?? "0.1.0",
      },
    })
  }

  if (request.method === "ping" || request.method === "notifications/initialized") {
    return jsonRpcResult(id, {})
  }

  const registry = options.registry ?? createKhalaPublicMcpToolRegistry()
  const services = options.services ?? makeKhalaToolServices()
  const policy = options.policy ?? defaultPublicMcpAuthorityPolicy

  if (request.method === "tools/list") {
    return jsonRpcResult(id, { tools: listKhalaMcpToolDefinitions(registry, policy) })
  }

  if (request.method === "tools/call") {
    const params = isRecord(request.params) ? request.params : {}
    const name = typeof params.name === "string" ? params.name : ""
    const args = isRecord(params.arguments) ? params.arguments : {}
    if (!listKhalaMcpToolDefinitions(registry, policy).some(tool => tool.name === name)) {
      return jsonRpcResult(id, toMcpToolResult(khalaToolError("unknown_tool", `Unknown MCP tool: ${name}`)))
    }
    const result = await Effect.runPromise(executeKhalaTool(
      registry,
      {
        arguments: args,
        id: `mcp.${Date.now().toString(36)}`,
        name,
        sessionId: "khala.mcp.stdio",
      },
      services,
    ))
    return jsonRpcResult(id, toMcpToolResult(result))
  }

  return jsonRpcError(id, -32601, `method not found: ${request.method}`)
}

export async function runKhalaMcpServerStdio(options: KhalaMcpServerOptions = {}): Promise<void> {
  const decoder = new TextDecoder()
  let buffer = ""
  for await (const chunk of Bun.stdin.stream()) {
    buffer += decoder.decode(chunk, { stream: true })
    let newline = buffer.indexOf("\n")
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trim()
      buffer = buffer.slice(newline + 1)
      if (line !== "") {
        const response = await handleKhalaMcpRequest(
          JSON.parse(line) as KhalaMcpRequest,
          options,
        )
        process.stdout.write(`${JSON.stringify(response)}\n`)
      }
      newline = buffer.indexOf("\n")
    }
  }
}

export function listKhalaMcpToolDefinitions(
  registry: KhalaToolRegistry,
  policy: KhalaMcpClientPolicy = defaultPublicMcpAuthorityPolicy,
): ReadonlyArray<KhalaMcpToolDefinition> {
  return registry
    .list()
    .filter(definition => khalaToolAllowedByPolicy(definition, policy))
    .map(definition => ({
      annotations: {
        khalaAuthority: definition.authority,
        policyScoped: true,
        readOnlyHint: definition.authority === "read" || definition.authority === "search",
        sourceLabel: "khala-built-in",
      },
      description: definition.description,
      inputSchema: definition.inputSchema,
      name: definition.name,
    }))
}

export function createExternalMcpRegisteredTools(input: {
  readonly client: KhalaMcpClient
  readonly sourceAvailability?: KhalaToolDefinition["availability"]
}): Promise<ReadonlyArray<RegisteredKhalaTool>> {
  return input.client.listTools().then(tools =>
    tools.map(tool => ({
      definition: {
        authority: "external_directory",
        availability: input.sourceAvailability ?? ["extension"],
        description: tool.description,
        executionMode: "delegated",
        inputSchema: tool.inputSchema,
        internalId: `khala.mcp.external.${tool.serverRef}.${tool.originalName}`,
        label: `${tool.sourceLabel}: ${tool.originalName}`,
        name: tool.name,
        permissionMode: "approval_required",
        prompt: `Call the external MCP tool ${tool.originalName} from ${tool.sourceLabel}.`,
        promptGuidelines: [
          "External MCP tools are always source-labeled and namespaced.",
          "Do not treat external tools as first-party Khala built-ins.",
        ],
      },
      execute: (args) =>
        Effect.promise(() => input.client.callTool(tool.name, args)).pipe(
          Effect.map(result => result.isError === true
            ? khalaToolError("external_mcp_tool_failed", firstMcpText(result) ?? "External MCP tool failed")
            : {
                artifacts: [],
                modelOutput: { text: firstMcpText(result) ?? JSON.stringify(result.content) },
                privateDataRefs: [],
                publicSafety: "private",
                publicSummary: `External MCP tool ${tool.name} completed.`,
                redactionRefs: [],
                status: "ok",
                ui: {
                  content: result.content,
                  kind: "external_mcp_tool_result",
                  sourceLabel: tool.sourceLabel,
                  sourceServerRef: tool.serverRef,
                },
              }),
        ),
    })),
  )
}

function projectMcpServer(
  server: KhalaMcpExternalServerConfig,
  policy: KhalaMcpClientPolicy,
): KhalaMcpExternalServerProjection {
  const lifecycleStatus = normalizeMcpLifecycle(server.lifecycleStatus)
  const policyAllowed = authoritiesAllowed(server.requestedAuthorities, policy)
  return {
    displayName: server.displayName,
    lifecycleStatus: policyAllowed ? lifecycleStatus : "blocked_by_policy",
    policyAllowed,
    requestedAuthorities: [...server.requestedAuthorities],
    serverRef: server.serverRef,
    sourceLabel: server.sourceLabel ?? server.displayName,
  }
}

async function listExternalMcpTools(
  servers: ReadonlyArray<KhalaMcpExternalServerProjection>,
  builtIns: ReadonlySet<string>,
  transport: KhalaMcpExternalTransport,
): Promise<ReadonlyArray<KhalaMcpExternalToolProjection>> {
  const output: KhalaMcpExternalToolProjection[] = []
  for (const server of servers) {
    if (!serverCanConnect(server)) continue
    const tools = await transport.listTools(server)
    for (const tool of tools) {
      const namespaced = namespaceExternalMcpToolName(server.serverRef, tool.name)
      if (builtIns.has(namespaced) || builtIns.has(tool.name)) {
        output.push({
          description: `${tool.description ?? tool.name} (source: ${server.sourceLabel})`,
          inputSchema: tool.inputSchema ?? {},
          name: namespaced,
          originalName: tool.name,
          serverRef: server.serverRef,
          sourceLabel: server.sourceLabel,
        })
        continue
      }
      output.push({
        description: `${tool.description ?? tool.name} (source: ${server.sourceLabel})`,
        inputSchema: tool.inputSchema ?? {},
        name: namespaced,
        originalName: tool.name,
        serverRef: server.serverRef,
        sourceLabel: server.sourceLabel,
      })
    }
  }
  return output
}

function namespaceExternalMcpToolName(serverRef: string, toolName: string): string {
  return `mcp.${safeMcpNameSegment(serverRef)}.${safeMcpNameSegment(toolName)}`
}

function safeMcpNameSegment(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, "_")
    .replace(/^_+|_+$/gu, "")
  return normalized.length > 0 ? normalized : "tool"
}

function serverCanConnect(server: KhalaMcpExternalServerProjection): boolean {
  return server.policyAllowed && (server.lifecycleStatus === "configured" || server.lifecycleStatus === "connected")
}

function normalizeMcpLifecycle(value: KhalaMcpExternalServerConfig["lifecycleStatus"]): KhalaMcpServerLifecycle {
  switch (value) {
    case "connected":
    case "configured":
    case "disabled":
    case "failed":
    case "needs_auth":
    case "pending":
    case "blocked_by_policy":
      return value
    case "enabled":
    case "connecting":
    case "discovered":
      return "configured"
    case "pending_approval":
      return "pending"
    case "rejected":
    case "revoked":
      return "disabled"
    default:
      return "failed"
  }
}

function khalaToolAllowedByPolicy(
  definition: KhalaToolDefinition,
  policy: KhalaMcpClientPolicy,
): boolean {
  const authority = khalaAuthorityToMcpAuthority(definition.authority)
  if (authority === undefined) return false
  return authoritiesAllowed([authority], policy)
}

function khalaAuthorityToMcpAuthority(authority: KhalaToolDefinition["authority"]): OpenAgentsMcpAuthorityClass | undefined {
  switch (authority) {
    case "read":
    case "search":
      return "workspace_read"
    case "network":
      return "public_read"
    case "edit":
    case "write":
    case "patch":
      return "workspace_write"
    case "shell":
    case "process_stdin":
      return "coding_session_control"
    case "credential":
      return "private_account_read"
    default:
      return undefined
  }
}

function authoritiesAllowed(
  authorities: ReadonlyArray<OpenAgentsMcpAuthorityClass>,
  policy: KhalaMcpClientPolicy,
): boolean {
  const allowed = new Set(policy.allowedAuthorities)
  return authorities.every(authority =>
    allowed.has(authority) && !(policy.denyHighRisk === true && isOpenAgentsMcpHighRiskAuthority(authority)),
  )
}

function toMcpToolResult(result: KhalaToolResult): KhalaMcpToolCallResult {
  return {
    content: [
      {
        text: result.modelOutput.text,
        type: "text",
      },
    ],
    ...(result.status === "ok" ? {} : { isError: true }),
  }
}

function jsonRpcResult(
  id: string,
  payload: Readonly<Record<string, unknown>>,
): KhalaMcpResponse {
  return {
    id,
    jsonrpc: "2.0",
    result: payload,
  }
}

function jsonRpcError(id: string, code: number, message: string): KhalaMcpResponse {
  return {
    error: { code, message },
    id,
    jsonrpc: "2.0",
  }
}

function firstMcpText(result: KhalaMcpToolCallResult): string | undefined {
  return result.content
    .map(item => item.text)
    .find(text => text !== undefined && text.trim() !== "")
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
