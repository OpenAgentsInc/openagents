/**
 * NIP-04: Encrypted Direct Messages (Browser-compatible)
 * Uses @noble/ciphers instead of node:crypto
 */

import { cbc } from "@noble/ciphers/aes"
import { secp256k1 } from "@noble/curves/secp256k1"
import { bytesToHex, randomBytes } from "@noble/hashes/utils"
import { base64 } from "@scure/base"
import { Context, Data, Effect, Layer, Schema } from "effect"
import type { PrivateKey } from "../core/Schema.js"
import { PublicKey, Signature } from "../core/Schema.js"

// Types
export const EncryptedMessage = Schema.Struct({
  content: Schema.String,
  iv: Schema.optional(Schema.String),
  mac: Schema.optional(Schema.String)
})
export type EncryptedMessage = Schema.Schema.Type<typeof EncryptedMessage>

export const DirectMessageEvent = Schema.Struct({
  id: Schema.String,
  pubkey: PublicKey,
  created_at: Schema.Number,
  kind: Schema.Literal(4),
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Signature
})
export type DirectMessageEvent = Schema.Schema.Type<typeof DirectMessageEvent>

export const MessageMetadata = Schema.Struct({
  sender: PublicKey,
  recipient: PublicKey,
  timestamp: Schema.Number,
  messageId: Schema.String,
  conversationId: Schema.optional(Schema.String)
})
export type MessageMetadata = Schema.Schema.Type<typeof MessageMetadata>

export const DecryptedMessage = Schema.Struct({
  content: Schema.String,
  metadata: MessageMetadata,
  verified: Schema.Boolean
})
export type DecryptedMessage = Schema.Schema.Type<typeof DecryptedMessage>

// Errors
export class Nip04Error extends Data.TaggedError("Nip04Error")<{
  reason:
    | "encryption_failed"
    | "decryption_failed"
    | "invalid_key"
    | "invalid_message"
    | "key_derivation_failed"
    | "mac_verification_failed"
  message: string
  messageId?: string
  sender?: PublicKey
  recipient?: PublicKey
  cause?: unknown
}> {}

// Helper functions
const utf8Encoder = new TextEncoder()
const utf8Decoder = new TextDecoder()

function getNormalizedX(key: Uint8Array): Uint8Array {
  return key.slice(1, 33)
}

// Service
export class Nip04Service extends Context.Tag("nips/Nip04Service")<
  Nip04Service,
  {
    readonly encryptMessage: (
      message: string,
      recipientPubkey: PublicKey,
      senderPrivkey: PrivateKey
    ) => Effect.Effect<EncryptedMessage, Nip04Error>

    readonly decryptMessage: (
      encryptedMessage: EncryptedMessage,
      senderPubkey: PublicKey,
      recipientPrivkey: PrivateKey
    ) => Effect.Effect<string, Nip04Error>

    readonly createDirectMessage: (
      message: string,
      recipientPubkey: PublicKey,
      senderPrivkey: PrivateKey,
      conversationId?: string
    ) => Effect.Effect<DirectMessageEvent, Nip04Error>

    readonly parseDirectMessage: (
      event: DirectMessageEvent,
      recipientPrivkey: PrivateKey
    ) => Effect.Effect<DecryptedMessage, Nip04Error>

    readonly deriveSharedSecret: (
      privateKey: PrivateKey,
      publicKey: PublicKey
    ) => Effect.Effect<Uint8Array, Nip04Error>

    readonly createConversationId: (
      pubkey1: PublicKey,
      pubkey2: PublicKey
    ) => Effect.Effect<string, Nip04Error>

    readonly validateEncryption: (
      encryptedMessage: EncryptedMessage
    ) => Effect.Effect<void, Nip04Error>
  }
>() {}

// Implementation
export const Nip04ServiceLive = Layer.succeed(
  Nip04Service,
  {
    encryptMessage: (message, recipientPubkey, senderPrivkey) =>
      Effect.gen(function*() {
        try {
          const privkey = typeof senderPrivkey === "string" ? senderPrivkey : bytesToHex(senderPrivkey)
          const key = secp256k1.getSharedSecret(privkey, "02" + recipientPubkey)
          const normalizedKey = getNormalizedX(key)

          const iv = randomBytes(16)
          const plaintext = utf8Encoder.encode(message)

          const cipher = cbc(normalizedKey, iv)
          const ciphertext = cipher.encrypt(plaintext)

          const ctb64 = base64.encode(new Uint8Array(ciphertext))
          const ivb64 = base64.encode(new Uint8Array(iv))

          return {
            content: `${ctb64}?iv=${ivb64}`,
            iv: ivb64,
            mac: undefined
          }
        } catch (error) {
          return yield* Effect.fail(
            new Nip04Error({
              reason: "encryption_failed",
              message: `Failed to encrypt message: ${error}`,
              recipient: recipientPubkey,
              cause: error
            })
          )
        }
      }),

    decryptMessage: (encryptedMessage, senderPubkey, recipientPrivkey) =>
      Effect.gen(function*() {
        try {
          const data = encryptedMessage.content
          const [ctb64, ivb64] = data.split("?iv=")

          if (!ctb64 || !ivb64) {
            return yield* Effect.fail(
              new Nip04Error({
                reason: "invalid_message",
                message: "Invalid NIP-04 format",
                sender: senderPubkey
              })
            )
          }

          const privkey = typeof recipientPrivkey === "string" ? recipientPrivkey : bytesToHex(recipientPrivkey)
          const key = secp256k1.getSharedSecret(privkey, "02" + senderPubkey)
          const normalizedKey = getNormalizedX(key)

          const iv = base64.decode(ivb64)
          const ciphertext = base64.decode(ctb64)

          const cipher = cbc(normalizedKey, iv)
          const plaintext = cipher.decrypt(ciphertext)

          return utf8Decoder.decode(plaintext)
        } catch (error) {
          return yield* Effect.fail(
            new Nip04Error({
              reason: "decryption_failed",
              message: `Failed to decrypt message: ${error}`,
              sender: senderPubkey,
              cause: error
            })
          )
        }
      }),

    createDirectMessage: (message, recipientPubkey, senderPrivkey, conversationId) =>
      Effect.gen(function*() {
        const service = yield* Nip04Service
        const encrypted = yield* service.encryptMessage(
          message,
          recipientPubkey,
          senderPrivkey
        )

        const tags: Array<Array<string>> = [["p", recipientPubkey]]
        if (conversationId) {
          tags.push(["conversation", conversationId])
        }

        const now = Math.floor(Date.now() / 1000)

        return {
          id: "placeholder-id",
          pubkey: "placeholder-pubkey" as PublicKey,
          created_at: now,
          kind: 4 as const,
          tags,
          content: encrypted.content,
          sig: "placeholder-signature" as Signature
        }
      }),

    parseDirectMessage: (event, recipientPrivkey) =>
      Effect.gen(function*() {
        const pTag = event.tags.find((tag) => tag[0] === "p")
        if (!pTag || !pTag[1]) {
          return yield* Effect.fail(
            new Nip04Error({
              reason: "invalid_message",
              message: "Missing recipient p-tag",
              messageId: event.id
            })
          )
        }

        const recipientPubkey = pTag[1] as PublicKey
        const encryptedMessage = { content: event.content }

        const service = yield* Nip04Service
        const decryptedContent = yield* service.decryptMessage(
          encryptedMessage,
          event.pubkey,
          recipientPrivkey
        )

        const conversationTag = event.tags.find((tag) => tag[0] === "conversation")
        const conversationId = conversationTag?.[1]

        return {
          content: decryptedContent,
          metadata: {
            sender: event.pubkey,
            recipient: recipientPubkey,
            timestamp: event.created_at,
            messageId: event.id,
            conversationId
          },
          verified: true
        }
      }),

    deriveSharedSecret: (privateKey, publicKey) =>
      Effect.try({
        try: () => {
          const privkey = typeof privateKey === "string" ? privateKey : bytesToHex(privateKey)
          const key = secp256k1.getSharedSecret(privkey, "02" + publicKey)
          return getNormalizedX(key)
        },
        catch: (error) =>
          new Nip04Error({
            reason: "key_derivation_failed",
            message: `Failed to derive shared secret: ${error}`,
            cause: error
          })
      }),

    createConversationId: (pubkey1, pubkey2) =>
      Effect.sync(() => {
        const sortedKeys = [pubkey1, pubkey2].sort()
        return sortedKeys.join("-").substring(0, 16)
      }),

    validateEncryption: (encryptedMessage) =>
      Effect.gen(function*() {
        const data = encryptedMessage.content
        const parts = data.split("?iv=")

        if (parts.length !== 2) {
          return yield* Effect.fail(
            new Nip04Error({
              reason: "invalid_message",
              message: "Invalid NIP-04 format"
            })
          )
        }

        try {
          base64.decode(parts[0])
          base64.decode(parts[1])
        } catch (error) {
          return yield* Effect.fail(
            new Nip04Error({
              reason: "invalid_message",
              message: "Invalid base64 encoding",
              cause: error
            })
          )
        }

        const ivBytes = base64.decode(parts[1])
        if (ivBytes.length !== 16) {
          return yield* Effect.fail(
            new Nip04Error({
              reason: "invalid_message",
              message: `Invalid IV length: ${ivBytes.length}`
            })
          )
        }
      })
  }
)

// Helper functions
export const generateConversationId = (): string => {
  return bytesToHex(randomBytes(8))
}

export const extractParticipants = (event: DirectMessageEvent): Array<PublicKey> => {
  const participants = [event.pubkey]
  const pTags = event.tags.filter((tag) => tag[0] === "p")
  for (const tag of pTags) {
    if (tag[1] && tag[1] !== event.pubkey) {
      participants.push(tag[1] as PublicKey)
    }
  }
  return participants
}

export const createThreadTags = (
  replyToEventId?: string,
  replyToAuthor?: PublicKey
): Array<Array<string>> => {
  const tags: Array<Array<string>> = []
  if (replyToEventId) {
    tags.push(["e", replyToEventId, "", "reply"])
  }
  if (replyToAuthor) {
    tags.push(["p", replyToAuthor])
  }
  return tags
}
