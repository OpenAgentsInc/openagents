/**
 * Desktop sovereign-identity projection (IDR-08).
 *
 * Desktop is a CONSUMER of the ONE shared local identity service. It resolves the
 * identity through the same Pylon loader that routes derivation through
 * `@openagentsinc/sovereign-identity`, and this pure function projects that
 * resolved `PylonNostrIdentity` into the public-safe `IdentityLoadResult` the
 * boot-sequence host consumes.
 *
 * The projection carries the ONE canonical `identityRef` and `npub` from the
 * shared service verbatim, plus the public Spark fingerprint, the frozen profile
 * id, and the status-only wallet mode (IDR-07). It never touches the mnemonic,
 * `nsec`, raw private key, or seed — those never leave the Pylon loader's bounded
 * scope. Extracting this projection makes the one-identity proof testable without
 * launching Electron.
 */
import type { PylonNostrIdentity } from "@openagentsinc/pylon-core/shared/nostr-identity"
import type { IdentityLoadResult } from "./identity-host.ts"
import type { IdentitySourceValue } from "./identity-contract.ts"

/**
 * Project a resolved `PylonNostrIdentity` into the Desktop public identity load
 * result. `source` records whether the mnemonic existed (`rehydrated`) or was
 * freshly created (`created`) this run; the loader determines it before any
 * create path runs.
 */
export const projectDesktopIdentity = (
  identity: PylonNostrIdentity,
  source: IdentitySourceValue,
): IdentityLoadResult => ({
  source,
  identityRef: identity.identityRef,
  npub: identity.npub,
  walletFingerprint: identity.sparkFingerprint,
  profileId: identity.profileId,
  // IDR-07: the STATUS-ONLY Spark wallet mode from the app-side adapter's public
  // projection. `status_only` when the recovered wallet opened; null otherwise.
  walletMode: identity.sparkWallet?.mode ?? null,
})
