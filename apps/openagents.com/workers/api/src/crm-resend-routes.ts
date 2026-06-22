/**
 * Admin-gated CRM Resend send route (epic #5980, sub-issue #5984).
 *
 *   POST /api/operator/crm/contacts/:id/resend-send
 *     { templateSlug, tenant?, sendReason? }
 *
 * Builds the Resend deps from env: armed only when `CRM_RESEND_SEND_ENABLED` is
 * truthy AND `RESEND_API_KEY` + `RESEND_FROM_EMAIL` are present. Disabled =>
 * dry-run (never calls Resend). Suppressed => 409. Provider failure => 502.
 */
import { Effect } from 'effect'

import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import {
  CrmEmailError,
  type CrmResendDeps,
  sendCrmEmailViaResend,
} from './crm-resend'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'

type HttpResponse = globalThis.Response

type CrmResendEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

/**
 * `resolveResendDeps` is injected by the coordinator (index.ts), which reads the
 * Resend config through the central `OpenAgentsWorkerConfig` resolver (the
 * zero-debt rule bans direct `env.RESEND_*` reads in business modules) and the
 * `CRM_RESEND_SEND_ENABLED` arming flag.
 */
type CrmResendRouteDependencies<Bindings extends CrmResendEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  resolveResendDeps: (env: Bindings) => CrmResendDeps
}>

const SEND = /^\/api\/operator\/crm\/contacts\/([^/]+)\/resend-send$/

export const makeCrmResendRoutes = <Bindings extends CrmResendEnv>(
  dependencies: CrmResendRouteDependencies<Bindings>,
) => ({
  routeCrmResendRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const match = SEND.exec(url.pathname)
    if (match === null) {
      return undefined
    }
    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }
    const contactId = decodeURIComponent(match[1] ?? '')
    const tenantParam = url.searchParams.get('tenant')

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
      const tenant =
        body !== null && typeof body.tenant === 'string' && body.tenant.trim() !== ''
          ? body.tenant.trim()
          : tenantParam !== null && tenantParam.trim() !== ''
            ? tenantParam.trim()
            : DEFAULT_CRM_TENANT_REF

      const result = yield* Effect.tryPromise({
        catch: error =>
          error instanceof CrmEmailError
            ? error
            : new CrmEmailError({ reason: `crm.resend: ${String(error)}` }),
        try: () =>
          sendCrmEmailViaResend(openAgentsDatabase(env), dependencies.resolveResendDeps(env), {
            contactId,
            sendReason: body !== null && typeof body.sendReason === 'string' ? body.sendReason : null,
            templateSlug,
            tenantRef: tenant,
          }),
      })

      const status =
        result.kind === 'suppressed'
          ? 409
          : result.kind === 'failed'
            ? 502
            : 200
      return noStoreJsonResponse({ result }, { status })
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof CrmEmailError
            ? noStoreJsonResponse(
                { error: 'crm_email_error', reason: error.reason },
                { status: 422 },
              )
            : noStoreJsonResponse({ error: 'crm_resend_error' }, { status: 500 }),
        ),
      ),
    )
  },
})
