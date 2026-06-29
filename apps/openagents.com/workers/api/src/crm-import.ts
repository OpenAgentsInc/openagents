/**
 * One-time CSV contact import for the native CRM (epic #5980, sub-issue #5982).
 *
 * Parses an owner-provided CSV, de-dupes within the file by normalized email,
 * validates each row, and upserts contacts into the tenant-scoped D1 model
 * (#5981), recording a `crm_source_import_runs` audit row with honest
 * imported/updated/duplicate/failed counts. Optional: derive an account from a
 * company column and add every imported contact to a named list.
 *
 * The import logic depends on injected store operations (not a raw D1 handle)
 * so it is deterministic and unit-testable; `crmImportDepsFromDb` binds the real
 * `crm-store.ts` functions for production use. No email is sent here — import is
 * a pure data migration off the old prod DB (CSV handoff), with zero ongoing
 * legacy dependency.
 */
import {
  addCrmContactListMembership,
  completeCrmSourceImportRun,
  type CrmRuntime,
  defaultCrmRuntime,
  normalizeCrmEmail,
  startCrmSourceImportRun,
  upsertCrmAccount,
  upsertCrmContact,
  upsertCrmContactList,
} from './crm-store'

// ---------------------------------------------------------------------------
// CSV parsing
// ---------------------------------------------------------------------------

/**
 * Minimal RFC-4180-ish CSV parser: handles quoted fields, embedded commas and
 * newlines inside quotes, and doubled-quote escapes. Returns rows of string
 * cells. Blank trailing lines are dropped.
 */
export const parseCsv = (input: string): ReadonlyArray<ReadonlyArray<string>> => {
  const rows: Array<Array<string>> = []
  let field = ''
  let row: Array<string> = []
  let inQuotes = false
  let i = 0
  const text = input.replace(/\r\n/g, '\n').replace(/\r/g, '\n')

  const pushField = () => {
    row.push(field)
    field = ''
  }
  const pushRow = () => {
    pushField()
    rows.push(row)
    row = []
  }

  while (i < text.length) {
    const char = text[i]
    if (inQuotes) {
      if (char === '"') {
        if (text[i + 1] === '"') {
          field += '"'
          i += 2
          continue
        }
        inQuotes = false
        i += 1
        continue
      }
      field += char
      i += 1
      continue
    }
    if (char === '"') {
      inQuotes = true
      i += 1
      continue
    }
    if (char === ',') {
      pushField()
      i += 1
      continue
    }
    if (char === '\n') {
      pushRow()
      i += 1
      continue
    }
    field += char
    i += 1
  }
  // flush the final field/row if anything is pending
  if (field !== '' || row.length > 0) {
    pushRow()
  }
  // drop fully-empty rows (e.g. trailing newline)
  return rows.filter(cells => cells.some(cell => cell.trim() !== ''))
}

// ---------------------------------------------------------------------------
// Header mapping
// ---------------------------------------------------------------------------

type CanonicalField =
  | 'company'
  | 'email'
  | 'first_name'
  | 'full_name'
  | 'job_title'
  | 'last_name'
  | 'notes'
  | 'secondary_email'

const HEADER_SYNONYMS: Readonly<Record<CanonicalField, ReadonlyArray<string>>> = {
  company: ['company', 'account', 'organization', 'organisation', 'org', 'affiliation', 'fund'],
  email: ['email', 'primary_email', 'e_mail', 'email_address', 'work_email'],
  first_name: ['first_name', 'firstname', 'first', 'given_name'],
  full_name: ['full_name', 'name', 'fullname', 'display_name', 'contact_name'],
  job_title: ['job_title', 'title', 'role', 'position'],
  last_name: ['last_name', 'lastname', 'last', 'surname', 'family_name'],
  notes: ['notes', 'note', 'comment', 'comments'],
  secondary_email: ['secondary_email', 'alt_email', 'other_email'],
}

const normalizeHeader = (header: string): string =>
  header.trim().toLowerCase().replace(/[\s-]+/g, '_')

const buildHeaderIndex = (
  headers: ReadonlyArray<string>,
): Partial<Record<CanonicalField, number>> => {
  const index: Partial<Record<CanonicalField, number>> = {}
  headers.forEach((rawHeader, position) => {
    const normalized = normalizeHeader(rawHeader)
    for (const [canonical, synonyms] of Object.entries(HEADER_SYNONYMS) as Array<
      [CanonicalField, ReadonlyArray<string>]
    >) {
      if (index[canonical] === undefined && synonyms.includes(normalized)) {
        index[canonical] = position
      }
    }
  })
  return index
}

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

const isValidEmail = (email: string): boolean => EMAIL_PATTERN.test(email)

const cellAt = (
  row: ReadonlyArray<string>,
  position: number | undefined,
): string | null => {
  if (position === undefined) return null
  const value = row[position]
  if (value === undefined) return null
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

// ---------------------------------------------------------------------------
// Import deps (injectable for tests; bound to crm-store for production)
// ---------------------------------------------------------------------------

export type CrmImportDeps = Readonly<{
  startRun: (input: Readonly<{ sourceLabel: string; tenantRef: string }>) => Promise<string>
  completeRun: (input: Readonly<{
    duplicateRows: number
    failedRows: number
    id: string
    importedRows: number
    status: 'completed' | 'failed'
    totalRows: number
    updatedRows: number
    errorSummary?: string | null
  }>) => Promise<void>
  upsertContact: (input: Readonly<{
    accountId?: string | null
    externalSourceLabel?: string | null
    firstName?: string | null
    fullName?: string | null
    jobTitle?: string | null
    lastName?: string | null
    notes?: string | null
    primaryEmail: string
    secondaryEmail?: string | null
    tenantRef: string
  }>) => Promise<Readonly<{ contact: Readonly<{ id: string }>; created: boolean }>>
  ensureAccount: (input: Readonly<{ name: string; tenantRef: string }>) => Promise<Readonly<{ id: string }>>
  ensureList: (input: Readonly<{ name: string; slug: string; tenantRef: string }>) => Promise<Readonly<{ id: string }>>
  addMembership: (input: Readonly<{
    contactId: string
    listId: string
    source: string
    tenantRef: string
  }>) => Promise<void>
}>

export const crmImportDepsFromDb = (
  db: D1Database,
  runtime: CrmRuntime = defaultCrmRuntime,
): CrmImportDeps => ({
  addMembership: input => addCrmContactListMembership(db, input, runtime),
  completeRun: input => completeCrmSourceImportRun(db, input, runtime),
  ensureAccount: async input => {
    const account = await upsertCrmAccount(db, input, runtime)
    return { id: account.id }
  },
  ensureList: async input => {
    const list = await upsertCrmContactList(db, input, runtime)
    return { id: list.id }
  },
  startRun: input => startCrmSourceImportRun(db, input, runtime),
  upsertContact: input => upsertCrmContact(db, input, runtime),
})

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

export type CrmImportInput = Readonly<{
  csv: string
  sourceLabel: string
  tenantRef: string
  listName?: string | null
  listSlug?: string | null
}>

export type CrmImportSummary = Readonly<{
  runId: string
  totalRows: number
  importedRows: number
  updatedRows: number
  duplicateRows: number
  failedRows: number
  sampleEmails: ReadonlyArray<string>
  errors: ReadonlyArray<Readonly<{ line: number; reason: string }>>
}>

export const importCrmContactsFromCsv = async (
  deps: CrmImportDeps,
  input: CrmImportInput,
): Promise<CrmImportSummary> => {
  const runId = await deps.startRun({
    sourceLabel: input.sourceLabel,
    tenantRef: input.tenantRef,
  })

  const parsed = parseCsv(input.csv)
  if (parsed.length === 0) {
    await deps.completeRun({
      duplicateRows: 0,
      failedRows: 0,
      id: runId,
      importedRows: 0,
      status: 'completed',
      totalRows: 0,
      updatedRows: 0,
    })
    return {
      duplicateRows: 0,
      errors: [],
      failedRows: 0,
      importedRows: 0,
      runId,
      sampleEmails: [],
      totalRows: 0,
      updatedRows: 0,
    }
  }

  const [headerRow, ...dataRows] = parsed
  const headerIndex = buildHeaderIndex(headerRow ?? [])

  if (headerIndex.email === undefined) {
    await deps.completeRun({
      duplicateRows: 0,
      errorSummary: 'no recognizable email column',
      failedRows: dataRows.length,
      id: runId,
      importedRows: 0,
      status: 'failed',
      totalRows: dataRows.length,
      updatedRows: 0,
    })
    return {
      duplicateRows: 0,
      errors: [{ line: 1, reason: 'no recognizable email column in header' }],
      failedRows: dataRows.length,
      importedRows: 0,
      runId,
      sampleEmails: [],
      totalRows: dataRows.length,
      updatedRows: 0,
    }
  }

  // Optional list created once up front.
  let listId: string | null = null
  if (input.listSlug !== undefined && input.listSlug !== null && input.listSlug.trim() !== '') {
    const list = await deps.ensureList({
      name: (input.listName ?? input.listSlug).trim(),
      slug: input.listSlug.trim(),
      tenantRef: input.tenantRef,
    })
    listId = list.id
  }

  const seen = new Set<string>()
  const accountCache = new Map<string, string>()
  const sampleEmails: Array<string> = []
  const errors: Array<{ line: number; reason: string }> = []
  let importedRows = 0
  let updatedRows = 0
  let duplicateRows = 0
  let failedRows = 0

  for (let r = 0; r < dataRows.length; r += 1) {
    const row = dataRows[r] ?? []
    const lineNumber = r + 2 // 1-based, accounting for the header row
    const rawEmail = cellAt(row, headerIndex.email)

    if (rawEmail === null) {
      failedRows += 1
      errors.push({ line: lineNumber, reason: 'missing email' })
      continue
    }
    const email = normalizeCrmEmail(rawEmail)
    if (!isValidEmail(email)) {
      failedRows += 1
      errors.push({ line: lineNumber, reason: `invalid email: ${rawEmail}` })
      continue
    }
    if (seen.has(email)) {
      duplicateRows += 1
      continue
    }
    seen.add(email)

    let accountId: string | null = null
    const company = cellAt(row, headerIndex.company)
    if (company !== null) {
      const cached = accountCache.get(company)
      if (cached !== undefined) {
        accountId = cached
      } else {
        const account = await deps.ensureAccount({ name: company, tenantRef: input.tenantRef })
        accountCache.set(company, account.id)
        accountId = account.id
      }
    }

    try {
      const result = await deps.upsertContact({
        accountId,
        externalSourceLabel: input.sourceLabel,
        firstName: cellAt(row, headerIndex.first_name),
        fullName: cellAt(row, headerIndex.full_name),
        jobTitle: cellAt(row, headerIndex.job_title),
        lastName: cellAt(row, headerIndex.last_name),
        notes: cellAt(row, headerIndex.notes),
        primaryEmail: email,
        secondaryEmail: cellAt(row, headerIndex.secondary_email),
        tenantRef: input.tenantRef,
      })
      if (result.created) {
        importedRows += 1
      } else {
        updatedRows += 1
      }
      if (sampleEmails.length < 5) {
        sampleEmails.push(email)
      }
      if (listId !== null) {
        await deps.addMembership({
          contactId: result.contact.id,
          listId,
          source: 'csv_import',
          tenantRef: input.tenantRef,
        })
      }
    } catch (error) {
      failedRows += 1
      errors.push({ line: lineNumber, reason: String(error) })
    }
  }

  await deps.completeRun({
    duplicateRows,
    failedRows,
    id: runId,
    importedRows,
    status: 'completed',
    totalRows: dataRows.length,
    updatedRows,
  })

  return {
    duplicateRows,
    errors,
    failedRows,
    importedRows,
    runId,
    sampleEmails,
    totalRows: dataRows.length,
    updatedRows,
  }
}
