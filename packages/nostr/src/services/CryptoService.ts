/**
 * Cryptographic operations for Nostr
 * @module
 */

import { schnorr } from "@noble/curves/secp256k1"
import { sha256 } from "@noble/hashes/sha256"
import { bytesToHex, hexToBytes, randomBytes } from "@noble/hashes/utils"
import { Context, Effect, Layer } from "effect"
import { CryptoError, InvalidPrivateKey } from "../core/Errors.js"
import type { EventId, PrivateKey, PublicKey, Signature } from "../core/Schema.js"

/**
 * Service for cryptographic operations
 */
export class CryptoService extends Context.Tag("nostr/CryptoService")<
  CryptoService,
  {
    /**
     * Generate a new private key
     */
    readonly generatePrivateKey: () => Effect.Effect<PrivateKey, CryptoError>

    /**
     * Get public key from private key
     */
    readonly getPublicKey: (privateKey: PrivateKey) => Effect.Effect<PublicKey, CryptoError | InvalidPrivateKey>

    /**
     * Sign a message with a private key
     */
    readonly sign: (
      message: string,
      privateKey: PrivateKey
    ) => Effect.Effect<Signature, CryptoError | InvalidPrivateKey>

    /**
     * Verify a signature
     */
    readonly verify: (
      signature: Signature,
      message: string,
      publicKey: PublicKey
    ) => Effect.Effect<boolean, CryptoError>

    /**
     * Hash a message to get event ID
     */
    readonly hash: (message: string) => Effect.Effect<EventId, CryptoError>
  }
>() {}

/**
 * Live implementation of CryptoService
 */
export const CryptoServiceLive = Layer.effect(
  CryptoService,
  Effect.sync(() => {
  const generatePrivateKey = (): Effect.Effect<PrivateKey, CryptoError> =>
    Effect.try({
      try: () => bytesToHex(randomBytes(32)) as PrivateKey,
      catch: (error) =>
        new CryptoError({
          operation: "generateKey",
          reason: String(error)
        })
    })

  const getPublicKey = (privateKey: PrivateKey): Effect.Effect<PublicKey, CryptoError | InvalidPrivateKey> =>
    Effect.gen(function*() {
      // Validate private key format
      if (!/^[0-9a-f]{64}$/.test(privateKey)) {
        return yield* new InvalidPrivateKey({
          reason: "Private key must be 64 hex characters"
        })
      }

      return yield* Effect.try({
        try: () => {
          const publicKey = schnorr.getPublicKey(privateKey)
          return bytesToHex(publicKey) as PublicKey
        },
        catch: (error) =>
          new CryptoError({
            operation: "generateKey",
            reason: String(error)
          })
      })
    })

  const sign = (message: string, privateKey: PrivateKey): Effect.Effect<Signature, CryptoError | InvalidPrivateKey> =>
    Effect.gen(function*() {
      // Validate private key format
      if (!/^[0-9a-f]{64}$/.test(privateKey)) {
        return yield* new InvalidPrivateKey({
          reason: "Private key must be 64 hex characters"
        })
      }

      return yield* Effect.try({
        try: () => {
          const messageHash = sha256(message)
          const signature = schnorr.sign(messageHash, privateKey)
          return bytesToHex(signature) as Signature
        },
        catch: (error) =>
          new CryptoError({
            operation: "sign",
            reason: String(error)
          })
      })
    })

  const verify = (signature: Signature, message: string, publicKey: PublicKey): Effect.Effect<boolean, CryptoError> =>
    Effect.try({
      try: () => {
        const messageHash = sha256(message)
        const signatureBytes = hexToBytes(signature)
        const publicKeyBytes = hexToBytes(publicKey)
        return schnorr.verify(signatureBytes, messageHash, publicKeyBytes)
      },
      catch: (error) =>
        new CryptoError({
          operation: "verify",
          reason: String(error)
        })
    })

  const hash = (message: string): Effect.Effect<EventId, CryptoError> =>
    Effect.try({
      try: () => bytesToHex(sha256(message)) as EventId,
      catch: (error) =>
        new CryptoError({
          operation: "hash",
          reason: String(error)
        })
    })

  return {
    generatePrivateKey,
    getPublicKey,
    sign,
    verify,
    hash
  }
})
)
