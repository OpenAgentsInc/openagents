// Khala Sync agent run + goal dual-write (KS-6.6, #8416).
//
// Best-effort projection of a just-queued/relaunched agent run (with its
// currently-attached goal) into `scope.agent_run.<runId>` via the
// KHALA_SYNC_DB Hyperdrive binding — invoked ALONGSIDE the legacy
// `notifySyncScopes(env, syncScopeForAgentRun(run))` poke at the three
// `omni-handlers.ts` call sites this issue targets (mission launch, goal
// continuation-after-completed-run, and the API mission launch).
//
// FAIL-SOFT CONTRACT (same discipline as KS-6.1 fleet / KS-6.3
// tokens-served / KS-6.5 gym run-progress): a projection failure — missing
// binding, unreachable Postgres, redaction refusal — NEVER fails the
// queued-run response. Every outcome is a value; nothing here throws.
//
// HONEST STATUS: unlike KS-6.4/KS-6.5 (blocked on an anonymous-read gap),
// `scope.agent_run.<runId>` is AUTHENTICATED-ONLY (run owner or an active
// team member — see `khala-sync-scope-auth.ts`'s `canReadResolvedRun`), so
// there is no anonymous-read blocker here. The reason the legacy
// `notifySyncScopes` calls stay live is different: the web client
// (`apps/web/src/subscriptions.ts`'s `syncScopesForModel` /
// `syncAgentRunScope`) still opens the LEGACY `/api/sync/agent-run/<id>/
// stream` WebSocket for the active chat run — it has not been repointed to
// `GET/WS /api/sync/connect` (khala-sync) for this scope. Deleting the
// legacy poke before that client repoint lands would silently break live
// run/goal updates on the chat page. See docs/khala-sync/RUNBOOK.md.

import {
  type AgentRunProjectionDiagnostic,
  projectAgentRunBestEffort,
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
