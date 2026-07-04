// Khala Sync fleet cockpit dual-write (KS-6.1, #8302).
//
// Best-effort projection of Worker-side assignment status transitions into
// `scope.fleet_run.<runId>` changelog entries via the KHALA_SYNC_DB
// Hyperdrive binding — invoked AFTER the authoritative D1 business write
// commits, from the Pylon assignment routes (create + closeout/status
// transitions).
//
// FAIL-SOFT CONTRACT (v1 dual-write): a projection failure — missing
// binding, unreachable Postgres, redaction refusal, foreign scope owner —
// NEVER fails the D1 business write. Every outcome is a value; nothing
// here throws. KS-8.1 (#8307) retires this dual-write by moving the
// assignment business write into the same Postgres transaction as the
// changelog append (SPEC §7 invariant 5).
//
// SCOPE DERIVATION: the Worker assignment record has no first-class fleet
// run column; fleet-dispatched assignments carry their run ref inside the
// `codingAssignment` payload (`fleetRunRef`). Assignments without a valid
// public-safe `fleetRunRef` are SKIPPED (outcome `skipped_no_fleet_run_ref`)
// — nothing is projected rather than inventing a scope.
//
// REDACTION (SPEC §7 invariant 9): only the public-safe scalar slice of
// the assignment reaches the projector (`fleetAssignmentPostImage`
// allowlist → fleet entity contract decode → forbidden-material guard).
// The codingAssignment payload itself — which can carry prompts and
// workspace paths — is read ONLY for the two bounded ref fields and never
// serialized.

import {
  fleetAssignmentPostImage,
  projectFleetEntitiesBestEffort,
  type FleetProjectionDiagnostic,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import type { PylonApiAssignmentRecord } from './pylon-api'

export type FleetAssignmentProjectionOutcome =
  | { readonly outcome: 'projected'; readonly runId: string }
  | { readonly outcome: 'skipped_no_fleet_run_ref' }
  | { readonly outcome: 'skipped_no_binding' }
  | {
      readonly outcome: 'failed'
      readonly runId: string
      readonly diagnostic: FleetProjectionDiagnostic
    }

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/
const ISSUE_REF_PATTERN =
  /^([A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._-]*)?#\d+$/

/**
 * The fleet run ref a coding assignment was dispatched under, or null.
 * Bounded public-safe refs only — anything else behaves as absent.
 */
export const fleetRunRefFromAssignment = (
  assignment: Pick<PylonApiAssignmentRecord, 'codingAssignment'>,
): string | null => {
  const raw = assignment.codingAssignment?.fleetRunRef
  return typeof raw === 'string' && PUBLIC_REF_PATTERN.test(raw) ? raw : null
}

const issueRefFromAssignment = (
  assignment: Pick<PylonApiAssignmentRecord, 'codingAssignment'>,
): string | null => {
  const raw = assignment.codingAssignment?.issueRef
  return typeof raw === 'string' && ISSUE_REF_PATTERN.test(raw) ? raw : null
}

export type FleetProjectionLog = (
  event: 'khala_sync_fleet_projection_failed',
  fields: Readonly<{ reason: string; runId: string; messageSafe: string }>,
) => void

export type ProjectFleetAssignmentDependencies = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable transaction-mode-safe client factory (same seam as the push
   * route). Tests inject a fake; production uses the postgres.js default.
   */
  makeSqlClient: MakeKhalaSyncPushSqlClient
  /** Diagnostic sink for failed projections (public-safe fields only). */
  log?: FleetProjectionLog | undefined
}>

/**
 * Project one assignment status transition as a `fleet_assignment` upsert
 * into its fleet run's scope. Never throws; the returned outcome is for
 * logging/metrics only — callers must not branch business behavior on it.
 */
export const projectFleetAssignmentTransition = async (
  deps: ProjectFleetAssignmentDependencies,
  input: Readonly<{
    assignment: PylonApiAssignmentRecord
    nowIso: string
  }>,
): Promise<FleetAssignmentProjectionOutcome> => {
  const runId = fleetRunRefFromAssignment(input.assignment)
  if (runId === null) {
    return { outcome: 'skipped_no_fleet_run_ref' }
  }
  if (
    deps.binding === undefined ||
    typeof deps.binding.connectionString !== 'string' ||
    deps.binding.connectionString.length === 0
  ) {
    return { outcome: 'skipped_no_binding' }
  }

  let client: KhalaSyncPushSqlClient | undefined
  try {
    client = await deps.makeSqlClient(deps.binding.connectionString)
    const issueRef = issueRefFromAssignment(input.assignment)
    const result = await projectFleetEntitiesBestEffort({
      changes: [
        {
          entity: fleetAssignmentPostImage({
            assignmentRef: input.assignment.assignmentRef,
            ...(issueRef === null ? {} : { issueRef }),
            state: input.assignment.state,
            updatedAt: input.assignment.updatedAt,
          }),
          kind: 'fleet_assignment',
          op: 'upsert',
        },
      ],
      ownerUserId: input.assignment.ownerAgentUserId,
      runId,
      sql: client.sql as SyncSql,
    })
    if (result.ok) {
      return { outcome: 'projected', runId }
    }
    deps.log?.('khala_sync_fleet_projection_failed', {
      messageSafe: result.diagnostic.messageSafe,
      reason: result.diagnostic.reason,
      runId,
    })
    return { diagnostic: result.diagnostic, outcome: 'failed', runId }
  } catch (error) {
    // The mapping (contract decode) itself can refuse — that is the
    // redaction boundary doing its job. Still fail-soft.
    const diagnostic: FleetProjectionDiagnostic = {
      messageSafe: 'fleet assignment mapping failed',
      reason: 'projection_failed',
    }
    void error
    deps.log?.('khala_sync_fleet_projection_failed', {
      messageSafe: diagnostic.messageSafe,
      reason: diagnostic.reason,
      runId,
    })
    return { diagnostic, outcome: 'failed', runId }
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
