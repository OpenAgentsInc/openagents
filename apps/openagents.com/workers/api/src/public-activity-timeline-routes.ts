import { Effect } from 'effect'

import { readArtanisTickMonitor } from './artanis-tick-monitor'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  buildPublicActivityTimelineEnvelope,
  publicActivityTimelineQueryFromUrl,
  type PublicActivityTimelineArtanisStore,
  type PublicActivityTimelineCapacityStore,
  type PublicActivityTimelineForumRecord,
  type PublicActivityTimelineForumStore,
  type PublicActivityTimelineSourceInput,
} from './public-activity-timeline'
import { makeD1PylonApiStore } from './pylon-api'
import {
  makeD1PylonCapacityFunnelSnapshotStore,
} from './pylon-capacity-funnel-live-routes'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import { makeD1TrainingAuthorityStore } from './training-run-window-authority'

type PublicActivityTimelineRouteInput = Readonly<
  PublicActivityTimelineSourceInput & {
    OPENAGENTS_DB?: D1Database
  }
>

type ForumActivityRow = Readonly<{
  actor_ref: string | null
  created_at: string
  event_ref: string
  kind: 'topic' | 'post'
  post_ref: string | null
  receipt_refs_json: string | null
  state: string
  title: string | null
  topic_ref: string
}>

export const makeD1PublicActivityTimelineForumStore = (
  db: D1Database,
): PublicActivityTimelineForumStore => ({
  listRecentActivity: async limit => {
    const topics = await db
      .prepare(
        `SELECT 'topic' AS kind,
                t.id AS event_ref,
                t.id AS topic_ref,
                NULL AS post_ref,
                t.actor_ref,
                t.title,
                t.state,
                '[]' AS receipt_refs_json,
                t.created_at
           FROM forum_topics t
           JOIN forum_forums f ON f.id = t.forum_id
          WHERE t.archived_at IS NULL
            AND f.archived_at IS NULL
            AND f.visibility = 'public'
            AND f.discoverability != 'hidden'
            AND t.state IN ('open', 'locked')
          ORDER BY t.created_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<ForumActivityRow>()
    const posts = await db
      .prepare(
        `SELECT 'post' AS kind,
                p.id AS event_ref,
                p.topic_id AS topic_ref,
                p.id AS post_ref,
                p.actor_ref,
                t.title,
                p.state,
                p.receipt_refs_json,
                p.created_at
           FROM forum_posts p
           JOIN forum_topics t ON t.id = p.topic_id
           JOIN forum_forums f ON f.id = p.forum_id
          WHERE p.archived_at IS NULL
            AND t.archived_at IS NULL
            AND f.archived_at IS NULL
            AND f.visibility = 'public'
            AND f.discoverability != 'hidden'
            AND t.state IN ('open', 'locked')
            AND p.state IN ('visible', 'edited')
          ORDER BY p.created_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<ForumActivityRow>()

    return [...(topics.results ?? []), ...(posts.results ?? [])]
      .sort(
        (left, right) =>
          right.created_at.localeCompare(left.created_at) ||
          right.event_ref.localeCompare(left.event_ref),
      )
      .slice(0, limit)
      .map(
        (row): PublicActivityTimelineForumRecord => ({
          actorRef: row.actor_ref,
          createdAt: row.created_at,
          eventRef: row.event_ref,
          kind: row.kind,
          postRef: row.post_ref,
          sourceRefs: [
            row.topic_ref,
            ...(row.post_ref === null ? [] : [row.post_ref]),
            ...parseJsonStringArray(row.receipt_refs_json),
            'route:/api/forum',
          ],
          state: row.state,
          title: row.title,
          topicRef: row.topic_ref,
        }),
      )
  },
})

export const makeD1PublicActivityTimelineArtanisStore = (
  db: D1Database,
  nowIso: string,
): PublicActivityTimelineArtanisStore => ({
  listRecentTicks: async limit => {
    const monitor = await readArtanisTickMonitor(db, { limit, nowIso })
    return monitor.decisions.map(decision => ({
      assignmentRef: decision.assignmentRef,
      createdAt: decision.createdAt,
      decisionRef: decision.decisionRef,
      sourceRefs: ['route:/api/public/artanis/admin-ticks', decision.decisionRef],
      state: decision.state,
    }))
  },
})

export const makeD1PublicActivityTimelineCapacityStore = (
  db: D1Database,
): PublicActivityTimelineCapacityStore => {
  const snapshotStore = makeD1PylonCapacityFunnelSnapshotStore(db)

  return {
    listRecentSnapshots: async limit => {
      const snapshots = await snapshotStore.listSnapshots({
        bucketKind: 'hourly',
        limit,
      })

      return snapshots.map(snapshot => {
        const projection = parseJsonRecord(snapshot.publicProjectionJson)
        const aggregate = parseJsonRecord(JSON.stringify(projection?.funnel ?? {}))
        const totalCount =
          typeof aggregate?.totalCount === 'number'
            ? aggregate.totalCount
            : snapshot.totalCount

        return {
          aggregateState: `total:${totalCount}`,
          snapshotAt: snapshot.snapshotAt,
          snapshotRef: snapshot.id,
          sourceRefs: [
            snapshot.id,
            'route:/api/public/pylon-capacity-funnel/history',
          ],
        }
      })
    },
  }
}

export const handlePublicActivityTimelineApi = (
  request: Request,
  input: PublicActivityTimelineRouteInput,
) => {
  if (request.method !== 'GET') return Effect.succeed(methodNotAllowed(['GET']))

  const url = new URL(request.url)
  const query = publicActivityTimelineQueryFromUrl(url)

  if (query instanceof Response) {
    return Effect.succeed(query)
  }

  return Effect.promise(async () => {
    const db = input.OPENAGENTS_DB ?? undefined
    const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
    const artanisStore =
      input.artanisStore ??
      (db === undefined
        ? undefined
        : makeD1PublicActivityTimelineArtanisStore(db, nowIso))
    const capacityStore =
      input.capacityStore ??
      (db === undefined
        ? undefined
        : makeD1PublicActivityTimelineCapacityStore(db))
    const forumStore =
      input.forumStore ??
      (db === undefined ? undefined : makeD1PublicActivityTimelineForumStore(db))
    const pylonStore =
      input.pylonStore ?? (db === undefined ? undefined : makeD1PylonApiStore(db))
    const receiptStore =
      input.receiptStore ??
      (db === undefined ? undefined : makeD1NexusTreasuryPayoutLedgerStore(db))
    const trainingStore =
      input.trainingStore ??
      (db === undefined ? undefined : makeD1TrainingAuthorityStore(db))
    const sourceInput: PublicActivityTimelineSourceInput = {
      ...(artanisStore === undefined ? {} : { artanisStore }),
      ...(capacityStore === undefined ? {} : { capacityStore }),
      ...(forumStore === undefined ? {} : { forumStore }),
      nowIso: () => nowIso,
      ...(pylonStore === undefined ? {} : { pylonStore }),
      query,
      ...(receiptStore === undefined ? {} : { receiptStore }),
      ...(trainingStore === undefined ? {} : { trainingStore }),
    }

    const envelope = await buildPublicActivityTimelineEnvelope(sourceInput)

    return noStoreJsonResponse(envelope)
  })
}

export const handlePublicActivityTimelineApiForEnv = (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
) =>
  handlePublicActivityTimelineApi(request, {
    OPENAGENTS_DB: openAgentsDatabase(env),
  })
