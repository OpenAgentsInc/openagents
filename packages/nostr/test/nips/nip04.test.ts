/**
 * Tests for NIP-04: Encrypted Direct Messages
 */

import { describe, expect, it } from "@effect/vitest"
import type { PublicKey, Signature } from "../../src/core/Schema.js"
import * as Nip04 from "../../src/nips/nip04.js"

describe("NIP-04", () => {
  const senderPubkey = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d" as PublicKey
  const recipientPubkey = "91cf9695cb99c77d5645f319b4b4d59b0b55e8907b23af0b11bb7b10dcf096d9" as PublicKey

  describe("Utility Functions", () => {
    it("should generate random conversation ID", () => {
      const id1 = Nip04.generateConversationId()
      const id2 = Nip04.generateConversationId()

      expect(id1).toHaveLength(16) // 8 bytes = 16 hex chars
      expect(id2).toHaveLength(16)
      expect(id1).not.toBe(id2)
    })

    it("should extract participants from event", () => {
      const event: Nip04.DirectMessageEvent = {
        id: "test-id",
        pubkey: senderPubkey,
        created_at: 12345,
        kind: 4,
        tags: [
          ["p", recipientPubkey],
          ["p", "another-pubkey" as PublicKey]
        ],
        content: "encrypted-content",
        sig: "test-sig" as Signature
      }

      const participants = Nip04.extractParticipants(event)
      expect(participants).toContain(senderPubkey)
      expect(participants).toContain(recipientPubkey)
      expect(participants).toContain("another-pubkey")
      expect(participants).toHaveLength(3)
    })

    it("should create thread tags", () => {
      const replyToEventId = "parent-event-id"
      const replyToAuthor = senderPubkey

      const tags = Nip04.createThreadTags(replyToEventId, replyToAuthor)
      expect(tags).toHaveLength(2)
      expect(tags[0]).toEqual(["e", replyToEventId, "", "reply"])
      expect(tags[1]).toEqual(["p", replyToAuthor])
    })

    it("should create thread tags without author", () => {
      const replyToEventId = "parent-event-id"

      const tags = Nip04.createThreadTags(replyToEventId)
      expect(tags).toHaveLength(1)
      expect(tags[0]).toEqual(["e", replyToEventId, "", "reply"])
    })

    it("should create empty thread tags", () => {
      const tags = Nip04.createThreadTags()
      expect(tags).toHaveLength(0)
    })
  })
})
