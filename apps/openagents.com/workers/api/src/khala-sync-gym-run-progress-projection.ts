// Khala Sync gym run-progress dual-write (KS-6.5, #8415).
//
// Best-effort projection of every ingested Gym / Harbor live run-progress
// snapshot into `scope.public.gym-run-progress` via the KHALA_SYNC_DB
// Hyperdrive binding — invoked ALONGSIDE the legacy sync-worker outbox
// append in `inference/gym/run-progress-sync.ts`'s
// `publishGymRunProgressSnapshot`.
//
// FAIL-SOFT CONTRACT (same discipline as KS-6.1 fleet / KS-6.3
// tokens-served): a projection failure — missing binding, unreachable
// Postgres, redaction refusal — NEVER fails the ingest path. Every outcome
// is a value; nothing here throws.
//
// HONEST STATUS: this is a DUAL-WRITE ADDITION, not a cutover. The `/gym`
// panel is read by ANONYMOUS/logged-out visitors, and
// `GET/WS /api/sync/connect` (+ /log, /bootstrap) require an authenticated
// actor even for `scope.public.*` reads — there is no anonymous read path on
// the khala-sync connect surface yet (see
// `khala-sync-connect-routes.ts`'s `deps.authenticate()` gate). So the
// legacy sync-worker producer remains the ONLY delivery path for anonymous
// `/gym` visitors; this projection exists for Postgres parity / future
// migration, matching what KS-6.3 (#8304) already did for tokens-served
// before ITS OWN client repoint. Do not delete the legacy producer or
// repoint `apps/web/src/subscriptions.ts`'s `GYM_RUN_PROGRESS_SCOPE` before
// that anonymous-read gap is closed — see docs/khala-sync/RUNBOOK.md.

import {
  type GymRunProgressProjectionDiagnostic,
  projectGymRunProgressBestEffort,
  type RawGymRunProgressProjection,
} from '@openagentsinc/khala-sync-server'

import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { defaultMakeKhalaSyncSqlClient } from './khala-sync-push-routes'

export type GymRunProgressProjectionOutcome =
  | { readonly outcome: 'projected'; readonly runRef: string }
  | { readonly outcome: 'skipped_no_binding' }
  | {
      readonly outcome: 'failed'
      readonly runRef: string
      readonly diagnostic: GymRunProgressProjectionDiagnostic
    }

export type GymRunProgressProjectionLog = (
  event: 'khala_sync_gym_run_progress_projection_failed',
  fields: Readonly<{ reason: string; runRef: string; messageSafe: string }>,
) => void

export type ProjectGymRunProgressDependencies = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable transaction-mode-safe client factory (same seam as the push
   * route / fleet projection). Tests inject a fake; production uses the
   * postgres.js default.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Diagnostic sink for failed projections (public-safe fields only). */
  log?: GymRunProgressProjectionLog | undefined
}>

/**
 * Project one gym run-progress snapshot into
 * `scope.public.gym-run-progress`. Never throws; the returned outcome is
 * for logging/metrics only — callers must not branch ingest behavior on it.
 */
export const projectGymRunProgress = async (
  deps: ProjectGymRunProgressDependencies,
  projection: RawGymRunProgressProjection,
): Promise<GymRunProgressProjectionOutcome> => {
  if (
    deps.binding === undefined ||
    typeof deps.binding.connectionString !== 'string' ||
    deps.binding.connectionString.length === 0
  ) {
    return { outcome: 'skipped_no_binding' }
  }

  const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  let client: KhalaSyncPushSqlClient | undefined
  try {
    client = await makeSqlClient(deps.binding.connectionString)
    const result = await projectGymRunProgressBestEffort(
      client.sql,
      projection,
    )
    if (result.ok) {
      return { outcome: 'projected', runRef: projection.runRef }
    }
    deps.log?.('khala_sync_gym_run_progress_projection_failed', {
      messageSafe: result.diagnostic.messageSafe,
      reason: result.diagnostic.reason,
      runRef: projection.runRef,
    })
    return {
      diagnostic: result.diagnostic,
      outcome: 'failed',
      runRef: projection.runRef,
    }
  } catch {
    // Client construction/teardown failures: still fail-soft. Never echo
    // driver errors (they can embed the DSN).
    const diagnostic: GymRunProgressProjectionDiagnostic = {
      messageSafe: 'gym run-progress projection client failed',
      reason: 'projection_failed',
    }
    deps.log?.('khala_sync_gym_run_progress_projection_failed', {
      messageSafe: diagnostic.messageSafe,
      reason: diagnostic.reason,
      runRef: projection.runRef,
    })
    return { diagnostic, outcome: 'failed', runRef: projection.runRef }
  } finally {
    if (client !== undefined) {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }
}
