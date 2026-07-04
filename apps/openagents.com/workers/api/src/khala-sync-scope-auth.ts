// Khala Sync scope-read authorization — Worker wiring (KS-7.1, #8305;
// SPEC §2.1 taxonomy, §3 auth, §7 invariant 7).
//
// The taxonomy-complete resolver lives in
// `@openagentsinc/khala-sync-server` (`resolveScopeRead` over injected
// capability callbacks). This module implements the callbacks against the
// data that actually holds membership/ownership TODAY:
//
//   scope.team.<teamId>       live D1 `team_memberships`
//                             (`readActiveTeamMembershipRole` — the same
//                             predicate the legacy sync-worker path uses in
//                             `authorizeSyncPath`)
//   scope.agent_run.<runId>   D1 `agent_runs` (owner user, or an active
//   scope.thread.<threadId>   member of the run's team — the exact
//                             `thread-access.ts` ownership rule, including
//                             the legacy-run-id and autopilot-thread
//                             mappings)
//   scope.fleet_run.<id>      Khala Sync Postgres `khala_sync_scope_owners`
//                             through the KHALA_SYNC_DB Hyperdrive binding
//                             (KS-6.1 `readScopeOwner`)
//
// FAIL-CLOSED: a missing binding or a failed lookup yields an
// `unavailable` decision (503 typed SyncError at the routes) — never a
// grant. Membership is re-read live on EVERY request, so a revoked user
// fails log/bootstrap/connect immediately; the paired push half of
// invariant 7 (broadcasting `MustRefetch(access_changed)` to already-open
// sockets) is `notifyKhalaSyncHubAccessChangedBestEffort` /
// `POST /api/internal/khala-sync/hub/access-changed` in khala-sync-hub-do.

import { Effect, Schema as S } from 'effect'

import { SyncError, type SyncScope } from '@openagentsinc/khala-sync'
import {
  readScopeOwner,
  resolveScopeRead,
  type ScopeReadDecision,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { noStoreJsonResponse } from './http/responses'
import type {
  KhalaSyncHyperdriveBinding,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { readActiveTeamMembershipRole } from './team-repository'
import {
  readAgentRunAccessRow,
  resolveAgentRunId,
  resolveAgentRunIdForAutopilotThread,
} from './thread-access'

type HttpResponse = globalThis.Response

const encodeSyncError = S.encodeSync(SyncError)

/** The injected route seam: one decision per (userId, scope) read attempt. */
export type KhalaSyncScopeReadResolver = (
  userId: string,
  scope: SyncScope,
) => Promise<ScopeReadDecision>

export type KhalaSyncScopeAuthDeps = Readonly<{
  /** `env.OPENAGENTS_DB` — the D1 database holding memberships/ownership. */
  db: D1Database
  /** `env.KHALA_SYNC_DB` — absent until the Hyperdrive binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable Postgres client factory for the fleet scope-owner lookup.
   * Default: dynamic import of `postgres` (postgres.js), Worker-runtime
   * only. Tests inject a fake — no network, no database.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

// Same transaction-mode-safe postgres.js client discipline as the push/log/
// bootstrap routes (SPEC §4): one connection, unnamed statements only, no
// session state. Duplicated rather than exported so each seam's driver
// stays independently visible and testable.
const defaultMakeSqlClient: MakeKhalaSyncPushSqlClient = async (
  connectionString,
) => {
  const mod = (await import('postgres')) as unknown as {
    default: (
      connectionString: string,
      options: Record<string, unknown>,
    ) => {
      end: (options?: { timeout?: number }) => Promise<void>
    }
  }
  const sql = mod.default(connectionString, {
    connect_timeout: 10,
    max: 1,
    prepare: false,
  })
  return {
    end: () => sql.end({ timeout: 5 }),
    sql: sql as unknown as SyncSql,
  }
}

/**
 * `agent_runs` ownership rule shared by agent_run and thread scopes: the
 * run's owning user, or (team runs) an active member of the run's team —
 * exactly `thread-access.ts` `readAuthorizedBundle` minus the bundle fetch.
 */
const canReadResolvedRun = async (
  db: D1Database,
  userId: string,
  runId: string | undefined,
): Promise<boolean> => {
  if (runId === undefined) return false
  const row = await Effect.runPromise(readAgentRunAccessRow(db, runId))
  if (row === undefined) return false
  if (row.team_id === null) return row.user_id === userId
  if (row.user_id === userId) return true
  const role = await readActiveTeamMembershipRole(db, row.team_id, userId)
  return role !== undefined
}

/**
 * Build the production scope-read resolver for the three read routes
 * (log / bootstrap / connect). Every capability is live-at-read; a failed
 * lookup or missing KHALA_SYNC_DB binding fails CLOSED as `unavailable`.
 */
export const makeKhalaSyncScopeReadResolver = (
  deps: KhalaSyncScopeAuthDeps,
): KhalaSyncScopeReadResolver => {
  const readFleetScopeOwner = async (
    scope: SyncScope,
  ): Promise<string | null> => {
    if (
      deps.binding === undefined ||
      typeof deps.binding.connectionString !== 'string' ||
      deps.binding.connectionString.length === 0
    ) {
      // Thrown ⇒ the resolver's guard maps it to `unavailable` (503),
      // matching the KS-6.1 fail-closed contract: no binding never grants.
      throw new Error('KHALA_SYNC_DB binding is absent')
    }
    const client = await (deps.makeSqlClient ?? defaultMakeSqlClient)(
      deps.binding.connectionString,
    )
    try {
      return await readScopeOwner(client.sql, scope)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown (same discipline as the read routes).
      }
    }
  }

  return (userId, scope) =>
    resolveScopeRead(
      {
        canReadAgentRun: async (uid, runId) =>
          canReadResolvedRun(
            deps.db,
            uid,
            await Effect.runPromise(resolveAgentRunId(deps.db, runId)),
          ),
        canReadThread: async (uid, threadId) =>
          canReadResolvedRun(
            deps.db,
            uid,
            (await Effect.runPromise(resolveAgentRunId(deps.db, threadId))) ??
              (await Effect.runPromise(
                resolveAgentRunIdForAutopilotThread(deps.db, threadId),
              )),
          ),
        isTeamMember: async (uid, teamId) =>
          (await readActiveTeamMembershipRole(deps.db, teamId, uid)) !==
          undefined,
        readFleetScopeOwner,
      },
      userId,
      scope,
    )
}

// ---------------------------------------------------------------------------
// Shared decision → HTTP mapping for the three read routes
// ---------------------------------------------------------------------------

/**
 * Map a non-allowed {@link ScopeReadDecision} to its typed `SyncError`
 * response; `undefined` when the read may proceed. Status map:
 * 403 `unauthorized_scope` (denied), 403 `unknown_scope` (taxonomy member
 * with no read policy — gated closed), 503 `storage_unavailable`
 * (capability failure — fail-closed, retryable).
 */
export const scopeReadDecisionResponse = (
  decision: ScopeReadDecision,
): HttpResponse | undefined => {
  if (decision.kind === 'allowed') return undefined
  if (decision.kind === 'unavailable') {
    return noStoreJsonResponse(
      encodeSyncError(
        new SyncError({
          code: 'storage_unavailable',
          messageSafe: decision.messageSafe,
          retryable: true,
        }),
      ),
      { status: 503 },
    )
  }
  return noStoreJsonResponse(
    encodeSyncError(
      new SyncError({
        code: decision.reason,
        messageSafe:
          decision.reason === 'unknown_scope'
            ? 'This scope kind has no read policy and is denied (fail-closed).'
            : 'This user cannot read the requested scope.',
        retryable: false,
      }),
    ),
    { status: 403 },
  )
}
