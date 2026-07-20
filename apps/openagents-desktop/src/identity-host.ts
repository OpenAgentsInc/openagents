/**
 * Sovereign identity host (IDR-BS, #9103).
 *
 * A PURE projector over an injected `IdentityLoader` boundary. The loader (the
 * concrete one lives in `main.ts`, built on the existing Pylon identity loader)
 * rehydrates an existing BIP-39 mnemonic or creates a new one, and hands this
 * host ONLY what main already holds: the mnemonic plus whether it was rehydrated
 * or created. This host derives the PUBLIC identifiers with the frozen IDR-00
 * derivation (`@openagentsinc/sovereign-identity`) and emits the public-safe
 * `IdentityStatus` projection verbatim, so the IPC layer forwards it without
 * re-deriving any private state.
 *
 * The mnemonic never leaves this boundary: it is consumed to derive the public
 * `npub` + Spark fingerprint and then discarded. The returned projection carries
 * public identifiers only — never the mnemonic, `nsec`, private key, or seed.
 * Any failure is fail-soft: the host returns the `unavailable` projection.
 */
import { deriveSovereignIdentityPublic } from "@openagentsinc/sovereign-identity"

import { IDENTITY_STATUS_SCHEMA_ID, type IdentitySourceValue, type IdentityStatus, unavailableIdentityStatus } from "./identity-contract.ts"

/**
 * The private load result main hands the host. The mnemonic is consumed to
 * derive public identifiers and is never re-emitted.
 */
export type IdentityLoadResult = Readonly<{
  source: IdentitySourceValue
  /** The BIP-39 mnemonic. Consumed for public derivation only; never re-emitted. */
  mnemonic: string
}>

/** The injected, impure boundary that rehydrates-or-creates the mnemonic. */
export type IdentityLoader = Readonly<{
  loadOrCreate: () => Promise<IdentityLoadResult>
}>

export type IdentityHost = Readonly<{
  /** Rehydrate-or-create on demand, derive the public projection, and memoize it. */
  status: () => Promise<IdentityStatus>
}>

/**
 * Thin internal seam for the Nostr public identity (IDR-BS #9103). Today it uses
 * IDR-00's frozen `@openagentsinc/sovereign-identity` derivation. When the
 * nostr-effect `IdentityKeys` façade is published (`nostr-effect/identity`, not
 * yet on npm), swapping the body to
 * `IdentityKeys.fromOpenAgentsLegacyMnemonic(mnemonic).toPublicManifest()` is a
 * one-line change: that façade uses the SAME NIP-06 path (`m/44'/1237'/0'/0/0`),
 * empty passphrase, and `openagents.legacy_unified_nostr_spark.v1` profile, so
 * it produces the same frozen `npub`/`pubkey` vectors. The Spark fingerprint
 * stays on IDR-00 (nostr-effect is Nostr-only).
 */
export const deriveNostrPublicIdentity = (mnemonic: string): { readonly npub: string; readonly pubkey: string } => {
  const pub = deriveSovereignIdentityPublic(mnemonic)
  return { npub: pub.npub, pubkey: pub.nostrPublicKeyHex }
}

export const createIdentityHost = (loader: IdentityLoader): IdentityHost => {
  let cached: IdentityStatus | null = null
  return {
    status: async (): Promise<IdentityStatus> => {
      if (cached !== null) return cached
      try {
        const loaded = await loader.loadOrCreate()
        const spark = deriveSovereignIdentityPublic(loaded.mnemonic)
        const nostr = deriveNostrPublicIdentity(loaded.mnemonic)
        cached = {
          schema: IDENTITY_STATUS_SCHEMA_ID,
          status: "available",
          npub: nostr.npub,
          walletFingerprint: spark.sparkBip32FingerprintHex,
          source: loaded.source,
          profileId: spark.derivationProfile,
        }
        return cached
      } catch {
        // Fail-soft: never surface an error shape or private material.
        return unavailableIdentityStatus()
      }
    },
  }
}
