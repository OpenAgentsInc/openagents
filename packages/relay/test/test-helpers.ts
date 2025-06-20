import { sha256 } from "@noble/hashes/sha256"
import { bytesToHex } from "@noble/hashes/utils"
import { CryptoService, EventService, Nip06Service } from "@openagentsinc/nostr"
import type { Schema } from "@openagentsinc/nostr"
import { Effect, Layer } from "effect"

// Test mnemonic for consistent test keys
export const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

// Generate test keypair using NIP-06
export const generateTestKeypair = (accountIndex = 0) =>
  Effect.gen(function*() {
    const nip06 = yield* Nip06Service.Nip06Service
    const privateKey = yield* nip06.derivePrivateKey(TEST_MNEMONIC, accountIndex)
    const publicKey = yield* nip06.derivePublicKey(privateKey)

    return { privateKey, publicKey }
  })

// Calculate event ID according to NIP-01
export function calculateEventId(event: Omit<Schema.NostrEvent, "id" | "sig">): string {
  const serialized = JSON.stringify([
    0,
    event.pubkey,
    event.created_at,
    event.kind,
    event.tags,
    event.content
  ])
  return bytesToHex(sha256(new TextEncoder().encode(serialized)))
}

// Sign event using EventService
export const signTestEvent = (
  event: Omit<Schema.NostrEvent, "sig">,
  privateKey: string
) =>
  Effect.gen(function*() {
    const eventService = yield* EventService.EventService

    // EventService expects the event to already have an ID
    const eventWithId = { ...event, id: event.id || calculateEventId(event) }
    const signature = yield* eventService.sign(eventWithId as any, privateKey)

    return { ...eventWithId, sig: signature } as Schema.NostrEvent
  })

// Create a complete test event
export const createTestEvent = (
  privateKey: string,
  publicKey: string,
  content: string,
  kind = 1,
  tags: Array<Array<string>> = []
) =>
  Effect.gen(function*() {
    const baseEvent = {
      pubkey: publicKey,
      created_at: Math.floor(Date.now() / 1000),
      kind,
      tags,
      content
    }

    const id = calculateEventId(baseEvent)
    const eventWithId = { ...baseEvent, id }

    const signedEvent = yield* signTestEvent(eventWithId, privateKey)
    return signedEvent
  })

// Provide NIP-06 layer for tests with all dependencies
export const TestNip06Live = Layer.merge(
  Nip06Service.Nip06ServiceLive,
  EventService.EventServiceLive
).pipe(
  Layer.provide(CryptoService.CryptoServiceLive)
)

// Wait for a condition with timeout
export const waitFor = <A>(
  check: () => Effect.Effect<A>,
  timeout = 5000,
  interval = 100
): Effect.Effect<A, Error> =>
  Effect.gen(function*() {
    const start = Date.now()

    while (Date.now() - start < timeout) {
      const result = yield* Effect.either(check())
      if (result._tag === "Right") {
        return result.right
      }
      yield* Effect.sleep(interval)
    }

    return yield* Effect.fail(new Error(`Timeout after ${timeout}ms`))
  })
