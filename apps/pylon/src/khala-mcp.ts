import {
  type McpResponseEnvelope,
  type McpToolCallResult,
  type McpToolContent,
} from "./tas/mcp-client.js"
import {
  createMcpToolRegistry,
  dispatchToolCall,
  handleToolsList,
  type McpToolContract,
} from "./tas/mcp-server.js"
import {
  type PylonKhalaWorkflow,
  issuePylonKhalaRequest,
} from "./khala-requester.js"
import type { TipsNetworkOptions } from "./tips.js"

export const PYLON_KHALA_MCP_PROTOCOL_VERSION = "2025-06-18"

export const PYLON_KHALA_MCP_TOOLS = [
  {
    handlerKind: "coding_session_control",
    name: "khala.request",
    readOnly: false,
  },
  {
    handlerKind: "private_account_read",
    name: "khala.resume",
    readOnly: true,
  },
  {
    handlerKind: "private_account_read",
    name: "khala.capacity",
    readOnly: true,
  },
  {
    handlerKind: "private_account_read",
    name: "khala.status",
    readOnly: true,
  },
] satisfies readonly McpToolContract[]

type PylonKhalaMcpRequest = Readonly<{
  id?: string | number | null
  jsonrpc?: unknown
  method?: unknown
  params?: unknown
}>

export type PylonKhalaMcpDeps = Readonly<{
  network: TipsNetworkOptions
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const textContent = (value: unknown): McpToolContent => ({
  text: JSON.stringify(value, null, 2),
  type: "text",
})

const toolResult = (value: unknown): McpToolCallResult => ({
  content: [textContent(value)],
})

const toolError = (message: string): McpToolCallResult => ({
  content: [{ text: message, type: "text" }],
  isError: true,
})

const result = (
  id: string,
  payload: Readonly<Record<string, unknown>>,
): McpResponseEnvelope<Readonly<Record<string, unknown>>> => ({
  id,
  jsonrpc: "2.0",
  result: payload,
})

const error = (
  id: string,
  code: number,
  message: string,
): McpResponseEnvelope<Readonly<Record<string, unknown>>> => ({
  error: { code, message },
  id,
  jsonrpc: "2.0",
})

const toolDefinitions = () =>
  handleToolsList(createMcpToolRegistry(PYLON_KHALA_MCP_TOOLS)).tools.map((tool) => ({
    annotations: {
      handlerKind: tool.handlerKind,
      readOnlyHint: tool.readOnly,
    },
    description:
      tool.name === "khala.request"
        ? "Issue a streamed openagents/khala request through caller-owned Pylon capacity."
        : tool.name === "khala.resume"
          ? "Resume a durable Khala stream from an offset without metering."
          : tool.name === "khala.status"
            ? "Read durable Khala stream status without metering."
            : "Read the configured local Khala MCP capacity/auth projection.",
    inputSchema:
      tool.name === "khala.request"
        ? {
            additionalProperties: false,
            properties: {
              objective: { type: "string" },
              prompt: { type: "string" },
              pylonRef: { type: "string" },
              targetPylonRef: { type: "string" },
              workflow: {
                enum: ["cloud_coding_session", "codex_agent_task"],
                type: "string",
              },
            },
            type: "object",
          }
        : tool.name === "khala.capacity"
          ? { additionalProperties: false, properties: {}, type: "object" }
          : {
              additionalProperties: false,
              properties: {
                durableRequestId: { type: "string" },
                offset: { type: ["number", "string"] },
              },
              required: ["durableRequestId"],
              type: "object",
            },
    name: tool.name,
  }))

const stringArg = (
  args: Record<string, unknown>,
  key: string,
): string | undefined => {
  const value = args[key]
  return typeof value === "string" && value.trim() !== ""
    ? value.trim()
    : undefined
}

const offsetArg = (
  args: Record<string, unknown>,
  key: string,
): number | string | undefined => {
  const value = args[key]
  if (typeof value === "number" && Number.isFinite(value)) return value
  return stringArg(args, key)
}

const workflowArg = (
  args: Record<string, unknown>,
): PylonKhalaWorkflow | undefined => {
  const value = stringArg(args, "workflow")
  if (value === undefined) return undefined
  if (value === "cloud_coding_session" || value === "codex_agent_task") {
    return value
  }
  throw new Error("workflow must be cloud_coding_session or codex_agent_task")
}

const requireAgentToken = (network: TipsNetworkOptions): string => {
  const token = network.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (token === undefined || token.trim() === "") {
    throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required for Khala MCP calls")
  }
  return token
}

async function callRemoteKhalaMcpTool(
  network: TipsNetworkOptions,
  name: "khala.resume" | "khala.status",
  args: Record<string, unknown>,
): Promise<McpToolCallResult> {
  const response = await (network.fetch ?? fetch)(new URL("/api/mcp", network.baseUrl), {
    body: JSON.stringify({
      id: `pylon.local.${name}`,
      jsonrpc: "2.0",
      method: "tools/call",
      params: {
        arguments: args,
        name,
      },
    }),
    headers: {
      Authorization: `Bearer ${requireAgentToken(network)}`,
      "Content-Type": "application/json",
    },
    method: "POST",
  })
  const text = await response.text()
  if (!response.ok) {
    return toolError(`remote ${name} failed (${response.status}): ${text.trim() || response.status}`)
  }
  const envelope = JSON.parse(text) as McpResponseEnvelope<McpToolCallResult>
  if (envelope.error !== undefined) {
    return toolError(envelope.error.message)
  }
  if (envelope.result === undefined) {
    return toolError(`remote ${name} returned no result`)
  }
  return envelope.result
}

async function callKhalaTool(
  deps: PylonKhalaMcpDeps,
  name: string,
  args: Record<string, unknown>,
): Promise<McpToolCallResult> {
  try {
    if (name === "khala.request") {
      const prompt = stringArg(args, "prompt") ?? stringArg(args, "objective")
      if (prompt === undefined) {
        return toolError("khala.request requires prompt or objective")
      }
      const targetPylonRef =
        stringArg(args, "targetPylonRef") ?? stringArg(args, "pylonRef")
      const workflow = workflowArg(args)
      return toolResult(
        await issuePylonKhalaRequest(deps.network, {
          prompt,
          ...(targetPylonRef === undefined ? {} : { targetPylonRef }),
          ...(workflow === undefined ? {} : { workflow }),
        }),
      )
    }

    if (name === "khala.resume") {
      const durableRequestId = stringArg(args, "durableRequestId")
      if (durableRequestId === undefined) {
        return toolError("khala.resume requires durableRequestId")
      }
      const offset = offsetArg(args, "offset")
      return callRemoteKhalaMcpTool(deps.network, "khala.resume", {
        durableRequestId,
        ...(offset === undefined ? {} : { offset }),
      })
    }

    if (name === "khala.status") {
      const durableRequestId = stringArg(args, "durableRequestId")
      if (durableRequestId === undefined) {
        return toolError("khala.status requires durableRequestId")
      }
      return callRemoteKhalaMcpTool(deps.network, "khala.status", {
        durableRequestId,
      })
    }

    if (name === "khala.capacity") {
      return toolResult({
        baseUrl: deps.network.baseUrl,
        schema: "openagents.pylon.khala_mcp_capacity.v1",
        tokenConfigured:
          deps.network.agentToken !== undefined ||
          process.env.OPENAGENTS_AGENT_TOKEN !== undefined,
        tools: PYLON_KHALA_MCP_TOOLS.map((tool) => ({
          authorityClass: tool.handlerKind,
          name: tool.name,
          readOnly: tool.readOnly,
        })),
      })
    }

    return toolError(`Unknown tool: ${name}`)
  } catch (caught) {
    return toolError(caught instanceof Error ? caught.message : String(caught))
  }
}

export async function handlePylonKhalaMcpRequest(
  request: PylonKhalaMcpRequest,
  deps: PylonKhalaMcpDeps,
): Promise<McpResponseEnvelope<Readonly<Record<string, unknown>>>> {
  const id =
    typeof request.id === "string" || typeof request.id === "number"
      ? String(request.id)
      : "null"
  if (request.jsonrpc !== "2.0" || typeof request.method !== "string") {
    return error(id, -32600, "invalid JSON-RPC request")
  }

  if (request.method === "initialize") {
    return result(id, {
      capabilities: { tools: {} },
      protocolVersion: PYLON_KHALA_MCP_PROTOCOL_VERSION,
      serverInfo: {
        name: "openagents-khala-pylon-mcp",
        title: "OpenAgents Khala via Pylon",
        version: "0.1.0",
      },
    })
  }

  if (request.method === "ping") {
    return result(id, {})
  }

  if (request.method === "notifications/initialized") {
    return result(id, {})
  }

  if (request.method === "tools/list") {
    return result(id, { tools: toolDefinitions() })
  }

  if (request.method === "tools/call") {
    const params = isRecord(request.params) ? request.params : {}
    const name = typeof params.name === "string" ? params.name : ""
    const args = isRecord(params.arguments) ? params.arguments : {}
    const dispatch = dispatchToolCall(createMcpToolRegistry(PYLON_KHALA_MCP_TOOLS), {
      args,
      name,
    })
    if (!dispatch.ok) {
      return result(id, toolError(dispatch.error.message))
    }

    return result(id, await callKhalaTool(deps, dispatch.route.name, args))
  }

  return error(id, -32601, `method not found: ${request.method}`)
}

export const pylonKhalaMcpConfig = (input: {
  baseUrl?: string | undefined
  command?: string | undefined
}) => {
  const baseUrl = input.baseUrl ?? "https://openagents.com"
  const command = input.command ?? "pylon"
  return {
    mcpServers: {
      "openagents-khala-local": {
        args: ["mcp"],
        command,
        env: {
          OPENAGENTS_AGENT_TOKEN: "${OPENAGENTS_AGENT_TOKEN}",
          PYLON_OPENAGENTS_BASE_URL: baseUrl,
        },
      },
      "openagents-khala-remote": {
        headers: {
          Authorization: "Bearer ${OPENAGENTS_AGENT_TOKEN}",
        },
        type: "http",
        url: `${baseUrl.replace(/\/+$/, "")}/api/mcp`,
      },
    },
    schema: "openagents.pylon.mcp_config.v1",
  }
}

export async function runPylonKhalaMcpStdio(
  deps: PylonKhalaMcpDeps,
): Promise<void> {
  const decoder = new TextDecoder()
  let buffer = ""
  for await (const chunk of Bun.stdin.stream()) {
    buffer += decoder.decode(chunk, { stream: true })
    let newline = buffer.indexOf("\n")
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trim()
      buffer = buffer.slice(newline + 1)
      if (line !== "") {
        const response = await handlePylonKhalaMcpRequest(
          JSON.parse(line) as PylonKhalaMcpRequest,
          deps,
        )
        process.stdout.write(`${JSON.stringify(response)}\n`)
      }
      newline = buffer.indexOf("\n")
    }
  }
}
