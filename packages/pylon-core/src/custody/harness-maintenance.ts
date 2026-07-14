/**
 * Typed per-harness maintenance actions (MAINT-1, issue #8785).
 *
 * One-click provider install/update for the wrapped coding harnesses (Codex
 * CLI, Claude Code, OpenCode): detect the installed binary + install channel,
 * resolve the latest version for that channel, execute the update through the
 * harness's NATIVE path (npm global vs bun/pnpm global vs Homebrew vs the
 * harness's own updater), then RE-PROBE the binary (version probe must
 * answer) before reporting success — the launch-receipt principle from
 * `docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md` lesson 4.
 *
 * The two additions the T3 reference lacks:
 *  - a PIN record captured before the update (expected version + binary
 *    sha256 + channel). A maintenance run REFUSES silent channel jumps: if
 *    the channel detected at execution time differs from the pinned channel
 *    the update does not run, and if the post-update binary resolves through
 *    a different channel the run is reported as a maintenance failure.
 *  - a PROVENANCE RECEIPT for the swapped binary (what was installed, from
 *    where, checksums before/after where obtainable, and the re-probe
 *    result), persisted under the Pylon home and projected public-safe.
 *
 * SCOPE NOTE (ledger unification follow-up): the repo has no code-level
 * component-ledger surface yet (the term exists only in product-spec docs),
 * so the pin + receipt records here are typed contracts owned by this
 * maintenance lane. When a real component ledger lands, these records are
 * the rows it ingests — do not fork a second shape.
 *
 * SAFETY (always true): maintenance updates BINARIES, never auth state. This
 * module never runs a login flow, never sets `CODEX_HOME`/`CLAUDE_CONFIG_DIR`
 * on the update or probe environment, and never reads or writes the default
 * `~/.codex` home (the standing clobber warning). `assertHarnessMaintenanceCommandSafe`
 * enforces the no-login guarantee on every command before it spawns.
 */
import { createHash } from "node:crypto"
import { spawn } from "node:child_process"
import { constants as fsConstants, promises as fsPromises } from "node:fs"
import { homedir } from "node:os"
import { delimiter, isAbsolute, join } from "node:path"

import type { BootstrapSummary } from "../shared/bootstrap.js"
import { assertPublicProjectionSafe } from "../shared/state.js"

export const PYLON_HARNESS_MAINTENANCE_STATUS_SCHEMA =
  "openagents.pylon.harness_maintenance_status.v0.1"
export const PYLON_HARNESS_MAINTENANCE_RECEIPT_SCHEMA =
  "openagents.pylon.harness_maintenance_receipt.v0.1"
export const PYLON_HARNESS_MAINTENANCE_PIN_SCHEMA =
  "openagents.pylon.harness_maintenance_pin.v0.1"

/**
 * Harnesses this lane knows how to maintain. Aligned with
 * `AgentDefinitionHarnessKind` literals (`codex` / `claude_code` /
 * `opencode`); grok has no owned update path yet and is deliberately absent
 * rather than half-supported.
 */
export type PylonMaintenanceHarness = "codex" | "claude_code" | "opencode"

export const PYLON_MAINTENANCE_HARNESSES: readonly PylonMaintenanceHarness[] = [
  "codex",
  "claude_code",
  "opencode",
]

export function normalizeMaintenanceHarness(value: string): PylonMaintenanceHarness | null {
  const normalized = value.trim().toLowerCase().replace(/-/g, "_")
  if (normalized === "codex") return "codex"
  if (normalized === "claude" || normalized === "claude_code" || normalized === "claude_agent") {
    return "claude_code"
  }
  if (normalized === "opencode") return "opencode"
  return null
}

/**
 * How the installed binary got onto the machine — the channel every update
 * must stay on. `native` means the harness's own installer/updater owns the
 * binary (e.g. `claude update` for the `~/.local` native install). `unknown`
 * means we could not classify the install and therefore refuse one-click
 * updates instead of guessing.
 */
export type HarnessInstallChannel =
  | "npm-global"
  | "bun-global"
  | "pnpm-global"
  | "homebrew"
  | "native"
  | "unknown"

export type HarnessUpdateCommand = {
  executable: string
  args: string[]
}

export type HarnessMaintenanceDefinition = {
  harness: PylonMaintenanceHarness
  /** The binary name resolved on PATH. */
  executable: string
  /** npm package that owns npm/bun/pnpm-global installs AND version truth. */
  npmPackageName: string
  homebrewFormula: string | null
  /** The harness-owned updater for `native` channel installs, if any. */
  nativeUpdate: {
    command: HarnessUpdateCommand
    isCommandPath: (commandPath: string) => boolean
  } | null
}

export function normalizeCommandPath(commandPath: string): string {
  return commandPath.replaceAll("\\", "/").toLowerCase()
}

function isClaudeNativeCommandPath(commandPath: string): boolean {
  const normalized = normalizeCommandPath(commandPath)
  return (
    normalized.endsWith("/.local/bin/claude") ||
    normalized.endsWith("/.local/bin/claude.exe") ||
    normalized.includes("/.local/share/claude/")
  )
}

function isOpenCodeNativeCommandPath(commandPath: string): boolean {
  const normalized = normalizeCommandPath(commandPath)
  return (
    normalized.includes("/.opencode/bin/") || normalized.includes("/.local/share/opencode/")
  )
}

/**
 * Driver-owned maintenance definitions (the T3 `Drivers/` idea): each harness
 * knows BOTH its npm package and its native updater, and channel detection
 * decides which one an update actually uses.
 */
export const HARNESS_MAINTENANCE_DEFINITIONS: Record<
  PylonMaintenanceHarness,
  HarnessMaintenanceDefinition
> = {
  codex: {
    harness: "codex",
    executable: "codex",
    npmPackageName: "@openai/codex",
    homebrewFormula: "codex",
    nativeUpdate: null,
  },
  claude_code: {
    harness: "claude_code",
    executable: "claude",
    npmPackageName: "@anthropic-ai/claude-code",
    homebrewFormula: "claude-code",
    nativeUpdate: {
      command: { executable: "claude", args: ["update"] },
      isCommandPath: isClaudeNativeCommandPath,
    },
  },
  opencode: {
    harness: "opencode",
    executable: "opencode",
    npmPackageName: "opencode-ai",
    homebrewFormula: "anomalyco/tap/opencode",
    nativeUpdate: {
      command: { executable: "opencode", args: ["upgrade"] },
      isCommandPath: isOpenCodeNativeCommandPath,
    },
  },
}

function isBunGlobalCommandPath(commandPath: string): boolean {
  return normalizeCommandPath(commandPath).includes("/.bun/")
}

function isPnpmGlobalCommandPath(commandPath: string): boolean {
  const normalized = normalizeCommandPath(commandPath)
  return (
    normalized.includes("/.local/share/pnpm/") ||
    normalized.includes("/library/pnpm/") ||
    normalized.includes("/appdata/local/pnpm/") ||
    normalized.includes("/pnpm/global/")
  )
}

function isNpmGlobalCommandPath(commandPath: string): boolean {
  const normalized = normalizeCommandPath(commandPath)
  return (
    normalized.includes("/node_modules/.bin/") ||
    normalized.includes("/lib/node_modules/") ||
    normalized.includes("/npm/node_modules/") ||
    normalized.includes("/.npm-global/")
  )
}

function isHomebrewCommandPath(commandPath: string): boolean {
  const normalized = normalizeCommandPath(commandPath)
  return (
    normalized.includes("/homebrew/cellar/") ||
    normalized.includes("/usr/local/cellar/") ||
    normalized.includes("/homebrew/caskroom/") ||
    normalized.startsWith("/opt/homebrew/bin/") ||
    normalized.startsWith("/usr/local/bin/")
  )
}

export function classifyHarnessInstallChannel(
  definition: HarnessMaintenanceDefinition,
  commandPaths: readonly string[],
): HarnessInstallChannel {
  const candidates = commandPaths.filter((value) => value.length > 0)
  if (candidates.length === 0) return "unknown"
  const nativeUpdate = definition.nativeUpdate
  if (nativeUpdate && candidates.some((path) => nativeUpdate.isCommandPath(path))) return "native"
  if (candidates.some(isBunGlobalCommandPath)) return "bun-global"
  if (candidates.some(isPnpmGlobalCommandPath)) return "pnpm-global"
  if (candidates.some(isNpmGlobalCommandPath)) return "npm-global"
  if (candidates.some(isHomebrewCommandPath)) return "homebrew"
  return "unknown"
}

export function harnessUpdateCommandForChannel(
  definition: HarnessMaintenanceDefinition,
  channel: HarnessInstallChannel,
): HarnessUpdateCommand | null {
  switch (channel) {
    case "npm-global":
      return { executable: "npm", args: ["install", "-g", `${definition.npmPackageName}@latest`] }
    case "bun-global":
      return { executable: "bun", args: ["install", "-g", `${definition.npmPackageName}@latest`] }
    case "pnpm-global":
      return { executable: "pnpm", args: ["add", "-g", `${definition.npmPackageName}@latest`] }
    case "homebrew":
      return definition.homebrewFormula === null
        ? null
        : { executable: "brew", args: ["upgrade", definition.homebrewFormula] }
    case "native":
      return definition.nativeUpdate?.command ?? null
    case "unknown":
      return null
  }
}

/**
 * Maintenance updates BINARIES, never auth state. Refuse any command whose
 * arguments would enter a login/auth flow — `codex login` CLEARS
 * `~/.codex/auth.json` at flow-start, so this guard is load-bearing, not
 * defensive decoration.
 */
export function assertHarnessMaintenanceCommandSafe(command: HarnessUpdateCommand): void {
  for (const arg of command.args) {
    const normalized = arg.trim().toLowerCase()
    if (normalized === "login" || normalized === "logout" || normalized === "auth" || normalized.startsWith("--device-auth")) {
      throw new Error(
        `harness maintenance refuses auth-flow command argument ${JSON.stringify(arg)}: maintenance updates binaries, never auth state`,
      )
    }
  }
}

/**
 * The environment every maintenance spawn (probe or update) runs with. Built
 * fresh from the caller env with the harness home-isolation variables
 * REMOVED so no update or probe can ever be pointed at (or accidentally
 * materialize) an auth home. `~/.codex` stays untouched because nothing in
 * this lane ever names it.
 */
export function harnessMaintenanceEnvironment(
  env: Record<string, string | undefined>,
): Record<string, string | undefined> {
  const scrubbed = { ...env }
  delete scrubbed.CODEX_HOME
  delete scrubbed.CLAUDE_CONFIG_DIR
  delete scrubbed.CLAUDE_CODE_OAUTH_TOKEN
  delete scrubbed.GROK_HOME
  return scrubbed
}

export type HarnessCommandResult = {
  exitCode: number | null
  stdout: string
  stderr: string
  timedOut: boolean
}

export type HarnessCommandRunner = (input: {
  executable: string
  args: readonly string[]
  env: Record<string, string | undefined>
  timeoutMs: number
}) => Promise<HarnessCommandResult>

export type HarnessMaintenanceDeps = {
  env?: Record<string, string | undefined>
  runCommand?: HarnessCommandRunner
  /** Resolve an executable name to an absolute path (PATH lookup). */
  whichPath?: (executable: string, env: Record<string, string | undefined>) => Promise<string | null>
  realPath?: (path: string) => Promise<string | null>
  sha256File?: (path: string) => Promise<string | null>
  /** Resolve the latest published version for an npm package (channel truth). */
  fetchLatestVersion?: (
    npmPackageName: string,
    env: Record<string, string | undefined>,
  ) => Promise<string | null>
  now?: () => Date
}

const COMMAND_OUTPUT_MAX_CHARS = 4_000
const PROBE_TIMEOUT_MS = 15_000
const UPDATE_TIMEOUT_MS = 5 * 60_000
const LATEST_VERSION_TIMEOUT_MS = 8_000

export const defaultHarnessCommandRunner: HarnessCommandRunner = (input) =>
  new Promise((resolve) => {
    let child: ReturnType<typeof spawn>
    try {
      child = spawn(input.executable, [...input.args], {
        stdio: ["ignore", "pipe", "pipe"],
        env: Object.fromEntries(
          Object.entries(input.env).filter(([, value]) => value !== undefined),
        ) as NodeJS.ProcessEnv,
      })
    } catch (error) {
      resolve({
        exitCode: null,
        stdout: "",
        stderr: error instanceof Error ? error.message : String(error),
        timedOut: false,
      })
      return
    }
    let stdout = ""
    let stderr = ""
    let settled = false
    let timedOut = false
    const finish = (exitCode: number | null): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve({ exitCode, stdout, stderr, timedOut })
    }
    child.stdout?.on("data", (chunk: Buffer | string) => {
      if (stdout.length < COMMAND_OUTPUT_MAX_CHARS) {
        stdout += String(chunk).slice(0, COMMAND_OUTPUT_MAX_CHARS - stdout.length)
      }
    })
    child.stderr?.on("data", (chunk: Buffer | string) => {
      if (stderr.length < COMMAND_OUTPUT_MAX_CHARS) {
        stderr += String(chunk).slice(0, COMMAND_OUTPUT_MAX_CHARS - stderr.length)
      }
    })
    child.on("error", (error) => {
      stderr = stderr || (error instanceof Error ? error.message : String(error))
      finish(null)
    })
    child.on("close", (code) => finish(typeof code === "number" ? code : null))
    const timer = setTimeout(() => {
      timedOut = true
      child.kill("SIGTERM")
      finish(null)
    }, input.timeoutMs)
  })

async function isExecutableFile(path: string): Promise<boolean> {
  try {
    const stat = await fsPromises.stat(path)
    if (!stat.isFile()) return false
    if (process.platform === "win32") return true
    await fsPromises.access(path, fsConstants.X_OK)
    return true
  } catch {
    return false
  }
}

export async function defaultWhichPath(
  executable: string,
  env: Record<string, string | undefined>,
): Promise<string | null> {
  if (isAbsolute(executable)) {
    return (await isExecutableFile(executable)) ? executable : null
  }
  const pathValue = env.PATH ?? env.Path ?? env.path ?? ""
  const extensions =
    process.platform === "win32" ? (env.PATHEXT ?? ".EXE;.CMD;.BAT;.COM").split(";") : [""]
  for (const directory of pathValue.split(delimiter)) {
    if (directory.length === 0) continue
    for (const extension of extensions) {
      const candidate = join(directory, `${executable}${extension.toLowerCase()}`)
      if (await isExecutableFile(candidate)) return candidate
    }
  }
  return null
}

async function defaultRealPath(path: string): Promise<string | null> {
  try {
    return await fsPromises.realpath(path)
  } catch {
    return null
  }
}

async function defaultSha256File(path: string): Promise<string | null> {
  try {
    const bytes = await fsPromises.readFile(path)
    return createHash("sha256").update(bytes).digest("hex")
  } catch {
    return null
  }
}

const NPM_REGISTRY_BASE = "https://registry.npmjs.org"

/**
 * Registry base override (tests point this at a local server so the fixture
 * round trip never touches the network; production never sets it).
 */
export function npmRegistryBase(env: Record<string, string | undefined>): string {
  const override = env.PYLON_NPM_REGISTRY_BASE
  return typeof override === "string" && override.trim().length > 0
    ? override.trim().replace(/\/+$/, "")
    : NPM_REGISTRY_BASE
}

async function defaultFetchLatestVersion(
  npmPackageName: string,
  env: Record<string, string | undefined>,
): Promise<string | null> {
  try {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), LATEST_VERSION_TIMEOUT_MS)
    try {
      const response = await fetch(
        `${npmRegistryBase(env)}/${encodeURIComponent(npmPackageName)}/latest`,
        { headers: { accept: "application/json" }, signal: controller.signal },
      )
      if (!response.ok) return null
      const payload = (await response.json()) as { version?: unknown }
      return typeof payload.version === "string" && payload.version.trim().length > 0
        ? payload.version.trim()
        : null
    } finally {
      clearTimeout(timer)
    }
  } catch {
    return null
  }
}

function resolveDeps(deps: HarnessMaintenanceDeps): Required<HarnessMaintenanceDeps> {
  return {
    env: deps.env ?? (process.env as Record<string, string | undefined>),
    runCommand: deps.runCommand ?? defaultHarnessCommandRunner,
    whichPath: deps.whichPath ?? defaultWhichPath,
    realPath: deps.realPath ?? defaultRealPath,
    sha256File: deps.sha256File ?? defaultSha256File,
    fetchLatestVersion: deps.fetchLatestVersion ?? defaultFetchLatestVersion,
    now: deps.now ?? (() => new Date()),
  }
}

const SEMVER_PATTERN = /\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?/

export function parseHarnessVersionOutput(output: string): string | null {
  const match = output.match(SEMVER_PATTERN)
  return match === null ? null : match[0]
}

export function compareHarnessVersions(a: string, b: string): number {
  const parse = (value: string): number[] =>
    value
      .split("-")[0]!
      .split(".")
      .map((part) => Number.parseInt(part, 10) || 0)
  const left = parse(a)
  const right = parse(b)
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const delta = (left[index] ?? 0) - (right[index] ?? 0)
    if (delta !== 0) return delta < 0 ? -1 : 1
  }
  return 0
}

/**
 * The detect/re-probe result: where the binary lives, which channel owns it,
 * and whether it ANSWERS a version probe (installed-but-mute counts as a
 * failed probe — presence on disk is not launch proof).
 */
export type HarnessInstallState = {
  harness: PylonMaintenanceHarness
  installed: boolean
  binaryPath: string | null
  realBinaryPath: string | null
  channel: HarnessInstallChannel
  /** Version probe result: null means the probe FAILED or nothing installed. */
  installedVersion: string | null
  probeOk: boolean
  binarySha256: string | null
  probedAt: string
}

export async function detectHarnessInstall(
  harness: PylonMaintenanceHarness,
  deps: HarnessMaintenanceDeps = {},
): Promise<HarnessInstallState> {
  const resolved = resolveDeps(deps)
  const definition = HARNESS_MAINTENANCE_DEFINITIONS[harness]
  const env = harnessMaintenanceEnvironment(resolved.env)
  const probedAt = resolved.now().toISOString()
  const binaryPath = await resolved.whichPath(definition.executable, env)
  if (binaryPath === null) {
    return {
      harness,
      installed: false,
      binaryPath: null,
      realBinaryPath: null,
      channel: "unknown",
      installedVersion: null,
      probeOk: false,
      binarySha256: null,
      probedAt,
    }
  }
  const realBinaryPath = (await resolved.realPath(binaryPath)) ?? binaryPath
  const channel = classifyHarnessInstallChannel(definition, [binaryPath, realBinaryPath])
  const probe = await resolved.runCommand({
    executable: binaryPath,
    args: ["--version"],
    env,
    timeoutMs: PROBE_TIMEOUT_MS,
  })
  const installedVersion =
    probe.exitCode === 0 ? parseHarnessVersionOutput(probe.stdout || probe.stderr) : null
  return {
    harness,
    installed: true,
    binaryPath,
    realBinaryPath,
    channel,
    installedVersion,
    probeOk: probe.exitCode === 0 && installedVersion !== null,
    binarySha256: await resolved.sha256File(realBinaryPath),
    probedAt,
  }
}

export type HarnessMaintenanceAdvisory = "current" | "behind_latest" | "unknown"

export type HarnessMaintenanceStatusEntry = HarnessInstallState & {
  latestVersion: string | null
  advisory: HarnessMaintenanceAdvisory
  updateSupported: boolean
  updateCommand: HarnessUpdateCommand | null
}

export type PylonHarnessMaintenanceStatusProjection = {
  schema: typeof PYLON_HARNESS_MAINTENANCE_STATUS_SCHEMA
  observedAt: string
  harnesses: HarnessMaintenanceStatusEntry[]
}

function deriveAdvisory(
  installedVersion: string | null,
  latestVersion: string | null,
): HarnessMaintenanceAdvisory {
  if (installedVersion === null || latestVersion === null) return "unknown"
  return compareHarnessVersions(installedVersion, latestVersion) < 0 ? "behind_latest" : "current"
}

export async function collectHarnessMaintenanceStatus(
  deps: HarnessMaintenanceDeps = {},
  harnesses: readonly PylonMaintenanceHarness[] = PYLON_MAINTENANCE_HARNESSES,
): Promise<PylonHarnessMaintenanceStatusProjection> {
  const resolved = resolveDeps(deps)
  const entries: HarnessMaintenanceStatusEntry[] = []
  for (const harness of harnesses) {
    const definition = HARNESS_MAINTENANCE_DEFINITIONS[harness]
    const state = await detectHarnessInstall(harness, deps)
    const latestVersion = state.installed
      ? await resolved.fetchLatestVersion(
          definition.npmPackageName,
          harnessMaintenanceEnvironment(resolved.env),
        )
      : null
    const updateCommand = harnessUpdateCommandForChannel(definition, state.channel)
    entries.push({
      ...state,
      latestVersion,
      advisory: deriveAdvisory(state.installedVersion, latestVersion),
      updateSupported: state.installed && updateCommand !== null,
      updateCommand,
    })
  }
  const projection: PylonHarnessMaintenanceStatusProjection = {
    schema: PYLON_HARNESS_MAINTENANCE_STATUS_SCHEMA,
    observedAt: resolved.now().toISOString(),
    harnesses: entries,
  }
  assertPublicProjectionSafe(projectHomePathsSafe(projection), "harness maintenance status")
  return projection
}

/**
 * The pin captured BEFORE an update runs: what we expect to be replacing
 * (version + checksum) and the only channel the update may use. This is the
 * component-ledger row of record for the maintenance lane (see the scope
 * note in the module doc).
 */
export type PylonHarnessMaintenancePin = {
  schema: typeof PYLON_HARNESS_MAINTENANCE_PIN_SCHEMA
  harness: PylonMaintenanceHarness
  channel: HarnessInstallChannel
  expectedVersion: string | null
  expectedBinarySha256: string | null
  targetVersion: string | null
  pinnedAt: string
}

export type HarnessMaintenanceOutcome =
  | "updated"
  | "already_current"
  | "channel_jump_refused"
  | "failed"

export type HarnessMaintenanceFailureReason =
  | "not_installed"
  | "probe_failed_before_update"
  | "unsupported_channel"
  | "updater_missing"
  | "update_command_failed"
  | "post_update_probe_failed"
  | "version_unchanged_after_update"
  | "channel_changed_after_update"

/**
 * Provenance receipt for a maintenance run. Persisted append-only under the
 * Pylon home and projected public-safe (home-directory prefixes collapsed to
 * `~`). `after === null` means the update never executed (refusal or
 * pre-flight failure) and the previous install state is intact by
 * construction.
 */
export type PylonHarnessMaintenanceReceipt = {
  schema: typeof PYLON_HARNESS_MAINTENANCE_RECEIPT_SCHEMA
  receiptId: string
  harness: PylonMaintenanceHarness
  startedAt: string
  finishedAt: string
  outcome: HarnessMaintenanceOutcome
  failureReason: HarnessMaintenanceFailureReason | null
  pin: PylonHarnessMaintenancePin | null
  before: HarnessInstallState
  after: HarnessInstallState | null
  update: {
    executable: string
    args: string[]
    exitCode: number | null
    timedOut: boolean
    outputExcerpt: string | null
  } | null
  /** Where the new bits came from (checksum source where obtainable). */
  source: {
    kind: "npm-registry" | "homebrew" | "native-updater"
    packageName: string | null
    url: string | null
  } | null
}

export type HarnessMaintenanceUpdateOptions = {
  harness: PylonMaintenanceHarness
  /**
   * Explicit channel override. When set and different from the detected
   * channel the run is REFUSED as a channel jump unless `allowChannelJump`.
   */
  channel?: HarnessInstallChannel
  allowChannelJump?: boolean
  deps?: HarnessMaintenanceDeps
}

function sourceForChannel(
  definition: HarnessMaintenanceDefinition,
  channel: HarnessInstallChannel,
): PylonHarnessMaintenanceReceipt["source"] {
  switch (channel) {
    case "npm-global":
    case "bun-global":
    case "pnpm-global":
      return {
        kind: "npm-registry",
        packageName: definition.npmPackageName,
        url: `${NPM_REGISTRY_BASE}/${encodeURIComponent(definition.npmPackageName)}/latest`,
      }
    case "homebrew":
      return { kind: "homebrew", packageName: definition.homebrewFormula, url: null }
    case "native":
      return { kind: "native-updater", packageName: definition.npmPackageName, url: null }
    case "unknown":
      return null
  }
}

function updateOutputExcerpt(result: HarnessCommandResult): string | null {
  const combined = [result.stderr, result.stdout]
    .map((value) => value.trim())
    .filter((value) => value.length > 0)
    .join("\n\n")
  if (combined.length === 0) return null
  return combined.slice(0, COMMAND_OUTPUT_MAX_CHARS)
}

function makeReceiptId(now: Date): string {
  const random = createHash("sha256")
    .update(`${now.toISOString()}:${Math.random()}:${process.pid}`)
    .digest("hex")
    .slice(0, 12)
  return `hmr-${now.toISOString().replaceAll(/[:.]/g, "-")}-${random}`
}

/**
 * Detect → pin → update → RE-PROBE → receipt. Success is only reported when
 * the post-update binary answers a version probe with a changed (or already
 * current) version on the SAME channel; anything else is an explicit typed
 * failure with the previous state recorded intact in the receipt.
 */
export async function runHarnessMaintenanceUpdate(
  options: HarnessMaintenanceUpdateOptions,
): Promise<PylonHarnessMaintenanceReceipt> {
  const deps = options.deps ?? {}
  const resolved = resolveDeps(deps)
  const definition = HARNESS_MAINTENANCE_DEFINITIONS[options.harness]
  const startedAt = resolved.now().toISOString()
  const before = await detectHarnessInstall(options.harness, deps)

  const finish = (
    partial: Pick<PylonHarnessMaintenanceReceipt, "outcome" | "failureReason"> &
      Partial<PylonHarnessMaintenanceReceipt>,
  ): PylonHarnessMaintenanceReceipt => ({
    schema: PYLON_HARNESS_MAINTENANCE_RECEIPT_SCHEMA,
    receiptId: makeReceiptId(resolved.now()),
    harness: options.harness,
    startedAt,
    finishedAt: resolved.now().toISOString(),
    pin: null,
    before,
    after: null,
    update: null,
    source: null,
    ...partial,
  })

  if (!before.installed) {
    return finish({ outcome: "failed", failureReason: "not_installed" })
  }
  if (!before.probeOk) {
    return finish({ outcome: "failed", failureReason: "probe_failed_before_update" })
  }

  const requestedChannel = options.channel ?? before.channel
  if (requestedChannel !== before.channel && options.allowChannelJump !== true) {
    // Silent channel jumps are refused: swapping an npm-global binary for a
    // Homebrew one (or vice versa) behind an "update" click is a supply-chain
    // surface, not a convenience.
    return finish({ outcome: "channel_jump_refused", failureReason: null })
  }

  const updateCommand = harnessUpdateCommandForChannel(definition, requestedChannel)
  if (updateCommand === null) {
    return finish({ outcome: "failed", failureReason: "unsupported_channel" })
  }
  assertHarnessMaintenanceCommandSafe(updateCommand)

  const env = harnessMaintenanceEnvironment(resolved.env)
  const updaterPath = await resolved.whichPath(updateCommand.executable, env)
  if (updaterPath === null) {
    return finish({ outcome: "failed", failureReason: "updater_missing" })
  }

  const latestVersion = await resolved.fetchLatestVersion(definition.npmPackageName, env)
  const pin: PylonHarnessMaintenancePin = {
    schema: PYLON_HARNESS_MAINTENANCE_PIN_SCHEMA,
    harness: options.harness,
    channel: requestedChannel,
    expectedVersion: before.installedVersion,
    expectedBinarySha256: before.binarySha256,
    targetVersion: latestVersion,
    pinnedAt: resolved.now().toISOString(),
  }
  const source = sourceForChannel(definition, requestedChannel)

  if (
    before.installedVersion !== null &&
    latestVersion !== null &&
    compareHarnessVersions(before.installedVersion, latestVersion) >= 0
  ) {
    return finish({ outcome: "already_current", failureReason: null, pin, source })
  }

  const updateResult = await resolved.runCommand({
    executable: updaterPath,
    args: updateCommand.args,
    env,
    timeoutMs: UPDATE_TIMEOUT_MS,
  })
  const update = {
    executable: updateCommand.executable,
    args: [...updateCommand.args],
    exitCode: updateResult.exitCode,
    timedOut: updateResult.timedOut,
    outputExcerpt: updateOutputExcerpt(updateResult),
  }
  if (updateResult.timedOut || updateResult.exitCode !== 0) {
    return finish({
      outcome: "failed",
      failureReason: "update_command_failed",
      pin,
      update,
      source,
    })
  }

  // RE-PROBE (lesson 4): the swapped binary must ANSWER, not merely exist.
  const after = await detectHarnessInstall(options.harness, deps)
  if (!after.installed || !after.probeOk) {
    return finish({
      outcome: "failed",
      failureReason: "post_update_probe_failed",
      pin,
      update,
      source,
      after,
    })
  }
  if (after.channel !== requestedChannel) {
    return finish({
      outcome: "failed",
      failureReason: "channel_changed_after_update",
      pin,
      update,
      source,
      after,
    })
  }
  if (
    before.installedVersion !== null &&
    after.installedVersion !== null &&
    compareHarnessVersions(after.installedVersion, before.installedVersion) <= 0 &&
    latestVersion !== null &&
    compareHarnessVersions(after.installedVersion, latestVersion) < 0
  ) {
    return finish({
      outcome: "failed",
      failureReason: "version_unchanged_after_update",
      pin,
      update,
      source,
      after,
    })
  }

  return finish({ outcome: "updated", failureReason: null, pin, update, source, after })
}

// ---------------------------------------------------------------------------
// Persistence + public-safe projection
// ---------------------------------------------------------------------------

export function harnessMaintenanceDirectory(summary: Pick<BootstrapSummary, "paths">): string {
  return join(summary.paths.home, "harness-maintenance")
}

export function harnessMaintenanceReceiptsPath(
  summary: Pick<BootstrapSummary, "paths">,
  harness: PylonMaintenanceHarness,
): string {
  return join(harnessMaintenanceDirectory(summary), `${harness}.receipts.jsonl`)
}

export function harnessMaintenancePinPath(
  summary: Pick<BootstrapSummary, "paths">,
  harness: PylonMaintenanceHarness,
): string {
  return join(harnessMaintenanceDirectory(summary), `${harness}.pin.json`)
}

export async function persistHarnessMaintenanceReceipt(
  summary: Pick<BootstrapSummary, "paths">,
  receipt: PylonHarnessMaintenanceReceipt,
): Promise<void> {
  const directory = harnessMaintenanceDirectory(summary)
  await fsPromises.mkdir(directory, { recursive: true })
  await fsPromises.appendFile(
    harnessMaintenanceReceiptsPath(summary, receipt.harness),
    `${JSON.stringify(receipt)}\n`,
    "utf8",
  )
  if (receipt.pin !== null) {
    await fsPromises.writeFile(
      harnessMaintenancePinPath(summary, receipt.harness),
      `${JSON.stringify(receipt.pin, null, 2)}\n`,
      "utf8",
    )
  }
}

export async function loadHarnessMaintenanceReceipts(
  summary: Pick<BootstrapSummary, "paths">,
  harness: PylonMaintenanceHarness,
): Promise<PylonHarnessMaintenanceReceipt[]> {
  try {
    const raw = await fsPromises.readFile(harnessMaintenanceReceiptsPath(summary, harness), "utf8")
    const receipts: PylonHarnessMaintenanceReceipt[] = []
    for (const line of raw.split("\n")) {
      const trimmed = line.trim()
      if (trimmed.length === 0) continue
      try {
        const parsed = JSON.parse(trimmed) as PylonHarnessMaintenanceReceipt
        if (parsed.schema === PYLON_HARNESS_MAINTENANCE_RECEIPT_SCHEMA) receipts.push(parsed)
      } catch {
        // Skip torn lines rather than failing the whole history read.
      }
    }
    return receipts
  } catch {
    return []
  }
}

export async function loadHarnessMaintenancePin(
  summary: Pick<BootstrapSummary, "paths">,
  harness: PylonMaintenanceHarness,
): Promise<PylonHarnessMaintenancePin | null> {
  try {
    const raw = await fsPromises.readFile(harnessMaintenancePinPath(summary, harness), "utf8")
    const parsed = JSON.parse(raw) as PylonHarnessMaintenancePin
    return parsed.schema === PYLON_HARNESS_MAINTENANCE_PIN_SCHEMA ? parsed : null
  } catch {
    return null
  }
}

function projectHomePathsSafe<T>(value: T, home: string = homedir()): T {
  if (typeof value === "string") {
    return (home.length > 1 ? value.replaceAll(home, "~") : value) as T
  }
  if (Array.isArray(value)) {
    return value.map((entry) => projectHomePathsSafe(entry, home)) as T
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, child]) => [
        key,
        projectHomePathsSafe(child, home),
      ]),
    ) as T
  }
  return value
}

/**
 * Public-safe projection of a receipt: home-directory prefixes collapse to
 * `~` (usernames are not public data) and the standard projection guard runs
 * over the result. The receipt carries no tokens by construction — this is
 * belt on top of that suspender.
 */
export function projectPublicHarnessMaintenanceReceipt(
  receipt: PylonHarnessMaintenanceReceipt,
  options: { home?: string } = {},
): PylonHarnessMaintenanceReceipt {
  const projected = projectHomePathsSafe(receipt, options.home ?? homedir())
  assertPublicProjectionSafe(projected, "harness maintenance receipt")
  return projected
}
