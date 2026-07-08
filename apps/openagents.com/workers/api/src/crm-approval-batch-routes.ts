/**
 * OB-4 (#8561): batch approval UX routes over the existing CRM approval queue.
 *
 *   GET  /api/operator/crm/commands/batch-queue?status=proposed
 *     -> { queue: CrmApprovalQueueView } grouped by day + segment, for the
 *        batch-review screen (read/edit/approve-or-reject a day's drafts).
 *
 *   POST /api/operator/crm/commands/batch-approve
 *     { commandIds: string[], approvedByRef?, dailyCap? }
 *     -> { result: CrmBatchApproveResult } — approves + executes each
 *        command individually (unchanged per-send gate + per-send receipt),
 *        records one rollup batch receipt, and stops sending once the daily
 *        cap is hit (remaining items come back `capped`, never sent).
 *
 * Authority is unchanged: nothing here sends without an explicit operator
 * approval decision, and every send still runs through the same
 * `dispatchCrmSend` unified channel as the single-item approval path.
 */
import { Effect } from 'effect'

import {
  batchApproveCrmSendCommands,
  listCrmApprovalQueue,
} from './crm-approval-batch'
import {
  type CrmEmailDatabase,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { CrmCommandError } from './crm-command'
import { type CrmResendDeps } from './crm-resend'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { isRecord, stringArrayFromUnknown } from './json-boundary'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmApprovalBatchEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmApprovalBatchRouteDependencies<Bindings extends CrmApprovalBatchEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  resolveResendDeps: (env: Bindings) => CrmResendDeps
}>

const BATCH_QUEUE = /^\/api\/operator\/crm\/commands\/batch-queue$/
const BATCH_APPROVE = /^\/api\/operator\/crm\/commands\/batch-approve$/

const ALL: ReadonlyArray<RegExp> = [BATCH_QUEUE, BATCH_APPROVE]

const tenantOf = (url: URL, bodyTenant?: unknown): string => {
  if (typeof bodyTenant === 'string' && bodyTenant.trim() !== '') return bodyTenant.trim()
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

export const makeCrmApprovalBatchRoutes = <Bindings extends CrmApprovalBatchEnv>(
  dependencies: CrmApprovalBatchRouteDependencies<Bindings>,
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
            : new CrmCommandError({ reason: `crm.approvalBatch: ${String(error)}` }),
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
            : noStoreJsonResponse({ error: 'crm_approval_batch_error' }, { status: 500 }),
        ),
      ),
    )

  return {
    routeCrmApprovalBatchRequest: (
      request: Request,
      env: Bindings,
      _ctx?: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname
      if (!ALL.some(pattern => pattern.test(path))) {
        return undefined
      }

      // Batch queue view (grouped by day + segment)
      if (BATCH_QUEUE.test(path)) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        return guard(request, env, async db => {
          const limitParam = url.searchParams.get('limit')
          const limit = limitParam === null ? undefined : Number(limitParam)
          const queue = await listCrmApprovalQueue(db, tenantOf(url), {
            limit: limit !== undefined && Number.isFinite(limit) ? limit : undefined,
            status: url.searchParams.get('status') ?? undefined,
          })
          return noStoreJsonResponse({ queue })
        })
      }

      // Batch approve (one-tap operator action; per-send receipts unchanged)
      if (BATCH_APPROVE.test(path)) {
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
          const commandIds = stringArrayFromUnknown(body.commandIds).filter(id => id.trim() !== '')
          if (commandIds.length === 0) {
            return noStoreJsonResponse(
              { error: 'bad_request', reason: 'commandIds[] required' },
              { status: 400 },
            )
          }
          const dailyCapRaw = body.dailyCap
          const dailyCap =
            typeof dailyCapRaw === 'number' && Number.isFinite(dailyCapRaw) ? dailyCapRaw : null

          const result = await batchApproveCrmSendCommands(
            db,
            { resend: dependencies.resolveResendDeps(env) },
            {
              approvedByRef:
                typeof body.approvedByRef === 'string' ? body.approvedByRef : null,
              commandIds,
              dailyCap,
              tenantRef: tenantOf(url, body.tenant),
            },
          )
          return noStoreJsonResponse({ result })
        })
      }

      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    },
  }
}
