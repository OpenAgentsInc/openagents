/**
 * Admin-gated read APIs over the native CRM model (epic #5980, sub-issue #5981).
 *
 * Thin HTTP surface over `crm-store.ts`. Read-only: list/get contacts, accounts,
 * lists, a contact's activities + engagement snapshot, and opportunities, plus
 * the import-run audit feed. Every route is tenant-scoped (the `tenant` query
 * param, defaulting to the OpenAgents tenant) and admin-gated, mirroring
 * `partner-agreement-routes.ts`.
 *
 * Writers (CSV import, send write-backs, command approvals) ship in later epic
 * issues; this module changes no behavior beyond exposing reads.
 *
 * COORDINATOR WIRING:
 *   import { makeCrmRoutes } from './crm-routes'
 *   const crmRoutes = makeCrmRoutes<WorkerBindings>({ requireAdminApiToken })
 *   ...chain crmRoutes.routeCrmRequest(request, env, ctx) in routeOmniRequest.
 *   Apply migration 0218_crm_contacts.sql before serving.
 */
// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import {
  type CrmEmailDatabase,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  CrmStorageError,
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

type HttpResponse = globalThis.Response

type CrmRouteEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type CrmRouteDependencies<Bindings extends CrmRouteEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const tenantOf = (url: URL): string => {
  const value = url.searchParams.get('tenant')
  return value === null || value.trim() === '' ? DEFAULT_CRM_TENANT_REF : value.trim()
}

const limitOf = (url: URL): number | undefined => {
  const value = url.searchParams.get('limit')
  if (value === null) return undefined
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : undefined
}

const notFound = (resource: string): HttpResponse =>
  noStoreJsonResponse({ error: 'not_found', resource }, { status: 404 })

/**
 * Auth-gate, run a D1 read that produces the final response, and convert any
 * storage error into a 500. The reader owns its own success shape (including
 * 404 for a missing single resource).
 */
const guarded = <Bindings extends CrmRouteEnv>(
  dependencies: CrmRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  read: (db: CrmEmailDatabase) => Promise<HttpResponse>,
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
        error instanceof CrmStorageError
          ? error
          : new CrmStorageError({ operation: `crm.read: ${String(error)}` }),
      try: () => read(makeCrmEmailDatabaseForEnv(env)),
    })
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(noStoreJsonResponse({ error: 'crm_storage_error' }, { status: 500 })),
    ),
  )

const CONTACTS = /^\/api\/operator\/crm\/contacts$/
const CONTACT_ACTIVITIES = /^\/api\/operator\/crm\/contacts\/([^/]+)\/activities$/
const CONTACT_ENGAGEMENT = /^\/api\/operator\/crm\/contacts\/([^/]+)\/engagement$/
const CONTACT = /^\/api\/operator\/crm\/contacts\/([^/]+)$/
const ACCOUNTS = /^\/api\/operator\/crm\/accounts$/
const ACCOUNT = /^\/api\/operator\/crm\/accounts\/([^/]+)$/
const LISTS = /^\/api\/operator\/crm\/lists$/
const OPPORTUNITIES = /^\/api\/operator\/crm\/opportunities$/
const OPPORTUNITY = /^\/api\/operator\/crm\/opportunities\/([^/]+)$/
const IMPORT_RUNS = /^\/api\/operator\/crm\/import-runs$/

const ALL_PATTERNS: ReadonlyArray<RegExp> = [
  CONTACTS,
  CONTACT_ACTIVITIES,
  CONTACT_ENGAGEMENT,
  CONTACT,
  ACCOUNTS,
  ACCOUNT,
  LISTS,
  OPPORTUNITIES,
  OPPORTUNITY,
  IMPORT_RUNS,
]

const captured = (pattern: RegExp, path: string): string | null => {
  const match = pattern.exec(path)
  return match === null ? null : decodeURIComponent(match[1] ?? '')
}

export const makeCrmRoutes = <Bindings extends CrmRouteEnv>(
  dependencies: CrmRouteDependencies<Bindings>,
) => ({
  routeCrmRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const path = url.pathname

    if (!ALL_PATTERNS.some(pattern => pattern.test(path))) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const tenant = tenantOf(url)
    const run = (read: (db: CrmEmailDatabase) => Promise<HttpResponse>) =>
      guarded(dependencies, request, env, read)

    if (CONTACTS.test(path)) {
      return run(async db =>
        noStoreJsonResponse({
          contacts: await listCrmContacts(db, tenant, {
            limit: limitOf(url),
            search: url.searchParams.get('search'),
          }),
        }),
      )
    }

    const activitiesContactId = captured(CONTACT_ACTIVITIES, path)
    if (activitiesContactId !== null) {
      return run(async db =>
        noStoreJsonResponse({
          activities: await listCrmActivitiesForContact(db, tenant, activitiesContactId, {
            limit: limitOf(url),
          }),
        }),
      )
    }

    const engagementContactId = captured(CONTACT_ENGAGEMENT, path)
    if (engagementContactId !== null) {
      return run(async db => {
        const snapshot = await getCrmEngagementSnapshot(db, tenant, engagementContactId)
        return snapshot === null
          ? notFound('engagement_snapshot')
          : noStoreJsonResponse({ snapshot })
      })
    }

    const contactId = captured(CONTACT, path)
    if (contactId !== null) {
      return run(async db => {
        const found = await getCrmContactById(db, tenant, contactId)
        return found === null ? notFound('contact') : noStoreJsonResponse({ contact: found })
      })
    }

    if (ACCOUNTS.test(path)) {
      return run(async db =>
        noStoreJsonResponse({
          accounts: await listCrmAccounts(db, tenant, { limit: limitOf(url) }),
        }),
      )
    }

    const accountId = captured(ACCOUNT, path)
    if (accountId !== null) {
      return run(async db => {
        const found = await getCrmAccountById(db, tenant, accountId)
        return found === null ? notFound('account') : noStoreJsonResponse({ account: found })
      })
    }

    if (LISTS.test(path)) {
      return run(async db =>
        noStoreJsonResponse({ lists: await listCrmContactLists(db, tenant) }),
      )
    }

    if (OPPORTUNITIES.test(path)) {
      return run(async db =>
        noStoreJsonResponse({
          opportunities: await listCrmOpportunities(db, tenant, { limit: limitOf(url) }),
        }),
      )
    }

    const opportunityId = captured(OPPORTUNITY, path)
    if (opportunityId !== null) {
      return run(async db => {
        const found = await getCrmOpportunityById(db, tenant, opportunityId)
        return found === null
          ? notFound('opportunity')
          : noStoreJsonResponse({ opportunity: found })
      })
    }

    if (IMPORT_RUNS.test(path)) {
      return run(async db =>
        noStoreJsonResponse({
          importRuns: await listCrmSourceImportRuns(db, tenant, { limit: limitOf(url) }),
        }),
      )
    }

    return Effect.succeed(notFound('crm'))
  },
})
