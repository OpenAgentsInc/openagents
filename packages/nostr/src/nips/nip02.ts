/**
 * NIP-02: Contact Lists and Petname System
 * Implements following/contact lists and the petname system for Nostr
 *
 * Contact Lists (kind 3):
 * - Stores a user's contact list as JSON in the content field
 * - Each contact has a public key and optional relay URL
 * - Uses p-tags for efficient querying
 *
 * Petname System:
 * - Maps public keys to human-readable names
 * - Local naming system independent of global usernames
 * - Supports hierarchical name resolution
 */

import { Cache, Context, Data, Duration, Effect, Layer, Schema } from "effect"
import type { PrivateKey } from "../core/Schema.js"
import { PublicKey, Signature } from "../core/Schema.js"

// --- Types ---
export const Contact = Schema.Struct({
  pubkey: PublicKey,
  mainRelay: Schema.optional(Schema.String),
  petname: Schema.optional(Schema.String)
})
export type Contact = Schema.Schema.Type<typeof Contact>

export const ContactList = Schema.Struct({
  contacts: Schema.Array(Contact),
  updatedAt: Schema.Number
})
export type ContactList = Schema.Schema.Type<typeof ContactList>

export const ContactEvent = Schema.Struct({
  id: Schema.String,
  pubkey: PublicKey,
  created_at: Schema.Number,
  kind: Schema.Literal(3),
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Signature
})
export type ContactEvent = Schema.Schema.Type<typeof ContactEvent>

export const PetnameEntry = Schema.Struct({
  pubkey: PublicKey,
  petname: Schema.String,
  displayName: Schema.optional(Schema.String),
  about: Schema.optional(Schema.String),
  picture: Schema.optional(Schema.String),
  lastUpdated: Schema.Number
})
export type PetnameEntry = Schema.Schema.Type<typeof PetnameEntry>

// --- Errors ---
export class Nip02Error extends Data.TaggedError("Nip02Error")<{
  reason:
    | "invalid_contact_list"
    | "contact_not_found"
    | "duplicate_contact"
    | "invalid_petname"
    | "encode_failed"
    | "decode_failed"
  message: string
  pubkey?: PublicKey
  petname?: string
  cause?: unknown
}> {}

// --- Service ---
export class Nip02Service extends Context.Tag("nips/Nip02Service")<
  Nip02Service,
  {
    /**
     * Create a contact list event (kind 3)
     */
    readonly createContactList: (
      contacts: Array<Contact>,
      privateKey: PrivateKey
    ) => Effect.Effect<ContactEvent, Nip02Error>

    /**
     * Parse a contact list event
     */
    readonly parseContactList: (
      event: ContactEvent
    ) => Effect.Effect<ContactList, Nip02Error>

    /**
     * Add a contact to the list
     */
    readonly addContact: (
      contactList: ContactList,
      contact: Contact
    ) => Effect.Effect<ContactList, Nip02Error>

    /**
     * Remove a contact from the list
     */
    readonly removeContact: (
      contactList: ContactList,
      pubkey: PublicKey
    ) => Effect.Effect<ContactList, Nip02Error>

    /**
     * Update contact information
     */
    readonly updateContact: (
      contactList: ContactList,
      pubkey: PublicKey,
      updates: Partial<Omit<Contact, "pubkey">>
    ) => Effect.Effect<ContactList, Nip02Error>

    /**
     * Find a contact by public key
     */
    readonly findContact: (
      contactList: ContactList,
      pubkey: PublicKey
    ) => Effect.Effect<Contact, Nip02Error>

    /**
     * Get contacts by petname pattern
     */
    readonly findContactsByPetname: (
      contactList: ContactList,
      pattern: string
    ) => Effect.Effect<Array<Contact>, Nip02Error>

    /**
     * Set a petname for a public key
     */
    readonly setPetname: (
      pubkey: PublicKey,
      petname: string
    ) => Effect.Effect<PetnameEntry, Nip02Error>

    /**
     * Get petname for a public key
     */
    readonly getPetname: (
      pubkey: PublicKey
    ) => Effect.Effect<string | undefined, Nip02Error>

    /**
     * Resolve a petname to a public key
     */
    readonly resolvePetname: (
      petname: string
    ) => Effect.Effect<PublicKey | undefined, Nip02Error>

    /**
     * Get all petname entries
     */
    readonly getAllPetnames: () => Effect.Effect<Array<PetnameEntry>, Nip02Error>

    /**
     * Clear petname cache
     */
    readonly clearPetnameCache: () => Effect.Effect<void>
  }
>() {}

// --- Implementation ---
export const Nip02ServiceLive = Layer.effect(
  Nip02Service,
  Effect.gen(function*() {
    // Petname cache with 24-hour TTL
    const petnameCache = yield* Cache.make({
      capacity: 10000,
      timeToLive: Duration.hours(24),
      lookup: (_pubkey: PublicKey) => Effect.succeed(undefined as string | undefined)
    })

    // Reverse petname lookup cache
    const reversePetnameCache = yield* Cache.make({
      capacity: 10000,
      timeToLive: Duration.hours(24),
      lookup: (_petname: string) => Effect.succeed(undefined as PublicKey | undefined)
    })

    const createContactList = (
      contacts: Array<Contact>,
      _privateKey: PrivateKey
    ): Effect.Effect<ContactEvent, Nip02Error> =>
      Effect.sync(() => {
        // Create JSON content with contact list
        const contactsJson = JSON.stringify(
          contacts.reduce((acc, contact) => {
            acc[contact.pubkey] = {
              relay: contact.mainRelay ?? undefined,
              petname: contact.petname ?? undefined
            }
            return acc
          }, {} as Record<string, { relay?: string | undefined; petname?: string | undefined }>)
        )

        // Create p-tags for each contact for efficient querying
        const tags: Array<Array<string>> = contacts.map((contact) => [
          "p",
          contact.pubkey,
          ...(contact.mainRelay ? [contact.mainRelay] : []),
          ...(contact.petname ? [contact.petname] : [])
        ])

        const now = Math.floor(Date.now() / 1000)

        // TODO: Implement proper event signing with privateKey
        // For now, create a placeholder event
        const event: ContactEvent = {
          id: "placeholder-id",
          pubkey: "placeholder-pubkey" as PublicKey,
          created_at: now,
          kind: 3,
          tags,
          content: contactsJson,
          sig: "placeholder-signature" as Signature
        }

        return event
      })

    const parseContactList = (
      event: ContactEvent
    ): Effect.Effect<ContactList, Nip02Error> =>
      Effect.gen(function*() {
        try {
          const contactsData = JSON.parse(event.content)
          const contacts: Array<Contact> = []

          for (const [pubkey, data] of Object.entries(contactsData)) {
            if (typeof data === "object" && data !== null) {
              const contactData = data as { relay?: string; petname?: string }
              contacts.push({
                pubkey: pubkey as PublicKey,
                mainRelay: contactData.relay,
                petname: contactData.petname
              })
            }
          }

          return {
            contacts,
            updatedAt: event.created_at
          }
        } catch (error) {
          return yield* Effect.fail(
            new Nip02Error({
              reason: "decode_failed",
              message: `Failed to parse contact list: ${error}`,
              cause: error
            })
          )
        }
      })

    const addContact = (
      contactList: ContactList,
      contact: Contact
    ): Effect.Effect<ContactList, Nip02Error> =>
      Effect.gen(function*() {
        // Check for duplicate contact
        const existingIndex = contactList.contacts.findIndex(
          (c) => c.pubkey === contact.pubkey
        )

        if (existingIndex !== -1) {
          return yield* Effect.fail(
            new Nip02Error({
              reason: "duplicate_contact",
              message: `Contact already exists: ${contact.pubkey}`,
              pubkey: contact.pubkey
            })
          )
        }

        return {
          contacts: [...contactList.contacts, contact],
          updatedAt: Math.floor(Date.now() / 1000)
        }
      })

    const removeContact = (
      contactList: ContactList,
      pubkey: PublicKey
    ): Effect.Effect<ContactList, Nip02Error> =>
      Effect.gen(function*() {
        const filtered = contactList.contacts.filter((c) => c.pubkey !== pubkey)

        if (filtered.length === contactList.contacts.length) {
          return yield* Effect.fail(
            new Nip02Error({
              reason: "contact_not_found",
              message: `Contact not found: ${pubkey}`,
              pubkey
            })
          )
        }

        return {
          contacts: filtered,
          updatedAt: Math.floor(Date.now() / 1000)
        }
      })

    const updateContact = (
      contactList: ContactList,
      pubkey: PublicKey,
      updates: Partial<Omit<Contact, "pubkey">>
    ): Effect.Effect<ContactList, Nip02Error> =>
      Effect.gen(function*() {
        const contactIndex = contactList.contacts.findIndex(
          (c) => c.pubkey === pubkey
        )

        if (contactIndex === -1) {
          return yield* Effect.fail(
            new Nip02Error({
              reason: "contact_not_found",
              message: `Contact not found: ${pubkey}`,
              pubkey
            })
          )
        }

        const updatedContacts = [...contactList.contacts]
        updatedContacts[contactIndex] = {
          ...updatedContacts[contactIndex],
          ...updates
        }

        return {
          contacts: updatedContacts,
          updatedAt: Math.floor(Date.now() / 1000)
        }
      })

    const findContact = (
      contactList: ContactList,
      pubkey: PublicKey
    ): Effect.Effect<Contact, Nip02Error> =>
      Effect.gen(function*() {
        const contact = contactList.contacts.find((c) => c.pubkey === pubkey)

        if (!contact) {
          return yield* Effect.fail(
            new Nip02Error({
              reason: "contact_not_found",
              message: `Contact not found: ${pubkey}`,
              pubkey
            })
          )
        }

        return contact
      })

    const findContactsByPetname = (
      contactList: ContactList,
      pattern: string
    ): Effect.Effect<Array<Contact>, Nip02Error> =>
      Effect.succeed(
        contactList.contacts.filter((contact) => contact.petname?.toLowerCase().includes(pattern.toLowerCase()))
      )

    const setPetname = (
      pubkey: PublicKey,
      petname: string
    ): Effect.Effect<PetnameEntry, Nip02Error> =>
      Effect.gen(function*() {
        // Validate petname format
        if (!petname.trim() || petname.length > 50) {
          return yield* Effect.fail(
            new Nip02Error({
              reason: "invalid_petname",
              message: "Petname must be 1-50 characters",
              petname
            })
          )
        }

        const entry: PetnameEntry = {
          pubkey,
          petname: petname.trim(),
          lastUpdated: Date.now()
        }

        // Update caches
        yield* petnameCache.set(pubkey, petname)
        yield* reversePetnameCache.set(petname, pubkey)

        return entry
      })

    const getPetname = (pubkey: PublicKey): Effect.Effect<string | undefined, Nip02Error> => petnameCache.get(pubkey)

    const resolvePetname = (petname: string): Effect.Effect<PublicKey | undefined, Nip02Error> =>
      reversePetnameCache.get(petname)

    const getAllPetnames = (): Effect.Effect<Array<PetnameEntry>, Nip02Error> => Effect.succeed([]) // TODO: Implement proper storage retrieval

    const clearPetnameCache = (): Effect.Effect<void, never, never> =>
      Effect.gen(function*() {
        yield* petnameCache.invalidateAll
        yield* reversePetnameCache.invalidateAll
      })

    return {
      createContactList,
      parseContactList,
      addContact,
      removeContact,
      updateContact,
      findContact,
      findContactsByPetname,
      setPetname,
      getPetname,
      resolvePetname,
      getAllPetnames,
      clearPetnameCache
    }
  })
)

// --- Utility Functions ---

/**
 * Validate contact list structure
 */
export const validateContactList = (
  contacts: Array<Contact>
): Effect.Effect<void, Nip02Error> =>
  Effect.gen(function*() {
    const pubkeys = new Set<PublicKey>()

    for (const contact of contacts) {
      if (pubkeys.has(contact.pubkey)) {
        return yield* Effect.fail(
          new Nip02Error({
            reason: "duplicate_contact",
            message: `Duplicate contact: ${contact.pubkey}`,
            pubkey: contact.pubkey
          })
        )
      }
      pubkeys.add(contact.pubkey)
    }
  })

/**
 * Convert contact list to p-tags for events
 */
export const contactListToTags = (contacts: Array<Contact>): Array<Array<string>> =>
  contacts.map((contact) => [
    "p",
    contact.pubkey,
    ...(contact.mainRelay ? [contact.mainRelay] : []),
    ...(contact.petname ? [contact.petname] : [])
  ])

/**
 * Extract contacts from p-tags
 */
export const tagsToContactList = (tags: Array<Array<string>>): Array<Contact> =>
  tags
    .filter((tag) => tag[0] === "p" && tag[1])
    .map((tag) => ({
      pubkey: tag[1] as PublicKey,
      mainRelay: tag[2] || undefined,
      petname: tag[3] || undefined
    }))

/**
 * Merge two contact lists, preferring newer entries
 */
export const mergeContactLists = (
  list1: ContactList,
  list2: ContactList
): ContactList => {
  const merged = new Map<PublicKey, Contact>()

  // Add all contacts from list1
  for (const contact of list1.contacts) {
    merged.set(contact.pubkey, contact)
  }

  // Add contacts from list2, overwriting if newer
  for (const contact of list2.contacts) {
    const existing = merged.get(contact.pubkey)
    if (!existing || list2.updatedAt > list1.updatedAt) {
      merged.set(contact.pubkey, contact)
    }
  }

  return {
    contacts: Array.from(merged.values()),
    updatedAt: Math.max(list1.updatedAt, list2.updatedAt)
  }
}
