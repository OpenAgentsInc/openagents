/**
 * Tests for NIP-19: bech32-encoded entities
 */

import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import * as Nip19 from "../../src/nips/nip19.js"

describe("NIP-19", () => {
  // Test vectors from NIP-19 specification
  const testPubkey = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
  const testPrivkey = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa"
  const testEventId = "44e1fdac7dd8ec1f0ee992a8e5cdd3a14ebef8e5cf5486f178f449252e548c5d"

  describe("npub encoding/decoding", () => {
    it("should encode a public key to npub", () => {
      const result = Effect.runSync(Nip19.npubEncode(testPubkey))
      expect(result).toMatch(/^npub1/)
      expect(result).toBe("npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6")
    })

    it("should decode an npub to public key", () => {
      const npub = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6" as Nip19.Npub
      const result = Effect.runSync(Nip19.npubDecode(npub))
      expect(result).toBe(testPubkey)
    })

    it("should fail with invalid npub", () => {
      const npub = "npub1invalid" as Nip19.Npub
      const result = Effect.runSyncExit(Nip19.npubDecode(npub))
      expect(result._tag).toBe("Failure")
    })
  })

  describe("nsec encoding/decoding", () => {
    it("should encode a private key to nsec", () => {
      const result = Effect.runSync(Nip19.nsecEncode(testPrivkey))
      expect(result).toMatch(/^nsec1/)
    })

    it("should decode an nsec to private key", () => {
      const nsec = Effect.runSync(Nip19.nsecEncode(testPrivkey))
      const result = Effect.runSync(Nip19.nsecDecode(nsec))
      expect(result).toBe(testPrivkey)
    })
  })

  describe("note encoding/decoding", () => {
    it("should encode an event ID to note", () => {
      const result = Effect.runSync(Nip19.noteEncode(testEventId))
      expect(result).toMatch(/^note1/)
    })

    it("should decode a note to event ID", () => {
      const note = Effect.runSync(Nip19.noteEncode(testEventId))
      const result = Effect.runSync(Nip19.noteDecode(note))
      expect(result).toBe(testEventId)
    })
  })

  describe("nprofile encoding/decoding", () => {
    it("should encode a profile pointer", () => {
      const profile: Nip19.ProfilePointer = {
        pubkey: testPubkey,
        relays: ["wss://relay.example.com", "wss://relay2.example.com"]
      }
      const result = Effect.runSync(Nip19.nprofileEncode(profile))
      expect(result).toMatch(/^nprofile1/)
    })

    it("should decode an nprofile", () => {
      const profile: Nip19.ProfilePointer = {
        pubkey: testPubkey,
        relays: ["wss://relay.example.com", "wss://relay2.example.com"]
      }
      const nprofile = Effect.runSync(Nip19.nprofileEncode(profile))
      const result = Effect.runSync(Nip19.nprofileDecode(nprofile))
      expect(result.pubkey).toBe(testPubkey)
      expect(result.relays).toEqual(profile.relays)
    })

    it("should encode profile without relays", () => {
      const profile: Nip19.ProfilePointer = {
        pubkey: testPubkey
      }
      const nprofile = Effect.runSync(Nip19.nprofileEncode(profile))
      const result = Effect.runSync(Nip19.nprofileDecode(nprofile))
      expect(result.pubkey).toBe(testPubkey)
      expect(result.relays).toBeUndefined()
    })
  })

  describe("nevent encoding/decoding", () => {
    it("should encode an event pointer with all fields", () => {
      const event: Nip19.EventPointer = {
        id: testEventId,
        relays: ["wss://relay.example.com"],
        author: testPubkey,
        kind: 1
      }
      const result = Effect.runSync(Nip19.neventEncode(event))
      expect(result).toMatch(/^nevent1/)
    })

    it("should decode an nevent", () => {
      const event: Nip19.EventPointer = {
        id: testEventId,
        relays: ["wss://relay.example.com"],
        author: testPubkey,
        kind: 1
      }
      const nevent = Effect.runSync(Nip19.neventEncode(event))
      const result = Effect.runSync(Nip19.neventDecode(nevent))
      expect(result.id).toBe(testEventId)
      expect(result.relays).toEqual(event.relays)
      expect(result.author).toBe(testPubkey)
      expect(result.kind).toBe(1)
    })

    it("should encode event with minimal fields", () => {
      const event: Nip19.EventPointer = {
        id: testEventId
      }
      const nevent = Effect.runSync(Nip19.neventEncode(event))
      const result = Effect.runSync(Nip19.neventDecode(nevent))
      expect(result.id).toBe(testEventId)
      expect(result.relays).toBeUndefined()
      expect(result.author).toBeUndefined()
      expect(result.kind).toBeUndefined()
    })
  })

  describe("naddr encoding/decoding", () => {
    it("should encode an address pointer", () => {
      const addr: Nip19.AddressPointer = {
        identifier: "my-article",
        pubkey: testPubkey,
        kind: 30023,
        relays: ["wss://relay.example.com"]
      }
      const result = Effect.runSync(Nip19.naddrEncode(addr))
      expect(result).toMatch(/^naddr1/)
    })

    it("should decode an naddr", () => {
      const addr: Nip19.AddressPointer = {
        identifier: "my-article",
        pubkey: testPubkey,
        kind: 30023,
        relays: ["wss://relay.example.com"]
      }
      const naddr = Effect.runSync(Nip19.naddrEncode(addr))
      const result = Effect.runSync(Nip19.naddrDecode(naddr))
      expect(result.identifier).toBe("my-article")
      expect(result.pubkey).toBe(testPubkey)
      expect(result.kind).toBe(30023)
      expect(result.relays).toEqual(addr.relays)
    })
  })

  describe("nrelay encoding/decoding", () => {
    it("should encode a relay URL", () => {
      const url = "wss://relay.example.com"
      const result = Effect.runSync(Nip19.nrelayEncode(url))
      expect(result).toMatch(/^nrelay1/)
    })

    it("should decode an nrelay", () => {
      const url = "wss://relay.example.com"
      const nrelay = Effect.runSync(Nip19.nrelayEncode(url))
      const result = Effect.runSync(Nip19.nrelayDecode(nrelay))
      expect(result).toBe(url)
    })
  })

  describe("generic decode", () => {
    it("should decode any valid bech32 entity", () => {
      const npub = Effect.runSync(Nip19.npubEncode(testPubkey))
      const result = Effect.runSync(Nip19.decode(npub))
      expect(result.type).toBe("npub")
      expect(result.data).toBe(testPubkey)
    })

    it("should decode nprofile", () => {
      const profile: Nip19.ProfilePointer = {
        pubkey: testPubkey,
        relays: ["wss://relay.example.com"]
      }
      const nprofile = Effect.runSync(Nip19.nprofileEncode(profile))
      const result = Effect.runSync(Nip19.decode(nprofile))
      expect(result.type).toBe("nprofile")
      if (result.type === "nprofile") {
        expect(result.data.pubkey).toBe(testPubkey)
        expect(result.data.relays).toEqual(profile.relays)
      }
    })

    it("should fail with invalid bech32", () => {
      const result = Effect.runSyncExit(Nip19.decode("invalid"))
      expect(result._tag).toBe("Failure")
    })

    it("should fail with unknown prefix", () => {
      const result = Effect.runSyncExit(Nip19.decode("nunknown1qqqqqqq"))
      expect(result._tag).toBe("Failure")
    })
  })

  describe("edge cases", () => {
    it("should handle empty relay lists", () => {
      const profile: Nip19.ProfilePointer = {
        pubkey: testPubkey,
        relays: []
      }
      const nprofile = Effect.runSync(Nip19.nprofileEncode(profile))
      const result = Effect.runSync(Nip19.nprofileDecode(nprofile))
      expect(result.relays).toBeUndefined() // Empty array becomes undefined
    })

    it("should handle very long relay URLs", () => {
      const longUrl = "wss://" + "a".repeat(100) + ".example.com"
      const profile: Nip19.ProfilePointer = {
        pubkey: testPubkey,
        relays: [longUrl]
      }
      const nprofile = Effect.runSync(Nip19.nprofileEncode(profile))
      const result = Effect.runSync(Nip19.nprofileDecode(nprofile))
      expect(result.relays?.[0]).toBe(longUrl)
    })

    it("should handle unicode in identifiers", () => {
      const addr: Nip19.AddressPointer = {
        identifier: "测试文章🚀",
        pubkey: testPubkey,
        kind: 30023
      }
      const naddr = Effect.runSync(Nip19.naddrEncode(addr))
      const result = Effect.runSync(Nip19.naddrDecode(naddr))
      expect(result.identifier).toBe("测试文章🚀")
    })
  })
})
