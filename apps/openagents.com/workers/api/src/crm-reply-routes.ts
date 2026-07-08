/**
 * OB-4 (#8561): Sarah reply routing routes — CRM-side plumbing.
 *
 *   POST /api/operator/crm/replies/inbound
 *     { fromEmail, subject?, bodyText?, inReplyToRef?, provider?,
 *       providerEventId? }
 *     -> { result: CrmReplyEventResult }
 *
 *   GET  /api/operator/crm/replies?contactId=&limit=
 *     -> { replies: CrmReplyEvent[] }
 *
 * The inbound route is the v0 fallback named in the issue: until the Sarah
 * repo's S-8 email channel (or a verified inbound-email provider webhook)
 * calls this directly, it is guarded by the same operator admin bearer token
 * as the rest of the CRM operator surface. When S-8 or a real inbound
 * provider is wired, that integration should bring its OWN signature/secret
 * verification (matching the `resend-webhooks.ts` svix pattern) rather than
 * sharing the operator token with an external system.
 */
import { Effect } from 'effect'

import {
  type CrmEmailDatabase,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { CrmCommandError } from './crm-command'
import { listCrmReplyEvents, recordCrmReplyEvent } from './crm-reply'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { isRecord } from './json-boundary'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmReplyEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmReplyRouteDependencies<Bindings extends CrmReplyEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const INBOUND = /^\/api\/operator\/crm\/replies\/inbound$/
const LIST = /^\/api\/operator\/crm\/replies$/

const ALL: ReadonlyArray<RegExp> = [INBOUND, LIST]

const tenantOf = (url: URL, bodyTenant?: unknown): string => {
  if (typeof bodyTenant === 'string' && bodyTenant.trim() !== '') return bodyTenant.trim()
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

export const makeCrmReplyRoutes = <Bindings extends CrmReplyEnv>(
  dependencies: CrmReplyRouteDependencies<Bindings>,
) => {
  const guard = (
    request: Request,
    env: Bindings,
    body: (db: CrmEmailDatabase) => Promise<HttpResponse>,
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
          error instanceof CrmCommandError
            ? error
            : new CrmCommandError({ reason: `crm.reply: ${String(error)}` }),
        try: () => body(makeCrmEmailDatabaseForEnv(env)),
      })
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof CrmCommandError
            ? noStoreJsonResponse(
                { error: 'crm_command_error', reason: error.reason },
                { status: 422 },
              )
            : noStoreJsonResponse({ error: 'crm_reply_error' }, { status: 500 }),
        ),
      ),
    )

  return {
    routeCrmReplyRequest: (
      request: Request,
      env: Bindings,
      _ctx?: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname
      if (!ALL.some(pattern => pattern.test(path))) {
        return undefined
      }

      if (INBOUND.test(path)) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }
        return guard(request, env, async db => {
          const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
          if (!isRecord(body)) {
            return noStoreJsonResponse(
              { error: 'bad_request', reason: 'json body required' },
              { status: 400 },
            )
          }
          const fromEmail = typeof body.fromEmail === 'string' ? body.fromEmail.trim() : ''
          if (fromEmail === '') {
            return noStoreJsonResponse(
              { error: 'bad_request', reason: 'fromEmail required' },
              { status: 400 },
            )
          }
          const providerValue =
            typeof body.provider === 'string' && body.provider.trim() !== ''
              ? body.provider.trim()
              : undefined
          const result = await recordCrmReplyEvent(db, {
            bodyText: typeof body.bodyText === 'string' ? body.bodyText : null,
            fromEmail,
            inReplyToRef: typeof body.inReplyToRef === 'string' ? body.inReplyToRef : null,
            ...(providerValue !== undefined ? { provider: providerValue } : {}),
            providerEventId:
              typeof body.providerEventId === 'string' ? body.providerEventId : null,
            subject: typeof body.subject === 'string' ? body.subject : null,
            tenantRef: tenantOf(url, body.tenant),
          })
          return noStoreJsonResponse({ result }, { status: 201 })
        })
      }

      if (LIST.test(path)) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        return guard(request, env, async db => {
          const limitParam = url.searchParams.get('limit')
          const limit = limitParam === null ? undefined : Number(limitParam)
          const replies = await listCrmReplyEvents(db, tenantOf(url), {
            contactId: url.searchParams.get('contactId'),
            limit: limit !== undefined && Number.isFinite(limit) ? limit : undefined,
          })
          return noStoreJsonResponse({ replies })
        })
      }

      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    },
  }
}
