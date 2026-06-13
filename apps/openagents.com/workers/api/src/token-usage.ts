import {
  type AutopilotTokenUsage,
  extractAutopilotTokenUsage,
  extractAutopilotTokenUsageFromJson,
} from '@openagentsinc/sync-schema'
import { Effect, Layer } from 'effect'
import * as Context from 'effect/Context'

import { OpenAgentsDatabase } from './bindings'
import type { OmniEventRecord } from './omni-runs'
import { currentIsoTimestamp } from './runtime-primitives'

export { extractAutopilotTokenUsage }

export type TokenUsageTotals = Readonly<{
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  cacheReadTokens: number
  cacheWrite5mTokens: number
  cacheWrite1hTokens: number
  totalTokens: number
  usageEvents: number
}>

export type TokenLeaderboardTeam = TokenUsageTotals &
  Readonly<{
    teamId: string
    teamName: string
    teamSlug: string | null
  }>

export type TokenLeaderboardUser = TokenUsageTotals &
  Readonly<{
    userId: string
    displayName: string
    email: string | null
    avatarUrl: string | null
    githubUsername: string | null
  }>

export type AutopilotTokenLeaderboards = Readonly<{
  generatedAt: string
  global: TokenUsageTotals
  currentUser: TokenUsageTotals
  teams: ReadonlyArray<TokenLeaderboardTeam>
  users: ReadonlyArray<TokenLeaderboardUser>
  currentUserTeams: ReadonlyArray<TokenLeaderboardTeam>
  missingUsageSignals: number
  recentRuns: ReadonlyArray<TokenUsageRunSummary>
}>

export type TokenUsageRuntime = Readonly<{
  nowIso: () => string
}>

export const systemTokenUsageRuntime: TokenUsageRuntime = {
  nowIso: currentIsoTimestamp,
}

export type TokenUsageRunSummary = TokenUsageTotals &
  Readonly<{
    runId: string
    title: string
    repository: string
    status: string
    runnerId: string
    updatedAt: string
  }>

type TokenUsageRow = Readonly<{
  input_tokens: number | null
  output_tokens: number | null
  reasoning_tokens: number | null
  cache_read_tokens: number | null
  cache_write_5m_tokens: number | null
  cache_write_1h_tokens: number | null
  total_tokens: number | null
  usage_events: number | null
}>

type TokenTeamRow = TokenUsageRow &
  Readonly<{
    team_id: string
    team_name: string
    team_slug: string | null
  }>

type TokenUserRow = TokenUsageRow &
  Readonly<{
    user_id: string
    display_name: string
    primary_email: string | null
    avatar_url: string | null
    github_username: string | null
  }>

type TokenRunRow = TokenUsageRow &
  Readonly<{
    run_id: string
    goal: string
    repository_owner: string
    repository_repo: string
    repository_ref: string
    status: string
    runner_id: string
    updated_at: string
  }>

type CountRow = Readonly<{
  count: number | null
}>

export const tokenUsageFromEvent = (
  event: OmniEventRecord,
): AutopilotTokenUsage | undefined => {
  return extractAutopilotTokenUsageFromJson(event.payloadJson)
}

export const sourceRefForTokenUsageEvent = (event: OmniEventRecord): string =>
  event.externalEventId ?? `${event.parentId}:${event.sequence}`

// Typed sentinel for token-usage rows whose write path genuinely cannot know
// the originating provider-account lease ref. Recording this value keeps the
// attribution discipline honest: it distinguishes "we looked and there was no
// account-leased work" from a faked or silently dropped attribution. Account
// aggregates expose these rows under the 'unattributed' bucket and never fold
// them into a real provider-account total.
export const TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF =
  'provider-account://unattributed' as const

export type TokenUsageAccountAttribution = Readonly<{
  accountRef: string
  attributed: boolean
}>

// Resolve the provider-account attribution for a leaderboard usage row from the
// run's lease-carried provider_account_ref. A run that was launched against an
// M8/M9 provider-account lease carries that lease's ref on `agent_runs`, so the
// usage row is attributed to it. A run with no lease ref (worker-secret or
// BYO-credential launches that never touch the account pool) is recorded as the
// typed unattributed sentinel rather than guessing an account.
export const resolveTokenUsageAccountAttribution = (
  runProviderAccountRef: string | null | undefined,
): TokenUsageAccountAttribution => {
  const trimmed = runProviderAccountRef?.trim()

  return trimmed === undefined || trimmed === ''
    ? { accountRef: TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF, attributed: false }
    : { accountRef: trimmed, attributed: true }
}

const totalsFromRow = (
  row: TokenUsageRow | null | undefined,
): TokenUsageTotals => ({
  inputTokens: row?.input_tokens ?? 0,
  outputTokens: row?.output_tokens ?? 0,
  reasoningTokens: row?.reasoning_tokens ?? 0,
  cacheReadTokens: row?.cache_read_tokens ?? 0,
  cacheWrite5mTokens: row?.cache_write_5m_tokens ?? 0,
  cacheWrite1hTokens: row?.cache_write_1h_tokens ?? 0,
  totalTokens: row?.total_tokens ?? 0,
  usageEvents: row?.usage_events ?? 0,
})

const runSummaryFromRow = (row: TokenRunRow): TokenUsageRunSummary => ({
  runId: row.run_id,
  title: row.goal,
  repository: `${row.repository_owner}/${row.repository_repo}@${row.repository_ref}`,
  status: row.status,
  runnerId: row.runner_id,
  updatedAt: row.updated_at,
  ...totalsFromRow(row),
})

export const readAutopilotTokenLeaderboards = async (
  db: D1Database,
  userId: string,
  runtime: TokenUsageRuntime = systemTokenUsageRuntime,
): Promise<AutopilotTokenLeaderboards> => {
  const [
    global,
    currentUser,
    teams,
    users,
    currentUserTeams,
    missingUsageSignals,
    recentRuns,
  ] = await Promise.all([
    db
      .prepare(
        `SELECT
            COALESCE(SUM(input_tokens), 0) AS input_tokens,
            COALESCE(SUM(output_tokens), 0) AS output_tokens,
            COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
            COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
            COALESCE(SUM(total_tokens), 0) AS total_tokens,
            COUNT(*) AS usage_events
         FROM autopilot_token_usage`,
      )
      .first<TokenUsageRow>(),
    db
      .prepare(
        `SELECT
            COALESCE(SUM(input_tokens), 0) AS input_tokens,
            COALESCE(SUM(output_tokens), 0) AS output_tokens,
            COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
            COALESCE(SUM(cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
            COALESCE(SUM(total_tokens), 0) AS total_tokens,
            COUNT(*) AS usage_events
         FROM autopilot_token_usage
         WHERE user_id = ?`,
      )
      .bind(userId)
      .first<TokenUsageRow>(),
    db
      .prepare(
        `SELECT
            teams.id AS team_id,
            teams.name AS team_name,
            teams.slug AS team_slug,
            COALESCE(SUM(autopilot_token_usage.input_tokens), 0) AS input_tokens,
            COALESCE(SUM(autopilot_token_usage.output_tokens), 0) AS output_tokens,
            COALESCE(SUM(autopilot_token_usage.reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
            COALESCE(SUM(autopilot_token_usage.total_tokens), 0) AS total_tokens,
            COUNT(autopilot_token_usage.id) AS usage_events
         FROM autopilot_token_usage
         INNER JOIN teams ON teams.id = autopilot_token_usage.team_id
         WHERE teams.status = 'active'
           AND teams.archived_at IS NULL
         GROUP BY teams.id, teams.name, teams.slug
         ORDER BY total_tokens DESC, team_name ASC
         LIMIT 20`,
      )
      .all<TokenTeamRow>(),
    db
      .prepare(
        `SELECT
            users.id AS user_id,
            users.display_name,
            users.primary_email,
            users.avatar_url,
            MAX(auth_identities.provider_username) AS github_username,
            COALESCE(SUM(autopilot_token_usage.input_tokens), 0) AS input_tokens,
            COALESCE(SUM(autopilot_token_usage.output_tokens), 0) AS output_tokens,
            COALESCE(SUM(autopilot_token_usage.reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
            COALESCE(SUM(autopilot_token_usage.total_tokens), 0) AS total_tokens,
            COUNT(autopilot_token_usage.id) AS usage_events
         FROM autopilot_token_usage
         INNER JOIN users ON users.id = autopilot_token_usage.user_id
         LEFT JOIN auth_identities
           ON auth_identities.user_id = users.id
          AND auth_identities.provider = 'github'
          AND auth_identities.deleted_at IS NULL
         WHERE users.status = 'active'
           AND users.deleted_at IS NULL
         GROUP BY users.id, users.display_name, users.primary_email, users.avatar_url
         ORDER BY total_tokens DESC, users.display_name ASC
         LIMIT 20`,
      )
      .all<TokenUserRow>(),
    db
      .prepare(
        `SELECT
            teams.id AS team_id,
            teams.name AS team_name,
            teams.slug AS team_slug,
            COALESCE(SUM(autopilot_token_usage.input_tokens), 0) AS input_tokens,
            COALESCE(SUM(autopilot_token_usage.output_tokens), 0) AS output_tokens,
            COALESCE(SUM(autopilot_token_usage.reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
            COALESCE(SUM(autopilot_token_usage.total_tokens), 0) AS total_tokens,
            COUNT(autopilot_token_usage.id) AS usage_events
         FROM team_memberships
         INNER JOIN teams ON teams.id = team_memberships.team_id
         LEFT JOIN autopilot_token_usage
           ON autopilot_token_usage.team_id = teams.id
         WHERE team_memberships.user_id = ?
           AND team_memberships.status = 'active'
           AND teams.status = 'active'
           AND teams.archived_at IS NULL
         GROUP BY teams.id, teams.name, teams.slug
         ORDER BY total_tokens DESC, team_name ASC`,
      )
      .bind(userId)
      .all<TokenTeamRow>(),
    db
      .prepare(
        `SELECT COUNT(*) AS count
         FROM agent_run_events
         INNER JOIN agent_runs ON agent_runs.id = agent_run_events.run_id
         WHERE agent_runs.user_id = ?
           AND agent_run_events.type IN (
             'usage.unavailable',
             'model.usage.unavailable',
             'token.usage.unavailable'
           )`,
      )
      .bind(userId)
      .first<CountRow>(),
    db
      .prepare(
        `SELECT
            agent_runs.id AS run_id,
            agent_runs.goal,
            agent_runs.repository_owner,
            agent_runs.repository_repo,
            agent_runs.repository_ref,
            agent_runs.status,
            agent_runs.runner_id,
            agent_runs.updated_at,
            COALESCE(SUM(autopilot_token_usage.input_tokens), 0) AS input_tokens,
            COALESCE(SUM(autopilot_token_usage.output_tokens), 0) AS output_tokens,
            COALESCE(SUM(autopilot_token_usage.reasoning_tokens), 0) AS reasoning_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_read_tokens), 0) AS cache_read_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_5m_tokens), 0) AS cache_write_5m_tokens,
            COALESCE(SUM(autopilot_token_usage.cache_write_1h_tokens), 0) AS cache_write_1h_tokens,
            COALESCE(SUM(autopilot_token_usage.total_tokens), 0) AS total_tokens,
            COUNT(autopilot_token_usage.id) AS usage_events
         FROM agent_runs
         LEFT JOIN autopilot_token_usage
           ON autopilot_token_usage.run_id = agent_runs.id
         WHERE agent_runs.user_id = ?
         GROUP BY agent_runs.id
         ORDER BY agent_runs.updated_at DESC
         LIMIT 20`,
      )
      .bind(userId)
      .all<TokenRunRow>(),
  ])

  return {
    currentUser: totalsFromRow(currentUser),
    currentUserTeams: currentUserTeams.results.map(row => ({
      teamId: row.team_id,
      teamName: row.team_name,
      teamSlug: row.team_slug,
      ...totalsFromRow(row),
    })),
    generatedAt: runtime.nowIso(),
    global: totalsFromRow(global),
    missingUsageSignals: missingUsageSignals?.count ?? 0,
    recentRuns: recentRuns.results.map(runSummaryFromRow),
    teams: teams.results.map(row => ({
      teamId: row.team_id,
      teamName: row.team_name,
      teamSlug: row.team_slug,
      ...totalsFromRow(row),
    })),
    users: users.results.map(row => ({
      userId: row.user_id,
      displayName: row.display_name,
      email: row.primary_email,
      avatarUrl: row.avatar_url,
      githubUsername: row.github_username,
      ...totalsFromRow(row),
    })),
  }
}

export type TokenUsageLeaderboardsShape = Readonly<{
  readForUser: (
    userId: string,
    runtime?: TokenUsageRuntime,
  ) => Promise<AutopilotTokenLeaderboards>
}>

export class TokenUsageLeaderboards extends Context.Service<
  TokenUsageLeaderboards,
  TokenUsageLeaderboardsShape
>()('@openagentsinc/TokenUsageLeaderboards') {
  static live = (db: D1Database) =>
    Layer.succeed(TokenUsageLeaderboards, {
      readForUser: (
        userId: string,
        runtime: TokenUsageRuntime = systemTokenUsageRuntime,
      ) => readAutopilotTokenLeaderboards(db, userId, runtime),
    })

  static effectCfLayer = () =>
    Layer.effect(
      TokenUsageLeaderboards,
      Effect.map(OpenAgentsDatabase, db => ({
        readForUser: (
          userId: string,
          runtime: TokenUsageRuntime = systemTokenUsageRuntime,
        ) => readAutopilotTokenLeaderboards(db, userId, runtime),
      })),
    )
}
