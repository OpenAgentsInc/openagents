import { existsSync } from "node:fs"
import { createHash } from "node:crypto"
import { join, resolve } from "node:path"
import {
  captureWorkspaceChanges,
  type WorkspaceChangeCapture,
} from "./workspace-materializer.js"

/**
 * Assignment pull-request publisher (issue #6439).
 *
 * The Khala -> Pylon -> Codex own-capacity lane materializes a bounded Git
 * worktree, drives one Codex thread inside it, and verifies the result with the
 * assignment's real verification command. Before this module, that diff died in
 * the local worktree at closeout: the fleet burned tokens and produced zero
 * commits or PRs. This module turns a verified, non-empty diff into exactly one
 * scoped pull request against the public repository's default branch.
 *
 * Hard guardrails (paired with the bounded-workspace invariants in
 * `workspace-materializer.ts` and `codex-agent-executor.ts`):
 *  - ONE PR per assignment. The branch name is deterministic from the
 *    assignment ref, and an existing open PR for that head branch is reused
 *    instead of duplicated.
 *  - NEVER push to the base branch. We push only to
 *    `pylon/assignment-<shortAssignmentRef>` and open a PR against the base.
 *  - Empty diff => `no_change`, no branch, no push, no PR.
 *  - Public-safe only. The branch name, commit message, and PR body carry the
 *    assignment ref, the public objective summary, the pinned base commit, the
 *    file-change count, and the verification command + result. They never carry
 *    raw prompts, secrets, provider payloads, or local cache paths. The diff
 *    itself is the public-repo change Codex already produced inside the bounded
 *    workspace.
 *  - Fail-soft. Any git/gh failure returns a typed `failed` result with a
 *    public-safe reason ref; it never throws into the closeout, so a verified
 *    assignment still closes out accepted even if PR creation is briefly
 *    unavailable.
 */

export const ASSIGNMENT_PR_BRANCH_PREFIX = "pylon/assignment-"

export type AssignmentPrCommandResult = {
  exitCode: number
  stdout: string
  stderr: string
  timedOut: boolean
}

export type AssignmentPrCommandRunner = (input: {
  args: string[]
  cwd: string
  timeoutMs?: number
}) => Promise<AssignmentPrCommandResult>

export type PublishAssignmentPullRequestInput = {
  cacheRoot: string
  workingDirectory: string
  workspaceRef: string
  sourceRef: string
  repository: {
    branch: string
    commitSha: string
    fullName: string
  }
  assignmentRef: string
  objectiveSummary?: string
  verification: {
    args: string[]
    exitCode: number
    passed: boolean
  }
  now?: Date
  runner?: AssignmentPrCommandRunner
}

export type PublishAssignmentPullRequestResult =
  | { state: "no_change" }
  | { state: "skipped"; reasonRef: string }
  | { state: "failed"; reasonRef: string; branch?: string; changedCount?: number }
  | {
      state: "opened"
      prUrl: string
      prNumber: number
      branch: string
      changedCount: number
      reused: boolean
    }

const GIT_TIMEOUT_MS = 60 * 1000
const PUSH_TIMEOUT_MS = 5 * 60 * 1000
const GH_TIMEOUT_MS = 2 * 60 * 1000
const MAX_SUBJECT_LENGTH = 100

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i

async function defaultRunner(input: {
  args: string[]
  cwd: string
  timeoutMs?: number
}): Promise<AssignmentPrCommandResult> {
  const proc = Bun.spawn(input.args, { cwd: input.cwd, stderr: "pipe", stdout: "pipe" })
  let timedOut = false
  const timer =
    input.timeoutMs === undefined
      ? undefined
      : setTimeout(() => {
          timedOut = true
          proc.kill()
        }, input.timeoutMs)
  try {
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])
    return { exitCode, stdout, stderr, timedOut }
  } finally {
    if (timer !== undefined) clearTimeout(timer)
  }
}

/** Stable, deterministic, public-safe branch name for one assignment. */
export function assignmentBranchName(assignmentRef: string): string {
  const short = createHash("sha256").update(assignmentRef).digest("hex").slice(0, 16)
  return `${ASSIGNMENT_PR_BRANCH_PREFIX}${short}`
}

/** Extracts the first `#NNNN` issue reference from a public objective summary. */
export function issueRefFromSummary(summary: string | undefined): string | null {
  if (typeof summary !== "string") return null
  const match = summary.match(/#(\d{1,7})\b/)
  return match === null ? null : `#${match[1]}`
}

/**
 * Collapses a public objective summary into a single-line, bounded commit
 * subject. Control characters are stripped so the value can never break the
 * commit/PR command surface. Falls back to a neutral subject when empty.
 */
function commitSubject(objectiveSummary: string | undefined, issueRef: string | null): string {
  const cleaned = (objectiveSummary ?? "")
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u001F\u007F]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
  const base = cleaned.length === 0 ? "Pylon Codex assignment change" : cleaned
  const suffix = issueRef === null ? "" : ` (${issueRef})`
  const room = Math.max(8, MAX_SUBJECT_LENGTH - suffix.length)
  const trimmed = base.length > room ? `${base.slice(0, room - 1).trimEnd()}…` : base
  return `pylon: ${trimmed}${suffix}`
}

function pullRequestBody(input: {
  assignmentRef: string
  issueRef: string | null
  baseCommit: string
  changedCount: number
  verificationArgs: string[]
  verificationExitCode: number
}): string {
  const verifyCommand = input.verificationArgs.join(" ")
  const lines = [
    "Automated pull request opened by an OpenAgents Pylon Codex assignment.",
    "",
    ...(input.issueRef === null ? [] : [`Refs ${input.issueRef}`, ""]),
    `Assignment: ${input.assignmentRef}`,
    `Base commit: ${input.baseCommit}`,
    `Files changed: ${input.changedCount}`,
    "",
    `Verification command: \`${verifyCommand}\``,
    `Verification result: passed (exit ${input.verificationExitCode})`,
    "",
    "The diff was produced by Codex in a bounded local workspace, and the",
    "verification command passed locally before this PR was opened.",
  ]
  return lines.join("\n")
}

function assertPylonOwnedWorkspaceTarget(input: { cacheRoot: string; workingDirectory: string }) {
  const cacheRoot = resolve(input.cacheRoot)
  const target = resolve(input.workingDirectory)
  if (target === cacheRoot || !target.startsWith(`${cacheRoot}/`)) {
    throw new Error("pull request publish refused: target is outside the Pylon-owned cache root")
  }
}

function githubRemoteUrl(fullName: string): string {
  return `https://github.com/${fullName}.git`
}

function parsePullRequestNumber(url: string): number | null {
  const match = url.match(/\/pull\/(\d+)/)
  if (match === null) return null
  const value = Number.parseInt(match[1], 10)
  return Number.isFinite(value) ? value : null
}

/**
 * Publishes one scoped pull request for a verified, non-empty assignment diff.
 * See the module header for the full guardrail contract. Returns a typed
 * outcome and never throws; the executor maps the outcome to public-safe
 * closeout refs.
 */
export async function publishAssignmentPullRequest(
  input: PublishAssignmentPullRequestInput,
): Promise<PublishAssignmentPullRequestResult> {
  const runner = input.runner ?? defaultRunner
  const workingDirectory = input.workingDirectory

  // Precondition guards: only a Pylon-owned, real git worktree for a public
  // GitHub repo with a pinned base commit is eligible.
  try {
    assertPylonOwnedWorkspaceTarget(input)
  } catch {
    return { state: "skipped", reasonRef: "pull_request.skipped_outside_cache_root" }
  }
  if (!existsSync(workingDirectory) || !existsSync(join(workingDirectory, ".git"))) {
    return { state: "skipped", reasonRef: "pull_request.skipped_not_git_worktree" }
  }
  if (!githubFullNamePattern.test(input.repository.fullName)) {
    return { state: "skipped", reasonRef: "pull_request.skipped_unsupported_repository" }
  }
  if (!gitCommitShaPattern.test(input.repository.commitSha)) {
    return { state: "skipped", reasonRef: "pull_request.skipped_unpinned_base" }
  }
  if (!input.verification.passed) {
    return { state: "skipped", reasonRef: "pull_request.skipped_verification_not_passed" }
  }

  let capture: WorkspaceChangeCapture
  try {
    capture = await captureWorkspaceChanges({
      cacheRoot: input.cacheRoot,
      workingDirectory,
      workspaceRef: input.workspaceRef,
      sourceRef: input.sourceRef,
      ...(input.now === undefined ? {} : { now: input.now }),
    })
  } catch {
    return { state: "failed", reasonRef: "pull_request.capture_failed" }
  }
  if (capture.changedCount === 0) {
    return { state: "no_change" }
  }

  const branch = assignmentBranchName(input.assignmentRef)
  const issueRef = issueRefFromSummary(input.objectiveSummary)
  const baseBranch = input.repository.branch
  const remoteUrl = githubRemoteUrl(input.repository.fullName)

  // Create the assignment branch at the pinned base commit (the detached HEAD
  // of the bounded worktree) without disturbing the working-tree changes.
  const branchCreate = await runner({
    args: ["git", "checkout", "-B", branch],
    cwd: workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (branchCreate.exitCode !== 0 || branchCreate.timedOut) {
    return { state: "failed", reasonRef: "pull_request.branch_create_failed", branch, changedCount: capture.changedCount }
  }

  const stage = await runner({
    args: ["git", "add", "-A"],
    cwd: workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (stage.exitCode !== 0 || stage.timedOut) {
    return { state: "failed", reasonRef: "pull_request.stage_failed", branch, changedCount: capture.changedCount }
  }

  const subject = commitSubject(input.objectiveSummary, issueRef)
  const body = pullRequestBody({
    assignmentRef: input.assignmentRef,
    issueRef,
    baseCommit: capture.baseCommit,
    changedCount: capture.changedCount,
    verificationArgs: input.verification.args,
    verificationExitCode: input.verification.exitCode,
  })
  const commit = await runner({
    args: [
      "git",
      "-c",
      "user.email=pylon-codex@users.noreply.github.com",
      "-c",
      "user.name=OpenAgents Pylon Codex",
      "commit",
      "-m",
      subject,
      "-m",
      `Assignment: ${input.assignmentRef}`,
      "-m",
      "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
    ],
    cwd: workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (commit.exitCode !== 0 || commit.timedOut) {
    return { state: "failed", reasonRef: "pull_request.commit_failed", branch, changedCount: capture.changedCount }
  }

  // Push only the assignment branch. The `+` allows re-pushing the same
  // assignment branch on a retry; it can never touch the base branch.
  const push = await runner({
    args: ["git", "push", remoteUrl, `+HEAD:refs/heads/${branch}`],
    cwd: workingDirectory,
    timeoutMs: PUSH_TIMEOUT_MS,
  })
  if (push.exitCode !== 0 || push.timedOut) {
    return { state: "failed", reasonRef: "pull_request.push_failed", branch, changedCount: capture.changedCount }
  }

  // One PR per assignment: reuse an existing open PR for this head branch.
  const existing = await runner({
    args: [
      "gh",
      "pr",
      "list",
      "--repo",
      input.repository.fullName,
      "--head",
      branch,
      "--state",
      "open",
      "--json",
      "url,number",
      "--limit",
      "1",
    ],
    cwd: workingDirectory,
    timeoutMs: GH_TIMEOUT_MS,
  })
  if (existing.exitCode === 0 && !existing.timedOut) {
    try {
      const rows = JSON.parse(existing.stdout) as Array<{ url?: string; number?: number }>
      const row = rows[0]
      if (row !== undefined && typeof row.url === "string" && typeof row.number === "number") {
        return {
          state: "opened",
          prUrl: row.url,
          prNumber: row.number,
          branch,
          changedCount: capture.changedCount,
          reused: true,
        }
      }
    } catch {
      // fall through to creation
    }
  }

  const create = await runner({
    args: [
      "gh",
      "pr",
      "create",
      "--repo",
      input.repository.fullName,
      "--base",
      baseBranch,
      "--head",
      branch,
      "--title",
      subject,
      "--body",
      body,
    ],
    cwd: workingDirectory,
    timeoutMs: GH_TIMEOUT_MS,
  })
  if (create.exitCode !== 0 || create.timedOut) {
    return { state: "failed", reasonRef: "pull_request.gh_create_failed", branch, changedCount: capture.changedCount }
  }
  const urlMatch = create.stdout.match(/https:\/\/github\.com\/\S+\/pull\/\d+/)
  if (urlMatch === null) {
    return { state: "failed", reasonRef: "pull_request.gh_create_url_missing", branch, changedCount: capture.changedCount }
  }
  const prUrl = urlMatch[0]
  const prNumber = parsePullRequestNumber(prUrl) ?? 0
  return {
    state: "opened",
    prUrl,
    prNumber,
    branch,
    changedCount: capture.changedCount,
    reused: false,
  }
}

export type AssignmentPullRequestPublisher = typeof publishAssignmentPullRequest
