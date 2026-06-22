import { describe, expect, test } from 'vitest'

import {
  approveAndExecuteCrmSendCommand,
  getCrmCommand,
  proposeCrmSendCommand,
  rejectCrmCommand,
} from './crm-command'
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

// Stateful fake: tracks crm_contact_commands rows; serves canned rows for the
// dispatch path so approve() actually executes.
const makeDb = (opts: { suppressed?: boolean } = {}) => {
  const commands = new Map<string, Record<string, unknown>>()

  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_contact_commands')) {
          const id = String(bound[1] ?? '')
          return Promise.resolve((commands.get(id) ?? null) as T | null)
        }
        if (query.includes('FROM crm_contacts')) return Promise.resolve(contactRow as T)
        if (query.includes('FROM crm_email_templates')) return Promise.resolve(templateRow as T)
        if (query.includes('FROM email_suppression_entries')) {
          return Promise.resolve((opts.suppressed === true ? { id: 's', scope: 'all' } : null) as T | null)
        }
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
  return { commands, db }
}

const runtime = { makeId: (p: string) => `${p}_1`, nowIso: () => '2026-06-22T00:00:00.000Z' }
const disabledResend: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }
const tenant = 'tenant.openagents'

describe('CRM send_email command — propose', () => {
  test('records a pending_approval send_email command with the payload', async () => {
    const { db } = makeDb()
    const command = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    expect(command.commandKind).toBe('send_email')
    expect(command.status).toBe('proposed')
    expect(command.approvalState).toBe('pending_approval')
    expect(command.payload.channel).toBe('gmail_gws')
    expect(command.payload.templateSlug).toBe('welcome')
  })
})

describe('CRM send_email command — approve + execute', () => {
  test('approving a proposed command executes the send and marks it applied', async () => {
    const { db } = makeDb()
    const proposed = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    const result = await approveAndExecuteCrmSendCommand(
      db,
      { resend: disabledResend },
      { approvedByRef: 'operator:chris', commandId: proposed.id, tenantRef: tenant },
      runtime,
    )
    expect(result.kind).toBe('executed')
    if (result.kind === 'executed') {
      expect(result.outcome.channel).toBe('gmail_gws')
      expect(result.command.status).toBe('applied')
    }
    const reread = await getCrmCommand(db, tenant, proposed.id)
    expect(reread?.status).toBe('applied')
  })

  test('a suppressed send marks the command failed (gate held)', async () => {
    const { db } = makeDb({ suppressed: true })
    const proposed = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    const result = await approveAndExecuteCrmSendCommand(
      db,
      { resend: disabledResend },
      { commandId: proposed.id, tenantRef: tenant },
      runtime,
    )
    expect(result.kind).toBe('executed')
    if (result.kind === 'executed') {
      expect(result.command.status).toBe('failed')
    }
  })

  test('approving a missing command => not_found', async () => {
    const { db } = makeDb()
    const result = await approveAndExecuteCrmSendCommand(
      db,
      { resend: disabledResend },
      { commandId: 'nope', tenantRef: tenant },
      runtime,
    )
    expect(result.kind).toBe('not_found')
  })

  test('re-approving an applied command => not_pending', async () => {
    const { db } = makeDb()
    const proposed = await proposeCrmSendCommand(
      db,
      { channel: 'gmail_gws', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    await approveAndExecuteCrmSendCommand(
      db,
      { resend: disabledResend },
      { commandId: proposed.id, tenantRef: tenant },
      runtime,
    )
    const again = await approveAndExecuteCrmSendCommand(
      db,
      { resend: disabledResend },
      { commandId: proposed.id, tenantRef: tenant },
      runtime,
    )
    expect(again.kind).toBe('not_pending')
  })
})

describe('CRM send_email command — reject', () => {
  test('rejecting a proposed command marks it rejected (no send)', async () => {
    const { db } = makeDb()
    const proposed = await proposeCrmSendCommand(
      db,
      { channel: 'resend', contactId: 'crm_contact_1', templateSlug: 'welcome', tenantRef: tenant },
      runtime,
    )
    await rejectCrmCommand(db, { commandId: proposed.id, reason: 'not now', tenantRef: tenant }, runtime)
    const reread = await getCrmCommand(db, tenant, proposed.id)
    expect(reread?.status).toBe('rejected')
  })
})
