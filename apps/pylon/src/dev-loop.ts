import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import { basename, dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { createBootstrapSummary, parseBootstrapArgs, type BootstrapSummary } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_DEV_CHECK_SCHEMA = "openagents.pylon.dev_check.v0.3"
export const PYLON_DEV_APPLY_SCHEMA = "openagents.pylon.dev_apply.v0.3"
export const PYLON_DEV_RELOAD_SCHEMA = "openagents.pylon.dev_reload.v0.3"
export const PYLON_DEV_CODEX_RUN_SCHEMA = "openagents.pylon.dev_codex_run.v0.3"

export type PylonDevFileArea =
  | "pylon.codex"
  | "pylon.dev"
  | "pylon.tui"
  | "pylon.tests"
  | "pylon.docs"
  | "pylon.source"
  | "docs"
  | "tests"
  | "source"
  | "unknown"

export type PylonDevChangedFileRef = {
  fileRef: string
  status: "modified" | "added" | "deleted" | "renamed" | "untracked" | "unknown"
  area: PylonDevFileArea
  extension: string | null
}

export type PylonDevChangeSummary = {
  repo: {
    state: "ready" | "not_git"
    rootRef: string | null
    branch: string | null
    commit: string | null
  }
  dirty: {
    state: "clean" | "dirty" | "unknown"
    changedCount: number
    stagedCount: number
    unstagedCount: number
    untrackedCount: number
  }
  changedFileRefs: PylonDevChangedFileRef[]
  areaRefs: string[]
  blockerRefs: string[]
}

export type PylonDevCommandSpec = {
  cwd: string
  argv: string[]
  reasonRef: string
}

export type PylonDevCommandResult = {
  commandRef: string
  reasonRef: string
  cwdRef: string
  argvRef: string
  exitCode: number | null
  status: "passed" | "failed" | "error"
  durationMs: number
  stdoutBytes: number
  stderrBytes: number
  stdoutDigestRef: string | null
  stderrDigestRef: string | null
}

export type PylonDevCheckProjection = {
  schema: typeof PYLON_DEV_CHECK_SCHEMA
  observedAt: string
  action: "check"
  state: "passed" | "failed" | "blocked" | "skipped"
  changeSummary: PylonDevChangeSummary
  checkPlan: {
    state: "ready" | "skipped"
    commandRefs: string[]
    blockerRefs: string[]
  }
  commandResults: PylonDevCommandResult[]
  latestRecordRef: string | null
  branchUntouched: true
  commitUntouched: true
  pushPerformed: false
  blockerRefs: string[]
}

export type PylonDevApplyProjection = {
  schema: typeof PYLON_DEV_APPLY_SCHEMA
  observedAt: string
  action: "apply"
  state: "no_op" | "blocked"
  changeSummary: PylonDevChangeSummary
  latestRecordRef: string | null
  branchUntouched: true
  commitUntouched: true
  pushPerformed: false
  destructiveGitPerformed: false
  blockerRefs: string[]
}

export type PylonDevReloadProjection = {
  schema: typeof PYLON_DEV_RELOAD_SCHEMA
  observedAt: string
  action: "reload"
  state: "noop"
  reasonRef: "dev_reload.no_controlled_process"
  changeSummary: PylonDevChangeSummary
  latestRecordRef: string | null
  branchUntouched: true
  commitUntouched: true
  pushPerformed: false
  destructiveGitPerformed: false
  blockerRefs: string[]
}

export type PylonDevCodexRunRecord = {
  schema: typeof PYLON_DEV_CODEX_RUN_SCHEMA
  observedAt: string
  action: "codex_run"
  repoRef: string
  executionMode: "local_bounded" | "local_supervised_danger"
  sandboxMode: "read-only" | "workspace-write" | "danger-full-access"
  eventCount: number
  commandCount: number
  editedFileCount: number
  totalTokens: number
  changeSummary: PylonDevChangeSummary
  latestRecordRef: string | null
  blockerRefs: string[]
}

export type PylonDevLoopOptions = {
  cwd?: string
  env?: Record<string, string | undefined>
  now?: Date
  summary?: BootstrapSummary
  allowDirty?: boolean
  /**
   * When true, a detached HEAD (or otherwise unknown branch) no longer blocks
   * command execution. The detached state is still reported honestly inside
   * `changeSummary.blockerRefs`. The bounded proof path opts in because it runs
   * over isolated worktrees that are legitimately detached (e.g. worktrees
   * materialized from a pinned commit). Default stays false so the normal dev
   * loop keeps refusing to operate on a detached checkout.
   */
  allowDetached?: boolean
  gitRunner?: (cwd: string, args: string[]) => Promise<string | null>
  commandRunner?: (command: PylonDevCommandSpec) => Promise<PylonDevCommandResult>
  commands?: PylonDevCommandSpec[]
  persist?: boolean
}

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

function digestRef(prefix: string, bytes: string) {
  return stableRef(prefix, bytes)
}

function packageRoot() {
  return resolve(dirname(fileURLToPath(import.meta.url)), "..")
}

function activeRepoCwd(env: Record<string, string | undefined>, fallback: string) {
  const configured = env.PYLON_CODEX_CWD ?? env.PYLON_ACTIVE_REPO
  return configured && configured.trim().length > 0 ? configured : fallback
}

async function defaultGitRunner(cwd: string, args: string[]): Promise<string | null> {
  try {
    const proc = Bun.spawn(["git", ...args], {
      cwd,
      stderr: "ignore",
      stdout: "pipe",
    })
    const [stdout, exitCode] = await Promise.all([new Response(proc.stdout).text(), proc.exited])
    if (exitCode !== 0) return null
    return stdout.replace(/\n$/, "")
  } catch {
    return null
  }
}

function statusKind(code: string): PylonDevChangedFileRef["status"] {
  if (code.includes("?")) return "untracked"
  if (code.includes("R")) return "renamed"
  if (code.includes("D")) return "deleted"
  if (code.includes("A")) return "added"
  if (code.includes("M")) return "modified"
  return "unknown"
}

function parsePorcelainStatus(status: string | null): Array<{ code: string; path: string }> {
  if (!status) return []
  return status
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const code = line.slice(0, 2)
      const rawPath = line.slice(3)
      const path = rawPath.includes(" -> ") ? rawPath.split(" -> ").at(-1) ?? rawPath : rawPath
      return { code, path }
    })
}

function extensionFor(path: string) {
  const name = basename(path)
  const dot = name.lastIndexOf(".")
  if (dot <= 0 || dot === name.length - 1) return null
  return name.slice(dot + 1).toLowerCase()
}

function areaForPath(path: string): PylonDevFileArea {
  if (path === "apps/pylon/README.md") return "pylon.docs"
  if (path.startsWith("apps/pylon/docs/")) return "pylon.docs"
  if (path.startsWith("apps/pylon/tests/")) return "pylon.tests"
  if (path.startsWith("apps/pylon/src/tui/")) return "pylon.tui"
  if (path.startsWith("apps/pylon/src/dev-") || path.startsWith("apps/pylon/src/index.ts")) return "pylon.dev"
  if (path.startsWith("apps/pylon/src/codex-") || path.startsWith("apps/pylon/src/codex-agent")) return "pylon.codex"
  if (path.startsWith("apps/pylon/src/")) return "pylon.source"
  if (path.startsWith("docs/")) return "docs"
  if (path.includes("/tests/") || path.startsWith("tests/")) return "tests"
  if (path.endsWith(".ts") || path.endsWith(".tsx") || path.endsWith(".js") || path.endsWith(".jsx")) return "source"
  return "unknown"
}

function safeChangedFileRef(repoRoot: string, path: string, status: PylonDevChangedFileRef["status"]) {
  return {
    fileRef: stableRef("file.local_change", `${repoRoot}:${path}`),
    status,
    area: areaForPath(path),
    extension: extensionFor(path),
  }
}

export async function collectPylonDevChangeSummary(
  options: PylonDevLoopOptions = {},
): Promise<PylonDevChangeSummary> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const cwd = activeRepoCwd(env, options.cwd ?? process.cwd())
  const git = options.gitRunner ?? defaultGitRunner

  const repoRoot = await git(cwd, ["rev-parse", "--show-toplevel"])
  if (!repoRoot) {
    return {
      repo: {
        state: "not_git",
        rootRef: null,
        branch: null,
        commit: null,
      },
      dirty: {
        state: "unknown",
        changedCount: 0,
        stagedCount: 0,
        unstagedCount: 0,
        untrackedCount: 0,
      },
      changedFileRefs: [],
      areaRefs: [],
      blockerRefs: ["blocker.dev_loop.repo_unknown"],
    }
  }

  const [branch, commit, statusText] = await Promise.all([
    git(repoRoot, ["branch", "--show-current"]),
    git(repoRoot, ["rev-parse", "HEAD"]),
    git(repoRoot, ["status", "--porcelain=v1"]),
  ])
  const statusEntries = parsePorcelainStatus(statusText)
  const changedFileRefs = statusEntries.map((entry) =>
    safeChangedFileRef(repoRoot, entry.path, statusKind(entry.code)),
  )
  const areaRefs = Array.from(new Set(changedFileRefs.map((entry) => `area.${entry.area}`))).sort()
  const stagedCount = statusEntries.filter((entry) => entry.code[0] !== " " && entry.code[0] !== "?").length
  const unstagedCount = statusEntries.filter((entry) => entry.code[1] !== " " && entry.code[1] !== "?").length
  const untrackedCount = statusEntries.filter((entry) => entry.code.includes("?")).length
  return {
    repo: {
      state: "ready",
      rootRef: stableRef("repo.root", repoRoot),
      branch: branch || null,
      commit,
    },
    dirty: {
      state: statusText === null ? "unknown" : statusEntries.length > 0 ? "dirty" : "clean",
      changedCount: statusEntries.length,
      stagedCount,
      unstagedCount,
      untrackedCount,
    },
    changedFileRefs,
    areaRefs,
    blockerRefs: [
      ...(commit ? [] : ["blocker.dev_loop.commit_unknown"]),
      ...(branch ? [] : ["blocker.dev_loop.branch_unknown_or_detached"]),
    ],
  }
}

function commandRef(command: PylonDevCommandSpec) {
  return stableRef("command.dev_check", `${command.cwd}\0${command.argv.join("\0")}`)
}

async function defaultCommandRunner(command: PylonDevCommandSpec): Promise<PylonDevCommandResult> {
  const started = Date.now()
  try {
    const proc = Bun.spawn(command.argv, {
      cwd: command.cwd,
      stderr: "pipe",
      stdout: "pipe",
    })
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])
    return {
      commandRef: commandRef(command),
      reasonRef: command.reasonRef,
      cwdRef: stableRef("command.cwd", command.cwd),
      argvRef: stableRef("command.argv", command.argv.join("\0")),
      exitCode,
      status: exitCode === 0 ? "passed" : "failed",
      durationMs: Date.now() - started,
      stdoutBytes: new TextEncoder().encode(stdout).length,
      stderrBytes: new TextEncoder().encode(stderr).length,
      stdoutDigestRef: stdout ? digestRef("command.stdout", stdout) : null,
      stderrDigestRef: stderr ? digestRef("command.stderr", stderr) : null,
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    return {
      commandRef: commandRef(command),
      reasonRef: command.reasonRef,
      cwdRef: stableRef("command.cwd", command.cwd),
      argvRef: stableRef("command.argv", command.argv.join("\0")),
      exitCode: null,
      status: "error",
      durationMs: Date.now() - started,
      stdoutBytes: 0,
      stderrBytes: new TextEncoder().encode(message).length,
      stdoutDigestRef: null,
      stderrDigestRef: digestRef("command.stderr", message),
    }
  }
}

function inferPylonFocusedTests(summary: PylonDevChangeSummary): string[] {
  const areas = new Set(summary.changedFileRefs.map((entry) => entry.area))
  const tests = new Set<string>()
  if (areas.has("pylon.dev")) {
    tests.add("tests/dev-loop.test.ts")
    tests.add("tests/dev-doctor.test.ts")
  }
  if (areas.has("pylon.tui")) {
    tests.add("tests/tui-commands.test.ts")
    tests.add("tests/tui-render-harness.test.ts")
  }
  if (areas.has("pylon.codex")) {
    tests.add("tests/codex-composer.test.ts")
    tests.add("tests/codex-agent.test.ts")
  }
  if (areas.has("pylon.tests")) tests.add("tests/dev-loop.test.ts")
  if (tests.size === 0 && summary.changedFileRefs.some((entry) => entry.area.startsWith("pylon."))) {
    tests.add("tests/dev-loop.test.ts")
  }
  return Array.from(tests)
}

function inferCheckCommands(summary: PylonDevChangeSummary): PylonDevCommandSpec[] {
  if (summary.repo.state !== "ready") return []
  const tests = inferPylonFocusedTests(summary)
  if (tests.length === 0) return []

  // The real repo root is intentionally not projected, but the focused command
  // needs a local cwd. packageRoot() is stable for Pylon's own test package.
  return [
    {
      cwd: packageRoot(),
      argv: ["bun", "test", ...tests],
      reasonRef: "check.pylon.focused_tests",
    },
  ]
}

function mergeBlockerRefs(...groups: string[][]) {
  return Array.from(new Set(groups.flat())).sort()
}

async function writeLatestRecord(
  summary: BootstrapSummary,
  kind: "check" | "apply" | "reload" | "codex-run",
  value: unknown,
  persist: boolean,
) {
  const recordRef = stableRef("record.pylon_dev", `${kind}:${JSON.stringify(value)}`)
  if (!persist) return recordRef
  const dir = join(summary.paths.home, "dev")
  await mkdir(dir, { recursive: true })
  await writeFile(join(dir, `latest-${kind}.json`), `${JSON.stringify(value, null, 2)}\n`, "utf8")
  await writeFile(join(dir, "latest.json"), `${JSON.stringify(value, null, 2)}\n`, "utf8")
  return recordRef
}

export async function runPylonDevCheck(options: PylonDevLoopOptions = {}): Promise<PylonDevCheckProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const summary = options.summary ?? createBootstrapSummary(parseBootstrapArgs(["--json"]), env as NodeJS.ProcessEnv)
  const observedAt = (options.now ?? new Date()).toISOString()
  const changeSummary = await collectPylonDevChangeSummary(options)
  const dirtyBlockers =
    changeSummary.dirty.untrackedCount > 0 && options.allowDirty !== true
      ? ["blocker.dev_check.dirty_prestate_requires_allow_dirty"]
      : []
  const inferredCommands = options.commands ?? inferCheckCommands(changeSummary)
  const checkPlanBlockers =
    inferredCommands.length > 0 ? [] : ["blocker.dev_check.no_focused_check_ladder"]
  const gatingSummaryBlockers =
    options.allowDetached === true
      ? changeSummary.blockerRefs.filter((ref) => ref !== "blocker.dev_loop.branch_unknown_or_detached")
      : changeSummary.blockerRefs
  const preflightBlockers = mergeBlockerRefs(gatingSummaryBlockers, dirtyBlockers)
  const commandResults: PylonDevCommandResult[] = []

  if (preflightBlockers.length === 0) {
    const runner = options.commandRunner ?? defaultCommandRunner
    for (const command of inferredCommands) {
      commandResults.push(await runner(command))
    }
  }

  const failed = commandResults.some((result) => result.status !== "passed")
  const state: PylonDevCheckProjection["state"] =
    preflightBlockers.length > 0
      ? "blocked"
      : inferredCommands.length === 0
        ? "skipped"
        : failed
          ? "failed"
          : "passed"
  const projection: Omit<PylonDevCheckProjection, "latestRecordRef"> & { latestRecordRef: string | null } = {
    schema: PYLON_DEV_CHECK_SCHEMA,
    observedAt,
    action: "check",
    state,
    changeSummary,
    checkPlan: {
      state: inferredCommands.length > 0 ? "ready" : "skipped",
      commandRefs: inferredCommands.map(commandRef),
      blockerRefs: checkPlanBlockers,
    },
    commandResults,
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    blockerRefs: mergeBlockerRefs(preflightBlockers, state === "skipped" ? checkPlanBlockers : []),
  }
  projection.latestRecordRef = await writeLatestRecord(summary, "check", projection, options.persist !== false)
  assertPublicProjectionSafe(projection)
  return projection
}

export async function runPylonDevApply(options: PylonDevLoopOptions = {}): Promise<PylonDevApplyProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const summary = options.summary ?? createBootstrapSummary(parseBootstrapArgs(["--json"]), env as NodeJS.ProcessEnv)
  const changeSummary = await collectPylonDevChangeSummary(options)
  const blockers =
    changeSummary.dirty.untrackedCount > 0 && options.allowDirty !== true
      ? ["blocker.dev_apply.dirty_prestate_requires_allow_dirty"]
      : []
  const projection: Omit<PylonDevApplyProjection, "latestRecordRef"> & { latestRecordRef: string | null } = {
    schema: PYLON_DEV_APPLY_SCHEMA,
    observedAt: (options.now ?? new Date()).toISOString(),
    action: "apply",
    state: blockers.length > 0 ? "blocked" : "no_op",
    changeSummary,
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    destructiveGitPerformed: false,
    blockerRefs: mergeBlockerRefs(changeSummary.blockerRefs, blockers),
  }
  projection.latestRecordRef = await writeLatestRecord(summary, "apply", projection, options.persist !== false)
  assertPublicProjectionSafe(projection)
  return projection
}

export async function runPylonDevReload(options: PylonDevLoopOptions = {}): Promise<PylonDevReloadProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const summary = options.summary ?? createBootstrapSummary(parseBootstrapArgs(["--json"]), env as NodeJS.ProcessEnv)
  const changeSummary = await collectPylonDevChangeSummary(options)
  const projection: Omit<PylonDevReloadProjection, "latestRecordRef"> & { latestRecordRef: string | null } = {
    schema: PYLON_DEV_RELOAD_SCHEMA,
    observedAt: (options.now ?? new Date()).toISOString(),
    action: "reload",
    state: "noop",
    reasonRef: "dev_reload.no_controlled_process",
    changeSummary,
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    destructiveGitPerformed: false,
    blockerRefs: ["blocker.dev_reload.no_controlled_process"],
  }
  projection.latestRecordRef = await writeLatestRecord(summary, "reload", projection, options.persist !== false)
  assertPublicProjectionSafe(projection)
  return projection
}

export async function recordPylonDevCodexRun(
  input: {
    cwd: string
    executionMode: PylonDevCodexRunRecord["executionMode"]
    sandboxMode: PylonDevCodexRunRecord["sandboxMode"]
    eventCount: number
    commandCount: number
    editedFileCount: number
    totalTokens: number
  },
  options: PylonDevLoopOptions = {},
): Promise<PylonDevCodexRunRecord> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const summary = options.summary ?? createBootstrapSummary(parseBootstrapArgs(["--json"]), env as NodeJS.ProcessEnv)
  const changeSummary = await collectPylonDevChangeSummary({ ...options, cwd: input.cwd })
  const projection: Omit<PylonDevCodexRunRecord, "latestRecordRef"> & { latestRecordRef: string | null } = {
    schema: PYLON_DEV_CODEX_RUN_SCHEMA,
    observedAt: (options.now ?? new Date()).toISOString(),
    action: "codex_run",
    repoRef: stableRef("repo.cwd", input.cwd),
    executionMode: input.executionMode,
    sandboxMode: input.sandboxMode,
    eventCount: input.eventCount,
    commandCount: input.commandCount,
    editedFileCount: input.editedFileCount,
    totalTokens: input.totalTokens,
    changeSummary,
    latestRecordRef: null,
    blockerRefs: changeSummary.blockerRefs,
  }
  projection.latestRecordRef = await writeLatestRecord(summary, "codex-run", projection, options.persist !== false)
  assertPublicProjectionSafe(projection)
  return projection
}
