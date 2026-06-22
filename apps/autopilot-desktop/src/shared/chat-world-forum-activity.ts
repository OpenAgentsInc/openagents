// BF-3 (#5906): project forum_* world_events into Verse pylon message icons.
//
// The forum->Verse bridge (#5905) writes `world_event` rows of kind
// `forum_post` / `forum_reply` / `forum_tip_settled`, anchored to the agent's
// forum actor ref (entityRef). This module turns those rows into per-entity
// message-icon descriptors the Verse renderer attaches to the matching pylon /
// remote avatar (matched by `entityRef` == the multiplayer agent's `actorRef`).
//
// Pure + additive: no edits to the multiplayer projection internals. The icon is
// a dereferenceable proof handle (its sourceUrl opens the real public forum
// topic), and rendering degrades gracefully — if Cloudflare world is down there are
// simply no forum world_events and no icons; the Verse still loads.

export const FORUM_ACTIVITY_WORLD_RUN_REF = "run.public_forum_activity"

export const FORUM_ACTIVITY_EVENT_KINDS = [
  "forum_post",
  "forum_reply",
  "forum_tip_settled",
] as const
export type ForumActivityEventKind = (typeof FORUM_ACTIVITY_EVENT_KINDS)[number]

const isForumEventKind = (kind: string): kind is ForumActivityEventKind =>
  (FORUM_ACTIVITY_EVENT_KINDS as readonly string[]).includes(kind)

const DEFAULT_FORUM_BASE_URL = "https://openagents.com"

// A world_event row as projected by the Cloudflare world client (camelCase, matching
// the other ChatWorld*Row types). `summary` is the bridge's JSON string.
export type ChatWorldWorldEventRow = Readonly<{
  eventRef: string
  runRef: string
  eventKind: string
  entityRef: string
  sourceRef: string
  sourceGeneratedAt: string
  summary: string
}>

export type ForumPylonMessage = Readonly<{
  // The agent's forum actor ref — matches a multiplayer agent's actorRef.
  entityRef: string
  eventKind: ForumActivityEventKind
  sourceRef: string
  topicRef: string | null
  // Public-safe one-line text for the bubble/icon tooltip.
  summary: string
  // Dereferenceable public forum URL the icon opens (null when unresolvable).
  sourceUrl: string | null
  sourceGeneratedAt: string
}>

const text = (value: unknown): string =>
  typeof value === "string" ? value.trim() : ""

// The bridge wrote `summary` as JSON (forum-activity-transform.mjs eventSummary).
// Recover the public-safe text + topicRef; fall back to the raw string as text.
const parseSummary = (
  summary: string,
): { readonly text: string; readonly topicRef: string | null } => {
  try {
    const parsed = JSON.parse(summary) as Record<string, unknown>
    if (parsed !== null && typeof parsed === "object") {
      return {
        text: text(parsed.text) || text(parsed.summary),
        topicRef: text(parsed.topicRef) || null,
      }
    }
  } catch {
    // Not JSON — treat the whole string as the (already public-safe) text.
  }
  return { text: text(summary), topicRef: null }
}

const forumSourceUrl = (
  baseUrl: string,
  eventKind: ForumActivityEventKind,
  topicRef: string | null,
  sourceRef: string,
): string | null => {
  const root = baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl
  if (topicRef !== null && topicRef.length > 0) {
    // Replies deep-link to the specific post within the topic.
    return eventKind === "forum_reply" && sourceRef.length > 0
      ? `${root}/forum/t/${encodeURIComponent(topicRef)}#post-${encodeURIComponent(sourceRef)}`
      : `${root}/forum/t/${encodeURIComponent(topicRef)}`
  }
  // No topic ref recoverable — don't fabricate a link.
  return null
}

/**
 * Project forum_* world_event rows into per-entity message-icon descriptors,
 * keeping the most recent event per entity (by sourceGeneratedAt, then eventRef).
 * Pure; ignores non-forum kinds and malformed rows. Never throws.
 */
export const projectForumPylonMessages = (
  rows: ReadonlyArray<ChatWorldWorldEventRow> | null | undefined,
  options: { readonly baseUrl?: string } = {},
): ReadonlyArray<ForumPylonMessage> => {
  const baseUrl = options.baseUrl ?? DEFAULT_FORUM_BASE_URL
  const latestByEntity = new Map<string, ForumPylonMessage>()
  for (const row of rows ?? []) {
    const eventKind = text(row?.eventKind)
    const entityRef = text(row?.entityRef)
    if (!isForumEventKind(eventKind) || entityRef === "") continue
    const sourceRef = text(row?.sourceRef)
    const { text: summaryText, topicRef } = parseSummary(text(row?.summary))
    const message: ForumPylonMessage = {
      entityRef,
      eventKind,
      sourceRef,
      topicRef,
      summary: summaryText || "Forum activity",
      sourceUrl: forumSourceUrl(baseUrl, eventKind, topicRef, sourceRef),
      sourceGeneratedAt: text(row?.sourceGeneratedAt),
    }
    const prior = latestByEntity.get(entityRef)
    if (
      prior === undefined ||
      message.sourceGeneratedAt > prior.sourceGeneratedAt ||
      (message.sourceGeneratedAt === prior.sourceGeneratedAt &&
        row.eventRef > "")
    ) {
      latestByEntity.set(entityRef, message)
    }
  }
  return [...latestByEntity.values()].sort((a, b) =>
    a.entityRef.localeCompare(b.entityRef),
  )
}

/**
 * Index forum pylon messages by entityRef for O(1) renderer lookup.
 */
export const forumMessagesByEntityRef = (
  messages: ReadonlyArray<ForumPylonMessage>,
): ReadonlyMap<string, ForumPylonMessage> =>
  new Map(messages.map(message => [message.entityRef, message]))

/**
 * Additively attach a `forumMessage` to each entity (remote avatar / station)
 * whose `actorRef` matches a forum message. Generic so it never depends on the
 * multiplayer layer's concrete type (zero coupling to #5887's shapes). Entities
 * with no matching forum activity are returned unchanged.
 */
export const withForumPylonMessages = <T extends { readonly actorRef?: string }>(
  entities: ReadonlyArray<T>,
  byEntityRef: ReadonlyMap<string, ForumPylonMessage>,
): ReadonlyArray<T & { forumMessage?: ForumPylonMessage }> =>
  entities.map(entity => {
    const actorRef = text(entity.actorRef)
    const forumMessage =
      actorRef === "" ? undefined : byEntityRef.get(actorRef)
    return forumMessage === undefined ? entity : { ...entity, forumMessage }
  })
