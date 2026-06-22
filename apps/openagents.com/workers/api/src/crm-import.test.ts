import { describe, expect, test } from 'vitest'

import { type CrmImportDeps, importCrmContactsFromCsv, parseCsv } from './crm-import'

describe('parseCsv', () => {
  test('parses quoted fields with embedded commas, newlines, and escaped quotes', () => {
    const csv = 'email,note\n"a@x.com","hello, ""world""\nsecond line"\nb@x.com,plain'
    const rows = parseCsv(csv)
    expect(rows).toHaveLength(3)
    expect(rows[0]).toEqual(['email', 'note'])
    expect(rows[1]?.[0]).toBe('a@x.com')
    expect(rows[1]?.[1]).toBe('hello, "world"\nsecond line')
    expect(rows[2]).toEqual(['b@x.com', 'plain'])
  })

  test('drops fully-empty trailing rows', () => {
    expect(parseCsv('email\na@x.com\n\n')).toEqual([['email'], ['a@x.com']])
  })
})

// In-memory deps that track contacts/accounts/memberships and produce honest
// created-vs-updated results so the import counters are genuinely exercised.
const makeFakeDeps = () => {
  const contacts = new Map<string, { id: string }>()
  const accounts = new Map<string, string>()
  const memberships: Array<{ contactId: string; listId: string }> = []
  let completion: Record<string, unknown> | null = null
  let counter = 0

  const deps: CrmImportDeps = {
    addMembership: async input => {
      memberships.push({ contactId: input.contactId, listId: input.listId })
    },
    completeRun: async input => {
      completion = input as unknown as Record<string, unknown>
    },
    ensureAccount: async input => {
      const key = `${input.tenantRef}:${input.name}`
      const existing = accounts.get(key)
      if (existing !== undefined) return { id: existing }
      counter += 1
      const id = `a_${counter}`
      accounts.set(key, id)
      return { id }
    },
    ensureList: async () => ({ id: 'list_1' }),
    startRun: async () => 'run_1',
    upsertContact: async input => {
      const key = `${input.tenantRef}:${input.primaryEmail}`
      const existing = contacts.get(key)
      if (existing !== undefined) return { contact: existing, created: false }
      counter += 1
      const contact = { id: `c_${counter}` }
      contacts.set(key, contact)
      return { contact, created: true }
    },
  }
  return { accounts, completion: () => completion, contacts, deps, memberships }
}

describe('importCrmContactsFromCsv', () => {
  test('counts new, within-file duplicate, invalid, and missing rows honestly', async () => {
    const fake = makeFakeDeps()
    const csv = [
      'email,first_name',
      'Ada@Example.com,Ada',
      'ada@example.com,Ada Dup', // within-file duplicate (normalized)
      'not-an-email,Bad',
      ',Missing',
      'bob@example.com,Bob',
    ].join('\n')

    const summary = await importCrmContactsFromCsv(fake.deps, {
      csv,
      sourceLabel: 'csv:test',
      tenantRef: 'tenant.openagents',
    })

    expect(summary.totalRows).toBe(5)
    expect(summary.importedRows).toBe(2)
    expect(summary.duplicateRows).toBe(1)
    expect(summary.failedRows).toBe(2)
    expect(summary.updatedRows).toBe(0)
    expect(fake.contacts.size).toBe(2)
    expect(summary.sampleEmails).toContain('ada@example.com')
    expect(summary.errors).toHaveLength(2)
    expect(fake.completion()?.status).toBe('completed')
  })

  test('re-importing the same email updates rather than re-creates', async () => {
    const fake = makeFakeDeps()
    const csv = 'email\nada@example.com'
    await importCrmContactsFromCsv(fake.deps, {
      csv,
      sourceLabel: 'csv:test',
      tenantRef: 'tenant.openagents',
    })
    const second = await importCrmContactsFromCsv(fake.deps, {
      csv,
      sourceLabel: 'csv:test',
      tenantRef: 'tenant.openagents',
    })
    expect(second.importedRows).toBe(0)
    expect(second.updatedRows).toBe(1)
  })

  test('maps human headers, derives accounts from company, and adds list membership', async () => {
    const fake = makeFakeDeps()
    const csv = [
      'Email,First Name,Last Name,Company',
      'ada@example.com,Ada,Lovelace,Analytical Engines',
      'bob@example.com,Bob,Jones,Analytical Engines',
    ].join('\n')

    const summary = await importCrmContactsFromCsv(fake.deps, {
      csv,
      listSlug: 'investors',
      sourceLabel: 'csv:test',
      tenantRef: 'tenant.openagents',
    })

    expect(summary.importedRows).toBe(2)
    // Both contacts share one company -> one account created.
    expect(fake.accounts.size).toBe(1)
    expect(fake.memberships).toHaveLength(2)
    expect(fake.memberships.every(m => m.listId === 'list_1')).toBe(true)
  })

  test('a header with no email column fails the whole run', async () => {
    const fake = makeFakeDeps()
    const summary = await importCrmContactsFromCsv(fake.deps, {
      csv: 'name,company\nAda,Engines',
      sourceLabel: 'csv:test',
      tenantRef: 'tenant.openagents',
    })
    expect(summary.importedRows).toBe(0)
    expect(summary.failedRows).toBe(1)
    expect(fake.completion()?.status).toBe('failed')
  })
})
