/**
 * Tests for NIP-05: DNS-based Internet Identifiers
 */

import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import * as Nip05 from "../../src/nips/nip05.js"

describe("NIP-05", () => {
  describe("identifier parsing", () => {
    it("should parse valid identifier", () => {
      const result = Effect.runSync(Nip05.parseIdentifier("alice@example.com"))
      expect(result).toEqual({
        name: "alice",
        domain: "example.com"
      })
    })

    it("should fail on invalid identifier", () => {
      const result = Effect.runSyncExit(Nip05.parseIdentifier("invalid"))
      expect(result._tag).toBe("Failure")
    })

    it("should normalize identifier", () => {
      const result = Effect.runSync(Nip05.normalizeIdentifier("ALICE@EXAMPLE.COM"))
      expect(result).toBe("alice@example.com")
    })

    it("should add _ for domain-only input", () => {
      const result = Effect.runSync(Nip05.normalizeIdentifier("example.com"))
      expect(result).toBe("_@example.com")
    })
  })

  describe("metadata event creation", () => {
    it("should create metadata with NIP-05", () => {
      const metadata = Nip05.createMetadataEvent({
        name: "Alice",
        about: "Test user",
        nip05: "alice@example.com",
        picture: "https://example.com/alice.jpg"
      })

      const parsed = JSON.parse(metadata)
      expect(parsed.nip05).toBe("alice@example.com")
      expect(parsed.name).toBe("Alice")
      expect(parsed.about).toBe("Test user")
      expect(parsed.picture).toBe("https://example.com/alice.jpg")
    })

    it("should omit undefined fields", () => {
      const metadata = Nip05.createMetadataEvent({
        nip05: "alice@example.com"
      })

      const parsed = JSON.parse(metadata)
      expect(parsed.nip05).toBe("alice@example.com")
      expect(parsed.name).toBeUndefined()
      expect(parsed.about).toBeUndefined()
    })

    it("should include Lightning address", () => {
      const metadata = Nip05.createMetadataEvent({
        nip05: "alice@example.com",
        lud16: "alice@ln.example.com"
      })

      const parsed = JSON.parse(metadata)
      expect(parsed.lud16).toBe("alice@ln.example.com")
    })
  })
})
