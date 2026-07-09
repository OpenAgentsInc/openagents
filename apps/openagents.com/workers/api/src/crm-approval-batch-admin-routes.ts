/**
 * OB-4 (#8561): admin/OpenAuth-gated surface for the CRM batch approval
 * queue so the Aiur ops console can list + batch-approve pending drafts.
 *
 * This is a thin owner-gated HTTP adapter over the EXISTING batch domain
 * in `crm-approval-batch.ts`. It does NOT introduce new send authority:
 *
 *   - GET  /api/admin/ops/crm/batch-queue
 *       -> { ok: true, queue } via listCrmApprovalQueue
 *   - POST /api/admin/ops/crm/batch-approve
 *       -> { ok: true, result } via batchApproveCrmSendCommands
 *
 * INVARIANT (lead_gen_agent.no_send_without_approval_receipt.v1): preserved.
 * Batch approve is UX only. `batchApproveCrmSendCommands` still walks each
 * command id through the unchanged `approveAndExecuteCrmSendCommand` path
 * one-by-one (one real approval + one real per-send receipt per row), then
 * writes one optional `crm_command_batches` rollup. Nothing here bypasses
 * that gate or invents a bulk-send primitive.
 *
 * Auth composition matches AIUR-3 / OB-6 admin ops routes:
 * `requireAdminCaller` (signed-in OpenAuth session + admin-email allowlist),
 * not the static admin API token used by the older
 * `/api/operator/crm/commands/batch-*` surface.
 */
import {
  batchApproveCrmSendCommands,
  listCrmApprovalQueue,
} from './crm-approval-batch'
import { CrmCommandError } from './crm-command'
import { type CrmEmailDatabase } from './crm-email-domain-store'
import { type CrmResendDeps } from './crm-resend'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { isRecord, stringArrayFromUnknown } from './json-boundary'

type HttpResponse = globalThis.Response

export const ADMIN_OPS_CRM_BATCH_QUEUE_PATH = '/api/admin/ops/crm/batch-queue'
export const ADMIN_OPS_CRM_BATCH_APPROVE_PATH =
  '/api/admin/ops/crm/batch-approve'

export type AdminCaller = Readonly<{ userId: string }>

export type CrmApprovalBatchAdminRouteDependencies<Bindings> = Readonly<{
  db: (env: Bindings) => CrmEmailDatabase
  requireAdminCaller: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AdminCaller | undefined>
  resolveResendDeps: (env: Bindings) => CrmResendDeps
}>

const tenantOf = (url: URL, bodyTenant?: unknown): string => {
  if (typeof bodyTenant === 'string' && bodyTenant.trim() !== '') {
    return bodyTenant.trim()
  }
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === ''
    ? DEFAULT_CRM_TENANT_REF
    : value.trim()
}

const resolveDb = <Bindings>(
  dependencies: CrmApprovalBatchAdminRouteDependencies<Bindings>,
  env: Bindings,
): CrmEmailDatabase => dependencies.db(env)

export const makeCrmApprovalBatchAdminRoutes = <Bindings>(
  dependencies: CrmApprovalBatchAdminRouteDependencies<Bindings>,
) => {
  const handleQueue = async (
    request: Request,
    env: Bindings,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') return methodNotAllowed(['GET'])
    const url = new URL(request.url)
    const limitParam = url.searchParams.get('limit')
    const limit = limitParam === null ? undefined : Number(limitParam)
    try {
      const queue = await listCrmApprovalQueue(
        resolveDb(dependencies, env),
        tenantOf(url),
        {
          limit:
            limit !== undefined && Number.isFinite(limit) ? limit : undefined,
          status: url.searchParams.get('status') ?? undefined,
        },
      )
      return noStoreJsonResponse({ ok: true, queue })
    } catch (error) {
      if (error instanceof CrmCommandError) {
        return noStoreJsonResponse(
          {
            messageSafe: error.reason,
            ok: false,
            reason: error.reason,
          },
          { status: 422 },
        )
      }
      throw error
    }
  }

  const handleApprove = async (
    request: Request,
    env: Bindings,
    caller: AdminCaller,
  ): Promise<HttpResponse> => {
    if (request.method !== 'POST') return methodNotAllowed(['POST'])
    const url = new URL(request.url)
    const body = (await request.json().catch(() => null)) as Record<
      string,
      unknown
    > | null
    if (!isRecord(body)) {
      return noStoreJsonResponse(
        { messageSafe: 'json body required', ok: false },
        { status: 400 },
      )
    }
    const commandIds = stringArrayFromUnknown(body.commandIds).filter(
      id => id.trim() !== '',
    )
    if (commandIds.length === 0) {
      return noStoreJsonResponse(
        { messageSafe: 'commandIds[] required', ok: false },
        { status: 400 },
      )
    }
    const dailyCapRaw = body.dailyCap
    const dailyCap =
      typeof dailyCapRaw === 'number' && Number.isFinite(dailyCapRaw)
        ? dailyCapRaw
        : null
    const approvedByRef =
      typeof body.approvedByRef === 'string' && body.approvedByRef.trim() !== ''
        ? body.approvedByRef.trim()
        : `admin:${caller.userId}`

    try {
      const result = await batchApproveCrmSendCommands(
        resolveDb(dependencies, env),
        { resend: dependencies.resolveResendDeps(env) },
        {
          approvedByRef,
          commandIds,
          dailyCap,
          tenantRef: tenantOf(url, body.tenant),
        },
      )
      return noStoreJsonResponse({ ok: true, result })
    } catch (error) {
      if (error instanceof CrmCommandError) {
        return noStoreJsonResponse(
          {
            messageSafe: error.reason,
            ok: false,
            reason: error.reason,
          },
          { status: 422 },
        )
      }
      throw error
    }
  }

  return {
    handleCrmBatchApproveApi: async (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Promise<HttpResponse> => {
      const caller = await dependencies.requireAdminCaller(request, env, ctx)
      if (caller === undefined) return unauthorized()
      return handleApprove(request, env, caller)
    },
    handleCrmBatchQueueApi: async (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Promise<HttpResponse> => {
      const caller = await dependencies.requireAdminCaller(request, env, ctx)
      if (caller === undefined) return unauthorized()
      return handleQueue(request, env)
    },
  }
}
