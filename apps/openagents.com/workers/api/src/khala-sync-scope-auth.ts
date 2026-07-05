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
//                             member of the run's team)
//   scope.thread.<threadId>   legacy D1 agent-run/autopilot-thread ownership
//                             OR owner-private MC-1 chat ownership via
//                             Khala Sync Postgres `khala_sync_scope_owners`
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
//
// This module returns typed {@link ScopeReadDecision} values only; the
// shared decision → HTTP response mapper the read routes consume is
// `scopeReadDecisionResponse` in ./http/khala-sync-scope-read-response.

import { threadScope, type SyncScope } from '@openagentsinc/khala-sync'
import {
  readScopeOwner,
  resolveScopeRead,
  type ScopeReadDecision,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import type {
  KhalaSyncHyperdriveBinding,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { readActiveTeamMembershipRole } from './team-repository'
import {
  readAgentRunAccessRowAsync,
  resolveAgentRunIdAsync,
  resolveAgentRunIdForAutopilotThreadAsync,
} from './thread-access'

/**
 * The injected route seam: one decision per (userId, scope) read attempt.
 * `userId === undefined` models an anonymous caller (KS-8.x anonymous-read
 * exception) — `resolveScopeRead` grants that ONLY for `scope.public.*`;
 * every other kind denies an anonymous caller before any capability
 * callback runs. Route handlers decide whether to pass `undefined` here by
 * calling `isAnonymousReadableScope` (from `@openagentsinc/khala-sync-server`)
 * BEFORE requiring `authenticate()` to succeed.
 */
export type KhalaSyncScopeReadResolver = (
  userId: string | undefined,
  scope: SyncScope,
) => Promise<ScopeReadDecision>

export type KhalaSyncScopeAuthDeps = Readonly<{
  /**
   * The OPENAGENTS_DB D1 database holding memberships/ownership, obtained
   * at the route boundary through the runtime capability accessor
   * (`openAgentsDatabase` in ./runtime).
   */
  db: D1Database
  /** `KHALA_SYNC_DB` — absent until the Hyperdrive binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable Postgres client factory for owner-scoped scope lookups.
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
 * Typed capability failure for an absent KHALA_SYNC_DB binding. Thrown ⇒
 * the resolver's guard maps it to `unavailable` (503), matching the KS-6.1
 * fail-closed contract: no binding never grants.
 */
class KhalaSyncBindingAbsentError extends Error {
  override readonly name = 'KhalaSyncBindingAbsentError'

  constructor() {
    super('KHALA_SYNC_DB binding is absent')
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
  const row = await readAgentRunAccessRowAsync(db, runId)
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
  const readOwnedScopeOwner = async (
    scope: SyncScope,
  ): Promise<string | null> => {
    if (
      deps.binding === undefined ||
      typeof deps.binding.connectionString !== 'string' ||
      deps.binding.connectionString.length === 0
    ) {
      throw new KhalaSyncBindingAbsentError()
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
            await resolveAgentRunIdAsync(deps.db, runId),
          ),
        canReadThread: async (uid, threadId) =>
          (await canReadResolvedRun(
            deps.db,
            uid,
            (await resolveAgentRunIdAsync(deps.db, threadId)) ??
              (await resolveAgentRunIdForAutopilotThreadAsync(
                deps.db,
                threadId,
              )),
          )) || (await readOwnedScopeOwner(threadScope(threadId))) === uid,
        isTeamMember: async (uid, teamId) =>
          (await readActiveTeamMembershipRole(deps.db, teamId, uid)) !==
          undefined,
        readFleetScopeOwner: readOwnedScopeOwner,
      },
      userId,
      scope,
    )
}
