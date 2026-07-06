import type { KhalaStoredCredentials } from "./khala-auth-store"
import type { KhalaAuthValidation } from "./khala-auth-validate"

/** On app launch, a stored credential must be re-verified against the
 * server before the app trusts it as a signed-in session — it may be a
 * leftover token from a retired auth model (e.g. the old Tailnet-pairing
 * flow) or a since-revoked mobile OpenAuth session, and Keychain data
 * otherwise survives a TestFlight build update untouched. Pure orchestration
 * so this is testable without mounting the provider: `null` in (no stored
 * credential) always yields `null` out with no calls; a credential that
 * validates is returned unchanged; a credential that fails validation is
 * cleared and `null` is returned. */
export const resolveVerifiedStoredCredentials = async (
  storedCredentials: KhalaStoredCredentials | null,
  dependencies: {
    validate: (credentials: KhalaStoredCredentials) => Promise<KhalaAuthValidation>
    clearStoredCredentials: () => Promise<void>
  },
): Promise<KhalaStoredCredentials | null> => {
  if (storedCredentials === null) return null

  const validation = await dependencies.validate(storedCredentials)
  if (validation.ok) return storedCredentials

  await dependencies.clearStoredCredentials()
  return null
}
