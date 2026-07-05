// KS-8.5 (#8316): agent runtime metadata domain — D1 → Cloud SQL migration
// machinery. Third KS-8 domain lane; mirrors the KS-8.1/8.2 templates
// (`pylon-dispatch-store.ts` #8307, `token-ledger-store.ts` #8308).
//
// Domain tables (khala-sync migration `0010_agent_runtime.sql`):
// `agent_definitions`, `agent_definition_runs`, `agent_definition_triggers`,
// `agent_runs`, `agent_run_events`, `agent_traces`, `agent_goals`,
// `agent_goal_events`. (The KS-8.5 issue's remaining tables — profiles,
// proposals, owner claims, credentials, event_ledger_entries, acceptance
// jobs/verdicts — move in the follow-up remainder lane; see
// MIGRATION_PLAN.md §3.2.)
//
// Three pieces:
//
//  1. `AgentRuntimeWriteStore` — the typed row-level repository seam:
//     converge upserts for the six state tables and dedupe-exact inserts
//     for the two event ledgers (`ON CONFLICT DO NOTHING` over the SAME
//     key sets as D1's `INSERT OR IGNORE`: id PK + (run_id, sequence) +
//     (run_id, external_event_id) for run events; id PK + partial
//     (goal_id, external_event_id) for goal events). Implementations:
//     `makeD1AgentRuntimeWriteStore` (real D1/SQLite),
//     `makePostgresAgentRuntimeStore` (KHALA_SYNC_DB Hyperdrive), and
//     `makeDualWriteAgentRuntimeWriteStore` (D1 authority + fail-soft
//     Postgres mirror). One behavioral contract suite runs against BOTH
//     concrete stores (`agent-runtime-repository.contract.test.ts`).
//
//  2. `makeAgentRuntimeMirror` — the production dual-write wiring. The
//     existing domain modules (`omni-runs.ts`, `agent-goals.ts`,
//     `agent-goal-runtime.ts`, `trace-store-d1.ts`,
//     `agent-definition-*.ts`) keep their authoritative D1 SQL; after a
//     successful D1 write the wrappers below READ BACK the affected rows
//     from D1 (point reads on the write keys) and converge-upsert the
//     byte-exact rows into Postgres. Read-back mirroring is what keeps
//     CASE-based status transitions, counter increments, and batch
//     archive+insert writes hash-identical across stores. A mirror
//     failure NEVER fails the request — it logs the typed drift
//     diagnostic `khala_sync_agent_runtime_dual_write_failed`.
//
//  3. `make*ForEnv` factories — drop-in replacements for the bare D1
//     factories at Worker call sites: definition store, definition-run
//     store, definition-trigger store, omni run store (+ the billing
//     cancel sweep), trace store, and the goal/goal-event repositories
//     (+ Effect layers). Flags:
//       KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE (default ON; off|0|false|disabled)
//       KHALA_SYNC_AGENT_RUNTIME_READS     (default 'd1'; d1|postgres|compare)
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//     Read routing in this lane covers the AgentDefinitionScheduler's
//     due-trigger scans (`listDueCronTriggers` / `listInboundWebhookTriggers`
//     — the cron this domain re-homes); all other reads stay on D1
//     authority until the runbook cutover.
//
// PRIVACY (agent_traces): traces are owner-private. The Postgres twin
// carries `visibility` / `owner_user_id` / consent columns verbatim and
// is never exposed by a new read path in this lane. Diagnostics reference
// row KEYS only (trace_uuid, run ids) — never trajectory content, goal
// objectives, or payload JSON.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Agent runtime metadata
// domain cutover"): dual-write on → backfill
// (scripts/backfill-agent-runtime.ts) → verify (exact counts, event-chain
// contiguity, trace content-hash sample) → compare reads → postgres reads
// → decommission D1 tables in a follow-up.

import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { Effect, Layer } from 'effect'

import {
  makeD1AgentDefinitionRunStore,
  type AgentDefinitionRunStore,
} from './agent-definition-run-routes'
import {
  makeD1AgentDefinitionStore,
  type AgentDefinitionStore,
} from './agent-definition-routes'
import {
  dueTriggerRecordFromRow,
  makeD1AgentDefinitionTriggerStore,
  type AgentDefinitionTriggerStore,
  type DueAgentDefinitionTriggerRecord,
  type DueAgentDefinitionTriggerRow,
} from './agent-definition-trigger-store'
import {
  AgentGoalEventRepository,
  makeD1AgentGoalEventRepository,
  type AgentGoalEventRepositoryShape,
} from './agent-goal-runtime'
import {
  AgentGoalRepository,
  makeD1AgentGoalRepository,
  systemAgentGoalRuntime,
  type AgentGoalRepositoryShape,
  type AgentGoalRuntime,
} from './agent-goals'
import {
  projectAgentRun,
  projectAgentRunEvents,
  type ProjectAgentRunDependencies,
} from './khala-sync-agent-run-projection'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import {
  agentRunEventProjection,
  agentRunSyncProjectionRaw,
  cancelActiveAgentRunsForBillingExhaustion,
  makeD1OmniRunStore,
  type AgentRunRecord,
  type BillingCanceledAgentRun,
  type OmniEventRecord,
  type OmniRunStore,
  type OmniRunStoreHooks,
} from './omni-runs'
import { openAgentsDatabase } from './runtime'
import {
  makeSupervisionLongtailMirrorForEnv,
  type SupervisionLongtailStoreEnv,
} from './supervision-longtail-domain-store'
import { makeD1TraceStore, type TraceStore } from './trace-store-d1'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type AgentRuntimeReadsMode = 'd1' | 'postgres' | 'compare'

export type AgentRuntimeFlags = Readonly<{
  dualWrite: boolean
  reads: AgentRuntimeReadsMode
}>

export type AgentRuntimeFlagEnv = Readonly<{
  KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE?: string | undefined
  KHALA_SYNC_AGENT_RUNTIME_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.5 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority until the runbook's cutover
 * sequence flips them. Unknown read values fall back to 'd1' — never
 * fail open into an unproven read path on a typo.
 */
export const agentRuntimeFlagsFromEnv = (
  env: AgentRuntimeFlagEnv,
): AgentRuntimeFlags => {
  const dualWriteRaw =
    env.KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_AGENT_RUNTIME_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type AgentRuntimeDiagnosticEvent =
  | 'khala_sync_agent_runtime_dual_write_failed'
  | 'khala_sync_agent_runtime_read_compare_mismatch'
  | 'khala_sync_agent_runtime_postgres_read_failed'
  | 'khala_sync_agent_runtime_postgres_read_fallback'

export type AgentRuntimeDiagnostic = Readonly<{
  /** The store operation, e.g. 'mirror:agent_runs'. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (ids/uuids/refs). NEVER payloads, trajectory content, objectives, or
   * summaries: agent_traces are owner-private and this log line must not
   * widen their exposure.
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type AgentRuntimeLog = (
  event: AgentRuntimeDiagnosticEvent,
  fields: AgentRuntimeDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

export type AgentRuntimeTable =
  | 'agent_definitions'
  | 'agent_definition_runs'
  | 'agent_definition_triggers'
  | 'agent_runs'
  | 'agent_run_events'
  | 'agent_traces'
  | 'agent_goals'
  | 'agent_goal_events'

/** A raw snake_case row exactly as `SELECT *` returns it from D1. */
export type AgentRuntimeRow = Readonly<Record<string, unknown>>

/**
 * The typed row-level write seam: converge upserts for state tables,
 * dedupe-exact `insert-if-absent` for the event ledgers, plus the
 * trigger-replace delete. Returns how many rows were freshly inserted
 * (event tables) or touched (state tables).
 */
export type AgentRuntimeWriteStore = Readonly<{
  upsertRows: (
    table: AgentRuntimeTable,
    rows: ReadonlyArray<AgentRuntimeRow>,
  ) => Promise<number>
  /**
   * The `replaceDefinitionTriggers` delete twin: remove triggers for
   * (owner, definition) whose trigger_ref is not in `keepTriggerRefs`
   * (empty = remove all for the definition).
   */
  deleteDefinitionTriggersNotIn: (
    ownerAgentUserId: string,
    definitionId: string,
    keepTriggerRefs: ReadonlyArray<string>,
  ) => Promise<void>
}>

/** Primary-key column(s) used by the read-back mirror, per table. */
export const AGENT_RUNTIME_TABLE_PK: Readonly<
  Record<AgentRuntimeTable, string>
> = {
  agent_definition_runs: 'run_id',
  agent_definition_triggers: 'trigger_id',
  agent_definitions: 'id',
  agent_goal_events: 'id',
  agent_goals: 'id',
  agent_run_events: 'id',
  agent_runs: 'id',
  agent_traces: 'trace_uuid',
}

const v = (row: AgentRuntimeRow, column: string): unknown => {
  const value = row[column]
  return value === undefined ? null : value
}

/**
 * `agent_goals` pages land archived-first so the partial
 * one-active-goal-per-scope unique never observes two active rows for a
 * scope mid-page (setGoal archives the predecessor and inserts the
 * successor in one batch).
 */
const orderGoalRows = (
  rows: ReadonlyArray<AgentRuntimeRow>,
): ReadonlyArray<AgentRuntimeRow> =>
  [...rows].sort((left, right) => {
    const leftActive = left['archived_at'] === null ? 1 : 0
    const rightActive = right['archived_at'] === null ? 1 : 0
    return leftActive - rightActive
  })

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresAgentRuntimeStore = AgentRuntimeWriteStore &
  Readonly<{
    /** The scheduler due-scan twin (flag-routable read). */
    listDueCronTriggerRows: (
      nowIso: string,
      limit: number,
    ) => Promise<ReadonlyArray<DueAgentDefinitionTriggerRow>>
    /** The inbound-webhook scan twin (flag-routable read). */
    listInboundWebhookTriggerRows: (
      limit: number,
    ) => Promise<ReadonlyArray<DueAgentDefinitionTriggerRow>>
  }>

export type MakePostgresAgentRuntimeStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the KS-8.1/8.2 stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresAgentRuntimeStore = (
  deps: MakePostgresAgentRuntimeStoreDependencies,
): PostgresAgentRuntimeStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  const upsertOne = async (
    sql: SyncSql,
    table: AgentRuntimeTable,
    row: AgentRuntimeRow,
  ): Promise<number> => {
    switch (table) {
      case 'agent_definitions': {
        const result: Array<{ touched: number }> = await sql`
          INSERT INTO agent_definitions (
            id, owner_agent_user_id, owner_ref, schema_literal, name, slug,
            goal, harness_json, toolset_json, triggers_json, lane,
            budget_json, escalation_json, source_refs_json, definition_json,
            created_at, updated_at, archived_at
          ) VALUES (
            ${v(row, 'id')}, ${v(row, 'owner_agent_user_id')},
            ${v(row, 'owner_ref')}, ${v(row, 'schema_literal')},
            ${v(row, 'name')}, ${v(row, 'slug')}, ${v(row, 'goal')},
            ${v(row, 'harness_json')}, ${v(row, 'toolset_json')},
            ${v(row, 'triggers_json')}, ${v(row, 'lane')},
            ${v(row, 'budget_json')}, ${v(row, 'escalation_json')},
            ${v(row, 'source_refs_json')}, ${v(row, 'definition_json')},
            ${v(row, 'created_at')}, ${v(row, 'updated_at')},
            ${v(row, 'archived_at')}
          )
          ON CONFLICT (id) DO UPDATE SET
            owner_agent_user_id = EXCLUDED.owner_agent_user_id,
            owner_ref = EXCLUDED.owner_ref,
            schema_literal = EXCLUDED.schema_literal,
            name = EXCLUDED.name,
            slug = EXCLUDED.slug,
            goal = EXCLUDED.goal,
            harness_json = EXCLUDED.harness_json,
            toolset_json = EXCLUDED.toolset_json,
            triggers_json = EXCLUDED.triggers_json,
            lane = EXCLUDED.lane,
            budget_json = EXCLUDED.budget_json,
            escalation_json = EXCLUDED.escalation_json,
            source_refs_json = EXCLUDED.source_refs_json,
            definition_json = EXCLUDED.definition_json,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at,
            archived_at = EXCLUDED.archived_at
          RETURNING 1 AS touched`
        return result.length
      }
      case 'agent_definition_runs': {
        const result: Array<{ touched: number }> = await sql`
          INSERT INTO agent_definition_runs (
            run_id, owner_agent_user_id, definition_id, definition_ref,
            trigger_ref, lane, status, pylon_ref, assignment_ref,
            durable_request_id, durable_stream_url, forge_tenant_ref,
            forge_work_ref, forge_repository_ref, forge_git_token_refs_json,
            refusal_error, refusal_reason, evidence_refs_json,
            trigger_payload_json, runtime_run_json, initial_events_json,
            budget_credits_reserved, created_at, updated_at
          ) VALUES (
            ${v(row, 'run_id')}, ${v(row, 'owner_agent_user_id')},
            ${v(row, 'definition_id')}, ${v(row, 'definition_ref')},
            ${v(row, 'trigger_ref')}, ${v(row, 'lane')}, ${v(row, 'status')},
            ${v(row, 'pylon_ref')}, ${v(row, 'assignment_ref')},
            ${v(row, 'durable_request_id')}, ${v(row, 'durable_stream_url')},
            ${v(row, 'forge_tenant_ref')}, ${v(row, 'forge_work_ref')},
            ${v(row, 'forge_repository_ref')},
            ${v(row, 'forge_git_token_refs_json') ?? '[]'},
            ${v(row, 'refusal_error')}, ${v(row, 'refusal_reason')},
            ${v(row, 'evidence_refs_json')}, ${v(row, 'trigger_payload_json')},
            ${v(row, 'runtime_run_json')}, ${v(row, 'initial_events_json')},
            ${v(row, 'budget_credits_reserved') ?? 0},
            ${v(row, 'created_at')}, ${v(row, 'updated_at')}
          )
          ON CONFLICT (run_id) DO UPDATE SET
            owner_agent_user_id = EXCLUDED.owner_agent_user_id,
            definition_id = EXCLUDED.definition_id,
            definition_ref = EXCLUDED.definition_ref,
            trigger_ref = EXCLUDED.trigger_ref,
            lane = EXCLUDED.lane,
            status = EXCLUDED.status,
            pylon_ref = EXCLUDED.pylon_ref,
            assignment_ref = EXCLUDED.assignment_ref,
            durable_request_id = EXCLUDED.durable_request_id,
            durable_stream_url = EXCLUDED.durable_stream_url,
            forge_tenant_ref = EXCLUDED.forge_tenant_ref,
            forge_work_ref = EXCLUDED.forge_work_ref,
            forge_repository_ref = EXCLUDED.forge_repository_ref,
            forge_git_token_refs_json = EXCLUDED.forge_git_token_refs_json,
            refusal_error = EXCLUDED.refusal_error,
            refusal_reason = EXCLUDED.refusal_reason,
            evidence_refs_json = EXCLUDED.evidence_refs_json,
            trigger_payload_json = EXCLUDED.trigger_payload_json,
            runtime_run_json = EXCLUDED.runtime_run_json,
            initial_events_json = EXCLUDED.initial_events_json,
            budget_credits_reserved = EXCLUDED.budget_credits_reserved,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at
          RETURNING 1 AS touched`
        return result.length
      }
      case 'agent_definition_triggers': {
        // Arbiter = (owner, trigger_ref): the LIVE upsert key. The D1
        // path REPLACES trigger_id under that key on definition
        // re-registration, so converging on the PK would strand ids.
        const result: Array<{ touched: number }> = await sql`
          INSERT INTO agent_definition_triggers (
            trigger_id, owner_agent_user_id, owner_ref, definition_id,
            trigger_ref, trigger_kind, trigger_json, state,
            consecutive_failures, next_run_at, paused_at, pause_reason,
            created_at, updated_at
          ) VALUES (
            ${v(row, 'trigger_id')}, ${v(row, 'owner_agent_user_id')},
            ${v(row, 'owner_ref')}, ${v(row, 'definition_id')},
            ${v(row, 'trigger_ref')}, ${v(row, 'trigger_kind')},
            ${v(row, 'trigger_json')}, ${v(row, 'state')},
            ${v(row, 'consecutive_failures') ?? 0}, ${v(row, 'next_run_at')},
            ${v(row, 'paused_at')}, ${v(row, 'pause_reason')},
            ${v(row, 'created_at')}, ${v(row, 'updated_at')}
          )
          ON CONFLICT (owner_agent_user_id, trigger_ref) DO UPDATE SET
            trigger_id = EXCLUDED.trigger_id,
            owner_ref = EXCLUDED.owner_ref,
            definition_id = EXCLUDED.definition_id,
            trigger_kind = EXCLUDED.trigger_kind,
            trigger_json = EXCLUDED.trigger_json,
            state = EXCLUDED.state,
            consecutive_failures = EXCLUDED.consecutive_failures,
            next_run_at = EXCLUDED.next_run_at,
            paused_at = EXCLUDED.paused_at,
            pause_reason = EXCLUDED.pause_reason,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at
          RETURNING 1 AS touched`
        return result.length
      }
      case 'agent_runs': {
        const result: Array<{ touched: number }> = await sql`
          INSERT INTO agent_runs (
            id, user_id, team_id, project_id, runtime, backend, runner_id,
            assignment_kind, repository_provider, repository_owner,
            repository_repo, repository_ref, goal, goal_id,
            provider_account_ref, auth_grant_ref, external_run_id, status,
            event_cursor, assignment_json, created_at, updated_at,
            started_at, completed_at, failed_at, canceled_at, archived_at
          ) VALUES (
            ${v(row, 'id')}, ${v(row, 'user_id')}, ${v(row, 'team_id')},
            ${v(row, 'project_id')}, ${v(row, 'runtime')},
            ${v(row, 'backend')}, ${v(row, 'runner_id')},
            ${v(row, 'assignment_kind')}, ${v(row, 'repository_provider')},
            ${v(row, 'repository_owner')}, ${v(row, 'repository_repo')},
            ${v(row, 'repository_ref')}, ${v(row, 'goal')},
            ${v(row, 'goal_id')}, ${v(row, 'provider_account_ref')},
            ${v(row, 'auth_grant_ref')}, ${v(row, 'external_run_id')},
            ${v(row, 'status')}, ${v(row, 'event_cursor') ?? 0},
            ${v(row, 'assignment_json')}, ${v(row, 'created_at')},
            ${v(row, 'updated_at')}, ${v(row, 'started_at')},
            ${v(row, 'completed_at')}, ${v(row, 'failed_at')},
            ${v(row, 'canceled_at')}, ${v(row, 'archived_at')}
          )
          ON CONFLICT (id) DO UPDATE SET
            user_id = EXCLUDED.user_id,
            team_id = EXCLUDED.team_id,
            project_id = EXCLUDED.project_id,
            runtime = EXCLUDED.runtime,
            backend = EXCLUDED.backend,
            runner_id = EXCLUDED.runner_id,
            assignment_kind = EXCLUDED.assignment_kind,
            repository_provider = EXCLUDED.repository_provider,
            repository_owner = EXCLUDED.repository_owner,
            repository_repo = EXCLUDED.repository_repo,
            repository_ref = EXCLUDED.repository_ref,
            goal = EXCLUDED.goal,
            goal_id = EXCLUDED.goal_id,
            provider_account_ref = EXCLUDED.provider_account_ref,
            auth_grant_ref = EXCLUDED.auth_grant_ref,
            external_run_id = EXCLUDED.external_run_id,
            status = EXCLUDED.status,
            event_cursor = EXCLUDED.event_cursor,
            assignment_json = EXCLUDED.assignment_json,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at,
            started_at = EXCLUDED.started_at,
            completed_at = EXCLUDED.completed_at,
            failed_at = EXCLUDED.failed_at,
            canceled_at = EXCLUDED.canceled_at,
            archived_at = EXCLUDED.archived_at
          RETURNING 1 AS touched`
        return result.length
      }
      case 'agent_run_events': {
        // Bare DO NOTHING covers id PK + (run_id, sequence) +
        // (run_id, external_event_id) — the exact D1 INSERT OR IGNORE keys.
        const result: Array<{ inserted: number }> = await sql`
          INSERT INTO agent_run_events (
            id, run_id, sequence, type, summary, status, source,
            payload_json, artifact_refs_json, external_event_id, created_at
          ) VALUES (
            ${v(row, 'id')}, ${v(row, 'run_id')}, ${v(row, 'sequence')},
            ${v(row, 'type')}, ${v(row, 'summary')}, ${v(row, 'status')},
            ${v(row, 'source')}, ${v(row, 'payload_json')},
            ${v(row, 'artifact_refs_json') ?? '[]'},
            ${v(row, 'external_event_id')}, ${v(row, 'created_at')}
          )
          ON CONFLICT DO NOTHING
          RETURNING 1 AS inserted`
        return result.length
      }
      case 'agent_traces': {
        const result: Array<{ touched: number }> = await sql`
          INSERT INTO agent_traces (
            trace_uuid, owner_user_id, agent_ref, schema_version,
            trajectory_id, session_id, visibility, step_count,
            trajectory_json, trajectory_r2_key, blob_refs_json,
            idempotency_key, training_consent, license, content_digest,
            reward_eligible, reward_amount_sats, upload_source,
            demand_kind, demand_source, created_at, updated_at
          ) VALUES (
            ${v(row, 'trace_uuid')}, ${v(row, 'owner_user_id')},
            ${v(row, 'agent_ref')}, ${v(row, 'schema_version')},
            ${v(row, 'trajectory_id')}, ${v(row, 'session_id')},
            ${v(row, 'visibility')}, ${v(row, 'step_count') ?? 0},
            ${v(row, 'trajectory_json') ?? '{}'},
            ${v(row, 'trajectory_r2_key')},
            ${v(row, 'blob_refs_json') ?? '[]'},
            ${v(row, 'idempotency_key')},
            ${v(row, 'training_consent') ?? 0}, ${v(row, 'license')},
            ${v(row, 'content_digest')}, ${v(row, 'reward_eligible') ?? 0},
            ${v(row, 'reward_amount_sats')},
            ${v(row, 'upload_source') ?? 'agent'}, ${v(row, 'demand_kind')},
            ${v(row, 'demand_source')}, ${v(row, 'created_at')},
            ${v(row, 'updated_at')}
          )
          ON CONFLICT (trace_uuid) DO UPDATE SET
            owner_user_id = EXCLUDED.owner_user_id,
            agent_ref = EXCLUDED.agent_ref,
            schema_version = EXCLUDED.schema_version,
            trajectory_id = EXCLUDED.trajectory_id,
            session_id = EXCLUDED.session_id,
            visibility = EXCLUDED.visibility,
            step_count = EXCLUDED.step_count,
            trajectory_json = EXCLUDED.trajectory_json,
            trajectory_r2_key = EXCLUDED.trajectory_r2_key,
            blob_refs_json = EXCLUDED.blob_refs_json,
            idempotency_key = EXCLUDED.idempotency_key,
            training_consent = EXCLUDED.training_consent,
            license = EXCLUDED.license,
            content_digest = EXCLUDED.content_digest,
            reward_eligible = EXCLUDED.reward_eligible,
            reward_amount_sats = EXCLUDED.reward_amount_sats,
            upload_source = EXCLUDED.upload_source,
            demand_kind = EXCLUDED.demand_kind,
            demand_source = EXCLUDED.demand_source,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at
          RETURNING 1 AS touched`
        return result.length
      }
      case 'agent_goals': {
        const result: Array<{ touched: number }> = await sql`
          INSERT INTO agent_goals (
            id, agent_id, user_id, team_id, project_id, objective, status,
            visibility, current_run_id, token_budget, tokens_used,
            time_used_seconds, created_at, updated_at, completed_at,
            paused_at, blocked_at, archived_at
          ) VALUES (
            ${v(row, 'id')}, ${v(row, 'agent_id')}, ${v(row, 'user_id')},
            ${v(row, 'team_id')}, ${v(row, 'project_id')},
            ${v(row, 'objective')}, ${v(row, 'status')},
            ${v(row, 'visibility')}, ${v(row, 'current_run_id')},
            ${v(row, 'token_budget')}, ${v(row, 'tokens_used') ?? 0},
            ${v(row, 'time_used_seconds') ?? 0}, ${v(row, 'created_at')},
            ${v(row, 'updated_at')}, ${v(row, 'completed_at')},
            ${v(row, 'paused_at')}, ${v(row, 'blocked_at')},
            ${v(row, 'archived_at')}
          )
          ON CONFLICT (id) DO UPDATE SET
            agent_id = EXCLUDED.agent_id,
            user_id = EXCLUDED.user_id,
            team_id = EXCLUDED.team_id,
            project_id = EXCLUDED.project_id,
            objective = EXCLUDED.objective,
            status = EXCLUDED.status,
            visibility = EXCLUDED.visibility,
            current_run_id = EXCLUDED.current_run_id,
            token_budget = EXCLUDED.token_budget,
            tokens_used = EXCLUDED.tokens_used,
            time_used_seconds = EXCLUDED.time_used_seconds,
            created_at = EXCLUDED.created_at,
            updated_at = EXCLUDED.updated_at,
            completed_at = EXCLUDED.completed_at,
            paused_at = EXCLUDED.paused_at,
            blocked_at = EXCLUDED.blocked_at,
            archived_at = EXCLUDED.archived_at
          RETURNING 1 AS touched`
        return result.length
      }
      case 'agent_goal_events': {
        // Bare DO NOTHING covers id PK + the partial
        // (goal_id, external_event_id) unique — the D1 INSERT OR IGNORE keys.
        const result: Array<{ inserted: number }> = await sql`
          INSERT INTO agent_goal_events (
            id, goal_id, run_id, expected_goal_id, caller_type, event_type,
            status, token_delta, time_delta_seconds, payload_json,
            external_event_id, created_at
          ) VALUES (
            ${v(row, 'id')}, ${v(row, 'goal_id')}, ${v(row, 'run_id')},
            ${v(row, 'expected_goal_id')}, ${v(row, 'caller_type')},
            ${v(row, 'event_type')}, ${v(row, 'status')},
            ${v(row, 'token_delta') ?? 0},
            ${v(row, 'time_delta_seconds') ?? 0}, ${v(row, 'payload_json')},
            ${v(row, 'external_event_id')}, ${v(row, 'created_at')}
          )
          ON CONFLICT DO NOTHING
          RETURNING 1 AS inserted`
        return result.length
      }
    }
  }

  return {
    deleteDefinitionTriggersNotIn: (
      ownerAgentUserId,
      definitionId,
      keepTriggerRefs,
    ) =>
      withSql(async sql => {
        if (keepTriggerRefs.length === 0) {
          await sql`
            DELETE FROM agent_definition_triggers
             WHERE owner_agent_user_id = ${ownerAgentUserId}
               AND definition_id = ${definitionId}`
          return
        }
        await sql`
          DELETE FROM agent_definition_triggers
           WHERE owner_agent_user_id = ${ownerAgentUserId}
             AND definition_id = ${definitionId}
             AND trigger_ref <> ALL(${[...keepTriggerRefs]})`
      }),

    listDueCronTriggerRows: (nowIso, limit) =>
      withSql(async sql => {
        const rows: Array<DueAgentDefinitionTriggerRow & {
          consecutive_failures: unknown
        }> = await sql`
          SELECT trigger_id, owner_agent_user_id, owner_ref, definition_id,
                 trigger_ref, trigger_json, state, consecutive_failures,
                 next_run_at, paused_at, pause_reason, created_at, updated_at
            FROM agent_definition_triggers
           WHERE trigger_kind = 'cron'
             AND state = 'enabled'
             AND next_run_at IS NOT NULL
             AND next_run_at <= ${nowIso}
           ORDER BY next_run_at ASC, trigger_id ASC
           LIMIT ${Math.max(1, Math.min(limit, 100))}`
        return rows.map(row => ({
          ...row,
          // bigint columns come back driver-typed; normalize to D1 numbers.
          consecutive_failures: Number(row.consecutive_failures ?? 0),
        }))
      }),

    listInboundWebhookTriggerRows: limit =>
      withSql(async sql => {
        const rows: Array<DueAgentDefinitionTriggerRow & {
          consecutive_failures: unknown
        }> = await sql`
          SELECT trigger_id, owner_agent_user_id, owner_ref, definition_id,
                 trigger_ref, trigger_json, state, consecutive_failures,
                 next_run_at, paused_at, pause_reason, created_at, updated_at
            FROM agent_definition_triggers
           WHERE trigger_kind = 'inbound_webhook'
             AND state = 'enabled'
           ORDER BY updated_at ASC, trigger_id ASC
           LIMIT ${Math.max(1, Math.min(limit, 500))}`
        return rows.map(row => ({
          ...row,
          consecutive_failures: Number(row.consecutive_failures ?? 0),
        }))
      }),

    upsertRows: async (table, rows) => {
      if (rows.length === 0) {
        return 0
      }
      const ordered = table === 'agent_goals' ? orderGoalRows(rows) : rows
      return withSql(async sql => {
        let touched = 0
        for (const row of ordered) {
          touched += await upsertOne(sql, table, row)
        }
        return touched
      })
    },
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

const D1_TABLE_COLUMNS: Readonly<Record<AgentRuntimeTable, ReadonlyArray<string>>> = {
  agent_definition_runs: [
    'run_id', 'owner_agent_user_id', 'definition_id', 'definition_ref',
    'trigger_ref', 'lane', 'status', 'pylon_ref', 'assignment_ref',
    'durable_request_id', 'durable_stream_url', 'forge_tenant_ref',
    'forge_work_ref', 'forge_repository_ref', 'forge_git_token_refs_json',
    'refusal_error', 'refusal_reason', 'evidence_refs_json',
    'trigger_payload_json', 'runtime_run_json', 'initial_events_json',
    'budget_credits_reserved', 'created_at', 'updated_at',
  ],
  agent_definition_triggers: [
    'trigger_id', 'owner_agent_user_id', 'owner_ref', 'definition_id',
    'trigger_ref', 'trigger_kind', 'trigger_json', 'state',
    'consecutive_failures', 'next_run_at', 'paused_at', 'pause_reason',
    'created_at', 'updated_at',
  ],
  agent_definitions: [
    'id', 'owner_agent_user_id', 'owner_ref', 'schema_literal', 'name',
    'slug', 'goal', 'harness_json', 'toolset_json', 'triggers_json', 'lane',
    'budget_json', 'escalation_json', 'source_refs_json', 'definition_json',
    'created_at', 'updated_at', 'archived_at',
  ],
  agent_goal_events: [
    'id', 'goal_id', 'run_id', 'expected_goal_id', 'caller_type',
    'event_type', 'status', 'token_delta', 'time_delta_seconds',
    'payload_json', 'external_event_id', 'created_at',
  ],
  agent_goals: [
    'id', 'agent_id', 'user_id', 'team_id', 'project_id', 'objective',
    'status', 'visibility', 'current_run_id', 'token_budget', 'tokens_used',
    'time_used_seconds', 'created_at', 'updated_at', 'completed_at',
    'paused_at', 'blocked_at', 'archived_at',
  ],
  agent_run_events: [
    'id', 'run_id', 'sequence', 'type', 'summary', 'status', 'source',
    'payload_json', 'artifact_refs_json', 'external_event_id', 'created_at',
  ],
  agent_runs: [
    'id', 'user_id', 'team_id', 'project_id', 'runtime', 'backend',
    'runner_id', 'assignment_kind', 'repository_provider',
    'repository_owner', 'repository_repo', 'repository_ref', 'goal',
    'goal_id', 'provider_account_ref', 'auth_grant_ref', 'external_run_id',
    'status', 'event_cursor', 'assignment_json', 'created_at', 'updated_at',
    'started_at', 'completed_at', 'failed_at', 'canceled_at', 'archived_at',
  ],
  agent_traces: [
    'trace_uuid', 'owner_user_id', 'agent_ref', 'schema_version',
    'trajectory_id', 'session_id', 'visibility', 'step_count',
    'trajectory_json', 'trajectory_r2_key', 'blob_refs_json',
    'idempotency_key', 'training_consent', 'license', 'content_digest',
    'reward_eligible', 'reward_amount_sats', 'upload_source', 'demand_kind',
    'demand_source', 'created_at', 'updated_at',
  ],
}

const D1_CONFLICT: Readonly<
  Record<
    AgentRuntimeTable,
    Readonly<{ keyColumns: ReadonlyArray<string>; mode: 'nothing' | 'converge' }>
  >
> = {
  agent_definition_runs: { keyColumns: ['run_id'], mode: 'converge' },
  agent_definition_triggers: {
    keyColumns: ['owner_agent_user_id', 'trigger_ref'],
    mode: 'converge',
  },
  agent_definitions: { keyColumns: ['id'], mode: 'converge' },
  agent_goal_events: { keyColumns: [], mode: 'nothing' },
  agent_goals: { keyColumns: ['id'], mode: 'converge' },
  agent_run_events: { keyColumns: [], mode: 'nothing' },
  agent_runs: { keyColumns: ['id'], mode: 'converge' },
  agent_traces: { keyColumns: ['trace_uuid'], mode: 'converge' },
}

/**
 * The D1 twin of the row-level seam (used by the contract suite and
 * available as the write path at eventual full cutover). Same converge /
 * insert-if-absent semantics over the same key sets.
 */
export const makeD1AgentRuntimeWriteStore = (
  db: D1Database,
): AgentRuntimeWriteStore => ({
  deleteDefinitionTriggersNotIn: async (
    ownerAgentUserId,
    definitionId,
    keepTriggerRefs,
  ) => {
    if (keepTriggerRefs.length === 0) {
      await db
        .prepare(
          `DELETE FROM agent_definition_triggers
            WHERE owner_agent_user_id = ? AND definition_id = ?`,
        )
        .bind(ownerAgentUserId, definitionId)
        .run()
      return
    }
    const placeholders = keepTriggerRefs.map(() => '?').join(', ')
    await db
      .prepare(
        `DELETE FROM agent_definition_triggers
          WHERE owner_agent_user_id = ? AND definition_id = ?
            AND trigger_ref NOT IN (${placeholders})`,
      )
      .bind(ownerAgentUserId, definitionId, ...keepTriggerRefs)
      .run()
  },
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const columns = D1_TABLE_COLUMNS[table]
    const conflict = D1_CONFLICT[table]
    const ordered = table === 'agent_goals' ? orderGoalRows(rows) : rows
    let touched = 0
    for (const row of ordered) {
      const values = columns.map(column => {
        const value = row[column]
        return value === undefined ? null : value
      })
      const placeholders = columns.map(() => '?').join(', ')
      if (conflict.mode === 'nothing') {
        const result = await db
          .prepare(
            `INSERT OR IGNORE INTO ${table} (${columns.join(', ')}) VALUES (${placeholders})`,
          )
          .bind(...values)
          .run()
        touched += (result.meta?.changes ?? 0) > 0 ? 1 : 0
      } else {
        const setClauses = columns
          .filter(column => !conflict.keyColumns.includes(column))
          .map(column => `${column} = excluded.${column}`)
          .join(', ')
        await db
          .prepare(
            `INSERT INTO ${table} (${columns.join(', ')}) VALUES (${placeholders})
             ON CONFLICT(${conflict.keyColumns.join(', ')}) DO UPDATE SET ${setClauses}`,
          )
          .bind(...values)
          .run()
        touched += 1
      }
    }
    return touched
  },
})

// ---------------------------------------------------------------------------
// Dual-write wrapper over the row seam
// ---------------------------------------------------------------------------

export type MakeDualWriteAgentRuntimeWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: AgentRuntimeWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: AgentRuntimeWriteStore | undefined
  flags: AgentRuntimeFlags
  log?: AgentRuntimeLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_agent_runtime_dual_write_failed` (the drift metric).
 */
export const makeDualWriteAgentRuntimeWriteStore = (
  deps: MakeDualWriteAgentRuntimeWriteStoreDependencies,
): AgentRuntimeWriteStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined || !flags.dualWrite) {
    return d1
  }

  return {
    deleteDefinitionTriggersNotIn: async (owner, definitionId, keepRefs) => {
      await d1.deleteDefinitionTriggersNotIn(owner, definitionId, keepRefs)
      try {
        await postgres.deleteDefinitionTriggersNotIn(
          owner,
          definitionId,
          keepRefs,
        )
      } catch (error) {
        log('khala_sync_agent_runtime_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: 'deleteDefinitionTriggersNotIn',
          refs: [definitionId],
        })
      }
    },
    upsertRows: async (table, rows) => {
      const outcome = await d1.upsertRows(table, rows)
      try {
        await postgres.upsertRows(table, rows)
      } catch (error) {
        log('khala_sync_agent_runtime_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: rows
            .slice(0, 10)
            .map(row => String(row[AGENT_RUNTIME_TABLE_PK[table]] ?? '')),
        })
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// The read-back mirror (production dual-write wiring)
// ---------------------------------------------------------------------------

export type AgentGoalScopeRef = Readonly<{
  agentId: string
  userId?: string | null | undefined
  teamId?: string | null | undefined
  projectId?: string | null | undefined
}>

export type AgentRuntimeMirror = Readonly<{
  /** Read the rows for `pkValues` back from D1 and upsert into Postgres. */
  mirrorRowsByPk: (
    table: AgentRuntimeTable,
    pkValues: ReadonlyArray<string>,
  ) => Promise<void>
  /** Mirror a run's event rows from `sinceSequence` (inclusive) upward. */
  mirrorAgentRunEventsSince: (
    runId: string,
    sinceSequence: number,
  ) => Promise<void>
  /** Mirror every goal row in one scope (archive+insert batches). */
  mirrorGoalScope: (scope: AgentGoalScopeRef) => Promise<void>
  /** Full trigger-set mirror for one definition (replace semantics). */
  mirrorTriggersForDefinition: (
    ownerAgentUserId: string,
    definitionId: string,
  ) => Promise<void>
  /** Mirror one trigger row by its live upsert key. */
  mirrorTriggerByRef: (
    ownerAgentUserId: string,
    triggerRef: string,
  ) => Promise<void>
}>

export type MakeAgentRuntimeMirrorDependencies = Readonly<{
  db: D1Database
  postgres: PostgresAgentRuntimeStore
  log: AgentRuntimeLog
}>

/**
 * Fail-soft read-back mirror: every method reads the authoritative rows
 * from D1 and converge-upserts them into Postgres; every failure is
 * logged (keys only) and swallowed. NEVER throws.
 */
export const makeAgentRuntimeMirror = (
  deps: MakeAgentRuntimeMirrorDependencies,
): AgentRuntimeMirror => {
  const { db, log, postgres } = deps

  const guarded = async (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> => {
    try {
      await run()
    } catch (error) {
      log('khala_sync_agent_runtime_dual_write_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: refs.slice(0, 10),
      })
    }
  }

  const mirrorRowsByPk = (
    table: AgentRuntimeTable,
    pkValues: ReadonlyArray<string>,
  ): Promise<void> =>
    guarded(`mirror:${table}`, pkValues, async () => {
      if (pkValues.length === 0) {
        return
      }
      const pk = AGENT_RUNTIME_TABLE_PK[table]
      const placeholders = pkValues.map(() => '?').join(', ')
      const rows = await db
        .prepare(
          `SELECT * FROM ${table} WHERE ${pk} IN (${placeholders})`,
        )
        .bind(...pkValues)
        .all<AgentRuntimeRow>()
      await postgres.upsertRows(table, rows.results ?? [])
    })

  return {
    mirrorAgentRunEventsSince: (runId, sinceSequence) =>
      guarded('mirror:agent_run_events', [runId], async () => {
        const rows = await db
          .prepare(
            `SELECT * FROM agent_run_events
              WHERE run_id = ? AND sequence >= ?
              ORDER BY sequence ASC`,
          )
          .bind(runId, sinceSequence)
          .all<AgentRuntimeRow>()
        await postgres.upsertRows('agent_run_events', rows.results ?? [])
      }),

    mirrorGoalScope: scope =>
      guarded('mirror:agent_goals:scope', [scope.agentId], async () => {
        const rows = await db
          .prepare(
            `SELECT * FROM agent_goals
              WHERE agent_id = ?
                AND COALESCE(user_id, '') = COALESCE(?, '')
                AND COALESCE(team_id, '') = COALESCE(?, '')
                AND COALESCE(project_id, '') = COALESCE(?, '')
              ORDER BY updated_at DESC
              LIMIT 20`,
          )
          .bind(
            scope.agentId,
            scope.userId ?? null,
            scope.teamId ?? null,
            scope.projectId ?? null,
          )
          .all<AgentRuntimeRow>()
        await postgres.upsertRows('agent_goals', rows.results ?? [])
      }),

    mirrorRowsByPk,

    mirrorTriggerByRef: (ownerAgentUserId, triggerRef) =>
      guarded('mirror:agent_definition_triggers', [triggerRef], async () => {
        const rows = await db
          .prepare(
            `SELECT * FROM agent_definition_triggers
              WHERE owner_agent_user_id = ? AND trigger_ref = ?`,
          )
          .bind(ownerAgentUserId, triggerRef)
          .all<AgentRuntimeRow>()
        await postgres.upsertRows(
          'agent_definition_triggers',
          rows.results ?? [],
        )
      }),

    mirrorTriggersForDefinition: (ownerAgentUserId, definitionId) =>
      guarded(
        'mirror:agent_definition_triggers:replace',
        [definitionId],
        async () => {
          const rows = await db
            .prepare(
              `SELECT * FROM agent_definition_triggers
                WHERE owner_agent_user_id = ? AND definition_id = ?`,
            )
            .bind(ownerAgentUserId, definitionId)
            .all<AgentRuntimeRow>()
          const current = rows.results ?? []
          await postgres.deleteDefinitionTriggersNotIn(
            ownerAgentUserId,
            definitionId,
            current.map(row => String(row['trigger_ref'] ?? '')),
          )
          await postgres.upsertRows('agent_definition_triggers', current)
        },
      ),
  }
}

// ---------------------------------------------------------------------------
// Env plumbing
// ---------------------------------------------------------------------------

export type AgentRuntimeStoreEnv = AgentRuntimeFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeAgentRuntimeStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: AgentRuntimeLog | undefined
  /** Bounded-retry backoff hook for routed reads (tests inject a no-op). */
  wait?: ((ms: number) => Promise<void>) | undefined
}>

const defaultLog: AgentRuntimeLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions,
): PostgresAgentRuntimeStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresAgentRuntimeStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

const mirrorForEnv = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions,
): AgentRuntimeMirror | undefined => {
  const flags = agentRuntimeFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return undefined
  }
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  return makeAgentRuntimeMirror({
    db: openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    log: options.log ?? defaultLog,
    postgres,
  })
}

// ---------------------------------------------------------------------------
// Domain store factories (the call-site drop-ins)
// ---------------------------------------------------------------------------

/** Drop-in for `makeD1AgentDefinitionStore(openAgentsDatabase(env))`. */
export const makeAgentDefinitionStoreForEnv = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions = {},
): AgentDefinitionStore => {
  const base = makeD1AgentDefinitionStore(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    createDefinition: async (ownerAgentUserId, definition) => {
      await base.createDefinition(ownerAgentUserId, definition)
      await mirror.mirrorRowsByPk('agent_definitions', [definition.id])
    },
    updateDefinition: async (ownerAgentUserId, definition) => {
      const changed = await base.updateDefinition(ownerAgentUserId, definition)
      if (changed) {
        await mirror.mirrorRowsByPk('agent_definitions', [definition.id])
      }
      return changed
    },
  }
}

/** Drop-in for `makeD1AgentDefinitionRunStore(openAgentsDatabase(env))`. */
export const makeAgentDefinitionRunStoreForEnv = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions = {},
): AgentDefinitionRunStore => {
  const base = makeD1AgentDefinitionRunStore(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    upsertRun: async record => {
      const stored = await base.upsertRun(record)
      await mirror.mirrorRowsByPk('agent_definition_runs', [record.runId])
      return stored
    },
  }
}

const READ_RETRY_DELAYS_MS: ReadonlyArray<number> = [50, 150]

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

/**
 * Drop-in for `makeD1AgentDefinitionTriggerStore(openAgentsDatabase(env))`:
 * mutations mirror fail-soft (replace = delete-not-in + full set;
 * state transitions read back by the live (owner, trigger_ref) key);
 * the due-trigger scans route per KHALA_SYNC_AGENT_RUNTIME_READS
 * (d1 | compare | postgres with bounded retry + D1 fallback) — the
 * AgentDefinitionScheduler.tick read this domain re-homes.
 */
export const makeAgentDefinitionTriggerStoreForEnv = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions = {},
): AgentDefinitionTriggerStore => {
  const base = makeD1AgentDefinitionTriggerStore(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
  )
  const flags = agentRuntimeFlagsFromEnv(env)
  const postgres = postgresStoreForEnv(env, options)
  const log = options.log ?? defaultLog
  const wait =
    options.wait ??
    ((ms: number) => new Promise(resolve => setTimeout(resolve, ms)))
  const mirror = mirrorForEnv(env, options)

  if (postgres === undefined || (mirror === undefined && flags.reads === 'd1')) {
    return base
  }

  const mirrored: AgentDefinitionTriggerStore =
    mirror === undefined
      ? base
      : {
          ...base,
          enableTrigger: async (owner, triggerRef, updatedAt) => {
            const changed = await base.enableTrigger(owner, triggerRef, updatedAt)
            if (changed) {
              await mirror.mirrorTriggerByRef(owner, triggerRef)
            }
            return changed
          },
          pauseTrigger: async (owner, triggerRef, pausedAt, reason) => {
            const changed = await base.pauseTrigger(
              owner,
              triggerRef,
              pausedAt,
              reason,
            )
            if (changed) {
              await mirror.mirrorTriggerByRef(owner, triggerRef)
            }
            return changed
          },
          recordTriggerDispatchFailure: async (
            owner,
            triggerRef,
            nextRunAt,
            updatedAt,
          ) => {
            const changed = await base.recordTriggerDispatchFailure(
              owner,
              triggerRef,
              nextRunAt,
              updatedAt,
            )
            if (changed) {
              await mirror.mirrorTriggerByRef(owner, triggerRef)
            }
            return changed
          },
          recordTriggerFailure: async (owner, triggerRef, updatedAt) => {
            const changed = await base.recordTriggerFailure(
              owner,
              triggerRef,
              updatedAt,
            )
            if (changed) {
              await mirror.mirrorTriggerByRef(owner, triggerRef)
            }
            return changed
          },
          recordTriggerSuccess: async (
            owner,
            triggerRef,
            nextRunAt,
            updatedAt,
          ) => {
            const changed = await base.recordTriggerSuccess(
              owner,
              triggerRef,
              nextRunAt,
              updatedAt,
            )
            if (changed) {
              await mirror.mirrorTriggerByRef(owner, triggerRef)
            }
            return changed
          },
          replaceDefinitionTriggers: async (owner, definition, nowIso) => {
            const records = await base.replaceDefinitionTriggers(
              owner,
              definition,
              nowIso,
            )
            await mirror.mirrorTriggersForDefinition(owner, definition.id)
            return records
          },
        }

  if (flags.reads === 'd1') {
    return mirrored
  }

  const route = async (
    op: string,
    d1Read: () => Promise<ReadonlyArray<DueAgentDefinitionTriggerRecord>>,
    postgresRead: () => Promise<ReadonlyArray<DueAgentDefinitionTriggerRecord>>,
  ): Promise<ReadonlyArray<DueAgentDefinitionTriggerRecord>> => {
    if (flags.reads === 'postgres') {
      for (let attempt = 0; ; attempt += 1) {
        try {
          return await postgresRead()
        } catch (error) {
          const delay = READ_RETRY_DELAYS_MS[attempt]
          if (delay === undefined) {
            log('khala_sync_agent_runtime_postgres_read_fallback', {
              messageSafe: safeMessage(error),
              op,
              refs: [],
            })
            return d1Read()
          }
          log('khala_sync_agent_runtime_postgres_read_failed', {
            messageSafe: safeMessage(error),
            op,
            refs: [],
          })
          await wait(delay)
        }
      }
    }

    // compare: read both, SERVE D1, log divergence (op name only).
    const d1Result = await d1Read()
    try {
      const postgresResult = await postgresRead()
      if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
        log('khala_sync_agent_runtime_read_compare_mismatch', {
          messageSafe: 'postgres read differs from d1 authority',
          op,
          refs: [],
        })
      }
    } catch (error) {
      log('khala_sync_agent_runtime_postgres_read_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: [],
      })
    }
    return d1Result
  }

  return {
    ...mirrored,
    listDueCronTriggers: (nowIso, limit) =>
      route(
        'listDueCronTriggers',
        () => mirrored.listDueCronTriggers(nowIso, limit),
        async () =>
          (await postgres.listDueCronTriggerRows(nowIso, limit)).map(
            dueTriggerRecordFromRow,
          ),
      ),
    listInboundWebhookTriggers: (source, limit) =>
      route(
        'listInboundWebhookTriggers',
        () => mirrored.listInboundWebhookTriggers(source, limit),
        async () =>
          (await postgres.listInboundWebhookTriggerRows(limit))
            .map(dueTriggerRecordFromRow)
            .filter(
              record =>
                record.trigger.kind === 'inbound_webhook' &&
                record.trigger.source === source,
            ),
      ),
  }
}

/**
 * KS-6.6 event-feed follow-up (#8416): fires the khala-sync
 * `scope.agent_run.<runId>` producer (BOTH the run/goal snapshot and the
 * new event-feed companion entities) on every `saveAgentRun`/
 * `appendAgentRunEvents` call, whenever a `KHALA_SYNC_DB` binding exists —
 * `undefined` when there is no binding so `makeOmniRunStoreForEnv` does zero
 * extra work in that case (same "with no binding everything degrades to
 * plain D1" discipline as the KS-8.5 raw-table mirror above).
 *
 * This is the fix for the "integration gap" the 2026-07-05 client-repoint
 * research recorded in RUNBOOK.md: the KS-6.6 producer previously only ran
 * at the three `omni-handlers.ts` run-CREATION call sites (via their own
 * explicit `projectAgentRunSyncScope` calls), never on the ONGOING
 * `appendAgentRunEvents` path that fires throughout a run's life. Baking it
 * in HERE — the one factory both `dependencies.makeBillingAwareOmniRunStore`
 * and every bare `makeOmniRunStoreForEnv(env)` call site route through —
 * makes the producer fire universally without each call site opting in.
 */
const khalaSyncAgentRunProjectionHook = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions,
):
  | ((
      run: AgentRunRecord,
      events: ReadonlyArray<OmniEventRecord>,
    ) => Promise<void>)
  | undefined => {
  const binding = env.KHALA_SYNC_DB
  if (binding === undefined || binding.connectionString.length === 0) {
    return undefined
  }
  return async (run, events) => {
    const deps: ProjectAgentRunDependencies = {
      binding,
      log: (event, fields) => logWorkerRouteWarning(event, fields),
      makeSqlClient: options.makeSqlClient,
    }
    // Both calls are individually fail-soft (never throw); run them
    // sequentially so an event-feed failure never skips the run/goal
    // snapshot refresh (the more load-bearing of the two).
    await projectAgentRun(deps, run.id, agentRunSyncProjectionRaw(run))
    if (events.length > 0) {
      await projectAgentRunEvents(
        deps,
        run.id,
        events.map(agentRunEventProjection),
      )
    }
  }
}

/** Drop-in for `makeD1OmniRunStore(openAgentsDatabase(env), hooks)`. */
export const makeOmniRunStoreForEnv = (
  env: AgentRuntimeStoreEnv,
  hooks: OmniRunStoreHooks = {},
  options: MakeAgentRuntimeStoreOptions = {},
): OmniRunStore => {
  const syncProjectionHook = khalaSyncAgentRunProjectionHook(env, options)
  const mergedHooks: OmniRunStoreHooks =
    syncProjectionHook === undefined
      ? hooks
      : {
          ...hooks,
          afterAgentRunSyncChanges: async (run, events) => {
            // Isolate the caller-supplied hook from ours: a throw in one
            // must never skip the other (both must run every time).
            try {
              await hooks.afterAgentRunSyncChanges?.(run, events)
            } catch {
              // The caller's own hook is a separate concern; `omni-runs.ts`'s
              // `callAfterAgentRunSyncChangesHook` is defense in depth for
              // this same rule, but do not rely on call order for it.
            }
            await syncProjectionHook(run, events)
          },
        }
  const base = makeD1OmniRunStore(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    mergedHooks,
  )
  const mirror = mirrorForEnv(env, options)
  // KS-8.17 (#8361): `autopilot_token_usage` (supervision long-tail registry)
  // has its one live writer inside `appendAgentRunEvents`'s batch (the
  // generated row id is never returned to this caller), so it mirrors by a
  // bounded `run_id` scan rather than by its own key — a different Postgres
  // twin/flag lane than the `agent_runs`/`agent_run_events` mirror above.
  const supervisionMirror = makeSupervisionLongtailMirrorForEnv(
    env as SupervisionLongtailStoreEnv,
  )
  if (mirror === undefined && supervisionMirror === undefined) {
    return base
  }
  return {
    ...base,
    appendAgentRunEvents: async (runId, events, status, externalRunId) => {
      await base.appendAgentRunEvents(runId, events, status, externalRunId)
      if (mirror !== undefined) {
        await mirror.mirrorRowsByPk('agent_runs', [runId])
        if (events.length > 0) {
          await mirror.mirrorAgentRunEventsSince(
            runId,
            Math.min(...events.map(event => event.sequence)),
          )
        }
      }
      await supervisionMirror?.mirrorRowsWhere(
        'autopilot_token_usage',
        ['run_id'],
        [runId],
      )
    },
    saveAgentRun: async (run, events) => {
      await base.saveAgentRun(run, events)
      if (mirror !== undefined) {
        await mirror.mirrorRowsByPk('agent_runs', [run.id])
        if (events.length > 0) {
          await mirror.mirrorAgentRunEventsSince(
            run.id,
            Math.min(...events.map(event => event.sequence)),
          )
        }
      }
    },
  }
}

/**
 * Drop-in for `cancelActiveAgentRunsForBillingExhaustion(db, ...)` — the
 * one agent_runs writer outside the omni store (billing sweep).
 */
export const cancelActiveAgentRunsForBillingExhaustionForEnv = async (
  env: AgentRuntimeStoreEnv,
  userId: string,
  input: Readonly<{ balanceCents: number; balanceFormatted: string }>,
  options: MakeAgentRuntimeStoreOptions = {},
): Promise<ReadonlyArray<BillingCanceledAgentRun>> => {
  const canceled = await cancelActiveAgentRunsForBillingExhaustion(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    userId,
    input,
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror !== undefined && canceled.length > 0) {
    await mirror.mirrorRowsByPk(
      'agent_runs',
      canceled.map(item => item.run.id),
    )
    for (const item of canceled) {
      await mirror.mirrorAgentRunEventsSince(item.run.id, item.event.sequence)
    }
  }
  return canceled
}

/** Drop-in for `makeD1TraceStore(openAgentsDatabase(env))`. */
export const makeTraceStoreForEnv = (
  env: AgentRuntimeStoreEnv,
  options: MakeAgentRuntimeStoreOptions = {},
): TraceStore => {
  const base = makeD1TraceStore(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    createTrace: async input => {
      const result = await base.createTrace(input)
      if (result.created) {
        // Read-back by trace_uuid keeps trajectory_json byte-exact for
        // hash reconciliation; the mirror logs the trace_uuid only.
        await mirror.mirrorRowsByPk('agent_traces', [input.traceUuid])
      }
      return result
    },
    updateTraceVisibility: async (traceUuid, ownerUserId, visibility, nowIso) => {
      const record = await base.updateTraceVisibility(
        traceUuid,
        ownerUserId,
        visibility,
        nowIso,
      )
      if (record !== undefined) {
        await mirror.mirrorRowsByPk('agent_traces', [traceUuid])
      }
      return record
    },
  }
}

/**
 * Drop-in for `makeD1AgentGoalRepository(db, runtime)`: every mutation
 * mirrors the affected goal row(s) fail-soft after the D1 write commits
 * (`attachRun` also mirrors the linked agent_run; `setGoal` mirrors the
 * whole scope — the archive+insert batch).
 */
export const makeAgentGoalRepositoryForEnv = (
  env: AgentRuntimeStoreEnv,
  runtime: AgentGoalRuntime = systemAgentGoalRuntime,
  options: MakeAgentRuntimeStoreOptions = {},
): AgentGoalRepositoryShape => {
  const base = makeD1AgentGoalRepository(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    runtime,
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }

  const mirrorGoal = (goalId: string) =>
    Effect.promise(() => mirror.mirrorRowsByPk('agent_goals', [goalId]))

  return {
    ...base,
    accountUsage: input =>
      base.accountUsage(input).pipe(Effect.tap(goal => mirrorGoal(goal.id))),
    archiveGoal: (goalId, expectedGoalId) =>
      base
        .archiveGoal(goalId, expectedGoalId)
        .pipe(Effect.tap(goal => mirrorGoal(goal.id))),
    attachRun: input =>
      base.attachRun(input).pipe(
        Effect.tap(goal =>
          mirrorGoal(goal.id).pipe(
            Effect.andThen(
              Effect.promise(() =>
                mirror.mirrorRowsByPk('agent_runs', [input.runId]),
              ),
            ),
          ),
        ),
      ),
    changeVisibility: (goalId, visibility, expectedGoalId) =>
      base
        .changeVisibility(goalId, visibility, expectedGoalId)
        .pipe(Effect.tap(goal => mirrorGoal(goal.id))),
    clearTokenBudget: (goalId, expectedGoalId) =>
      base
        .clearTokenBudget(goalId, expectedGoalId)
        .pipe(Effect.tap(goal => mirrorGoal(goal.id))),
    editObjective: (goalId, objective, expectedGoalId) =>
      base
        .editObjective(goalId, objective, expectedGoalId)
        .pipe(Effect.tap(goal => mirrorGoal(goal.id))),
    setGoal: input =>
      base.setGoal(input).pipe(
        Effect.tap(() =>
          Effect.promise(() =>
            mirror.mirrorGoalScope({
              agentId: input.agentId,
              projectId: input.projectId,
              teamId: input.teamId,
              userId: input.userId,
            }),
          ),
        ),
      ),
    setStatus: (goalId, status, expectedGoalId) =>
      base
        .setStatus(goalId, status, expectedGoalId)
        .pipe(Effect.tap(goal => mirrorGoal(goal.id))),
    setTokenBudget: (goalId, tokenBudget, expectedGoalId) =>
      base
        .setTokenBudget(goalId, tokenBudget, expectedGoalId)
        .pipe(Effect.tap(goal => mirrorGoal(goal.id))),
  }
}

/** Layer twin of `makeAgentGoalRepositoryForEnv`. */
export const makeAgentGoalRepositoryLayerForEnv = (
  env: AgentRuntimeStoreEnv,
  runtime?: AgentGoalRuntime,
  options: MakeAgentRuntimeStoreOptions = {},
) =>
  Layer.succeed(
    AgentGoalRepository,
    makeAgentGoalRepositoryForEnv(env, runtime, options),
  )

/** Drop-in for `makeD1AgentGoalEventRepository(db, runtime)`. */
export const makeAgentGoalEventRepositoryForEnv = (
  env: AgentRuntimeStoreEnv,
  runtime?: AgentGoalRuntime,
  options: MakeAgentRuntimeStoreOptions = {},
): AgentGoalEventRepositoryShape => {
  const base = makeD1AgentGoalEventRepository(
    openAgentsDatabase(env as { OPENAGENTS_DB: D1Database }),
    runtime,
  )
  const mirror = mirrorForEnv(env, options)
  if (mirror === undefined) {
    return base
  }

  const mirrorEvent = (eventId: string) =>
    Effect.promise(() => mirror.mirrorRowsByPk('agent_goal_events', [eventId]))

  return {
    ...base,
    record: input =>
      base.record(input).pipe(Effect.tap(record => mirrorEvent(record.id))),
    recordOnce: input =>
      base
        .recordOnce(input)
        .pipe(
          Effect.tap(record =>
            record === undefined ? Effect.void : mirrorEvent(record.id),
          ),
        ),
  }
}

/** Layer twin of `makeAgentGoalEventRepositoryForEnv`. */
export const makeAgentGoalEventRepositoryLayerForEnv = (
  env: AgentRuntimeStoreEnv,
  runtime?: AgentGoalRuntime,
  options: MakeAgentRuntimeStoreOptions = {},
) =>
  Layer.succeed(
    AgentGoalEventRepository,
    makeAgentGoalEventRepositoryForEnv(env, runtime, options),
  )
