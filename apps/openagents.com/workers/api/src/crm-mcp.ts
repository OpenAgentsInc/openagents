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
import {
  type OpenAgentsMcpToolDescriptor,
  projectOpenAgentsMcpOutput,
} from '@openagentsinc/mcp-contract'

import {
  composeCrmEmailForContact,
  listCrmEmailMessagesForContact,
  listCrmEmailTemplates,
  listCrmQueuedGmailMessages,
} from './crm-email'
import { listCrmCommands } from './crm-command'
import type { CrmMcpCatalog, McpToolCallOutcome, McpToolListing } from './crm-mcp-routes'
import {
  DEFAULT_CRM_TENANT_REF,
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
import { openAgentsDatabase } from './runtime'

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

const tenantOf = (a: Record<string, unknown>): string =>
  typeof a.tenant === 'string' && a.tenant.trim() !== '' ? a.tenant.trim() : DEFAULT_CRM_TENANT_REF

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

// --- dispatch (read fns) ----------------------------------------------------

type CrmReadHandler = (db: D1Database, a: Record<string, unknown>) => Promise<unknown>

const CRM_MCP_READ_DISPATCH: Readonly<Record<string, CrmReadHandler>> = {
  'crm.account.get': (db, a) => getCrmAccountById(db, tenantOf(a), requiredId(a, 'accountId')),
  'crm.accounts.list': (db, a) => listCrmAccounts(db, tenantOf(a), { limit: optLimit(a) }),
  'crm.commands.list': (db, a) =>
    listCrmCommands(db, tenantOf(a), {
      limit: optLimit(a),
      status: typeof a.status === 'string' ? a.status : undefined,
    }),
  'crm.contact.activities.list': (db, a) =>
    listCrmActivitiesForContact(db, tenantOf(a), requiredId(a, 'contactId'), { limit: optLimit(a) }),
  'crm.contact.emails.list': (db, a) =>
    listCrmEmailMessagesForContact(db, tenantOf(a), requiredId(a, 'contactId'), { limit: optLimit(a) }),
  'crm.contact.engagement.get': (db, a) =>
    getCrmEngagementSnapshot(db, tenantOf(a), requiredId(a, 'contactId')),
  'crm.contact.get': (db, a) => getCrmContactById(db, tenantOf(a), requiredId(a, 'contactId')),
  'crm.contact.render': async (db, a) => {
    const composed = await composeCrmEmailForContact(db, {
      contactId: requiredId(a, 'contactId'),
      templateSlug: requiredId(a, 'template'),
      tenantRef: tenantOf(a),
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
  'crm.contacts.list': (db, a) =>
    listCrmContacts(db, tenantOf(a), { limit: optLimit(a), search: optSearch(a) }),
  'crm.gmail.queue.list': (db, a) => listCrmQueuedGmailMessages(db, tenantOf(a), { limit: optLimit(a) }),
  'crm.imports.list': (db, a) => listCrmSourceImportRuns(db, tenantOf(a), { limit: optLimit(a) }),
  'crm.lists.list': (db, a) => listCrmContactLists(db, tenantOf(a)),
  'crm.opportunities.list': (db, a) => listCrmOpportunities(db, tenantOf(a), { limit: optLimit(a) }),
  'crm.opportunity.get': (db, a) => getCrmOpportunityById(db, tenantOf(a), requiredId(a, 'opportunityId')),
  'crm.templates.list': (db, a) => listCrmEmailTemplates(db, tenantOf(a)),
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

export const makeCrmMcpReadCatalog = <Bindings extends CrmMcpEnv>(): CrmMcpCatalog<Bindings> => ({
  callTool: async (env, _request, name, callArgs) => {
    const handler = CRM_MCP_READ_DISPATCH[name]
    if (handler === undefined) {
      throw new CrmMcpToolError('unknown_tool')
    }
    const data = await handler(openAgentsDatabase(env), args(callArgs))
    return projectReadOutcome(name, data)
  },
  listResources: () => Promise.resolve([]),
  listTools: () => Promise.resolve(CRM_MCP_READ_TOOLS.map(toolListing)),
  readResource: () => Promise.reject(new CrmMcpToolError('unknown_resource')),
})
