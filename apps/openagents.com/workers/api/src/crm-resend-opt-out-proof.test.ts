import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'

import { type CrmRuntime } from './crm-store'
import { recordCrmReplyEvent } from './crm-reply'
import {
  type CrmResendSender,
  sendCrmEmailViaResend,
} from './crm-resend'
import { makeSqliteD1 } from './test/sqlite-d1'

const TENANT_REF = 'tenant.openagents'
const NOW = '2026-07-09T02:40:00.000Z'

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeRuntime = (): CrmRuntime => {
  let counter = 0
  return {
    makeId: prefix => `${prefix}_${++counter}`,
    nowIso: () => NOW,
  }
}

const seedCrmEmailProofSchema = (exec: (sql: string) => void): void => {
  exec('CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL);')
  exec(migration('0063_email_campaign_records.sql'))
  exec(migration('0218_crm_contacts.sql'))
  exec(migration('0219_crm_email_templates_and_messages.sql'))
  exec(migration('0310_crm_command_batches_and_replies.sql'))

  exec(`
    INSERT INTO crm_contacts (
      id, tenant_ref, primary_email, full_name, first_name,
      created_at, updated_at
    ) VALUES (
      'crm_contact_1', '${TENANT_REF}', 'ada@example.com', 'Ada Lovelace', 'Ada',
      '${NOW}', '${NOW}'
    );

    INSERT INTO crm_email_templates (
      id, tenant_ref, slug, name, subject_template, body_markdown_template,
      status, created_at, updated_at
    ) VALUES (
      'crm_template_1',
      '${TENANT_REF}',
      'ob1-proof',
      'OB-1 proof',
      'Hi {{ contact.first_name_or_there }}',
      'Hello {{ contact.first_name_or_there }},\\n\\nSarah can help.',
      'active',
      '${NOW}',
      '${NOW}'
    );
  `)
}

describe('CRM Resend OB-1 opt-out proof', () => {
  test('reply opt-out suppresses an armed Sarah/Resend send before any provider call', async () => {
    const sqlite = makeSqliteD1()
    const runtime = makeRuntime()
    const senderCalls: Array<unknown> = []
    const sender: CrmResendSender = async input => {
      senderCalls.push(input)
      return { ok: true, providerMessageId: 'resend_should_not_send' }
    }

    try {
      seedCrmEmailProofSchema(sqlite.exec)

      const reply = await recordCrmReplyEvent(
        sqlite.db,
        {
          bodyText: 'Please unsubscribe me from future emails.',
          fromEmail: 'ADA@EXAMPLE.COM',
          provider: 'resend',
          providerEventId: 'resend.event.unsubscribe.1',
          subject: 'Stop',
          tenantRef: TENANT_REF,
        },
        runtime,
      )

      expect(reply).toMatchObject({
        contactId: 'crm_contact_1',
        duplicate: false,
        optOut: true,
        routedTo: 'operator_notification',
      })

      const suppressions = await sqlite.db
        .prepare(
          `SELECT email, reason, scope, active, source_authority_ref
             FROM email_suppression_entries
            ORDER BY created_at ASC`,
        )
        .all<{
          active: number
          email: string
          reason: string
          scope: string
          source_authority_ref: string
        }>()
      expect(suppressions.results).toEqual([
        {
          active: 1,
          email: 'ada@example.com',
          reason: 'unsubscribe',
          scope: 'all',
          source_authority_ref: `crm.reply_event:${reply.replyEventId}`,
        },
      ])

      const result = await sendCrmEmailViaResend(
        sqlite.db,
        {
          enabled: true,
          fromEmail: 'Sarah <sarah@openagents.com>',
          sender,
        },
        {
          contactId: 'crm_contact_1',
          sendReason: 'ob1_opt_out_proof',
          templateSlug: 'ob1-proof',
          tenantRef: TENANT_REF,
        },
        runtime,
      )

      expect(result).toEqual({
        kind: 'suppressed',
        reason: 'all_suppressed',
        toEmail: 'ada@example.com',
      })
      expect(senderCalls).toHaveLength(0)

      const emailMessages = await sqlite.db
        .prepare('SELECT id FROM crm_email_messages')
        .all<{ id: string }>()
      expect(emailMessages.results).toEqual([])
    } finally {
      sqlite.close()
    }
  })
})
