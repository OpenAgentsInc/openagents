/**
 * CRM MCP tool catalog — Wave 1, read-only (epic #5991, sub-issue #5993).
 *
 * Builds the catalog the transport (#5992) delegates to: schema-bound MCP tool
 * descriptors (from `@openagentsinc/mcp-contract`) for the read surface of the
 * CRM, dispatch to the existing `crm-*` store/read functions, and result
 * projection through the contract's output-safety helpers. No new authority —
 * each tool wraps a read the admin HTTP routes already expose. All
 * `operator_read`, `read_only`, no receipts. Resources land in #5994; grant
 * filtering + tenant binding in #5995.
 */
// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import {
  type CrmEmailDatabase,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import {
  filterOpenAgentsMcpDescriptorsByGrantSet,
  openAgentsMcpDescriptorIsGranted,
  openAgentsMcpGrantedAuthoritySet,
  type OpenAgentsMcpReceipt,
  type OpenAgentsMcpReceiptKind,
  type OpenAgentsMcpToolDescriptor,
  parseOpenAgentsMcpResourceUri,
  projectOpenAgentsMcpOutput,
} from '@openagentsinc/mcp-contract'

import {
  composeCrmEmailForContact,
  listCrmEmailMessagesForContact,
  listCrmEmailTemplates,
  listCrmQueuedGmailMessages,
  upsertCrmEmailTemplate,
} from './crm-email'
import {
  approveAndExecuteCrmSendCommand,
  listCrmCommands,
  proposeCrmSendCommand,
  rejectCrmCommand,
} from './crm-command'
import { runCrmBatch } from './crm-batch'
import { crmImportDepsFromDb, importCrmContactsFromCsv } from './crm-import'
import type { CrmDispatchDeps } from './crm-send'
import type {
  CrmMcpCatalog,
  McpResourceListing,
  McpResourceReadOutcome,
  McpToolCallOutcome,
  McpToolListing,
} from './crm-mcp-routes'
import {
  getCrmAccountById,
  getCrmContactById,
  getCrmEngagementSnapshot,
  getCrmOpportunityById,
  listCrmAccounts,
  listCrmActivitiesForContact,
  listCrmContactLists,
  listCrmContacts,
  listCrmOpportunities,
  listCrmSourceImportRuns,
} from './crm-store'
import { readEmailSendEligibility } from './email-preferences'
import { stringArrayFromUnknown } from './json-boundary'

type CrmMcpEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

const SOURCE = 'docs/mcp/2026-06-22-crm-mcp-server-phase-1-audit.md'

/**
 * Typed tool error. The transport (crm-mcp-routes) inspects `instanceof Error` +
 * `.message` (e.g. `unknown_tool`) to shape an isError result, so this stays an
 * Error subclass rather than a generic thrown Error.
 */
class CrmMcpToolError extends Error {}

// --- input arg helpers ------------------------------------------------------

const args = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : {}

const requiredId = (a: Record<string, unknown>, key: string): string => {
  const value = a[key]
  if (typeof value !== 'string' || value.trim() === '') {
    throw new CrmMcpToolError(`${key} is required`)
  }
  return value.trim()
}

const optLimit = (a: Record<string, unknown>): number | undefined => {
  const value = a.limit
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

const optSearch = (a: Record<string, unknown>): string | undefined =>
  typeof a.search === 'string' ? a.search : undefined

// --- JSON Schemas emitted in tools/list (the wire `inputSchema`) ------------

const TENANT_PROP = { description: 'Tenant ref (defaults to the bound tenant).', type: 'string' }
const LIMIT_PROP = { description: 'Max rows to return.', type: 'integer' }

const listSchema = (): Record<string, unknown> => ({
  additionalProperties: false,
  properties: { limit: LIMIT_PROP, tenant: TENANT_PROP },
  type: 'object',
})

const idSchema = (idKey: string, idDesc: string): Record<string, unknown> => ({
  additionalProperties: false,
  properties: { [idKey]: { description: idDesc, type: 'string' }, tenant: TENANT_PROP },
  required: [idKey],
  type: 'object',
})

const CRM_MCP_INPUT_SCHEMAS: Readonly<Record<string, Record<string, unknown>>> = {
  'crm.send.command.propose': {
    additionalProperties: false,
    properties: {
      channel: { description: "Send channel.", enum: ['gmail_gws', 'resend'], type: 'string' },
      contactId: { description: 'CRM contact id.', type: 'string' },
      sendReason: { description: 'Optional reason recorded with the command.', type: 'string' },
      templateSlug: { description: 'Template slug to send.', type: 'string' },
      tenant: TENANT_PROP,
    },
    required: ['contactId', 'templateSlug'],
    type: 'object',
  },
  'crm.send.command.approve': {
    additionalProperties: false,
    properties: { commandId: { description: 'Command id to approve + execute.', type: 'string' }, tenant: TENANT_PROP },
    required: ['commandId'],
    type: 'object',
  },
  'crm.send.command.reject': {
    additionalProperties: false,
    properties: {
      commandId: { description: 'Command id to reject.', type: 'string' },
      reason: { description: 'Optional rejection reason.', type: 'string' },
      tenant: TENANT_PROP,
    },
    required: ['commandId'],
    type: 'object',
  },
  'crm.import.run': {
    additionalProperties: false,
    properties: {
      csv: { description: 'CSV text (header row + contacts).', type: 'string' },
      listName: { description: 'Optional list display name.', type: 'string' },
      listSlug: { description: 'Optional list slug to add imported contacts to.', type: 'string' },
      sourceLabel: { description: 'Audit label for the import run.', type: 'string' },
      tenant: TENANT_PROP,
    },
    required: ['csv'],
    type: 'object',
  },
  'crm.batch.send': {
    additionalProperties: false,
    properties: {
      channel: { description: 'Send channel.', enum: ['gmail_gws', 'resend'], type: 'string' },
      contactIds: { description: 'Contact ids to plan a send for.', items: { type: 'string' }, type: 'array' },
      sendReason: { description: 'Optional reason.', type: 'string' },
      templateSlug: { description: 'Template slug.', type: 'string' },
      tenant: TENANT_PROP,
    },
    required: ['contactIds', 'templateSlug'],
    type: 'object',
  },
  'crm.template.upsert': {
    additionalProperties: false,
    properties: {
      bodyMarkdownTemplate: { description: 'Markdown body template ({{ contact.first_name }} etc.).', type: 'string' },
      name: { description: 'Human template name.', type: 'string' },
      slug: { description: 'Template slug (stable id within the tenant).', type: 'string' },
      subjectTemplate: { description: 'Subject template.', type: 'string' },
      tenant: TENANT_PROP,
    },
    required: ['slug', 'name', 'subjectTemplate', 'bodyMarkdownTemplate'],
    type: 'object',
  },
  'crm.account.get': idSchema('accountId', 'CRM account id.'),
  'crm.accounts.list': listSchema(),
  'crm.commands.list': {
    additionalProperties: false,
    properties: {
      limit: LIMIT_PROP,
      status: { description: 'Filter by command status (e.g. proposed).', type: 'string' },
      tenant: TENANT_PROP,
    },
    type: 'object',
  },
  'crm.contact.activities.list': idSchema('contactId', 'CRM contact id.'),
  'crm.contact.emails.list': idSchema('contactId', 'CRM contact id.'),
  'crm.contact.engagement.get': idSchema('contactId', 'CRM contact id.'),
  'crm.contact.get': idSchema('contactId', 'CRM contact id.'),
  'crm.contact.render': {
    additionalProperties: false,
    properties: {
      contactId: { description: 'CRM contact id.', type: 'string' },
      template: { description: 'Template slug to render.', type: 'string' },
      tenant: TENANT_PROP,
    },
    required: ['contactId', 'template'],
    type: 'object',
  },
  'crm.contacts.list': {
    additionalProperties: false,
    properties: {
      limit: LIMIT_PROP,
      search: { description: 'Substring match on email or full name.', type: 'string' },
      tenant: TENANT_PROP,
    },
    type: 'object',
  },
  'crm.gmail.queue.list': listSchema(),
  'crm.imports.list': listSchema(),
  'crm.lists.list': { additionalProperties: false, properties: { tenant: TENANT_PROP }, type: 'object' },
  'crm.opportunities.list': listSchema(),
  'crm.opportunity.get': idSchema('opportunityId', 'CRM opportunity id.'),
  'crm.templates.list': { additionalProperties: false, properties: { tenant: TENANT_PROP }, type: 'object' },
}

// --- tool descriptors -------------------------------------------------------

const readTool = (
  name: string,
  title: string,
  description: string,
): OpenAgentsMcpToolDescriptor => ({
  description,
  inputSchemaRef: `${name}.input`,
  name,
  outputSchemaRef: `${name}.output`,
  progressBehavior: 'none',
  publicSummary: title,
  receiptBehavior: 'none',
  requiredAuthorities: ['operator_read'],
  riskClass: 'read_only',
  sourceRefs: [SOURCE],
  title,
})

export const CRM_MCP_READ_TOOLS: ReadonlyArray<OpenAgentsMcpToolDescriptor> = [
  readTool('crm.contacts.list', 'List contacts', 'List CRM contacts for the tenant (optional search + limit).'),
  readTool('crm.contact.get', 'Get contact', 'Read one CRM contact by id.'),
  readTool('crm.contact.activities.list', 'List contact activities', "Read a contact's activity timeline."),
  readTool('crm.contact.engagement.get', 'Get contact engagement', "Read a contact's engagement snapshot."),
  readTool('crm.contact.emails.list', 'List contact emails', "Read a contact's CRM send ledger."),
  readTool('crm.accounts.list', 'List accounts', 'List CRM accounts for the tenant.'),
  readTool('crm.account.get', 'Get account', 'Read one CRM account by id.'),
  readTool('crm.lists.list', 'List contact lists', 'List CRM contact lists (segments).'),
  readTool('crm.opportunities.list', 'List opportunities', 'List CRM opportunities for the tenant.'),
  readTool('crm.opportunity.get', 'Get opportunity', 'Read one CRM opportunity by id.'),
  readTool('crm.imports.list', 'List import runs', 'List CRM CSV import-run audit rows.'),
  readTool('crm.templates.list', 'List email templates', 'List CRM email templates for the tenant.'),
  readTool('crm.contact.render', 'Render contact email', 'Compose a personalized email preview for a contact + report send eligibility. Sends nothing.'),
  readTool('crm.gmail.queue.list', 'List Gmail queue', 'List queued Gmail (gws) messages awaiting the local executor.'),
  readTool('crm.commands.list', 'List send commands', 'List approval-gated send_email commands (e.g. status=proposed).'),
]

// --- Wave 2 write tools (propose-only; no send) -----------------------------

const writeTool = (
  name: string,
  title: string,
  description: string,
  requiredAuthorities: ReadonlyArray<OpenAgentsMcpToolDescriptor['requiredAuthorities'][number]>,
): OpenAgentsMcpToolDescriptor => ({
  description,
  inputSchemaRef: `${name}.input`,
  name,
  outputSchemaRef: `${name}.output`,
  progressBehavior: 'none',
  publicSummary: title,
  receiptBehavior: 'mutation_receipt',
  requiredAuthorities,
  riskClass: 'low',
  sourceRefs: [SOURCE],
  title,
})

const approvalTool = (
  name: string,
  title: string,
  description: string,
): OpenAgentsMcpToolDescriptor => ({
  description,
  inputSchemaRef: `${name}.input`,
  name,
  outputSchemaRef: `${name}.output`,
  progressBehavior: 'none',
  publicSummary: title,
  receiptBehavior: 'approval_receipt',
  requiredAuthorities: ['approval_resolution'],
  riskClass: 'medium',
  sourceRefs: [SOURCE],
  title,
})

// Dry-run-only batch: operator_read, no receipt (mutates nothing over MCP).
const readishTool = (
  name: string,
  title: string,
  description: string,
): OpenAgentsMcpToolDescriptor => ({
  description,
  inputSchemaRef: `${name}.input`,
  name,
  outputSchemaRef: `${name}.output`,
  progressBehavior: 'none',
  publicSummary: title,
  receiptBehavior: 'none',
  requiredAuthorities: ['operator_read'],
  riskClass: 'low',
  sourceRefs: [SOURCE],
  title,
})

export const CRM_MCP_WRITE_TOOLS: ReadonlyArray<OpenAgentsMcpToolDescriptor> = [
  writeTool(
    'crm.send.command.propose',
    'Propose a send',
    'Propose an approval-gated send_email command for a contact. Records a pending_approval command — SENDS NOTHING. A human approves it separately.',
    ['operator_read'],
  ),
  writeTool(
    'crm.template.upsert',
    'Upsert email template',
    'Create or update a CRM email template for the tenant.',
    ['workspace_write'],
  ),
  // --- Wave 3: gated execution ---
  approvalTool(
    'crm.send.command.approve',
    'Approve + execute a send command',
    'Approve a pending send_email command and execute it (the suppression/unsubscribe gate still applies).',
  ),
  approvalTool(
    'crm.send.command.reject',
    'Reject a send command',
    'Reject a pending send_email command (nothing is sent).',
  ),
  writeTool(
    'crm.import.run',
    'Import contacts from CSV',
    'Import contacts from CSV text into the tenant CRM, recording an audited import run.',
    ['workspace_write'],
  ),
  readishTool(
    'crm.batch.send',
    'Plan a batch send (dry-run)',
    'Plan a batch send across contacts. Over MCP this is DRY-RUN ONLY: it reports would_send/suppressed/failed counts and sends nothing.',
  ),
]

/** All CRM MCP tools (read + write), filtered per-principal at list time. */
export const CRM_MCP_TOOLS: ReadonlyArray<OpenAgentsMcpToolDescriptor> = [
  ...CRM_MCP_READ_TOOLS,
  ...CRM_MCP_WRITE_TOOLS,
]

// --- dispatch (read fns) ----------------------------------------------------

// Tenant is the principal's bound tenant — never client-supplied `args.tenant`.
type CrmToolContext = Readonly<{ subjectRef: string; dispatchDeps: CrmDispatchDeps }>
type CrmToolHandler = (
  db: CrmEmailDatabase,
  tenant: string,
  a: Record<string, unknown>,
  ctx: CrmToolContext,
) => Promise<unknown>

const CRM_MCP_READ_DISPATCH: Readonly<Record<string, CrmToolHandler>> = {
  'crm.account.get': (db, tenant, a) => getCrmAccountById(db, tenant, requiredId(a, 'accountId')),
  'crm.accounts.list': (db, tenant, a) => listCrmAccounts(db, tenant, { limit: optLimit(a) }),
  'crm.commands.list': (db, tenant, a) =>
    listCrmCommands(db, tenant, {
      limit: optLimit(a),
      status: typeof a.status === 'string' ? a.status : undefined,
    }),
  'crm.contact.activities.list': (db, tenant, a) =>
    listCrmActivitiesForContact(db, tenant, requiredId(a, 'contactId'), { limit: optLimit(a) }),
  'crm.contact.emails.list': (db, tenant, a) =>
    listCrmEmailMessagesForContact(db, tenant, requiredId(a, 'contactId'), { limit: optLimit(a) }),
  'crm.contact.engagement.get': (db, tenant, a) =>
    getCrmEngagementSnapshot(db, tenant, requiredId(a, 'contactId')),
  'crm.contact.get': (db, tenant, a) => getCrmContactById(db, tenant, requiredId(a, 'contactId')),
  'crm.contact.render': async (db, tenant, a) => {
    const composed = await composeCrmEmailForContact(db, {
      contactId: requiredId(a, 'contactId'),
      templateSlug: requiredId(a, 'template'),
      tenantRef: tenant,
    })
    const eligibility = await readEmailSendEligibility(db, {
      category: 'marketing',
      email: composed.toEmail,
    })
    return {
      eligibility,
      message: {
        bodyHtml: composed.bodyHtml,
        bodyMarkdown: composed.bodyMarkdown,
        contactId: composed.contact.id,
        subject: composed.subject,
        templateId: composed.template.id,
        toEmail: composed.toEmail,
      },
    }
  },
  'crm.contacts.list': (db, tenant, a) =>
    listCrmContacts(db, tenant, { limit: optLimit(a), search: optSearch(a) }),
  'crm.gmail.queue.list': (db, tenant, a) => listCrmQueuedGmailMessages(db, tenant, { limit: optLimit(a) }),
  'crm.imports.list': (db, tenant, a) => listCrmSourceImportRuns(db, tenant, { limit: optLimit(a) }),
  'crm.lists.list': (db, tenant) => listCrmContactLists(db, tenant),
  'crm.opportunities.list': (db, tenant, a) => listCrmOpportunities(db, tenant, { limit: optLimit(a) }),
  'crm.opportunity.get': (db, tenant, a) => getCrmOpportunityById(db, tenant, requiredId(a, 'opportunityId')),
  'crm.templates.list': (db, tenant) => listCrmEmailTemplates(db, tenant),
}

const CRM_MCP_WRITE_DISPATCH: Readonly<Record<string, CrmToolHandler>> = {
  'crm.send.command.propose': (db, tenant, a, ctx) =>
    proposeCrmSendCommand(db, {
      channel: a.channel === 'resend' ? 'resend' : 'gmail_gws',
      contactId: requiredId(a, 'contactId'),
      proposedByRef: ctx.subjectRef,
      sendReason: typeof a.sendReason === 'string' ? a.sendReason : null,
      templateSlug: requiredId(a, 'templateSlug'),
      tenantRef: tenant,
    }),
  'crm.template.upsert': (db, tenant, a) =>
    upsertCrmEmailTemplate(db, {
      bodyMarkdownTemplate: requiredId(a, 'bodyMarkdownTemplate'),
      name: requiredId(a, 'name'),
      slug: requiredId(a, 'slug'),
      subjectTemplate: requiredId(a, 'subjectTemplate'),
      tenantRef: tenant,
    }),
  // --- Wave 3: gated execution ---
  'crm.batch.send': (db, tenant, a, ctx) =>
    runCrmBatch(db, ctx.dispatchDeps, {
      channel: a.channel === 'resend' ? 'resend' : 'gmail_gws',
      contactIds: stringArrayFromUnknown(a.contactIds),
      dryRun: true, // MCP batch is DRY-RUN ONLY
      sendReason: typeof a.sendReason === 'string' ? a.sendReason : null,
      templateSlug: requiredId(a, 'templateSlug'),
      tenantRef: tenant,
    }),
  'crm.import.run': (db, tenant, a) =>
    importCrmContactsFromCsv(crmImportDepsFromDb(db), {
      csv: requiredId(a, 'csv'),
      listName: typeof a.listName === 'string' ? a.listName : null,
      listSlug: typeof a.listSlug === 'string' ? a.listSlug : null,
      sourceLabel:
        typeof a.sourceLabel === 'string' && a.sourceLabel.trim() !== '' ? a.sourceLabel : 'mcp:import',
      tenantRef: tenant,
    }),
  'crm.send.command.approve': (db, tenant, a, ctx) =>
    approveAndExecuteCrmSendCommand(db, ctx.dispatchDeps, {
      approvedByRef: ctx.subjectRef,
      commandId: requiredId(a, 'commandId'),
      tenantRef: tenant,
    }),
  'crm.send.command.reject': (db, tenant, a) =>
    rejectCrmCommand(db, {
      commandId: requiredId(a, 'commandId'),
      reason: typeof a.reason === 'string' ? a.reason : null,
      tenantRef: tenant,
    }),
}

const CRM_MCP_DISPATCH: Readonly<Record<string, CrmToolHandler>> = {
  ...CRM_MCP_READ_DISPATCH,
  ...CRM_MCP_WRITE_DISPATCH,
}

/** Build a mutation receipt for a write tool result (best-effort refs). */
const makeWriteReceipt = (
  descriptor: OpenAgentsMcpToolDescriptor,
  data: unknown,
): OpenAgentsMcpReceipt => {
  const record = (data ?? {}) as Record<string, unknown>
  const receiptRef = typeof record.id === 'string' ? record.id : `receipt.${descriptor.name}`
  const generatedAt =
    typeof record.createdAt === 'string'
      ? record.createdAt
      : typeof record.updatedAt === 'string'
        ? record.updatedAt
        : ''
  const kind: OpenAgentsMcpReceiptKind =
    descriptor.receiptBehavior === 'approval_receipt' ? 'approval' : 'mutation'
  return {
    artifactRefs: [receiptRef],
    authorityClass: descriptor.requiredAuthorities[0] ?? 'operator_read',
    generatedAt,
    kind,
    receiptRef,
    sourceRefs: [SOURCE],
    status: 'recorded',
    summary: `${descriptor.name} recorded`,
    targetRef: typeof record.contactId === 'string' ? record.contactId : receiptRef,
  }
}

const projectWriteOutcome = (
  descriptor: OpenAgentsMcpToolDescriptor,
  data: unknown,
): McpToolCallOutcome => {
  const payload = { receipt: makeWriteReceipt(descriptor, data), result: data }
  const projection = projectOpenAgentsMcpOutput({
    maxTextBytes: 131072,
    outputRef: `mcp.crm.${descriptor.name}`,
    safetyClass: 'operator',
    sourceRefs: [`tool.${descriptor.name}`],
    text: JSON.stringify(payload, null, 2),
  })
  if (projection.text === undefined) {
    return { content: [{ text: projection.summary, type: 'text' }], isError: true }
  }
  return { content: [{ text: projection.text, type: 'text' }], isError: false, structuredContent: payload }
}

// --- catalog ----------------------------------------------------------------

const toolListing = (descriptor: OpenAgentsMcpToolDescriptor): McpToolListing => ({
  annotations: { readOnlyHint: true },
  description: descriptor.description,
  inputSchema: CRM_MCP_INPUT_SCHEMAS[descriptor.name] ?? { type: 'object' },
  name: descriptor.name,
  title: descriptor.title,
})

/** Project read data into a safe MCP tool result (operator class). */
const projectReadOutcome = (name: string, data: unknown): McpToolCallOutcome => {
  const text = JSON.stringify(data, null, 2)
  const projection = projectOpenAgentsMcpOutput({
    maxTextBytes: 131072,
    outputRef: `mcp.crm.${name}`,
    safetyClass: 'operator',
    sourceRefs: [`tool.${name}`],
    text,
  })
  if (projection.text === undefined) {
    return { content: [{ text: projection.summary, type: 'text' }], isError: true }
  }
  return {
    content: [{ text: projection.text, type: 'text' }],
    isError: false,
    structuredContent: data,
  }
}

// --- resources (mcp://openagents/worker/crm/*) ------------------------------

const RESOURCE_PREFIX = 'mcp://openagents/worker/'

/** Collection resources advertised by resources/list (single items read by URI). */
export const CRM_MCP_RESOURCES: ReadonlyArray<McpResourceListing> = [
  { description: 'CRM contacts for the tenant.', mimeType: 'application/json', name: 'crm.contacts', title: 'CRM contacts', uri: `${RESOURCE_PREFIX}crm/contacts` },
  { description: 'CRM accounts for the tenant.', mimeType: 'application/json', name: 'crm.accounts', title: 'CRM accounts', uri: `${RESOURCE_PREFIX}crm/accounts` },
  { description: 'CRM contact lists (segments).', mimeType: 'application/json', name: 'crm.lists', title: 'CRM lists', uri: `${RESOURCE_PREFIX}crm/lists` },
  { description: 'CRM opportunities for the tenant.', mimeType: 'application/json', name: 'crm.opportunities', title: 'CRM opportunities', uri: `${RESOURCE_PREFIX}crm/opportunities` },
  { description: 'CRM CSV import-run audit rows.', mimeType: 'application/json', name: 'crm.imports', title: 'CRM import runs', uri: `${RESOURCE_PREFIX}crm/imports` },
  { description: 'Approval-gated send_email commands.', mimeType: 'application/json', name: 'crm.commands', title: 'CRM send commands', uri: `${RESOURCE_PREFIX}crm/commands` },
]

/** Resolve a parsed `crm/...` resource path to its read data (bound tenant). */
const readCrmResourceData = async (
  db: CrmEmailDatabase,
  tenant: string,
  path: string,
): Promise<unknown> => {
  const segments = path.split('/').filter(part => part !== '')
  if (segments[0] !== 'crm') {
    throw new CrmMcpToolError('unknown_resource')
  }
  const [, head, id, sub] = segments
  switch (head) {
    case 'contacts':
      return listCrmContacts(db, tenant, {})
    case 'accounts':
      return listCrmAccounts(db, tenant, {})
    case 'lists':
      return listCrmContactLists(db, tenant)
    case 'opportunities':
      return listCrmOpportunities(db, tenant, {})
    case 'imports':
      return listCrmSourceImportRuns(db, tenant, {})
    case 'commands':
      return listCrmCommands(db, tenant, {})
    case 'contact':
      if (id === undefined) throw new CrmMcpToolError('unknown_resource')
      if (sub === 'activities') return listCrmActivitiesForContact(db, tenant, id, {})
      if (sub === undefined) return getCrmContactById(db, tenant, id)
      throw new CrmMcpToolError('unknown_resource')
    case 'account':
      if (id === undefined) throw new CrmMcpToolError('unknown_resource')
      return getCrmAccountById(db, tenant, id)
    case 'opportunity':
      if (id === undefined) throw new CrmMcpToolError('unknown_resource')
      return getCrmOpportunityById(db, tenant, id)
    default:
      throw new CrmMcpToolError('unknown_resource')
  }
}

export type CrmMcpCatalogDeps<Bindings extends CrmMcpEnv> = Readonly<{
  resolveResendDeps: (env: Bindings) => CrmDispatchDeps['resend']
}>

export const makeCrmMcpCatalog = <Bindings extends CrmMcpEnv>(
  deps: CrmMcpCatalogDeps<Bindings>,
): CrmMcpCatalog<Bindings> => ({
  callTool: async (env, _request, principal, name, callArgs) => {
    // Ungranted tools are ABSENT: an ungranted/unknown name is unknown_tool.
    const grantedAuthorities = openAgentsMcpGrantedAuthoritySet(principal.grants)
    const descriptor = CRM_MCP_TOOLS.find(d => d.name === name)
    if (descriptor === undefined || !openAgentsMcpDescriptorIsGranted(descriptor, grantedAuthorities)) {
      throw new CrmMcpToolError('unknown_tool')
    }
    const handler = CRM_MCP_DISPATCH[name]
    if (handler === undefined) {
      throw new CrmMcpToolError('unknown_tool')
    }
    const data = await handler(makeCrmEmailDatabaseForEnv(env), principal.tenantRef, args(callArgs), {
      dispatchDeps: { resend: deps.resolveResendDeps(env) },
      subjectRef: principal.subjectRef,
    })
    return descriptor.receiptBehavior === 'none'
      ? projectReadOutcome(name, data)
      : projectWriteOutcome(descriptor, data)
  },
  listResources: (_env, _request, principal) =>
    Promise.resolve(
      openAgentsMcpGrantedAuthoritySet(principal.grants).has('operator_read') ? CRM_MCP_RESOURCES : [],
    ),
  listTools: (_env, _request, principal) =>
    Promise.resolve(
      filterOpenAgentsMcpDescriptorsByGrantSet(CRM_MCP_TOOLS, principal.grants).map(toolListing),
    ),
  readResource: async (env, _request, principal, uri): Promise<McpResourceReadOutcome> => {
    if (!openAgentsMcpGrantedAuthoritySet(principal.grants).has('operator_read')) {
      throw new CrmMcpToolError('unknown_resource')
    }
    const parsed = parseOpenAgentsMcpResourceUri(uri)
    if (parsed.namespace !== 'worker') {
      throw new CrmMcpToolError('unknown_resource')
    }
    const data = await readCrmResourceData(makeCrmEmailDatabaseForEnv(env), principal.tenantRef, parsed.path)
    const projection = projectOpenAgentsMcpOutput({
      maxTextBytes: 131072,
      outputRef: uri,
      safetyClass: 'operator',
      sourceRefs: [uri],
      text: JSON.stringify(data, null, 2),
    })
    return {
      contents: [
        {
          mimeType: 'application/json',
          text: projection.text ?? projection.summary,
          uri,
        },
      ],
    }
  },
})
