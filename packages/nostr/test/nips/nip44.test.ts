/**
 * Tests for NIP-44: Versioned Encryption
 */

import { describe, expect, it } from "@effect/vitest"
import { Effect, Exit } from "effect"
import type { PrivateKey, PublicKey, Signature } from "../../src/core/Schema.js"
import * as Nip44 from "../../src/nips/nip44.js"

describe("NIP-44", () => {
  const senderPrivkey = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa" as PrivateKey
  const senderPubkey = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d" as PublicKey
  const recipientPrivkey = "44e1fdac7dd8ec1f0ee992a8e5cdd3a14ebef8e5cf5486f178f449252e548c5d" as PrivateKey
  const recipientPubkey = "91cf9695cb99c77d5645f319b4b4d59b0b55e8907b23af0b11bb7b10dcf096d9" as PublicKey

  describe("Versioned Encryption/Decryption", () => {
    it.skip("should encrypt and decrypt a simple message", () => {
      const message = "Hello, this is a secret message using NIP-44!"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted = yield* service.encrypt(message, recipientPubkey, senderPrivkey)

        expect(encrypted.version).toBe(1)
        expect(encrypted.nonce).toBeDefined()
        expect(encrypted.ciphertext).toBeDefined()
        expect(encrypted.payload).toBeDefined()

        const decrypted = yield* service.decrypt(encrypted, senderPubkey, recipientPrivkey)
        expect(decrypted).toBe(message)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it.skip("should encrypt and decrypt using payload format", () => {
      const message = "Testing payload format"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const payload = yield* service.encryptFromPayload(message, recipientPubkey, senderPrivkey)

        expect(typeof payload).toBe("string")
        expect(payload.length).toBeGreaterThan(0)

        const decrypted = yield* service.decryptFromPayload(payload, senderPubkey, recipientPrivkey)
        expect(decrypted).toBe(message)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it.skip("should handle empty messages", () => {
      const message = ""

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted = yield* service.encrypt(message, recipientPubkey, senderPrivkey)
        const decrypted = yield* service.decrypt(encrypted, senderPubkey, recipientPrivkey)
        expect(decrypted).toBe(message)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it.skip("should handle unicode messages", () => {
      const message = "Hello 🌍! 这是一个加密消息 🔐 with emojis"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted = yield* service.encrypt(message, recipientPubkey, senderPrivkey)
        const decrypted = yield* service.decrypt(encrypted, senderPubkey, recipientPrivkey)
        expect(decrypted).toBe(message)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it.skip("should handle long messages", () => {
      const message = "A".repeat(5000) // 5000 character message

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted = yield* service.encrypt(message, recipientPubkey, senderPrivkey)
        const decrypted = yield* service.decrypt(encrypted, senderPubkey, recipientPrivkey)
        expect(decrypted).toBe(message)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it("should fail with wrong private key", () => {
      const message = "Secret message"
      const wrongPrivkey = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" as PrivateKey

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted = yield* service.encrypt(message, recipientPubkey, senderPrivkey)
        yield* service.decrypt(encrypted, senderPubkey, wrongPrivkey)
      })

      const result = Effect.runSyncExit(
        program.pipe(Effect.provide(Nip44.Nip44ServiceLive))
      )

      expect(Exit.isFailure(result)).toBe(true)
    })

    it("should reject unsupported encryption version", () => {
      const message = "Test message"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        yield* service.encrypt(message, recipientPubkey, senderPrivkey, 2 as any)
      })

      const result = Effect.runSyncExit(
        program.pipe(Effect.provide(Nip44.Nip44ServiceLive))
      )

      expect(Exit.isFailure(result)).toBe(true)
    })
  })

  describe("Conversation Key Derivation", () => {
    it("should derive consistent conversation keys", () => {
      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        // Test that the same key pair produces the same result
        const key1 = yield* service.deriveConversationKey(senderPrivkey, recipientPubkey)
        const key2 = yield* service.deriveConversationKey(senderPrivkey, recipientPubkey)

        expect(key1.sharedSecret).toBe(key2.sharedSecret)
        expect(key1.conversationKey).toBe(key2.conversationKey)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it("should derive different keys for different pairs", () => {
      const otherPrivkey = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" as PrivateKey

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const key1 = yield* service.deriveConversationKey(senderPrivkey, recipientPubkey)
        const key2 = yield* service.deriveConversationKey(otherPrivkey, recipientPubkey)

        expect(key1.sharedSecret).not.toBe(key2.sharedSecret)
        expect(key1.conversationKey).not.toBe(key2.conversationKey)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it("should use salt for key derivation", () => {
      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const key1 = yield* service.deriveConversationKey(senderPrivkey, recipientPubkey)
        const key2 = yield* service.deriveConversationKey(senderPrivkey, recipientPubkey, "custom-salt")

        expect(key1.conversationKey).not.toBe(key2.conversationKey)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })
  })

  describe("Format Validation", () => {
    it("should validate proper encrypted payload", () => {
      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        // Create a valid payload first
        const payload = yield* service.encryptFromPayload("test message", recipientPubkey, senderPrivkey)
        const validated = yield* service.validateFormat(payload)

        expect(validated.version).toBe(1)
        expect(validated.nonce).toBeDefined()
        expect(validated.ciphertext).toBeDefined()
        expect(validated.payload).toBe(payload)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it("should reject invalid base64 payload", () => {
      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        yield* service.validateFormat("not-valid-base64!@#$")
      })

      const result = Effect.runSyncExit(
        program.pipe(Effect.provide(Nip44.Nip44ServiceLive))
      )

      expect(Exit.isFailure(result)).toBe(true)
    })

    it("should reject too short payload", () => {
      const shortPayload = Buffer.from("short").toString("base64")

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        yield* service.validateFormat(shortPayload)
      })

      const result = Effect.runSyncExit(
        program.pipe(Effect.provide(Nip44.Nip44ServiceLive))
      )

      expect(Exit.isFailure(result)).toBe(true)
    })

    it("should reject unsupported version in payload", () => {
      // Create payload with version 2 (unsupported)
      const invalidPayload = Buffer.concat([
        Buffer.from([2]), // Invalid version
        Buffer.alloc(32), // Nonce
        Buffer.alloc(16) // Minimum ciphertext
      ]).toString("base64")

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        yield* service.validateFormat(invalidPayload)
      })

      const result = Effect.runSyncExit(
        program.pipe(Effect.provide(Nip44.Nip44ServiceLive))
      )

      expect(Exit.isFailure(result)).toBe(true)
    })
  })

  describe("Direct Message Events", () => {
    it("should create encrypted direct message event", () => {
      const message = "Secret event message"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const event = yield* service.createEncryptedMessage(message, recipientPubkey, senderPrivkey)

        expect(event.kind).toBe(4)
        expect(event.content).toBeDefined()
        expect(event.tags).toContainEqual(["p", recipientPubkey])

        // Content should be a valid base64 string
        expect(() => Buffer.from(event.content, "base64")).not.toThrow()
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it.skip("should parse and decrypt encrypted message event", () => {
      const originalMessage = "Original secret message"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        // Create encrypted payload first
        const payload = yield* service.encryptFromPayload(originalMessage, recipientPubkey, senderPrivkey)

        // Create event structure
        const event: Nip44.EncryptedDirectMessage = {
          id: "test-event-id",
          pubkey: senderPubkey,
          created_at: Math.floor(Date.now() / 1000),
          kind: 4,
          tags: [["p", recipientPubkey]],
          content: payload,
          sig: "test-signature" as Signature
        }

        const result = yield* service.parseEncryptedMessage(event, recipientPrivkey)

        expect(result.content).toBe(originalMessage)
        expect(result.sender).toBe(senderPubkey)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })
  })

  describe("Version Support", () => {
    it("should return supported versions", () => {
      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const versions = yield* service.getSupportedVersions()
        expect(versions).toEqual([1])
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })
  })

  describe("Utility Functions", () => {
    it("should calculate padded size correctly", () => {
      expect(Nip44.calculatePaddedSize(10)).toBe(32) // Minimum 32 bytes
      expect(Nip44.calculatePaddedSize(32)).toBe(32) // Already multiple of 32
      expect(Nip44.calculatePaddedSize(40)).toBe(64) // Round up to next 32
      expect(Nip44.calculatePaddedSize(100)).toBe(128) // Round up to 128
    })

    it("should check version support correctly", () => {
      expect(Nip44.isVersionSupported(1)).toBe(true)
      expect(Nip44.isVersionSupported(0)).toBe(false)
      expect(Nip44.isVersionSupported(2)).toBe(false)
      expect(Nip44.isVersionSupported(-1)).toBe(false)
    })

    it("should generate random salt", () => {
      const salt1 = Nip44.generateSalt()
      const salt2 = Nip44.generateSalt()

      expect(salt1).not.toBe(salt2)
      expect(salt1).toHaveLength(64) // 32 bytes = 64 hex chars
      expect(/^[0-9a-f]+$/.test(salt1)).toBe(true) // Valid hex
    })

    it("should pad and unpad messages correctly", () => {
      const messages = [
        "a",
        "hello",
        "this is a longer message that needs padding",
        "x".repeat(100),
        "y".repeat(1000)
      ]

      for (const msg of messages) {
        const padded = Nip44.padMessage(msg)
        expect(padded.length).toBeGreaterThanOrEqual(32)
        expect(padded.length % 32).toBe(0)

        const unpadded = Nip44.unpadMessage(padded)
        expect(unpadded).toBe(msg)
      }
    })

    it("should reject NIP-04 migration for now", () => {
      const nip04Content = "legacy-encrypted-content"

      const result = Effect.runSyncExit(
        Nip44.migrateFromNip04(
          nip04Content,
          senderPubkey,
          recipientPrivkey,
          senderPrivkey
        )
      )

      expect(Exit.isFailure(result)).toBe(true)
    })
  })

  describe("Security Features", () => {
    it.skip("should produce different ciphertexts for same message", () => {
      const message = "Same message"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted1 = yield* service.encrypt(message, recipientPubkey, senderPrivkey)
        const encrypted2 = yield* service.encrypt(message, recipientPubkey, senderPrivkey)

        // Nonces should be different
        expect(encrypted1.nonce).not.toBe(encrypted2.nonce)
        // Ciphertexts should be different
        expect(encrypted1.ciphertext).not.toBe(encrypted2.ciphertext)

        // But both should decrypt to same message
        const decrypted1 = yield* service.decrypt(encrypted1, senderPubkey, recipientPrivkey)
        const decrypted2 = yield* service.decrypt(encrypted2, senderPubkey, recipientPrivkey)

        expect(decrypted1).toBe(message)
        expect(decrypted2).toBe(message)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })

    it.skip("should handle message padding securely", () => {
      // Messages of different lengths should have predictable padding
      const shortMessage = "Hi"
      const mediumMessage = "Hello there, this is a longer message"

      const program = Effect.gen(function*() {
        const service = yield* Nip44.Nip44Service
        const encrypted1 = yield* service.encrypt(shortMessage, recipientPubkey, senderPrivkey)
        const encrypted2 = yield* service.encrypt(mediumMessage, recipientPubkey, senderPrivkey)

        // Verify decryption works correctly
        const decrypted1 = yield* service.decrypt(encrypted1, senderPubkey, recipientPrivkey)
        const decrypted2 = yield* service.decrypt(encrypted2, senderPubkey, recipientPrivkey)

        expect(decrypted1).toBe(shortMessage)
        expect(decrypted2).toBe(mediumMessage)
      })

      Effect.runSync(program.pipe(Effect.provide(Nip44.Nip44ServiceLive)))
    })
  })
})
