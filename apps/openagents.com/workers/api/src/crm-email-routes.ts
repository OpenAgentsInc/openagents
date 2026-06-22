/**
 * CRM email routes: templates, contact render, Gmail/gws write-back, and the
 * per-contact send ledger read (epic #5980, sub-issue #5983).
 *
 *   GET  /api/operator/crm/templates                         list templates
 *   POST /api/operator/crm/templates                         upsert a template
 *   GET  /api/operator/crm/contacts/:id/render?template=...  compose a message
 *                                                            + send eligibility
 *   POST /api/operator/crm/contacts/:id/gmail-writeback      record a gmail_gws
 *                                                            draft/sent message
 *                                                            + a crm_activity
 *   GET  /api/operator/crm/contacts/:id/emails               list sent messages
 *
 * The Gmail channel sends as the operator's own mailbox via the local `gws`
 * executor (Gmail OAuth can't live in a Worker), so the Worker's job is to (a)
 * compose the personalized message, (b) report send eligibility (suppression /
 * unsubscribe), and (c) record the executor's outcome back to the ledger — the
 * write-back the old Laravel-coupled script never did. Resend (#5984) reuses the
 * same compose + ledger.
 */
import { Effect } from 'effect'

import {
  composeCrmEmailForContact,
  CrmEmailError,
  type CrmSendChannel,
  getCrmEmailMessageById,
  listCrmEmailMessagesForContact,
  listCrmEmailTemplates,
  recordCrmEmailMessage,
  updateCrmEmailMessageDelivery,
  upsertCrmEmailTemplate,
} from './crm-email'
import { DEFAULT_CRM_TENANT_REF, recordCrmActivity } from './crm-store'
import { readEmailSendEligibility } from './email-preferences'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'

type HttpResponse = globalThis.Response

type CrmEmailEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type CrmEmailRouteDependencies<Bindings extends CrmEmailEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const TEMPLATES = /^\/api\/operator\/crm\/templates$/
const RENDER = /^\/api\/operator\/crm\/contacts\/([^/]+)\/render$/
const WRITEBACK = /^\/api\/operator\/crm\/contacts\/([^/]+)\/gmail-writeback$/
const EMAILS = /^\/api\/operator\/crm\/contacts\/([^/]+)\/emails$/

const ALL: ReadonlyArray<RegExp> = [TEMPLATES, RENDER, WRITEBACK, EMAILS]

const tenantOf = (url: URL): string => {
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

const captured = (pattern: RegExp, path: string): string | null => {
  const match = pattern.exec(path)
  return match === null ? null : decodeURIComponent(match[1] ?? '')
}

const channelOf = (raw: unknown): CrmSendChannel =>
  raw === 'resend' ? 'resend' : 'gmail_gws'

export const makeCrmEmailRoutes = <Bindings extends CrmEmailEnv>(
  dependencies: CrmEmailRouteDependencies<Bindings>,
) => {
  const guard = (
    request: Request,
    env: Bindings,
    body: (db: D1Database) => Promise<HttpResponse>,
  ): Effect.Effect<HttpResponse> =>
    Effect.gen(function* () {
      const authorized = yield* Effect.tryPromise({
        catch: () => false as const,
        try: () => dependencies.requireAdminApiToken(request, env),
      })
      if (!authorized) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }
      return yield* Effect.tryPromise({
        catch: error =>
          error instanceof CrmEmailError
            ? error
            : new CrmEmailError({ reason: `crm.email: ${String(error)}` }),
        try: () => body(openAgentsDatabase(env)),
      })
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof CrmEmailError
            ? noStoreJsonResponse({ error: 'crm_email_error', reason: error.reason }, { status: 422 })
            : noStoreJsonResponse({ error: 'crm_email_error' }, { status: 500 }),
        ),
      ),
    )

  return {
    routeCrmEmailRequest: (
      request: Request,
      env: Bindings,
      _ctx?: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname
      if (!ALL.some(pattern => pattern.test(path))) {
        return undefined
      }
      const tenant = tenantOf(url)

      // Templates collection
      if (TEMPLATES.test(path)) {
        if (request.method === 'GET') {
          return guard(request, env, async db =>
            noStoreJsonResponse({ templates: await listCrmEmailTemplates(db, tenant) }),
          )
        }
        if (request.method === 'POST') {
          return guard(request, env, async db => {
            const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
            if (
              body === null ||
              typeof body.slug !== 'string' ||
              typeof body.name !== 'string' ||
              typeof body.subjectTemplate !== 'string' ||
              typeof body.bodyMarkdownTemplate !== 'string'
            ) {
              return noStoreJsonResponse(
                { error: 'bad_request', reason: 'slug, name, subjectTemplate, bodyMarkdownTemplate required' },
                { status: 400 },
              )
            }
            const template = await upsertCrmEmailTemplate(db, {
              bodyMarkdownTemplate: body.bodyMarkdownTemplate,
              name: body.name,
              slug: body.slug,
              subjectTemplate: body.subjectTemplate,
              tenantRef:
                typeof body.tenant === 'string' && body.tenant.trim() !== ''
                  ? body.tenant.trim()
                  : tenant,
            })
            return noStoreJsonResponse({ template })
          })
        }
        return Effect.succeed(methodNotAllowed(['GET', 'POST']))
      }

      // Render a contact's message + report eligibility
      const renderContactId = captured(RENDER, path)
      if (renderContactId !== null) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        const templateSlug = url.searchParams.get('template')
        if (templateSlug === null || templateSlug.trim() === '') {
          return Effect.succeed(
            noStoreJsonResponse(
              { error: 'bad_request', reason: 'template query param required' },
              { status: 400 },
            ),
          )
        }
        return guard(request, env, async db => {
          const composed = await composeCrmEmailForContact(db, {
            contactId: renderContactId,
            templateSlug: templateSlug.trim(),
            tenantRef: tenant,
          })
          const eligibility = await readEmailSendEligibility(db, {
            category: 'marketing',
            email: composed.toEmail,
          })
          return noStoreJsonResponse({
            channel: channelOf(url.searchParams.get('channel')),
            eligibility,
            message: {
              bodyHtml: composed.bodyHtml,
              bodyMarkdown: composed.bodyMarkdown,
              contactId: composed.contact.id,
              subject: composed.subject,
              templateId: composed.template.id,
              toEmail: composed.toEmail,
            },
          })
        })
      }

      // Gmail/gws write-back: record a draft/sent message + activity
      const writebackContactId = captured(WRITEBACK, path)
      if (writebackContactId !== null) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }
        return guard(request, env, async db => {
          const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
          if (
            body === null ||
            typeof body.subject !== 'string' ||
            typeof body.bodyMarkdown !== 'string' ||
            typeof body.toEmail !== 'string'
          ) {
            return noStoreJsonResponse(
              { error: 'bad_request', reason: 'toEmail, subject, bodyMarkdown required' },
              { status: 400 },
            )
          }
          const status = body.status === 'sent' ? 'sent' : 'draft'

          // Sent messages must pass the suppression/unsubscribe gate.
          if (status === 'sent') {
            const eligibility = await readEmailSendEligibility(db, {
              category: 'marketing',
              email: body.toEmail,
            })
            if (!eligibility.allowed) {
              return noStoreJsonResponse(
                { error: 'suppressed', reason: eligibility.reason },
                { status: 409 },
              )
            }
          }

          // If messageId is given, the executor is closing out a queued row
          // (unified dispatch, #5985) — update it in place rather than inserting
          // a duplicate. Otherwise insert a fresh ledger row.
          const existingId = typeof body.messageId === 'string' ? body.messageId.trim() : ''
          let message
          if (existingId !== '') {
            await updateCrmEmailMessageDelivery(db, {
              id: existingId,
              providerDraftId:
                typeof body.providerDraftId === 'string' ? body.providerDraftId : null,
              providerMessageId:
                typeof body.providerMessageId === 'string' ? body.providerMessageId : null,
              status,
              tenantRef: tenant,
            })
            const updated = await getCrmEmailMessageById(db, tenant, existingId)
            if (updated === null) {
              return noStoreJsonResponse(
                { error: 'not_found', resource: 'crm_email_message' },
                { status: 404 },
              )
            }
            message = updated
          } else {
            message = await recordCrmEmailMessage(db, {
              bodyHtml: typeof body.bodyHtml === 'string' ? body.bodyHtml : null,
              bodyMarkdown: body.bodyMarkdown,
              channel: 'gmail_gws',
              contactId: writebackContactId,
              fromEmail: typeof body.fromEmail === 'string' ? body.fromEmail : null,
              providerDraftId:
                typeof body.providerDraftId === 'string' ? body.providerDraftId : null,
              providerMessageId:
                typeof body.providerMessageId === 'string' ? body.providerMessageId : null,
              sendReason:
                typeof body.sendReason === 'string' ? body.sendReason : 'crm_gmail_outreach',
              status,
              subject: body.subject,
              templateId: typeof body.templateId === 'string' ? body.templateId : null,
              tenantRef: tenant,
              toEmail: body.toEmail,
            })
          }

          await recordCrmActivity(db, {
            activityType: status === 'sent' ? 'email_sent' : 'email_drafted',
            contactId: writebackContactId,
            sourceRecordId: message.id,
            sourceRecordType: 'crm_email_message',
            sourceSystem: 'gmail_gws',
            subject: body.subject,
            summary: `Gmail ${status} via gws`,
            tenantRef: tenant,
          })

          return noStoreJsonResponse({ message })
        })
      }

      // Per-contact message ledger read
      const emailsContactId = captured(EMAILS, path)
      if (emailsContactId !== null) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        const limitParam = url.searchParams.get('limit')
        const limit = limitParam === null ? undefined : Number(limitParam)
        return guard(request, env, async db =>
          noStoreJsonResponse({
            messages: await listCrmEmailMessagesForContact(db, tenant, emailsContactId, {
              limit: limit !== undefined && Number.isFinite(limit) ? limit : undefined,
            }),
          }),
        )
      }

      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    },
  }
}
