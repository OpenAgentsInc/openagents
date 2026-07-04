/**
 * Public, unauthenticated MCP server — `POST` to `PUBLIC_MCP_PATH` below
 * (orank agent-readiness gap: "MCP server / manifest" + "MCP Apps support").
 *
 * This is a SEPARATE, always-open MCP endpoint from the admin/grant-gated CRM
 * MCP transport at `/api/mcp` (`crm-mcp-routes.ts`). That endpoint requires an
 * authenticated principal even for `initialize`, which is correct for private
 * CRM data but wrong for a discovery-oriented server meant to be probed by any
 * MCP client with no credential. This endpoint exposes only already-public,
 * read-only data (the same facts served by `/AGENTS.md`, `/llms.txt`, and
 * `/.well-known/openagents.json`) as MCP tools, so it creates no new
 * authority and leaks nothing beyond what is already public.
 *
 * Implements the MCP Apps extension (`io.modelcontextprotocol/ui`,
 * https://github.com/modelcontextprotocol/ext-apps) by hand at the wire
 * level — the same minimal, self-contained approach `crm-mcp-routes.ts` uses
 * for core MCP, rather than adding the `@modelcontextprotocol/ext-apps`
 * dependency to the Worker bundle for one static card. `tools/list` entries
 * declare `_meta.ui.resourceUri`; `resources/read` on that `ui://` URI
 * returns `text/html;profile=mcp-app` per the spec.
 */
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsCapabilityManifest } from './openagents-capability-manifest'
import {
  DEVELOPER_RESOURCES_URI,
  PUBLIC_DEVELOPER_RESOURCES,
  PUBLIC_MCP_PATH,
  PUBLIC_MCP_PROTOCOL_VERSION,
  PUBLIC_MCP_SERVER_INFO,
  PUBLIC_MCP_TOOLS,
  PUBLIC_MCP_UI_EXTENSION,
  PUBLIC_MCP_UI_MIME_TYPE,
  type PublicMcpToolDescriptor,
} from './public-agent-mcp-discovery'

export {
  PUBLIC_DEVELOPER_RESOURCES,
  PUBLIC_MCP_PATH,
  PUBLIC_MCP_PROTOCOL_VERSION,
  PUBLIC_MCP_SERVER_INFO,
  PUBLIC_MCP_TOOLS,
  PUBLIC_MCP_UI_EXTENSION,
  PUBLIC_MCP_UI_MIME_TYPE,
} from './public-agent-mcp-discovery'
export type {
  PublicMcpToolDescriptor,
  PublicMcpToolName,
} from './public-agent-mcp-discovery'

type HttpResponse = globalThis.Response

// Deliberately not under the public-projection prefix reserved for
// snapshots of stored/live D1 data with a staleness contract (the
// projection-surface ledger in `scripts/check-zero-debt-architecture.mjs`).
// This is a stateless protocol transport, not a data snapshot, so it lives
// alongside the other MCP transport surfaces instead.
const toolListing = (tool: PublicMcpToolDescriptor) => ({
  _meta:
    tool.uiResourceUri === undefined
      ? undefined
      : { ui: { resourceUri: tool.uiResourceUri } },
  description: tool.description,
  inputSchema: { properties: {}, type: 'object' },
  name: tool.name,
  title: tool.title,
})

const developerResourcesHtml = (): string => {
  const rows = Object.entries(PUBLIC_DEVELOPER_RESOURCES)
    .map(
      ([key, href]) =>
        `<tr><td>${key}</td><td><a href="${href}" target="_blank" rel="noopener">${href}</a></td></tr>`,
    )
    .join('')
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="color-scheme" content="dark light">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'">
<style>
body{font:14px system-ui,sans-serif;margin:0;padding:12px;color:#e6e6e6;background:#0b0b12}
table{width:100%;border-collapse:collapse}
td{padding:6px 8px;border-bottom:1px solid #262633;vertical-align:top}
td:first-child{font-weight:600;white-space:nowrap;color:#9fb3ff}
a{color:#7fd1ff}
h1{font-size:15px;margin:0 0 10px}
</style></head>
<body>
<h1>OpenAgents developer resources</h1>
<table>${rows}</table>
</body>
</html>`
}

const callTool = (
  name: string,
): Effect.Effect<{ content: ReadonlyArray<{ type: 'text'; text: string }>; structuredContent?: unknown; isError?: boolean }> => {
  switch (name) {
    case 'openagents.get_developer_resources':
      return Effect.succeed({
        content: [
          { text: JSON.stringify(PUBLIC_DEVELOPER_RESOURCES, null, 2), type: 'text' },
        ],
        structuredContent: PUBLIC_DEVELOPER_RESOURCES,
      })
    case 'openagents.get_capability_manifest':
      return openAgentsCapabilityManifest().pipe(
        Effect.map(manifest => ({
          content: [{ text: JSON.stringify(manifest, null, 2), type: 'text' as const }],
          structuredContent: manifest,
        })),
        Effect.catch(() =>
          Effect.succeed({
            content: [{ text: 'capability manifest unavailable', type: 'text' as const }],
            isError: true,
          }),
        ),
      )
    default:
      return Effect.succeed({
        content: [{ text: `Unknown tool: ${name}`, type: 'text' as const }],
        isError: true,
      })
  }
}

// --- JSON-RPC wire (same minimal shape as crm-mcp-routes.ts) ----------------

type JsonRpcId = string | number | null
const JSONRPC = '2.0' as const

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const rpcResult = (id: JsonRpcId, result: unknown): HttpResponse =>
  noStoreJsonResponse({ id, jsonrpc: JSONRPC, result })

const rpcError = (id: JsonRpcId, code: number, message: string): HttpResponse =>
  noStoreJsonResponse({ error: { code, message }, id, jsonrpc: JSONRPC })

const PARSE_ERROR = -32700
const INVALID_PARAMS = -32602
const INTERNAL_ERROR = -32603

export const routePublicAgentMcpRequest = (
  request: Request,
  _env?: unknown,
  _ctx?: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined => {
  const url = new URL(request.url)
  if (url.pathname !== PUBLIC_MCP_PATH) return undefined
  if (request.method !== 'POST') return Effect.succeed(methodNotAllowed(['POST']))

  return Effect.gen(function* () {
    const body = yield* Effect.tryPromise({
      catch: () => null,
      try: () => request.json() as Promise<unknown>,
    })
    if (!isRecord(body) || body.jsonrpc !== JSONRPC || typeof body.method !== 'string') {
      return rpcError(null, PARSE_ERROR, 'invalid JSON-RPC request')
    }

    const id: JsonRpcId =
      typeof body.id === 'string' || typeof body.id === 'number' ? body.id : null
    const method = body.method
    const params = isRecord(body.params) ? body.params : {}
    const isNotification = body.id === undefined || body.id === null

    switch (method) {
      case 'initialize':
        return rpcResult(id, {
          capabilities: {
            extensions: {
              [PUBLIC_MCP_UI_EXTENSION]: { mimeTypes: [PUBLIC_MCP_UI_MIME_TYPE] },
            },
            resources: {},
            tools: {},
          },
          protocolVersion: PUBLIC_MCP_PROTOCOL_VERSION,
          serverInfo: PUBLIC_MCP_SERVER_INFO,
        })
      case 'ping':
        return rpcResult(id, {})
      case 'notifications/initialized':
      case 'notifications/cancelled':
        return new Response(null, { status: 202 })
      case 'tools/list':
        return rpcResult(id, { tools: PUBLIC_MCP_TOOLS.map(toolListing) })
      case 'tools/call': {
        const name = typeof params.name === 'string' ? params.name : ''
        if (name === '') return rpcError(id, INVALID_PARAMS, 'tools/call requires a tool name')
        const outcome = yield* callTool(name)
        return rpcResult(id, outcome)
      }
      case 'resources/list':
        return rpcResult(id, {
          resources: [
            {
              mimeType: PUBLIC_MCP_UI_MIME_TYPE,
              name: 'OpenAgents developer resources card',
              uri: DEVELOPER_RESOURCES_URI,
            },
          ],
        })
      case 'resources/read': {
        const uri = typeof params.uri === 'string' ? params.uri : ''
        if (uri !== DEVELOPER_RESOURCES_URI) {
          return rpcError(id, INVALID_PARAMS, `Unknown resource: ${uri}`)
        }
        return rpcResult(id, {
          contents: [
            {
              mimeType: PUBLIC_MCP_UI_MIME_TYPE,
              text: developerResourcesHtml(),
              uri: DEVELOPER_RESOURCES_URI,
            },
          ],
        })
      }
      default:
        return isNotification
          ? new Response(null, { status: 202 })
          : rpcError(id, -32601, `Method not found: ${method}`)
    }
  }).pipe(
    Effect.catch(() => Effect.succeed(rpcError(null, INTERNAL_ERROR, 'internal MCP error'))),
  )
}
