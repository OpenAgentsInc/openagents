import { Schema as S } from "effect"
import { verifyEvent } from "nostr-effect/pure"

import {
  NostrEvent,
  PUBLIC_CHAT_GROUP_ID,
  PUBLIC_CHAT_RELAY_URL,
  PublicChatAttachment,
  hasContentWarning,
  isAuthorDeletion,
  parseInlineAttachments,
  relayGroupAdministrators,
  stableChronological,
} from "./profile.js"

export const PUBLIC_CHAT_PARITY_SCHEMA =
  "openagents.agent_chat_parity_fixture.v1" as const

export const PublicChatRelayState = S.Literals([
  "disconnected",
  "connecting",
  "replaying",
  "current",
  "reconnecting",
  "stale",
])
export type PublicChatRelayState = typeof PublicChatRelayState.Type

export const PublicChatMediaState = S.Literals([
  "gated",
  "loading",
  "verified",
  "mismatch",
  "unavailable",
])
export type PublicChatMediaState = typeof PublicChatMediaState.Type

export const PublicChatMediaAction = S.Literals([
  "load-requested",
  "load-verified",
  "digest-mismatch",
  "load-unavailable",
])
export type PublicChatMediaAction = typeof PublicChatMediaAction.Type

export const PublicChatContentPart = S.Union([
  S.Struct({
    type: S.Literal("text"),
    value: S.String,
  }),
  S.Struct({
    type: S.Literal("http-link"),
    value: S.String,
  }),
  S.Struct({
    type: S.Literal("nostr-reference"),
    value: S.String,
  }),
])
export type PublicChatContentPart = typeof PublicChatContentPart.Type

export const PublicChatAuthorProfile = S.Struct({
  bot: S.Boolean,
  displayName: S.NullOr(S.String),
})
export type PublicChatAuthorProfile = typeof PublicChatAuthorProfile.Type

export const PublicChatReactionCount = S.Struct({
  count: S.Int,
  value: S.String,
})
export type PublicChatReactionCount = typeof PublicChatReactionCount.Type

export const PublicChatTimelineItem = S.Struct({
  attachments: S.Array(PublicChatAttachment),
  contentParts: S.Array(PublicChatContentPart),
  contentWarning: S.Boolean,
  deletion: S.NullOr(S.Literals(["author", "moderator"])),
  event: NostrEvent,
  pinned: S.Boolean,
  profile: S.NullOr(PublicChatAuthorProfile),
  reactions: S.Array(PublicChatReactionCount),
})
export type PublicChatTimelineItem = typeof PublicChatTimelineItem.Type

export const PublicChatBehaviorMatrixRow = S.Struct({
  desktopState: PublicChatRelayState,
  emptyView: S.Literals(["loading-history", "quiet-channel"]),
  isCurrent: S.Boolean,
  lastCurrentAt: S.Literals(["preserve", "update"]),
  state: PublicChatRelayState,
  verifiedTimeline: S.Literal("retain"),
  webStatus: S.Literals([
    "Current",
    "Offline · history may be stale",
    "Reconnecting · repairing history",
  ]),
})
export type PublicChatBehaviorMatrixRow =
  typeof PublicChatBehaviorMatrixRow.Type

export const PUBLIC_CHAT_BEHAVIOR_MATRIX: ReadonlyArray<PublicChatBehaviorMatrixRow> =
  [
    {
      desktopState: "disconnected",
      emptyView: "quiet-channel",
      isCurrent: false,
      lastCurrentAt: "preserve",
      state: "disconnected",
      verifiedTimeline: "retain",
      webStatus: "Reconnecting · repairing history",
    },
    {
      desktopState: "connecting",
      emptyView: "loading-history",
      isCurrent: false,
      lastCurrentAt: "preserve",
      state: "connecting",
      verifiedTimeline: "retain",
      webStatus: "Reconnecting · repairing history",
    },
    {
      desktopState: "replaying",
      emptyView: "quiet-channel",
      isCurrent: false,
      lastCurrentAt: "preserve",
      state: "replaying",
      verifiedTimeline: "retain",
      webStatus: "Reconnecting · repairing history",
    },
    {
      desktopState: "current",
      emptyView: "quiet-channel",
      isCurrent: true,
      lastCurrentAt: "update",
      state: "current",
      verifiedTimeline: "retain",
      webStatus: "Current",
    },
    {
      desktopState: "reconnecting",
      emptyView: "quiet-channel",
      isCurrent: false,
      lastCurrentAt: "preserve",
      state: "reconnecting",
      verifiedTimeline: "retain",
      webStatus: "Reconnecting · repairing history",
    },
    {
      desktopState: "stale",
      emptyView: "quiet-channel",
      isCurrent: false,
      lastCurrentAt: "preserve",
      state: "stale",
      verifiedTimeline: "retain",
      webStatus: "Offline · history may be stale",
    },
  ]

const parseProfile = (content: string): PublicChatAuthorProfile | undefined => {
  try {
    const profile: unknown = JSON.parse(content)
    if (typeof profile !== "object" || profile === null) return undefined
    const displayName =
      "display_name" in profile && typeof profile.display_name === "string"
        ? profile.display_name
        : "name" in profile && typeof profile.name === "string"
          ? profile.name
          : null
    return {
      bot: "bot" in profile && profile.bot === true,
      displayName,
    }
  } catch {
    return undefined
  }
}

export const parsePublicChatContent = (
  content: string,
): ReadonlyArray<PublicChatContentPart> =>
  content
    .split(/(https?:\/\/[^\s]+|nostr:[a-z0-9]+)/gi)
    .filter((value) => value.length > 0)
    .map((value) =>
      /^https?:\/\//i.test(value)
        ? { type: "http-link" as const, value }
        : /^nostr:/i.test(value)
          ? { type: "nostr-reference" as const, value }
          : { type: "text" as const, value },
    )

export const projectPublicChatTimeline = (
  sourceEvents: ReadonlyArray<NostrEvent>,
  groupId: string = PUBLIC_CHAT_GROUP_ID,
): ReadonlyArray<PublicChatTimelineItem> => {
  const events = stableChronological(sourceEvents)
  const administrators = relayGroupAdministrators(events)
  const profiles = new Map<string, PublicChatAuthorProfile>()
  for (const profileEvent of events.filter((event) => event.kind === 0)) {
    const profile = parseProfile(profileEvent.content)
    if (profile !== undefined) profiles.set(profileEvent.pubkey, profile)
  }

  const deleted = new Map<string, "author" | "moderator">()
  for (const deletion of events.filter((event) => event.kind === 5)) {
    for (const target of events) {
      if (isAuthorDeletion(deletion, target, groupId)) {
        deleted.set(target.id, "author")
      }
    }
  }
  for (const deletion of events.filter(
    (event) => event.kind === 9005 && administrators.has(event.pubkey),
  )) {
    for (const targetId of deletion.tags
      .filter((tag) => tag[0] === "e")
      .map((tag) => tag[1])
      .filter((value): value is string => value !== undefined)) {
      deleted.set(targetId, "moderator")
    }
  }

  const reactions = new Map<string, Map<string, number>>()
  for (const reaction of events.filter(
    (event) =>
      event.kind === 7 &&
      verifyEvent({
        ...event,
        tags: event.tags.map((tag) => [...tag]),
      }),
  )) {
    const target = reaction.tags.find((tag) => tag[0] === "e")?.[1]
    if (target === undefined || !events.some((event) => event.id === target)) {
      continue
    }
    const values = reactions.get(target) ?? new Map<string, number>()
    values.set(reaction.content, (values.get(reaction.content) ?? 0) + 1)
    reactions.set(target, values)
  }

  // `.sort` on an already-copied array, not `.toSorted`. `.toSorted` failed
  // the repository typecheck gate with TS2550 ("change lib to es2023 or
  // later") plus five cascading implicit-any errors, in this package and in
  // the api worker that typechecks this source transitively. Same
  // non-mutating semantics, and no dependency on any `lib` setting.
  const pinState = [...events.filter((event) => event.kind === 39005)].sort(
    (left, right) =>
      right.created_at - left.created_at || right.id.localeCompare(left.id),
  )[0]
  const pinnedIds = new Set(
    pinState?.tags
      .filter((tag) => tag[0] === "e")
      .map((tag) => tag[1])
      .filter((value): value is string => value !== undefined) ?? [],
  )

  return events
    .filter((event) => event.kind === 9 || event.kind === 1337)
    .map((event) => ({
      attachments: [...parseInlineAttachments(event)],
      contentParts: [...parsePublicChatContent(event.content)],
      contentWarning: hasContentWarning(event),
      deletion: deleted.get(event.id) ?? null,
      event,
      pinned: pinnedIds.has(event.id),
      profile: profiles.get(event.pubkey) ?? null,
      reactions: [...(reactions.get(event.id) ?? new Map()).entries()].map(
        ([value, count]) => ({
          count,
          value,
        }),
      ),
    }))
}

export const transitionPublicChatMedia = (
  _state: PublicChatMediaState,
  action: PublicChatMediaAction,
): PublicChatMediaState => {
  switch (action) {
    case "load-requested":
      return "loading"
    case "load-verified":
      return "verified"
    case "digest-mismatch":
      return "mismatch"
    case "load-unavailable":
      return "unavailable"
  }
}

const sha256Hex = async (bytes: ArrayBuffer): Promise<string> =>
  [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("")

export const verifyPublicChatMedia = async (
  attachment: PublicChatAttachment,
  bytes: ArrayBuffer,
): Promise<"verified" | "mismatch"> =>
  attachment.digest === undefined ||
  (await sha256Hex(bytes)) === attachment.digest
    ? "verified"
    : "mismatch"

const PublicChatParityExpectedTimelineItem = S.Struct({
  attachments: S.Array(PublicChatAttachment),
  contentParts: S.Array(PublicChatContentPart),
  contentWarning: S.Boolean,
  deletion: S.NullOr(S.Literals(["author", "moderator"])),
  eventId: NostrEvent.fields.id,
  kind: S.Int,
  pinned: S.Boolean,
  profile: S.NullOr(PublicChatAuthorProfile),
  reactions: S.Array(PublicChatReactionCount),
})

const PublicChatParityMediaStep = S.Struct({
  action: PublicChatMediaAction,
  expectedState: PublicChatMediaState,
})

export const PublicChatParityFixture = S.Struct({
  behaviorMatrix: S.Array(PublicChatBehaviorMatrixRow),
  lifecycle: S.Struct({
    expectedStateSequence: S.Array(PublicChatRelayState),
    historyPageSize: S.Int,
    latestEventCreatedAt: S.Int,
    oldestAcceptedEventCreatedAt: S.Int,
    reconnectDelayMs: S.Int,
    reconnectOverlapSeconds: S.Int,
    relaySelfPubkey: NostrEvent.fields.pubkey,
    requiredEose: S.Array(S.Literals(["history", "group-state"])),
  }),
  media: S.Struct({
    cases: S.Array(
      S.Struct({
        expectedMessageVisible: S.Boolean,
        initialState: PublicChatMediaState,
        name: S.String,
        steps: S.Array(PublicChatParityMediaStep),
      }),
    ),
    digestHex: NostrEvent.fields.id,
    payloadBase64: S.String,
  }),
  projection: S.Struct({
    events: S.Array(NostrEvent),
    expectedTimeline: S.Array(PublicChatParityExpectedTimelineItem),
  }),
  schemaVersion: S.Literal(PUBLIC_CHAT_PARITY_SCHEMA),
  source: S.Struct({
    groupId: S.Literal(PUBLIC_CHAT_GROUP_ID),
    manifestPath: S.Literal("/api/public/nostr-chat/manifest"),
    messageKinds: S.Tuple([S.Literal(9), S.Literal(1337)]),
    relayUrl: S.Literal(PUBLIC_CHAT_RELAY_URL),
    route: S.Literal("/agentchat"),
    transport: S.Literal("nip01-websocket-with-nip29-h-filter"),
  }),
})
export type PublicChatParityFixture = typeof PublicChatParityFixture.Type

export const PUBLIC_CHAT_IMPLEMENTATION_SEAMS = {
  bridge: {
    requiredForReadOnlyOmega: false,
    reason:
      "Omega can use its Rust WebSocket client. A desktop bridge is not necessary for this public read path.",
  },
  currentWebRisks: [
    {
      behavior:
        "relayGroupAdministrators unions all retained kind 39001 events. A removed administrator can remain authorized in the web projection.",
      disposition:
        "Do not copy this behavior into Rust. Resolve current relay-state replacement rules in a separate security change.",
    },
    {
      behavior:
        "AgentChatPage starts once with a null manifest and starts again after the manifest loads.",
      disposition:
        "Do not treat the first short relay session as required parity. A later web lifecycle change can remove it.",
    },
    {
      behavior:
        "The reconnect cursor includes every retained event kind, including profile and relay-state events.",
      disposition:
        "The fixture preserves this current behavior. A cursor policy change needs a coordinated web and desktop contract revision.",
    },
  ],
  rustEquivalent: [
    {
      behavior:
        "NIP-01 relay frames, one-second replay overlap, EOSE completion, and event-ID duplicate removal",
      source: "client.ts#makePublicChatRelayClient",
    },
    {
      behavior:
        "Nostr signature checks, group checks, moderation checks, and deterministic timeline projection",
      source:
        "profile.ts#validatePublicChatEvent, profile.ts#validateRelayGroupState, parity.ts#projectPublicChatTimeline",
    },
    {
      behavior:
        "Click-to-load media states and SHA-256 verification before display",
      source:
        "parity.ts#transitionPublicChatMedia, parity.ts#verifyPublicChatMedia",
    },
  ],
  sharedTypeScript: [
    "makePublicChatRelayClient",
    "projectPublicChatTimeline",
    "parsePublicChatContent",
    "parseInlineAttachments",
    "transitionPublicChatMedia",
    "verifyPublicChatMedia",
  ],
} as const
