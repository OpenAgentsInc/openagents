import { Schema as S } from "effect"
import { naddrEncode, neventEncode, npubEncode } from "nostr-effect/nip19"
import { verifyEvent } from "nostr-effect/pure"

export const PUBLIC_CHAT_PROFILE = "openagents.public_chat.v1" as const
export const PUBLIC_CHAT_RICH_CONTENT_PROFILE =
  "openagents.public_chat.rich_content.v1" as const
export const PUBLIC_CHAT_MANIFEST_SCHEMA =
  "openagents.public_nostr_chat_manifest.v1" as const
export const PUBLIC_CHAT_GROUP_ID = "openagents-public" as const
export const PUBLIC_CHAT_RELAY_URL = "wss://relay.openagents.com" as const
export const PUBLIC_CHAT_RELAY_INFO_URL =
  "https://relay.openagents.com/" as const

export const PUBLIC_CHAT_ACCEPTED_KINDS = [5, 7, 9, 1337, 1984] as const
export const PUBLIC_CHAT_GROUP_STATE_KINDS = [
  39000, 39001, 39003, 39005,
] as const
export const PUBLIC_CHAT_MODERATION_KINDS = [9002, 9005, 9010] as const
export const PUBLIC_CHAT_SIGNER_KINDS = [
  5, 7, 9, 1337, 1984, 9002, 9005, 9010, 22242, 24242,
] as const

export const PUBLIC_CHAT_LIMITS = {
  attachmentCount: 4,
  attachmentBytes: 25 * 1024 * 1024,
  contentBytes: 8_192,
  eventBytes: 32_768,
  futureSkewSeconds: 60,
  historyPageSize: 50,
  maxAgeSeconds: 7 * 24 * 60 * 60,
  tags: 64,
} as const

export const Hex64 = S.String.check(
  S.isPattern(/^[0-9a-f]{64}$/),
  S.isMinLength(64),
  S.isMaxLength(64),
)
export const Hex128 = S.String.check(
  S.isPattern(/^[0-9a-f]{128}$/),
  S.isMinLength(128),
  S.isMaxLength(128),
)
export const SafeGroupId = S.String.check(
  S.isPattern(/^[a-z0-9][a-z0-9-]{0,63}$/),
)

export const NostrEvent = S.Struct({
  content: S.String,
  created_at: S.Int,
  id: Hex64,
  kind: S.Int,
  pubkey: Hex64,
  sig: Hex128,
  tags: S.Array(S.Array(S.String)),
})
export type NostrEvent = typeof NostrEvent.Type

export const PublicChatCursor = S.Struct({
  createdAt: S.Int,
  eventIdsAtCreatedAt: S.Array(Hex64),
})
export type PublicChatCursor = typeof PublicChatCursor.Type

export const PublicChatRejectionReason = S.Literals([
  "auth-required",
  "restricted",
  "duplicate",
  "invalid",
  "error",
  "blocked",
  "rate-limited",
  "pow",
  "wrong-kind",
  "wrong-group",
  "missing-previous",
  "unknown-previous",
  "content-too-large",
  "event-too-large",
  "too-many-tags",
  "stale",
  "future",
  "signature-invalid",
  "unsafe-media",
  "digest-mismatch",
  "relay-unavailable",
  "relay-self-unavailable",
  "fork-detected",
])
export type PublicChatRejectionReason =
  typeof PublicChatRejectionReason.Type

export const PublicChatValidation = S.Union([
  S.Struct({ ok: S.Literal(true) }),
  S.Struct({
    ok: S.Literal(false),
    reason: PublicChatRejectionReason,
  }),
])
export type PublicChatValidation = typeof PublicChatValidation.Type

export const PublicChatAttachment = S.Struct({
  alt: S.optionalKey(S.String),
  blurhash: S.optionalKey(S.String),
  dimensions: S.optionalKey(S.String),
  digest: S.optionalKey(Hex64),
  durationSeconds: S.optionalKey(S.Number),
  mimeType: S.String,
  size: S.optionalKey(S.Int),
  thumbnailUrl: S.optionalKey(S.String),
  url: S.String,
  waveform: S.optionalKey(S.Array(S.Number)),
})
export type PublicChatAttachment = typeof PublicChatAttachment.Type

const utf8Bytes = (value: string): number =>
  new TextEncoder().encode(value).byteLength

export const tagValue = (
  event: Pick<NostrEvent, "tags">,
  name: string,
): string | undefined => event.tags.find((tag) => tag[0] === name)?.[1]

export const tagValues = (
  event: Pick<NostrEvent, "tags">,
  name: string,
): ReadonlyArray<ReadonlyArray<string>> =>
  event.tags.filter((tag) => tag[0] === name)

export const hasGroupTag = (
  event: Pick<NostrEvent, "tags">,
  groupId: string = PUBLIC_CHAT_GROUP_ID,
): boolean =>
  event.tags.some((tag) => tag[0] === "h" && tag[1] === groupId)

export const validatePublicChatEvent = (
  event: NostrEvent,
  input: Readonly<{
    acceptedKinds?: ReadonlyArray<number>
    groupId?: string
    nowSeconds?: number
    knownPreviousIds?: ReadonlySet<string>
    requirePrevious?: boolean
  }> = {},
): PublicChatValidation => {
  const nowSeconds = input.nowSeconds ?? Math.floor(Date.now() / 1_000)
  const acceptedKinds: ReadonlyArray<number> =
    input.acceptedKinds ?? PUBLIC_CHAT_ACCEPTED_KINDS
  if (!acceptedKinds.includes(event.kind)) {
    return { ok: false, reason: "wrong-kind" }
  }
  if (!hasGroupTag(event, input.groupId ?? PUBLIC_CHAT_GROUP_ID)) {
    return { ok: false, reason: "wrong-group" }
  }
  if (event.tags.length > PUBLIC_CHAT_LIMITS.tags) {
    return { ok: false, reason: "too-many-tags" }
  }
  if (utf8Bytes(event.content) > PUBLIC_CHAT_LIMITS.contentBytes) {
    return { ok: false, reason: "content-too-large" }
  }
  if (utf8Bytes(JSON.stringify(event)) > PUBLIC_CHAT_LIMITS.eventBytes) {
    return { ok: false, reason: "event-too-large" }
  }
  if (event.created_at > nowSeconds + PUBLIC_CHAT_LIMITS.futureSkewSeconds) {
    return { ok: false, reason: "future" }
  }
  if (event.created_at < nowSeconds - PUBLIC_CHAT_LIMITS.maxAgeSeconds) {
    return { ok: false, reason: "stale" }
  }
  if (
    !verifyEvent({
      ...event,
      tags: event.tags.map((tag) => [...tag]),
    })
  ) {
    return { ok: false, reason: "signature-invalid" }
  }

  const previous = tagValues(event, "previous").flatMap((tag) =>
    tag.slice(1).filter((value) => /^[0-9a-f]{8}$/.test(value)),
  )
  if (input.requirePrevious === true && previous.length === 0) {
    return { ok: false, reason: "missing-previous" }
  }
  if (
    input.knownPreviousIds !== undefined &&
    previous.some(
      (prefix) =>
        ![...(input.knownPreviousIds ?? [])].some((id) =>
          id.startsWith(prefix),
        ),
    )
  ) {
    return { ok: false, reason: "unknown-previous" }
  }
  return { ok: true }
}

export const validateRelayGroupState = (
  event: NostrEvent,
  relaySelfPubkey: string,
  groupId: string = PUBLIC_CHAT_GROUP_ID,
  groupStateKinds: ReadonlyArray<number> = PUBLIC_CHAT_GROUP_STATE_KINDS,
): PublicChatValidation => {
  if (
    !groupStateKinds.includes(event.kind) ||
    event.pubkey !== relaySelfPubkey ||
    !event.tags.some(
      (tag) => tag[0] === "d" && tag[1] === groupId,
    )
  ) {
    return { ok: false, reason: "relay-self-unavailable" }
  }
  if (
    !verifyEvent({
      ...event,
      tags: event.tags.map((tag) => [...tag]),
    })
  ) {
    return { ok: false, reason: "signature-invalid" }
  }
  return { ok: true }
}

export const validateModerationCommand = (
  event: NostrEvent,
  input: Readonly<{
    adminPubkeys: ReadonlySet<string>
    groupId?: string
  }>,
): PublicChatValidation => {
  if (
    !PUBLIC_CHAT_MODERATION_KINDS.includes(event.kind as never) ||
    !input.adminPubkeys.has(event.pubkey) ||
    !hasGroupTag(event, input.groupId ?? PUBLIC_CHAT_GROUP_ID)
  ) {
    return { ok: false, reason: "restricted" }
  }
  return verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })
    ? { ok: true }
    : { ok: false, reason: "signature-invalid" }
}

export const relayGroupAdministrators = (
  events: ReadonlyArray<NostrEvent>,
): ReadonlySet<string> =>
  new Set(
    events
      .filter((event) => event.kind === 39001)
      .flatMap((event) =>
        event.tags
          .filter((tag) => tag[0] === "p" && /^[0-9a-f]{64}$/.test(tag[1] ?? ""))
          .map((tag) => tag[1]!),
      ),
  )

export const previousReferences = (
  events: ReadonlyArray<NostrEvent>,
  ownPubkey: string,
): ReadonlyArray<string> =>
  [...events]
    .filter((event) => event.pubkey !== ownPubkey)
    .sort(
      (left, right) =>
        right.created_at - left.created_at || right.id.localeCompare(left.id),
    )
    .slice(0, 50)
    .slice(0, 3)
    .map((event) => event.id.slice(0, 8))

export const publicChatEventTemplate = (input: Readonly<{
  content: string
  groupId?: string
  kind?: 5 | 7 | 9 | 1337 | 1984 | 9002 | 9005 | 9010
  nowSeconds?: number
  tags?: ReadonlyArray<ReadonlyArray<string>>
  previous?: ReadonlyArray<string>
}>): Readonly<{
  content: string
  created_at: number
  kind: number
  tags: string[][]
}> => ({
  content: input.content,
  created_at: input.nowSeconds ?? Math.floor(Date.now() / 1_000),
  kind: input.kind ?? 9,
  tags: [
    ["h", input.groupId ?? PUBLIC_CHAT_GROUP_ID],
    ...(input.previous === undefined || input.previous.length === 0
      ? []
      : [["previous", ...input.previous]]),
    ...(input.tags ?? []).map((tag) => [...tag]),
  ],
})

export const replyTagsAndContent = (input: Readonly<{
  content: string
  parent: NostrEvent
  relayUrl?: string
}>): Readonly<{ content: string; tags: string[][] }> => {
  const relayUrl = input.relayUrl ?? PUBLIC_CHAT_RELAY_URL
  const reference = `nostr:${neventEncode({
    id: input.parent.id,
    author: input.parent.pubkey,
    kind: input.parent.kind,
    relays: [relayUrl],
  })}`
  return {
    content: `${reference}\n\n${input.content}`,
    tags: [["q", input.parent.id, relayUrl, input.parent.pubkey]],
  }
}

export const npubFor = (pubkey: string): string => npubEncode(pubkey)

export const groupNaddrFor = (
  relaySelfPubkey: string,
  input: Readonly<{ groupId?: string; relayUrl?: string }> = {},
): string =>
  naddrEncode({
    identifier: input.groupId ?? PUBLIC_CHAT_GROUP_ID,
    kind: 39000,
    pubkey: relaySelfPubkey,
    relays: [input.relayUrl ?? PUBLIC_CHAT_RELAY_URL],
  })

export const relayResultPrefix = (
  message: string,
): PublicChatRejectionReason => {
  const prefix = message.trim().split(":")[0]?.toLowerCase()
  return S.is(PublicChatRejectionReason)(prefix) ? prefix : "error"
}

export const stableChronological = (
  events: ReadonlyArray<NostrEvent>,
): ReadonlyArray<NostrEvent> =>
  [...new Map(events.map((event) => [event.id, event])).values()].sort(
    (left, right) =>
      left.created_at - right.created_at || left.id.localeCompare(right.id),
  )

export const nextCursor = (
  events: ReadonlyArray<NostrEvent>,
): PublicChatCursor | undefined => {
  const ordered = stableChronological(events)
  const last = ordered.at(-1)
  if (last === undefined) return undefined
  return {
    createdAt: last.created_at,
    eventIdsAtCreatedAt: ordered
      .filter((event) => event.created_at === last.created_at)
      .map((event) => event.id),
  }
}

const safeMediaMime =
  /^(?:image\/(?:avif|gif|jpeg|png|webp)|audio\/(?:aac|flac|mpeg|mp4|ogg|wav|webm)|video\/(?:mp4|ogg|webm)|application\/(?:json|pdf)|text\/(?:csv|plain))$/i

export const isSafeHttpUrl = (value: string): boolean => {
  try {
    const url = new URL(value)
    return url.protocol === "https:" || url.protocol === "http:"
  } catch {
    return false
  }
}

export const parseInlineAttachments = (
  event: NostrEvent,
): ReadonlyArray<PublicChatAttachment> => {
  const contentUrls = new Set(
    event.content
      .split(/\s+/)
      .filter(isSafeHttpUrl)
      .map((value) => value.replace(/[),.;!?]+$/, "")),
  )
  return tagValues(event, "imeta")
    .map((tag) =>
      Object.fromEntries(
        tag.slice(1).map((field) => {
          const separator = field.indexOf(" ")
          return separator === -1
            ? [field, ""]
            : [field.slice(0, separator), field.slice(separator + 1)]
        }),
      ),
    )
    .flatMap((fields) => {
      const url = fields.url
      const mimeType = fields.m
      if (
        url === undefined ||
        mimeType === undefined ||
        !contentUrls.has(url) ||
        !safeMediaMime.test(mimeType)
      ) {
        return []
      }
      const size = fields.size === undefined ? undefined : Number(fields.size)
      if (
        size !== undefined &&
        (!Number.isSafeInteger(size) ||
          size < 0 ||
          size > PUBLIC_CHAT_LIMITS.attachmentBytes)
      ) {
        return []
      }
      return [
        {
          ...(fields.alt === undefined ? {} : { alt: fields.alt }),
          ...(fields.blurhash === undefined
            ? {}
            : { blurhash: fields.blurhash }),
          ...(fields.dim === undefined ? {} : { dimensions: fields.dim }),
          ...(fields.duration === undefined
            ? {}
            : { durationSeconds: Number(fields.duration) }),
          ...(size === undefined ? {} : { size }),
          ...(fields.thumb === undefined
            ? {}
            : { thumbnailUrl: fields.thumb }),
          ...(fields.waveform === undefined
            ? {}
            : {
                waveform: fields.waveform
                  .split(" ")
                  .map(Number)
                  .filter(Number.isFinite)
                  .slice(0, 256),
              }),
          ...(fields.x === undefined || !/^[0-9a-f]{64}$/.test(fields.x)
            ? {}
            : { digest: fields.x }),
          mimeType,
          url,
        },
      ]
    })
    .slice(0, PUBLIC_CHAT_LIMITS.attachmentCount)
}

export const hasContentWarning = (event: NostrEvent): boolean =>
  event.tags.some((tag) => tag[0] === "content-warning")

export const isAuthorDeletion = (
  deletion: NostrEvent,
  target: NostrEvent,
): boolean =>
  deletion.kind === 5 &&
  deletion.pubkey === target.pubkey &&
  hasGroupTag(deletion) &&
  deletion.tags.some((tag) => tag[0] === "e" && tag[1] === target.id)
