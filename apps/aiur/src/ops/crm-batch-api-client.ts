/**
 * Client-side (browser) API for the OB-4 (#8561) CRM batch approval queue.
 *
 * Hits Aiur's same-origin admin proxy paths, which forward to the main
 * Worker with the signed-in owner's bearer. Never talks to openagents.com
 * directly from the browser (no CORS, no token leak).
 *
 * Authority note: batch approve is UX only. Upstream still walks each
 * command through `approveAndExecuteCrmSendCommand` one-by-one
 * (`lead_gen_agent.no_send_without_approval_receipt.v1`).
 */
import {
  AIUR_ADMIN_OPS_CRM_BATCH_APPROVE_PATH,
  AIUR_ADMIN_OPS_CRM_BATCH_QUEUE_PATH,
} from '../admin-credits-proxy'

export type CrmBatchCommand = Readonly<{
  id: string
  tenantRef: string
  contactId: string | null
  commandKind: string
  status: string
  approvalState: string
  proposedByRef: string | null
  payload: Readonly<Record<string, unknown>>
  result: Readonly<Record<string, unknown>>
  createdAt: string
  updatedAt: string
}>

export type CrmBatchQueueGroup = Readonly<{
  day: string
  segmentRef: string
  commands: ReadonlyArray<CrmBatchCommand>
}>

export type CrmBatchQueueView = Readonly<{
  total: number
  groups: ReadonlyArray<CrmBatchQueueGroup>
}>

export type CrmBatchQueueResponse = Readonly<{
  ok: true
  queue: CrmBatchQueueView
}>

export type CrmBatchApproveDisposition =
  | 'executed'
  | 'failed'
  | 'not_pending'
  | 'not_found'
  | 'capped'

export type CrmBatchApproveItemResult = Readonly<{
  commandId: string
  disposition: CrmBatchApproveDisposition
}>

export type CrmBatchApproveResult = Readonly<{
  batchRef: string
  requestedCount: number
  executedCount: number
  failedCount: number
  notPendingCount: number
  notFoundCount: number
  cappedCount: number
  items: ReadonlyArray<CrmBatchApproveItemResult>
}>

export type CrmBatchApproveResponse = Readonly<{
  ok: true
  result: CrmBatchApproveResult
}>

export type CrmBatchApiError = Readonly<{
  ok: false
  status: number
  messageSafe: string
}>

export type CrmBatchApiResult<T> =
  | Readonly<{ ok: true; value: T }>
  | CrmBatchApiError

const parseJsonSafe = async (
  response: Response,
): Promise<Record<string, unknown>> => {
  try {
    const parsed: unknown = await response.json()
    return typeof parsed === 'object' && parsed !== null
      ? (parsed as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

async function requestJson<T>(
  path: string,
  init?: RequestInit,
): Promise<CrmBatchApiResult<T>> {
  const response = await fetch(path, init)
  const body = await parseJsonSafe(response)
  if (!response.ok) {
    return {
      messageSafe:
        typeof body.messageSafe === 'string'
          ? body.messageSafe
          : typeof body.reason === 'string'
            ? body.reason
            : `Request failed (${response.status}).`,
      ok: false,
      status: response.status,
    }
  }
  return { ok: true, value: body as unknown as T }
}

export const fetchCrmBatchQueue = (
  params: Readonly<{ limit?: number; status?: string; tenant?: string }> = {},
): Promise<CrmBatchApiResult<CrmBatchQueueResponse>> => {
  const query = new URLSearchParams()
  if (params.limit !== undefined) query.set('limit', String(params.limit))
  if (params.status !== undefined) query.set('status', params.status)
  if (params.tenant !== undefined) query.set('tenant', params.tenant)
  const search = query.toString()
  return requestJson(
    `${AIUR_ADMIN_OPS_CRM_BATCH_QUEUE_PATH}${search === '' ? '' : `?${search}`}`,
  )
}

export const postCrmBatchApprove = (
  input: Readonly<{
    commandIds: ReadonlyArray<string>
    approvedByRef?: string
    dailyCap?: number
    tenant?: string
  }>,
): Promise<CrmBatchApiResult<CrmBatchApproveResponse>> =>
  requestJson(AIUR_ADMIN_OPS_CRM_BATCH_APPROVE_PATH, {
    body: JSON.stringify({
      approvedByRef: input.approvedByRef,
      commandIds: input.commandIds,
      dailyCap: input.dailyCap,
      tenant: input.tenant,
    }),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })
