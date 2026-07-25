import { Schema as S } from "effect"
import { finalizeEvent, generateSecretKey } from "nostr-effect/pure"
import { describe, expect, it } from "vitest"

import {
  NostrEvent,
  PUBLIC_CHAT_GROUP_ID,
  PUBLIC_CHAT_RELAY_URL,
  groupNaddrFor,
  hasContentWarning,
  isAuthorDeletion,
  nextCursor,
  parseInlineAttachments,
  previousReferences,
  publicChatEventTemplate,
  relayResultPrefix,
  replyTagsAndContent,
  stableChronological,
  validatePublicChatEvent,
} from "./profile.js"

const secret = generateSecretKey()
const signed = (
  content: string,
  createdAt: number,
  tags: string[][] = [["h", PUBLIC_CHAT_GROUP_ID]],
) =>
  S.decodeUnknownSync(NostrEvent)(
    finalizeEvent(
      {
        content,
        created_at: createdAt,
        kind: 9,
        tags,
      },
      secret,
    ),
  )

describe("openagents.public_chat.v1", () => {
  it("uses configurable relay and group values without a private codec", () => {
    const event = S.decodeUnknownSync(NostrEvent)(
      finalizeEvent(
        publicChatEventTemplate({
          content: "portable",
          groupId: "another-public-group",
          nowSeconds: 1_000,
        }),
        secret,
      ),
    )
    expect(
      validatePublicChatEvent(event, {
        groupId: "another-public-group",
        nowSeconds: 1_000,
      }),
    ).toEqual({ ok: true })
    expect(
      groupNaddrFor("a".repeat(64), {
        groupId: "another-public-group",
        relayUrl: "wss://relay.example",
      }),
    ).toMatch(/^naddr1/)
  })

  it("validates a signed group message and rejects wrong groups and timestamps", () => {
    const current = signed("hello", 1_000)
    expect(
      validatePublicChatEvent(current, { nowSeconds: 1_000 }),
    ).toEqual({ ok: true })
    expect(
      validatePublicChatEvent(
        signed("wrong", 1_000, [["h", "another-group"]]),
        { nowSeconds: 1_000 },
      ),
    ).toEqual({ ok: false, reason: "wrong-group" })
    expect(
      validatePublicChatEvent(signed("future", 1_061), {
        nowSeconds: 1_000,
      }),
    ).toEqual({ ok: false, reason: "future" })
  })

  it("selects three previous events from the latest fifty and excludes the writer", () => {
    const events = Array.from({ length: 60 }, (_, index) =>
      signed(String(index), 1_000 + index),
    )
    expect(previousReferences(events, "f".repeat(64))).toEqual(
      events
        .slice(-3)
        .reverse()
        .map((event) => event.id.slice(0, 8)),
    )
    expect(previousReferences(events, events.at(-1)!.pubkey)).toEqual([])
  })

  it("builds NIP-C7 reply tags and a standard nevent URI", () => {
    const parent = signed("parent", 1_000)
    const reply = replyTagsAndContent({ content: "child", parent })
    expect(reply.tags).toEqual([
      ["q", parent.id, PUBLIC_CHAT_RELAY_URL, parent.pubkey],
    ])
    expect(reply.content).toMatch(/^nostr:nevent1/)
    expect(reply.content).toContain("\n\nchild")
  })

  it("keeps every same-timestamp event in a stable cursor", () => {
    const first = signed("b", 1_000)
    const second = signed("a", 1_000)
    const ordered = stableChronological([second, first, second])
    expect(ordered).toHaveLength(2)
    expect(ordered.map((event) => event.id)).toEqual(
      [...ordered.map((event) => event.id)].sort(),
    )
    expect(nextCursor(ordered)).toEqual({
      createdAt: 1_000,
      eventIdsAtCreatedAt: ordered.map((event) => event.id),
    })
  })

  it("matches only safe NIP-92 imeta records that occur in content", () => {
    const digest = "a".repeat(64)
    const event = signed(
      "photo https://cdn.example/photo.png missing https://cdn.example/file.svg",
      1_000,
      [
        ["h", PUBLIC_CHAT_GROUP_ID],
        [
          "imeta",
          "url https://cdn.example/photo.png",
          "m image/png",
          `x ${digest}`,
          "size 128",
          "alt Public diagram",
        ],
        [
          "imeta",
          "url https://cdn.example/not-in-content.png",
          "m image/png",
        ],
        [
          "imeta",
          "url https://cdn.example/file.svg",
          "m image/svg+xml",
        ],
      ],
    )
    expect(parseInlineAttachments(event)).toEqual([
      {
        alt: "Public diagram",
        digest,
        mimeType: "image/png",
        size: 128,
        url: "https://cdn.example/photo.png",
      },
    ])
  })

  it("separates author deletion from warnings and relay moderation", () => {
    const target = signed("sensitive", 1_000, [
      ["h", PUBLIC_CHAT_GROUP_ID],
      ["content-warning", "spoilers"],
    ])
    const deletion = S.decodeUnknownSync(NostrEvent)(
      finalizeEvent(
        {
          content: "removed",
          created_at: 1_001,
          kind: 5,
          tags: [
            ["h", PUBLIC_CHAT_GROUP_ID],
            ["e", target.id],
          ],
        },
        secret,
      ),
    )
    expect(hasContentWarning(target)).toBe(true)
    expect(isAuthorDeletion(deletion, target)).toBe(true)
  })

  it("preserves NIP-01 result prefixes as typed errors", () => {
    expect(relayResultPrefix("rate-limited: slow down")).toBe("rate-limited")
    expect(relayResultPrefix("unknown response")).toBe("error")
  })

  it("creates an interoperable template and group naddr", () => {
    expect(
      publicChatEventTemplate({
        content: "hello",
        nowSeconds: 1_000,
        previous: ["a".repeat(8)],
      }),
    ).toEqual({
      content: "hello",
      created_at: 1_000,
      kind: 9,
      tags: [
        ["h", PUBLIC_CHAT_GROUP_ID],
        ["previous", "a".repeat(8)],
      ],
    })
    expect(groupNaddrFor("b".repeat(64))).toMatch(/^naddr1/)
  })
})
