import { describe, expect, test } from 'vitest'

import {
  batchApproveCrmSendCommands,
  DEFAULT_CRM_DAILY_SEND_CAP,
  listCrmApprovalQueue,
} from './crm-approval-batch'
import { proposeCrmSendCommand } from './crm-command'
import { type CrmResendDeps } from './crm-resend'

const contactRow = {
  created_at: '2026-06-22T00:00:00.000Z',
  first_name: 'Ada',
  id: 'crm_contact_1',
  primary_email: 'ada@example.com',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}
const templateRow = {
  body_markdown_template: 'Hi {{ contact.first_name }}',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_template_1',
  name: 'Welcome',
  slug: 'welcome',
  status: 'active',
  subject_template: 'Hello {{ contact.first_name }}',
  tenant_ref: 'tenant.openagents',
  updated_at: '2026-06-22T00:00:00.000Z',
}
const queuedMessageRow = {
  body_markdown: 'Hi Ada',
  channel: 'gmail_gws',
  contact_id: 'crm_contact_1',
  created_at: '2026-06-22T00:00:00.000Z',
  id: 'crm_email_q',
  status: 'queued',
  subject: 'Hello Ada',
  tenant_ref: 'tenant.openagents',
  to_email: 'ada@example.com',
  updated_at: '2026-06-22T00:00:00.000Z',
}

// Stateful fake: tracks crm_contact_commands + crm_command_batches rows so
// propose -> batch-approve actually executes and the batch receipt lands.
const makeDb = (opts: { sentTodayCount?: number } = {}) => {
  const commands = new Map<string, Record<string, unknown>>()
  const batches: Array<Record<string, unknown>> = []

  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_contact_commands')) {
          const id = String(bound[1] ?? '')
          return Promise.resolve((commands.get(id) ?? null) as T | null)
        }
        if (query.includes('COUNT(*)') && query.includes('crm_email_messages')) {
          return Promise.resolve({ n: opts.sentTodayCount ?? 0 } as T)
        }
        if (query.includes('FROM crm_contacts')) return Promise.resolve(contactRow as T)
        if (query.includes('FROM crm_email_templates')) return Promise.resolve(templateRow as T)
        if (query.includes('FROM email_suppression_entries')) return Promise.resolve(null as T | null)
        if (query.includes('FROM email_preferences')) return Promise.resolve(null as T | null)
        if (query.includes('FROM crm_email_messages')) return Promise.resolve(queuedMessageRow as T)
        return Promise.resolve(null as T | null)
      },
      all: <T,>() => {
        if (query.includes('FROM crm_contact_commands')) {
          return Promise.resolve({
            meta: {} as D1Meta,
            results: [...commands.values()] as unknown as Array<T>,
            success: true,
          } as D1Result<T>)
        }
        return Promise.resolve({ meta: {} as D1Meta, results: [] as unknown as Array<T>, success: true } as D1Result<T>)
      },
      run: () => {
        if (query.includes('INSERT INTO crm_contact_commands')) {
          const [id, tenantRef, contactId, proposedByRef, payloadJson, createdAt, updatedAt] = bound
          commands.set(String(id), {
            approval_state: 'pending_approval',
            command_kind: 'send_email',
            contact_id: contactId,
            created_at: createdAt,
            id,
            payload_json: payloadJson,
            proposed_by_ref: proposedByRef,
            result_json: '{}',
            status: 'proposed',
            tenant_ref: tenantRef,
            updated_at: updatedAt,
          })
        }
        if (query.includes('UPDATE crm_contact_commands')) {
          const [status, approvalState, resultJson, updatedAt, , id] = bound
          const existing = commands.get(String(id))
          if (existing !== undefined) {
            commands.set(String(id), {
              ...existing,
              approval_state: approvalState,
              result_json: resultJson,
              status,
              updated_at: updatedAt,
            })
          }
        }
        if (query.includes('INSERT INTO crm_command_batches')) {
          batches.push({ raw: bound })
        }
        return Promise.resolve({ meta: {} as D1Meta, results: [], success: true } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement

  const db = {
    batch: () => Promise.reject(new Error('batch')),
    dump: () => Promise.reject(new Error('dump')),
    exec: () => Promise.reject(new Error('exec')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
  return { batches, commands, db }
}

let counter = 0
const runtime = {
  makeId: (p: string) => `${p}_${(counter += 1)}`,
  nowIso: () => '2026-06-22T00:00:00.000Z',
}
const disabledResend: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }
const tenant = 'tenant.openagents'

describe('listCrmApprovalQueue', () => {
  test('groups pending send_email drafts by day + segmentRef', async () => {
    const { db } = makeDb()
    await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', segmentRef: 'mastermind', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    await proposeCrmSendCommand(
      db,
      { channel: 'resend', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )

    const queue = await listCrmApprovalQueue(db, tenant)
    expect(queue.total).toBe(2)
    expect(queue.groups.length).toBe(2)
    const segments = queue.groups.map(g => g.segmentRef).sort()
    expect(segments).toEqual(['mastermind', 'unassigned'])
  })
})

describe('batchApproveCrmSendCommands', () => {
  test('approves + executes each command individually and writes one batch receipt', async () => {
    const { batches, db } = makeDb()
    const a = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    const b = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )

    const result = await batchApproveCrmSendCommands(
      db,
      { resend: disabledResend },
      { approvedByRef: 'operator:chris', commandIds: [a.id, b.id], tenantRef: tenant },
      runtime,
    )

    expect(result.requestedCount).toBe(2)
    expect(result.executedCount).toBe(2)
    expect(result.cappedCount).toBe(0)
    expect(result.items.every(item => item.disposition === 'executed')).toBe(true)
    expect(batches.length).toBe(1)
  })

  test('stops sending once the daily cap is hit; remaining items come back capped', async () => {
    const { db } = makeDb({ sentTodayCount: DEFAULT_CRM_DAILY_SEND_CAP - 1 })
    const a = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    const b = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )

    const result = await batchApproveCrmSendCommands(
      db,
      { resend: disabledResend },
      { commandIds: [a.id, b.id], tenantRef: tenant },
      runtime,
    )

    expect(result.executedCount).toBe(1)
    expect(result.cappedCount).toBe(1)
    expect(result.items[0]?.disposition).toBe('executed')
    expect(result.items[1]?.disposition).toBe('capped')
  })

  test('an explicit dailyCap override is honored', async () => {
    const { db } = makeDb({ sentTodayCount: 0 })
    const a = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    const b = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )

    const result = await batchApproveCrmSendCommands(
      db,
      { resend: disabledResend },
      { commandIds: [a.id, b.id], dailyCap: 1, tenantRef: tenant },
      runtime,
    )

    expect(result.executedCount).toBe(1)
    expect(result.cappedCount).toBe(1)
  })

  test('not_found and not_pending commands are reported without aborting the batch', async () => {
    const { db } = makeDb()
    const a = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )

    const result = await batchApproveCrmSendCommands(
      db,
      { resend: disabledResend },
      { commandIds: [a.id, 'nope'], tenantRef: tenant },
      runtime,
    )

    expect(result.executedCount).toBe(1)
    expect(result.notFoundCount).toBe(1)
  })
})
