/**
 * Public MCP discovery doc (epic #5991, sub-issue #5998).
 *
 *   GET /.well-known/openagents-mcp.json   (public, refs-only)
 *
 * Advertises the CRM MCP server: schema/protocol version, transport + endpoint,
 * the auth model, and a PUBLIC-SAFE tool/resource catalog (names, titles,
 * required authority classes, risk class — NO data, NO secrets). Clients use
 * this to discover the endpoint; actual `tools/list` is grant-filtered per
 * caller at `POST /api/mcp`.
 */
import { Effect } from 'effect'

import { OPENAGENTS_MCP_SCHEMA_VERSION } from '@openagentsinc/mcp-contract'

import { CRM_MCP_RESOURCES, CRM_MCP_TOOLS } from './crm-mcp'
import { CRM_MCP_PATH, CRM_MCP_PROTOCOL_VERSION, CRM_MCP_SERVER_INFO } from './crm-mcp-routes'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

const DISCOVERY_PATH = '/.well-known/openagents-mcp.json'

const SOURCE = 'docs/mcp/2026-06-22-crm-mcp-server-phase-1-audit.md'

const discoveryDocument = () => ({
  authority: {
    model: 'admin_token_or_scoped_grant',
    note:
      'Authenticate at the endpoint with an admin Bearer token (full CRM authority on the X-OpenAgents-Tenant / default tenant) or a scoped MCP grant token (its declared authority classes + bound tenant). tools/list is filtered by the caller grant; ungranted tools are absent.',
    tenantHeader: 'X-OpenAgents-Tenant',
  },
  resources: CRM_MCP_RESOURCES.map(resource => ({
    description: resource.description,
    name: resource.name,
    uri: resource.uri,
  })),
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  server: CRM_MCP_SERVER_INFO,
  sourceRefs: [SOURCE],
  tools: CRM_MCP_TOOLS.map(tool => ({
    name: tool.name,
    requiredAuthorities: tool.requiredAuthorities,
    riskClass: tool.riskClass,
    summary: tool.publicSummary,
    title: tool.title,
  })),
  transport: {
    endpoint: CRM_MCP_PATH,
    kind: 'streamable_http',
    protocolVersion: CRM_MCP_PROTOCOL_VERSION,
  },
})

export const makeCrmMcpDiscoveryRoutes = () => ({
  routeCrmMcpDiscoveryRequest: (
    request: Request,
    _env?: unknown,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    if (url.pathname !== DISCOVERY_PATH) {
      return undefined
    }
    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }
    return Effect.succeed(noStoreJsonResponse(discoveryDocument()))
  },
})
