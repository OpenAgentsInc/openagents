import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmApprovalBatchRoutes } from './crm-approval-batch-routes'
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
const messageRow = {
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

const makeDb = () => {
  const commands = new Map<string, Record<string, unknown>>()
  const statement = (query: string, bound: ReadonlyArray<unknown> = []): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_contact_commands')) {
          return Promise.resolve((commands.get(String(bound[1] ?? '')) ?? null) as T | null)
        }
        if (query.includes('COUNT(*)') && query.includes('crm_email_messages')) {
          return Promise.resolve({ n: 0 } as T)
        }
        if (query.includes('FROM crm_contacts')) return Promise.resolve(contactRow as T)
        if (query.includes('FROM crm_email_templates')) return Promise.resolve(templateRow as T)
        if (query.includes('FROM email_suppression_entries')) return Promise.resolve(null as T | null)
        if (query.includes('FROM email_preferences')) return Promise.resolve(null as T | null)
        if (query.includes('FROM crm_email_messages')) return Promise.resolve(messageRow as T)
        return Promise.resolve(null as T | null)
      },
      all: <T,>() =>
        Promise.resolve({
          meta: {} as D1Meta,
          results: (query.includes('FROM crm_contact_commands')
            ? [...commands.values()]
            : []) as unknown as Array<T>,
          success: true,
        } as D1Result<T>),
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
  return {
    batch: () => Promise.reject(new Error('batch')),
    dump: () => Promise.reject(new Error('dump')),
    exec: () => Promise.reject(new Error('exec')),
    prepare: (query: string) => statement(query),
    withSession: () => {
      throw new Error('session')
    },
  } as unknown as D1Database
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>
const ctx = {} as ExecutionContext
const disabledResend: CrmResendDeps = { enabled: false, fromEmail: null, sender: null }

const routesFor = (admin: boolean, db: D1Database) => {
  const routes = makeCrmApprovalBatchRoutes<Env>({
    requireAdminApiToken: () => Promise.resolve(admin),
    resolveResendDeps: () => disabledResend,
  })
  return (request: Request): Promise<Response> => {
    const effect = routes.routeCrmApprovalBatchRequest(request, { OPENAGENTS_DB: db }, ctx)
    if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
    return Effect.runPromise(effect)
  }
}

const base = 'https://openagents.com'

const post = (path: string, body: unknown): Request =>
  new Request(`${base}${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

// A minimal helper to propose a command through the sibling crm-command
// routes shape isn't available here, so we insert directly through the fake
// DB's INSERT branch via a raw prepared statement, mirroring proposeCrmSendCommand.
const seedProposedCommand = async (db: D1Database, id: string) => {
  await db
    .prepare(
      `INSERT INTO crm_contact_commands (
         id, tenant_ref, contact_id, command_kind, status, proposed_by_ref,
         approval_state, payload_json, result_json, created_at, updated_at
       ) VALUES (?, ?, ?, 'send_email', 'proposed', ?, 'pending_approval', ?, '{}', ?, ?)`,
    )
    .bind(
      id,
      'tenant.openagents',
      'crm_contact_1',
      null,
      JSON.stringify({ channel: 'gmail_gws', templateSlug: 'welcome' }),
      '2026-06-22T00:00:00.000Z',
      '2026-06-22T00:00:00.000Z',
    )
    .run()
}

describe('CRM approval-batch routes', () => {
  test('batch-queue lists grouped pending drafts', async () => {
    const db = makeDb()
    await seedProposedCommand(db, 'crm_cmd_1')
    const run = routesFor(true, db)

    const res = await run(new Request(`${base}/api/operator/crm/commands/batch-queue`))
    expect(res.status).toBe(200)
    const { queue } = (await res.json()) as { queue: { total: number; groups: Array<{ commands: Array<{ id: string }> }> } }
    expect(queue.total).toBe(1)
    expect(queue.groups[0]?.commands[0]?.id).toBe('crm_cmd_1')
  })

  test('batch-approve executes each command and returns a rollup result', async () => {
    const db = makeDb()
    await seedProposedCommand(db, 'crm_cmd_1')
    await seedProposedCommand(db, 'crm_cmd_2')
    const run = routesFor(true, db)

    const res = await run(
      post('/api/operator/crm/commands/batch-approve', {
        approvedByRef: 'operator:chris',
        commandIds: ['crm_cmd_1', 'crm_cmd_2'],
      }),
    )
    expect(res.status).toBe(200)
    const { result } = (await res.json()) as { result: { executedCount: number; batchRef: string } }
    expect(result.executedCount).toBe(2)
    expect(typeof result.batchRef).toBe('string')
  })

  test('batch-approve without commandIds => 400', async () => {
    const run = routesFor(true, makeDb())
    const res = await run(post('/api/operator/crm/commands/batch-approve', {}))
    expect(res.status).toBe(400)
  })

  test('401 without admin', async () => {
    const run = routesFor(false, makeDb())
    const res = await run(new Request(`${base}/api/operator/crm/commands/batch-queue`))
    expect(res.status).toBe(401)
  })

  test('non-matching path passes through', () => {
    const routes = makeCrmApprovalBatchRoutes<Env>({
      requireAdminApiToken: () => Promise.resolve(true),
      resolveResendDeps: () => disabledResend,
    })
    const effect = routes.routeCrmApprovalBatchRequest(
      new Request(`${base}/api/operator/crm/commands`),
      { OPENAGENTS_DB: makeDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })
})
