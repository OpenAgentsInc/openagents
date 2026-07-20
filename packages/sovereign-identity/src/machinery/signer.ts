/**
 * IDR-01 sovereign signer ports — the SIGNER BOUNDARY.
 *
 * A normal Nostr caller can ONLY:
 *
 * - read the public key,
 * - sign an admitted event,
 * - NIP-44 encrypt or decrypt an admitted payload,
 * - create a NIP-98 HTTP auth token,
 * - read the public identity manifest (which proves the derivation profile).
 *
 * The interface has NO method that returns the mnemonic, the `nsec`, the raw
 * private key, or the BIP-39 seed to a normal caller. That narrowing is enforced
 * with a static import test in IDR-06; this module defines the port shape now so
 * it is the only surface a caller ever sees.
 *
 * ALIGNMENT. `NostrSignerPort` structurally matches the `LocalSignerPort` façade
 * in the workspace `nostr-effect` repo (`src/core/LocalSigner.ts`), method for
 * method and argument for argument, wrapped in `Effect` instead of `Promise`.
 * When `nostr-effect` publishes that façade, `sovereign-identity` implements
 * this port by wrapping `IdentityKeys.asSigner()`, so this stays a clean
 * interface, not a bespoke crypto implementation. The upstream port deliberately
 * omits the class-only `exportPrivateKeyBytes` / `exportNsec` escape hatches;
 * those live on the separate custody/recovery port below.
 *
 * The Spark wallet needs secret material. It never receives it through this
 * signer. It receives it only inside the bounded `SparkSecretMaterial` callback
 * port below, which passes the seed to the admitted SDK and clears its buffer.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Context, type Effect, Schema as S } from "effect";
import { DERIVATION_PROFILE_ID, Npub } from "../contract/index.ts";

/** A lowercase hex string. */
const HexString = S.String.check(S.isPattern(/^[0-9a-f]+$/));

/**
 * An unsigned event template the signer may sign. It matches the `nostr-effect`
 * `SignEventTemplate` shape. It carries no `id`, `pubkey`, or `sig`: the signer
 * owns the public key and produces the identifier and signature, so a caller can
 * never present a forged author. `created_at` is optional; the signer fills it.
 */
export const SignEventTemplate = S.Struct({
  kind: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  content: S.String,
  tags: S.Array(S.Array(S.String)),
  created_at: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
});
export type SignEventTemplate = typeof SignEventTemplate.Type;

/** A signed Nostr event. The signer fills the public author and signature. */
export const SignedNostrEvent = S.Struct({
  id: HexString,
  pubkey: HexString,
  created_at: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  kind: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  tags: S.Array(S.Array(S.String)),
  content: S.String,
  sig: HexString,
});
export type SignedNostrEvent = typeof SignedNostrEvent.Type;

/**
 * The public-only identity manifest a signer reports. It matches the
 * `nostr-effect` `PublicIdentityManifest` shape. It is safe to log or persist. It
 * carries the derivation profile id, which proves the active profile.
 */
export const PublicIdentityManifest = S.Struct({
  pubkey: HexString,
  npub: Npub,
  accountPath: S.optionalKey(S.String),
  profileId: S.optionalKey(S.String),
});
export type PublicIdentityManifest = typeof PublicIdentityManifest.Type;

/** The options a NIP-98 HTTP auth token accepts. */
export interface HttpAuthTokenOptions {
  readonly includeAuthorizationScheme?: boolean;
  readonly body?: unknown;
}

/** A proof of the active derivation profile. It carries public data only. */
export const DerivationProfileProof = S.Struct({
  derivationProfile: S.Literal(DERIVATION_PROFILE_ID),
  nostrPublicKeyHex: HexString,
});
export type DerivationProfileProof = typeof DerivationProfileProof.Type;

/** A typed signer failure. It never carries secret material. */
export class SignerError extends S.TaggedErrorClass<SignerError>()(
  "sovereign-identity.SignerError",
  {
    reason: S.Literals([
      "unavailable",
      "not_ready",
      "unauthorized",
      "invalid_event",
      "encrypt_failed",
      "decrypt_failed",
    ]),
  },
) {}

/**
 * The Nostr signer port. It structurally matches the `nostr-effect`
 * `LocalSignerPort` (Effect-wrapped). It exposes no secret-returning method by
 * construction, so it is the only surface a normal caller uses.
 */
export interface NostrSignerPort {
  /** The hex public key. */
  readonly getPublicKey: () => Effect.Effect<string, SignerError>;
  /** Sign an admitted event template, returning a fully signed event. */
  readonly signEvent: (event: SignEventTemplate) => Effect.Effect<SignedNostrEvent, SignerError>;
  /** NIP-44 encrypt a plaintext to a recipient hex public key. */
  readonly nip44Encrypt: (
    recipientPubkey: string,
    plaintext: string,
  ) => Effect.Effect<string, SignerError>;
  /** NIP-44 decrypt a ciphertext from a sender hex public key. */
  readonly nip44Decrypt: (
    senderPubkey: string,
    ciphertext: string,
  ) => Effect.Effect<string, SignerError>;
  /** Create a NIP-98 HTTP auth token for a URL and method. */
  readonly createHttpAuthToken: (
    url: string,
    method: string,
    options?: HttpAuthTokenOptions,
  ) => Effect.Effect<string, SignerError>;
  /** The public-only identity manifest, safe to log or persist. */
  readonly toPublicManifest: () => Effect.Effect<PublicIdentityManifest, SignerError>;
}

/**
 * The sovereign signer surface: the aligned `NostrSignerPort` plus a typed
 * derivation-profile proof. It adds no secret-returning method.
 */
export interface SovereignSignerInterface extends NostrSignerPort {
  /** Prove the active derivation profile without exposing any secret. */
  readonly proveDerivationProfile: () => Effect.Effect<DerivationProfileProof, SignerError>;
}

/** The `SovereignSigner` service tag. IDR-06 supplies the narrowed real layer. */
export class SovereignSigner extends Context.Service<SovereignSigner, SovereignSignerInterface>()(
  "sovereign-identity.SovereignSigner",
) {}

/**
 * The custody/recovery key-export port.
 *
 * This is the ONLY place `exportPrivateKeyBytes` and `exportNsec` live. It is a
 * SEPARATE tag from `SovereignSigner` on purpose: a normal caller resolves the
 * signer, never this port. IDR-06 keeps this port out of normal callers with a
 * static import test. A custody or recovery composition root uses it under owner
 * authority only.
 */
export interface CustodyKeyExportInterface {
  /** Export the raw 32-byte private key. Custody and recovery use only. */
  readonly exportPrivateKeyBytes: () => Effect.Effect<Uint8Array, SignerError>;
  /** Export the `nsec`. Custody and recovery use only. */
  readonly exportNsec: () => Effect.Effect<string, SignerError>;
}

/** The `CustodyKeyExport` service tag. A normal caller must never resolve it. */
export class CustodyKeyExport extends Context.Service<
  CustodyKeyExport,
  CustodyKeyExportInterface
>()("sovereign-identity.CustodyKeyExport") {}

/**
 * The bounded Spark secret-material port.
 *
 * The Spark wallet needs the seed. It never gets it from `SovereignSigner`. It
 * gets it only inside `withSeedMaterial`, which passes the seed bytes to the
 * caller's effect and then clears the temporary buffer. The seed never enters a
 * process-wide config object or a cache key. The callback returns a result, not
 * the seed, so the material never escapes the bounded scope.
 */
export interface SparkSecretMaterialInterface {
  readonly withSeedMaterial: <A, E, R>(
    use: (seed: Uint8Array) => Effect.Effect<A, E, R>,
  ) => Effect.Effect<A, E | SignerError, R>;
}

/** The `SparkSecretMaterial` service tag. IDR-07 supplies the bounded real layer. */
export class SparkSecretMaterial extends Context.Service<
  SparkSecretMaterial,
  SparkSecretMaterialInterface
>()("sovereign-identity.SparkSecretMaterial") {}
