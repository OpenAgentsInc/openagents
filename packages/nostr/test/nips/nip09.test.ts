/**
 * Tests for NIP-09: Event Deletion
 */

import { describe, expect, it } from "@effect/vitest"
import { Effect, Exit } from "effect"
import type { PublicKey, Signature } from "../../src/core/Schema.js"
import * as Nip09 from "../../src/nips/nip09.js"

describe("NIP-09", () => {
  const authorPubkey = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d" as PublicKey

  describe("Utility Functions", () => {
    it("should extract event IDs from deletion event", () => {
      const event: Nip09.DeletionEvent = {
        id: "deletion-event",
        pubkey: authorPubkey,
        created_at: 12345,
        kind: 5,
        tags: [
          ["e", "event1"],
          ["e", "event2"],
          ["p", "some-pubkey"],
          ["e", "event3"]
        ],
        content: "Deletion reason",
        sig: "sig" as Signature
      }

      const eventIds = Nip09.extractEventIds(event)
      expect(eventIds).toEqual(["event1", "event2", "event3"])
    })

    it("should extract deletion reason from event", () => {
      const event: Nip09.DeletionEvent = {
        id: "deletion-event",
        pubkey: authorPubkey,
        created_at: 12345,
        kind: 5,
        tags: [
          ["e", "event1"],
          ["reason", "spam"]
        ],
        content: "",
        sig: "sig" as Signature
      }

      const reason = Nip09.extractDeletionReason(event)
      expect(reason).toBe("spam")
    })

    it("should return undefined for missing reason", () => {
      const event: Nip09.DeletionEvent = {
        id: "deletion-event",
        pubkey: authorPubkey,
        created_at: 12345,
        kind: 5,
        tags: [["e", "event1"]],
        content: "",
        sig: "sig" as Signature
      }

      const reason = Nip09.extractDeletionReason(event)
      expect(reason).toBeUndefined()
    })

    it("should check if deletion is within time limit", () => {
      const eventCreatedAt = 1000
      const deletionRequestedAt = 1500
      const timeLimit = 600 // 10 minutes

      expect(Nip09.isWithinTimeLimit(eventCreatedAt, deletionRequestedAt, timeLimit)).toBe(true)
      expect(Nip09.isWithinTimeLimit(eventCreatedAt, deletionRequestedAt + 200, timeLimit)).toBe(false)
    })

    it("should format deletion reason", () => {
      expect(Nip09.formatDeletionReason("spam")).toBe("Spam")
      expect(Nip09.formatDeletionReason("inappropriate_content")).toBe("Inappropriate Content")
      expect(Nip09.formatDeletionReason("personal_request")).toBe("Personal Request")
      expect(Nip09.formatDeletionReason("legal_requirement")).toBe("Legal Requirement")
      expect(Nip09.formatDeletionReason("duplicate")).toBe("Duplicate Content")
      expect(Nip09.formatDeletionReason("error")).toBe("Posted in Error")
      expect(Nip09.formatDeletionReason("other")).toBe("Other")
    })

    it("should create single event deletion request", () => {
      const request = Nip09.createSingleEventDeletion("event-id", "spam", "This is spam")
      expect(request).toEqual({
        eventIds: ["event-id"],
        reason: "spam",
        comment: "This is spam"
      })
    })

    it("should create bulk deletion request", () => {
      const eventIds = ["event1", "event2", "event3"]
      const request = Nip09.createBulkDeletion(eventIds, "duplicate")
      expect(request).toEqual({
        eventIds,
        reason: "duplicate",
        comment: undefined
      })
    })

    it("should create default deletion policy", () => {
      const policy = Nip09.createDefaultDeletionPolicy()
      expect(policy.allowSelfDeletion).toBe(true)
      expect(policy.allowModeratorDeletion).toBe(true)
      expect(policy.preserveArchive).toBe(true)
      expect(policy.timeLimit).toBe(24 * 60 * 60) // 24 hours
      expect(policy.moderatorPubkeys).toEqual([])
    })

    it("should create restrictive deletion policy", () => {
      const policy = Nip09.createRestrictiveDeletionPolicy()
      expect(policy.allowSelfDeletion).toBe(false)
      expect(policy.allowModeratorDeletion).toBe(true)
      expect(policy.preserveArchive).toBe(true)
      expect(policy.timeLimit).toBe(60 * 60) // 1 hour
      expect(policy.moderatorPubkeys).toEqual([])
    })

    it("should validate deletion event structure", () => {
      const validEvent: Nip09.DeletionEvent = {
        id: "valid-id",
        pubkey: authorPubkey,
        created_at: 12345,
        kind: 5,
        tags: [["e", "a".repeat(64)]],
        content: "",
        sig: "sig" as Signature
      }

      const result = Effect.runSyncExit(Nip09.validateDeletionEventStructure(validEvent))
      expect(Exit.isSuccess(result)).toBe(true)
    })

    it("should reject deletion event with wrong kind", () => {
      const invalidEvent: any = {
        id: "invalid-id",
        pubkey: authorPubkey,
        created_at: 12345,
        kind: 4, // Wrong kind
        tags: [["e", "event-id"]],
        content: "",
        sig: "sig"
      }

      const result = Effect.runSyncExit(Nip09.validateDeletionEventStructure(invalidEvent))
      expect(Exit.isFailure(result)).toBe(true)
    })

    it("should reject deletion event without event references", () => {
      const invalidEvent: Nip09.DeletionEvent = {
        id: "invalid-id",
        pubkey: authorPubkey,
        created_at: 12345,
        kind: 5,
        tags: [["p", "pubkey"]], // No e-tags
        content: "",
        sig: "sig" as Signature
      }

      const result = Effect.runSyncExit(Nip09.validateDeletionEventStructure(invalidEvent))
      expect(Exit.isFailure(result)).toBe(true)
    })
  })
})
