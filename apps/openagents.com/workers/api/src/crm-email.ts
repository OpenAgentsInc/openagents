/**
 * CRM email templates, rendering, and per-contact send ledger
 * (epic #5980, sub-issue #5983; shared by the Resend channel in #5984 and the
 * unified send abstraction in #5985).
 *
 * - Template store over `crm_email_templates` (migration 0219).
 * - A tiny, safe `{{ path }}` renderer + minimal markdown→HTML.
 * - `composeCrmEmailForContact`: resolve a contact + template into a
 *   personalized, channel-ready message.
 * - Send ledger over `crm_email_messages`: record a draft/sent message and
 *   update its delivery state (used by the Gmail/gws write-back and Resend).
 *
 * No transport here — composing/recording only. The Gmail executor (local) and
 * the Resend sender (#5984) call these.
 */
import { Schema as S } from 'effect'

// KS-8.11 (#8322): CrmEmailDatabase union — plain D1 keeps working; the
// dual-write handle mirrors template/message writes to Postgres fail-soft.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import {
  type CrmContact,
  type CrmRuntime,
  defaultCrmRuntime,
  getCrmContactById,
} from './crm-store'

export class CrmEmailError extends S.TaggedErrorClass<CrmEmailError>()(
  'CrmEmailError',
  {
    reason: S.String,
  },
) {}

export type CrmSendChannel = 'gmail_gws' | 'resend'

export type CrmEmailTemplate = Readonly<{
  id: string
  tenantRef: string
  slug: string
  name: string
  subjectTemplate: string
  bodyMarkdownTemplate: string
  status: string
  createdAt: string
  updatedAt: string
}>

export type CrmEmailMessage = Readonly<{
  id: string
  tenantRef: string
  contactId: string
  templateId: string | null
  channel: CrmSendChannel
  fromEmail: string | null
  toEmail: string
  subject: string
  bodyMarkdown: string
  bodyHtml: string | null
  status: string
  sendReason: string | null
  providerMessageId: string | null
  providerDraftId: string | null
  errorMessage: string | null
  sentAt: string | null
  createdAt: string
  updatedAt: string
}>

export type ComposedCrmEmail = Readonly<{
  contact: CrmContact
  template: CrmEmailTemplate
  toEmail: string
  subject: string
  bodyMarkdown: string
  bodyHtml: string
}>

const str = (v: unknown): string =>
  v === null || v === undefined ? '' : String(v)
const nullableStr = (v: unknown): string | null =>
  v === null || v === undefined ? null : String(v)

const decodeTemplate = (row: Record<string, unknown>): CrmEmailTemplate => ({
  bodyMarkdownTemplate: str(row.body_markdown_template),
  createdAt: str(row.created_at),
  id: str(row.id),
  name: str(row.name),
  slug: str(row.slug),
  status: str(row.status),
  subjectTemplate: str(row.subject_template),
  tenantRef: str(row.tenant_ref),
  updatedAt: str(row.updated_at),
})

const decodeMessage = (row: Record<string, unknown>): CrmEmailMessage => ({
  bodyHtml: nullableStr(row.body_html),
  bodyMarkdown: str(row.body_markdown),
  channel: (str(row.channel) === 'resend'
    ? 'resend'
    : 'gmail_gws') as CrmSendChannel,
  contactId: str(row.contact_id),
  createdAt: str(row.created_at),
  errorMessage: nullableStr(row.error_message),
  fromEmail: nullableStr(row.from_email),
  id: str(row.id),
  providerDraftId: nullableStr(row.provider_draft_id),
  providerMessageId: nullableStr(row.provider_message_id),
  sendReason: nullableStr(row.send_reason),
  sentAt: nullableStr(row.sent_at),
  status: str(row.status),
  subject: str(row.subject),
  templateId: nullableStr(row.template_id),
  tenantRef: str(row.tenant_ref),
  toEmail: str(row.to_email),
  updatedAt: str(row.updated_at),
})

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

export type CrmRenderContext = Readonly<{
  appBaseUrl: string
  appName: string
}>

export const defaultCrmRenderContext: CrmRenderContext = {
  appBaseUrl: 'https://openagents.com',
  appName: 'OpenAgents',
}

const firstNameOf = (contact: CrmContact): string => {
  if (contact.firstName !== null && contact.firstName.trim() !== '') {
    return contact.firstName.trim()
  }
  if (contact.fullName !== null && contact.fullName.trim() !== '') {
    return contact.fullName.trim().split(/\s+/)[0] ?? ''
  }
  return ''
}

const buildTokens = (
  contact: CrmContact,
  context: CrmRenderContext,
): Readonly<Record<string, string>> => {
  const first = firstNameOf(contact)
  return {
    'app.base_url': context.appBaseUrl,
    'app.name': context.appName,
    'contact.first_name': first,
    'contact.first_name_or_there': first === '' ? 'there' : first,
    'contact.full_name': contact.fullName ?? '',
    'contact.job_title': contact.jobTitle ?? '',
    'contact.last_name': contact.lastName ?? '',
    'contact.primary_email': contact.primaryEmail,
  }
}

/** Replace `{{ token }}` occurrences; unknown tokens render to empty string. */
export const renderCrmTemplateString = (
  template: string,
  tokens: Readonly<Record<string, string>>,
): string =>
  template.replace(/\{\{\s*([a-zA-Z0-9_.]+)\s*\}\}/g, (_match, key: string) =>
    Object.prototype.hasOwnProperty.call(tokens, key)
      ? (tokens[key] ?? '')
      : '',
  )

const escapeHtml = (text: string): string =>
  text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/**
 * Minimal, safe markdown→HTML: escapes first, then applies `**bold**`,
 * `[text](url)` links, and paragraph/line breaks. Intentionally small — rich
 * templating is not the point; personalization + safety is.
 */
export const crmMarkdownToHtml = (markdown: string): string => {
  const paragraphs = markdown.split(/\n{2,}/)
  return paragraphs
    .map(block => {
      const escaped = escapeHtml(block)
        .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
        .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2">$1</a>')
        .replace(/\n/g, '<br />')
      return `<p>${escaped}</p>`
    })
    .join('\n')
}

export const renderCrmEmail = (
  template: CrmEmailTemplate,
  contact: CrmContact,
  context: CrmRenderContext = defaultCrmRenderContext,
): Readonly<{ bodyHtml: string; bodyMarkdown: string; subject: string }> => {
  const tokens = buildTokens(contact, context)
  const subject = renderCrmTemplateString(template.subjectTemplate, tokens)
  const bodyMarkdown = renderCrmTemplateString(
    template.bodyMarkdownTemplate,
    tokens,
  )
  return { bodyHtml: crmMarkdownToHtml(bodyMarkdown), bodyMarkdown, subject }
}

// ---------------------------------------------------------------------------
// Template store
// ---------------------------------------------------------------------------

const wrap = async (
  operation: string,
  fn: () => Promise<unknown>,
): Promise<void> => {
  try {
    await fn()
  } catch (error) {
    throw new CrmEmailError({ reason: `${operation}: ${String(error)}` })
  }
}

export const upsertCrmEmailTemplate = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    slug: string
    name: string
    subjectTemplate: string
    bodyMarkdownTemplate: string
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmEmailTemplate> => {
  const now = runtime.nowIso()
  await wrap('crm.upsertTemplate', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_email_templates (
           id, tenant_ref, slug, name, subject_template, body_markdown_template,
           status, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?)
         ON CONFLICT(tenant_ref, slug) DO UPDATE SET
           name = excluded.name,
           subject_template = excluded.subject_template,
           body_markdown_template = excluded.body_markdown_template,
           status = 'active',
           updated_at = excluded.updated_at`,
      )
      .bind(
        runtime.makeId('crm_template'),
        input.tenantRef,
        input.slug,
        input.name,
        input.subjectTemplate,
        input.bodyMarkdownTemplate,
        now,
        now,
      )
      .run(),
  )
  const stored = await getCrmEmailTemplateBySlug(
    db,
    input.tenantRef,
    input.slug,
  )
  if (stored === null) {
    throw new CrmEmailError({
      reason: 'crm.upsertTemplate: template vanished after upsert',
    })
  }
  await mirrorCrmEmailRows(db, 'crm_email_templates', 'id', [stored.id])
  return stored
}

export const getCrmEmailTemplateBySlug = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  slug: string,
): Promise<CrmEmailTemplate | null> => {
  try {
    const row = await crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_email_templates WHERE tenant_ref = ? AND slug = ? AND archived_at IS NULL LIMIT 1',
      )
      .bind(tenantRef, slug)
      .first<Record<string, unknown>>()
    return row === null ? null : decodeTemplate(row)
  } catch (error) {
    throw new CrmEmailError({ reason: `crm.getTemplate: ${String(error)}` })
  }
}

export const listCrmEmailTemplates = async (
  db: CrmEmailDatabase,
  tenantRef: string,
): Promise<ReadonlyArray<CrmEmailTemplate>> => {
  try {
    const result = await crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_email_templates WHERE tenant_ref = ? AND archived_at IS NULL ORDER BY created_at DESC',
      )
      .bind(tenantRef)
      .all<Record<string, unknown>>()
    return (result.results ?? []).map(decodeTemplate)
  } catch (error) {
    throw new CrmEmailError({ reason: `crm.listTemplates: ${String(error)}` })
  }
}

// ---------------------------------------------------------------------------
// Compose
// ---------------------------------------------------------------------------

export const composeCrmEmailForContact = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    contactId: string
    templateSlug: string
    renderContext?: CrmRenderContext
  }>,
): Promise<ComposedCrmEmail> => {
  const contact = await getCrmContactById(db, input.tenantRef, input.contactId)
  if (contact === null) {
    throw new CrmEmailError({ reason: `contact not found: ${input.contactId}` })
  }
  const template = await getCrmEmailTemplateBySlug(
    db,
    input.tenantRef,
    input.templateSlug,
  )
  if (template === null) {
    throw new CrmEmailError({
      reason: `template not found: ${input.templateSlug}`,
    })
  }
  const rendered = renderCrmEmail(template, contact, input.renderContext)
  return {
    bodyHtml: rendered.bodyHtml,
    bodyMarkdown: rendered.bodyMarkdown,
    contact,
    subject: rendered.subject,
    template,
    toEmail: contact.primaryEmail,
  }
}

// ---------------------------------------------------------------------------
// Send ledger
// ---------------------------------------------------------------------------

export type RecordCrmEmailMessageInput = Readonly<{
  tenantRef: string
  contactId: string
  channel: CrmSendChannel
  toEmail: string
  subject: string
  bodyMarkdown: string
  bodyHtml?: string | null
  status: 'draft' | 'queued' | 'sent' | 'failed'
  templateId?: string | null
  fromEmail?: string | null
  sendReason?: string | null
  providerMessageId?: string | null
  providerDraftId?: string | null
  errorMessage?: string | null
  sentAt?: string | null
}>

export const recordCrmEmailMessage = async (
  db: CrmEmailDatabase,
  input: RecordCrmEmailMessageInput,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<CrmEmailMessage> => {
  const id = runtime.makeId('crm_email')
  const now = runtime.nowIso()
  await wrap('crm.recordEmailMessage', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO crm_email_messages (
           id, tenant_ref, contact_id, template_id, channel, from_email,
           to_email, subject, body_markdown, body_html, status, send_reason,
           provider_message_id, provider_draft_id, error_message, sent_at,
           created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        id,
        input.tenantRef,
        input.contactId,
        input.templateId ?? null,
        input.channel,
        input.fromEmail ?? null,
        input.toEmail,
        input.subject,
        input.bodyMarkdown,
        input.bodyHtml ?? null,
        input.status,
        input.sendReason ?? null,
        input.providerMessageId ?? null,
        input.providerDraftId ?? null,
        input.errorMessage ?? null,
        input.sentAt ?? (input.status === 'sent' ? now : null),
        now,
        now,
      )
      .run(),
  )
  const stored = await getCrmEmailMessageById(db, input.tenantRef, id)
  if (stored === null) {
    throw new CrmEmailError({
      reason: 'crm.recordEmailMessage: row vanished after insert',
    })
  }
  await mirrorCrmEmailRows(db, 'crm_email_messages', 'id', [id])
  return stored
}

export const updateCrmEmailMessageDelivery = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    id: string
    status: 'draft' | 'queued' | 'sent' | 'failed'
    providerMessageId?: string | null
    providerDraftId?: string | null
    errorMessage?: string | null
    sentAt?: string | null
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<void> => {
  const now = runtime.nowIso()
  await wrap('crm.updateEmailMessageDelivery', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE crm_email_messages SET
           status = ?,
           provider_message_id = COALESCE(?, provider_message_id),
           provider_draft_id = COALESCE(?, provider_draft_id),
           error_message = COALESCE(?, error_message),
           sent_at = COALESCE(?, sent_at),
           updated_at = ?
         WHERE tenant_ref = ? AND id = ?`,
      )
      .bind(
        input.status,
        input.providerMessageId ?? null,
        input.providerDraftId ?? null,
        input.errorMessage ?? null,
        input.sentAt ?? (input.status === 'sent' ? now : null),
        now,
        input.tenantRef,
        input.id,
      )
      .run(),
  )
  await mirrorCrmEmailRows(db, 'crm_email_messages', 'id', [input.id])
}

export const getCrmEmailMessageById = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  id: string,
): Promise<CrmEmailMessage | null> => {
  try {
    const row = await crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_email_messages WHERE tenant_ref = ? AND id = ? LIMIT 1',
      )
      .bind(tenantRef, id)
      .first<Record<string, unknown>>()
    return row === null ? null : decodeMessage(row)
  } catch (error) {
    throw new CrmEmailError({ reason: `crm.getEmailMessage: ${String(error)}` })
  }
}

/**
 * Queued Gmail/gws messages awaiting the LOCAL executor (#5987). The unified
 * dispatch (#5985) records a `gmail_gws` row as `queued`; the desktop/local
 * executor lists them, sends via `gws`, and writes the outcome back.
 */
export const listCrmQueuedGmailMessages = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  query: Readonly<{ limit?: number | undefined }> = {},
): Promise<ReadonlyArray<CrmEmailMessage>> => {
  const limit =
    query.limit === undefined ||
    !Number.isFinite(query.limit) ||
    query.limit <= 0
      ? 100
      : Math.min(Math.floor(query.limit), 500)
  try {
    const result = await crmEmailAuthorityDb(db)
      .prepare(
        `SELECT * FROM crm_email_messages
           WHERE tenant_ref = ? AND channel = 'gmail_gws' AND status = 'queued'
           ORDER BY created_at ASC LIMIT ?`,
      )
      .bind(tenantRef, limit)
      .all<Record<string, unknown>>()
    return (result.results ?? []).map(decodeMessage)
  } catch (error) {
    throw new CrmEmailError({ reason: `crm.listQueuedGmail: ${String(error)}` })
  }
}

export const listCrmEmailMessagesForContact = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  contactId: string,
  query: Readonly<{ limit?: number | undefined }> = {},
): Promise<ReadonlyArray<CrmEmailMessage>> => {
  const limit =
    query.limit === undefined ||
    !Number.isFinite(query.limit) ||
    query.limit <= 0
      ? 100
      : Math.min(Math.floor(query.limit), 500)
  try {
    const result = await crmEmailAuthorityDb(db)
      .prepare(
        'SELECT * FROM crm_email_messages WHERE tenant_ref = ? AND contact_id = ? ORDER BY created_at DESC LIMIT ?',
      )
      .bind(tenantRef, contactId, limit)
      .all<Record<string, unknown>>()
    return (result.results ?? []).map(decodeMessage)
  } catch (error) {
    throw new CrmEmailError({
      reason: `crm.listEmailMessages: ${String(error)}`,
    })
  }
}
