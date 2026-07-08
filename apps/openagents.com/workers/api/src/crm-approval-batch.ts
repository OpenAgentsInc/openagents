/**
 * OB-4 (#8561): batch approval UX over the EXISTING CRM approval queue.
 *
 * The law is unchanged and stays enforced here: outbound send remains
 * approval-gated. `crm_contact_commands` propose -> operator approve ->
 * `dispatchCrmSend` (crm-command.ts) is the only path a send ever takes, and
 * every send still gets its own individual result on that command row. This
 * module adds a BATCH UX, not batch AUTHORITY:
 *
 *   - `listCrmApprovalQueue` groups a tenant's pending `send_email` drafts by
 *     day (created_at date) and segment (an optional `segmentRef` a drafting
 *     agent can attach to its proposal payload — see crm-command.ts) so an
 *     operator can review a day's queue in one screen.
 *   - `batchApproveCrmSendCommands` takes a list of already-PROPOSED command
 *     ids and approves+executes each one individually through the unchanged
 *     `approveAndExecuteCrmSendCommand` path (one real approval + one real
 *     send decision per row, one real receipt per row), then writes ONE
 *     rollup receipt (`crm_command_batches`) recording the batch action
 *     itself and how each item disposed. A daily send cap (a stand-in for
 *     OB-1's full ramp-config governance, which has not landed yet) is
 *     enforced at dispatch: once the cap is hit mid-batch, remaining items
 *     are marked `capped` and never sent.
 */
// KS-8.11 (#8322): CrmEmailDatabase union — the batch-approve path calls the
// already-mirrored crm-command.ts writes; the new crm_command_batches rollup
// receipt is plain-D1-only for now (not registered in the Postgres mirror
// table set — see crm-email-domain-store.ts).
import { type CrmEmailDatabase, crmEmailAuthorityDb } from './crm-email-domain-store'
import {
  approveAndExecuteCrmSendCommand,
  type CrmContactCommand,
  listCrmCommands,
} from './crm-command'
import { type CrmDispatchDeps } from './crm-send'
import { type CrmRuntime, defaultCrmRuntime } from './crm-store'

// ---------------------------------------------------------------------------
// Batch queue view (list, grouped)
// ---------------------------------------------------------------------------

export type CrmApprovalQueueGroup = Readonly<{
  day: string
  segmentRef: string
  commands: ReadonlyArray<CrmContactCommand>
}>

export type CrmApprovalQueueView = Readonly<{
  total: number
  groups: ReadonlyArray<CrmApprovalQueueGroup>
}>

const dayOf = (isoTimestamp: string): string =>
  isoTimestamp.length >= 10 ? isoTimestamp.slice(0, 10) : 'unknown'

const segmentRefOf = (command: CrmContactCommand): string => {
  const ref = command.payload.segmentRef
  return typeof ref === 'string' && ref.trim() !== '' ? ref.trim() : 'unassigned'
}

export const listCrmApprovalQueue = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: Readonly<{ status?: string | undefined; limit?: number | undefined }> = {},
): Promise<CrmApprovalQueueView> => {
  const commands = await listCrmCommands(db, tenantRef, {
    limit: query.limit ?? 500,
    status: query.status ?? 'proposed',
  })
  const sendCommands = commands.filter(command => command.commandKind === 'send_email')

  const byKey = new Map<
    string,
    { day: string; segmentRef: string; commands: Array<CrmContactCommand> }
  >()
  for (const command of sendCommands) {
    const day = dayOf(command.createdAt)
    const segmentRef = segmentRefOf(command)
    const key = `${day}::${segmentRef}`
    const existing = byKey.get(key)
    if (existing === undefined) {
      byKey.set(key, { commands: [command], day, segmentRef })
    } else {
      existing.commands.push(command)
    }
  }

  const groups = [...byKey.values()]
    .sort((a, b) =>
      a.day === b.day ? a.segmentRef.localeCompare(b.segmentRef) : b.day < a.day ? 1 : -1,
    )
    .map(group => ({
      commands: group.commands,
      day: group.day,
      segmentRef: group.segmentRef,
    }))

  return { groups, total: sendCommands.length }
}

// ---------------------------------------------------------------------------
// Daily send cap (stand-in for OB-1's ramp-config governance)
// ---------------------------------------------------------------------------

/**
 * OB-1 (#8558) owns the real warm-up ramp config (typed, deliverability-gated,
 * owner sign-off per raise). Until it lands, this fixed default keeps "the
 * cap is enforced server-side at dispatch, not by convention" true for the
 * 100/day target named in this issue. Callers may override via `dailyCap`.
 */
export const DEFAULT_CRM_DAILY_SEND_CAP = 100

const countCrmSendsForDay = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  day: string,
): Promise<number> => {
  const row = await crmEmailAuthorityDb(db)
    .prepare(
      `SELECT COUNT(*) as n FROM crm_email_messages
        WHERE tenant_ref = ? AND status IN ('queued', 'sent') AND created_at LIKE ?`,
    )
    .bind(tenantRef, `${day}%`)
    .first<{ n: number }>()
  return row === null || typeof row.n !== 'number' ? 0 : row.n
}

// ---------------------------------------------------------------------------
// Batch approve (the one-tap operator action)
// ---------------------------------------------------------------------------

const MAX_BATCH_APPROVE = 500

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

const recordCrmCommandBatch = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    batchRef: string
    tenantRef: string
    approvedByRef: string | null
    commandIds: ReadonlyArray<string>
    requestedCount: number
    executedCount: number
    failedCount: number
    notPendingCount: number
    notFoundCount: number
    cappedCount: number
  }>,
  runtime: CrmRuntime,
): Promise<void> => {
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO crm_command_batches (
         id, tenant_ref, approved_by_ref, command_ids_json, requested_count,
         executed_count, failed_count, not_pending_count, not_found_count,
         capped_count, created_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      input.batchRef,
      input.tenantRef,
      input.approvedByRef,
      JSON.stringify(input.commandIds),
      input.requestedCount,
      input.executedCount,
      input.failedCount,
      input.notPendingCount,
      input.notFoundCount,
      input.cappedCount,
      runtime.nowIso(),
    )
    .run()
}

export type CrmBatchApproveInput = Readonly<{
  tenantRef: string
  commandIds: ReadonlyArray<string>
  approvedByRef?: string | null
  /** Override for tests / an explicit operator-set cap. Defaults to 100/day. */
  dailyCap?: number | null
}>

/**
 * Approve + execute a batch of already-PROPOSED send_email commands in one
 * operator action. Each command still goes through the unchanged
 * `approveAndExecuteCrmSendCommand` gate one at a time — this is batch UX
 * over individual authority, never a bulk send primitive. Writes ONE
 * `crm_command_batches` rollup receipt alongside the per-command receipts
 * that already land on each `crm_contact_commands` row.
 */
export const batchApproveCrmSendCommands = async (
  db: CrmEmailDatabase,
  deps: CrmDispatchDeps,
  input: CrmBatchApproveInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmBatchApproveResult> => {
  const commandIds = [...new Set(input.commandIds.filter(id => id.trim() !== ''))].slice(
    0,
    MAX_BATCH_APPROVE,
  )
  const dailyCap =
    input.dailyCap === undefined || input.dailyCap === null || !Number.isFinite(input.dailyCap) || input.dailyCap <= 0
      ? DEFAULT_CRM_DAILY_SEND_CAP
      : Math.floor(input.dailyCap)
  const today = dayOf(runtime.nowIso())
  let sentToday = await countCrmSendsForDay(db, input.tenantRef, today)

  const items: Array<CrmBatchApproveItemResult> = []
  let executedCount = 0
  let failedCount = 0
  let notPendingCount = 0
  let notFoundCount = 0
  let cappedCount = 0

  for (const commandId of commandIds) {
    if (sentToday >= dailyCap) {
      items.push({ commandId, disposition: 'capped' })
      cappedCount += 1
      continue
    }

    const result = await approveAndExecuteCrmSendCommand(
      db,
      deps,
      { approvedByRef: input.approvedByRef ?? null, commandId, tenantRef: input.tenantRef },
      runtime,
    )

    if (result.kind === 'not_found') {
      items.push({ commandId, disposition: 'not_found' })
      notFoundCount += 1
      continue
    }
    if (result.kind === 'not_pending') {
      items.push({ commandId, disposition: 'not_pending' })
      notPendingCount += 1
      continue
    }

    if (result.command.status === 'applied') {
      items.push({ commandId, disposition: 'executed' })
      executedCount += 1
      sentToday += 1
    } else {
      items.push({ commandId, disposition: 'failed' })
      failedCount += 1
    }
  }

  const batchRef = runtime.makeId('crm_batch')
  await recordCrmCommandBatch(
    db,
    {
      approvedByRef: input.approvedByRef ?? null,
      batchRef,
      cappedCount,
      commandIds,
      executedCount,
      failedCount,
      notFoundCount,
      notPendingCount,
      requestedCount: commandIds.length,
      tenantRef: input.tenantRef,
    },
    runtime,
  )

  return {
    batchRef,
    cappedCount,
    executedCount,
    failedCount,
    items,
    notFoundCount,
    notPendingCount,
    requestedCount: commandIds.length,
  }
}
