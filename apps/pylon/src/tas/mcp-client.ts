export type McpJsonPrimitive = string | number | boolean | null

export type McpJsonValue =
  | McpJsonPrimitive
  | readonly McpJsonValue[]
  | { readonly [key: string]: McpJsonValue }

export type McpRequestId = string

export type McpRequestEnvelope<
  Method extends string,
  Params extends McpJsonValue | undefined,
> = Readonly<{
  jsonrpc: "2.0"
  id: McpRequestId
  method: Method
  params: Params
}>

export type McpResponseError = Readonly<{
  code: number
  message: string
  data?: McpJsonValue
}>

// `Result` is the structured, JSON-serializable success payload. We constrain
// to an object shape rather than the stricter `McpJsonValue` so structured
// result types (e.g. McpToolCallResult, whose nested index signatures permit
// `| undefined`) are accepted without weakening the wire model.
export type McpResponseEnvelope<Result extends Readonly<Record<string, unknown>>> = Readonly<{
  jsonrpc: "2.0"
  id: McpRequestId
  result?: Result
  error?: McpResponseError
}>

export type McpClientCapabilities = Readonly<{
  tools?: Record<string, never>
}>

export type McpServerCapabilities = Readonly<{
  tools?: Readonly<{
    listChanged?: boolean
  }>
}>

export type McpInitializeRequest = McpRequestEnvelope<
  "initialize",
  Readonly<{
    protocolVersion: string
    capabilities: McpClientCapabilities
    clientInfo: Readonly<{
      name: string
      version: string
    }>
  }>
>

export type McpInitializeResult = Readonly<{
  protocolVersion: string
  capabilities: McpServerCapabilities
  serverInfo: Readonly<{
    name: string
    version: string
  }>
}>

export type McpToolsListRequest = McpRequestEnvelope<
  "tools/list",
  Readonly<{
    cursor?: string
  }>
>

export type McpToolDefinition = Readonly<{
  name: string
  description?: string
  inputSchema?: McpJsonValue
}>

export type McpToolsListResult = Readonly<{
  tools: readonly McpToolDefinition[]
  nextCursor?: string
}>

export type McpToolsCallRequest = McpRequestEnvelope<
  "tools/call",
  Readonly<{
    name: string
    arguments: McpJsonValue
  }>
>

export type McpToolContent = Readonly<{
  type: string
  text?: string
  data?: string
  mimeType?: string
  [key: string]: McpJsonValue | undefined
}>

export type McpToolCallResult = Readonly<{
  content: readonly McpToolContent[]
  isError?: boolean
}>

export type McpParsedToolResult =
  | Readonly<{
      ok: true
      content: readonly McpToolContent[]
    }>
  | Readonly<{
      ok: false
      error: string
    }>

export function buildToolsListRequest(): McpToolsListRequest {
  const params = {}

  return {
    jsonrpc: "2.0",
    id: buildRequestId("tools/list", params),
    method: "tools/list",
    params,
  }
}

export function buildToolCallRequest(
  name: string,
  args: McpJsonValue = {},
): McpToolsCallRequest {
  const params = {
    name,
    arguments: args,
  }

  return {
    jsonrpc: "2.0",
    id: buildRequestId("tools/call", params),
    method: "tools/call",
    params,
  }
}

export function parseToolResult(
  response: McpResponseEnvelope<McpToolCallResult>,
): McpParsedToolResult {
  if (response.error) {
    return {
      ok: false,
      error: response.error.message,
    }
  }

  if (!response.result) {
    return {
      ok: false,
      error: "Missing MCP tool result",
    }
  }

  if (response.result.isError === true) {
    return {
      ok: false,
      error: summarizeToolError(response.result.content),
    }
  }

  return {
    ok: true,
    content: response.result.content,
  }
}

export function isResponseForRequest(
  request: Readonly<{ id: McpRequestId }>,
  response: Readonly<{ id: McpRequestId }>,
): boolean {
  return request.id === response.id
}

function buildRequestId(method: string, params: McpJsonValue): McpRequestId {
  return `tas.mcp.${method}:${canonicalJson(params)}`
}

function canonicalJson(value: McpJsonValue): string {
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalJson(item)).join(",")}]`
  }

  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => `${JSON.stringify(key)}:${canonicalJson(entry)}`)
      .join(",")}}`
  }

  return JSON.stringify(value)
}

function summarizeToolError(content: readonly McpToolContent[]): string {
  const text = content
    .map((item) => item.text)
    .find((candidate) => candidate !== undefined && candidate.trim() !== "")

  return text ?? "MCP tool returned an error result"
}
