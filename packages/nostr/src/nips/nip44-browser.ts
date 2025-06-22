/**
 * NIP-44: Versioned Encryption (Browser-compatible)
 * Uses @noble/ciphers instead of node:crypto
 */

import { chacha20 } from "@noble/ciphers/chacha"
import { equalBytes } from "@noble/ciphers/utils"
import { secp256k1 } from "@noble/curves/secp256k1"
import { hkdf } from "@noble/hashes/hkdf"
import { hmac } from "@noble/hashes/hmac"
import { sha256 } from "@noble/hashes/sha256"
import { bytesToHex, concatBytes, randomBytes } from "@noble/hashes/utils"
import { base64 } from "@scure/base"
import { Context, Data, Effect, Layer, Schema } from "effect"
import type { PrivateKey } from "../core/Schema.js"
import { PublicKey, Signature } from "../core/Schema.js"

// Define types locally to avoid importing from Node.js version

export const EncryptionVersion = Schema.Literal(1, 2)
export type EncryptionVersion = Schema.Schema.Type<typeof EncryptionVersion>

export const VersionedEncryptedMessage = Schema.Struct({
  version: EncryptionVersion,
  nonce: Schema.String,
  ciphertext: Schema.String,
  mac: Schema.optional(Schema.String),
  payload: Schema.String
})
export type VersionedEncryptedMessage = Schema.Schema.Type<typeof VersionedEncryptedMessage>

export const EncryptedDirectMessage = Schema.Struct({
  id: Schema.String,
  pubkey: PublicKey,
  created_at: Schema.Number,
  kind: Schema.Literal(4),
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Signature
})
export type EncryptedDirectMessage = Schema.Schema.Type<typeof EncryptedDirectMessage>

export const ConversationKey = Schema.Struct({
  sharedSecret: Schema.String,
  conversationKey: Schema.String,
  createdAt: Schema.Number,
  expiresAt: Schema.Number
})
export type ConversationKey = Schema.Schema.Type<typeof ConversationKey>

export class Nip44Error extends Data.TaggedError("Nip44Error")<{
  reason:
    | "encryption_failed"
    | "decryption_failed"
    | "invalid_key"
    | "invalid_format"
    | "invalid_version"
    | "unsupported_version"
    | "key_derivation_failed"
    | "authentication_failed"
    | "padding_error"
  message: string
  sender?: PublicKey
  recipient?: PublicKey
  version?: EncryptionVersion
  conversationId?: string
  cause?: unknown
}> {}

// Constants
const minPlaintextSize = 0x0001
const maxPlaintextSize = 0xffff

// Helper functions
const utf8Encoder = new TextEncoder()
const utf8Decoder = new TextDecoder()

function hexToUint8Array(hex: string): Uint8Array {
  const array = new Uint8Array(hex.length / 2)
  for (let i = 0; i < hex.length; i += 2) {
    array[i / 2] = parseInt(hex.substr(i, 2), 16)
  }
  return array
}

function getConversationKey(privkeyA: Uint8Array | string, pubkeyB: string): Uint8Array {
  const privkey = typeof privkeyA === "string" ? hexToUint8Array(privkeyA) : privkeyA
  const sharedX = secp256k1.getSharedSecret(privkey, "02" + pubkeyB).subarray(1, 33)
  return hkdf(sha256, sharedX, "nip44-v2", undefined, 32)
}

function getMessageKeys(
  conversationKey: Uint8Array,
  nonce: Uint8Array
): { chacha_key: Uint8Array; chacha_nonce: Uint8Array; hmac_key: Uint8Array } {
  const keys = hkdf(sha256, conversationKey, nonce, undefined, 76)
  return {
    chacha_key: keys.subarray(0, 32),
    chacha_nonce: keys.subarray(32, 44),
    hmac_key: keys.subarray(44, 76)
  }
}

function calcPaddedLen(len: number): number {
  if (!Number.isSafeInteger(len) || len < 1) throw new Error("expected positive integer")
  if (len <= 32) return 32
  const nextPower = 1 << (Math.floor(Math.log2(len - 1)) + 1)
  const chunk = nextPower <= 256 ? 32 : nextPower / 8
  return chunk * (Math.floor((len - 1) / chunk) + 1)
}

function writeU16BE(num: number): Uint8Array {
  if (!Number.isSafeInteger(num) || num < minPlaintextSize || num > maxPlaintextSize) {
    throw new Error("invalid plaintext size: must be between 1 and 65535 bytes")
  }
  const arr = new Uint8Array(2)
  new DataView(arr.buffer).setUint16(0, num, false)
  return arr
}

function pad(plaintext: string): Uint8Array {
  const unpadded = utf8Encoder.encode(plaintext)
  const unpaddedLen = unpadded.length
  const prefix = writeU16BE(unpaddedLen)
  const suffix = new Uint8Array(calcPaddedLen(unpaddedLen) - unpaddedLen)
  return concatBytes(prefix, unpadded, suffix)
}

function unpad(padded: Uint8Array): string {
  const unpaddedLen = new DataView(padded.buffer).getUint16(0)
  const unpadded = padded.subarray(2, 2 + unpaddedLen)
  if (
    unpaddedLen < minPlaintextSize ||
    unpaddedLen > maxPlaintextSize ||
    unpadded.length !== unpaddedLen ||
    padded.length !== 2 + calcPaddedLen(unpaddedLen)
  ) {
    throw new Error("invalid padding")
  }
  return utf8Decoder.decode(unpadded)
}

function hmacAad(key: Uint8Array, message: Uint8Array, aad: Uint8Array): Uint8Array {
  if (aad.length !== 32) throw new Error("AAD associated data must be 32 bytes")
  const combined = concatBytes(aad, message)
  return hmac(sha256, key, combined)
}

// Service
export class Nip44Service extends Context.Tag("nips/Nip44Service")<
  Nip44Service,
  {
    readonly encrypt: (
      message: string,
      recipientPubkey: PublicKey,
      senderPrivkey: PrivateKey,
      version?: EncryptionVersion
    ) => Effect.Effect<VersionedEncryptedMessage, Nip44Error>

    readonly decrypt: (
      encryptedMessage: VersionedEncryptedMessage,
      senderPubkey: PublicKey,
      recipientPrivkey: PrivateKey
    ) => Effect.Effect<string, Nip44Error>

    readonly encryptFromPayload: (
      message: string,
      recipientPubkey: PublicKey,
      senderPrivkey: PrivateKey
    ) => Effect.Effect<string, Nip44Error>

    readonly decryptFromPayload: (
      payload: string,
      senderPubkey: PublicKey,
      recipientPrivkey: PrivateKey
    ) => Effect.Effect<string, Nip44Error>

    readonly parsePayload: (
      payload: string
    ) => Effect.Effect<VersionedEncryptedMessage, Nip44Error>

    readonly deriveConversationKey: (
      privateKey: PrivateKey,
      publicKey: PublicKey
    ) => Effect.Effect<ConversationKey, Nip44Error>

    readonly validateFormat: (
      encryptedMessage: VersionedEncryptedMessage
    ) => Effect.Effect<void, Nip44Error>
  }
>() {}

// Implementation
const makeNip44Service = () => ({
  encrypt: (message, recipientPubkey, senderPrivkey) =>
    Effect.gen(function*() {
      try {
        const conversationKey = getConversationKey(senderPrivkey, recipientPubkey)
        const nonce = randomBytes(32)
        const { chacha_key, chacha_nonce, hmac_key } = getMessageKeys(conversationKey, nonce)

        const padded = pad(message)
        const ciphertext = chacha20(chacha_key, chacha_nonce, padded)
        const mac = hmacAad(hmac_key, ciphertext, nonce)

        const payload = base64.encode(concatBytes(new Uint8Array([2]), nonce, ciphertext, mac))

        return {
          version: 2 as EncryptionVersion,
          nonce: base64.encode(nonce),
          ciphertext: base64.encode(ciphertext),
          mac: base64.encode(mac),
          payload
        }
      } catch (error) {
        return yield* Effect.fail(
          new Nip44Error({
            reason: "encryption_failed",
            message: `Failed to encrypt message: ${error}`,
            recipient: recipientPubkey,
            cause: error
          })
        )
      }
    }),

  decrypt: (encryptedMessage, senderPubkey, recipientPrivkey) =>
    Effect.gen(function*() {
      try {
        if (encryptedMessage.version !== 2) {
          return yield* Effect.fail(
            new Nip44Error({
              reason: "unsupported_version",
              message: `Unsupported version: ${encryptedMessage.version}`,
              version: encryptedMessage.version
            })
          )
        }

        const conversationKey = getConversationKey(recipientPrivkey, senderPubkey)
        const nonce = base64.decode(encryptedMessage.nonce)
        const ciphertext = base64.decode(encryptedMessage.ciphertext)
        const receivedMac = base64.decode(encryptedMessage.mac!)

        const { chacha_key, chacha_nonce, hmac_key } = getMessageKeys(conversationKey, nonce)

        const calculatedMac = hmacAad(hmac_key, ciphertext, nonce)
        if (!equalBytes(calculatedMac, receivedMac)) {
          return yield* Effect.fail(
            new Nip44Error({
              reason: "authentication_failed",
              message: "Invalid MAC",
              sender: senderPubkey
            })
          )
        }

        const padded = chacha20(chacha_key, chacha_nonce, ciphertext)
        return unpad(padded)
      } catch (error) {
        return yield* Effect.fail(
          new Nip44Error({
            reason: "decryption_failed",
            message: `Failed to decrypt: ${error}`,
            sender: senderPubkey,
            cause: error
          })
        )
      }
    }),

  encryptFromPayload: (message, recipientPubkey, senderPrivkey) =>
    Effect.gen(function*() {
      const encrypted = yield* makeNip44Service().encrypt(message, recipientPubkey, senderPrivkey)
      return encrypted.payload
    }),

  decryptFromPayload: (payload, senderPubkey, recipientPrivkey) =>
    Effect.gen(function*() {
      const service = makeNip44Service()
      const parsed = yield* service.parsePayload(payload)
      return yield* service.decrypt(parsed, senderPubkey, recipientPrivkey)
    }),

  parsePayload: (payload) =>
    Effect.gen(function*() {
      try {
        const data = base64.decode(payload)

        if (data.length < 99) {
          throw new Error("payload too short")
        }

        const version = data[0]
        if (version !== 2) {
          throw new Error(`unsupported version ${version}`)
        }

        const nonce = data.subarray(1, 33)
        const ciphertext = data.subarray(33, -32)
        const mac = data.subarray(-32)

        return {
          version: 2 as EncryptionVersion,
          nonce: base64.encode(nonce),
          ciphertext: base64.encode(ciphertext),
          mac: base64.encode(mac),
          payload
        }
      } catch (error) {
        return yield* Effect.fail(
          new Nip44Error({
            reason: "invalid_format",
            message: `Failed to parse payload: ${error}`,
            cause: error
          })
        )
      }
    }),

  deriveConversationKey: (privateKey, publicKey) =>
    Effect.gen(function*() {
      try {
        const conversationKey = getConversationKey(privateKey, publicKey)
        const privkey = typeof privateKey === "string" ? hexToUint8Array(privateKey) : privateKey
        const sharedX = secp256k1.getSharedSecret(privkey, "02" + publicKey).subarray(1, 33)

        return {
          sharedSecret: bytesToHex(sharedX),
          conversationKey: bytesToHex(conversationKey),
          createdAt: Date.now(),
          expiresAt: Date.now() + 24 * 60 * 60 * 1000
        }
      } catch (error) {
        return yield* Effect.fail(
          new Nip44Error({
            reason: "key_derivation_failed",
            message: `Failed to derive key: ${error}`,
            cause: error
          })
        )
      }
    }),

  validateFormat: (encryptedMessage) =>
    Effect.gen(function*() {
      if (encryptedMessage.version !== 2) {
        return yield* Effect.fail(
          new Nip44Error({
            reason: "invalid_version",
            message: `Invalid version: ${encryptedMessage.version}`,
            version: encryptedMessage.version
          })
        )
      }

      try {
        base64.decode(encryptedMessage.nonce)
        base64.decode(encryptedMessage.ciphertext)
        if (encryptedMessage.mac) base64.decode(encryptedMessage.mac)
        base64.decode(encryptedMessage.payload)
      } catch (error) {
        return yield* Effect.fail(
          new Nip44Error({
            reason: "invalid_format",
            message: "Invalid base64 encoding",
            cause: error
          })
        )
      }
    })
})

export const Nip44ServiceLive = Layer.succeed(
  Nip44Service,
  makeNip44Service()
)

// Helper functions
export const createEncryptedDirectMessage = (
  message: string,
  recipientPubkey: PublicKey,
  senderPrivkey: PrivateKey
): Effect.Effect<EncryptedDirectMessage, Nip44Error> =>
  Effect.gen(function*() {
    const encrypted = yield* makeNip44Service().encrypt(message, recipientPubkey, senderPrivkey)
    const now = Math.floor(Date.now() / 1000)

    return {
      id: "placeholder-id",
      pubkey: "placeholder-pubkey" as PublicKey,
      created_at: now,
      kind: 4 as const,
      tags: [["p", recipientPubkey], ["nip44", "2"]],
      content: encrypted.payload,
      sig: "placeholder-signature" as Signature
    }
  })

export const isNip44Message = (event: EncryptedDirectMessage): boolean => {
  return event.tags.some((tag) => tag[0] === "nip44" && (tag[1] === "1" || tag[1] === "2"))
}

export const generateMessageId = (): string => {
  return bytesToHex(randomBytes(16))
}
