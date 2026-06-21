import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

const badRequest = (reason: string) =>
  noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 })

// BF-1 (#5904): public-safe forum-activity projection source.
//
// The Verse reflection feature (Part B) needs a tokenless, public-safe feed of
// forum activity that the `project-forum-activity` service-identity bridge
// (#5905) can map into `world_event` rows. This endpoint emits ONLY public-safe
// fields drawn from already-public forum topics/posts:
//   { agentRef, pylonRef, eventKind, eventRef, sourceRef, topicRef,
//     sourceGeneratedAt, summary }
//
// Guardrails:
//   - Public read, no token. Only public, discoverable, non-archived forums and
//     visible posts are projected (the same filters as the public activity
//     timeline). No private/draft/hidden content, no payment material, no token,
//     no seeds, no raw addresses.
//   - No forum/business authority lives here — this is a read-only projection.
//     SpacetimeDB authority stays out (the bridge writes world rows, this only
//     surfaces public facts).

export const PublicForumActivityEndpoint = '/api/public/forum-activity'

const DEFAULT_LIMIT = 50
const MAX_LIMIT = 200
const SUMMARY_MAX = 160

export type PublicForumActivityEventKind = 'forum_post' | 'forum_reply'

export type PublicForumActivityRecord = {
  // Forum actor ref of the agent/user who posted (public identity ref).
  readonly agentRef: string
  // Pylon ref, when resolvable from the forum row. Null here — the bridge maps
  // agentRef -> pylon (ensure_pylon_agent_avatar) since forum rows carry no
  // pylon linkage. Kept in the shape so the bridge contract is stable.
  readonly pylonRef: string | null
  // forum_post for a new topic, forum_reply for a reply (non-first post).
  readonly eventKind: PublicForumActivityEventKind
  // Deterministic id (topic/post id) — the bridge uses it as a stable
  // `event_ref` so `append_world_event` no-ops on duplicates (idempotent).
  readonly eventRef: string
  // Dereferenceable public forum ref (the post/topic id).
  readonly sourceRef: string
  // The topic id, so a client can build /forum/t/{topicRef}.
  readonly topicRef: string
  readonly sourceGeneratedAt: string
  // Public-safe one-line summary (derived from the public topic title).
  readonly summary: string
}

export type PublicForumActivityEnvelope = {
  readonly generatedAt: string
  readonly sourceUrl: typeof PublicForumActivityEndpoint
  // Public-projection staleness contract (epic #4751). This projection is
  // live-at-read (queried from forum tables per request), rebuilt on forum
  // topic/post writes — so maxStalenessSeconds is 0.
  readonly staleness: PublicProjectionStalenessContract
  readonly maxStalenessSeconds: number
  readonly activity: ReadonlyArray<PublicForumActivityRecord>
}

const FORUM_ACTIVITY_STALENESS = liveAtReadStaleness([
  'forum_topic_created',
  'forum_post_created',
])

type ForumActivityRow = {
  kind: 'topic' | 'reply'
  event_ref: string
  topic_ref: string
  actor_ref: string
  title: string | null
  created_at: string
}

export type PublicForumActivityStore = {
  readonly listRecentActivity: (
    limit: number,
  ) => Promise<ReadonlyArray<PublicForumActivityRecord>>
}

const publicSafeSummary = (
  kind: PublicForumActivityEventKind,
  title: string | null,
): string => {
  const trimmed = (title ?? '').replace(/\s+/g, ' ').trim()
  if (trimmed.length === 0) {
    return kind === 'forum_post' ? 'Posted a new topic' : 'Replied on the Forum'
  }
  const verb = kind === 'forum_post' ? 'Posted' : 'Replied'
  return `${verb}: ${trimmed}`.slice(0, SUMMARY_MAX)
}

export const makeD1PublicForumActivityStore = (
  db: D1Database,
): PublicForumActivityStore => ({
  listRecentActivity: async limit => {
    // New topics -> forum_post.
    const topics = await db
      .prepare(
        `SELECT 'topic' AS kind,
                t.id AS event_ref,
                t.id AS topic_ref,
                t.actor_ref,
                t.title,
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
    // Replies (posts that are NOT the topic's first post) -> forum_reply. The
    // first post is already covered by its topic row, so excluding it avoids
    // double-counting one logical action.
    const replies = await db
      .prepare(
        `SELECT 'reply' AS kind,
                p.id AS event_ref,
                p.topic_id AS topic_ref,
                p.actor_ref,
                t.title,
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
            AND p.id != t.first_post_id
          ORDER BY p.created_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<ForumActivityRow>()

    return [...(topics.results ?? []), ...(replies.results ?? [])]
      .sort(
        (left, right) =>
          right.created_at.localeCompare(left.created_at) ||
          right.event_ref.localeCompare(left.event_ref),
      )
      .slice(0, limit)
      .map((row): PublicForumActivityRecord => {
        const eventKind: PublicForumActivityEventKind =
          row.kind === 'topic' ? 'forum_post' : 'forum_reply'
        return {
          agentRef: row.actor_ref,
          pylonRef: null,
          eventKind,
          eventRef: row.event_ref,
          sourceRef: row.event_ref,
          topicRef: row.topic_ref,
          sourceGeneratedAt: row.created_at,
          summary: publicSafeSummary(eventKind, row.title),
        }
      })
  },
})

const limitFromUrl = (url: URL): number | Response => {
  const raw = url.searchParams.get('limit')
  if (raw === null) return DEFAULT_LIMIT
  const parsed = Number(raw)
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > MAX_LIMIT) {
    return badRequest(`limit must be an integer in [1, ${MAX_LIMIT}]`)
  }
  return parsed
}

export type PublicForumActivityRouteInput = {
  readonly store?: PublicForumActivityStore
  readonly OPENAGENTS_DB?: D1Database
  readonly nowIso?: () => string
}

export const buildPublicForumActivityEnvelope = async (
  input: PublicForumActivityRouteInput,
  limit: number,
): Promise<PublicForumActivityEnvelope> => {
  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const store =
    input.store ??
    (input.OPENAGENTS_DB === undefined
      ? undefined
      : makeD1PublicForumActivityStore(input.OPENAGENTS_DB))
  const activity = store === undefined ? [] : await store.listRecentActivity(limit)
  return {
    generatedAt: nowIso,
    sourceUrl: PublicForumActivityEndpoint,
    staleness: FORUM_ACTIVITY_STALENESS,
    maxStalenessSeconds: FORUM_ACTIVITY_STALENESS.maxStalenessSeconds,
    activity,
  }
}

export const handlePublicForumActivityApi = (
  request: Request,
  input: PublicForumActivityRouteInput,
) => {
  if (request.method !== 'GET') return Effect.succeed(methodNotAllowed(['GET']))

  const url = new URL(request.url)
  const limit = limitFromUrl(url)
  if (limit instanceof Response) return Effect.succeed(limit)

  return Effect.promise(async () =>
    noStoreJsonResponse(await buildPublicForumActivityEnvelope(input, limit)),
  )
}

export const handlePublicForumActivityApiForEnv = (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
) =>
  handlePublicForumActivityApi(request, {
    OPENAGENTS_DB: openAgentsDatabase(env),
  })
