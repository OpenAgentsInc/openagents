/**
 * Retroactive assignment-PR recovery (issue #6439).
 *
 * Context: before the PR-per-assignment wiring landed, the Khala -> Pylon ->
 * Codex fleet executed hundreds of assignments whose verified diffs died in the
 * per-assignment bounded worktrees under the Pylon cache. Those worktrees are
 * retained (24h TTL) and were swept into an archive directory. This tool
 * recovers them into pull requests — SAFELY, not as a blind flood.
 *
 * Two honest constraints shape this tool:
 *
 *  1. The archival move SEVERED the git worktree links. Each archived
 *     `<workspace>/.git` is a gitlink into the shared bare repo's
 *     `worktrees/<name>` admin dir, whose back-pointer is now stale, so an
 *     in-place `git status` fails ("fatal: not a git repository"). The working
 *     files still exist, so a diff is reconstructed by checking out the pinned
 *     base commit into an isolated temp clone and overlaying the archived files.
 *     We never mutate the live bare repo the running fleet is using.
 *
 *  2. The originating GitHub issue number, the verification command, and the
 *     per-issue dedup key are NOT present in any local file. They live in the
 *     server D1 trace tables (agent_traces / token_usage_events / closeout
 *     rows), all keyed by assignmentRef / task_ref. Opening issue-referenced,
 *     verified, deduped PRs therefore REQUIRES a D1-produced map. Without it,
 *     this tool only REPORTS the recoverable candidate set and opens zero PRs.
 *
 * Produce the D1 map (one object keyed by assignmentRef) with, per assignment:
 *   { "issue": 1234, "verify": ["bun","test","..."] }
 * from e.g.:
 *   SELECT task_ref, ... FROM token_usage_events
 *     WHERE provider='pylon-codex-own-capacity' AND demand_source='khala_coding_delegation';
 *   -- join closeout/assignment rows to recover the originating issue number.
 *
 * Usage:
 *   bun apps/pylon/scripts/recover-assignment-prs.ts \
 *     [--archive <dir>]   (default ~/pylon-cache-archive/codex-agent-tasks)
 *     [--state <file>]    (default ~/.pylon-fable/assignment-state.json)
 *     [--leases <dir>]    (default ~/.pylon-fable/cache/workspace-leases)
 *     [--map <file.json>] (D1-produced assignmentRef -> {issue, verify})
 *     [--open]            (actually open PRs; requires --map)
 *     [--limit <n>]
 *     [--json]
 *
 * Safety: report-only by default. `--open` without `--map` is refused. Never
 * touches ~/.codex, never prints tokens, never writes to the live bare repo's
 * index or refs.
 */

import { existsSync } from "node:fs"
import { mkdtemp, readFile, readdir, rm, cp } from "node:fs/promises"
import { homedir, tmpdir } from "node:os"
import { createHash } from "node:crypto"
import { join } from "node:path"
import { publishAssignmentPullRequest } from "../src/codex-pr-publisher.js"

type Args = {
  archive: string
  state: string
  leases: string
  map?: string
  open: boolean
  limit?: number
  json: boolean
}

function parseArgs(argv: string[]): Args {
  const home = homedir()
  const out: Args = {
    archive: join(home, "pylon-cache-archive", "codex-agent-tasks"),
    state: join(home, ".pylon-fable", "assignment-state.json"),
    leases: join(home, ".pylon-fable", "cache", "workspace-leases"),
    open: false,
    json: false,
  }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === "--archive") out.archive = argv[++i] ?? out.archive
    else if (arg === "--state") out.state = argv[++i] ?? out.state
    else if (arg === "--leases") out.leases = argv[++i] ?? out.leases
    else if (arg === "--map") out.map = argv[++i]
    else if (arg === "--open") out.open = true
    else if (arg === "--limit") out.limit = Number.parseInt(argv[++i] ?? "", 10)
    else if (arg === "--json") out.json = true
  }
  return out
}

function workspaceRefForAssignment(assignmentRef: string): string {
  // Mirrors stableRef("workspace.pylon.codex_agent_task", leaseRef) where
  // leaseRef === assignmentRef for khala_coding assignments.
  const hash = createHash("sha256").update(assignmentRef).digest("hex").slice(0, 24)
  return `workspace.pylon.codex_agent_task.${hash}`
}

async function loadAssignmentIndex(statePath: string): Promise<Map<string, string>> {
  const index = new Map<string, string>()
  try {
    const parsed = JSON.parse(await readFile(statePath, "utf8")) as {
      leases?: Record<string, { assignmentRef?: string }>
    }
    for (const [key, value] of Object.entries(parsed.leases ?? {})) {
      const assignmentRef = value.assignmentRef ?? key
      index.set(workspaceRefForAssignment(assignmentRef), assignmentRef)
    }
  } catch {
    // tolerated: index just stays partial
  }
  return index
}

async function leaseBaseFor(leasesDir: string, workspaceRef: string): Promise<{ sourceRef: string; fullName: string; baseCommit: string } | null> {
  try {
    const record = JSON.parse(await readFile(join(leasesDir, `${workspaceRef}.json`), "utf8")) as {
      sourceRef?: string
    }
    const sourceRef = record.sourceRef
    if (typeof sourceRef !== "string") return null
    const sep = sourceRef.lastIndexOf(":")
    if (sep <= 0) return null
    const fullName = sourceRef.slice(0, sep)
    const baseCommit = sourceRef.slice(sep + 1)
    if (!/^[a-f0-9]{40}$/i.test(baseCommit)) return null
    return { sourceRef, fullName, baseCommit }
  } catch {
    return null
  }
}

async function run(args: string[], cwd: string, timeoutMs = 5 * 60 * 1000) {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  let timedOut = false
  const timer = setTimeout(() => {
    timedOut = true
    proc.kill()
  }, timeoutMs)
  try {
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])
    return { exitCode, stdout, stderr, timedOut }
  } finally {
    clearTimeout(timer)
  }
}

/**
 * Reconstructs a recovered assignment's diff in an isolated temp clone of the
 * pinned base commit, overlays the archived working files, verifies, and opens
 * one PR via the shared publisher. Never touches the live bare repo.
 */
async function reconstructAndOpen(input: {
  assignmentRef: string
  archivedWorkspace: string
  fullName: string
  baseCommit: string
  issue: number
  verify: string[]
}): Promise<{ state: string; prUrl?: string; reasonRef?: string }> {
  const cacheRoot = await mkdtemp(join(tmpdir(), "pylon-recover-"))
  const workingDirectory = join(cacheRoot, "ws")
  try {
    const remoteUrl = `https://github.com/${input.fullName}.git`
    const steps: Array<{ args: string[] }> = [
      { args: ["git", "init", "-q", workingDirectory] },
      { args: ["git", "-C", workingDirectory, "remote", "add", "origin", remoteUrl] },
      { args: ["git", "-C", workingDirectory, "fetch", "--depth", "1", "origin", input.baseCommit] },
      { args: ["git", "-C", workingDirectory, "checkout", "--detach", input.baseCommit] },
    ]
    for (const step of steps) {
      const r = await run(step.args, cacheRoot)
      if (r.exitCode !== 0 || r.timedOut) {
        return { state: "failed", reasonRef: `recover.setup_failed:${step.args[1] ?? step.args[0]}` }
      }
    }
    // Overlay archived working files (everything except git metadata) on top of
    // the clean base checkout, so the working tree reflects Codex's changes.
    const entries = await readdir(input.archivedWorkspace, { withFileTypes: true })
    for (const entry of entries) {
      if (entry.name === ".git") continue
      await cp(join(input.archivedWorkspace, entry.name), join(workingDirectory, entry.name), {
        recursive: true,
        force: true,
      })
    }
    const verifyResult = await run(input.verify, workingDirectory)
    const passed = verifyResult.exitCode === 0 && !verifyResult.timedOut
    const result = await publishAssignmentPullRequest({
      cacheRoot,
      workingDirectory,
      workspaceRef: workspaceRefForAssignment(input.assignmentRef),
      sourceRef: `${input.fullName}:${input.baseCommit}`,
      repository: { branch: "main", commitSha: input.baseCommit, fullName: input.fullName },
      assignmentRef: input.assignmentRef,
      objectiveSummary: `Recovered Pylon Codex assignment for issue #${input.issue}.`,
      verification: { args: input.verify, exitCode: verifyResult.exitCode, passed },
    })
    if (result.state === "opened") return { state: "opened", prUrl: result.prUrl }
    if (result.state === "no_change") return { state: "no_change" }
    if (result.state === "skipped") return { state: "skipped", reasonRef: result.reasonRef }
    return { state: "failed", reasonRef: result.reasonRef }
  } finally {
    await rm(cacheRoot, { recursive: true, force: true })
  }
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2))
  if (args.open && args.map === undefined) {
    console.error(
      "refused: --open requires --map <D1 assignmentRef->{issue,verify}>. " +
        "Recovery without the issue/verify map would flood main with unverified, " +
        "issue-less, duplicate PRs. Produce the map from the server D1 trace tables first.",
    )
    process.exit(2)
  }

  const index = await loadAssignmentIndex(args.state)
  let dirs: string[] = []
  try {
    dirs = (await readdir(args.archive)).filter((d) => d.startsWith("workspace.pylon.codex_agent_task."))
  } catch {
    console.error(`archive not found: ${args.archive}`)
    process.exit(1)
  }

  const candidates: Array<{
    workspaceRef: string
    assignmentRef: string | null
    base: { fullName: string; baseCommit: string } | null
  }> = []
  for (const workspaceRef of dirs) {
    const assignmentRef = index.get(workspaceRef) ?? null
    const base = await leaseBaseFor(args.leases, workspaceRef)
    candidates.push({
      workspaceRef,
      assignmentRef,
      base: base === null ? null : { fullName: base.fullName, baseCommit: base.baseCommit },
    })
  }

  const mapped = candidates.filter((c) => c.assignmentRef !== null)
  const withBase = candidates.filter((c) => c.base !== null)

  const report = {
    archive: args.archive,
    totalWorkspaces: dirs.length,
    mappedToAssignmentRef: mapped.length,
    withResolvableBaseCommit: withBase.length,
    note:
      "In-place git recovery is broken by the archival move (severed worktree links). " +
      "Recovery reconstructs each diff from the pinned base commit in an isolated temp clone. " +
      "Safe issue-referenced, verified, deduped PRs require a D1-produced --map.",
  }

  if (!args.open) {
    if (args.json) console.log(JSON.stringify({ ...report, opened: 0 }, null, 2))
    else {
      console.log(`archive: ${report.archive}`)
      console.log(`total archived workspaces: ${report.totalWorkspaces}`)
      console.log(`mapped to assignmentRef:    ${report.mappedToAssignmentRef}`)
      console.log(`with resolvable base commit: ${report.withResolvableBaseCommit}`)
      console.log(`\n${report.note}`)
      console.log(`\nReport-only. Pass --map <file> --open to reconstruct + open deduped PRs.`)
    }
    return
  }

  const recoveryMap = JSON.parse(await readFile(args.map as string, "utf8")) as Record<
    string,
    { issue?: number; verify?: string[] }
  >
  // The map is keyed by assignmentRef (from D1); the local index is mostly
  // pruned, so derive the archive dir + base commit directly from each
  // assignmentRef. Dedup per issue: keep one assignment per issue.
  const byIssue = new Map<
    number,
    { assignmentRef: string; verify: string[]; workspaceRef: string; base: { fullName: string; baseCommit: string } }
  >()
  for (const [assignmentRef, entry] of Object.entries(recoveryMap)) {
    if (entry?.issue === undefined || !Array.isArray(entry.verify) || entry.verify.length === 0) continue
    const workspaceRef = workspaceRefForAssignment(assignmentRef)
    if (!existsSync(join(args.archive, workspaceRef))) continue
    const base = await leaseBaseFor(args.leases, workspaceRef)
    if (base === null) continue
    if (!byIssue.has(entry.issue)) {
      byIssue.set(entry.issue, {
        assignmentRef,
        verify: entry.verify,
        workspaceRef,
        base: { fullName: base.fullName, baseCommit: base.baseCommit },
      })
    }
  }

  const targets = [...byIssue.entries()].slice(0, args.limit ?? Number.MAX_SAFE_INTEGER)
  const opened: Array<{ issue: number; assignmentRef: string; prUrl: string }> = []
  const failures: Array<{ issue: number; assignmentRef: string; reason: string }> = []
  for (const [issue, target] of targets) {
    const result = await reconstructAndOpen({
      assignmentRef: target.assignmentRef,
      archivedWorkspace: join(args.archive, target.workspaceRef),
      fullName: target.base.fullName,
      baseCommit: target.base.baseCommit,
      issue,
      verify: target.verify,
    })
    if (result.state === "opened" && result.prUrl !== undefined) {
      opened.push({ issue, assignmentRef: target.assignmentRef, prUrl: result.prUrl })
    } else {
      failures.push({ issue, assignmentRef: target.assignmentRef, reason: result.reasonRef ?? result.state })
    }
  }

  const final = {
    ...report,
    dedupedPerIssue: byIssue.size,
    attempted: targets.length,
    opened: opened.length,
    failed: failures.length,
    prUrls: opened.map((o) => o.prUrl),
  }
  console.log(JSON.stringify(final, null, 2))
}

void main()
