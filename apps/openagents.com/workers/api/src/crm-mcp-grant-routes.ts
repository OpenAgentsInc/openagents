/**
 * Admin routes to mint / list / revoke scoped CRM MCP grants (epic #5991, #5995).
 *
 *   POST   /api/operator/crm/mcp-grants   { tenant?, authorities[], label?, expiresAt? }
 *          -> { grant: summary, token }   (token shown ONCE)
 *   GET    /api/operator/crm/mcp-grants?tenant=   -> { grants: summary[] }
 *   DELETE /api/operator/crm/mcp-grants/:grantRef -> { revoked: boolean }
 *
 * Admin-gated. The minted token authenticates an MCP client at `POST /api/mcp`
 * with exactly the declared authorities + bound tenant.
 */
import { Effect, Schema as S } from 'effect'

import {
  CRM_MCP_AUTHORITY_CLASSES,
  listCrmMcpGrants,
  mintCrmMcpGrant,
  revokeCrmMcpGrant,
} from './crm-mcp-grant'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { isRecord, stringArrayFromUnknown } from './json-boundary'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'

type HttpResponse = globalThis.Response

class CrmMcpGrantRouteError extends S.TaggedErrorClass<CrmMcpGrantRouteError>()(
  'CrmMcpGrantRouteError',
  { message: S.String },
) {}

type CrmMcpGrantEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmMcpGrantRouteDependencies<Bindings extends CrmMcpGrantEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const COLLECTION = /^\/api\/operator\/crm\/mcp-grants$/
const ITEM = /^\/api\/operator\/crm\/mcp-grants\/([^/]+)$/

const tenantOf = (url: URL, bodyTenant?: unknown): string => {
  if (typeof bodyTenant === 'string' && bodyTenant.trim() !== '') return bodyTenant.trim()
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

export const makeCrmMcpGrantRoutes = <Bindings extends CrmMcpGrantEnv>(
  dependencies: CrmMcpGrantRouteDependencies<Bindings>,
) => {
  const guard = (
    request: Request,
    env: Bindings,
    body: (db: D1Database) => Promise<HttpResponse>,
  ): Effect.Effect<HttpResponse> =>
    Effect.gen(function* () {
      const authorized = yield* Effect.tryPromise({
        catch: () => false as const,
        try: () => dependencies.requireAdminApiToken(request, env),
      })
      if (!authorized) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }
      return yield* Effect.tryPromise({
        catch: error =>
          new CrmMcpGrantRouteError({
            message: error instanceof Error ? error.message : String(error),
          }),
        try: () => body(openAgentsDatabase(env)),
      })
    }).pipe(
      Effect.catch(() =>
        Effect.succeed(noStoreJsonResponse({ error: 'crm_mcp_grant_error' }, { status: 500 })),
      ),
    )

  return {
    routeCrmMcpGrantRequest: (
      request: Request,
      env: Bindings,
      _ctx?: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname

      if (COLLECTION.test(path)) {
        if (request.method === 'GET') {
          return guard(request, env, async db =>
            noStoreJsonResponse({ grants: await listCrmMcpGrants(db, tenantOf(url)) }),
          )
        }
        if (request.method === 'POST') {
          return guard(request, env, async db => {
            const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
            if (!isRecord(body)) {
              return noStoreJsonResponse({ error: 'bad_request', reason: 'json body required' }, { status: 400 })
            }
            const authorities = stringArrayFromUnknown(body.authorities)
            const valid = authorities.filter(a => (CRM_MCP_AUTHORITY_CLASSES as ReadonlyArray<string>).includes(a))
            if (valid.length === 0) {
              return noStoreJsonResponse(
                { error: 'bad_request', reason: `authorities[] required (one of ${CRM_MCP_AUTHORITY_CLASSES.join(', ')})` },
                { status: 400 },
              )
            }
            const minted = await mintCrmMcpGrant(db, {
              authorities: valid,
              expiresAt: typeof body.expiresAt === 'string' ? body.expiresAt : null,
              label: typeof body.label === 'string' ? body.label : null,
              tenantRef: tenantOf(url, body.tenant),
            })
            return noStoreJsonResponse({ grant: minted.summary, token: minted.token }, { status: 201 })
          })
        }
        return Effect.succeed(methodNotAllowed(['GET', 'POST']))
      }

      const item = ITEM.exec(path)
      if (item !== null) {
        if (request.method !== 'DELETE') {
          return Effect.succeed(methodNotAllowed(['DELETE']))
        }
        const grantRef = decodeURIComponent(item[1] ?? '')
        return guard(request, env, async db =>
          noStoreJsonResponse({ revoked: await revokeCrmMcpGrant(db, tenantOf(url), grantRef) }),
        )
      }

      return undefined
    },
  }
}
