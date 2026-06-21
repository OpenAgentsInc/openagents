import { existsSync } from "node:fs"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import {
  buildOnboardingChildEnv,
  loadPersistedCredential,
  selfRegisterAgent,
  type SelfRegisterResult,
} from "./agent-onboarding"
import { discoverPylonHome } from "./node-home"
import {
  fetchNodeSparkAddress,
  probeControlCompatibility,
  readControlToken,
} from "./pylon-control"
import type { NodeLaunchStatus } from "../shared/rpc"

// #5011 (JUNE15_LAUNCH_PLAN §0/§4.F): the install seam. Today the desktop only
// *discovers* an already-running local node; a fresh install therefore reports
// honest "offline" forever. This module makes the Bun main process *launch* the
// local Pylon node runtime when none is discovered — so "install Autopilot and
// contribute" is one step — while *adopting* an already-running node when one is
// present (never double-spawning).
//
// Phase 1 (#5011, dev build): launch the repo's `apps/pylon/src/index.ts` under
// Bun, into a managed `.pylon-local` home that `discoverPylonHome` already scans,
// so the rest of the app (resolveHome/poller/commands) picks it up unchanged.
//
// Phase 2 (#5027, packaged build): a shipped `.app` has no repo entry to walk to,
// so the dev path resolves nothing. Instead we look for a *bundled* Pylon node
// entry the build step copied into the app's Resources (electrobun `PATHS`), and
// launch it with the *bundled* Bun (`process.execPath` inside the `.app`) into a
// per-user managed home (a packaged install has no writable repo root). The
// bring-up, supervision, readiness, and stop/restart semantics are identical to
// the dev path — only entry discovery and the managed-home location differ.
//
// Resolution order in `ensureManagedNode`: adopt a running node → dev repo entry
// → bundled packaged entry → honest `unavailable`. When the packaged Pylon
// bundle has not shipped yet, the packaged entry resolves nothing and we stay
// discover-only/honest-offline exactly as before (no regression).

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
  readonly probeCompatible?: (baseUrl: string) => Promise<boolean>
  readonly sleep?: (ms: number) => Promise<void>
  readonly bunBin?: string
  readonly readinessTimeoutMs?: number
  readonly readinessIntervalMs?: number
  // #5027 (Phase 2, packaged build): the app's Resources dir (electrobun
  // `PATHS.RESOURCES_FOLDER`). When the dev repo entry is not resolvable, we look
  // for a bundled Pylon node entry under `<resourcesDir>/app/pylon-node/` and
  // launch it with the bundled Bun. `null`/absent => no packaged path (dev/test).
  readonly resourcesDir?: string | null
  // The user's home directory; the packaged build's managed `.pylon-local` home
  // lives under it (a shipped `.app` has no writable repo root). Injectable.
  readonly homeDir?: string
  // Honest status callback (issue #5011 §2). Fired on each launch-lifecycle
  // transition so the Bun main process can surface launching/online/failed
  // (no fake "online"). Optional; pure-bring-up callers may omit it.
  readonly onStatus?: (status: NodeLaunchStatus) => void
  // Forwarded to the spawned child so a supervisor can restart on crash.
  readonly onChildExit?: (info: { readonly code: number | null }) => void
  // AO-1/AO-2 (#5442/#5443): auto-onboarding. When enabled (the default in the
  // app), the launcher (a) injects the onboarding env switches
  // (PYLON_OPENAGENTS_BASE_URL / OPENAGENTS_AGENT_TOKEN / PYLON_ASSIGNMENT_WORKER)
  // into the node child from a persisted token before spawn, and (b) after the
  // node reports online, self-registers the agent if no token is persisted yet.
  // Disabled (`false`) keeps the prior isolated-node behavior for tests/dev.
  readonly autoOnboarding?: boolean
  // The product base URL to register + announce presence against. Defaults to
  // https://openagents.com (see agent-onboarding DEFAULT_OPENAGENTS_BASE_URL).
  readonly onboardingBaseUrl?: string
  // AO-3 (#5444): when the user chose "use your existing Pylon identity", this is
  // the detected seed-bearing home to boot against (so the existing wallet /
  // payout target / history carry over instead of being forked). When set, the
  // launcher forces PYLON_HOME to it and skips minting a fresh managed home. The
  // home is verified to hold an identity seed BEFORE it reaches here
  // (saveIdentityChoice re-checks the marker); we never adopt the wrong home.
  // Null/absent => the default fresh "create new" managed-home path.
  readonly useExistingHome?: string | null
  // AO-3 (#5444): the display name the user chose for a freshly created identity
  // ("create new" path). Threaded into self-registration so the agent registers
  // under the user's chosen name rather than the neutral auto default. Ignored
  // for the `useExistingHome` path (that identity is already named/registered).
  readonly onboardingDisplayName?: string | null
  // Injectable self-registration (defaults to the real `selfRegisterAgent`).
  // Returns the outcome so the supervisor can decide whether a restart (to pick
  // up a freshly minted token in the child env) is warranted.
  readonly registerAgent?: (input: {
    readonly home: string
    readonly baseUrl: string
    readonly displayName: string | null
  }) => Promise<SelfRegisterResult>
  // Injectable persisted-token reader (defaults to `loadPersistedCredential`).
  readonly readPersistedToken?: (home: string) => string | null
  // AF-1 (#5898): injectable reader for the node's OWN Spark receive address
  // (defaults to a control-server `wallet.spark_backup_status` read). Returns
  // null until the wallet is receive-ready. Payment material — the result is
  // only forwarded into the authenticated registration body, never logged.
  readonly readSparkAddress?: (input: {
    readonly home: string
    readonly controlBaseUrl: string
  }) => Promise<string | null>
  // Fired after a *fresh* registration mints a token so the supervisor can
  // restart the child with the token injected (the env is read once at boot).
  // Not fired when a token was already persisted before spawn (no restart
  // needed — the env already carried it).
  readonly onTokenMinted?: () => void
}

const MANAGED_HOME_SUBDIR = ".pylon-local"
const PYLON_ENTRY_RELATIVE = join("apps", "pylon", "src", "index.ts")

// Where the desktop build step places a *bundled* Pylon node entry inside the
// packaged app's Resources. electrobun copies `build.copy` sources under
// `<RESOURCES_FOLDER>/app/...`, so a `copy` of `"<pylon-bundle>": "pylon-node"`
// lands the entry at `<RESOURCES_FOLDER>/app/pylon-node/<file>`. We try a
// prebuilt single-file bundle first (`index.js`), then a TS entry as a fallback
// for a source copy. See node-launcher's Phase 2 header and the §F2 follow-up.
const PACKAGED_PYLON_DIR = join("app", "pylon-node")
const PACKAGED_PYLON_ENTRIES = ["index.js", "index.ts"] as const

// The launch args the bundled headless Pylon node is invoked with. `node` is the
// headless control-server mode (Pylon issue #4740): it brings up the loopback
// control server + coordinator without the OpenTUI dashboard renderer, which is
// exactly what the desktop drives. (The dev path inherits Pylon's default arg
// handling; the packaged headless bundle is launched explicitly in node mode.)
const PACKAGED_PYLON_ARGS = ["node"] as const

// Per-user managed home for a packaged install. A shipped `.app` has no writable
// repo root, so the launched node's `.pylon-local` home lives under the user's
// home directory, where `discoverPylonHome` also scans (PYLON_HOME is forced to
// it so discovery + the poller pick the launched node up unchanged).
const PACKAGED_HOME_PARENT = join(".openagents", "autopilot-desktop")

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

// Find a *bundled* Pylon node entry inside a packaged app's Resources dir. The
// desktop build step copies a (prebuilt, headless) Pylon node bundle to
// `<resourcesDir>/app/pylon-node/`; we return the first existing known entry.
// Returns null when no bundle was shipped (the build is unsigned/early, or the
// Pylon-side headless artifact does not exist yet) — the caller then stays
// honest-`unavailable`. `resourcesDir` is electrobun's `PATHS.RESOURCES_FOLDER`
// at runtime; passing `null` (e.g. a dev/test run with no packaged resources)
// short-circuits to null.
export const findPackagedPylonEntry = (
  resourcesDir: string | null,
  fileExists: (path: string) => boolean = existsSync,
): { readonly entry: string; readonly bundleDir: string } | null => {
  if (resourcesDir === null || resourcesDir.length === 0) return null
  const bundleDir = join(resourcesDir, PACKAGED_PYLON_DIR)
  for (const file of PACKAGED_PYLON_ENTRIES) {
    const entry = join(bundleDir, file)
    if (fileExists(entry)) return { entry, bundleDir }
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
  const probeCompatible = options.probeCompatible ?? ((baseUrl: string) => probeControlCompatibility({ baseUrl }))
  const sleep = options.sleep ?? defaultSleep
  const bunBin = options.bunBin ?? process.execPath
  const homeDir = options.homeDir ?? homedir()
  const resourcesDir = options.resourcesDir ?? null
  const readinessTimeoutMs = options.readinessTimeoutMs ?? 30_000
  const readinessIntervalMs = options.readinessIntervalMs ?? 500
  const emit = (status: NodeLaunchStatus): void => options.onStatus?.(status)

  // AO-3 (#5444): when the user chose "use existing identity", that detected
  // seed-bearing home is authoritative — it overrides PYLON_HOME for both
  // discovery (adopt an already-running existing node) and launch (boot the
  // existing home so its wallet/payout/history carry over, no fork). An explicit
  // env PYLON_HOME still wins (operator override), matching bootstrap.ts.
  const chosenExistingHome =
    typeof options.useExistingHome === "string" &&
    options.useExistingHome.length > 0
      ? options.useExistingHome
      : null
  const discoveryEnvHome = options.env.PYLON_HOME ?? chosenExistingHome ?? undefined

  // 1. Adopt an already-running node — never double-spawn.
  const discovered = discover({ env: discoveryEnvHome, cwd: options.cwd })
  if (discovered !== null) {
    if (!(await probeCompatible(options.controlBaseUrl))) {
      emit("failed")
      return unavailable
    }
    emit("adopted")
    return adopted(discovered)
  }

  // 2. No node found. Resolve a launch plan:
  //    - dev build: launch the repo's `apps/pylon/src/index.ts` (default args)
  //      into a managed `.pylon-local` home at the repo root (discovery scans it);
  //    - packaged build (#5027): launch the bundled headless Pylon node entry
  //      with the bundled Bun into a per-user managed home under the home dir.
  //    With neither resolvable (early/unsigned build or no shipped bundle) stay
  //    discover-only and honest-offline.
  const dev = findDevPylonEntry(options.cwd, fileExists)
  const plan: {
    readonly command: ReadonlyArray<string>
    readonly cwd: string
    readonly managedHome: string
  } | null = dev
    ? {
        command: [bunBin, dev.entry],
        cwd: dev.repoRoot,
        managedHome: join(dev.repoRoot, MANAGED_HOME_SUBDIR),
      }
    : (() => {
        const packaged = findPackagedPylonEntry(resourcesDir, fileExists)
        if (packaged === null) return null
        return {
          command: [bunBin, packaged.entry, ...PACKAGED_PYLON_ARGS],
          cwd: packaged.bundleDir,
          managedHome: join(homeDir, PACKAGED_HOME_PARENT, MANAGED_HOME_SUBDIR),
        }
      })()

  if (plan === null) {
    emit("unavailable")
    return unavailable
  }

  emit("launching")
  // AO-3 (#5444): boot the user's chosen existing home when "use existing" was
  // selected; otherwise the freshly minted managed home ("create new"). We only
  // ever launch INTO the chosen home — never overwrite a different one. An
  // explicit env PYLON_HOME still wins (operator override).
  const managedHome =
    options.env.PYLON_HOME ?? chosenExistingHome ?? plan.managedHome
  const childEnv: Record<string, string> = {}
  for (const [key, value] of Object.entries(options.env)) {
    if (value !== undefined) childEnv[key] = value
  }
  childEnv.PYLON_HOME = managedHome

  // Owner mandate (2026-06-19): nothing the desktop runs is sandboxed. The
  // desktop launches its OWN authenticated local Pylon node, so its Codex
  // control sessions run full-access with network enabled (git / GitHub /
  // credentials work — a no-network sandbox is exactly why "Codex didn't connect
  // to GitHub"). This forwards the node-boot opt-in the control-session executor
  // reads (codexControlSessionNoSandboxOptIn); it is a local node-boot env, not a
  // `session.spawn` wire field, so the remote-spawn danger-mode rejection stays
  // intact. An explicit operator override still wins.
  if (childEnv.PYLON_CODEX_NO_SANDBOX === undefined) {
    childEnv.PYLON_CODEX_NO_SANDBOX = "1"
  }

  // AO-2 (#5443): when auto-onboarding is on, inject the env switches the
  // existing Pylon runtime reads at boot so presence / payout-target / the
  // Tassadar assignment worker light up — reusing the already-built runtime, no
  // new node code. The agent token comes from a *persisted* credential
  // (AO-1/#5442) when one already exists for this home; on the very first run
  // there is no token yet, so this boot carries only the base URL and the
  // supervisor restarts the child with the token after registration. The env is
  // read once at node boot (apps/pylon/src/index.ts), which is why a fresh
  // registration needs a restart rather than an in-place env mutation.
  if (options.autoOnboarding === true) {
    const readPersistedToken =
      options.readPersistedToken ??
      ((home: string) => loadPersistedCredential(home)?.token ?? null)
    const persistedToken = readPersistedToken(managedHome)
    const onboarded = buildOnboardingChildEnv({
      base: childEnv,
      agentToken: persistedToken,
      ...(options.onboardingBaseUrl
        ? { baseUrl: options.onboardingBaseUrl }
        : {}),
    })
    for (const [key, value] of Object.entries(onboarded)) childEnv[key] = value
  }

  const child = spawnNode({
    command: plan.command,
    cwd: plan.cwd,
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

  // AO-1 (#5442): once the node is online (its identity.json is written), self-
  // register the agent if no token is persisted yet, then persist the minted
  // token. Idempotent + offline-tolerant: a reused token is a no-op; a deferred
  // (offline) result just retries on the next bring-up. When a token is *freshly*
  // minted we fire onTokenMinted so the supervisor restarts the child with the
  // token in its env (presence/assignment read env once at boot). This never
  // throws and never logs the token (selfRegisterAgent redacts).
  if (ready && options.autoOnboarding === true) {
    const baseUrl =
      options.onboardingBaseUrl ?? "https://openagents.com"
    // AO-3 (#5444): a user-chosen "create new" name flows into registration so
    // the agent registers under it. Ignored for "use existing" (the existing
    // identity already has its own name/registration).
    const displayName =
      chosenExistingHome === null
        ? (options.onboardingDisplayName ?? null)
        : null
    // AF-1 (#5898): best-effort reader for the node's OWN Spark receive address
    // so the DEFAULT registration can land tip readiness as `spark_address`. The
    // wallet may not be receive-ready this early in bring-up; a null result
    // simply registers without it (unchanged behavior). Payment material — never
    // logged here. Confined to the default registration closure so an injected
    // `registerAgent` (tests) performs no network read.
    const readSparkAddress =
      options.readSparkAddress ??
      (async (input: { home: string; controlBaseUrl: string }) => {
        const controlToken = readToken(input.home)
        if (controlToken === null || controlToken.length === 0) return null
        return fetchNodeSparkAddress({
          baseUrl: input.controlBaseUrl,
          token: controlToken,
        })
      })
    const registerAgent =
      options.registerAgent ??
      (async (input: {
        home: string
        baseUrl: string
        displayName: string | null
      }) => {
        let sparkAddress: string | null = null
        try {
          sparkAddress = await readSparkAddress({
            home: input.home,
            controlBaseUrl: options.controlBaseUrl,
          })
        } catch {
          // A failed Spark-address read must never block registration.
          sparkAddress = null
        }
        return selfRegisterAgent({
          home: input.home,
          baseUrl: input.baseUrl,
          displayName: input.displayName,
          sparkAddress,
        })
      })
    try {
      const result = await registerAgent({ home: managedHome, baseUrl, displayName })
      if (result.outcome === "registered") {
        // A token now exists but the running child booted without it. Ask the
        // supervisor to restart so the child re-reads the env with the token.
        options.onTokenMinted?.()
      }
    } catch {
      // Defensive: registerAgent is contractually non-throwing, but never let a
      // registration hiccup crash the launcher / kill the node.
    }
  }

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
  "onChildExit" | "onTokenMinted"
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

    // AO-1/AO-2: a deliberate token-minted restart is not a crash. Restart
    // immediately and do not consume the crash budget. The flag is cleared in
    // bringUp once the restarted child is live.
    if (restartingForToken) {
      setStatus("launching")
      void bringUp()
      return
    }

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

  // AO-1/AO-2 (#5442/#5443): when the post-online self-registration mints a
  // *fresh* token, the running child booted without it (env is read once at
  // boot). We restart the child once so it re-reads its env with the token,
  // which un-gates presence / payout-target / the Tassadar assignment worker.
  // A deliberate restart, not a crash: it must not consume the crash budget and
  // must not loop (a reused token on the next bring-up returns no "minted").
  //
  // `onTokenMinted` fires from *inside* `ensureManagedNode` (post-online,
  // before it returns), so `current` is not yet assigned. We record the intent
  // and act on it in `bringUp` once `current` is set.
  let restartingForToken = false
  let tokenRestartPending = false
  const onTokenMinted = (): void => {
    if (stopped || restartingForToken) return
    tokenRestartPending = true
  }

  const bringUp = async (): Promise<void> => {
    if (stopped) return
    tokenRestartPending = false
    const node = await ensureManagedNode({
      ...options,
      onStatus: setStatus,
      // Only a *launched* child gets a restart hook; adopt/unavailable don't.
      onChildExit,
      onTokenMinted,
    })
    if (stopped) {
      // Lost the race with stop(): kill anything we just launched.
      node.stop()
      return
    }
    current = node
    restartingForToken = false
    if (node.mode === "launched") {
      startedAt = now()
    }
    // A fresh token was minted while this child was booting without it. Restart
    // it once so it re-reads the env (now including OPENAGENTS_AGENT_TOKEN +
    // PYLON_ASSIGNMENT_WORKER). The kill triggers onChildExit, which restarts.
    if (tokenRestartPending && node.mode === "launched") {
      tokenRestartPending = false
      restartingForToken = true
      node.stop()
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
