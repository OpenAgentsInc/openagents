import path from "node:path"

export const IsolatedAppProofEnvironment = "OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF"
export const ProviderAccountsBootstrapReceiptEnvironment = "OPENAGENTS_DESKTOP_PROVIDER_ACCOUNTS_BOOTSTRAP_RECEIPT"
export const IsolatedAppProofWorkspaceEnvironment = "OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT"

/** Chromium switches allowed only for the double-gated temporary proof. */
export const isolatedAppProofChromiumSwitches = (enabled: boolean): ReadonlyArray<string> =>
  enabled ? ["use-mock-keychain"] : []

export const isolatedAppProofWorkspaceRoot = (input: Readonly<{
  enabled: boolean
  env: NodeJS.ProcessEnv
}>): string | null => {
  const candidate = input.env[IsolatedAppProofWorkspaceEnvironment]
  return input.enabled && candidate !== undefined && path.isAbsolute(candidate)
    ? path.resolve(candidate)
    : null
}

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

export const CodexSessionsRootEnvironment = "OPENAGENTS_DESKTOP_CODEX_SESSIONS"
export const ClaudeProjectsRootEnvironment = "OPENAGENTS_DESKTOP_CLAUDE_PROJECTS"

/**
 * Codex/Claude history-importer source-directory resolution (#8999).
 *
 * Before this fix, isolated-app-proof mode correctly scoped Electron's own
 * `userData` (session vault, mock Keychain) below the OS temp directory, but
 * the Codex-history importer computed its `sessionsRoot` independently and
 * fell straight through to the real global `~/.codex/sessions` whenever
 * smoke mode and the explicit env override were both absent — a real
 * UI-automation session under isolated-app-proof surfaced genuine unrelated
 * Codex session titles from the host machine's real history in the sidebar.
 *
 * The fix scopes the importer's source directory the same way `userData` is
 * scoped: under isolated-app-proof mode (and no smoke mode, no explicit
 * override) it resolves to a directory nested under the already-isolated
 * `userDataPath` — which `isIsolatedAppProof` has already proven sits
 * strictly beneath the OS temp directory. That directory never exists on a
 * real host, so the importer legitimately finds zero sessions rather than
 * reading the operator's real history. An explicit
 * `OPENAGENTS_DESKTOP_CODEX_SESSIONS` / `OPENAGENTS_DESKTOP_CLAUDE_PROJECTS`
 * override still wins even in isolated mode, since that is how tests point
 * the importer at deliberate fixture data.
 */
export const resolveCodexSessionsRoot = (input: Readonly<{
  env: NodeJS.ProcessEnv
  smokeMode: boolean
  isolatedAppProofMode: boolean
  smokeFixtureRoot: string
  userDataPath: string
  realHome: string
}>): string => {
  const explicit = input.env[CodexSessionsRootEnvironment]
  if (explicit !== undefined) return path.resolve(explicit)
  if (input.smokeMode) return path.resolve(path.join(input.smokeFixtureRoot, "codex-smoke", "sessions"))
  if (input.isolatedAppProofMode) return path.resolve(path.join(input.userDataPath, "isolated-codex-home", "sessions"))
  return path.resolve(path.join(input.realHome, ".codex", "sessions"))
}

/** Same isolation reasoning as {@link resolveCodexSessionsRoot}, for the
 * Claude Code history import tree (#8712 H3). `null` disables the import
 * entirely (an explicit empty-string override does this today too). */
export const resolveClaudeProjectsRoot = (input: Readonly<{
  env: NodeJS.ProcessEnv
  smokeMode: boolean
  isolatedAppProofMode: boolean
  smokeFixtureRoot: string
  userDataPath: string
  realHome: string
}>): string | null => {
  const explicit = input.env[ClaudeProjectsRootEnvironment]
  if (explicit !== undefined) return explicit === "" ? null : path.resolve(explicit)
  if (input.smokeMode) return path.resolve(path.join(input.smokeFixtureRoot, "claude-smoke", "projects"))
  if (input.isolatedAppProofMode) return path.resolve(path.join(input.userDataPath, "isolated-claude-home", "projects"))
  return path.resolve(path.join(input.realHome, ".claude", "projects"))
}
