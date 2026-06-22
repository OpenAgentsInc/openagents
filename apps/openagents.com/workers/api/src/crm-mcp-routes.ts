/**
 * MCP JSON-RPC transport handler — `POST /api/mcp` (epic #5991, sub-issue #5992).
 *
 * A stateless Streamable-HTTP JSON-RPC endpoint mounted in the Worker omni
 * cascade. It speaks the MCP lifecycle (`initialize`, `notifications/*`,
 * `ping`) and the tool/resource methods (`tools/list`, `tools/call`,
 * `resources/list`, `resources/read`), delegating the actual catalog to an
 * INJECTED `CrmMcpCatalog`. This issue ships the transport + an empty catalog;
 * the CRM read tools (#5993) and resources (#5994) provide the real catalog.
 *
 * MCP is a projection of the existing CRM HTTP routes — it creates no new
 * authority. Auth is at the transport boundary (admin token first, #5995 adds
 * scoped grants). Protocol failures are JSON-RPC errors; tool failures are
 * returned as `isError` tool results so MCP clients see them as tool errors,
 * not transport errors.
 */
import { Effect, Schema as S } from 'effect'

import type { OpenAgentsMcpGrant } from '@openagentsinc/mcp-contract'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

/**
 * The authenticated MCP caller: a bound tenant + a set of authority grants.
 * Resolved at the transport boundary (admin token = full grant; a scoped MCP
 * grant row = its declared authorities + bound tenant). The catalog filters
 * tools/resources by these grants and reads ONLY `tenantRef` — client-supplied
 * tenant is never trusted. (#5995)
 */
export type McpPrincipal = Readonly<{
  subjectRef: string
  tenantRef: string
  grants: ReadonlyArray<OpenAgentsMcpGrant>
}>

/**
 * Typed adapter error so catalog promise calls use `Effect.tryPromise` with a
 * typed `catch` (no untyped-catch warning, and not a counted `Effect.promise`
 * route-dependency adapter).
 */
class CrmMcpAdapterError extends S.TaggedErrorClass<CrmMcpAdapterError>()(
  'CrmMcpAdapterError',
  { message: S.String, unknownTarget: S.Boolean },
) {}

const adapterError = (error: unknown): CrmMcpAdapterError =>
  new CrmMcpAdapterError({
    message: error instanceof Error ? error.message : String(error),
    unknownTarget:
      error instanceof Error &&
      (error.message === 'unknown_tool' || error.message === 'unknown_resource'),
  })

export const CRM_MCP_PATH = '/api/mcp'
// Protocol version we advertise; clients negotiate against their own.
export const CRM_MCP_PROTOCOL_VERSION = '2025-06-18'
export const CRM_MCP_SERVER_INFO = {
  name: 'openagents-crm-mcp',
  title: 'OpenAgents CRM',
  version: '0.1.0',
} as const

// --- MCP wire shapes (server-side; minimal, self-contained) ----------------

export type McpToolListing = Readonly<{
  name: string
  title: string
  description: string
  inputSchema: Readonly<Record<string, unknown>>
  annotations?: Readonly<Record<string, unknown>>
}>

export type McpToolContentBlock = Readonly<{ type: 'text'; text: string }>

export type McpToolCallOutcome = Readonly<{
  content: ReadonlyArray<McpToolContentBlock>
  isError?: boolean
  structuredContent?: unknown
}>

export type McpResourceListing = Readonly<{
  uri: string
  name: string
  title?: string
  description?: string
  mimeType?: string
}>

export type McpResourceReadOutcome = Readonly<{
  contents: ReadonlyArray<Readonly<{ uri: string; mimeType: string; text: string }>>
}>

/**
 * The catalog the transport delegates to. Implemented for real in #5993/#5994;
 * `(env, request)` give it the D1 binding + the caller (for grant/tenant).
 */
export type CrmMcpCatalog<Bindings> = Readonly<{
  listTools: (
    env: Bindings,
    request: Request,
    principal: McpPrincipal,
  ) => Promise<ReadonlyArray<McpToolListing>>
  callTool: (
    env: Bindings,
    request: Request,
    principal: McpPrincipal,
    name: string,
    args: unknown,
  ) => Promise<McpToolCallOutcome>
  listResources: (
    env: Bindings,
    request: Request,
    principal: McpPrincipal,
  ) => Promise<ReadonlyArray<McpResourceListing>>
  readResource: (
    env: Bindings,
    request: Request,
    principal: McpPrincipal,
    uri: string,
  ) => Promise<McpResourceReadOutcome>
}>

/** The empty catalog used until the CRM tools/resources land (#5993/#5994). */
export const emptyCrmMcpCatalog = <Bindings>(): CrmMcpCatalog<Bindings> => ({
  callTool: () => Promise.reject(new Error('unknown_tool')),
  listResources: () => Promise.resolve([]),
  listTools: () => Promise.resolve([]),
  readResource: () => Promise.reject(new Error('unknown_resource')),
})

// --- JSON-RPC helpers -------------------------------------------------------

type JsonRpcId = string | number | null

const JSONRPC = '2.0' as const

const rpcResult = (id: JsonRpcId, result: unknown): HttpResponse =>
  noStoreJsonResponse({ id, jsonrpc: JSONRPC, result })

const rpcError = (
  id: JsonRpcId,
  code: number,
  message: string,
  data?: unknown,
): HttpResponse =>
  noStoreJsonResponse({
    error: data === undefined ? { code, message } : { code, data, message },
    id,
    jsonrpc: JSONRPC,
  })

// JSON-RPC standard codes
const PARSE_ERROR = -32700
const INVALID_REQUEST = -32600
const METHOD_NOT_FOUND = -32601
const INVALID_PARAMS = -32602
const INTERNAL_ERROR = -32603

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

type CrmMcpEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmMcpRouteDependencies<Bindings extends CrmMcpEnv> = Readonly<{
  /** Resolve the caller to a principal (admin or scoped grant); null = 401. */
  authenticate: (request: Request, env: Bindings) => Promise<McpPrincipal | null>
  catalog: CrmMcpCatalog<Bindings>
}>

export const makeCrmMcpRoutes = <Bindings extends CrmMcpEnv>(
  dependencies: CrmMcpRouteDependencies<Bindings>,
) => ({
  routeCrmMcpRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    if (url.pathname !== CRM_MCP_PATH) {
      return undefined
    }
    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return Effect.gen(function* () {
      // Transport-level auth → a bound principal (admin or scoped grant).
      const principal = yield* Effect.tryPromise({
        catch: () => null,
        try: () => dependencies.authenticate(request, env),
      })
      if (principal === null) {
        // 401 at the transport boundary, matching the contract's needs_auth status.
        return noStoreJsonResponse(
          { error: { code: INVALID_REQUEST, message: 'unauthorized' }, id: null, jsonrpc: JSONRPC },
          { status: 401 },
        )
      }

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
      // Notifications carry no id and expect no response body.
      const isNotification = body.id === undefined || body.id === null

      switch (method) {
        case 'initialize':
          return rpcResult(id, {
            capabilities: { resources: {}, tools: {} },
            protocolVersion: CRM_MCP_PROTOCOL_VERSION,
            serverInfo: CRM_MCP_SERVER_INFO,
          })
        case 'ping':
          return rpcResult(id, {})
        case 'notifications/initialized':
        case 'notifications/cancelled':
          // Acknowledge notifications with 202 + no body.
          return new Response(null, { status: 202 })
        case 'tools/list':
          return yield* Effect.tryPromise({
            catch: adapterError,
            try: () => dependencies.catalog.listTools(env, request, principal),
          }).pipe(
            Effect.map(tools => rpcResult(id, { tools })),
            Effect.catch(() => Effect.succeed(rpcError(id, INTERNAL_ERROR, 'tools/list failed'))),
          )
        case 'tools/call': {
          const name = typeof params.name === 'string' ? params.name : ''
          if (name === '') {
            return rpcError(id, INVALID_PARAMS, 'tools/call requires a tool name')
          }
          const args = isRecord(params.arguments) ? params.arguments : {}
          return yield* Effect.tryPromise({
            catch: adapterError,
            try: () => dependencies.catalog.callTool(env, request, principal, name, args),
          }).pipe(
            Effect.map(outcome => rpcResult(id, outcome)),
            Effect.catch(error =>
              Effect.succeed(
                rpcResult(id, {
                  content: [
                    {
                      text: error.unknownTarget
                        ? `Unknown tool: ${name}`
                        : `Tool error: ${error.message}`,
                      type: 'text',
                    },
                  ],
                  isError: true,
                } satisfies McpToolCallOutcome),
              ),
            ),
          )
        }
        case 'resources/list':
          return yield* Effect.tryPromise({
            catch: adapterError,
            try: () => dependencies.catalog.listResources(env, request, principal),
          }).pipe(
            Effect.map(resources => rpcResult(id, { resources })),
            Effect.catch(() => Effect.succeed(rpcError(id, INTERNAL_ERROR, 'resources/list failed'))),
          )
        case 'resources/read': {
          const uri = typeof params.uri === 'string' ? params.uri : ''
          if (uri === '') {
            return rpcError(id, INVALID_PARAMS, 'resources/read requires a uri')
          }
          return yield* Effect.tryPromise({
            catch: adapterError,
            try: () => dependencies.catalog.readResource(env, request, principal, uri),
          }).pipe(
            Effect.map(outcome => rpcResult(id, outcome)),
            Effect.catch(error =>
              Effect.succeed(
                rpcError(
                  id,
                  INVALID_PARAMS,
                  error.unknownTarget ? `Unknown resource: ${uri}` : `Resource error: ${error.message}`,
                ),
              ),
            ),
          )
        }
        default:
          return isNotification
            ? new Response(null, { status: 202 })
            : rpcError(id, METHOD_NOT_FOUND, `method not found: ${method}`)
      }
    }).pipe(
      Effect.catch(() =>
        Effect.succeed(rpcError(null, INTERNAL_ERROR, 'internal MCP error')),
      ),
    )
  },
})
