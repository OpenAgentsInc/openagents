/**
 * IDR-06 — the narrowed real signer, backed by the `nostr-effect` `IdentityKeys`
 * façade.
 *
 * This module is the adoption seam that scope IDR-06 requires: instead of a
 * hand-rolled parallel Nostr derivation, `sovereign-identity` derives the public
 * identity and the signer from the audited `nostr-effect` façade
 * (`nostr-effect/identity`). The engine underneath is `IdentityKeys`; the typed
 * boundary above stays this package's own.
 *
 * SECRET BOUNDARY. `deriveLocalNostrIdentity` returns ONLY the public
 * identifiers plus a `LocalSignerPort`. The `LocalSignerPort` object from
 * `IdentityKeys.asSigner()` exposes exactly six operations — `getPublicKey`,
 * `signEvent`, `nip44Encrypt`, `nip44Decrypt`, `createHttpAuthToken`,
 * `toPublicManifest` — and NO `exportPrivateKeyBytes` / `exportNsec`. The raw
 * `IdentityKeys` instance (which does carry the escape hatches) is discarded
 * here; the private key survives only inside the closure the signer holds. The
 * secret-returning custody port lives in the separate, unexported
 * `./custody.ts` module.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect, Layer, Schema as S } from "effect";
import { IdentityKeys, type LocalSignerPort } from "nostr-effect/identity";
import { DERIVATION_PROFILE_ID } from "../contract/index.ts";
import {
  PublicIdentityManifest,
  SignerError,
  SovereignSigner,
  type SovereignSignerInterface,
} from "./signer.ts";

/**
 * The `nostr-effect` local signer port, re-exported so a non-Effect host (the
 * Pylon and Desktop identity loaders) can name the signer type without importing
 * `nostr-effect` directly. This is the ONLY signer surface a normal caller sees.
 */
export type { LocalSignerPort } from "nostr-effect/identity";

/**
 * A public local Nostr identity: the public identifiers plus the signer. It
 * carries no mnemonic, no `nsec`, no raw private key, and no seed.
 */
export interface LocalNostrIdentity {
  /** The hex x-only Nostr public key. */
  readonly publicKey: string;
  /** The NIP-19 `npub`. */
  readonly npub: string;
  /** The active derivation profile id. */
  readonly profileId: string;
  /** The NIP-06 account path, when the façade reports one. */
  readonly accountPath: string | undefined;
  /** The narrow signer surface (`getPublicKey`/`signEvent`/NIP-44/NIP-98/manifest). */
  readonly signer: LocalSignerPort;
}

/**
 * Derive the public local Nostr identity and its signer from a mnemonic under
 * the frozen OpenAgents legacy profile (NIP-06 account zero, empty passphrase).
 * The `IdentityKeys` instance is consumed to produce the public manifest and the
 * narrow signer, then discarded; the mnemonic is never re-emitted.
 */
export const deriveLocalNostrIdentity = (mnemonic: string): LocalNostrIdentity => {
  const keys = IdentityKeys.fromOpenAgentsLegacyMnemonic(mnemonic);
  const manifest = keys.toPublicManifest();
  return {
    publicKey: manifest.pubkey,
    npub: manifest.npub,
    profileId: manifest.profileId ?? DERIVATION_PROFILE_ID,
    accountPath: manifest.accountPath,
    // `asSigner()` returns a NEW object that exposes only the six port methods.
    // The escape-hatch methods on `keys` are not reachable through it.
    signer: keys.asSigner(),
  };
};

/**
 * Build the Effect `SovereignSignerInterface` over a mnemonic. It wraps the
 * `nostr-effect` `LocalSignerPort` (Promise-based) into the Effect-typed port
 * this package exposes, mapping any provider failure to a typed `SignerError`
 * that never carries secret material. This is the narrowed real signer the
 * IDR-01 `SovereignSigner` tag was reserved for.
 */
export const makeSovereignSignerFromMnemonic = (mnemonic: string): SovereignSignerInterface => {
  const { signer, publicKey, npub, profileId, accountPath } = deriveLocalNostrIdentity(mnemonic);

  const manifest = S.decodeSync(PublicIdentityManifest)({
    pubkey: publicKey,
    npub,
    ...(accountPath === undefined ? {} : { accountPath }),
    profileId,
  });

  const fail = (reason: SignerError["reason"]) => new SignerError({ reason });

  return {
    getPublicKey: () =>
      Effect.tryPromise({ try: () => signer.getPublicKey(), catch: () => fail("unavailable") }),
    signEvent: (event) =>
      Effect.tryPromise({ try: () => signer.signEvent(event), catch: () => fail("invalid_event") }).pipe(
        Effect.map((signed) => ({
          id: signed.id,
          pubkey: signed.pubkey,
          created_at: signed.created_at,
          kind: signed.kind,
          tags: signed.tags.map((tag) => [...tag]),
          content: signed.content,
          sig: signed.sig,
        })),
      ),
    nip44Encrypt: (recipientPubkey, plaintext) =>
      Effect.tryPromise({
        try: () => signer.nip44Encrypt(recipientPubkey, plaintext),
        catch: () => fail("encrypt_failed"),
      }),
    nip44Decrypt: (senderPubkey, ciphertext) =>
      Effect.tryPromise({
        try: () => signer.nip44Decrypt(senderPubkey, ciphertext),
        catch: () => fail("decrypt_failed"),
      }),
    createHttpAuthToken: (url, method, options) =>
      Effect.tryPromise({
        try: () => signer.createHttpAuthToken(url, method, options),
        catch: () => fail("unauthorized"),
      }),
    toPublicManifest: () => Effect.succeed(manifest),
    proveDerivationProfile: () =>
      Effect.succeed({
        derivationProfile: DERIVATION_PROFILE_ID,
        nostrPublicKeyHex: publicKey,
      }),
  };
};

/**
 * The narrowed real `SovereignSigner` layer for one mnemonic. A custody or
 * identity composition root builds it after loading the root secret; a normal
 * caller resolves the resulting `SovereignSigner` tag and never sees the
 * mnemonic.
 */
export const sovereignSignerFromMnemonicLayer = (mnemonic: string): Layer.Layer<SovereignSigner> =>
  Layer.succeed(SovereignSigner, SovereignSigner.of(makeSovereignSignerFromMnemonic(mnemonic)));
