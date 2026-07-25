/**
 * FA-H13 (#8886): a minimal stdio MCP server over the Full Auto local control
 * API -- a deliberately thin pass-through client of the one OpenAPI surface
 * Desktop main serves. Each tool discovers the loopback server from the
 * mode-0600 connection file, calls the HTTP route, and returns the JSON.
 *
 * The MCP handshake (initialize / tools/list / tools/call over
 * newline-delimited JSON-RPC on stdio) is implemented by hand: the workspace
 * carries no direct `@modelcontextprotocol/sdk` dependency (it appears only
 * as a transitive peer of the Claude agent SDK, which pnpm's strict linker
 * does not expose to this package), and the protocol subset needed for these
 * pass-through tools is deliberately small. The protocol revision matches the
 * repo's public MCP surface (PUBLIC_MCP_PROTOCOL_VERSION in
 * apps/openagents.com/workers/api/src/public-agent-mcp-discovery.ts).
 *
 * The tool surface itself and the name -> control-operation dispatcher live in
 * `full-auto-mcp-tools.ts` so `full-auto-mcp-tools.test.ts` can assert the
 * restated gate 8 property ("no model-initiated path can start Full Auto
 * authority", omega#26) over the REGISTERED surface rather than over source
 * text. This file owns only transport.
 *
 * Usage: node --import tsx scripts/full-auto-mcp.ts [--user-data <path>]
 * (or set OPENAGENTS_DESKTOP_USER_DATA).
 */
import { createInterface } from "node:readline"

import {
  ControlUnavailableError,
  controlOperations,
  readControlConnection,
  resolveUserDataDir,
} from "./full-auto-control-client.ts"
import { dispatchFullAutoMcpTool, FULL_AUTO_MCP_TOOLS } from "./full-auto-mcp-tools.ts"

const MCP_PROTOCOL_VERSION = "2025-06-18"
const SERVER_INFO = { name: "openagents-desktop-full-auto", version: "1.0.0" } as const

type JsonRpcRequest = Readonly<{
  jsonrpc?: string
  id?: number | string | null
  method?: string
  params?: Record<string, unknown>
}>

const send = (message: Record<string, unknown>): void => {
  process.stdout.write(`${JSON.stringify(message)}\n`)
}
const sendResult = (id: number | string | null, result: unknown): void =>
  send({ jsonrpc: "2.0", id, result })
const sendRpcError = (id: number | string | null, code: number, message: string): void =>
  send({ jsonrpc: "2.0", id, error: { code, message } })

const takeOption = (name: string): string | undefined => {
  const index = process.argv.indexOf(name)
  return index === -1 ? undefined : process.argv[index + 1]
}
const userDataDir = resolveUserDataDir(takeOption("--user-data"))

const callTool = async (name: string, args: Record<string, unknown>): Promise<{
  content: Array<{ type: "text"; text: string }>
  isError?: boolean
}> => {
  const operations = controlOperations(readControlConnection(userDataDir))
  const result = await dispatchFullAutoMcpTool(operations, name, args)
  if (result === null) {
    return { content: [{ type: "text", text: `unknown tool: ${name}` }], isError: true }
  }
  return {
    content: [{ type: "text", text: JSON.stringify(result.body, null, 2) }],
    ...(result.status >= 200 && result.status < 300 ? {} : { isError: true }),
  }
}

const handle = async (request: JsonRpcRequest): Promise<void> => {
  const id = request.id ?? null
  switch (request.method) {
    case "initialize":
      sendResult(id, {
        protocolVersion: MCP_PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: SERVER_INFO,
      })
      return
    case "notifications/initialized":
    case "notifications/cancelled":
      return // notifications get no response
    case "ping":
      sendResult(id, {})
      return
    case "tools/list":
      sendResult(id, { tools: FULL_AUTO_MCP_TOOLS })
      return
    case "tools/call": {
      const name = typeof request.params?.name === "string" ? request.params.name : ""
      const args = (request.params?.arguments ?? {}) as Record<string, unknown>
      try {
        sendResult(id, await callTool(name, args))
      } catch (error) {
        const message = error instanceof ControlUnavailableError
          ? error.message
          : `Full Auto control call failed: ${error instanceof Error ? error.message : String(error)}`
        sendResult(id, { content: [{ type: "text", text: message }], isError: true })
      }
      return
    }
    default:
      if (id !== null) sendRpcError(id, -32601, `method not found: ${request.method ?? "(none)"}`)
  }
}

const lines = createInterface({ input: process.stdin, terminal: false })
lines.on("line", line => {
  const trimmed = line.trim()
  if (trimmed.length === 0) return
  let parsed: JsonRpcRequest
  try {
    parsed = JSON.parse(trimmed) as JsonRpcRequest
  } catch {
    sendRpcError(null, -32700, "parse error")
    return
  }
  void handle(parsed)
})
lines.on("close", () => process.exit(0))
