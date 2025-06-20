/**
 * NIP-06: Basic key derivation from mnemonic seed phrase
 * @module
 */
import { Context, Effect, Layer } from "effect";
import { InvalidMnemonic, KeyDerivationError, Nip06Error } from "../core/Errors.js";
import type { DerivationPath, Mnemonic, Npub, Nsec, PrivateKey, PublicKey } from "../core/Schema.js";
import { KeyDerivationResult } from "../core/Schema.js";
import { CryptoService } from "../services/CryptoService.js";
declare const Nip06Service_base: Context.TagClass<Nip06Service, "nostr/Nip06Service", {
    /**
     * Generate a new BIP39 mnemonic phrase
     */
    readonly generateMnemonic: (wordCount?: 12 | 15 | 18 | 21 | 24) => Effect.Effect<Mnemonic, Nip06Error>;
    /**
     * Validate a BIP39 mnemonic phrase
     */
    readonly validateMnemonic: (mnemonic: string) => Effect.Effect<boolean, Nip06Error>;
    /**
     * Derive private key from mnemonic using NIP-06 path
     */
    readonly derivePrivateKey: (mnemonic: Mnemonic, account?: number) => Effect.Effect<PrivateKey, Nip06Error | InvalidMnemonic | KeyDerivationError>;
    /**
     * Derive public key from private key (delegates to CryptoService)
     */
    readonly derivePublicKey: (privateKey: PrivateKey) => Effect.Effect<PublicKey, Nip06Error>;
    /**
     * Encode private key as nsec (bech32)
     */
    readonly encodeNsec: (privateKey: PrivateKey) => Effect.Effect<Nsec, Nip06Error>;
    /**
     * Encode public key as npub (bech32)
     */
    readonly encodeNpub: (publicKey: PublicKey) => Effect.Effect<Npub, Nip06Error>;
    /**
     * Decode nsec to private key
     */
    readonly decodeNsec: (nsec: Nsec) => Effect.Effect<PrivateKey, Nip06Error>;
    /**
     * Decode npub to public key
     */
    readonly decodeNpub: (npub: Npub) => Effect.Effect<PublicKey, Nip06Error>;
    /**
     * Derive all keys from mnemonic (comprehensive derivation)
     */
    readonly deriveAllKeys: (mnemonic: Mnemonic, account?: number) => Effect.Effect<KeyDerivationResult, Nip06Error | InvalidMnemonic | KeyDerivationError>;
    /**
     * Get BIP32 derivation path for account
     */
    readonly getDerivationPath: (account?: number) => Effect.Effect<DerivationPath, Nip06Error>;
}>;
/**
 * Service for NIP-06 key derivation operations
 */
export declare class Nip06Service extends Nip06Service_base {
}
/**
 * Live implementation of Nip06Service
 */
export declare const Nip06ServiceLive: Layer.Layer<Nip06Service, never, CryptoService>;
export {};
//# sourceMappingURL=Nip06Service.d.ts.map