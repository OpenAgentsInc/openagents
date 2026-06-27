import { Effect } from 'effect'

import {
  ArtanisPublicReportUnsafe,
  artanisFleetActivitySummary,
  artanisPublicReportSnapshot,
} from './artanis-public-report'
import { ArtanisLoopTickRecord, type ArtanisLoopState } from './artanis-loop'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  decodeUnknownWithSchema,
  isRecord,
  parseJsonUnknown,
  stringArrayFromUnknown,
} from './json-boundary'
import {
  type ArtanisPersistenceStoredRow,
  readLatestArtanisPersistedRows,
} from './artanis-persistence'
import {
  type PublicPylonStatsStore,
  publicPylonStatsSnapshot,
} from './public-pylon-stats'
import { makeD1PylonApiStore, type PylonApiStore } from './pylon-api'
import { currentIsoTimestamp } from './runtime-primitives'

const routeErrorResponse = (error: ArtanisPublicReportUnsafe) =>
  noStoreJsonResponse(
    {
      error: 'public_artanis_report_unsafe',
      reason: error.reason,
    },
    { status: 500 },
  )

type PublicArtanisReportRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  loopTicks?: ReadonlyArray<ArtanisLoopTickRecord>
  pylonApiStore?: PylonApiStore
  store?: PublicPylonStatsStore
}>

const closeoutStates = new Set<ArtanisLoopState>([
  'blocked',
  'completed',
  'failed',
])

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const closeoutReceiptRefsFromJson = (
  value: string | null,
): ReadonlyArray<string> => {
  if (value === null) {
    return []
  }

  const parsed = parseJsonUnknown(value)

  return isRecord(parsed)
    ? stringArrayFromUnknown(parsed.closeoutReceiptRefs)
    : []
}

const completedTickCanProject = (tick: ArtanisLoopTickRecord): boolean =>
  tick.closeoutReceiptRefs.length > 0 &&
  tick.artifactRefs.length > 0 &&
  tick.forumPublicationIntentRefs.length > 0 &&
  tick.nextTickAtIso !== null

const loopTickFromStoredRow = (
  row: ArtanisPersistenceStoredRow,
): ArtanisLoopTickRecord => {
  const current = decodeUnknownWithSchema(ArtanisLoopTickRecord, row.record)
  const closeoutReceiptRefs = uniqueRefs([
    ...current.closeoutReceiptRefs,
    ...closeoutReceiptRefsFromJson(row.closeoutJson),
  ])
  const rowState = closeoutStates.has(row.state as ArtanisLoopState)
    ? (row.state as ArtanisLoopState)
    : current.state
  const candidate = new ArtanisLoopTickRecord({
    ...current,
    closeoutReceiptRefs,
    state: rowState,
    updatedAtIso: row.updatedAtIso,
  })

  return rowState === 'completed' && !completedTickCanProject(candidate)
    ? new ArtanisLoopTickRecord({
        ...candidate,
        state: current.state,
      })
    : candidate
}

const loopTicksForRoute = (
  input: PublicArtanisReportRouteInput,
) =>
  input.loopTicks !== undefined
    ? Effect.succeed(input.loopTicks)
    : input.OPENAGENTS_DB === undefined
      ? Effect.succeed([])
      : readLatestArtanisPersistedRows(
          input.OPENAGENTS_DB,
          'loop_tick',
          50,
        ).pipe(
          Effect.mapError(
            error =>
              new ArtanisPublicReportUnsafe({
                reason: `Artanis persisted loop ticks unavailable: ${error.reason}`,
              }),
          ),
          Effect.flatMap(rows =>
            Effect.try({
              catch: () =>
                new ArtanisPublicReportUnsafe({
                  reason:
                    'Artanis persisted loop ticks could not be decoded for the public report.',
                }),
              try: () => rows.map(loopTickFromStoredRow),
            }),
          ),
        )

const fleetActivityForRoute = async (
  input: PublicArtanisReportRouteInput,
) => {
  const store =
    input.pylonApiStore ??
    (input.OPENAGENTS_DB === undefined
      ? undefined
      : makeD1PylonApiStore(input.OPENAGENTS_DB))

  if (store === undefined) {
    return undefined
  }

  const registrations = await store.listRegistrations(100)
  const pylonRefs = registrations.map(registration => registration.pylonRef)
  const assignments =
    store.listAssignmentsForPylons === undefined
      ? (
          await Promise.all(
            pylonRefs.map(pylonRef =>
              store.listAssignmentsForPylon(pylonRef, 25),
            ),
          )
        ).flat()
      : await store.listAssignmentsForPylons(pylonRefs, 250)

  return artanisFleetActivitySummary({
    assignments,
    nowIso: currentIsoTimestamp(),
    registrations,
  })
}

export const handlePublicArtanisReportApi = (
  request: Request,
  input: PublicArtanisReportRouteInput,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.all({
        fleetActivity: Effect.tryPromise({
          catch: error =>
            new ArtanisPublicReportUnsafe({
              reason:
                error instanceof Error
                  ? `Artanis fleet activity unavailable: ${error.message}`
                  : 'Artanis fleet activity unavailable.',
            }),
          try: () => fleetActivityForRoute(input),
        }).pipe(Effect.catch(() => Effect.succeed(undefined))),
        loopTicks: loopTicksForRoute(input),
        pylonStats: publicPylonStatsSnapshot({
          store:
            input.store ?? makeD1PylonApiStore(input.OPENAGENTS_DB as D1Database),
        }),
      }).pipe(
        Effect.flatMap(({ fleetActivity, loopTicks, pylonStats }) =>
          Effect.try({
            try: () =>
              noStoreJsonResponse(
                artanisPublicReportSnapshot({
                  fleetActivity,
                  loopTicks,
                  pylonStats,
                }),
              ),
            catch: error =>
              error instanceof ArtanisPublicReportUnsafe
                ? error
                : new ArtanisPublicReportUnsafe({
                    reason: 'Artanis public report projection failed.',
                  }),
          }),
        ),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
