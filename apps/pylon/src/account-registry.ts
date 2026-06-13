import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import { homedir } from "node:os"
import { resolve } from "node:path"

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
      throw new PylonAccountSelectionError(
        "Pylon account ref is not registered for this provider",
        "blocker.pylon.account_ref_unknown",
      )
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

export function pylonAccountEnvironment(
  baseEnv: Record<string, string | undefined>,
  account: ResolvedPylonAccountSelection | null | undefined,
): Record<string, string | undefined> {
  if (!account) return { ...baseEnv }
  if (account.provider === "codex") {
    return { ...baseEnv, CODEX_HOME: account.home }
  }
  return { ...baseEnv, CLAUDE_CONFIG_DIR: account.home }
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
