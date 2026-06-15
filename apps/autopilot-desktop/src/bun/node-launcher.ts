import { existsSync } from "node:fs"
import { dirname, join } from "node:path"
import { discoverPylonHome } from "./node-home"
import { readControlToken } from "./pylon-control"
import type { NodeLaunchStatus } from "../shared/rpc"

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

// Honest node-launch status surfaced as Bun-side state (issue #5011 §2),
// distinct from the live node-state poll (which projects online/offline from the
// control server). No fake "online": `online` is set only after the control
// token + a reachable control server are observed; a readiness timeout or a
// crash with no restart budget left is `failed`. The type is defined in the
// shared rpc module (#5025) so the webview agrees; re-exported here for the
// launcher's existing importers.
//   - launching  : we spawned the child; control server not yet confirmed up.
//   - online     : control token present + control server reachable.
//   - adopted    : an already-running node we did not start (never spawned).
//   - failed     : launch/readiness failed, or the child crashed and exhausted
//                  its restart budget.
//   - unavailable: nothing to launch (no repo entry — packaged build, Phase 2).
export type { NodeLaunchStatus }

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
  // Invoked once when the child process exits (crash or clean exit). The
  // supervisor uses this to restart on crash; one-shot callers may omit it.
  readonly onExit?: (info: { readonly code: number | null }) => void
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
  // Honest status callback (issue #5011 §2). Fired on each launch-lifecycle
  // transition so the Bun main process can surface launching/online/failed
  // (no fake "online"). Optional; pure-bring-up callers may omit it.
  readonly onStatus?: (status: NodeLaunchStatus) => void
  // Forwarded to the spawned child so a supervisor can restart on crash.
  readonly onChildExit?: (info: { readonly code: number | null }) => void
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
  // Notify the supervisor when the child exits so it can restart on crash.
  // Best-effort: `exited` rejects only if the handle is already gone, which is
  // itself an exit, so we still fire onExit (code unknown).
  if (input.onExit) {
    void child.exited
      .then(code => input.onExit?.({ code }))
      .catch(() => input.onExit?.({ code: null }))
  }
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
  const emit = (status: NodeLaunchStatus): void => options.onStatus?.(status)

  // 1. Adopt an already-running node — never double-spawn.
  const discovered = discover({ env: options.env.PYLON_HOME, cwd: options.cwd })
  if (discovered !== null) {
    emit("adopted")
    return adopted(discovered)
  }

  // 2. No node found. In the dev build, launch the repo's Pylon runtime into a
  //    managed `.pylon-local` home (which discovery scans). Without a repo entry
  //    (packaged build, Phase 2) stay discover-only and honest-offline.
  const located = findDevPylonEntry(options.cwd, fileExists)
  if (located === null) {
    emit("unavailable")
    return unavailable
  }

  emit("launching")
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
    ...(options.onChildExit ? { onExit: options.onChildExit } : {}),
  })

  // 3. Wait for the node to write its control token and bring up the control
  //    server. On success the status is honest `online`; on timeout it is
  //    honest `failed` (the poller still surfaces offline; discovery picks the
  //    home up if the token lands later). Either way we return `launched` so the
  //    caller can supervise/stop the child it owns.
  let ready = false
  const deadline = Date.now() + readinessTimeoutMs
  while (Date.now() < deadline) {
    const token = readToken(managedHome)
    if (
      token !== null &&
      token.length > 0 &&
      (await probeReady(options.controlBaseUrl))
    ) {
      ready = true
      break
    }
    await sleep(readinessIntervalMs)
  }
  emit(ready ? "online" : "failed")

  return {
    mode: "launched",
    home: managedHome,
    pid: child.pid,
    stop: () => child.kill(),
  }
}

// --- Supervisor (issue #5011 §2): restart-on-crash with backoff -------------

export type SupervisedNode = {
  // The current launch lifecycle status (last emitted), readable synchronously.
  status(): NodeLaunchStatus
  // The current launch mode of the underlying node.
  mode(): NodeLaunchMode
  // The managed home (when we launched/adopted one), else null.
  home(): string | null
  // Stop supervising and kill the managed child if we launched one. Idempotent;
  // a deliberate stop never triggers a restart. A no-op for adopted/unavailable.
  stop(): void
}

export type SuperviseManagedNodeOptions = Omit<
  EnsureManagedNodeOptions,
  "onChildExit"
> & {
  // Backoff schedule for crash restarts. Defaults to a bounded exponential.
  readonly restartBackoffMs?: ReadonlyArray<number>
  // Max consecutive crash restarts before giving up (status stays `failed`).
  readonly maxRestarts?: number
  // After the child has stayed up at least this long, the crash counter resets
  // so a later, unrelated crash gets the full restart budget again.
  readonly stableUptimeMs?: number
  // Injectable scheduler (defaults to setTimeout). Returns a cancel handle.
  readonly schedule?: (fn: () => void, ms: number) => { cancel(): void }
  readonly now?: () => number
}

const DEFAULT_BACKOFF_MS = [500, 1_000, 2_000, 5_000, 10_000] as const
const DEFAULT_MAX_RESTARTS = 5
const DEFAULT_STABLE_UPTIME_MS = 30_000

const defaultSchedule = (
  fn: () => void,
  ms: number,
): { cancel(): void } => {
  const handle = setTimeout(fn, ms)
  return { cancel: () => clearTimeout(handle) }
}

/**
 * Bring up the local node (adopt-or-launch) and keep a *launched* child alive:
 * restart it on crash with bounded exponential backoff, surface honest
 * launching/online/failed status, and stop it on app close (deliberate stops
 * never restart). Adopted and unavailable nodes are not restarted — we only
 * supervise a child we started ourselves.
 */
export const superviseManagedNode = (
  options: SuperviseManagedNodeOptions,
): SupervisedNode => {
  const backoff = options.restartBackoffMs ?? DEFAULT_BACKOFF_MS
  const maxRestarts = options.maxRestarts ?? DEFAULT_MAX_RESTARTS
  const stableUptimeMs = options.stableUptimeMs ?? DEFAULT_STABLE_UPTIME_MS
  const schedule = options.schedule ?? defaultSchedule
  const now = options.now ?? Date.now

  let stopped = false
  let current: ManagedNode | null = null
  let restarts = 0
  let startedAt = 0
  let lastStatus: NodeLaunchStatus = "launching"
  let pending: { cancel(): void } | null = null

  const setStatus = (status: NodeLaunchStatus): void => {
    lastStatus = status
    options.onStatus?.(status)
  }

  const onChildExit = (info: { readonly code: number | null }): void => {
    // A deliberate stop (app close / supervisor stop) never restarts.
    if (stopped) return
    current = null

    // If the child stayed up long enough, treat this as a fresh failure and
    // reset the restart budget so an unrelated later crash gets full retries.
    if (startedAt > 0 && now() - startedAt >= stableUptimeMs) {
      restarts = 0
    }

    if (restarts >= maxRestarts) {
      setStatus("failed")
      return
    }

    const delay = backoff[Math.min(restarts, backoff.length - 1)] ?? 0
    restarts += 1
    setStatus("launching")
    pending = schedule(() => {
      pending = null
      if (!stopped) void bringUp()
    }, delay)
  }

  const bringUp = async (): Promise<void> => {
    if (stopped) return
    const node = await ensureManagedNode({
      ...options,
      onStatus: setStatus,
      // Only a *launched* child gets a restart hook; adopt/unavailable don't.
      onChildExit,
    })
    if (stopped) {
      // Lost the race with stop(): kill anything we just launched.
      node.stop()
      return
    }
    current = node
    if (node.mode === "launched") {
      startedAt = now()
    }
  }

  // Kick off the first bring-up; fire-and-forget like the prior index.ts call.
  void bringUp()

  return {
    status: () => lastStatus,
    mode: () => current?.mode ?? "unavailable",
    home: () => current?.home ?? null,
    stop: () => {
      if (stopped) return
      stopped = true
      pending?.cancel()
      pending = null
      current?.stop()
      current = null
    },
  }
}
