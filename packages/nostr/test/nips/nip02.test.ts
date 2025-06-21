/**
 * Tests for NIP-02: Contact Lists and Petname System
 */

import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import type { PublicKey } from "../../src/core/Schema.js"
import * as Nip02 from "../../src/nips/nip02.js"

describe("NIP-02", () => {
  const testPubkey1 = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d" as PublicKey
  const testPubkey2 = "44e1fdac7dd8ec1f0ee992a8e5cdd3a14ebef8e5cf5486f178f449252e548c5d" as PublicKey
  const testPubkey3 = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa" as PublicKey

  describe("Contact List Validation", () => {
    it("should validate contact list structure", () => {
      const contacts: Array<Nip02.Contact> = [
        {
          pubkey: testPubkey1,
          mainRelay: "wss://relay.example.com",
          petname: "alice"
        },
        {
          pubkey: testPubkey2,
          petname: "bob"
        }
      ]

      const result = Effect.runSync(Nip02.validateContactList(contacts))
      expect(result).toBeUndefined() // No error means validation passed
    })

    it("should detect duplicate contacts", () => {
      const contacts: Array<Nip02.Contact> = [
        {
          pubkey: testPubkey1,
          petname: "alice"
        },
        {
          pubkey: testPubkey1, // duplicate
          petname: "alice2"
        }
      ]

      const result = Effect.runSyncExit(Nip02.validateContactList(contacts))
      expect(result._tag).toBe("Failure")
    })
  })

  describe("Utility Functions", () => {
    it("should convert contacts to tags", () => {
      const contacts: Array<Nip02.Contact> = [
        {
          pubkey: testPubkey1,
          mainRelay: "wss://relay1.com",
          petname: "alice"
        },
        {
          pubkey: testPubkey2,
          petname: "bob"
        }
      ]

      const tags = Nip02.contactListToTags(contacts)
      expect(tags).toHaveLength(2)
      expect(tags[0]).toEqual(["p", testPubkey1, "wss://relay1.com", "alice"])
      expect(tags[1]).toEqual(["p", testPubkey2, "bob"])
    })

    it("should extract contacts from tags", () => {
      const tags = [
        ["p", testPubkey1, "wss://relay1.com", "alice"],
        ["p", testPubkey2, "", "bob"],
        ["other", "tag"]
      ]

      const contacts = Nip02.tagsToContactList(tags)
      expect(contacts).toHaveLength(2)
      expect(contacts[0]).toEqual({
        pubkey: testPubkey1,
        mainRelay: "wss://relay1.com",
        petname: "alice"
      })
      expect(contacts[1]).toEqual({
        pubkey: testPubkey2,
        mainRelay: undefined,
        petname: "bob"
      })
    })

    it("should merge contact lists", () => {
      const list1: Nip02.ContactList = {
        contacts: [
          { pubkey: testPubkey1, petname: "alice" },
          { pubkey: testPubkey2, petname: "bob" }
        ],
        updatedAt: 1000
      }

      const list2: Nip02.ContactList = {
        contacts: [
          { pubkey: testPubkey2, petname: "bobby", mainRelay: "wss://relay.com" },
          { pubkey: testPubkey3, petname: "charlie" }
        ],
        updatedAt: 2000
      }

      const merged = Nip02.mergeContactLists(list1, list2)
      expect(merged.contacts).toHaveLength(3)
      expect(merged.updatedAt).toBe(2000)

      // Should prefer newer entries
      const bobContact = merged.contacts.find((c) => c.pubkey === testPubkey2)
      expect(bobContact?.petname).toBe("bobby")
      expect(bobContact?.mainRelay).toBe("wss://relay.com")
    })
  })
})
