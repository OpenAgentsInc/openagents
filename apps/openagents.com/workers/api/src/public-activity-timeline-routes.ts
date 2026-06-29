import { Effect } from 'effect'
import type {
  PublicActivityTimelineEnvelope,
  PublicActivityTimelineEvent,
} from '@openagentsinc/public-activity-timeline'

import { readArtanisTickMonitor } from './artanis-tick-monitor'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { makeD1InferenceReceiptStore } from './inference-receipts'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  buildPublicActivityTimelineEnvelope,
  publicActivityTimelineQueryFromUrl,
  type PublicActivityTimelineArtanisStore,
  type PublicActivityTimelineCapacityStore,
  type PublicActivityTimelineForumRecord,
  type PublicActivityTimelineForumStore,
  type PublicActivityTimelineInferenceReceiptStore,
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

const buildPublicActivityTimelineEnvelopeForRequest = async (
  request: Request,
  input: PublicActivityTimelineRouteInput,
  query: Exclude<ReturnType<typeof publicActivityTimelineQueryFromUrl>, Response>,
): Promise<PublicActivityTimelineEnvelope> => {
  void request
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
  const inferenceReceiptStore: PublicActivityTimelineInferenceReceiptStore | undefined =
    input.inferenceReceiptStore ??
    (db === undefined ? undefined : makeD1InferenceReceiptStore(db))
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
    ...(inferenceReceiptStore === undefined ? {} : { inferenceReceiptStore }),
    nowIso: () => nowIso,
    ...(pylonStore === undefined ? {} : { pylonStore }),
    query,
    ...(receiptStore === undefined ? {} : { receiptStore }),
    ...(trainingStore === undefined ? {} : { trainingStore }),
  }

  return buildPublicActivityTimelineEnvelope(sourceInput)
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
    const envelope = await buildPublicActivityTimelineEnvelopeForRequest(
      request,
      input,
      query,
    )

    return noStoreJsonResponse(envelope)
  })
}

const sanitizeSseLine = (value: string): string =>
  value.replace(/[\r\n]+/g, ' ')

const sseFrame = (
  input: Readonly<{
    data?: unknown
    event?: string
    id?: string
  }>,
): string => {
  const lines: string[] = []
  if (input.id !== undefined) lines.push(`id: ${sanitizeSseLine(input.id)}`)
  if (input.event !== undefined) {
    lines.push(`event: ${sanitizeSseLine(input.event)}`)
  }
  if (input.data !== undefined) {
    lines.push(`data: ${JSON.stringify(input.data)}`)
  }
  return `${lines.join('\n')}\n\n`
}

const pollingFallbackUrl = (
  request: Request,
  envelope: PublicActivityTimelineEnvelope,
): string => {
  const url = new URL(request.url)
  url.pathname = '/api/public/activity-timeline'
  const cursor = envelope.nextCursor ?? envelope.events.at(-1)?.cursor
  if (cursor !== undefined) url.searchParams.set('since', cursor)
  if (!url.searchParams.has('limit')) url.searchParams.set('limit', '50')
  return url.toString()
}

const activityTimelineMetaFrame = (
  envelope: PublicActivityTimelineEnvelope,
) =>
  sseFrame({
    event: 'activity_timeline_meta',
    data: {
      generatedAt: envelope.generatedAt,
      nextCursor: envelope.nextCursor ?? null,
      range: envelope.range,
      schemaVersion: envelope.schemaVersion,
      sourceLag: envelope.sourceLag,
      staleness: envelope.staleness,
    },
  })

const activityTimelineEventFrame = (
  event: PublicActivityTimelineEvent,
) =>
  sseFrame({
    data: { event },
    event: event.kind,
    id: event.cursor,
  })

const publicActivityTimelineStreamPayload = (
  request: Request,
  envelope: PublicActivityTimelineEnvelope,
): string => {
  const fallback = pollingFallbackUrl(request, envelope)
  const eventFrames = envelope.events.map(activityTimelineEventFrame).join('')
  return [
    'retry: 15000\n',
    `: polling-fallback ${sanitizeSseLine(fallback)}\n\n`,
    activityTimelineMetaFrame(envelope),
    eventFrames === '' ? ': no events\n\n' : eventFrames,
  ].join('')
}

const publicActivityTimelineStreamResponse = (
  request: Request,
  envelope: PublicActivityTimelineEnvelope,
) =>
  new Response(publicActivityTimelineStreamPayload(request, envelope), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/event-stream; charset=utf-8',
      'x-accel-buffering': 'no',
      'x-openagents-authority': 'observation_only',
      'x-openagents-polling-fallback': pollingFallbackUrl(request, envelope),
    },
  })

const requestWithLastEventCursor = (request: Request): Request => {
  const url = new URL(request.url)
  if (url.searchParams.has('since')) return request
  const cursor = request.headers.get('Last-Event-ID')?.trim()
  if (cursor === undefined || cursor === '') return request
  url.searchParams.set('since', cursor)
  return new Request(url.toString(), request)
}

export const handlePublicActivityTimelineStreamApi = (
  request: Request,
  input: PublicActivityTimelineRouteInput,
) => {
  if (request.method !== 'GET') return Effect.succeed(methodNotAllowed(['GET']))

  const resumedRequest = requestWithLastEventCursor(request)
  const url = new URL(resumedRequest.url)
  const query = publicActivityTimelineQueryFromUrl(url)

  if (query instanceof Response) {
    return Effect.succeed(query)
  }

  return Effect.promise(async () => {
    const envelope = await buildPublicActivityTimelineEnvelopeForRequest(
      resumedRequest,
      input,
      query,
    )

    return publicActivityTimelineStreamResponse(resumedRequest, envelope)
  })
}

export const handlePublicActivityTimelineApiForEnv = (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
) =>
  handlePublicActivityTimelineApi(request, {
    OPENAGENTS_DB: openAgentsDatabase(env),
  })

export const handlePublicActivityTimelineStreamApiForEnv = (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
) =>
  handlePublicActivityTimelineStreamApi(request, {
    OPENAGENTS_DB: openAgentsDatabase(env),
  })
