/**
 * Dual-channel batch send route (epic #5980, sub-issue #5988).
 *
 *   POST /api/operator/crm/send-batch
 *     { contactIds: string[], channel: 'gmail_gws'|'resend', templateSlug,
 *       dryRun?: boolean (DEFAULT true), tenant?, sendReason? }
 *     -> CrmBatchSummary (dispositions per contact + counts)
 *
 * DEFAULTS TO dry-run: you must pass `dryRun:false` to actually send. Resend
 * deps are injected via the central config resolver.
 */
// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import { makeCrmEmailDatabaseForEnv } from './crm-email-domain-store'
import { Effect } from 'effect'

import { type CrmBatchSummary, runCrmBatch } from './crm-batch'
import { CrmEmailError } from './crm-email'
import { type CrmResendDeps } from './crm-resend'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { isRecord, stringArrayFromUnknown } from './json-boundary'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmBatchEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmBatchRouteDependencies<Bindings extends CrmBatchEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  resolveResendDeps: (env: Bindings) => CrmResendDeps
}>

const BATCH = /^\/api\/operator\/crm\/send-batch$/

const MAX_BATCH = 500

export const makeCrmBatchRoutes = <Bindings extends CrmBatchEnv>(
  dependencies: CrmBatchRouteDependencies<Bindings>,
) => ({
  routeCrmBatchRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    if (!BATCH.test(url.pathname)) {
      return undefined
    }
    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

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
        try: () => request.json() as Promise<unknown>,
      })
      if (!isRecord(body)) {
        return noStoreJsonResponse({ error: 'bad_request', reason: 'json body required' }, { status: 400 })
      }
      const contactIds = stringArrayFromUnknown(body.contactIds).filter(id => id.trim() !== '')
      const templateSlug = typeof body.templateSlug === 'string' ? body.templateSlug.trim() : ''
      if (contactIds.length === 0 || templateSlug === '') {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: 'contactIds[] and templateSlug required' },
          { status: 400 },
        )
      }
      if (contactIds.length > MAX_BATCH) {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: `at most ${MAX_BATCH} contacts per batch` },
          { status: 400 },
        )
      }
      const channel = body.channel === 'resend' ? 'resend' : 'gmail_gws'
      // Safety: dry-run UNLESS the caller explicitly opts out.
      const dryRun = body.dryRun !== false
      const tenant =
        typeof body.tenant === 'string' && body.tenant.trim() !== ''
          ? body.tenant.trim()
          : DEFAULT_CRM_TENANT_REF

      const summary: CrmBatchSummary = yield* Effect.tryPromise({
        catch: error =>
          error instanceof CrmEmailError
            ? error
            : new CrmEmailError({ reason: `crm.batch: ${String(error)}` }),
        try: () =>
          runCrmBatch(makeCrmEmailDatabaseForEnv(env), { resend: dependencies.resolveResendDeps(env) }, {
            channel,
            contactIds,
            dryRun,
            sendReason: typeof body.sendReason === 'string' ? body.sendReason : null,
            templateSlug,
            tenantRef: tenant,
          }),
      })

      return noStoreJsonResponse({ summary })
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof CrmEmailError
            ? noStoreJsonResponse({ error: 'crm_email_error', reason: error.reason }, { status: 422 })
            : noStoreJsonResponse({ error: 'crm_batch_error' }, { status: 500 }),
        ),
      ),
    )
  },
})
