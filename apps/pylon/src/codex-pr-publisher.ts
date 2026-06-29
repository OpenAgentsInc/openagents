import { existsSync } from "node:fs"
import { createHash } from "node:crypto"
import { join, resolve } from "node:path"
import {
  captureWorkspaceChanges,
  type WorkspaceChangeCapture,
} from "./workspace-materializer.js"

/**
 * Assignment pull-request publisher (issue #6439, dedup hardening #6439-reopen).
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
 *  - ONE PR per ISSUE. The dedup key is the public issue number carried by the
 *    objective summary, NOT the per-run assignment ref. Every supervisor run
 *    mints a fresh assignment ref, so keying on it let the same issue spawn one
 *    new branch and one new PR per run (the duplicate-PR bug: 123 PRs across 49
 *    issues). We now (a) look up any existing OPEN `pylon/assignment-*` PR that
 *    references the issue and reuse it instead of opening a second, and (b) make
 *    the branch deterministic from the issue number so retries land on the same
 *    branch/PR. Assignments with no issue ref fall back to the assignment-ref
 *    branch and head-branch dedup.
 *  - NEVER push to the base branch. We push only to the assignment/issue branch
 *    (always under `pylon/assignment-`) and open a PR against the base.
 *  - Empty diff => `no_change`, no branch, no push, no PR.
 *  - Real, issue-specific title + body. The conventional-commit title and the
 *    structured body (`Addresses #N.` / `### Changes` / `### Verification`) are
 *    generated from the issue and diff via the injected own-capacity generator,
 *    with a safe deterministic fallback derived from the issue title and diff
 *    when generation is unavailable or returns nothing usable.
 *  - Public-safe only. The branch name, commit message, and PR body carry the
 *    issue ref, a public summary, the file-change set, and the verification
 *    command + result. They never carry raw prompts, secrets, provider
 *    payloads, or local cache paths.
 *  - Fail-soft. Any git/gh failure returns a typed `failed` result with a
 *    public-safe reason ref; it never throws into the closeout.
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

/** Output of a title/body generator (the own-capacity model or a stub). */
export type AssignmentPrTitleBody = {
  title: string
  body: string
}

/**
 * Generates an issue-specific PR title and body from the issue and diff. Wired
 * by the executor to the own-capacity Codex model. Must never throw and must
 * return `null` when it cannot produce a usable result so the deterministic
 * fallback takes over.
 */
export type AssignmentPrTitleBodyGenerator = (input: {
  issueRef: string | null
  issueNumber: number | null
  issueTitle: string | null
  objectiveSummary?: string
  changedPaths: string[]
  diffStat: string
  diffText: string
  verifyCommand: string
  verifyExitCode: number
  verifyPassed: boolean
}) => Promise<AssignmentPrTitleBody | null>

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
  /** Own-capacity title/body generator; deterministic fallback when omitted. */
  generateTitleBody?: AssignmentPrTitleBodyGenerator
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
const MAX_SUBJECT_LENGTH = 72
const MAX_CHANGE_BULLETS = 40
const MAX_DIFF_TEXT_CHARS = 12000

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const conventionalTitlePattern =
  /^(feat|fix|docs|test|chore|refactor|perf|build|ci|style|revert)(\([A-Za-z0-9_.\-/]+\))?!?: .+/

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

/**
 * Deterministic, public-safe branch name keyed on the ISSUE number, so every
 * run that resolves the same issue lands on the same branch (and therefore the
 * same PR) instead of minting a new branch per assignment ref.
 */
export function issueBranchName(issueNumber: number): string {
  return `${ASSIGNMENT_PR_BRANCH_PREFIX}issue-${issueNumber}`
}

/** Extracts the first `#NNNN` issue reference from a public objective summary. */
export function issueRefFromSummary(summary: string | undefined): string | null {
  if (typeof summary !== "string") return null
  const match = summary.match(/#(\d{1,7})\b/)
  return match === null ? null : `#${match[1]}`
}

/** Extracts the first issue number from a public objective summary. */
export function issueNumberFromSummary(summary: string | undefined): number | null {
  const ref = issueRefFromSummary(summary)
  if (ref === null) return null
  const value = Number.parseInt(ref.slice(1), 10)
  return Number.isFinite(value) ? value : null
}

/**
 * Collapses arbitrary text into a single-line, bounded, control-character-free
 * subject. Returns "" when empty so callers can fall back.
 */
function sanitizeSubject(raw: string, maxLength = MAX_SUBJECT_LENGTH): string {
  const cleaned = raw
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u001F\u007F]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
  if (cleaned.length === 0) return ""
  return cleaned.length > maxLength ? `${cleaned.slice(0, maxLength - 1).trimEnd()}…` : cleaned
}

/** True when a string already reads as a conventional-commit subject. */
export function looksConventional(title: string): boolean {
  return conventionalTitlePattern.test(title.trim())
}

function meaningfulSummary(summary: string | undefined): string {
  const s = (summary ?? "").trim()
  if (s.length === 0) return ""
  // The supervisor's stock prompt summary is generic boilerplate; treat it as
  // empty so the title falls back to the real issue title / diff-derived scope.
  if (/implement public issue #\d+ and run the (named )?verification/i.test(s)) return ""
  return s.replace(/#\d+/g, "").replace(/\s+/g, " ").trim()
}

function conventionalTypeFromPaths(paths: string[]): "docs" | "test" | "feat" {
  if (paths.length === 0) return "feat"
  const isDoc = (p: string) => p.endsWith(".md") || p.startsWith("docs/") || p.includes("/docs/")
  const isTest = (p: string) =>
    /(^|\/)tests?\//.test(p) || /\.test\.[cm]?[jt]sx?$/.test(p) || /\.spec\.[cm]?[jt]sx?$/.test(p)
  if (paths.every(isDoc)) return "docs"
  if (paths.every(isTest)) return "test"
  return "feat"
}

function scopeForPath(path: string): string | null {
  const parts = path.split("/").filter((p) => p.length > 0)
  if (parts.length < 2) return null
  if (parts[0] === "apps" && parts[1] === "openagents.com") {
    if (parts.includes("api")) return "api"
    if (parts.includes("web")) return "web"
    return "web"
  }
  if (parts[0] === "apps" || parts[0] === "packages" || parts[0] === "clients") {
    return parts[1] ?? null
  }
  return parts[0]
}

function conventionalScopeFromPaths(paths: string[]): string | null {
  const counts = new Map<string, number>()
  for (const path of paths) {
    const scope = scopeForPath(path)
    if (scope === null) continue
    counts.set(scope, (counts.get(scope) ?? 0) + 1)
  }
  let best: string | null = null
  let bestCount = 0
  for (const [scope, count] of counts) {
    if (count > bestCount) {
      best = scope
      bestCount = count
    }
  }
  return best
}

/** Deterministic conventional-commit title from the issue + diff. */
export function deriveConventionalTitle(input: {
  issueNumber: number | null
  issueTitle?: string | null
  objectiveSummary?: string
  changedPaths: string[]
}): string {
  const issueTitle = (input.issueTitle ?? "").trim()
  // A good issue title may already be conventional; keep it verbatim.
  if (looksConventional(issueTitle)) return sanitizeSubject(issueTitle)

  let base = issueTitle
  if (base.length === 0) base = meaningfulSummary(input.objectiveSummary)
  if (base.length === 0) {
    base = input.issueNumber !== null ? `resolve issue #${input.issueNumber}` : "apply assignment change"
  }
  base = base.replace(/\.+\s*$/, "").trim()
  if (base.length > 0) base = base.charAt(0).toLowerCase() + base.slice(1)

  const type = conventionalTypeFromPaths(input.changedPaths)
  const rawScope = conventionalScopeFromPaths(input.changedPaths)
  // A scope that just restates the type (e.g. docs/docs) adds no information.
  const scope = rawScope === null || rawScope === type ? null : rawScope
  const prefix = scope === null ? type : `${type}(${scope})`
  return sanitizeSubject(`${prefix}: ${base}`)
}

function changesBlockFromPaths(changedPaths: string[]): string {
  if (changedPaths.length === 0) return "_No tracked file changes detected._"
  const shown = changedPaths.slice(0, MAX_CHANGE_BULLETS).map((p) => `- \`${p}\``)
  if (changedPaths.length > MAX_CHANGE_BULLETS) {
    shown.push(`- …and ${changedPaths.length - MAX_CHANGE_BULLETS} more file(s)`)
  }
  return shown.join("\n")
}

/** Deterministic structured PR body from the issue + diff + verification. */
export function buildStructuredBody(input: {
  issueNumber: number | null
  changedPaths: string[]
  changesBlock?: string
  verifyCommand: string
  verifyExitCode: number
  verifyPassed: boolean
}): string {
  const changes =
    input.changesBlock !== undefined && input.changesBlock.trim().length > 0
      ? input.changesBlock.trim()
      : changesBlockFromPaths(input.changedPaths)
  const lines = [
    input.issueNumber === null
      ? "Automated OpenAgents Pylon Codex assignment change."
      : `Addresses #${input.issueNumber}.`,
    "",
    "### Changes",
    changes,
    "",
    "### Verification",
    "```",
    input.verifyCommand,
    "```",
    input.verifyPassed
      ? `Passed locally (exit ${input.verifyExitCode}).`
      : `Exited ${input.verifyExitCode} locally.`,
  ]
  return lines.join("\n")
}

/**
 * Normalizes a generated body: ensures the issue link line is present so the PR
 * always closes/links the issue even if the model omitted it.
 */
function normalizeGeneratedBody(body: string, issueNumber: number | null): string {
  const trimmed = body.trim()
  if (issueNumber === null) return trimmed
  const pattern = new RegExp(`(?<!\\d)#${issueNumber}(?!\\d)`)
  if (pattern.test(trimmed)) return trimmed
  return `Addresses #${issueNumber}.\n\n${trimmed}`
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

type ExistingPullRequest = { number: number; url: string; headRefName: string }

/**
 * Finds an existing OPEN fleet PR (`pylon/assignment-*` head branch) that
 * references the given issue number in its title or body. This is the dedup
 * that stops the fleet opening a second PR for an issue that already has one.
 */
export async function findOpenPullRequestForIssue(input: {
  runner: AssignmentPrCommandRunner
  cwd: string
  fullName: string
  issueNumber: number
}): Promise<ExistingPullRequest | null> {
  const res = await input.runner({
    args: [
      "gh",
      "pr",
      "list",
      "--repo",
      input.fullName,
      "--state",
      "open",
      "--search",
      `#${input.issueNumber} in:title in:body`,
      "--json",
      "number,url,headRefName,title,body",
      "--limit",
      "50",
    ],
    cwd: input.cwd,
    timeoutMs: GH_TIMEOUT_MS,
  })
  if (res.exitCode !== 0 || res.timedOut) return null
  let rows: Array<{ number?: unknown; url?: unknown; headRefName?: unknown; title?: unknown; body?: unknown }>
  try {
    rows = JSON.parse(res.stdout)
  } catch {
    return null
  }
  if (!Array.isArray(rows)) return null
  const pattern = new RegExp(`(?<!\\d)#${input.issueNumber}(?!\\d)`)
  for (const row of rows) {
    const headRefName = typeof row.headRefName === "string" ? row.headRefName : ""
    if (!headRefName.startsWith(ASSIGNMENT_PR_BRANCH_PREFIX)) continue
    const title = typeof row.title === "string" ? row.title : ""
    const body = typeof row.body === "string" ? row.body : ""
    if (!pattern.test(title) && !pattern.test(body)) continue
    if (typeof row.number === "number" && typeof row.url === "string") {
      return { number: row.number, url: row.url, headRefName }
    }
  }
  return null
}

async function fetchIssueTitle(input: {
  runner: AssignmentPrCommandRunner
  cwd: string
  fullName: string
  issueNumber: number
}): Promise<string | null> {
  try {
    const res = await input.runner({
      args: [
        "gh",
        "issue",
        "view",
        String(input.issueNumber),
        "--repo",
        input.fullName,
        "--json",
        "title",
        "-q",
        ".title",
      ],
      cwd: input.cwd,
      timeoutMs: GH_TIMEOUT_MS,
    })
    if (res.exitCode !== 0 || res.timedOut) return null
    const title = res.stdout.trim()
    return title.length === 0 ? null : title
  } catch {
    return null
  }
}

async function gitDiffStat(input: {
  runner: AssignmentPrCommandRunner
  cwd: string
}): Promise<string> {
  try {
    const res = await input.runner({
      args: ["git", "-c", "core.pager=cat", "diff", "--stat", "HEAD", "--"],
      cwd: input.cwd,
      timeoutMs: GIT_TIMEOUT_MS,
    })
    return res.exitCode === 0 && !res.timedOut ? res.stdout.trim() : ""
  } catch {
    return ""
  }
}

async function gitDiffText(input: {
  runner: AssignmentPrCommandRunner
  cwd: string
}): Promise<string> {
  try {
    const res = await input.runner({
      args: ["git", "-c", "core.pager=cat", "diff", "HEAD", "--"],
      cwd: input.cwd,
      timeoutMs: GIT_TIMEOUT_MS,
    })
    if (res.exitCode !== 0 || res.timedOut) return ""
    return res.stdout.length > MAX_DIFF_TEXT_CHARS
      ? `${res.stdout.slice(0, MAX_DIFF_TEXT_CHARS)}\n…(diff truncated)`
      : res.stdout
  } catch {
    return ""
  }
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

  const issueRef = issueRefFromSummary(input.objectiveSummary)
  const issueNumber = issueNumberFromSummary(input.objectiveSummary)
  const baseBranch = input.repository.branch
  const remoteUrl = githubRemoteUrl(input.repository.fullName)
  const changedPaths = capture.local.changedPaths

  // ONE PR PER ISSUE (the core dedup fix). Before doing any branch/push/create
  // work, reuse an existing open fleet PR for this issue instead of opening a
  // duplicate. Keyed on the issue number, not the per-run assignment ref.
  if (issueNumber !== null) {
    const existing = await findOpenPullRequestForIssue({
      runner,
      cwd: workingDirectory,
      fullName: input.repository.fullName,
      issueNumber,
    })
    if (existing !== null) {
      console.warn(
        `[pylon-pr-publisher] existing open PR #${existing.number} for issue #${issueNumber}; not duplicating`,
      )
      return {
        state: "opened",
        prUrl: existing.url,
        prNumber: existing.number,
        branch: existing.headRefName,
        changedCount: capture.changedCount,
        reused: true,
      }
    }
  }

  const branch = issueNumber !== null ? issueBranchName(issueNumber) : assignmentBranchName(input.assignmentRef)

  // Gather diff context for title/body generation (best-effort).
  const issueTitle =
    issueNumber === null
      ? null
      : await fetchIssueTitle({ runner, cwd: workingDirectory, fullName: input.repository.fullName, issueNumber })
  const diffStat = await gitDiffStat({ runner, cwd: workingDirectory })
  const verifyCommand = input.verification.args.join(" ")

  let title: string | null = null
  let body: string | null = null
  if (input.generateTitleBody !== undefined) {
    try {
      const diffText = await gitDiffText({ runner, cwd: workingDirectory })
      const generated = await input.generateTitleBody({
        issueRef,
        issueNumber,
        issueTitle,
        ...(input.objectiveSummary === undefined ? {} : { objectiveSummary: input.objectiveSummary }),
        changedPaths,
        diffStat,
        diffText,
        verifyCommand,
        verifyExitCode: input.verification.exitCode,
        verifyPassed: input.verification.passed,
      })
      if (generated !== null) {
        const candidate = sanitizeSubject(generated.title)
        if (candidate.length > 0) title = candidate
        const candidateBody = generated.body.trim()
        if (candidateBody.length > 0) body = normalizeGeneratedBody(candidateBody, issueNumber)
      }
    } catch {
      // fall through to deterministic generation
    }
  }

  if (title === null) {
    title = deriveConventionalTitle({
      issueNumber,
      issueTitle,
      ...(input.objectiveSummary === undefined ? {} : { objectiveSummary: input.objectiveSummary }),
      changedPaths,
    })
  }
  if (body === null) {
    body = buildStructuredBody({
      issueNumber,
      changedPaths,
      changesBlock: diffStat.length > 0 ? changesBlockFromPaths(changedPaths) : undefined,
      verifyCommand,
      verifyExitCode: input.verification.exitCode,
      verifyPassed: input.verification.passed,
    })
  }

  // Create the assignment/issue branch at the pinned base commit (the detached
  // HEAD of the bounded worktree) without disturbing the working-tree changes.
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

  const commit = await runner({
    args: [
      "git",
      "-c",
      "user.email=pylon-codex@users.noreply.github.com",
      "-c",
      "user.name=OpenAgents Pylon Codex",
      "commit",
      "-m",
      title,
      "-m",
      issueNumber === null ? `Assignment: ${input.assignmentRef}` : `Addresses #${issueNumber}.`,
      "-m",
      "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>",
    ],
    cwd: workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (commit.exitCode !== 0 || commit.timedOut) {
    return { state: "failed", reasonRef: "pull_request.commit_failed", branch, changedCount: capture.changedCount }
  }

  // Push only the assignment/issue branch. The `+` allows re-pushing the same
  // branch on a retry; it can never touch the base branch.
  const push = await runner({
    args: ["git", "push", remoteUrl, `+HEAD:refs/heads/${branch}`],
    cwd: workingDirectory,
    timeoutMs: PUSH_TIMEOUT_MS,
  })
  if (push.exitCode !== 0 || push.timedOut) {
    return { state: "failed", reasonRef: "pull_request.push_failed", branch, changedCount: capture.changedCount }
  }

  // Second-guard dedup: reuse an existing open PR for this exact head branch
  // (covers concurrent retries of the same issue/assignment branch).
  const existingForBranch = await runner({
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
  if (existingForBranch.exitCode === 0 && !existingForBranch.timedOut) {
    try {
      const rows = JSON.parse(existingForBranch.stdout) as Array<{ url?: string; number?: number }>
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
      title,
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
