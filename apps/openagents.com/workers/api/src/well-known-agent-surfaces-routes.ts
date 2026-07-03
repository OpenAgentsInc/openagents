/**
 * Well-known agent-discovery manifests (orank agent-readiness gaps: "ARD
 * catalog" and "MCP server / manifest"). Both paths previously fell through
 * to the SPA shell and returned HTML instead of JSON, so scanners looking at
 * exactly these conventional paths found "something" that was not valid
 * JSON. These routes serve real, valid documents instead.
 *
 *   GET /.well-known/ai-catalog.json   — Agentic Resource Discovery (ARD)
 *                                         catalog, https://agenticresourcediscovery.org/spec/
 *   GET /.well-known/mcp.json          — flat MCP server manifest (the
 *   GET /.well-known/mcp/manifest.json   convention used by agent-readiness
 *                                         scanners; same document at both
 *                                         paths)
 *
 * Both reference the real public MCP server at `PUBLIC_MCP_PATH`
 * (`public-agent-mcp.ts`), not the admin/grant-gated CRM MCP transport.
 */
import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'
import { PUBLIC_MCP_PATH, PUBLIC_MCP_TOOLS } from './public-agent-mcp-routes'

type HttpResponse = globalThis.Response

const ORIGIN = 'https://openagents.com'

export const MCP_MANIFEST_PATHS = ['/.well-known/mcp.json', '/.well-known/mcp/manifest.json'] as const
export const AI_CATALOG_PATH = '/.well-known/ai-catalog.json'

// Mirrors the flat shape used by public MCP-server manifests in the wild
// (name/kind/description/icon/url/transport/capabilities/tools), so scanners
// that expect that convention parse it the same way regardless of client.
const mcpManifestDocument = () => ({
  capabilities: { resources: true, tools: true },
  description:
    'OpenAgents builds public, verifiable AI agents for coding, research, payments, and operational work. This public MCP server exposes read-only developer-resource discovery and the capability manifest; no authentication required.',
  icon: `${ORIGIN}/icon.svg`,
  kind: 'product',
  name: 'openagents',
  tools: PUBLIC_MCP_TOOLS.map(tool => ({
    description: tool.description,
    name: tool.name,
    parameters: {},
    ...(tool.uiResourceUri === undefined ? {} : { ui: tool.uiResourceUri }),
  })),
  transport: 'streamable-http',
  url: `${ORIGIN}${PUBLIC_MCP_PATH}`,
})

const aiCatalogDocument = () => ({
  entries: [
    {
      capabilities: PUBLIC_MCP_TOOLS.map(tool => tool.name),
      description:
        'Public, unauthenticated MCP server exposing OpenAgents developer-resource discovery and the public capability manifest, with an MCP Apps UI card.',
      displayName: 'OpenAgents Public MCP Server',
      identifier: 'urn:air:openagents.com:server:public-mcp',
      representativeQueries: [
        'what developer resources does OpenAgents have',
        'show me the OpenAgents capability manifest',
      ],
      type: 'application/mcp-server-card+json',
      url: `${ORIGIN}${MCP_MANIFEST_PATHS[0]}`,
    },
    {
      description:
        'OpenAI-compatible, pay-per-call LLM inference API (openagents/khala) with receipt-first metering and Machine Payments Protocol (x402) support.',
      displayName: 'Khala Inference API (OpenAI-compatible)',
      identifier: 'urn:air:openagents.com:api:khala-inference',
      representativeQueries: [
        'call an OpenAI-compatible inference API and pay per request',
        'pay for LLM inference with Lightning or USDC',
      ],
      type: 'application/vnd.oai.openapi+json',
      url: `${ORIGIN}/api/openapi.json`,
    },
  ],
  host: {
    displayName: 'OpenAgents',
    documentationUrl: `${ORIGIN}/AGENTS.md`,
    identifier: 'openagents.com',
    logoUrl: `${ORIGIN}/icon.svg`,
  },
  specVersion: '1.0',
})

// Public + cacheable so a scanner/crawler can fetch it cheaply, matching the
// other discovery surfaces (`discovery-surfaces.ts`); no auth, no robots
// block — these are meant to be found.
const renderJsonDocument = (request: Request, document: unknown): Effect.Effect<HttpResponse> =>
  Effect.sync(() => {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return methodNotAllowed(['GET', 'HEAD'])
    }
    const headers = new Headers({
      'cache-control': 'public, max-age=300',
      'content-type': 'application/json',
    })
    return new Response(request.method === 'HEAD' ? null : JSON.stringify(document, null, 2), {
      headers,
      status: 200,
    })
  })

export const routeWellKnownAgentSurfaceRequest = (
  request: Request,
  _env?: unknown,
  _ctx?: ExecutionContext,
): Effect.Effect<HttpResponse> | undefined => {
  const path = new URL(request.url).pathname
  if ((MCP_MANIFEST_PATHS as ReadonlyArray<string>).includes(path)) {
    return renderJsonDocument(request, mcpManifestDocument())
  }
  if (path === AI_CATALOG_PATH) {
    return renderJsonDocument(request, aiCatalogDocument())
  }
  return undefined
}
