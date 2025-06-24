import { Effect, Layer } from "effect"
import { describe, expect, it } from "vitest"
import { AuthEventKind } from "../src/core/Schema.js"
import { Nip06Service, Nip06ServiceLive } from "../src/nip06/Nip06Service.js"
import { Nip42Service, Nip42ServiceLive } from "../src/nip42/Nip42Service.js"
import { CryptoService, CryptoServiceLive } from "../src/services/CryptoService.js"
import type { EventService } from "../src/services/EventService.js"
import { EventServiceLive } from "../src/services/EventService.js"

describe("Nip42Service", () => {
  const TestLayer = Layer.mergeAll(
    CryptoServiceLive,
    EventServiceLive.pipe(Layer.provide(CryptoServiceLive)),
    Nip06ServiceLive.pipe(Layer.provide(CryptoServiceLive)),
    Nip42ServiceLive.pipe(Layer.provide(EventServiceLive.pipe(Layer.provide(CryptoServiceLive))))
  )

  const runTest = <E, A>(effect: Effect.Effect<A, E, Nip42Service | EventService | CryptoService | Nip06Service>) =>
    effect.pipe(
      Effect.provide(TestLayer),
      Effect.runPromise
    )

  describe("generateChallenge", () => {
    it("should generate a valid auth challenge", () =>
      runTest(
        Effect.gen(function*() {
          const nip42 = yield* Nip42Service
          const challenge = yield* nip42.generateChallenge()

          expect(challenge).toBeDefined()
          expect(challenge.length).toBeGreaterThanOrEqual(16)
          expect(challenge).toMatch(/^[0-9a-f]+$/) // Hex string
        })
      ))

    it("should generate unique challenges", () =>
      runTest(
        Effect.gen(function*() {
          const nip42 = yield* Nip42Service
          const challenge1 = yield* nip42.generateChallenge()
          const challenge2 = yield* nip42.generateChallenge()

          expect(challenge1).not.toBe(challenge2)
        })
      ))
  })

  describe("createAuthEvent", () => {
    it("should create a valid auth event", () =>
      runTest(
        Effect.gen(function*() {
          const nip42 = yield* Nip42Service
          const nip06 = yield* Nip06Service
          const crypto = yield* CryptoService

          // Generate test keys
          const mnemonic = yield* nip06.generateMnemonic()
          const privateKey = yield* nip06.derivePrivateKey(mnemonic)
          const publicKey = yield* crypto.getPublicKey(privateKey)

          const challenge = yield* nip42.generateChallenge()
          const relayUrl = "wss://relay.example.com"

          const authEvent = yield* nip42.createAuthEvent({
            challenge,
            relayUrl,
            privateKey
          })

          expect(authEvent.kind).toBe(AuthEventKind)
          expect(authEvent.pubkey).toBe(publicKey)
          expect(authEvent.content).toBe("")

          // Check tags
          const relayTag = authEvent.tags.find((t) => t[0] === "relay")
          const challengeTag = authEvent.tags.find((t) => t[0] === "challenge")

          expect(relayTag).toBeDefined()
          expect(relayTag![1]).toBe(relayUrl)
          expect(challengeTag).toBeDefined()
          expect(challengeTag![1]).toBe(challenge)

          // Verify signature
          expect(authEvent.sig).toBeDefined()
          expect(authEvent.sig.length).toBe(128) // 64 bytes hex
        })
      ))
  })

  describe("verifyAuthEvent", () => {
    it("should verify a valid auth event", () =>
      runTest(
        Effect.gen(function*() {
          const nip42 = yield* Nip42Service
          const nip06 = yield* Nip06Service

          // Generate test keys
          const mnemonic = yield* nip06.generateMnemonic()
          const privateKey = yield* nip06.derivePrivateKey(mnemonic)

          const challenge = yield* nip42.generateChallenge()
          const relayUrl = "wss://relay.example.com"

          // Create auth event
          const authEvent = yield* nip42.createAuthEvent({
            challenge,
            relayUrl,
            privateKey
          })

          // Verify it
          const isValid = yield* nip42.verifyAuthEvent({
            event: authEvent,
            challenge,
            relayUrl
          })

          expect(isValid).toBe(true)
        })
      ))

    it("should reject auth event with wrong challenge", () =>
      runTest(
        Effect.gen(function*() {
          const nip42 = yield* Nip42Service
          const nip06 = yield* Nip06Service

          // Generate test keys
          const mnemonic = yield* nip06.generateMnemonic()
          const privateKey = yield* nip06.derivePrivateKey(mnemonic)

          const challenge = yield* nip42.generateChallenge()
          const wrongChallenge = yield* nip42.generateChallenge()
          const relayUrl = "wss://relay.example.com"

          // Create auth event
          const authEvent = yield* nip42.createAuthEvent({
            challenge,
            relayUrl,
            privateKey
          })

          // Verify with wrong challenge
          const isValid = yield* nip42.verifyAuthEvent({
            event: authEvent,
            challenge: wrongChallenge,
            relayUrl
          })

          expect(isValid).toBe(false)
        })
      ))

    it("should reject auth event with wrong relay URL", () =>
      runTest(
        Effect.gen(function*() {
          const nip42 = yield* Nip42Service
          const nip06 = yield* Nip06Service

          // Generate test keys
          const mnemonic = yield* nip06.generateMnemonic()
          const privateKey = yield* nip06.derivePrivateKey(mnemonic)

          const challenge = yield* nip42.generateChallenge()
          const relayUrl = "wss://relay.example.com"
          const wrongRelayUrl = "wss://wrong.relay.com"

          // Create auth event
          const authEvent = yield* nip42.createAuthEvent({
            challenge,
            relayUrl,
            privateKey
          })

          // Verify with wrong relay URL
          const isValid = yield* nip42.verifyAuthEvent({
            event: authEvent,
            challenge,
            relayUrl: wrongRelayUrl
          })

          expect(isValid).toBe(false)
        })
      ))
  })
})
