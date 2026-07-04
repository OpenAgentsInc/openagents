// KS-8.2 (#8308): token ledger domain — D1 → Cloud SQL migration machinery.
// Second KS-8 domain lane; mirrors the KS-8.1 template
// (`pylon-dispatch-store.ts`, #8307).
//
// Three pieces:
//
//  1. `makePostgresTokenLedgerStore` — the Postgres implementation of the
//     ledger WRITE seam (`TokenLedgerWriteStore`: dedupe read + the
//     rollup-consistent insert as ONE transaction) plus the five public
//     read paths (aggregate / history / model mix / demand mix / channel
//     mix) over the structural `SyncSql` seam via the KHALA_SYNC_DB
//     Hyperdrive binding. Tables: `token_usage_events`,
//     `public_khala_tokens_served_*` rollups,
//     `token_usage_leaderboard_preferences` (khala-sync migration 0008).
//     D1's dedupe-SELECT-then-INSERT collapses to a bare
//     `ON CONFLICT DO NOTHING` on the SAME key set (id PK +
//     idempotency_key unique — MIGRATION_PLAN universal porting rule).
//
//  2. `makeDualWriteTokenLedgerWriteStore` + `makeReadRoutedTokenUsageLedger`
//     — the flag-routed production wiring. Writes go D1-first (authority),
//     then mirror the FRESH row to Postgres best-effort: a mirror failure
//     NEVER fails the ingest; it logs the typed drift diagnostic
//     `khala_sync_ledger_dual_write_failed`. IMPORTANT #8304 interplay:
//     the public tokens-served counter observer rides `ingestEvent` keyed
//     to the AUTHORITATIVE D1 insert result, outside this seam — the
//     Postgres mirror can never re-fire a counter increment (regression
//     test in token-ledger-store.test.ts). Reads route per flag:
//       d1        — D1 only (default)
//       compare   — read both, SERVE D1, log mismatches
//       postgres  — Postgres with bounded retry (50/150ms), D1 fallback
//     Only the five PUBLIC read paths route; the internal admin aggregates
//     (readAggregates / readInferenceAnalytics / readLeaderboards) stay on
//     D1 in this lane and move with the decommission follow-up.
//
//  3. `makeTokenUsageLedgerForEnv` / `makeTokenLedgerWriteStoreForEnv` —
//     the drop-in factories Worker call sites use instead of bare
//     `makeD1TokenUsageLedger`. Flags:
//       KHALA_SYNC_LEDGER_DUAL_WRITE  (default ON; 'off'|'0'|'false'|'disabled')
//       KHALA_SYNC_LEDGER_READS       (default 'd1'; 'd1'|'postgres'|'compare')
//     With no KHALA_SYNC_DB binding everything degrades to plain D1.
//
// ROLLUP FIDELITY: the Postgres rollups are maintained by the SAME rule as
// D1 — only the ledger `ingestEvent` path increments them (in the insert
// transaction); the low-volume direct-insert paths
// (`public-khala-chat-served-tokens.ts`, the khala-MCP recorder) mirror
// the EVENT row only via `mirrorTokenLedgerDirectInsertBestEffort`,
// matching D1's live behavior, so Postgres rollups reconcile against D1
// rollups byte-for-byte and the backfill copies D1 rollup rows verbatim.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Token ledger domain"):
// dual-write on → backfill (scripts/backfill-token-ledger.ts) → verify →
// compare reads → postgres reads → decommission D1 tables in a follow-up.

import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'
import {
  calendarDayKeyAfter,
  dayKeyInTimezone,
  startOfCalendarDayIsoTimestampInTimezone,
  startOfDayIsoTimestampInTimezone,
} from './runtime-primitives'
import {
  decodePublicTokensServedAggregate,
  decodePublicTokensServedChannelMix,
  decodePublicTokensServedDemandMix,
  decodePublicTokensServedHistory,
  decodePublicTokensServedModelMix,
  demandChannelFromText,
  leaderboardWindowSince,
  makeD1TokenLedgerWriteStore,
  makeD1TokenUsageLedger,
  normalizeHistoryBucket,
  normalizeHistoryTimezone,
  normalizeLeaderboardWindow,
  PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE,
  publicMixRollupWindow,
  publicTokensServedChannelMixFromRows,
  publicTokensServedDemandMixFromRows,
  publicTokensServedFromRow,
  publicTokensServedHistoryDayWindows,
  publicTokensServedModelMixFromRows,
  systemTokenUsageLedgerRuntime,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerValidationError,
  type PublicTokensServedChannelMixRow,
  type PublicTokensServedDemandMixRow,
  type PublicTokensServedModelMixRow,
  type TokenLedgerWriteStore,
  type TokenUsageEventRow,
  type TokenUsageLedgerIngestObserver,
  type TokenUsageLedgerRuntime,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type TokenLedgerReadsMode = 'd1' | 'postgres' | 'compare'

export type TokenLedgerFlags = Readonly<{
  dualWrite: boolean
  reads: TokenLedgerReadsMode
}>

export type TokenLedgerFlagEnv = Readonly<{
  KHALA_SYNC_LEDGER_DUAL_WRITE?: string | undefined
  KHALA_SYNC_LEDGER_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.2 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority until the runbook's cutover sequence flips
 * them. Unknown read values fall back to 'd1' — never fail open into an
 * unproven read path on a typo.
 */
export const tokenLedgerFlagsFromEnv = (
  env: TokenLedgerFlagEnv,
): TokenLedgerFlags => {
  const dualWriteRaw = env.KHALA_SYNC_LEDGER_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_LEDGER_READS?.trim().toLowerCase()

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

export type TokenLedgerDiagnosticEvent =
  | 'khala_sync_ledger_dual_write_failed'
  | 'khala_sync_ledger_read_compare_mismatch'
  | 'khala_sync_ledger_postgres_read_failed'
  | 'khala_sync_ledger_postgres_read_fallback'

export type TokenLedgerDiagnostic = Readonly<{
  /** The store operation, e.g. 'insertEventRow'. */
  op: string
  /** Public-safe refs identifying the affected rows (never payloads). */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no SQL). */
  messageSafe: string
}>

export type TokenLedgerLog = (
  event: TokenLedgerDiagnosticEvent,
  fields: TokenLedgerDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// Postgres token ledger store
// ---------------------------------------------------------------------------

export type PublicTokensServedHistoryPoint = Readonly<{
  day: string
  tokensServed: number
}>

/**
 * The KS-8.2 Postgres store: the write seam (`TokenLedgerWriteStore`)
 * plus the direct-path mirror, the preference mirror, and the five public
 * read paths the KHALA_SYNC_LEDGER_READS flag can route.
 */
export type PostgresTokenLedgerStore = TokenLedgerWriteStore &
  Readonly<{
    /**
     * Mirror one row from a D1 DIRECT-insert path (khala chat / MCP): the
     * event row only, NO rollup increments — exactly what D1 does on those
     * paths, so both stores' rollups stay comparable.
     */
    insertDirectEventRow: (
      row: TokenUsageEventRow,
    ) => Promise<'inserted' | 'duplicate'>
    mirrorLeaderboardPreference: (
      preference: Readonly<{
        leaderboardParticipation: string
        leaderboardVisibility: string
        subjectKind: string
        subjectRef: string
        updatedAt: string
        updatedByUserId: string | null
      }>,
    ) => Promise<void>
    readPublicTokensServedTotal: () => Promise<number>
    readPublicTokensServedHistorySeries: (
      input: Readonly<{
        nowIso: string
        since: string | undefined
        timezone: string
      }>,
    ) => Promise<ReadonlyArray<PublicTokensServedHistoryPoint>>
    readPublicTokensServedModelMixRows: (
      input: Readonly<{ nowIso: string; since: string | undefined }>,
    ) => Promise<ReadonlyArray<PublicTokensServedModelMixRow>>
    readPublicTokensServedDemandMixRows: (
      input: Readonly<{ since: string | undefined }>,
    ) => Promise<ReadonlyArray<PublicTokensServedDemandMixRow>>
    readPublicTokensServedChannelMixRows: (
      input: Readonly<{ nowIso: string; since: string | undefined }>,
    ) => Promise<ReadonlyArray<PublicTokensServedChannelMixRow>>
  }>

export type MakePostgresTokenLedgerStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the KS-8.1 store.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

const toCount = (value: unknown): number =>
  Math.max(0, Math.trunc(Number(value ?? 0)))

export const makePostgresTokenLedgerStore = (
  deps: MakePostgresTokenLedgerStoreDependencies,
): PostgresTokenLedgerStore => {
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

  const insertEventStatement = (
    sql: SyncSql,
    row: TokenUsageEventRow,
  ): Promise<Array<{ id: string }>> =>
    // Bare ON CONFLICT DO NOTHING covers BOTH unique surfaces (id primary
    // key + idempotency_key unique) — the same key set as D1's dedupe.
    sql`
      INSERT INTO token_usage_events (
        id, idempotency_key, observed_at, ingested_at, producer_system,
        source_route, role_ref, actor_user_id, actor_team_id, account_ref,
        anonymized_source_ref, run_ref, session_ref, task_ref,
        repository_ref, provider, model, backend_profile, input_tokens,
        output_tokens, reasoning_tokens, cache_read_tokens,
        cache_write_5m_tokens, cache_write_1h_tokens, total_tokens,
        usage_truth, cost_amount, currency, demand_channel, demand_kind,
        demand_source, demand_client, leaderboard_eligible, privacy_opt_out,
        safe_metadata_json
      ) VALUES (
        ${row.id}, ${row.idempotency_key}, ${row.observed_at},
        ${row.ingested_at}, ${row.producer_system}, ${row.source_route},
        ${row.role_ref}, ${row.actor_user_id}, ${row.actor_team_id},
        ${row.account_ref}, ${row.anonymized_source_ref}, ${row.run_ref},
        ${row.session_ref}, ${row.task_ref}, ${row.repository_ref},
        ${row.provider}, ${row.model}, ${row.backend_profile},
        ${row.input_tokens ?? 0}, ${row.output_tokens ?? 0},
        ${row.reasoning_tokens ?? 0}, ${row.cache_read_tokens ?? 0},
        ${row.cache_write_5m_tokens ?? 0}, ${row.cache_write_1h_tokens ?? 0},
        ${row.total_tokens ?? 0}, ${row.usage_truth}, ${row.cost_amount},
        ${row.currency}, ${row.demand_channel ?? 'khala_api'},
        ${row.demand_kind ?? 'unlabeled'}, ${row.demand_source},
        ${row.demand_client}, ${row.leaderboard_eligible ?? 1},
        ${row.privacy_opt_out ?? 0}, ${row.safe_metadata_json ?? '{}'}
      )
      ON CONFLICT DO NOTHING
      RETURNING id`

  /**
   * The rollup-consistent increments — the SAME math the D1 batch applies
   * (`publicTokensServedRollupStatements`): the America/Chicago daily
   * rollup plus the UTC-day model/channel mix rollups, added only for a
   * FRESH event row, inside the same transaction as the insert.
   */
  const applyRollups = async (
    sql: SyncSql,
    row: TokenUsageEventRow,
  ): Promise<void> => {
    const tokensServed = publicTokensServedFromRow(row)
    const dailyDay = dayKeyInTimezone(
      row.observed_at,
      PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE,
    )
    if (dailyDay !== undefined) {
      await sql`
        INSERT INTO public_khala_tokens_served_daily_rollups
          (timezone, day, tokens_served, usage_events, updated_at)
        VALUES
          (${PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE}, ${dailyDay},
           ${tokensServed}, 1, ${row.ingested_at})
        ON CONFLICT (timezone, day) DO UPDATE SET
          tokens_served = public_khala_tokens_served_daily_rollups.tokens_served
            + EXCLUDED.tokens_served,
          usage_events = public_khala_tokens_served_daily_rollups.usage_events
            + EXCLUDED.usage_events,
          updated_at = EXCLUDED.updated_at`
    }

    const utcDay = dayKeyInTimezone(row.observed_at, 'UTC')
    if (utcDay !== undefined) {
      await sql`
        INSERT INTO public_khala_tokens_served_model_daily_rollups
          (day, provider, model, tokens_served, usage_events, updated_at)
        VALUES
          (${utcDay}, ${row.provider ?? ''}, ${row.model ?? ''},
           ${tokensServed}, 1, ${row.ingested_at})
        ON CONFLICT (day, provider, model) DO UPDATE SET
          tokens_served = public_khala_tokens_served_model_daily_rollups.tokens_served
            + EXCLUDED.tokens_served,
          usage_events = public_khala_tokens_served_model_daily_rollups.usage_events
            + EXCLUDED.usage_events,
          updated_at = EXCLUDED.updated_at`
      await sql`
        INSERT INTO public_khala_tokens_served_channel_daily_rollups
          (day, demand_channel, tokens_served, usage_events, updated_at)
        VALUES
          (${utcDay}, ${demandChannelFromText(row.demand_channel)},
           ${tokensServed}, 1, ${row.ingested_at})
        ON CONFLICT (day, demand_channel) DO UPDATE SET
          tokens_served = public_khala_tokens_served_channel_daily_rollups.tokens_served
            + EXCLUDED.tokens_served,
          usage_events = public_khala_tokens_served_channel_daily_rollups.usage_events
            + EXCLUDED.usage_events,
          updated_at = EXCLUDED.updated_at`
    }
  }

  const readPartialDay = async (
    sql: SyncSql,
    input: Readonly<{ day: string; endIso: string; startIso: string }>,
  ): Promise<ReadonlyArray<PublicTokensServedHistoryPoint>> => {
    const rows: Array<{
      tokens: unknown
      usage_events: unknown
    }> = await sql`
      SELECT
          COALESCE(SUM(CASE
            WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
              THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
            ELSE COALESCE(total_tokens, 0)
          END), 0) AS tokens,
          COUNT(*) AS usage_events
        FROM token_usage_events
       WHERE observed_at >= ${input.startIso}
         AND observed_at < ${input.endIso}`
    const row = rows[0]
    return row === undefined || toCount(row.usage_events) === 0
      ? []
      : [{ day: input.day, tokensServed: toCount(row.tokens) }]
  }

  return {
    findExistingRow: async input =>
      withSql(async sql => {
        const rows: Array<TokenUsageEventRow> = await sql`
          SELECT *
            FROM token_usage_events
           WHERE idempotency_key = ${input.idempotencyKey}
              OR id = ${input.eventId}
           ORDER BY ingested_at ASC
           LIMIT 1`
        const row = rows[0]
        if (row === undefined) {
          return undefined
        }
        // bigint/real columns come back driver-typed (postgres.js returns
        // bigint as string); normalize to the D1 row shape.
        return {
          ...row,
          cache_read_tokens: toCount(row.cache_read_tokens),
          cache_write_1h_tokens: toCount(row.cache_write_1h_tokens),
          cache_write_5m_tokens: toCount(row.cache_write_5m_tokens),
          cost_amount:
            row.cost_amount === null ? null : Number(row.cost_amount),
          input_tokens: toCount(row.input_tokens),
          leaderboard_eligible: toCount(row.leaderboard_eligible),
          output_tokens: toCount(row.output_tokens),
          privacy_opt_out: toCount(row.privacy_opt_out),
          reasoning_tokens: toCount(row.reasoning_tokens),
          total_tokens: toCount(row.total_tokens),
        }
      }),

    insertEventRow: row =>
      withSql(sql =>
        sql.begin(async tx => {
          const inserted = await insertEventStatement(tx as SyncSql, row)
          if (inserted.length === 0) {
            return 'duplicate' as const
          }
          await applyRollups(tx as SyncSql, row)
          return 'inserted' as const
        }),
      ),

    insertDirectEventRow: row =>
      withSql(async sql => {
        const inserted = await insertEventStatement(sql, row)
        return inserted.length === 0
          ? ('duplicate' as const)
          : ('inserted' as const)
      }),

    mirrorLeaderboardPreference: preference =>
      withSql(async sql => {
        await sql`
          INSERT INTO token_usage_leaderboard_preferences (
            subject_kind, subject_ref, leaderboard_participation,
            leaderboard_visibility, updated_at, updated_by_user_id
          ) VALUES (
            ${preference.subjectKind}, ${preference.subjectRef},
            ${preference.leaderboardParticipation},
            ${preference.leaderboardVisibility}, ${preference.updatedAt},
            ${preference.updatedByUserId}
          )
          ON CONFLICT (subject_kind, subject_ref) DO UPDATE SET
            leaderboard_participation = EXCLUDED.leaderboard_participation,
            leaderboard_visibility = EXCLUDED.leaderboard_visibility,
            updated_at = EXCLUDED.updated_at,
            updated_by_user_id = EXCLUDED.updated_by_user_id`
      }),

    readPublicTokensServedTotal: () =>
      withSql(async sql => {
        const rows: Array<{ tokens_served: unknown }> = await sql`
          SELECT
              COALESCE(SUM(CASE
                WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                  THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                ELSE COALESCE(total_tokens, 0)
              END), 0) AS tokens_served
            FROM token_usage_events`
        return toCount(rows[0]?.tokens_served)
      }),

    readPublicTokensServedHistorySeries: input =>
      withSql(async sql => {
        if (input.timezone === 'UTC') {
          // ISO-8601 text timestamps: the first 10 chars ARE the UTC day
          // key (the D1 side uses date(observed_at) over the same text).
          const rows: Array<{ day: string | null; tokens: unknown }> =
            input.since === undefined
              ? await sql`
                  SELECT substr(observed_at, 1, 10) AS day,
                         COALESCE(SUM(CASE
                           WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                             THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                           ELSE COALESCE(total_tokens, 0)
                         END), 0) AS tokens
                    FROM token_usage_events
                   GROUP BY substr(observed_at, 1, 10)
                   ORDER BY day ASC`
              : await sql`
                  SELECT substr(observed_at, 1, 10) AS day,
                         COALESCE(SUM(CASE
                           WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                             THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                           ELSE COALESCE(total_tokens, 0)
                         END), 0) AS tokens
                    FROM token_usage_events
                   WHERE observed_at >= ${input.since}
                   GROUP BY substr(observed_at, 1, 10)
                   ORDER BY day ASC`
          return rows
            .filter(
              (row): row is { day: string; tokens: unknown } =>
                typeof row.day === 'string' && row.day !== '',
            )
            .map(row => ({ day: row.day, tokensServed: toCount(row.tokens) }))
        }

        // Non-UTC (the default America/Chicago path): rollup-backed middle
        // days + raw partial first day + raw live last day — the same plan
        // as D1's readPublicTokensServedHistoryFromDailyRollups. A failure
        // here propagates; the read router falls back to the
        // still-authoritative D1 ledger.
        let startAtIso = input.since
        if (startAtIso === undefined) {
          const rows: Array<{ first_observed_at: string | null }> = await sql`
            SELECT MIN(observed_at) AS first_observed_at
              FROM token_usage_events`
          startAtIso = rows[0]?.first_observed_at ?? undefined
        }
        if (startAtIso === undefined) {
          return []
        }

        const windows = publicTokensServedHistoryDayWindows({
          nowIso: input.nowIso,
          since: input.since,
          startAtIso,
          timezone: input.timezone,
        })
        const firstWindow = windows[0]
        const lastWindow = windows[windows.length - 1]
        if (firstWindow === undefined || lastWindow === undefined) {
          return []
        }

        const firstDayStartIso = startOfCalendarDayIsoTimestampInTimezone(
          firstWindow.day,
          input.timezone,
        )
        const firstDayIsPartial =
          input.since !== undefined &&
          firstDayStartIso !== undefined &&
          input.since > firstDayStartIso
        const rollupStartDay = firstDayIsPartial
          ? calendarDayKeyAfter(firstWindow.day, 1)
          : firstWindow.day
        const rollupEndDay = calendarDayKeyAfter(lastWindow.day, -1)

        const partialFirstDay = firstDayIsPartial
          ? await readPartialDay(sql, firstWindow)
          : []
        const liveLastDay =
          firstWindow.day === lastWindow.day && firstDayIsPartial
            ? []
            : await readPartialDay(sql, lastWindow)
        let rollupDays: ReadonlyArray<PublicTokensServedHistoryPoint> = []
        if (
          rollupStartDay !== undefined &&
          rollupEndDay !== undefined &&
          rollupStartDay <= rollupEndDay
        ) {
          const rows: Array<{ day: string | null; tokens_served: unknown }> =
            await sql`
              SELECT day, tokens_served
                FROM public_khala_tokens_served_daily_rollups
               WHERE timezone = ${input.timezone}
                 AND day >= ${rollupStartDay}
                 AND day <= ${rollupEndDay}
               ORDER BY day ASC`
          rollupDays = rows
            .filter(
              (row): row is { day: string; tokens_served: unknown } =>
                typeof row.day === 'string' && row.day !== '',
            )
            .map(row => ({
              day: row.day,
              tokensServed: toCount(row.tokens_served),
            }))
        }

        return [...partialFirstDay, ...rollupDays, ...liveLastDay].sort(
          (left, right) => left.day.localeCompare(right.day),
        )
      }),

    readPublicTokensServedModelMixRows: input =>
      withSql(async sql => {
        const window = publicMixRollupWindow(input)
        const rawRows = async (
          since: string,
          before: string,
        ): Promise<Array<PublicTokensServedModelMixRow>> => {
          const rows: Array<{
            model: string | null
            provider: string | null
            tokens: unknown
            usage_events: unknown
          }> = await sql`
            SELECT provider, model,
                   COALESCE(SUM(CASE
                     WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                       THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                     ELSE COALESCE(total_tokens, 0)
                   END), 0) AS tokens,
                   COUNT(*) AS usage_events
              FROM token_usage_events
             WHERE observed_at >= ${since}
               AND observed_at < ${before}
             GROUP BY provider, model`
          return rows.map(row => ({
            model: row.model,
            provider: row.provider,
            tokens: toCount(row.tokens),
            usage_events: toCount(row.usage_events),
          }))
        }

        const rollupRows = async (
          range: Readonly<{ endDay: string; startDay: string }> | undefined,
        ): Promise<Array<PublicTokensServedModelMixRow>> => {
          const rows: Array<{
            model: string | null
            provider: string | null
            tokens: unknown
            usage_events: unknown
          }> =
            range === undefined
              ? await sql`
                  SELECT NULLIF(provider, '') AS provider,
                         NULLIF(model, '') AS model,
                         COALESCE(SUM(tokens_served), 0) AS tokens,
                         COALESCE(SUM(usage_events), 0) AS usage_events
                    FROM public_khala_tokens_served_model_daily_rollups
                   GROUP BY provider, model`
              : await sql`
                  SELECT NULLIF(provider, '') AS provider,
                         NULLIF(model, '') AS model,
                         COALESCE(SUM(tokens_served), 0) AS tokens,
                         COALESCE(SUM(usage_events), 0) AS usage_events
                    FROM public_khala_tokens_served_model_daily_rollups
                   WHERE day >= ${range.startDay}
                     AND day <= ${range.endDay}
                   GROUP BY provider, model`
          return rows.map(row => ({
            model: row.model,
            provider: row.provider,
            tokens: toCount(row.tokens),
            usage_events: toCount(row.usage_events),
          }))
        }

        if (window.mode === 'all') {
          return rollupRows(undefined)
        }
        const partial =
          window.firstDayIsPartial === true
            ? await rawRows(window.since, window.firstDayEndIso)
            : []
        const rollups =
          window.rollupStartDay > window.lastDay
            ? []
            : await rollupRows({
                endDay: window.lastDay,
                startDay: window.rollupStartDay,
              })
        return [...partial, ...rollups]
      }),

    readPublicTokensServedDemandMixRows: input =>
      withSql(async sql => {
        const rows: Array<{
          demand_client: string | null
          demand_kind: string | null
          demand_source: string | null
          tokens: unknown
          usage_events: unknown
        }> =
          input.since === undefined
            ? await sql`
                SELECT COALESCE(demand_kind, 'unlabeled') AS demand_kind,
                       COALESCE(NULLIF(demand_source, ''), 'unknown') AS demand_source,
                       COALESCE(NULLIF(demand_client, ''), 'unknown') AS demand_client,
                       COALESCE(SUM(CASE
                         WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                           THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                         ELSE COALESCE(total_tokens, 0)
                       END), 0) AS tokens,
                       COUNT(*) AS usage_events
                  FROM token_usage_events
                 GROUP BY COALESCE(demand_kind, 'unlabeled'),
                          COALESCE(NULLIF(demand_source, ''), 'unknown'),
                          COALESCE(NULLIF(demand_client, ''), 'unknown')`
            : await sql`
                SELECT COALESCE(demand_kind, 'unlabeled') AS demand_kind,
                       COALESCE(NULLIF(demand_source, ''), 'unknown') AS demand_source,
                       COALESCE(NULLIF(demand_client, ''), 'unknown') AS demand_client,
                       COALESCE(SUM(CASE
                         WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                           THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                         ELSE COALESCE(total_tokens, 0)
                       END), 0) AS tokens,
                       COUNT(*) AS usage_events
                  FROM token_usage_events
                 WHERE observed_at >= ${input.since}
                 GROUP BY COALESCE(demand_kind, 'unlabeled'),
                          COALESCE(NULLIF(demand_source, ''), 'unknown'),
                          COALESCE(NULLIF(demand_client, ''), 'unknown')`
        return rows.map(row => ({
          demand_client: row.demand_client,
          demand_kind: row.demand_kind,
          demand_source: row.demand_source,
          tokens: toCount(row.tokens),
          usage_events: toCount(row.usage_events),
        }))
      }),

    readPublicTokensServedChannelMixRows: input =>
      withSql(async sql => {
        const window = publicMixRollupWindow(input)
        const rawRows = async (
          since: string,
          before: string,
        ): Promise<Array<PublicTokensServedChannelMixRow>> => {
          const rows: Array<{
            demand_channel: string | null
            tokens: unknown
            usage_events: unknown
          }> = await sql`
            SELECT COALESCE(NULLIF(demand_channel, ''), 'khala_api') AS demand_channel,
                   COALESCE(SUM(CASE
                     WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
                       THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
                     ELSE COALESCE(total_tokens, 0)
                   END), 0) AS tokens,
                   COUNT(*) AS usage_events
              FROM token_usage_events
             WHERE observed_at >= ${since}
               AND observed_at < ${before}
             GROUP BY COALESCE(NULLIF(demand_channel, ''), 'khala_api')`
          return rows.map(row => ({
            demand_channel: row.demand_channel,
            tokens: toCount(row.tokens),
            usage_events: toCount(row.usage_events),
          }))
        }

        const rollupRows = async (
          range: Readonly<{ endDay: string; startDay: string }> | undefined,
        ): Promise<Array<PublicTokensServedChannelMixRow>> => {
          const rows: Array<{
            demand_channel: string | null
            tokens: unknown
            usage_events: unknown
          }> =
            range === undefined
              ? await sql`
                  SELECT demand_channel,
                         COALESCE(SUM(tokens_served), 0) AS tokens,
                         COALESCE(SUM(usage_events), 0) AS usage_events
                    FROM public_khala_tokens_served_channel_daily_rollups
                   GROUP BY demand_channel`
              : await sql`
                  SELECT demand_channel,
                         COALESCE(SUM(tokens_served), 0) AS tokens,
                         COALESCE(SUM(usage_events), 0) AS usage_events
                    FROM public_khala_tokens_served_channel_daily_rollups
                   WHERE day >= ${range.startDay}
                     AND day <= ${range.endDay}
                   GROUP BY demand_channel`
          return rows.map(row => ({
            demand_channel: row.demand_channel,
            tokens: toCount(row.tokens),
            usage_events: toCount(row.usage_events),
          }))
        }

        if (window.mode === 'all') {
          return rollupRows(undefined)
        }
        const partial =
          window.firstDayIsPartial === true
            ? await rawRows(window.since, window.firstDayEndIso)
            : []
        const rollups =
          window.rollupStartDay > window.lastDay
            ? []
            : await rollupRows({
                endDay: window.lastDay,
                startDay: window.rollupStartDay,
              })
        return [...partial, ...rollups]
      }),
  }
}

// ---------------------------------------------------------------------------
// Dual-write write store (D1 authority, fail-soft Postgres mirror)
// ---------------------------------------------------------------------------

export type MakeDualWriteTokenLedgerWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store (extracted, behavior-identical). */
  d1: TokenLedgerWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: Pick<PostgresTokenLedgerStore, 'insertEventRow'> | undefined
  flags: TokenLedgerFlags
  log?: TokenLedgerLog | undefined
}>

/**
 * The production `TokenLedgerWriteStore`: D1 writes first (authority);
 * a FRESH insert then mirrors to Postgres best-effort (rollups included,
 * one transaction). A mirror failure never fails the ingest — it emits
 * `khala_sync_ledger_dual_write_failed` (the drift metric). Duplicates do
 * NOT mirror (the row already exists on the authority; the backfill owns
 * historical convergence). Dedupe reads stay on D1 — the write-path
 * decision must never depend on mirror state.
 */
export const makeDualWriteTokenLedgerWriteStore = (
  deps: MakeDualWriteTokenLedgerWriteStoreDependencies,
): TokenLedgerWriteStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined || !flags.dualWrite) {
    return d1
  }

  return {
    findExistingRow: d1.findExistingRow,
    insertEventRow: async row => {
      const outcome = await d1.insertEventRow(row)
      if (outcome === 'inserted') {
        try {
          await postgres.insertEventRow(row)
        } catch (error) {
          log('khala_sync_ledger_dual_write_failed', {
            messageSafe: safeMessage(error),
            op: 'insertEventRow',
            refs: [row.id],
          })
        }
      }
      return outcome
    },
  }
}

// ---------------------------------------------------------------------------
// Postgres public reads (the flag-routable slice of TokenUsageLedgerShape)
// ---------------------------------------------------------------------------

export type PublicTokensServedReads = Pick<
  TokenUsageLedgerShape,
  | 'readPublicTokensServed'
  | 'readPublicTokensServedHistory'
  | 'readPublicTokensServedModelMix'
  | 'readPublicTokensServedDemandMix'
  | 'readPublicTokensServedChannelMix'
>

const pgEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TokenUsageLedgerStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new TokenUsageLedgerStorageError({ operation, error }),
  })

const requireNowTimestamp = (
  value: string,
): Effect.Effect<string, TokenUsageLedgerValidationError> =>
  Number.isFinite(Date.parse(value))
    ? Effect.succeed(value)
    : Effect.fail(
        new TokenUsageLedgerValidationError({
          field: 'now',
          message: 'now must be an ISO-compatible timestamp.',
        }),
      )

/**
 * The Postgres implementations of the five public read paths. Same
 * normalization, SAME shared shaping helpers, and the same response
 * decoders as the D1 ledger — only the row source differs (Postgres
 * events + rollup twins), so compare-mode diffs are honest.
 */
export const makePostgresPublicTokensReads = (
  store: PostgresTokenLedgerStore,
  runtime: TokenUsageLedgerRuntime = systemTokenUsageLedgerRuntime,
): PublicTokensServedReads => ({
  readPublicTokensServed: () =>
    Effect.gen(function* () {
      const tokensServed = yield* pgEffect(
        'tokenUsageEvents.publicTokensServed.postgres',
        () => store.readPublicTokensServedTotal(),
      )
      return yield* decodePublicTokensServedAggregate({ tokensServed })
    }),

  readPublicTokensServedHistory: (filters = {}) =>
    Effect.gen(function* () {
      const window = yield* normalizeLeaderboardWindow(filters.window ?? '30d')
      const bucket = yield* normalizeHistoryBucket(filters.bucket)
      const timezone = yield* normalizeHistoryTimezone(filters.timezone)
      const nowIso = yield* requireNowTimestamp(
        filters.now ?? runtime.nowIso(),
      )
      const since =
        window === 'today'
          ? startOfDayIsoTimestampInTimezone(nowIso, timezone)
          : leaderboardWindowSince(window, nowIso, runtime)
      const series = yield* pgEffect(
        'tokenUsageEvents.publicTokensServedHistory.postgres',
        () =>
          store.readPublicTokensServedHistorySeries({
            nowIso,
            since,
            timezone,
          }),
      )
      return yield* decodePublicTokensServedHistory({
        bucket,
        series,
        timezone,
        window,
      })
    }),

  readPublicTokensServedModelMix: (filters = {}) =>
    Effect.gen(function* () {
      const window = yield* normalizeLeaderboardWindow(filters.window ?? '30d')
      const nowIso = yield* requireNowTimestamp(
        filters.now ?? runtime.nowIso(),
      )
      const since = leaderboardWindowSince(window, nowIso, runtime)
      const rows = yield* pgEffect(
        'tokenUsageEvents.publicTokensServedModelMix.postgres',
        () => store.readPublicTokensServedModelMixRows({ nowIso, since }),
      )
      return yield* decodePublicTokensServedModelMix(
        publicTokensServedModelMixFromRows(rows, window),
      )
    }),

  readPublicTokensServedDemandMix: (filters = {}) =>
    Effect.gen(function* () {
      const window = yield* normalizeLeaderboardWindow(filters.window ?? '30d')
      const nowIso = yield* requireNowTimestamp(
        filters.now ?? runtime.nowIso(),
      )
      const since = leaderboardWindowSince(window, nowIso, runtime)
      const rows = yield* pgEffect(
        'tokenUsageEvents.publicTokensServedDemandMix.postgres',
        () => store.readPublicTokensServedDemandMixRows({ since }),
      )
      return yield* decodePublicTokensServedDemandMix(
        publicTokensServedDemandMixFromRows(rows, window),
      )
    }),

  readPublicTokensServedChannelMix: (filters = {}) =>
    Effect.gen(function* () {
      const window = yield* normalizeLeaderboardWindow(filters.window ?? '30d')
      const nowIso = yield* requireNowTimestamp(
        filters.now ?? runtime.nowIso(),
      )
      const since = leaderboardWindowSince(window, nowIso, runtime)
      const rows = yield* pgEffect(
        'tokenUsageEvents.publicTokensServedChannelMix.postgres',
        () => store.readPublicTokensServedChannelMixRows({ nowIso, since }),
      )
      return yield* decodePublicTokensServedChannelMix(
        publicTokensServedChannelMixFromRows(rows, window),
      )
    }),
})

// ---------------------------------------------------------------------------
// Read routing (d1 | compare | postgres with bounded retry + fallback)
// ---------------------------------------------------------------------------

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

export type MakeReadRoutedTokenUsageLedgerDependencies = Readonly<{
  /** The full D1-authoritative ledger (writes already dual-write-wrapped). */
  d1: TokenUsageLedgerShape
  /** The Postgres public reads, or undefined when no binding. */
  postgres: PublicTokensServedReads | undefined
  flags: TokenLedgerFlags
  log?: TokenLedgerLog | undefined
  /** Bounded-retry backoff hook (tests inject a no-op). */
  wait?: ((ms: number) => Promise<void>) | undefined
}>

/**
 * Route the five public read paths per KHALA_SYNC_LEDGER_READS; everything
 * else (ingest, admin aggregates, leaderboards, preferences) passes
 * through to the D1 ledger unchanged.
 */
export const makeReadRoutedTokenUsageLedger = (
  deps: MakeReadRoutedTokenUsageLedgerDependencies,
): TokenUsageLedgerShape => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})
  const wait =
    deps.wait ??
    ((ms: number) => new Promise(resolve => setTimeout(resolve, ms)))

  if (postgres === undefined || flags.reads === 'd1') {
    return d1
  }

  const route = <A, E>(
    op: string,
    d1Read: Effect.Effect<A, E>,
    postgresRead: Effect.Effect<A, E>,
  ): Effect.Effect<A, E> => {
    if (flags.reads === 'postgres') {
      const attempt = (
        attemptIndex: number,
      ): Effect.Effect<A, E> =>
        postgresRead.pipe(
          Effect.catch(error => {
            const delay = READ_RETRY_DELAYS_MS[attemptIndex]
            if (delay === undefined) {
              log('khala_sync_ledger_postgres_read_fallback', {
                messageSafe: safeMessage(error),
                op,
                refs: [],
              })
              return d1Read
            }
            log('khala_sync_ledger_postgres_read_failed', {
              messageSafe: safeMessage(error),
              op,
              refs: [],
            })
            return Effect.promise(() => wait(delay)).pipe(
              Effect.flatMap(() => attempt(attemptIndex + 1)),
            )
          }),
        )
      return attempt(0)
    }

    // compare: read both, SERVE D1, log divergence with the op name.
    return Effect.gen(function* () {
      const d1Result = yield* d1Read
      yield* postgresRead.pipe(
        Effect.map(postgresResult => {
          if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
            log('khala_sync_ledger_read_compare_mismatch', {
              messageSafe: 'postgres read differs from d1 authority',
              op,
              refs: [],
            })
          }
        }),
        Effect.catch(error => {
          log('khala_sync_ledger_postgres_read_failed', {
            messageSafe: safeMessage(error),
            op,
            refs: [],
          })
          return Effect.void
        }),
      )
      return d1Result
    })
  }

  return {
    ...d1,
    readPublicTokensServed: () =>
      route(
        'readPublicTokensServed',
        d1.readPublicTokensServed(),
        postgres.readPublicTokensServed(),
      ),
    readPublicTokensServedHistory: filters =>
      route(
        'readPublicTokensServedHistory',
        d1.readPublicTokensServedHistory(filters),
        postgres.readPublicTokensServedHistory(filters),
      ),
    readPublicTokensServedModelMix: filters =>
      route(
        'readPublicTokensServedModelMix',
        d1.readPublicTokensServedModelMix(filters),
        postgres.readPublicTokensServedModelMix(filters),
      ),
    readPublicTokensServedDemandMix: filters =>
      route(
        'readPublicTokensServedDemandMix',
        d1.readPublicTokensServedDemandMix(filters),
        postgres.readPublicTokensServedDemandMix(filters),
      ),
    readPublicTokensServedChannelMix: filters =>
      route(
        'readPublicTokensServedChannelMix',
        d1.readPublicTokensServedChannelMix(filters),
        postgres.readPublicTokensServedChannelMix(filters),
      ),
  }
}

// ---------------------------------------------------------------------------
// Env factories (the index.ts / route-module drop-ins)
// ---------------------------------------------------------------------------

export type TokenLedgerStoreEnv = TokenLedgerFlagEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeTokenLedgerStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: TokenLedgerLog | undefined
}>

const defaultLog: TokenLedgerLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

const postgresStoreForEnv = (
  env: TokenLedgerStoreEnv,
  options: MakeTokenLedgerStoreOptions,
): PostgresTokenLedgerStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresTokenLedgerStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The production `TokenLedgerWriteStore` for call sites that assemble
 * their own D1 ledger from a raw database handle (served-tokens recorder,
 * GLM pool heartbeat, Codex turn ingest): D1 authority + flag-gated
 * Postgres mirror. Returns undefined when the mirror is unavailable or
 * disabled — callers then fall back to the ledger's built-in D1 store.
 */
export const makeTokenLedgerWriteStoreForEnv = (
  env: TokenLedgerStoreEnv,
  options: MakeTokenLedgerStoreOptions = {},
): TokenLedgerWriteStore | undefined => {
  const flags = tokenLedgerFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return undefined
  }
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  return makeDualWriteTokenLedgerWriteStore({
    d1: makeD1TokenLedgerWriteStore(openAgentsDatabase(env)),
    flags,
    log: options.log ?? defaultLog,
    postgres,
  })
}

export type MakeTokenUsageLedgerForEnvOptions = MakeTokenLedgerStoreOptions &
  Readonly<{
    /** KS-6.3 (#8304) public-counter projection producer (fail-soft). */
    onIngestedEvent?: TokenUsageLedgerIngestObserver | undefined
  }>

/**
 * The production `TokenUsageLedgerShape` factory for the token ledger
 * domain: D1 authority + flag-gated Postgres dual-write, read routing on
 * the five public read paths, and a fail-soft leaderboard-preference
 * mirror. Replaces bare `makeD1TokenUsageLedger(...)` at Worker call
 * sites (KS-8.2, #8308).
 */
export const makeTokenUsageLedgerForEnv = (
  env: TokenLedgerStoreEnv,
  runtime: TokenUsageLedgerRuntime = systemTokenUsageLedgerRuntime,
  options: MakeTokenUsageLedgerForEnvOptions = {},
): TokenUsageLedgerShape => {
  const db = openAgentsDatabase(env)
  const flags = tokenLedgerFlagsFromEnv(env)
  const ledgerOptions =
    options.onIngestedEvent === undefined
      ? {}
      : { onIngestedEvent: options.onIngestedEvent }
  const postgres = postgresStoreForEnv(env, options)

  if (postgres === undefined || (!flags.dualWrite && flags.reads === 'd1')) {
    return makeD1TokenUsageLedger(db, runtime, ledgerOptions)
  }

  const log = options.log ?? defaultLog
  const writeStore = makeDualWriteTokenLedgerWriteStore({
    d1: makeD1TokenLedgerWriteStore(db),
    flags,
    log,
    postgres,
  })
  const d1Ledger = makeD1TokenUsageLedger(db, runtime, {
    ...ledgerOptions,
    writeStore,
  })
  const routed = makeReadRoutedTokenUsageLedger({
    d1: d1Ledger,
    flags,
    log,
    postgres: makePostgresPublicTokensReads(postgres, runtime),
  })

  if (!flags.dualWrite) {
    return routed
  }

  return {
    ...routed,
    updateLeaderboardPreference: (input, body) =>
      routed.updateLeaderboardPreference(input, body).pipe(
        Effect.tap(response =>
          Effect.promise(() =>
            postgres
              .mirrorLeaderboardPreference(response.preference)
              .catch((error: unknown) => {
                log('khala_sync_ledger_dual_write_failed', {
                  messageSafe: safeMessage(error),
                  op: 'updateLeaderboardPreference',
                  refs: [`${input.subjectKind}:${input.subjectRef}`],
                })
              }),
          ),
        ),
      ),
  }
}

/**
 * The env slice public read routes actually receive (they are handed the
 * whole Worker env as `input`, typed narrowly). Intersect route input
 * types with this so `tokenUsageLedgerFromRouteInput(input)` can pick up
 * the KS-8.2 binding + flags without widening the route contract.
 */
export type TokenLedgerRouteEnvSlice = Readonly<{
  OPENAGENTS_DB?: D1Database
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  KHALA_SYNC_LEDGER_DUAL_WRITE?: string | undefined
  KHALA_SYNC_LEDGER_READS?: string | undefined
}>

/** Drop-in for `makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)`. */
export const tokenUsageLedgerFromRouteInput = (
  input: TokenLedgerRouteEnvSlice,
): TokenUsageLedgerShape =>
  makeTokenUsageLedgerForEnv({
    KHALA_SYNC_DB: input.KHALA_SYNC_DB,
    KHALA_SYNC_LEDGER_DUAL_WRITE: input.KHALA_SYNC_LEDGER_DUAL_WRITE,
    KHALA_SYNC_LEDGER_READS: input.KHALA_SYNC_LEDGER_READS,
    OPENAGENTS_DB: input.OPENAGENTS_DB as D1Database,
  })

// ---------------------------------------------------------------------------
// Direct-insert mirror (khala chat + MCP recorder paths)
// ---------------------------------------------------------------------------

/**
 * The structural slice of a served-tokens ingest body the direct-insert
 * mirror needs (`buildServedTokensIngestBody` output satisfies it).
 */
export type DirectTokenLedgerIngestBody = Readonly<{
  eventId: string
  idempotencyKey: string
  observedAt: string
  producerSystem: string
  sourceRoute: string
  actor?: Readonly<{ accountRef?: string | undefined }> | undefined
  backendProfile?: string | null | undefined
  cost?: Readonly<{ amount: number; currency: string }> | null | undefined
  demand?:
    | Readonly<{
        demandChannel?: string | undefined
        demandClient?: string | undefined
        demandKind?: string | undefined
        demandSource?: string | undefined
      }>
    | undefined
  model?: string | null | undefined
  provider?: string | null | undefined
  privacy?:
    | Readonly<{
        leaderboardEligible?: boolean | undefined
        privacyOptOut?: boolean | undefined
      }>
    | undefined
  safeMetadata?: Record<string, unknown> | undefined
  tokenCounts: Readonly<{
    inputTokens: number
    outputTokens: number
    reasoningTokens: number
    cacheReadTokens: number
    cacheWrite5mTokens: number
    cacheWrite1hTokens: number
    totalTokens: number
  }>
  usageTruth: string
}>

/**
 * Build the exact row the D1 DIRECT-insert paths store (khala-chat public
 * completions and the khala-MCP recorder): refs NULLed, `demand_channel`
 * left to the D1 column default ('khala_api' — those INSERTs omit the
 * column), `role_ref` NULL. Byte-fidelity here is what makes the row-hash
 * reconciliation exact.
 */
export const directTokenLedgerRowFromIngestBody = (
  body: DirectTokenLedgerIngestBody,
  ingestedAt: string,
): TokenUsageEventRow => ({
  account_ref: body.actor?.accountRef ?? null,
  actor_team_id: null,
  actor_user_id: null,
  anonymized_source_ref: null,
  backend_profile: body.backendProfile ?? null,
  cache_read_tokens: body.tokenCounts.cacheReadTokens,
  cache_write_1h_tokens: body.tokenCounts.cacheWrite1hTokens,
  cache_write_5m_tokens: body.tokenCounts.cacheWrite5mTokens,
  cost_amount: body.cost?.amount ?? null,
  currency: body.cost?.currency ?? null,
  demand_channel: 'khala_api',
  demand_client: body.demand?.demandClient ?? null,
  demand_kind: body.demand?.demandKind ?? 'unlabeled',
  demand_source: body.demand?.demandSource ?? null,
  id: body.eventId,
  idempotency_key: body.idempotencyKey,
  ingested_at: ingestedAt,
  input_tokens: body.tokenCounts.inputTokens,
  leaderboard_eligible: body.privacy?.leaderboardEligible === false ? 0 : 1,
  model: body.model ?? null,
  observed_at: body.observedAt,
  output_tokens: body.tokenCounts.outputTokens,
  privacy_opt_out: body.privacy?.privacyOptOut === true ? 1 : 0,
  producer_system: body.producerSystem,
  provider: body.provider ?? null,
  reasoning_tokens: body.tokenCounts.reasoningTokens,
  repository_ref: null,
  role_ref: null,
  run_ref: null,
  safe_metadata_json: JSON.stringify(body.safeMetadata ?? {}),
  session_ref: null,
  source_route: body.sourceRoute,
  task_ref: null,
  total_tokens: body.tokenCounts.totalTokens,
  usage_truth: body.usageTruth,
})

/**
 * Fail-soft Postgres mirror for a row a D1 DIRECT-insert path just
 * inserted (`meta.changes > 0`). Event row only — NO rollup increments
 * (matching D1's behavior on those paths) and NO #8304 counter increment
 * (the direct paths fire their own producer hook exactly once, before and
 * independent of this mirror). Never throws.
 */
export const mirrorTokenLedgerDirectInsertBestEffort = async (
  env: TokenLedgerStoreEnv,
  row: TokenUsageEventRow,
  options: MakeTokenLedgerStoreOptions = {},
): Promise<void> => {
  const flags = tokenLedgerFlagsFromEnv(env)
  if (!flags.dualWrite) {
    return
  }
  const postgres = postgresStoreForEnv(env, options)
  if (postgres === undefined) {
    return
  }
  const log = options.log ?? defaultLog
  try {
    await postgres.insertDirectEventRow(row)
  } catch (error) {
    log('khala_sync_ledger_dual_write_failed', {
      messageSafe: safeMessage(error),
      op: 'insertDirectEventRow',
      refs: [row.id],
    })
  }
}
