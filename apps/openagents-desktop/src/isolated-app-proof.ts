import path from "node:path"

export const IsolatedAppProofEnvironment = "OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF"
export const ProviderAccountsBootstrapReceiptEnvironment = "OPENAGENTS_DESKTOP_PROVIDER_ACCOUNTS_BOOTSTRAP_RECEIPT"

/**
 * Allows a signed candidate to exercise local coding surfaces without opening
 * macOS Keychain UI. The escape hatch is intentionally double-gated: an
 * explicit test-only environment flag and user data strictly below the OS
 * temporary directory. Production/default user-data paths can never opt in.
 */
export const isIsolatedAppProof = (input: Readonly<{
  env: NodeJS.ProcessEnv
  userDataPath: string
  temporaryDirectory: string
}>): boolean => {
  if (input.env[IsolatedAppProofEnvironment] !== "1") return false
  const relative = path.relative(
    path.resolve(input.temporaryDirectory),
    path.resolve(input.userDataPath),
  )
  return relative.length > 0 && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)
}

export const isolatedProofReceiptPath = (input: Readonly<{
  env: NodeJS.ProcessEnv
  temporaryDirectory: string
}>): string | null => {
  const candidate = input.env[ProviderAccountsBootstrapReceiptEnvironment]
  if (candidate === undefined || candidate.trim() === "") return null
  const resolved = path.resolve(candidate)
  const relative = path.relative(path.resolve(input.temporaryDirectory), resolved)
  return relative.length > 0 && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)
    ? resolved
    : null
}
