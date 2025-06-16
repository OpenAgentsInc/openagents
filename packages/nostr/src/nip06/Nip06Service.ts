/**
 * NIP-06: Basic key derivation from mnemonic seed phrase
 * @module
 */

import { bytesToHex, hexToBytes, randomBytes } from "@noble/hashes/utils"
import { bech32 } from "@scure/base"
import { HDKey } from "@scure/bip32"
import * as bip39 from "@scure/bip39"
import { wordlist } from "@scure/bip39/wordlists/english"
import { Context, Effect, Layer } from "effect"
import { InvalidMnemonic, KeyDerivationError, Nip06Error } from "../core/Errors.js"
import type { DerivationPath, Mnemonic, Npub, Nsec, PrivateKey, PublicKey } from "../core/Schema.js"
import { KeyDerivationResult } from "../core/Schema.js"
import { CryptoService } from "../services/CryptoService.js"

/**
 * Service for NIP-06 key derivation operations
 */
export class Nip06Service extends Context.Tag("nostr/Nip06Service")<
  Nip06Service,
  {
    /**
     * Generate a new BIP39 mnemonic phrase
     */
    readonly generateMnemonic: (
      wordCount?: 12 | 15 | 18 | 21 | 24
    ) => Effect.Effect<Mnemonic, Nip06Error>

    /**
     * Validate a BIP39 mnemonic phrase
     */
    readonly validateMnemonic: (mnemonic: string) => Effect.Effect<boolean, Nip06Error>

    /**
     * Derive private key from mnemonic using NIP-06 path
     */
    readonly derivePrivateKey: (
      mnemonic: Mnemonic,
      account?: number
    ) => Effect.Effect<PrivateKey, Nip06Error | InvalidMnemonic | KeyDerivationError>

    /**
     * Derive public key from private key (delegates to CryptoService)
     */
    readonly derivePublicKey: (
      privateKey: PrivateKey
    ) => Effect.Effect<PublicKey, Nip06Error>

    /**
     * Encode private key as nsec (bech32)
     */
    readonly encodeNsec: (privateKey: PrivateKey) => Effect.Effect<Nsec, Nip06Error>

    /**
     * Encode public key as npub (bech32)
     */
    readonly encodeNpub: (publicKey: PublicKey) => Effect.Effect<Npub, Nip06Error>

    /**
     * Decode nsec to private key
     */
    readonly decodeNsec: (nsec: Nsec) => Effect.Effect<PrivateKey, Nip06Error>

    /**
     * Decode npub to public key
     */
    readonly decodeNpub: (npub: Npub) => Effect.Effect<PublicKey, Nip06Error>

    /**
     * Derive all keys from mnemonic (comprehensive derivation)
     */
    readonly deriveAllKeys: (
      mnemonic: Mnemonic,
      account?: number
    ) => Effect.Effect<KeyDerivationResult, Nip06Error | InvalidMnemonic | KeyDerivationError>

    /**
     * Get BIP32 derivation path for account
     */
    readonly getDerivationPath: (account?: number) => Effect.Effect<DerivationPath, Nip06Error>
  }
>() {}

/**
 * Live implementation of Nip06Service
 */
export const Nip06ServiceLive = Layer.effect(
  Nip06Service,
  Effect.gen(function*() {
    const crypto = yield* CryptoService

    const generateMnemonic = (
      wordCount: 12 | 15 | 18 | 21 | 24 = 12
    ): Effect.Effect<Mnemonic, Nip06Error> =>
      Effect.try({
        try: () => {
          // Map word count to entropy bytes
          const entropyBytesMap = {
            12: 16, // 128 bits
            15: 20, // 160 bits
            18: 24, // 192 bits
            21: 28, // 224 bits
            24: 32 // 256 bits
          }

          const entropyBytes = entropyBytesMap[wordCount]
          if (!entropyBytes) {
            throw new Error(`Invalid word count: ${wordCount}`)
          }

          // Generate secure random entropy
          const entropy = randomBytes(entropyBytes)

          const mnemonic = bip39.entropyToMnemonic(entropy, wordlist)
          return mnemonic as Mnemonic
        },
        catch: (error) =>
          new Nip06Error({
            operation: "generateMnemonic",
            reason: `Failed to generate mnemonic: ${String(error)}`
          })
      })

    const validateMnemonic = (mnemonic: string): Effect.Effect<boolean, Nip06Error> =>
      Effect.try({
        try: () => bip39.validateMnemonic(mnemonic, wordlist),
        catch: (error) =>
          new Nip06Error({
            operation: "validateMnemonic",
            reason: `Failed to validate mnemonic: ${String(error)}`
          })
      })

    const getDerivationPath = (account = 0): Effect.Effect<DerivationPath, Nip06Error> =>
      Effect.try({
        try: () => `m/44'/1237'/${account}'/0/0` as DerivationPath,
        catch: (error) =>
          new Nip06Error({
            operation: "deriveKey",
            reason: `Failed to create derivation path: ${String(error)}`
          })
      })

    const derivePrivateKey = (
      mnemonic: Mnemonic,
      account = 0
    ): Effect.Effect<PrivateKey, Nip06Error | InvalidMnemonic | KeyDerivationError> =>
      Effect.gen(function*() {
        // Validate mnemonic first
        const isValid = yield* validateMnemonic(mnemonic)
        if (!isValid) {
          return yield* new InvalidMnemonic({
            mnemonic,
            reason: "Invalid BIP39 mnemonic phrase"
          })
        }

        const path = yield* getDerivationPath(account)

        return yield* Effect.try({
          try: () => {
            // Convert mnemonic to seed
            const seed = bip39.mnemonicToSeedSync(mnemonic)

            // Create master key and derive
            const masterKey = HDKey.fromMasterSeed(seed)
            const derived = masterKey.derive(path)

            if (!derived.privateKey) {
              throw new Error("Failed to derive private key")
            }

            return bytesToHex(derived.privateKey) as PrivateKey
          },
          catch: (error) =>
            new KeyDerivationError({
              path,
              reason: `Key derivation failed: ${String(error)}`
            })
        })
      })

    const derivePublicKey = (privateKey: PrivateKey): Effect.Effect<PublicKey, Nip06Error> =>
      crypto.getPublicKey(privateKey).pipe(
        Effect.mapError((error) =>
          new Nip06Error({
            operation: "deriveKey",
            reason: `Failed to derive public key: ${String(error)}`
          })
        )
      )

    const encodeNsec = (privateKey: PrivateKey): Effect.Effect<Nsec, Nip06Error> =>
      Effect.try({
        try: () => {
          const bytes = hexToBytes(privateKey)
          const words = bech32.toWords(bytes)
          const encoded = bech32.encode("nsec", words)
          return encoded as Nsec
        },
        catch: (error) =>
          new Nip06Error({
            operation: "encodeKey",
            reason: `Failed to encode nsec: ${String(error)}`
          })
      })

    const encodeNpub = (publicKey: PublicKey): Effect.Effect<Npub, Nip06Error> =>
      Effect.try({
        try: () => {
          const bytes = hexToBytes(publicKey)
          const words = bech32.toWords(bytes)
          const encoded = bech32.encode("npub", words)
          return encoded as Npub
        },
        catch: (error) =>
          new Nip06Error({
            operation: "encodeKey",
            reason: `Failed to encode npub: ${String(error)}`
          })
      })

    const decodeNsec = (nsec: Nsec): Effect.Effect<PrivateKey, Nip06Error> =>
      Effect.try({
        try: () => {
          const nsecStr = nsec as string
          // Use a type assertion to work around strict typing
          const { prefix, words } = (bech32.decode as any)(nsecStr, 90)
          if (prefix !== "nsec") {
            throw new Error(`Invalid prefix: expected 'nsec', got '${prefix}'`)
          }
          const bytes = bech32.fromWords(words)
          return bytesToHex(new Uint8Array(bytes)) as PrivateKey
        },
        catch: (error) =>
          new Nip06Error({
            operation: "decodeKey",
            reason: `Failed to decode nsec: ${String(error)}`
          })
      })

    const decodeNpub = (npub: Npub): Effect.Effect<PublicKey, Nip06Error> =>
      Effect.try({
        try: () => {
          const npubStr = npub as string
          // Use a type assertion to work around strict typing
          const { prefix, words } = (bech32.decode as any)(npubStr, 90)
          if (prefix !== "npub") {
            throw new Error(`Invalid prefix: expected 'npub', got '${prefix}'`)
          }
          const bytes = bech32.fromWords(words)
          return bytesToHex(new Uint8Array(bytes)) as PublicKey
        },
        catch: (error) =>
          new Nip06Error({
            operation: "decodeKey",
            reason: `Failed to decode npub: ${String(error)}`
          })
      })

    const deriveAllKeys = (
      mnemonic: Mnemonic,
      account = 0
    ): Effect.Effect<KeyDerivationResult, Nip06Error | InvalidMnemonic | KeyDerivationError> =>
      Effect.gen(function*() {
        const privateKey = yield* derivePrivateKey(mnemonic, account)
        const publicKey = yield* derivePublicKey(privateKey)
        const nsec = yield* encodeNsec(privateKey)
        const npub = yield* encodeNpub(publicKey)

        return new KeyDerivationResult({
          privateKey,
          publicKey,
          nsec,
          npub
        })
      })

    return {
      generateMnemonic,
      validateMnemonic,
      derivePrivateKey,
      derivePublicKey,
      encodeNsec,
      encodeNpub,
      decodeNsec,
      decodeNpub,
      deriveAllKeys,
      getDerivationPath
    }
  })
)
