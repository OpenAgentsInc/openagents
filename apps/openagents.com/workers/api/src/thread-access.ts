import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  type AgentRunBundle,
  legacyAgentRunIdFromUuid,
  makeD1OmniRunStore,
} from './omni-runs'
import { openAgentsDatabase } from './runtime'
import { readActiveTeamMembershipRole } from './team-repository'

type ThreadAccessEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type ThreadAccessInput = Readonly<{
  routeId: string
  userId: string
}>

export type AgentRunAccessRow = Readonly<{
  id: string
  team_id: string | null
  user_id: string
}>

type TeamAutopilotThreadRunRow = Readonly<{
  agent_run_id: string
}>

export class RouteAccessNotFound extends S.TaggedErrorClass<RouteAccessNotFound>()(
  'RouteAccessNotFound',
  {
    routeId: S.String,
  },
) {}

export class RouteAccessForbidden extends S.TaggedErrorClass<RouteAccessForbidden>()(
  'RouteAccessForbidden',
  {
    routeId: S.String,
  },
) {}

export const RouteAccessError = S.Union([
  RouteAccessNotFound,
  RouteAccessForbidden,
])
export type RouteAccessError = typeof RouteAccessError.Type

// Promise-shaped read implementations, exported for callback seams whose
// contracts are Promise-based (the Khala Sync scope-read capability
// callbacks in ./khala-sync-scope-auth consume these directly, so no
// Effect.runPromise bridge is needed there). The Effect-shaped exports
// below stay the primary surface for Effect-composed callers.

export const readAgentRunAccessRowAsync = async (
  db: D1Database,
  runId: string,
): Promise<AgentRunAccessRow | undefined> => {
  const row = await db
    .prepare(
      `SELECT id, team_id, user_id
       FROM agent_runs
       WHERE id = ?
         AND archived_at IS NULL
       LIMIT 1`,
    )
    .bind(runId)
    .first<AgentRunAccessRow>()

  return row ?? undefined
}

export const resolveAgentRunIdAsync = async (
  db: D1Database,
  runId: string,
): Promise<string | undefined> => {
  const row = await readAgentRunAccessRowAsync(db, runId)

  if (row !== undefined) {
    return row.id
  }

  const legacyRunId = legacyAgentRunIdFromUuid(runId)

  if (legacyRunId === undefined) {
    return undefined
  }

  return (await readAgentRunAccessRowAsync(db, legacyRunId))?.id
}

export const resolveAgentRunIdForAutopilotThreadAsync = async (
  db: D1Database,
  threadId: string,
): Promise<string | undefined> => {
  const row = await db
    .prepare(
      `SELECT agent_run_id
       FROM team_chat_messages
       WHERE autopilot_thread_id = ?
         AND agent_run_id IS NOT NULL
         AND deleted_at IS NULL
       ORDER BY created_at DESC
       LIMIT 1`,
    )
    .bind(threadId)
    .first<TeamAutopilotThreadRunRow>()

  return row?.agent_run_id
}

export const readAgentRunAccessRow = (
  db: D1Database,
  runId: string,
): Effect.Effect<AgentRunAccessRow | undefined> =>
  Effect.promise(() => readAgentRunAccessRowAsync(db, runId))

export const resolveAgentRunId = (
  db: D1Database,
  runId: string,
): Effect.Effect<string | undefined> =>
  Effect.promise(() => resolveAgentRunIdAsync(db, runId))

export const resolveAgentRunIdForAutopilotThread = (
  db: D1Database,
  threadId: string,
): Effect.Effect<string | undefined> =>
  Effect.promise(() => resolveAgentRunIdForAutopilotThreadAsync(db, threadId))

const resolveRouteRunId = (
  db: D1Database,
  routeId: string,
): Effect.Effect<string, RouteAccessNotFound> =>
  Effect.gen(function* () {
    const runId =
      (yield* resolveAgentRunId(db, routeId)) ??
      (yield* resolveAgentRunIdForAutopilotThread(db, routeId))

    if (runId === undefined) {
      return yield* new RouteAccessNotFound({ routeId })
    }

    return runId
  })

const readActiveTeamMembership = (
  db: D1Database,
  teamId: string,
  userId: string,
): Effect.Effect<string | undefined> =>
  Effect.promise(() => readActiveTeamMembershipRole(db, teamId, userId))

const readAuthorizedBundle = (
  env: ThreadAccessEnv,
  input: ThreadAccessInput,
): Effect.Effect<AgentRunBundle, RouteAccessError> =>
  Effect.gen(function* () {
    const runId = yield* resolveRouteRunId(
      openAgentsDatabase(env),
      input.routeId,
    )
    const row = yield* readAgentRunAccessRow(openAgentsDatabase(env), runId)

    if (row === undefined) {
      return yield* new RouteAccessNotFound({ routeId: input.routeId })
    }

    if (row.team_id === null && row.user_id !== input.userId) {
      return yield* new RouteAccessForbidden({ routeId: input.routeId })
    }

    if (row.team_id !== null) {
      const role = yield* readActiveTeamMembership(
        openAgentsDatabase(env),
        row.team_id,
        input.userId,
      )

      if (role === undefined) {
        return yield* new RouteAccessForbidden({ routeId: input.routeId })
      }
    }

    const bundle = yield* Effect.promise(() =>
      makeD1OmniRunStore(openAgentsDatabase(env)).findAgentRunForUser(
        row.user_id,
        runId,
      ),
    )

    if (bundle === undefined) {
      return yield* new RouteAccessNotFound({ routeId: input.routeId })
    }

    return bundle
  })

export class ThreadAccessService extends Context.Service<
  ThreadAccessService,
  {
    readonly findAuthorizedBundle: (
      input: ThreadAccessInput,
    ) => Effect.Effect<AgentRunBundle, RouteAccessError>
  }
>()('@openagentsinc/autopilot-omega/ThreadAccessService') {
  static readonly layer = (env: ThreadAccessEnv) =>
    Layer.succeed(ThreadAccessService, {
      findAuthorizedBundle: Effect.fn(
        'ThreadAccessService.findAuthorizedBundle',
      )(input => readAuthorizedBundle(env, input)),
    })
}

export const runThreadAccess = <A, E>(
  env: ThreadAccessEnv,
  effect: Effect.Effect<A, E, ThreadAccessService>,
): Promise<A> =>
  Effect.runPromise(effect.pipe(Effect.provide(ThreadAccessService.layer(env))))

export const requireAuthorizedAgentRunBundle = (
  env: ThreadAccessEnv,
  userId: string,
  routeId: string,
): Promise<AgentRunBundle> =>
  runThreadAccess(
    env,
    Effect.gen(function* () {
      const threadAccess = yield* ThreadAccessService

      return yield* threadAccess.findAuthorizedBundle({ routeId, userId })
    }),
  )

export const findAuthorizedAgentRunBundle = (
  env: ThreadAccessEnv,
  userId: string,
  routeId: string,
): Promise<AgentRunBundle | undefined> =>
  runThreadAccess(
    env,
    Effect.gen(function* () {
      const threadAccess = yield* ThreadAccessService

      return yield* threadAccess.findAuthorizedBundle({ routeId, userId }).pipe(
        Effect.match({
          onFailure: () => undefined,
          onSuccess: value => value,
        }),
      )
    }),
  )
