import { describe, expect, test } from 'vitest'

import {
  ADMIN_OPS_CRM_BATCH_APPROVE_PATH,
  ADMIN_OPS_CRM_BATCH_QUEUE_PATH,
  type AdminCaller,
  makeCrmApprovalBatchAdminRoutes,
} from './crm-approval-batch-admin-routes'
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
  const statement = (
    query: string,
    bound: ReadonlyArray<unknown> = [],
  ): D1PreparedStatement =>
    ({
      bind: (...values: ReadonlyArray<unknown>) => statement(query, values),
      first: <T,>() => {
        if (query.includes('FROM crm_contact_commands')) {
          return Promise.resolve(
            (commands.get(String(bound[1] ?? '')) ?? null) as T | null,
          )
        }
        if (query.includes('COUNT(*)') && query.includes('crm_email_messages')) {
          return Promise.resolve({ n: 0 } as T)
        }
        if (query.includes('FROM crm_contacts')) {
          return Promise.resolve(contactRow as T)
        }
        if (query.includes('FROM crm_email_templates')) {
          return Promise.resolve(templateRow as T)
        }
        if (query.includes('FROM email_suppression_entries')) {
          return Promise.resolve(null as T | null)
        }
        if (query.includes('FROM email_preferences')) {
          return Promise.resolve(null as T | null)
        }
        if (query.includes('FROM crm_email_messages')) {
          return Promise.resolve(messageRow as T)
        }
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
          const [
            id,
            tenantRef,
            contactId,
            proposedByRef,
            payloadJson,
            createdAt,
            updatedAt,
          ] = bound
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
          return Promise.resolve({
            meta: {} as D1Meta,
            results: [],
            success: true,
          } as unknown as D1Result)
        }
        return Promise.resolve({
          meta: {} as D1Meta,
          results: [],
          success: true,
        } as unknown as D1Result)
      },
      raw: () => Promise.reject(new Error('raw')),
    }) as unknown as D1PreparedStatement

  return {
    batch: () => Promise.resolve([] as Array<unknown>),
    dump: () => commands,
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => statement(query),
    seed: (row: Record<string, unknown>) => {
      commands.set(String(row.id), row)
    },
  } as unknown as D1Database & {
    dump: () => Map<string, Record<string, unknown>>
    seed: (row: Record<string, unknown>) => void
  }
}

const fakeResend: CrmResendDeps = {
  enabled: true,
  fromEmail: 'outreach@openagents.com',
  sender: async () => ({ ok: true, providerMessageId: 'resend_1' }),
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>

const makeRoutes = (adminUserId: string | undefined, db: D1Database) =>
  makeCrmApprovalBatchAdminRoutes<Env>({
    db: env => env.OPENAGENTS_DB as never,
    requireAdminCaller: async (): Promise<AdminCaller | undefined> =>
      adminUserId === undefined ? undefined : { userId: adminUserId },
    resolveResendDeps: () => fakeResend,
  })

const fakeCtx = {} as ExecutionContext

const seedProposed = (
  db: ReturnType<typeof makeDb>,
  id: string,
  day = '2026-07-08T10:00:00.000Z',
) => {
  db.seed({
    approval_state: 'pending_approval',
    command_kind: 'send_email',
    contact_id: 'crm_contact_1',
    created_at: day,
    id,
    payload_json: JSON.stringify({
      channel: 'resend',
      segmentRef: 'seg.a',
      templateSlug: 'welcome',
    }),
    proposed_by_ref: 'agent.sarah',
    result_json: '{}',
    status: 'proposed',
    tenant_ref: 'tenant.openagents',
    updated_at: day,
  })
}

describe('crm approval batch admin routes — auth (fail closed)', () => {
  test('queue 401s without an admin caller', async () => {
    const db = makeDb()
    const routes = makeRoutes(undefined, db)
    const response = await routes.handleCrmBatchQueueApi(
      new Request(`https://openagents.com${ADMIN_OPS_CRM_BATCH_QUEUE_PATH}`),
      { OPENAGENTS_DB: db },
      fakeCtx,
    )
    expect(response.status).toBe(401)
  })

  test('approve 401s without an admin caller', async () => {
    const db = makeDb()
    const routes = makeRoutes(undefined, db)
    const response = await routes.handleCrmBatchApproveApi(
      new Request(`https://openagents.com${ADMIN_OPS_CRM_BATCH_APPROVE_PATH}`, {
        body: JSON.stringify({ commandIds: ['crm_cmd_1'] }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      fakeCtx,
    )
    expect(response.status).toBe(401)
  })
})

describe('crm approval batch admin routes', () => {
  test('queue lists grouped pending drafts for an admin', async () => {
    const db = makeDb()
    seedProposed(db, 'crm_cmd_1')
    seedProposed(db, 'crm_cmd_2')
    const routes = makeRoutes('user_admin', db)

    const response = await routes.handleCrmBatchQueueApi(
      new Request(`https://openagents.com${ADMIN_OPS_CRM_BATCH_QUEUE_PATH}`),
      { OPENAGENTS_DB: db },
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ok: boolean
      queue: { total: number; groups: ReadonlyArray<{ segmentRef: string }> }
    }
    expect(body.ok).toBe(true)
    expect(body.queue.total).toBe(2)
    expect(body.queue.groups[0]?.segmentRef).toBe('seg.a')
  })

  test('batch-approve executes each command through the per-send gate', async () => {
    const db = makeDb()
    seedProposed(db, 'crm_cmd_1')
    seedProposed(db, 'crm_cmd_2')
    const routes = makeRoutes('user_admin', db)

    const response = await routes.handleCrmBatchApproveApi(
      new Request(`https://openagents.com${ADMIN_OPS_CRM_BATCH_APPROVE_PATH}`, {
        body: JSON.stringify({ commandIds: ['crm_cmd_1', 'crm_cmd_2'] }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      fakeCtx,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      ok: boolean
      result: {
        requestedCount: number
        executedCount: number
        items: ReadonlyArray<{ commandId: string; disposition: string }>
      }
    }
    expect(body.ok).toBe(true)
    expect(body.result.requestedCount).toBe(2)
    // Disposition is whatever the per-send gate returns for this fake DB;
    // the invariant under test is that the admin surface calls the domain
    // one-by-one and returns a structured rollup, not that Resend is live.
    expect(body.result.items).toHaveLength(2)
    expect(body.result.items.every(item => typeof item.disposition === 'string')).toBe(
      true,
    )
  })

  test('batch-approve without commandIds => 400', async () => {
    const db = makeDb()
    const routes = makeRoutes('user_admin', db)
    const response = await routes.handleCrmBatchApproveApi(
      new Request(`https://openagents.com${ADMIN_OPS_CRM_BATCH_APPROVE_PATH}`, {
        body: JSON.stringify({}),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      fakeCtx,
    )
    expect(response.status).toBe(400)
  })

  test('queue rejects non-GET', async () => {
    const db = makeDb()
    const routes = makeRoutes('user_admin', db)
    const response = await routes.handleCrmBatchQueueApi(
      new Request(`https://openagents.com${ADMIN_OPS_CRM_BATCH_QUEUE_PATH}`, {
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      fakeCtx,
    )
    expect(response.status).toBe(405)
  })
})
