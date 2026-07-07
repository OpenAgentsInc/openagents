// Seam A (#8503, AC-1) — admin-guarded single-turn dispatch trigger.
//
// THE GAP THIS CLOSES. `dispatchCloudGcpRuntimeTurn`
// (`khala-cloud-runtime-dispatch.ts`) is the server-owned `cloud-gcp` consumer,
// but there is no persistent admitted-work-context queue to feed it yet. This
// route is the REACHABLE trigger: an admin POSTs ONE admitted `cloud-gcp`
// work-context (owner + repo + pinned commit + thread + turn) and the handler
// drives the full Seam A chain — mint owner-linked execution token -> build the
// inference block + base64 work-context -> POST `/v1/placement` through the
// cloud-control adapter -> stream owner-attributed runtime events -> return the
// outcome, INCLUDING the minted `credentialId` so the caller can revoke the
// token once the async microVM turn's lifecycle terminal is observed.
//
// FAIL-CLOSED. `dispatchCloudGcpRuntimeTurn` does NOT itself gate on `armed`
// (only the batch runner `runCloudGcpRuntimeDispatch` does). So this route
// enforces the not-armed posture explicitly: when the `cloud-gcp` lane is not
// armed (the production default), it refuses with 409 `not_armed` and NEVER
// mints a token, builds a work-context, or POSTs a placement. It also fails
// closed with 503 when the dispatch context is not configured (no
// `KHALA_SYNC_DB` connection / no cloud-control endpoint). PROD stays default-
// off until an operator arms staging/prod (`OA_CODEX_GCE_PROVISIONER=live`).
//
// AUTH. Admin-only via the injected `requireAdminApiToken` seam (the shared
// Worker admin bearer). No user session, no public reachability.
//
// SECRET DISCIPLINE. The minted raw bearer + no-meter secret live only inside
// `dispatchCloudGcpRuntimeTurn`'s in-memory work-context build; this route never
// reads or returns them — only the non-secret `credentialId`, `placementRef`,
// `sessionId`, and outcome are echoed back.

import { methodNotAllowed, noStoreJsonResponse, serverError, unauthorized } from './http/responses'
import type {
  CloudGcpAdmittedWorkContext,
  CloudGcpDispatchOutcome,
} from './khala-cloud-runtime-dispatch'

/** Admin-only reachable trigger path for a single `cloud-gcp` dispatch. */
export const KHALA_CLOUD_RUNTIME_DISPATCH_ADMIN_PATH =
  '/api/admin/khala/cloud/runtime-dispatch'

/**
 * A resolved dispatch context. `configured:false` means the environment has no
 * `KHALA_SYNC_DB` connection / cloud-control config wired (503). When
 * configured, `armed` reflects `isCloudGceProvisioningArmed(...)` and `run`
 * encapsulates the SQL-client lifecycle around a single
 * {@link dispatchCloudGcpRuntimeTurn}.
 */
export type CloudGcpRuntimeDispatchContext =
  | Readonly<{
      configured: true
      armed: boolean
      run: (admitted: CloudGcpAdmittedWorkContext) => Promise<CloudGcpDispatchOutcome>
    }>
  | Readonly<{ configured: false }>

export type CloudGcpRuntimeDispatchAdminRouteDeps<Env> = Readonly<{
  /** Shared Worker admin-bearer gate. */
  requireAdminApiToken: (request: Request, env: Env) => Promise<boolean>
  /** Resolve the dispatch context (SQL client + armed flag + run closure). */
  resolveContext: (env: Env) => Promise<CloudGcpRuntimeDispatchContext>
  log?: ((line: string, fields?: Record<string, unknown>) => void) | undefined
}>

type ParsedBody =
  | Readonly<{ ok: true; value: CloudGcpAdmittedWorkContext }>
  | Readonly<{ ok: false; error: string }>

const asString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.length > 0 ? value : undefined

const parseAdmittedBody = (raw: unknown): ParsedBody => {
  if (raw === null || typeof raw !== 'object') {
    return { error: 'body_must_be_object', ok: false }
  }
  const body = raw as Record<string, unknown>
  const ownerUserId = asString(body['ownerUserId'])
  const threadId = asString(body['threadId'])
  const turnId = asString(body['turnId'])
  const workContextRef = asString(body['workContextRef'])
  const repo = asString(body['repo'])
  const commit = asString(body['commit'])
  const missing: Array<string> = []
  if (ownerUserId === undefined) missing.push('ownerUserId')
  if (threadId === undefined) missing.push('threadId')
  if (turnId === undefined) missing.push('turnId')
  if (workContextRef === undefined) missing.push('workContextRef')
  if (repo === undefined) missing.push('repo')
  if (commit === undefined) missing.push('commit')
  if (
    ownerUserId === undefined ||
    threadId === undefined ||
    turnId === undefined ||
    workContextRef === undefined ||
    repo === undefined ||
    commit === undefined
  ) {
    return { error: `missing_fields:${missing.join(',')}`, ok: false }
  }

  const eventCountRaw = body['eventCount']
  const eventCount =
    eventCountRaw === undefined
      ? 0
      : typeof eventCountRaw === 'number' && Number.isSafeInteger(eventCountRaw) && eventCountRaw >= 0
        ? eventCountRaw
        : undefined
  if (eventCount === undefined) {
    return { error: 'event_count_must_be_nonnegative_integer', ok: false }
  }

  const branch = asString(body['branch'])
  const objective = asString(body['objective'])
  const repoBindingRef = asString(body['repoBindingRef'])
  const runtimeLane = asString(body['runtimeLane'])

  // MM-C5 (#8477) optional writeback block. Present => the microVM pushes a
  // scoped branch / opens a PR under the user GitHub authorization and records
  // the thread-scoped writeback.recorded event. `mode` must be a known literal.
  const writebackRaw = body['writeback']
  let writeback: CloudGcpAdmittedWorkContext['writeback'] | undefined
  if (writebackRaw !== undefined) {
    if (writebackRaw === null || typeof writebackRaw !== 'object') {
      return { error: 'writeback_must_be_object', ok: false }
    }
    const wb = writebackRaw as Record<string, unknown>
    const mode = wb['mode']
    if (mode !== undefined && mode !== 'branch_only' && mode !== 'pull_request') {
      return { error: 'writeback_mode_invalid', ok: false }
    }
    const wbBranch = asString(wb['branch'])
    const wbBaseBranch = asString(wb['baseBranch'])
    writeback = {
      ...(mode === undefined ? {} : { mode: mode as 'branch_only' | 'pull_request' }),
      ...(wbBranch === undefined ? {} : { branch: wbBranch }),
      ...(wbBaseBranch === undefined ? {} : { baseBranch: wbBaseBranch }),
    }
  }

  return {
    ok: true,
    value: {
      commit,
      eventCount,
      ownerUserId,
      repo,
      threadId,
      turnId,
      workContextRef,
      ...(branch === undefined ? {} : { branch }),
      ...(objective === undefined ? {} : { objective }),
      ...(repoBindingRef === undefined ? {} : { repoBindingRef }),
      ...(runtimeLane === undefined
        ? {}
        : { runtimeLane: runtimeLane as CloudGcpAdmittedWorkContext['runtimeLane'] }),
      ...(writeback === undefined ? {} : { writeback }),
    },
  }
}

/**
 * Handle a POST to {@link KHALA_CLOUD_RUNTIME_DISPATCH_ADMIN_PATH}. Admin-only.
 * Seeds ONE admitted `cloud-gcp` work-context from the request body and runs a
 * single {@link dispatchCloudGcpRuntimeTurn}. Fail-closed: 401 without admin,
 * 503 when not configured, 409 `not_armed` when the lane is not armed (no
 * mint/placement), 400 on a malformed body.
 */
export const handleCloudGcpRuntimeDispatchAdminRoute = async <Env>(
  request: Request,
  env: Env,
  deps: CloudGcpRuntimeDispatchAdminRouteDeps<Env>,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }
  if (!(await deps.requireAdminApiToken(request, env))) {
    return unauthorized()
  }

  let rawBody: unknown
  try {
    rawBody = await request.json()
  } catch {
    return noStoreJsonResponse({ error: 'invalid_json', ok: false }, { status: 400 })
  }
  const parsed = parseAdmittedBody(rawBody)
  if (!parsed.ok) {
    return noStoreJsonResponse({ error: parsed.error, ok: false }, { status: 400 })
  }

  let context: CloudGcpRuntimeDispatchContext
  try {
    context = await deps.resolveContext(env)
  } catch (error) {
    deps.log?.('cloud_gcp_runtime_dispatch_admin_resolve_threw', {
      detail: error instanceof Error ? error.message : 'unknown',
    })
    return serverError()
  }

  if (!context.configured) {
    return noStoreJsonResponse(
      { error: 'cloud_gcp_runtime_not_configured', ok: false },
      { status: 503 },
    )
  }

  // FAIL-CLOSED: refuse before any mint/placement when the lane is not armed.
  if (!context.armed) {
    deps.log?.('cloud_gcp_runtime_dispatch_admin_not_armed', {
      turnId: parsed.value.turnId,
    })
    return noStoreJsonResponse(
      { armed: false, ok: false, reason: 'cloud_gcp_runtime_not_armed' },
      { status: 409 },
    )
  }

  let outcome: CloudGcpDispatchOutcome
  try {
    outcome = await context.run(parsed.value)
  } catch (error) {
    deps.log?.('cloud_gcp_runtime_dispatch_admin_run_threw', {
      detail: error instanceof Error ? error.message : 'unknown',
      turnId: parsed.value.turnId,
    })
    return serverError()
  }

  return noStoreJsonResponse(
    {
      armed: true,
      ok: outcome.outcome === 'launched',
      outcome: outcome.outcome,
      tokenRevoked: outcome.tokenRevoked,
      ...(outcome.credentialId === undefined ? {} : { credentialId: outcome.credentialId }),
      ...(outcome.placementRef === undefined ? {} : { placementRef: outcome.placementRef }),
      ...(outcome.sessionId === undefined ? {} : { sessionId: outcome.sessionId }),
      ...(outcome.reason === undefined ? {} : { reason: outcome.reason }),
    },
    { status: 200 },
  )
}
