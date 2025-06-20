/**
 * Cryptographic operations for Nostr
 * @module
 */
import { Context, Effect, Layer } from "effect";
import { CryptoError, InvalidPrivateKey } from "../core/Errors.js";
import type { EventId, PrivateKey, PublicKey, Signature } from "../core/Schema.js";
declare const CryptoService_base: Context.TagClass<CryptoService, "nostr/CryptoService", {
    /**
     * Generate a new private key
     */
    readonly generatePrivateKey: () => Effect.Effect<PrivateKey, CryptoError>;
    /**
     * Get public key from private key
     */
    readonly getPublicKey: (privateKey: PrivateKey) => Effect.Effect<PublicKey, CryptoError | InvalidPrivateKey>;
    /**
     * Sign a message with a private key
     */
    readonly sign: (message: string, privateKey: PrivateKey) => Effect.Effect<Signature, CryptoError | InvalidPrivateKey>;
    /**
     * Verify a signature
     */
    readonly verify: (signature: Signature, message: string, publicKey: PublicKey) => Effect.Effect<boolean, CryptoError>;
    /**
     * Hash a message to get event ID
     */
    readonly hash: (message: string) => Effect.Effect<EventId, CryptoError>;
}>;
/**
 * Service for cryptographic operations
 */
export declare class CryptoService extends CryptoService_base {
}
/**
 * Live implementation of CryptoService
 */
export declare const CryptoServiceLive: Layer.Layer<CryptoService, never, never>;
export {};
//# sourceMappingURL=CryptoService.d.ts.map