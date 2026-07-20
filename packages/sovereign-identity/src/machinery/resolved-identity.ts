/**
 * IDR-08 — the ONE resolved local-identity projection.
 *
 * This module is the single, shared "identity service" surface every local
 * application consumes. Given a shared-root mnemonic, it resolves the ONE
 * canonical public identity: the `identityRef` (the `npub`), the Nostr public
 * key, the Spark wallet public fingerprint, the frozen derivation profile id,
 * and the narrow signer. Both Pylon and Desktop route their identity/signer
 * derivation through this one function, so no surface can fork a second,
 * divergent identity path — there is one `identityRef` and one `npub` across
 * every admitted local surface.
 *
 * SECRET BOUNDARY. Exactly like `deriveLocalNostrIdentity`, the projection
 * carries only PUBLIC identifiers plus a `LocalSignerPort`. The `mnemonic`
 * argument lives only inside this bounded scope; the return never carries the
 * mnemonic, `nsec`, raw private key, or seed. The wallet SDK is NOT linked here
 * (the boundary test forbids it); a surface that needs a status-only wallet
 * layers its own bounded adapter on top of this projection.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { deriveIdentityRef, deriveSovereignIdentityPublic } from "../contract/index.ts";
import { deriveLocalNostrIdentity, type LocalSignerPort } from "./local-signer.ts";

/**
 * The ONE resolved local identity: the canonical `identityRef`, the public Nostr
 * identifiers, the public Spark wallet fingerprint, the frozen profile id, and
 * the narrow signer. Public data plus the signer only — never a secret.
 */
export interface ResolvedLocalIdentity {
  /** The ONE canonical cross-surface identity reference (the `npub`). */
  readonly identityRef: string;
  /** The Nostr NIP-19 `npub`. Equal to `identityRef` under the frozen profile. */
  readonly npub: string;
  /** The Nostr NIP-06 x-only public key (hex). */
  readonly publicKey: string;
  /** The PUBLIC Spark wallet BIP-32 fingerprint (hex). Safe to display/persist. */
  readonly sparkFingerprint: string;
  /** The active (frozen) derivation profile id. */
  readonly profileId: string;
  /** The NIP-06 account path, when the façade reports one. */
  readonly accountPath: string | undefined;
  /** The narrow signer surface — signer operations only, no secret-returning method. */
  readonly signer: LocalSignerPort;
}

/**
 * Resolve the ONE public local identity from a shared-root mnemonic. This is the
 * single derivation both application surfaces consume: the Nostr identity and
 * signer come from the audited `nostr-effect` `IdentityKeys` façade, the public
 * Spark fingerprint from the frozen reference, and the `identityRef` from the ONE
 * canonical `deriveIdentityRef`. The three agree by construction, so every
 * surface that resolves the same mnemonic gets the identical `identityRef` and
 * `npub`.
 */
export function resolveLocalIdentityPublic(mnemonic: string): ResolvedLocalIdentity {
  const nostr = deriveLocalNostrIdentity(mnemonic);
  const spark = deriveSovereignIdentityPublic(mnemonic);
  const identityRef = deriveIdentityRef(mnemonic);
  return {
    identityRef,
    npub: nostr.npub,
    publicKey: nostr.publicKey,
    sparkFingerprint: spark.sparkBip32FingerprintHex,
    profileId: nostr.profileId,
    accountPath: nostr.accountPath,
    signer: nostr.signer,
  };
}
