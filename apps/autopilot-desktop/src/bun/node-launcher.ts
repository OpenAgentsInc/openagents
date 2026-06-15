import { existsSync } from "node:fs"
import { dirname, join } from "node:path"
import { discoverPylonHome } from "./node-home"
import { readControlToken } from "./pylon-control"

// #5011 (JUNE15_LAUNCH_PLAN §0/§4.F): the install seam. Today the desktop only
// *discovers* an already-running local node; a fresh install therefore reports
// honest "offline" forever. This module makes the Bun main process *launch* the
// local Pylon node runtime when none is discovered — so "install Autopilot and
// contribute" is one step — while *adopting* an already-running node when one is
// present (never double-spawning).
//
// Phase 1 is the dev build: it launches the repo's `apps/pylon/src/index.ts`
// under Bun, into a managed `.pylon-local` home that `discoverPylonHome` already
// scans, so the rest of the app (resolveHome/poller/commands) picks it up
// unchanged. Packaged-binary launch is Phase 2 and is intentionally out of scope
// here: with no repo entry resolvable we stay discover-only and honest-offline.

export type NodeLaunchMode = "adopted" | "launched" | "unavailable"

export type ManagedNode = {
  readonly mode: NodeLaunchMode
  readonly home: string | null
  readonly pid: number | null
  // Stops the managed child if (and only if) we launched it; a no-op for an
  // adopted node we did not start, and for the unavailable case.
  stop(): void
}

export type LaunchedProcess = {
  readonly pid: number
  kill(): void
}

export type SpawnNodeInput = {
  readonly command: ReadonlyArray<string>
  readonly cwd: string
  readonly env: Record<string, string>
}

export type EnsureManagedNodeOptions = {
  readonly cwd: string
  readonly env: Record<string, string | undefined>
  readonly controlBaseUrl: string
  // Injectables (default to the real implementations).
  readonly discover?: (opts: { env?: string; cwd: string }) => string | null
  readonly readToken?: (home: string) => string | null
  readonly fileExists?: (path: string) => boolean
  readonly spawnNode?: (input: SpawnNodeInput) => LaunchedProcess
  readonly probeReady?: (baseUrl: string) => Promise<boolean>
  readonly sleep?: (ms: number) => Promise<void>
  readonly bunBin?: string
  readonly readinessTimeoutMs?: number
  readonly readinessIntervalMs?: number
}

const MANAGED_HOME_SUBDIR = ".pylon-local"
const PYLON_ENTRY_RELATIVE = join("apps", "pylon", "src", "index.ts")

const ancestors = (cwd: string): string[] => {
  const out: string[] = []
  let cur = cwd
  for (let i = 0; i < 64; i++) {
    out.push(cur)
    const parent = dirname(cur)
    if (parent === cur) break
    cur = parent
  }
  return out
}

// Find the repo's dev Pylon entrypoint by walking up from cwd for the first
// ancestor that contains `apps/pylon/src/index.ts`. Returns the entry path and
// its repo root, or null when not in the monorepo (e.g. a packaged build).
export const findDevPylonEntry = (
  cwd: string,
  fileExists: (path: string) => boolean = existsSync,
): { readonly entry: string; readonly repoRoot: string } | null => {
  for (const anc of ancestors(cwd)) {
    const entry = join(anc, PYLON_ENTRY_RELATIVE)
    if (fileExists(entry)) {
      return { entry, repoRoot: anc }
    }
  }
  return null
}

const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

// A control server that is up returns *some* HTTP response (401 without a token
// counts as up); only a refused/failed connection means not-yet-ready.
const defaultProbeReady = async (baseUrl: string): Promise<boolean> => {
  try {
    await fetch(baseUrl, { method: "GET" })
    return true
  } catch {
    return false
  }
}

const defaultSpawnNode = (input: SpawnNodeInput): LaunchedProcess => {
  const child = Bun.spawn([...input.command], {
    cwd: input.cwd,
    env: input.env,
    stdout: "inherit",
    stderr: "inherit",
  })
  return {
    pid: child.pid,
    kill: () => {
      try {
        child.kill()
      } catch {
        // already exited
      }
    },
  }
}

const adopted = (home: string): ManagedNode => ({
  mode: "adopted",
  home,
  pid: null,
  stop: () => {},
})

const unavailable: ManagedNode = {
  mode: "unavailable",
  home: null,
  pid: null,
  stop: () => {},
}

/**
 * Adopt an already-running local Pylon node, or launch one if none is
 * discovered. Returns a ManagedNode whose `home` (when non-null) is a directory
 * `discoverPylonHome` scans, so the rest of the app resolves it without change.
 */
export const ensureManagedNode = async (
  options: EnsureManagedNodeOptions,
): Promise<ManagedNode> => {
  const discover = options.discover ?? discoverPylonHome
  const readToken = options.readToken ?? readControlToken
  const fileExists = options.fileExists ?? existsSync
  const spawnNode = options.spawnNode ?? defaultSpawnNode
  const probeReady = options.probeReady ?? defaultProbeReady
  const sleep = options.sleep ?? defaultSleep
  const bunBin = options.bunBin ?? process.execPath
  const readinessTimeoutMs = options.readinessTimeoutMs ?? 30_000
  const readinessIntervalMs = options.readinessIntervalMs ?? 500

  // 1. Adopt an already-running node — never double-spawn.
  const discovered = discover({ env: options.env.PYLON_HOME, cwd: options.cwd })
  if (discovered !== null) {
    return adopted(discovered)
  }

  // 2. No node found. In the dev build, launch the repo's Pylon runtime into a
  //    managed `.pylon-local` home (which discovery scans). Without a repo entry
  //    (packaged build, Phase 2) stay discover-only and honest-offline.
  const located = findDevPylonEntry(options.cwd, fileExists)
  if (located === null) {
    return unavailable
  }

  const managedHome = join(located.repoRoot, MANAGED_HOME_SUBDIR)
  const childEnv: Record<string, string> = {}
  for (const [key, value] of Object.entries(options.env)) {
    if (value !== undefined) childEnv[key] = value
  }
  childEnv.PYLON_HOME = managedHome

  const child = spawnNode({
    command: [bunBin, located.entry],
    cwd: located.repoRoot,
    env: childEnv,
  })

  // 3. Wait for the node to write its control token and bring up the control
  //    server. On timeout we still return launched: the poller surfaces offline
  //    honestly and discovery picks the home up once the token lands.
  const deadline = Date.now() + readinessTimeoutMs
  while (Date.now() < deadline) {
    const token = readToken(managedHome)
    if (
      token !== null &&
      token.length > 0 &&
      (await probeReady(options.controlBaseUrl))
    ) {
      break
    }
    await sleep(readinessIntervalMs)
  }

  return {
    mode: "launched",
    home: managedHome,
    pid: child.pid,
    stop: () => child.kill(),
  }
}
