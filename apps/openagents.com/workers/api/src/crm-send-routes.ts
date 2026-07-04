/**
 * Unified CRM send route + Gmail executor queue (epic #5980, sub-issue #5985).
 *
 *   POST /api/operator/crm/contacts/:id/send
 *        { channel: 'gmail_gws'|'resend', templateSlug, tenant?, sendReason? }
 *        -> one entry point, shared suppression gate, ledger write for both.
 *   GET  /api/operator/crm/gmail-queue?tenant=&limit=
 *        -> queued gmail_gws messages awaiting the local executor (#5987).
 *
 * Resend deps are injected via the central config resolver (zero-debt rule).
 */
// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import { makeCrmEmailDatabaseForEnv } from './crm-email-domain-store'
import { Effect } from 'effect'

import { listCrmQueuedGmailMessages } from './crm-email'
import { type CrmResendDeps } from './crm-resend'
import { CrmEmailError, dispatchCrmSend } from './crm-send'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmSendEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmSendRouteDependencies<Bindings extends CrmSendEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  resolveResendDeps: (env: Bindings) => CrmResendDeps
}>

const SEND = /^\/api\/operator\/crm\/contacts\/([^/]+)\/send$/
const QUEUE = /^\/api\/operator\/crm\/gmail-queue$/

const tenantOf = (url: URL, bodyTenant?: unknown): string => {
  if (typeof bodyTenant === 'string' && bodyTenant.trim() !== '') return bodyTenant.trim()
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

const statusForOutcome = (
  outcome: Awaited<ReturnType<typeof dispatchCrmSend>>,
): number => {
  if (outcome.channel === 'resend') {
    return outcome.result.kind === 'suppressed'
      ? 409
      : outcome.result.kind === 'failed'
        ? 502
        : 200
  }
  return outcome.kind === 'suppressed' ? 409 : 200
}

export const makeCrmSendRoutes = <Bindings extends CrmSendEnv>(
  dependencies: CrmSendRouteDependencies<Bindings>,
) => ({
  routeCrmSendRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const path = url.pathname

    // Gmail executor queue
    if (QUEUE.test(path)) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }
      const limitParam = url.searchParams.get('limit')
      const limit = limitParam === null ? undefined : Number(limitParam)
      return Effect.gen(function* () {
        const authorized = yield* Effect.tryPromise({
          catch: () => false as const,
          try: () => dependencies.requireAdminApiToken(request, env),
        })
        if (!authorized) {
          return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
        }
        const messages = yield* Effect.tryPromise({
          catch: error =>
            error instanceof CrmEmailError
              ? error
              : new CrmEmailError({ reason: `crm.gmailQueue: ${String(error)}` }),
          try: () =>
            listCrmQueuedGmailMessages(makeCrmEmailDatabaseForEnv(env), tenantOf(url), {
              limit: limit !== undefined && Number.isFinite(limit) ? limit : undefined,
            }),
        })
        return noStoreJsonResponse({ messages })
      }).pipe(
        Effect.catch(() =>
          Effect.succeed(noStoreJsonResponse({ error: 'crm_send_error' }, { status: 500 })),
        ),
      )
    }

    const match = SEND.exec(path)
    if (match === null) {
      return undefined
    }
    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }
    const contactId = decodeURIComponent(match[1] ?? '')

    return Effect.gen(function* () {
      const authorized = yield* Effect.tryPromise({
        catch: () => false as const,
        try: () => dependencies.requireAdminApiToken(request, env),
      })
      if (!authorized) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const body = yield* Effect.tryPromise({
        catch: () => null,
        try: () => request.json() as Promise<Record<string, unknown>>,
      })
      const templateSlug =
        body !== null && typeof body.templateSlug === 'string' ? body.templateSlug.trim() : ''
      if (templateSlug === '') {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: 'templateSlug required' },
          { status: 400 },
        )
      }
      const channel = body !== null && body.channel === 'resend' ? 'resend' : 'gmail_gws'

      const outcome = yield* Effect.tryPromise({
        catch: error =>
          error instanceof CrmEmailError
            ? error
            : new CrmEmailError({ reason: `crm.send: ${String(error)}` }),
        try: () =>
          dispatchCrmSend(
            makeCrmEmailDatabaseForEnv(env),
            { resend: dependencies.resolveResendDeps(env) },
            {
              channel,
              contactId,
              sendReason:
                body !== null && typeof body.sendReason === 'string' ? body.sendReason : null,
              templateSlug,
              tenantRef: tenantOf(url, body?.tenant),
            },
          ),
      })

      return noStoreJsonResponse({ outcome }, { status: statusForOutcome(outcome) })
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof CrmEmailError
            ? noStoreJsonResponse(
                { error: 'crm_email_error', reason: error.reason },
                { status: 422 },
              )
            : noStoreJsonResponse({ error: 'crm_send_error' }, { status: 500 }),
        ),
      ),
    )
  },
})
