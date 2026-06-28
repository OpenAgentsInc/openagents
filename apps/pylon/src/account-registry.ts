import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { readdir, readFile, stat } from "node:fs/promises"
import { homedir } from "node:os"
import { join, resolve } from "node:path"

export type PylonAccountProvider = "codex" | "claude_agent"
export type PylonAccountSelectorKind = "registry_ref" | "direct_home"

export type PylonAccountRegistryEntry = {
  ref: string
  provider: PylonAccountProvider
  home: string
}

export type PylonAccountSelectionInput = {
  provider: PylonAccountProvider
  accountRef?: string
  accountHome?: string
}

export type ResolvedPylonAccountSelection = {
  provider: PylonAccountProvider
  selector: PylonAccountSelectorKind
  accountRef: string | null
  accountRefHash: string
  home: string
}

export type PublicPylonAccountSelection = {
  provider: PylonAccountProvider
  selector: PylonAccountSelectorKind
  accountRefHash: string
}

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

export class PylonAccountSelectionError extends Error {
  readonly blockerRef: string

  constructor(message: string, blockerRef: string) {
    super(message)
    this.name = "PylonAccountSelectionError"
    this.blockerRef = blockerRef
  }
}

function accountRefIsValid(value: unknown): value is string {
  return typeof value === "string" && accountRefPattern.test(value)
}

function providerFrom(value: unknown): PylonAccountProvider | null {
  return value === "codex" || value === "claude_agent" ? value : null
}

export function normalizeAccountHome(value: string): string {
  const trimmed = value.trim()
  if (trimmed === "~") return homedir()
  if (trimmed.startsWith("~/")) return resolve(homedir(), trimmed.slice(2))
  return resolve(trimmed)
}

export function hashPylonAccountRef(provider: PylonAccountProvider, value: string): string {
  return `account.pylon.${provider}.${createHash("sha256").update(`${provider}:${value}`).digest("hex").slice(0, 24)}`
}

export async function loadPylonAccountRegistry(
  summary: { paths: { config: string } },
): Promise<PylonAccountRegistryEntry[]> {
  try {
    const raw = JSON.parse(await readFile(summary.paths.config, "utf8")) as { dev?: unknown }
    const section = raw.dev
    if (section === null || typeof section !== "object") return []
    const accounts = (section as { accounts?: unknown }).accounts
    if (!Array.isArray(accounts)) return []
    const entries: PylonAccountRegistryEntry[] = []
    for (const account of accounts) {
      if (account === null || typeof account !== "object") continue
      const record = account as Record<string, unknown>
      const provider = providerFrom(record.provider)
      if (!provider || !accountRefIsValid(record.ref) || typeof record.home !== "string") continue
      const home = record.home.trim()
      if (home.length === 0) continue
      entries.push({
        provider,
        ref: record.ref,
        home: normalizeAccountHome(home),
      })
    }
    return entries
  } catch {
    return []
  }
}

async function assertAccountHomePresent(home: string): Promise<void> {
  try {
    const info = await stat(home)
    if (info.isDirectory()) return
  } catch {
    // normalized below
  }
  throw new PylonAccountSelectionError(
    "Pylon account home is missing",
    "blocker.pylon.account_home_missing",
  )
}

export async function resolvePylonAccountSelection(
  summary: { paths: { config: string } },
  input: PylonAccountSelectionInput,
): Promise<ResolvedPylonAccountSelection | null> {
  const accountRef = input.accountRef?.trim()
  const accountHome = input.accountHome?.trim()
  if (accountRef && accountHome) {
    throw new PylonAccountSelectionError(
      "Use either accountRef or accountHome, not both",
      "blocker.pylon.account_selector_ambiguous",
    )
  }
  if (accountRef) {
    if (!accountRefIsValid(accountRef)) {
      throw new PylonAccountSelectionError(
        "Pylon account ref is invalid",
        "blocker.pylon.account_ref_invalid",
      )
    }
    const registry = await loadPylonAccountRegistry(summary)
    const entry = registry.find(
      candidate => candidate.provider === input.provider && candidate.ref === accountRef,
    )
    if (!entry) {
      const sibling = (await discoverPylonSiblingAccountHomes()).find(
        candidate => candidate.provider === input.provider && candidate.ref === accountRef,
      )
      if (!sibling) {
        throw new PylonAccountSelectionError(
          "Pylon account ref is not registered for this provider",
          "blocker.pylon.account_ref_unknown",
        )
      }
      await assertAccountHomePresent(sibling.home)
      return {
        provider: input.provider,
        selector: "registry_ref",
        accountRef,
        accountRefHash: hashPylonAccountRef(input.provider, sibling.home),
        home: sibling.home,
      }
    }
    await assertAccountHomePresent(entry.home)
    return {
      provider: input.provider,
      selector: "registry_ref",
      accountRef,
      accountRefHash: hashPylonAccountRef(input.provider, accountRef),
      home: entry.home,
    }
  }
  if (accountHome) {
    const home = normalizeAccountHome(accountHome)
    await assertAccountHomePresent(home)
    return {
      provider: input.provider,
      selector: "direct_home",
      accountRef: null,
      accountRefHash: hashPylonAccountRef(input.provider, home),
      home,
    }
  }
  return null
}

/**
 * Local, untracked token file inside a pooled Claude account home. It carries a
 * long-lived `claude setup-token` OAuth token (one line, raw). It is the Claude
 * analogue of a Codex home's `auth.json`: on macOS the Keychain holds only one
 * `/login` credential, so a per-account token is how multiple Claude accounts
 * coexist. Read only when building a child session env; never projected.
 */
export const PYLON_CLAUDE_OAUTH_TOKEN_FILE = "claude-oauth-token"

function readClaudeOauthToken(home: string): string | null {
  try {
    const token = readFileSync(join(home, PYLON_CLAUDE_OAUTH_TOKEN_FILE), "utf8").trim()
    return token.length > 0 ? token : null
  } catch {
    return null
  }
}

export async function pylonClaudeAccountHomeHasAuth(home: string): Promise<boolean> {
  try {
    const info = await stat(join(home, PYLON_CLAUDE_OAUTH_TOKEN_FILE))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

export async function discoverPylonSiblingAccountHomes(
  env: Record<string, string | undefined> = process.env,
): Promise<{ provider: PylonAccountProvider; home: string; ref: string }[]> {
  const root = (env.PYLON_ACCOUNT_HOME_ROOT ?? "").trim() || homedir()
  let entries: string[]
  try {
    entries = await readdir(root)
  } catch {
    return []
  }
  const out: { provider: PylonAccountProvider; home: string; ref: string }[] = []
  for (const name of entries) {
    const provider: PylonAccountProvider | null = name.startsWith(".codex")
      ? "codex"
      : name.startsWith(".claude")
        ? "claude_agent"
        : null
    if (provider === null) continue
    const home = join(root, name)
    try {
      if (!(await stat(home)).isDirectory()) continue
    } catch {
      continue
    }
    out.push({ provider, home, ref: name.replace(/^\./, "") })
  }
  return out
}

export function pylonAccountEnvironment(
  baseEnv: Record<string, string | undefined>,
  account: ResolvedPylonAccountSelection | null | undefined,
): Record<string, string | undefined> {
  if (!account) return { ...baseEnv }
  if (account.provider === "codex") {
    return { ...baseEnv, CODEX_HOME: account.home }
  }
  // claude_agent: isolate the SDK config per account home, and when that home
  // carries a pooled OAuth token, authenticate as that account through
  // CLAUDE_CODE_OAUTH_TOKEN (which outranks the macOS Keychain credential). The
  // token only ever enters this per-session env, not any resolved/projected
  // account object.
  const token = readClaudeOauthToken(account.home)
  return {
    ...baseEnv,
    CLAUDE_CONFIG_DIR: account.home,
    ...(token === null ? {} : { CLAUDE_CODE_OAUTH_TOKEN: token }),
  }
}

export function publicPylonAccountSelection(
  account: ResolvedPylonAccountSelection | null | undefined,
): PublicPylonAccountSelection | null {
  if (!account) return null
  return {
    provider: account.provider,
    selector: account.selector,
    accountRefHash: account.accountRefHash,
  }
}
