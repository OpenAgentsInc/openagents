/**
 * Sovereign identity host (IDR-BS #9103, narrowed by IDR-06).
 *
 * A PURE projector over an injected `IdentityLoader` boundary. Before IDR-06 the
 * loader handed this host the raw BIP-39 mnemonic and the host derived the public
 * identifiers here. IDR-06 narrows that seam: the loader (the concrete one lives
 * in `main.ts`, built on the Pylon signer-boundary loader) now rehydrates or
 * creates the identity BEHIND the signer boundary and hands this host ONLY the
 * PUBLIC projection — the Nostr `npub`, the Spark wallet public fingerprint,
 * whether the identity was rehydrated or created, and the derivation profile id.
 * The raw mnemonic never crosses this boundary and never reaches the renderer.
 *
 * This host maps that public projection to the public-safe `IdentityStatus`
 * verbatim, memoizes it, and forwards it through the IPC layer. Any failure is
 * fail-soft: the host returns the `unavailable` projection.
 */
import { IDENTITY_STATUS_SCHEMA_ID, type IdentitySourceValue, type IdentityStatus, type WalletModeValue, unavailableIdentityStatus } from "./identity-contract.ts"

/**
 * The PUBLIC identity projection main hands the host. It is derived behind the
 * signer boundary (Pylon loader → `@openagentsinc/sovereign-identity` →
 * `nostr-effect` `IdentityKeys`) and carries public identifiers only — never the
 * mnemonic, `nsec`, private key, or seed.
 */
export type IdentityLoadResult = Readonly<{
  /** `rehydrated` when an existing mnemonic was opened, `created` for a fresh one. */
  source: IdentitySourceValue
  /** The Nostr NIP-06 `npub`. */
  npub: string
  /** The Spark wallet public BIP-32 fingerprint (lower-hex). */
  walletFingerprint: string
  /** The frozen derivation profile id. */
  profileId: string
  /**
   * The STATUS-ONLY Spark wallet mode (IDR-07): `status_only` when the app-side
   * Spark adapter opened the recovered wallet, null when it could not. Public.
   */
  walletMode?: WalletModeValue | null
}>

/** The injected, impure boundary that rehydrates-or-creates the identity. */
export type IdentityLoader = Readonly<{
  loadOrCreate: () => Promise<IdentityLoadResult>
}>

export type IdentityHost = Readonly<{
  /** Rehydrate-or-create on demand, project the public status, and memoize it. */
  status: () => Promise<IdentityStatus>
}>

export const createIdentityHost = (loader: IdentityLoader): IdentityHost => {
  let cached: IdentityStatus | null = null
  return {
    status: async (): Promise<IdentityStatus> => {
      if (cached !== null) return cached
      try {
        const loaded = await loader.loadOrCreate()
        cached = {
          schema: IDENTITY_STATUS_SCHEMA_ID,
          status: "available",
          npub: loaded.npub,
          walletFingerprint: loaded.walletFingerprint,
          source: loaded.source,
          profileId: loaded.profileId,
          walletMode: loaded.walletMode ?? null,
        }
        return cached
      } catch {
        // Fail-soft: never surface an error shape or private material.
        return unavailableIdentityStatus()
      }
    },
  }
}
