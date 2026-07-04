/**
 * Admin-gated CSV import route for the native CRM (epic #5980, sub-issue #5982).
 *
 *   POST /api/operator/crm/import
 *     Body: application/json { csv, sourceLabel, tenant?, listSlug?, listName? }
 *        OR text/csv with ?sourceLabel=&tenant=&listSlug=&listName=
 *     Returns the import-run summary (honest imported/updated/duplicate/failed
 *     counts + a small email sample + per-line errors).
 *
 * This is a one-time migration surface (CSV handoff off the old prod DB); it
 * writes only into the tenant-scoped CRM model and records a
 * `crm_source_import_runs` audit row. It never sends email.
 */
// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import { makeCrmEmailDatabaseForEnv } from './crm-email-domain-store'
import { Effect } from 'effect'

import { crmImportDepsFromDb, importCrmContactsFromCsv } from './crm-import'
import { CrmStorageError, DEFAULT_CRM_TENANT_REF } from './crm-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

type CrmImportEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type CrmImportRouteDependencies<Bindings extends CrmImportEnv> = Readonly<{
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

const IMPORT_PATTERN = /^\/api\/operator\/crm\/import$/

type ImportArgs = Readonly<{
  csv: string
  listName: string | null
  listSlug: string | null
  sourceLabel: string
  tenant: string
}>

const readArgs = async (request: Request, url: URL): Promise<ImportArgs | null> => {
  const contentType = request.headers.get('content-type') ?? ''
  if (contentType.includes('application/json')) {
    const body = (await request.json().catch(() => null)) as Record<string, unknown> | null
    if (body === null || typeof body.csv !== 'string' || body.csv.trim() === '') {
      return null
    }
    const sourceLabel =
      typeof body.sourceLabel === 'string' && body.sourceLabel.trim() !== ''
        ? body.sourceLabel.trim()
        : 'csv:import'
    return {
      csv: body.csv,
      listName: typeof body.listName === 'string' ? body.listName : null,
      listSlug: typeof body.listSlug === 'string' ? body.listSlug : null,
      sourceLabel,
      tenant:
        typeof body.tenant === 'string' && body.tenant.trim() !== ''
          ? body.tenant.trim()
          : DEFAULT_CRM_TENANT_REF,
    }
  }

  // Raw CSV body with query params.
  const csv = await request.text().catch(() => '')
  if (csv.trim() === '') {
    return null
  }
  return {
    csv,
    listName: url.searchParams.get('listName'),
    listSlug: url.searchParams.get('listSlug'),
    sourceLabel: url.searchParams.get('sourceLabel')?.trim() || 'csv:import',
    tenant: url.searchParams.get('tenant')?.trim() || DEFAULT_CRM_TENANT_REF,
  }
}

export const makeCrmImportRoutes = <Bindings extends CrmImportEnv>(
  dependencies: CrmImportRouteDependencies<Bindings>,
) => ({
  routeCrmImportRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    if (!IMPORT_PATTERN.test(url.pathname)) {
      return undefined
    }
    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return Effect.gen(function* () {
      const authorized = yield* Effect.tryPromise({
        catch: () => false as const,
        try: () => dependencies.requireAdminApiToken(request, env),
      })
      if (!authorized) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const args = yield* Effect.tryPromise({
        catch: () => null,
        try: () => readArgs(request, url),
      })
      if (args === null) {
        return noStoreJsonResponse(
          { error: 'bad_request', reason: 'csv body is required' },
          { status: 400 },
        )
      }

      const summary = yield* Effect.tryPromise({
        catch: error =>
          error instanceof CrmStorageError
            ? error
            : new CrmStorageError({ operation: `crm.import: ${String(error)}` }),
        try: () =>
          importCrmContactsFromCsv(crmImportDepsFromDb(makeCrmEmailDatabaseForEnv(env)), {
            csv: args.csv,
            listName: args.listName,
            listSlug: args.listSlug,
            sourceLabel: args.sourceLabel,
            tenantRef: args.tenant,
          }),
      })

      return noStoreJsonResponse({ summary })
    }).pipe(
      Effect.catch(() =>
        Effect.succeed(
          noStoreJsonResponse({ error: 'crm_import_error' }, { status: 500 }),
        ),
      ),
    )
  },
})
