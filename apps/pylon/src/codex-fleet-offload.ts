import { stat } from "node:fs/promises"
import { join, posix, resolve } from "node:path"
import {
  hashPylonAccountRef,
  loadPylonAccountRegistry,
  type PylonAccountRegistryEntry,
} from "./account-registry.js"

export const PYLON_CODEX_FLEET_OFFLOAD_PLAN_SCHEMA = "openagents.pylon.codex_fleet_offload_plan.v0.1"

export type CodexFleetOffloadTarget = {
  host: string
  capacity: number
  remoteHome: string
  remoteRepo: string
}

export type CodexFleetOffloadOptions = {
  accounts: string[]
  targets: CodexFleetOffloadTarget[]
  bundleDir: string
  includePrivatePaths: boolean
}

export type CodexFleetOffloadAssignment = {
  accountRef: string
  accountRefHash: string
  host: string
  remoteHome: string
  remoteRepo: string
  bundleName: string
  archiveLabel: string
  commands: {
    pack?: string
    copy?: string
    launch?: string
  }
}

export type CodexFleetOffloadPlan = {
  schema: typeof PYLON_CODEX_FLEET_OFFLOAD_PLAN_SCHEMA
  ok: true
  mode: "redacted" | "private_commands"
  safetyRefs: string[]
  source: {
    accountCount: number
    bundleDirLabel: string
  }
  targets: Array<{
    host: string
    assigned: number
    capacity: number
    remoteHome: string
    remoteRepo: string
    launchConcurrency: number
  }>
  assignments: CodexFleetOffloadAssignment[]
  nextSteps: string[]
}

const accountListPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}(,[A-Za-z0-9][A-Za-z0-9._-]{0,79})*$/
const hostPattern = /^[A-Za-z0-9._-]+$/
const remotePathPattern = /^~?(\/[A-Za-z0-9._+-]+)+$/

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", `'"'"'`)}'`
}

function parsePositiveInt(value: string, label: string): number {
  if (!/^[1-9][0-9]*$/.test(value)) throw new Error(`${label} must be a positive integer`)
  return Number(value)
}

function assertRemotePath(value: string, flag: string): string {
  if (!remotePathPattern.test(value)) {
    throw new Error(`${flag} must be a remote path containing only /, ~, letters, numbers, dot, underscore, plus, or dash`)
  }
  return value
}

export function parseCodexFleetOffloadArgs(args: string[]): CodexFleetOffloadOptions & { json: boolean } {
  let accounts: string[] | null = null
  const targets: CodexFleetOffloadTarget[] = []
  let bundleDir = "~/.pylon-fable/offload-bundles"
  let remoteHome = "~/.pylon-fable"
  let remoteRepo = "~/work/openagents"
  let json = false
  let includePrivatePaths = false

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    const takeValue = (flag: string): string => {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) throw new Error(`${flag} requires a value`)
      index += 1
      return value
    }
    if (arg === "--accounts") {
      const value = takeValue(arg)
      if (!accountListPattern.test(value)) throw new Error("--accounts must be a comma-separated list of account refs")
      accounts = value.split(",")
      continue
    }
    if (arg === "--target") {
      const value = takeValue(arg)
      const [host, capacityRaw] = value.split(":")
      if (!hostPattern.test(host ?? "")) throw new Error("--target must look like host:capacity")
      targets.push({
        host,
        capacity: parsePositiveInt(capacityRaw ?? "", "--target capacity"),
        remoteHome,
        remoteRepo,
      })
      continue
    }
    if (arg === "--remote-home") {
      remoteHome = assertRemotePath(takeValue(arg), arg)
      for (const target of targets) target.remoteHome = remoteHome
      continue
    }
    if (arg === "--remote-repo") {
      remoteRepo = assertRemotePath(takeValue(arg), arg)
      for (const target of targets) target.remoteRepo = remoteRepo
      continue
    }
    if (arg === "--bundle-dir") {
      bundleDir = takeValue(arg)
      continue
    }
    if (arg === "--include-private-paths") {
      includePrivatePaths = true
      continue
    }
    if (arg === "--json") {
      json = true
      continue
    }
    throw new Error(`unknown codex fleet offload-plan option: ${arg}`)
  }

  if (!accounts || accounts.length === 0) throw new Error("codex fleet offload-plan requires --accounts")
  if (targets.length === 0) throw new Error("codex fleet offload-plan requires at least one --target host:capacity")
  const totalCapacity = targets.reduce((sum, target) => sum + target.capacity, 0)
  if (accounts.length > totalCapacity) {
    throw new Error(`selected account count (${accounts.length}) exceeds target capacity (${totalCapacity})`)
  }
  return { accounts, bundleDir, includePrivatePaths, json, targets }
}

function assignTargets(accounts: string[], targets: CodexFleetOffloadTarget[]): Map<string, CodexFleetOffloadTarget> {
  const slots = targets.flatMap((target) => Array.from({ length: target.capacity }, () => target))
  return new Map(accounts.map((account, index) => [account, slots[index]!]))
}

async function assertAccountHomesPresent(entries: PylonAccountRegistryEntry[]): Promise<void> {
  for (const entry of entries) {
    const info = await stat(entry.home).catch(() => null)
    if (!info?.isDirectory()) throw new Error(`Codex account home is missing for ${entry.ref}`)
  }
}

export async function createCodexFleetOffloadPlan(
  summary: { paths: { home: string; config: string } },
  options: CodexFleetOffloadOptions,
): Promise<CodexFleetOffloadPlan> {
  const registry = (await loadPylonAccountRegistry(summary)).filter((entry) => entry.provider === "codex")
  const entries = options.accounts.map((accountRef) => {
    const entry = registry.find((candidate) => candidate.ref === accountRef)
    if (!entry) throw new Error(`Codex account ref is not registered: ${accountRef}`)
    return entry
  })
  await assertAccountHomesPresent(entries)

  const bundleDir = resolve(options.bundleDir.replace(/^~(?=\/|$)/, process.env.HOME ?? "~"))
  const targetByAccount = assignTargets(options.accounts, options.targets)
  const assignments = entries.map((entry) => {
    const target = targetByAccount.get(entry.ref)!
    const bundleName = `pylon-codex-${entry.ref}.tgz`
    const archivePath = join(bundleDir, bundleName)
    const remoteArchive = posix.join(target.remoteHome, "incoming", bundleName)
    const remoteAccountDir = posix.join(target.remoteHome, "accounts", "codex", entry.ref)
    const accountParent = resolve(entry.home, "..")
    const pack = [
      "mkdir",
      "-p",
      shellQuote(bundleDir),
      "&&",
      "tar",
      "-C",
      shellQuote(accountParent),
      "-czf",
      shellQuote(archivePath),
      shellQuote(entry.ref),
    ].join(" ")
    const copy = `ssh ${shellQuote(target.host)} ${shellQuote(`mkdir -p ${target.remoteHome}/incoming ${target.remoteHome}/accounts/codex`)} && scp ${shellQuote(archivePath)} ${shellQuote(`${target.host}:${remoteArchive}`)}`
    const launch = [
      "ssh",
      shellQuote(target.host),
      shellQuote([
        `mkdir -p ${target.remoteHome}/accounts/codex`,
        `tar -C ${target.remoteHome}/accounts/codex -xzf ${remoteArchive}`,
        `cd ${target.remoteRepo}`,
        `PYLON_HOME=${target.remoteHome} CODEX_HOME=${remoteAccountDir} OPENAGENTS_PYLON_CODEX_CONCURRENCY=1 bun apps/pylon/src/index.ts provider go-online --json`,
        `PYLON_HOME=${target.remoteHome} CODEX_HOME=${remoteAccountDir} OPENAGENTS_PYLON_CODEX_CONCURRENCY=1 bun apps/pylon/src/index.ts presence heartbeat --json`,
        `PYLON_HOME=${target.remoteHome} CODEX_HOME=${remoteAccountDir} OPENAGENTS_PYLON_CODEX_CONCURRENCY=1 bun apps/pylon/src/index.ts node`,
      ].join(" && ")),
    ].join(" ")
    return {
      accountRef: entry.ref,
      accountRefHash: hashPylonAccountRef("codex", entry.ref),
      archiveLabel: options.includePrivatePaths ? archivePath : `<bundle-dir>/${bundleName}`,
      bundleName,
      commands: options.includePrivatePaths ? { copy, launch, pack } : {},
      host: target.host,
      remoteHome: target.remoteHome,
      remoteRepo: target.remoteRepo,
    }
  })

  return {
    schema: PYLON_CODEX_FLEET_OFFLOAD_PLAN_SCHEMA,
    ok: true,
    mode: options.includePrivatePaths ? "private_commands" : "redacted",
    safetyRefs: [
      "policy.local.codex_account_homes.isolated",
      "policy.local.no_relogin_required",
      "policy.private_paths.redacted_by_default",
    ],
    source: {
      accountCount: entries.length,
      bundleDirLabel: options.includePrivatePaths ? bundleDir : "<bundle-dir>",
    },
    targets: options.targets.map((target) => {
      const assigned = assignments.filter((assignment) => assignment.host === target.host).length
      return {
        assigned,
        capacity: target.capacity,
        host: target.host,
        launchConcurrency: assigned,
        remoteHome: target.remoteHome,
        remoteRepo: target.remoteRepo,
      }
    }),
    assignments,
    nextSteps: options.includePrivatePaths
      ? [
          "Run each pack command locally.",
          "Run each copy command over Tailnet.",
          "Run each launch command on the target host under a process supervisor.",
          "Verify with pylon codex accounts list --json, provider go-online --json, and presence heartbeat --json on each host.",
        ]
      : [
          "Re-run with --include-private-paths on the operator machine to print tar/scp/launch commands.",
          "Do not paste the private command output into public issues, Forum posts, or traces.",
        ],
  }
}
