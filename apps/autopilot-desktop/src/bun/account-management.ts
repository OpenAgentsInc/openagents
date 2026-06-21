// CS-A1: desktop-side account management over the node's LOCAL `dev.accounts`
// config — add / remove / set-priority / list. This is the write side of the
// "turn the read-only AccountList into add/select/priority/quota management"
// gap (audit gap #2, phase 3). It deliberately does NOT add a new control-
// protocol verb: the Pylon runtime already reads `dev.accounts` from
// `config.json` to discover and select accounts (see
// apps/pylon/src/account-registry.ts `loadPylonAccountRegistry`), so editing
// that file from the Bun host — which already owns the node home — is the
// minimal, contract-free way to manage the registry. The live readiness +
// per-session selection still flow through the existing `accounts.list` /
// `session.spawn` control verbs.
//
// All paths/secrets stay in the Bun host; the webview only ever sees the
// public-safe ManagedAccountRow projection (ref / provider / homePresent /
// priority).

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"
import type {
  ManagedAccountMutationResponse,
  ManagedAccountRow,
  ManagedAccountsResponse,
} from "../shared/rpc.js"

export type ManagedAccountProvider = "codex" | "claude_agent"

// Same validation contract the runtime uses (account-registry.ts).
const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

type RawAccountEntry = {
  ref: string
  provider: ManagedAccountProvider
  home: string
  priority?: number
}

const isProvider = (value: unknown): value is ManagedAccountProvider =>
  value === "codex" || value === "claude_agent"

const normalizeHome = (value: string): string => {
  const trimmed = value.trim()
  if (trimmed === "~") return homedir()
  if (trimmed.startsWith("~/")) return resolve(homedir(), trimmed.slice(2))
  return resolve(trimmed)
}

const homePresent = (home: string): boolean => {
  try {
    return statSync(home).isDirectory()
  } catch {
    return false
  }
}

// The node keeps `config.json` directly in its home directory (the same file
// `loadPylonAccountRegistry` reads via `summary.paths.config`).
export const accountConfigPath = (pylonHome: string): string =>
  join(pylonHome, "config.json")

type ParsedConfig = {
  // The full parsed config object, preserved so writes never drop unrelated keys.
  readonly root: Record<string, unknown>
  readonly entries: RawAccountEntry[]
}

const readConfig = (configPath: string): ParsedConfig => {
  if (!existsSync(configPath)) {
    return { root: {}, entries: [] }
  }
  let root: Record<string, unknown> = {}
  try {
    const parsed = JSON.parse(readFileSync(configPath, "utf8"))
    if (parsed !== null && typeof parsed === "object") {
      root = parsed as Record<string, unknown>
    }
  } catch {
    return { root: {}, entries: [] }
  }
  const dev = root.dev
  const rawAccounts =
    dev !== null && typeof dev === "object"
      ? (dev as { accounts?: unknown }).accounts
      : undefined
  const entries: RawAccountEntry[] = []
  if (Array.isArray(rawAccounts)) {
    for (const account of rawAccounts) {
      if (account === null || typeof account !== "object") continue
      const record = account as Record<string, unknown>
      if (
        !isProvider(record.provider) ||
        typeof record.ref !== "string" ||
        !accountRefPattern.test(record.ref) ||
        typeof record.home !== "string" ||
        record.home.trim().length === 0
      ) {
        continue
      }
      entries.push({
        ref: record.ref,
        provider: record.provider,
        home: normalizeHome(record.home),
        ...(typeof record.priority === "number" && Number.isFinite(record.priority)
          ? { priority: Math.trunc(record.priority) }
          : {}),
      })
    }
  }
  return { root, entries }
}

const sortEntries = (entries: RawAccountEntry[]): RawAccountEntry[] =>
  [...entries].sort((a, b) => {
    // Lower priority dispatches first; entries with no priority sort last,
    // then alphabetically by ref for a stable order.
    const pa = a.priority ?? Number.POSITIVE_INFINITY
    const pb = b.priority ?? Number.POSITIVE_INFINITY
    if (pa !== pb) return pa - pb
    return a.ref.localeCompare(b.ref)
  })

const toRow = (entry: RawAccountEntry): ManagedAccountRow => ({
  ref: entry.ref,
  provider: entry.provider,
  homePresent: homePresent(entry.home),
  priority: entry.priority ?? null,
})

const writeConfig = (configPath: string, parsed: ParsedConfig, entries: RawAccountEntry[]): void => {
  const root = { ...parsed.root }
  const dev =
    root.dev !== null && typeof root.dev === "object"
      ? { ...(root.dev as Record<string, unknown>) }
      : {}
  dev.accounts = sortEntries(entries).map((entry) => ({
    ref: entry.ref,
    provider: entry.provider,
    home: entry.home,
    ...(entry.priority === undefined ? {} : { priority: entry.priority }),
  }))
  root.dev = dev
  mkdirSync(dirname(configPath), { recursive: true })
  writeFileSync(configPath, `${JSON.stringify(root, null, 2)}\n`, { mode: 0o600 })
}

const okList = (entries: RawAccountEntry[]): ManagedAccountsResponse => ({
  ok: true,
  accounts: sortEntries(entries).map(toRow),
})

const okMutation = (entries: RawAccountEntry[]): ManagedAccountMutationResponse => ({
  ok: true,
  accounts: sortEntries(entries).map(toRow),
})

const errMutation = (error: string): ManagedAccountMutationResponse => ({
  ok: false,
  accounts: [],
  error,
})

export const listManagedAccounts = (pylonHome: string | null): ManagedAccountsResponse => {
  if (pylonHome === null) {
    return { ok: false, accounts: [], error: "local node home unavailable" }
  }
  return okList(readConfig(accountConfigPath(pylonHome)).entries)
}

export const addManagedAccount = (
  pylonHome: string | null,
  input: { ref: string; provider: ManagedAccountProvider; home: string; priority?: number },
): ManagedAccountMutationResponse => {
  if (pylonHome === null) return errMutation("local node home unavailable")
  const ref = input.ref.trim()
  if (!accountRefPattern.test(ref)) {
    return errMutation("account ref is invalid (letters, digits, . _ - ; max 80)")
  }
  if (!isProvider(input.provider)) return errMutation("provider must be codex or claude_agent")
  const home = input.home.trim()
  if (home.length === 0) return errMutation("account home path is required")
  const configPath = accountConfigPath(pylonHome)
  const parsed = readConfig(configPath)
  if (parsed.entries.some((e) => e.provider === input.provider && e.ref === ref)) {
    return errMutation(`account ref already exists for ${input.provider}`)
  }
  const entry: RawAccountEntry = {
    ref,
    provider: input.provider,
    home: normalizeHome(home),
    ...(typeof input.priority === "number" && Number.isFinite(input.priority)
      ? { priority: Math.trunc(input.priority) }
      : {}),
  }
  const next = [...parsed.entries, entry]
  writeConfig(configPath, parsed, next)
  return okMutation(next)
}

export const removeManagedAccount = (
  pylonHome: string | null,
  input: { ref: string; provider: ManagedAccountProvider },
): ManagedAccountMutationResponse => {
  if (pylonHome === null) return errMutation("local node home unavailable")
  if (!isProvider(input.provider)) return errMutation("provider must be codex or claude_agent")
  const ref = input.ref.trim()
  const configPath = accountConfigPath(pylonHome)
  const parsed = readConfig(configPath)
  const next = parsed.entries.filter(
    (e) => !(e.provider === input.provider && e.ref === ref),
  )
  if (next.length === parsed.entries.length) {
    return errMutation(`no such account ref for ${input.provider}`)
  }
  writeConfig(configPath, parsed, next)
  return okMutation(next)
}

export const setManagedAccountPriority = (
  pylonHome: string | null,
  input: { ref: string; provider: ManagedAccountProvider; priority: number },
): ManagedAccountMutationResponse => {
  if (pylonHome === null) return errMutation("local node home unavailable")
  if (!isProvider(input.provider)) return errMutation("provider must be codex or claude_agent")
  if (!Number.isFinite(input.priority)) return errMutation("priority must be a finite number")
  const ref = input.ref.trim()
  const configPath = accountConfigPath(pylonHome)
  const parsed = readConfig(configPath)
  let found = false
  const next = parsed.entries.map((e) => {
    if (e.provider === input.provider && e.ref === ref) {
      found = true
      return { ...e, priority: Math.trunc(input.priority) }
    }
    return e
  })
  if (!found) return errMutation(`no such account ref for ${input.provider}`)
  writeConfig(configPath, parsed, next)
  return okMutation(next)
}
