/**
 * Tests for CryptoService
 */

import { Effect } from "effect"
import { describe, expect, it } from "vitest"
import type { PrivateKey } from "../src/core/Schema.js"
import { CryptoService, CryptoServiceLive } from "../src/services/CryptoService.js"

describe("CryptoService", () => {
  const runTest = <E, A>(effect: Effect.Effect<A, E, CryptoService>) =>
    Effect.gen(function*() {
      return yield* effect
    }).pipe(
      Effect.provide(CryptoServiceLive),
      Effect.runPromise
    )

  describe("generatePrivateKey", () => {
    it("should generate a valid 32-byte hex private key", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const privateKey = yield* crypto.generatePrivateKey()

          expect(privateKey).toMatch(/^[0-9a-f]{64}$/)
          expect(privateKey.length).toBe(64)
        })
      ))

    it("should generate different keys each time", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const key1 = yield* crypto.generatePrivateKey()
          const key2 = yield* crypto.generatePrivateKey()

          expect(key1).not.toBe(key2)
        })
      ))
  })

  describe("getPublicKey", () => {
    it("should derive public key from private key", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const privateKey = yield* crypto.generatePrivateKey()
          const publicKey = yield* crypto.getPublicKey(privateKey)

          expect(publicKey).toMatch(/^[0-9a-f]{64}$/)
          expect(publicKey.length).toBe(64)
        })
      ))

    it("should derive the same public key for the same private key", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const privateKey = "d09ca2ba7e7a5b3c3fba8b56e70aa9de9e3291453ec56b9c28110edbb3e3c4b7" as PrivateKey

          const pubKey1 = yield* crypto.getPublicKey(privateKey)
          const pubKey2 = yield* crypto.getPublicKey(privateKey)

          expect(pubKey1).toBe(pubKey2)
        })
      ))

    it("should fail for invalid private key", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const invalidKey = "invalid" as PrivateKey

          const result = yield* crypto.getPublicKey(invalidKey).pipe(
            Effect.map(() => "success" as const),
            Effect.catchTag("InvalidPrivateKey", () => Effect.succeed("failed" as const))
          )

          expect(result).toBe("failed")
        })
      ))
  })

  describe("sign and verify", () => {
    it("should sign and verify a message", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const privateKey = yield* crypto.generatePrivateKey()
          const publicKey = yield* crypto.getPublicKey(privateKey)
          const message = "Hello, Nostr!"

          const signature = yield* crypto.sign(message, privateKey)
          expect(signature).toMatch(/^[0-9a-f]{128}$/)
          expect(signature.length).toBe(128)

          const isValid = yield* crypto.verify(signature, message, publicKey)
          expect(isValid).toBe(true)
        })
      ))

    it("should fail verification with wrong public key", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const privateKey1 = yield* crypto.generatePrivateKey()
          const privateKey2 = yield* crypto.generatePrivateKey()
          const publicKey2 = yield* crypto.getPublicKey(privateKey2)
          const message = "Hello, Nostr!"

          const signature = yield* crypto.sign(message, privateKey1)
          const isValid = yield* crypto.verify(signature, message, publicKey2)

          expect(isValid).toBe(false)
        })
      ))

    it("should fail verification with wrong message", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const privateKey = yield* crypto.generatePrivateKey()
          const publicKey = yield* crypto.getPublicKey(privateKey)
          const message1 = "Hello, Nostr!"
          const message2 = "Goodbye, Nostr!"

          const signature = yield* crypto.sign(message1, privateKey)
          const isValid = yield* crypto.verify(signature, message2, publicKey)

          expect(isValid).toBe(false)
        })
      ))
  })

  describe("hash", () => {
    it("should create a sha256 hash", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const message = "Hello, Nostr!"

          const hash = yield* crypto.hash(message)
          expect(hash).toMatch(/^[0-9a-f]{64}$/)
          expect(hash.length).toBe(64)
        })
      ))

    it("should create the same hash for the same message", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService
          const message = "Hello, Nostr!"

          const hash1 = yield* crypto.hash(message)
          const hash2 = yield* crypto.hash(message)

          expect(hash1).toBe(hash2)
        })
      ))

    it("should create different hashes for different messages", () =>
      runTest(
        Effect.gen(function*() {
          const crypto = yield* CryptoService

          const hash1 = yield* crypto.hash("Hello, Nostr!")
          const hash2 = yield* crypto.hash("Goodbye, Nostr!")

          expect(hash1).not.toBe(hash2)
        })
      ))
  })
})
