/**
 * Tests for Nip06Service - NIP-06 Key Derivation
 */

import { Effect } from "effect"
import { describe, expect, it } from "vitest"
import type { Mnemonic, Npub, Nsec, PrivateKey, PublicKey } from "../src/core/Schema.js"
import { Nip06Service, Nip06ServiceLive } from "../src/nip06/Nip06Service.js"
import { CryptoService, CryptoServiceLive } from "../src/services/CryptoService.js"

describe("Nip06Service", () => {
  const runTest = <E, A>(effect: Effect.Effect<A, E, Nip06Service | CryptoService>) =>
    Effect.gen(function*() {
      return yield* effect
    }).pipe(
      Effect.provide(Nip06ServiceLive),
      Effect.provide(CryptoServiceLive),
      Effect.runPromise
    )

  // Test vectors from NIP-06 specification
  const testVectors = [
    {
      mnemonic: "leader monkey parrot ring guide accident before fence cannon height naive bean",
      privateKey: "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a",
      publicKey: "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917",
      nsec: "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp",
      npub: "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu"
    },
    {
      mnemonic:
        "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade",
      privateKey: "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add",
      publicKey: "d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573",
      nsec: "nsec1c9wh8xy5eqdzln7n5t0ctgxjcrdug73gp5yj0x03gntn67h83twssdfhel",
      npub: "npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h"
    }
  ]

  describe("generateMnemonic", () => {
    it("should generate a valid 12-word mnemonic by default", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const mnemonic = yield* nip06.generateMnemonic()

          const words = mnemonic.split(/\s+/)
          expect(words).toHaveLength(12)

          // Validate it's a valid mnemonic
          const isValid = yield* nip06.validateMnemonic(mnemonic)
          expect(isValid).toBe(true)
        })
      ))

    it("should generate different mnemonics each time", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const mnemonic1 = yield* nip06.generateMnemonic()
          const mnemonic2 = yield* nip06.generateMnemonic()

          expect(mnemonic1).not.toBe(mnemonic2)
        })
      ))

    it("should generate mnemonics with specified word counts", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          const lengths = [12, 15, 18, 21, 24] as const

          for (const length of lengths) {
            const mnemonic = yield* nip06.generateMnemonic(length)
            const words = mnemonic.split(/\s+/)
            expect(words).toHaveLength(length)

            const isValid = yield* nip06.validateMnemonic(mnemonic)
            expect(isValid).toBe(true)
          }
        })
      ))
  })

  describe("validateMnemonic", () => {
    it("should validate test vector mnemonics", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const isValid = yield* nip06.validateMnemonic(vector.mnemonic)
            expect(isValid).toBe(true)
          }
        })
      ))

    it("should reject invalid mnemonics", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          const invalidMnemonics = [
            "invalid mnemonic phrase",
            "leader monkey parrot ring guide accident before fence cannon height naive", // wrong word
            "leader monkey parrot ring guide", // too short
            "", // empty
            "leader monkey parrot ring guide accident before fence cannon height naive bean extra" // too long for standard
          ]

          for (const invalid of invalidMnemonics) {
            const isValid = yield* nip06.validateMnemonic(invalid)
            expect(isValid).toBe(false)
          }
        })
      ))
  })

  describe("derivePrivateKey", () => {
    it("should derive correct private keys from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const privateKey = yield* nip06.derivePrivateKey(vector.mnemonic as Mnemonic)
            expect(privateKey).toBe(vector.privateKey)
          }
        })
      ))

    it("should derive different keys for different accounts", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const mnemonic = testVectors[0].mnemonic as Mnemonic

          const key0 = yield* nip06.derivePrivateKey(mnemonic, 0)
          const key1 = yield* nip06.derivePrivateKey(mnemonic, 1)
          const key2 = yield* nip06.derivePrivateKey(mnemonic, 2)

          expect(key0).not.toBe(key1)
          expect(key1).not.toBe(key2)
          expect(key0).not.toBe(key2)
        })
      ))

    it("should fail for invalid mnemonic", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const invalidMnemonic = "invalid mnemonic phrase" as Mnemonic

          const result = yield* nip06.derivePrivateKey(invalidMnemonic).pipe(
            Effect.map(() => "success" as const),
            Effect.catchTag("InvalidMnemonic", () => Effect.succeed("failed" as const))
          )

          expect(result).toBe("failed")
        })
      ))
  })

  describe("derivePublicKey", () => {
    it("should derive correct public keys from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const publicKey = yield* nip06.derivePublicKey(vector.privateKey as PrivateKey)
            expect(publicKey).toBe(vector.publicKey)
          }
        })
      ))
  })

  describe("nsec encoding/decoding", () => {
    it("should encode private keys to nsec format from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const nsec = yield* nip06.encodeNsec(vector.privateKey as PrivateKey)
            expect(nsec).toBe(vector.nsec)
          }
        })
      ))

    it("should decode nsec to private keys from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const privateKey = yield* nip06.decodeNsec(vector.nsec as Nsec)
            expect(privateKey).toBe(vector.privateKey)
          }
        })
      ))

    it("should round-trip encode/decode nsec", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const originalKey = testVectors[0].privateKey as PrivateKey

          const nsec = yield* nip06.encodeNsec(originalKey)
          const decodedKey = yield* nip06.decodeNsec(nsec)

          expect(decodedKey).toBe(originalKey)
        })
      ))
  })

  describe("npub encoding/decoding", () => {
    it("should encode public keys to npub format from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const npub = yield* nip06.encodeNpub(vector.publicKey as PublicKey)
            expect(npub).toBe(vector.npub)
          }
        })
      ))

    it("should decode npub to public keys from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const publicKey = yield* nip06.decodeNpub(vector.npub as Npub)
            expect(publicKey).toBe(vector.publicKey)
          }
        })
      ))

    it("should round-trip encode/decode npub", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const originalKey = testVectors[0].publicKey as PublicKey

          const npub = yield* nip06.encodeNpub(originalKey)
          const decodedKey = yield* nip06.decodeNpub(npub)

          expect(decodedKey).toBe(originalKey)
        })
      ))
  })

  describe("deriveAllKeys", () => {
    it("should derive complete key sets from test vectors", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          for (const vector of testVectors) {
            const result = yield* nip06.deriveAllKeys(vector.mnemonic as Mnemonic)

            expect(result.privateKey).toBe(vector.privateKey)
            expect(result.publicKey).toBe(vector.publicKey)
            expect(result.nsec).toBe(vector.nsec)
            expect(result.npub).toBe(vector.npub)
          }
        })
      ))

    it("should derive different key sets for different accounts", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const mnemonic = testVectors[0].mnemonic as Mnemonic

          const keys0 = yield* nip06.deriveAllKeys(mnemonic, 0)
          const keys1 = yield* nip06.deriveAllKeys(mnemonic, 1)

          expect(keys0.privateKey).not.toBe(keys1.privateKey)
          expect(keys0.publicKey).not.toBe(keys1.publicKey)
          expect(keys0.nsec).not.toBe(keys1.nsec)
          expect(keys0.npub).not.toBe(keys1.npub)
        })
      ))
  })

  describe("getDerivationPath", () => {
    it("should generate correct derivation paths", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          const path0 = yield* nip06.getDerivationPath(0)
          expect(path0).toBe("m/44'/1237'/0'/0/0")

          const path1 = yield* nip06.getDerivationPath(1)
          expect(path1).toBe("m/44'/1237'/1'/0/0")

          const path5 = yield* nip06.getDerivationPath(5)
          expect(path5).toBe("m/44'/1237'/5'/0/0")
        })
      ))

    it("should use account 0 by default", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          const defaultPath = yield* nip06.getDerivationPath()
          const explicitPath = yield* nip06.getDerivationPath(0)

          expect(defaultPath).toBe(explicitPath)
          expect(defaultPath).toBe("m/44'/1237'/0'/0/0")
        })
      ))
  })

  describe("integration with CryptoService", () => {
    it("should produce keys that work with CryptoService sign/verify", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service
          const mnemonic = testVectors[0].mnemonic as Mnemonic

          const keys = yield* nip06.deriveAllKeys(mnemonic)

          // Use CryptoService to test signing
          const crypto = yield* CryptoService
          const message = "Hello, NIP-06!"

          const signature = yield* crypto.sign(message, keys.privateKey)
          const isValid = yield* crypto.verify(signature, message, keys.publicKey)

          expect(isValid).toBe(true)
        })
      ))
  })

  describe("error handling", () => {
    it("should handle invalid bech32 formats gracefully", () =>
      runTest(
        Effect.gen(function*() {
          const nip06 = yield* Nip06Service

          const invalidNsec = "invalid_nsec_format" as Nsec
          const invalidNpub = "invalid_npub_format" as Npub

          const nsecResult = yield* nip06.decodeNsec(invalidNsec).pipe(
            Effect.map(() => "success" as const),
            Effect.catchTag("Nip06Error", () => Effect.succeed("failed" as const))
          )

          const npubResult = yield* nip06.decodeNpub(invalidNpub).pipe(
            Effect.map(() => "success" as const),
            Effect.catchTag("Nip06Error", () => Effect.succeed("failed" as const))
          )

          expect(nsecResult).toBe("failed")
          expect(npubResult).toBe("failed")
        })
      ))
  })
})
