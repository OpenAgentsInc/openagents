/**
 * OB-4 (#8561): Sarah reply routing routes — CRM-side plumbing.
 * OB-5 (#8562): the inbound route also issues a Sarah handoff link and
 * advances the reply's sales opportunity to the `replied` stage (§ below).
 *
 *   POST /api/operator/crm/replies/inbound
 *     { fromEmail, subject?, bodyText?, inReplyToRef?, provider?,
 *       providerEventId?, sourceRef? }
 *     -> { result: CrmReplyEventResult,
 *          sarahHandoff?: { url, handoffToken },
 *          opportunity?: { id, stage } }
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
 *
 * OB-5 handoff: when a reply matches an existing CRM contact and does NOT
 * read as an opt-out, this route (1) mints a Sarah handoff link
 * (crm-sarah-handoff.ts) so the response — and any future email-reply
 * continuation Sarah's own channel sends — can carry a personal link back
 * into sarah.openagents.com that keeps the prospect_ref/CRM-contact context,
 * and (2) finds (or opens) the contact's open sales opportunity and advances
 * its `crm_opportunities.stage` to `replied` (never regressing a deal that
 * has already moved further — see `advanceCrmOpportunityStage`). Both are
 * best-effort: neither failure ever fails the reply-recording request (the
 * reply and any opt-out suppression are already durable by the time this
 * runs).
 */
import { Effect } from 'effect'

import { decodeBusinessSourceRef } from './business-source-attribution'
import {
  crmEmailAuthorityDb,
  type CrmEmailDatabase,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { CrmCommandError } from './crm-command'
import { listCrmReplyEvents, recordCrmReplyEvent } from './crm-reply'
import {
  CRM_SARAH_HANDOFF_DEFAULT_BASE_URL,
  makeD1CrmSarahHandoffStore,
} from './crm-sarah-handoff'
import {
  advanceCrmOpportunityStage,
  createCrmOpportunity,
  DEFAULT_CRM_TENANT_REF,
  findOpenCrmOpportunityForContact,
  upsertCrmOpportunityContactRole,
} from './crm-store'
import { isRecord } from './json-boundary'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmReplyEnv = Readonly<{
  OPENAGENTS_DB: D1Database
  /** Base URL for prospect-facing Sarah handoff links. Defaults to the
   * production `sarah.openagents.com` app (CRM_SARAH_HANDOFF_DEFAULT_BASE_URL)
   * when unset — useful for staging/local without requiring config. */
  SARAH_APP_URL?: string | undefined
}>

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
          const tenantRef = tenantOf(url, body.tenant)
          const result = await recordCrmReplyEvent(db, {
            bodyText: typeof body.bodyText === 'string' ? body.bodyText : null,
            fromEmail,
            inReplyToRef: typeof body.inReplyToRef === 'string' ? body.inReplyToRef : null,
            ...(providerValue !== undefined ? { provider: providerValue } : {}),
            providerEventId:
              typeof body.providerEventId === 'string' ? body.providerEventId : null,
            subject: typeof body.subject === 'string' ? body.subject : null,
            tenantRef,
          })

          // OB-5 (#8562): best-effort handoff-link issuance + opportunity
          // stage advance. Neither ever fails the reply-recording request —
          // the reply row (and any opt-out suppression) is already durable
          // above.
          let sarahHandoff: Readonly<{ url: string; handoffToken: string }> | undefined
          let opportunity: Readonly<{ id: string; stage: string }> | undefined
          if (result.contactId !== null && !result.optOut) {
            const sourceRefDecode = decodeBusinessSourceRef(body.sourceRef)
            const sourceRef =
              'sourceRef' in sourceRefDecode ? sourceRefDecode.sourceRef : undefined

            try {
              const handoffStore = makeD1CrmSarahHandoffStore(
                crmEmailAuthorityDb(db),
              )
              const issued = await handoffStore.issueHandoffLink(
                {
                  tenantRef,
                  contactId: result.contactId,
                  replyEventId: result.replyEventId,
                  ...(sourceRef !== undefined ? { sourceRef } : {}),
                },
                env.SARAH_APP_URL ?? CRM_SARAH_HANDOFF_DEFAULT_BASE_URL,
              )
              sarahHandoff = { url: issued.url, handoffToken: issued.handoffToken }
            } catch {
              // Swallow: handoff-link issuance is non-authoritative for reply
              // recording; buildSarahHandoffUrl/issueHandoffLink failures
              // (e.g. token collision exhaustion) never block the reply.
            }

            try {
              // Advance (or open) the OB-5 sales-stage funnel: the deal a
              // reply belongs to should read "replied" from here on, without
              // ever regressing a deal that has already moved further
              // (quoted/closed) — `advanceCrmOpportunityStage` enforces that.
              const existingOpportunity = await findOpenCrmOpportunityForContact(
                db,
                tenantRef,
                result.contactId,
              )
              const targetOpportunity =
                existingOpportunity ??
                (await createCrmOpportunity(db, {
                  tenantRef,
                  name: `${fromEmail} — inbound reply`,
                  stage: 'sourced',
                  ...(sourceRef !== undefined
                    ? { metadata: { sourceRef, contactId: result.contactId } }
                    : { metadata: { contactId: result.contactId } }),
                }))
              await upsertCrmOpportunityContactRole(db, {
                tenantRef,
                opportunityId: targetOpportunity.id,
                contactId: result.contactId,
              })
              const advanced = await advanceCrmOpportunityStage(db, {
                tenantRef,
                id: targetOpportunity.id,
                stage: 'replied',
              })
              opportunity = { id: advanced.id, stage: advanced.stage }
            } catch {
              // Swallow: the funnel-stage advance is non-authoritative for
              // reply recording.
            }
          }

          return noStoreJsonResponse(
            {
              result,
              ...(sarahHandoff !== undefined ? { sarahHandoff } : {}),
              ...(opportunity !== undefined ? { opportunity } : {}),
            },
            { status: 201 },
          )
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
