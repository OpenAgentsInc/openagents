/**
 * Approval-gated CRM send_email command routes (epic #5980, sub-issue #5986).
 *
 *   POST /api/operator/crm/contacts/:id/commands/send-email   propose (chat/agent)
 *        { channel, templateSlug, sendReason?, proposedByRef? }
 *   GET  /api/operator/crm/commands?status=proposed           list (approval UI)
 *   POST /api/operator/crm/commands/:id/approve               approve + execute
 *   POST /api/operator/crm/commands/:id/reject                reject
 *
 * The chat UI / agent program calls the propose endpoint to raise a
 * Blueprint-style `send_email` proposal; an operator approves, and only then is
 * the unified `dispatchCrmSend` run over the chosen channel. Resend deps are
 * injected via the central config resolver (zero-debt rule).
 */
// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import {
  type CrmEmailDatabase,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { Effect } from 'effect'

import {
  approveAndExecuteCrmSendCommand,
  CrmCommandError,
  listCrmCommands,
  proposeCrmSendCommand,
  rejectCrmCommand,
} from './crm-command'
import { type CrmResendDeps } from './crm-resend'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmCommandEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmCommandRouteDependencies<Bindings extends CrmCommandEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  resolveResendDeps: (env: Bindings) => CrmResendDeps
}>

const PROPOSE = /^\/api\/operator\/crm\/contacts\/([^/]+)\/commands\/send-email$/
const COMMANDS = /^\/api\/operator\/crm\/commands$/
const APPROVE = /^\/api\/operator\/crm\/commands\/([^/]+)\/approve$/
const REJECT = /^\/api\/operator\/crm\/commands\/([^/]+)\/reject$/

const ALL: ReadonlyArray<RegExp> = [PROPOSE, COMMANDS, APPROVE, REJECT]

const tenantOf = (url: URL, bodyTenant?: unknown): string => {
  if (typeof bodyTenant === 'string' && bodyTenant.trim() !== '') return bodyTenant.trim()
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

const captured = (pattern: RegExp, path: string): string | null => {
  const match = pattern.exec(path)
  return match === null ? null : decodeURIComponent(match[1] ?? '')
}

export const makeCrmCommandRoutes = <Bindings extends CrmCommandEnv>(
  dependencies: CrmCommandRouteDependencies<Bindings>,
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
            : new CrmCommandError({ reason: `crm.command: ${String(error)}` }),
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
            : noStoreJsonResponse({ error: 'crm_command_error' }, { status: 500 }),
        ),
      ),
    )

  return {
    routeCrmCommandRequest: (
      request: Request,
      env: Bindings,
      _ctx?: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname
      if (!ALL.some(pattern => pattern.test(path))) {
        return undefined
      }

      // Propose a send_email command (chat / agent)
      const proposeContactId = captured(PROPOSE, path)
      if (proposeContactId !== null) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }
        return guard(request, env, async db => {
          const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
          const templateSlug =
            body !== null && typeof body.templateSlug === 'string' ? body.templateSlug.trim() : ''
          if (templateSlug === '') {
            return noStoreJsonResponse(
              { error: 'bad_request', reason: 'templateSlug required' },
              { status: 400 },
            )
          }
          const channel = body !== null && body.channel === 'resend' ? 'resend' : 'gmail_gws'
          const command = await proposeCrmSendCommand(db, {
            channel,
            contactId: proposeContactId,
            proposedByRef:
              body !== null && typeof body.proposedByRef === 'string' ? body.proposedByRef : null,
            sendReason:
              body !== null && typeof body.sendReason === 'string' ? body.sendReason : null,
            templateSlug,
            tenantRef: tenantOf(url, body?.tenant),
          })
          return noStoreJsonResponse({ command }, { status: 201 })
        })
      }

      // List commands (approval UI)
      if (COMMANDS.test(path)) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        return guard(request, env, async db =>
          noStoreJsonResponse({
            commands: await listCrmCommands(db, tenantOf(url), {
              status: url.searchParams.get('status') ?? undefined,
            }),
          }),
        )
      }

      // Approve + execute
      const approveId = captured(APPROVE, path)
      if (approveId !== null) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }
        return guard(request, env, async db => {
          const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
          const result = await approveAndExecuteCrmSendCommand(
            db,
            { resend: dependencies.resolveResendDeps(env) },
            {
              approvedByRef:
                body !== null && typeof body.approvedByRef === 'string'
                  ? body.approvedByRef
                  : null,
              commandId: approveId,
              tenantRef: tenantOf(url, body?.tenant),
            },
          )
          if (result.kind === 'not_found') {
            return noStoreJsonResponse({ error: 'not_found', resource: 'command' }, { status: 404 })
          }
          if (result.kind === 'not_pending') {
            return noStoreJsonResponse({ error: 'not_pending', result }, { status: 409 })
          }
          return noStoreJsonResponse({ result })
        })
      }

      // Reject
      const rejectId = captured(REJECT, path)
      if (rejectId !== null) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }
        return guard(request, env, async db => {
          const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
          const result = await rejectCrmCommand(db, {
            commandId: rejectId,
            reason: body !== null && typeof body.reason === 'string' ? body.reason : null,
            tenantRef: tenantOf(url, body?.tenant),
          })
          if (result.kind === 'not_found') {
            return noStoreJsonResponse({ error: 'not_found', resource: 'command' }, { status: 404 })
          }
          return noStoreJsonResponse({ result })
        })
      }

      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    },
  }
}
