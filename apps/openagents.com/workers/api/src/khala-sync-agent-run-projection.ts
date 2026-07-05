// Khala Sync agent run + goal projection (KS-6.6, #8416).
//
// Best-effort projection of a just-queued/relaunched agent run (with its
// currently-attached goal) into `scope.agent_run.<runId>` via the
// KHALA_SYNC_DB Hyperdrive binding. This is now the SOLE producer at the
// three `omni-handlers.ts` call sites this issue targets (mission launch,
// goal continuation-after-completed-run, and the API mission launch) — the
// legacy `notifySyncScopes(env, syncScopeForAgentRun(run))` poke at those
// same sites was deleted (2026-07-05, #8416 final pass) once the web
// client's active-run WebSocket was repointed to the khala-sync
// `/api/sync/connect` surface (`apps/web/src/subscriptions.ts`, commit
// `6ff849527f`) and proven correct. See docs/khala-sync/RUNBOOK.md's
// "2026-07-05 legacy poke deleted" subsection for the full disposition,
// including the production evidence (zero `agent_runs` rows created since
// 2026-06-07, so the mission-launch feature itself was simply quiet — not
// obsolete) that made the deletion safe.
//
// FAIL-SOFT CONTRACT (same discipline as KS-6.1 fleet / KS-6.3
// tokens-served / KS-6.5 gym run-progress): a projection failure — missing
// binding, unreachable Postgres, redaction refusal — NEVER fails the
// queued-run response. Every outcome is a value; nothing here throws.
//
// AUTH STATUS: unlike KS-6.4/KS-6.5 (blocked on an anonymous-read gap),
// `scope.agent_run.<runId>` is AUTHENTICATED-ONLY (run owner or an active
// team member — see `khala-sync-scope-auth.ts`'s `canReadResolvedRun`), so
// there is no anonymous-read blocker here.
//
// EVENT-FEED FOLLOW-UP (KS-6.6 producer-completeness pass): the
// 2026-07-05 client-repoint research recorded in RUNBOOK.md found two real
// gaps blocking a safe client repoint — (1) `AgentRunEntity` alone had no
// equivalent of the legacy scope's `agent_run_events` transcript collection,
// and (2) `projectAgentRun` above only fired at the three run-CREATION call
// sites, never on the ONGOING `appendAgentRunEvents` path that fires
// throughout a run's life. `projectAgentRunEvents` below closes gap (1) —
// see `agent-runtime-store.ts`'s `makeOmniRunStoreForEnv` for how BOTH this
// function and `projectAgentRun` are now wired into every
// `saveAgentRun`/`appendAgentRunEvents` call unconditionally, closing gap
// (2). Both gaps were closed before the client repoint (which then landed)
// and before this legacy-poke deletion.

import {
  type AgentRunProjectionDiagnostic,
  projectAgentRunBestEffort,
  projectAgentRunEventsBestEffort,
} from '@openagentsinc/khala-sync-server'

import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { defaultMakeKhalaSyncSqlClient } from './khala-sync-push-routes'

export type AgentRunProjectionOutcome =
  | { readonly outcome: 'projected'; readonly runId: string }
  | { readonly outcome: 'skipped_no_binding' }
  | {
      readonly outcome: 'failed'
      readonly runId: string
      readonly diagnostic: AgentRunProjectionDiagnostic
    }

export type AgentRunProjectionLog = (
  event: 'khala_sync_agent_run_projection_failed',
  fields: Readonly<{ reason: string; runId: string; messageSafe: string }>,
) => void

export type ProjectAgentRunDependencies = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /**
   * Injectable transaction-mode-safe client factory (same seam as the push
   * route / gym run-progress projection). Tests inject a fake; production
   * uses the postgres.js default.
   */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Diagnostic sink for failed projections (public-safe fields only). */
  log?: AgentRunProjectionLog | undefined
}>

/**
 * Project one queued/relaunched agent run into `scope.agent_run.<runId>`.
 * Never throws; the returned outcome is for logging/metrics only — callers
 * must not branch queued-run response behavior on it.
 *
 * `raw` should be the already public-safe run + goal-context shape (see
 * `agent-run.ts`'s `AgentRunEntity` doc header for the exact field
 * allowlist); this function decodes it through the contract before
 * anything is serialized.
 */
export const projectAgentRun = async (
  deps: ProjectAgentRunDependencies,
  runId: string,
  raw: unknown,
): Promise<AgentRunProjectionOutcome> => {
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
    const result = await projectAgentRunBestEffort(client.sql, raw)
    if (result.ok) {
      return { outcome: 'projected', runId }
    }
    deps.log?.('khala_sync_agent_run_projection_failed', {
      messageSafe: result.diagnostic.messageSafe,
      reason: result.diagnostic.reason,
      runId,
    })
    return {
      diagnostic: result.diagnostic,
      outcome: 'failed',
      runId,
    }
  } catch {
    // Client construction/teardown failures: still fail-soft. Never echo
    // driver errors (they can embed the DSN).
    const diagnostic: AgentRunProjectionDiagnostic = {
      messageSafe: 'agent run projection client failed',
      reason: 'projection_failed',
    }
    deps.log?.('khala_sync_agent_run_projection_failed', {
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

export type AgentRunEventsProjectionOutcome =
  | { readonly outcome: 'projected'; readonly runId: string; readonly count: number }
  | { readonly outcome: 'skipped_no_binding' }
  | { readonly outcome: 'skipped_no_events' }
  | {
      readonly outcome: 'failed'
      readonly runId: string
      readonly diagnostic: AgentRunProjectionDiagnostic
    }

/**
 * Project a batch of agent-run events into `scope.agent_run.<runId>` as
 * companion `agent_run_event` entities (KS-6.6 event-feed follow-up,
 * #8416) — closes the "schema gap" from RUNBOOK.md's 2026-07-05
 * client-repoint research: the legacy scope multiplexes `agent_runs` AND
 * `agent_run_events` onto one room, but `AgentRunEntity` alone had no
 * equivalent of the latter.
 *
 * Never throws; the returned outcome is for logging/metrics only — callers
 * must not branch runner-event ingest response behavior on it. `rawEvents`
 * should be the already public-safe per-event shape (see `agent-run.ts`'s
 * `AgentRunEventEntity` doc header); this function decodes each one through
 * the contract before anything is serialized.
 */
export const projectAgentRunEvents = async (
  deps: ProjectAgentRunDependencies,
  runId: string,
  rawEvents: ReadonlyArray<unknown>,
): Promise<AgentRunEventsProjectionOutcome> => {
  if (rawEvents.length === 0) {
    return { outcome: 'skipped_no_events' }
  }

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
    const result = await projectAgentRunEventsBestEffort(
      client.sql,
      runId,
      rawEvents,
    )
    if (result.ok) {
      return { count: result.entries.length, outcome: 'projected', runId }
    }
    deps.log?.('khala_sync_agent_run_projection_failed', {
      messageSafe: result.diagnostic.messageSafe,
      reason: result.diagnostic.reason,
      runId,
    })
    return {
      diagnostic: result.diagnostic,
      outcome: 'failed',
      runId,
    }
  } catch {
    // Client construction/teardown failures: still fail-soft. Never echo
    // driver errors (they can embed the DSN).
    const diagnostic: AgentRunProjectionDiagnostic = {
      messageSafe: 'agent run event projection client failed',
      reason: 'projection_failed',
    }
    deps.log?.('khala_sync_agent_run_projection_failed', {
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
