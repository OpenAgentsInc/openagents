/**
 * Approval-gated CRM `send_email{channel}` command (epic #5980, sub-issue #5986).
 *
 * This is the CRM projection of the Blueprint `send_email` effect kind
 * (`blueprint-routes.ts`): chat (or an agent program) PROPOSES a send as a
 * `crm_contact_commands` row (kind `send_email`, `pending_approval`); an
 * operator APPROVES; the executor runs the unified `dispatchCrmSend` over the
 * chosen channel and records the outcome. Nothing sends without an explicit
 * approval — the same proposal→approval→execute gate Blueprint uses, kept on the
 * CRM's own command table (added in migration 0218) so the send is tenant-scoped
 * and tied to the contact.
 */
import { Schema as S } from 'effect'

import { type CrmSendChannel } from './crm-email'
// KS-8.11 (#8322): CrmEmailDatabase union — command proposals/updates mirror
// their crm_contact_commands rows to Postgres fail-soft.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import {
  type CrmDispatchDeps,
  type CrmSendOutcome,
  dispatchCrmSend,
} from './crm-send'
import { type CrmRuntime, defaultCrmRuntime } from './crm-store'
import { parseJsonRecord } from './json-boundary'

export class CrmCommandError extends S.TaggedErrorClass<CrmCommandError>()(
  'CrmCommandError',
  { reason: S.String },
) {}

export type CrmContactCommand = Readonly<{
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

export type CrmSendCommandPayload = Readonly<{
  channel: CrmSendChannel
  templateSlug: string
  sendReason?: string | null
}>

const str = (v: unknown): string =>
  v === null || v === undefined ? '' : String(v)
const nullableStr = (v: unknown): string | null =>
  v === null || v === undefined ? null : String(v)

const parseJson = (value: unknown): Record<string, unknown> =>
  typeof value === 'string' ? (parseJsonRecord(value) ?? {}) : {}

const decodeCommand = (row: Record<string, unknown>): CrmContactCommand => ({
  approvalState: str(row.approval_state),
  commandKind: str(row.command_kind),
  contactId: nullableStr(row.contact_id),
  createdAt: str(row.created_at),
  id: str(row.id),
  payload: parseJson(row.payload_json),
  proposedByRef: nullableStr(row.proposed_by_ref),
  result: parseJson(row.result_json),
  status: str(row.status),
  tenantRef: str(row.tenant_ref),
  updatedAt: str(row.updated_at),
})

const wrap = async (
  operation: string,
  fn: () => Promise<unknown>,
): Promise<void> => {
  try {
    await fn()
  } catch (error) {
    throw new CrmCommandError({ reason: `${operation}: ${String(error)}` })
  }
}

// ---------------------------------------------------------------------------
// Propose (chat / agent)
// ---------------------------------------------------------------------------

export const proposeCrmSendCommand = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    contactId: string
    channel: CrmSendChannel
    templateSlug: string
    sendReason?: string | null
    proposedByRef?: string | null
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmContactCommand> => {
  const id = runtime.makeId('crm_cmd')
  const now = runtime.nowIso()
  const payload: CrmSendCommandPayload = {
    channel: input.channel,
    sendReason: input.sendReason ?? null,
    templateSlug: input.templateSlug,
  }
  await wrap('crm.proposeSendCommand', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_contact_commands (
           id, tenant_ref, contact_id, command_kind, status, proposed_by_ref,
           approval_state, payload_json, result_json, created_at, updated_at
         ) VALUES (?, ?, ?, 'send_email', 'proposed', ?, 'pending_approval', ?, '{}', ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        input.contactId,
        input.proposedByRef ?? null,
        JSON.stringify(payload),
        now,
        now,
      )
      .run(),
  )
  const stored = await getCrmCommand(db, input.tenantRef, id)
  if (stored === null) {
    throw new CrmCommandError({
      reason: 'crm.proposeSendCommand: vanished after insert',
    })
  }
  await mirrorCrmEmailRows(db, 'crm_contact_commands', 'id', [id])
  return stored
}

export const getCrmCommand = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  id: string,
): Promise<CrmContactCommand | null> => {
  try {
    const row = await crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_contact_commands WHERE tenant_ref = ? AND id = ? LIMIT 1',
      )
      .bind(tenantRef, id)
      .first<Record<string, unknown>>()
    return row === null ? null : decodeCommand(row)
  } catch (error) {
    throw new CrmCommandError({ reason: `crm.getCommand: ${String(error)}` })
  }
}

export const listCrmCommands = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: Readonly<{
    status?: string | undefined
    limit?: number | undefined
  }> = {},
): Promise<ReadonlyArray<CrmContactCommand>> => {
  const limit =
    query.limit === undefined ||
    !Number.isFinite(query.limit) ||
    query.limit <= 0
      ? 100
      : Math.min(Math.floor(query.limit), 500)
  try {
    const statement =
      query.status === undefined || query.status.trim() === ''
        ? crmEmailAuthorityDb(db)
            .prepare(
              'SELECT * FROM crm_contact_commands WHERE tenant_ref = ? ORDER BY created_at DESC LIMIT ?',
            )
            .bind(tenantRef, limit)
        : crmEmailAuthorityDb(db)
            .prepare(
              'SELECT * FROM crm_contact_commands WHERE tenant_ref = ? AND status = ? ORDER BY created_at DESC LIMIT ?',
            )
            .bind(tenantRef, query.status.trim(), limit)
    const result = await statement.all<Record<string, unknown>>()
    return (result.results ?? []).map(decodeCommand)
  } catch (error) {
    throw new CrmCommandError({ reason: `crm.listCommands: ${String(error)}` })
  }
}

const updateCommand = (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    id: string
    status: string
    approvalState: string
    resultJson: string
  }>,
  runtime: CrmRuntime,
): Promise<void> =>
  wrap('crm.updateCommand', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE crm_contact_commands SET
           status = ?, approval_state = ?, result_json = ?, updated_at = ?
         WHERE tenant_ref = ? AND id = ?`,
      )
      .bind(
        input.status,
        input.approvalState,
        input.resultJson,
        runtime.nowIso(),
        input.tenantRef,
        input.id,
      )
      .run()
      .then(() =>
        mirrorCrmEmailRows(db, 'crm_contact_commands', 'id', [input.id]),
      ),
  )

// ---------------------------------------------------------------------------
// Approve + execute / reject (operator)
// ---------------------------------------------------------------------------

export type ApproveCrmCommandResult =
  | Readonly<{
      kind: 'executed'
      command: CrmContactCommand
      outcome: CrmSendOutcome
    }>
  | Readonly<{ kind: 'not_pending'; command: CrmContactCommand }>
  | Readonly<{ kind: 'not_found' }>

const outcomeFailed = (outcome: CrmSendOutcome): boolean => {
  if (outcome.channel === 'resend') {
    return (
      outcome.result.kind === 'failed' || outcome.result.kind === 'suppressed'
    )
  }
  return outcome.kind === 'suppressed'
}

export const approveAndExecuteCrmSendCommand = async (
  db: CrmEmailDatabase,
  deps: CrmDispatchDeps,
  input: Readonly<{
    tenantRef: string
    commandId: string
    approvedByRef?: string | null
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<ApproveCrmCommandResult> => {
  const command = await getCrmCommand(db, input.tenantRef, input.commandId)
  if (command === null) {
    return { kind: 'not_found' }
  }
  if (command.status !== 'proposed') {
    return { command, kind: 'not_pending' }
  }
  if (command.contactId === null) {
    throw new CrmCommandError({ reason: 'command has no contact' })
  }

  const channel = command.payload.channel === 'resend' ? 'resend' : 'gmail_gws'
  const templateSlug =
    typeof command.payload.templateSlug === 'string'
      ? command.payload.templateSlug
      : ''
  if (templateSlug === '') {
    throw new CrmCommandError({
      reason: 'command payload missing templateSlug',
    })
  }

  const outcome = await dispatchCrmSend(
    db,
    deps,
    {
      channel,
      contactId: command.contactId,
      sendReason:
        typeof command.payload.sendReason === 'string'
          ? command.payload.sendReason
          : null,
      templateSlug,
      tenantRef: input.tenantRef,
    },
    runtime,
  )

  const failed = outcomeFailed(outcome)
  await updateCommand(
    db,
    {
      approvalState: 'approved',
      id: command.id,
      resultJson: JSON.stringify({
        approvedByRef: input.approvedByRef ?? null,
        outcome,
      }),
      status: failed ? 'failed' : 'applied',
      tenantRef: input.tenantRef,
    },
    runtime,
  )

  const updated = await getCrmCommand(db, input.tenantRef, command.id)
  return { command: updated ?? command, kind: 'executed', outcome }
}

export const rejectCrmCommand = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    commandId: string
    reason?: string | null
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<ApproveCrmCommandResult> => {
  const command = await getCrmCommand(db, input.tenantRef, input.commandId)
  if (command === null) {
    return { kind: 'not_found' }
  }
  if (command.status !== 'proposed') {
    return { command, kind: 'not_pending' }
  }
  await updateCommand(
    db,
    {
      approvalState: 'rejected',
      id: command.id,
      resultJson: JSON.stringify({ rejectedReason: input.reason ?? null }),
      status: 'rejected',
      tenantRef: input.tenantRef,
    },
    runtime,
  )
  const updated = await getCrmCommand(db, input.tenantRef, command.id)
  return { command: updated ?? command, kind: 'not_pending' }
}
