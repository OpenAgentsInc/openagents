/**
 * Typed Git/GitHub host service (EP250 capability E2–E5, #8712).
 *
 * Runs the closed operation set from ./git-github-contract.ts against the
 * active, canonicalized workspace root. Discipline (owner-local executor
 * invariant): the renderer never supplies argv — every operation builds a
 * FIXED argument vector, user strings only ever reach git/gh as bounded,
 * validated, path/ref/message values placed AFTER `--` (or as `-m`/`--title`
 * values that cannot be reinterpreted as flags). Failures are mapped onto the
 * typed `GitGithubErrorCode` classes; raw stderr, tokens, credentials, and
 * absolute paths never cross the bridge.
 *
 * This is main-process code (full owner-local filesystem/exec access, matching
 * the repo's owner-local executor invariant); it is not reachable from an
 * untrusted renderer surface.
 */
import { createHash } from "node:crypto"
import { realpathSync } from "node:fs"
import path from "node:path"
import { spawnSync } from "node:child_process"
import { workspaceGitEnvironment } from "./git-process-environment.ts"

import {
  decodeGitGithubRequest,
  gitGithubError,
  type GitBranch,
  type GitFileEntry,
  type GitDiffHunk,
  type GitFileStatus,
  type GitGithubErrorCode,
  type GitGithubRequest,
  type GitGithubResult,
  type GitHubIssueRef,
  type GitHubPrRef,
} from "./git-github-contract.ts"

const gitTimeoutMs = 10_000
const ghTimeoutMs = 25_000
const maxBuffer = 8_000_000
const maxStatusEntries = 500
const maxBranches = 300
const maxListItems = 50
const maxBodyChars = 8_000
const maxTitleChars = 400
const maxMessageChars = 20_000
const maxReviewDiffBytes = 120_000

// ---------------------------------------------------------------------------
// Bounded exec (no throw; typed outcome)
// ---------------------------------------------------------------------------

type ExecResult =
  | Readonly<{ ok: true; stdout: string }>
  | Readonly<{ ok: false; kind: "enoent" | "timeout" | "nonzero"; code: number | null; stderr: string }>

const runBinary = (bin: string, args: ReadonlyArray<string>, cwd: string, timeoutMs: number): ExecResult => {
  const proc = spawnSync(bin, [...args], {
    cwd,
    encoding: "utf8",
    timeout: timeoutMs,
    maxBuffer,
    // A hung credential prompt must never wedge the host: git/gh run with no
    // inherited stdin, and gh is told never to prompt.
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...workspaceGitEnvironment(),
      GIT_TERMINAL_PROMPT: "0",
      GH_PROMPT_DISABLED: "1",
      GH_NO_UPDATE_NOTIFIER: "1",
    },
  })
  if (proc.error) {
    const err = proc.error as NodeJS.ErrnoException
    if (err.code === "ENOENT") return { ok: false, kind: "enoent", code: null, stderr: "" }
    if (err.code === "ETIMEDOUT") return { ok: false, kind: "timeout", code: null, stderr: "" }
    return { ok: false, kind: "nonzero", code: null, stderr: "" }
  }
  if (proc.status !== 0) {
    return { ok: false, kind: "nonzero", code: proc.status, stderr: (proc.stderr ?? "").toString() }
  }
  return { ok: true, stdout: (proc.stdout ?? "").toString() }
}

// ---------------------------------------------------------------------------
// Root + input validation
// ---------------------------------------------------------------------------

type RepoRoot = Readonly<{ ok: true; root: string }> | Readonly<{ ok: false; error: GitGithubErrorCode }>

const resolveRepoRoot = (root: string | null): RepoRoot => {
  if (root === null) return { ok: false, error: "no_workspace" }
  let canonical: string
  try {
    canonical = realpathSync(path.resolve(root))
  } catch {
    return { ok: false, error: "no_workspace" }
  }
  const probe = runBinary("git", ["-C", canonical, "rev-parse", "--is-inside-work-tree"], canonical, gitTimeoutMs)
  if (!probe.ok) return { ok: false, error: probe.kind === "enoent" ? "git_unavailable" : "not_a_repo" }
  // Exit status is the authority. Bun 1.3 may omit successful child stdout in
  // tests, while Electron/Node returns "true"; requiring the bytes here would
  // turn a verified worktree into `not_a_repo` despite Git's successful probe.
  return { ok: true, root: canonical }
}

/** Repo-relative, containment-checked path; absolute inputs must land inside. */
const safeRepoRelativePath = (root: string, value: string): string | null => {
  if (value === "" || /[\0\r\n]/u.test(value)) return null
  let relative = value
  if (path.isAbsolute(value)) {
    const rel = path.relative(root, path.resolve(value))
    if (rel === "" || rel.startsWith("..") || path.isAbsolute(rel)) return null
    relative = rel
  }
  const normalized = path.posix.normalize(relative.replaceAll("\\", "/"))
  if (normalized === "." || normalized === "" || normalized.startsWith("../") || path.posix.isAbsolute(normalized)) {
    return null
  }
  return normalized
}

/** git check-ref-format-shaped branch validation (defense in depth). */
export const validBranchName = (name: string): boolean =>
  /^[A-Za-z0-9][A-Za-z0-9._/-]{0,200}$/.test(name) &&
  !name.includes("..") &&
  !name.includes("//") &&
  !name.endsWith("/") &&
  !name.endsWith(".lock") &&
  !name.includes("@{")

// ---------------------------------------------------------------------------
// Status (porcelain v2, -z, --branch)
// ---------------------------------------------------------------------------

const statusLetter = (code: string): GitFileStatus | null => {
  switch (code) {
    case "M": return "modified"
    case "A": return "added"
    case "D": return "deleted"
    case "R": return "renamed"
    case "C": return "copied"
    case "T": return "type-changed"
    case "U": return "unmerged"
    default: return null
  }
}

type ParsedStatus = Readonly<{
  branch: string | null
  upstream: string | null
  detached: boolean
  ahead: number
  behind: number
  staged: ReadonlyArray<GitFileEntry>
  unstaged: ReadonlyArray<GitFileEntry>
  untracked: ReadonlyArray<GitFileEntry>
  truncated: boolean
}>

type StatusSnapshot = ParsedStatus & Readonly<{
  repositoryRef: string
  statusRef: string
  headRef: string | null
}>

const opaqueRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`

export const parsePorcelainV2 = (raw: string): ParsedStatus => {
  const parts = raw.split("\0")
  let branch: string | null = null
  let upstream: string | null = null
  let detached = false
  let ahead = 0
  let behind = 0
  const staged: GitFileEntry[] = []
  const unstaged: GitFileEntry[] = []
  const untracked: GitFileEntry[] = []
  let truncated = false

  const bounded = (list: GitFileEntry[], entry: GitFileEntry): void => {
    if (list.length >= maxStatusEntries) { truncated = true; return }
    list.push(entry)
  }

  for (let index = 0; index < parts.length; index++) {
    const record = parts[index]!
    if (record === "") continue
    if (record.startsWith("# ")) {
      const header = record.slice(2)
      if (header.startsWith("branch.head ")) {
        const value = header.slice("branch.head ".length)
        if (value === "(detached)") { detached = true; branch = null } else branch = value
      } else if (header.startsWith("branch.upstream ")) {
        upstream = header.slice("branch.upstream ".length)
      } else if (header.startsWith("branch.ab ")) {
        const ab = header.slice("branch.ab ".length).split(" ")
        ahead = Math.max(0, Number.parseInt(ab[0] ?? "+0", 10) || 0)
        behind = Math.max(0, Math.abs(Number.parseInt(ab[1] ?? "-0", 10) || 0))
      }
      continue
    }
    const kind = record[0]
    if (kind === "1" || kind === "2") {
      const sp = record.split(" ")
      const xy = sp[1] ?? ".."
      // Type 2 (rename/copy) carries an extra "<X><score>" field, so the path
      // begins one field later; its original name is the NEXT NUL token.
      const pathStart = kind === "2" ? 9 : 8
      const relative = sp.slice(pathStart).join(" ")
      if (kind === "2") index++ // consume the original path token
      if (relative === "") continue
      const x = statusLetter(xy[0] ?? ".")
      const y = statusLetter(xy[1] ?? ".")
      if (x !== null) bounded(staged, { path: relative, status: x })
      if (y !== null) bounded(unstaged, { path: relative, status: y })
    } else if (kind === "u") {
      const relative = record.split(" ").slice(10).join(" ")
      if (relative !== "") {
        bounded(staged, { path: relative, status: "unmerged" })
        bounded(unstaged, { path: relative, status: "unmerged" })
      }
    } else if (kind === "?") {
      const relative = record.slice(2)
      if (relative !== "") bounded(untracked, { path: relative, status: "untracked" })
    }
    // "!" ignored entries are intentionally dropped.
  }
  return { branch, upstream, detached, ahead, behind, staged, unstaged, untracked, truncated }
}

const readStatus = (root: string): StatusSnapshot | null => {
  const out = runBinary(
    "git",
    ["-C", root, "status", "--porcelain=v2", "--branch", "-z", "--untracked-files=normal"],
    root,
    gitTimeoutMs,
  )
  if (!out.ok) return null
  const head = runBinary("git", ["-C", root, "rev-parse", "--verify", "HEAD"], root, gitTimeoutMs)
  const headRef = head.ok && head.stdout.trim() !== "" ? head.stdout.trim() : null
  const repositoryRef = opaqueRef("workspace.repository", root)
  const unstagedFingerprint = runBinary("git", ["-C", root, "diff", "--no-ext-diff", "--no-textconv", "--binary"], root, gitTimeoutMs)
  const stagedFingerprint = runBinary("git", ["-C", root, "diff", "--cached", "--no-ext-diff", "--no-textconv", "--binary"], root, gitTimeoutMs)
  const fingerprint = `${unstagedFingerprint.ok ? unstagedFingerprint.stdout : "unstaged-unavailable"}\0${stagedFingerprint.ok ? stagedFingerprint.stdout : "staged-unavailable"}`
  return {
    ...parsePorcelainV2(out.stdout),
    repositoryRef,
    headRef,
    statusRef: opaqueRef("workspace.git-status", `${repositoryRef}\0${headRef ?? "unborn"}\0${out.stdout}\0${fingerprint}`),
  }
}

export const privateGitDiffOutput = (content: string): boolean =>
  /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----|(?:github_pat|gh[pousr]_|sk-|AKIA)[A-Za-z0-9_\-]{8,}|authorization\s*:\s*bearer\s+\S+|(?:password|secret|token)\s*[:=]\s*[^\s]+/iu.test(content)

export const parseUnifiedDiffHunks = (content: string): ReadonlyArray<GitDiffHunk> => {
  const lines = content.split("\n")
  const hunks: GitDiffHunk[] = []
  let current: { header: string; oldStart: number; oldLines: number; newStart: number; newLines: number; lines: string[] } | null = null
  const flush = (): void => {
    if (current === null || hunks.length >= 500) return
    hunks.push({
      header: current.header.slice(0, 400),
      oldStart: current.oldStart,
      oldLines: current.oldLines,
      newStart: current.newStart,
      newLines: current.newLines,
      content: current.lines.join("\n").slice(0, maxReviewDiffBytes),
    })
  }
  for (const line of lines) {
    const match = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$/u.exec(line)
    if (match !== null) {
      flush()
      current = {
        header: line,
        oldStart: Number.parseInt(match[1]!, 10),
        oldLines: Number.parseInt(match[2] ?? "1", 10),
        newStart: Number.parseInt(match[3]!, 10),
        newLines: Number.parseInt(match[4] ?? "1", 10),
        lines: [line],
      }
    } else if (current !== null) {
      current.lines.push(line)
    }
  }
  flush()
  return hunks
}

const currentSnapshot = (
  root: string,
  repositoryRef: string,
  statusRef: string,
): StatusSnapshot | null => {
  const status = readStatus(root)
  return status !== null && status.repositoryRef === repositoryRef && status.statusRef === statusRef
    ? status
    : null
}

// ---------------------------------------------------------------------------
// Push failure classification
// ---------------------------------------------------------------------------

export const classifyPushError = (stderr: string): GitGithubErrorCode => {
  const text = stderr.toLowerCase()
  if (/\b(non-fast-forward|fetch first|rejected|behind its remote)\b/.test(text)) return "non_fast_forward"
  if (/(could not read username|authentication failed|permission denied|403|401|invalid username or password|access denied)/.test(text)) {
    return "auth_failed"
  }
  if (/(pre-push hook|hook declined|hook failed|prohibited)/.test(text)) return "blocked_by_hook"
  return "operation_failed"
}

// ---------------------------------------------------------------------------
// gh helpers
// ---------------------------------------------------------------------------

/** gh availability + auth gate. Never triggers a login prompt. */
const ghGate = (root: string): GitGithubErrorCode | null => {
  const version = runBinary("gh", ["--version"], root, gitTimeoutMs)
  if (!version.ok) return version.kind === "enoent" ? "gh_unavailable" : "gh_unavailable"
  const auth = runBinary("gh", ["auth", "status"], root, gitTimeoutMs)
  return auth.ok ? null : "gh_unauthenticated"
}

export const classifyGhError = (stderr: string): GitGithubErrorCode => {
  const text = stderr.toLowerCase()
  if (/(not logged in|gh auth login|authentication|no such host|401)/.test(text)) return "gh_unauthenticated"
  if (/(could not resolve to|not found|no.*found|404|does not exist)/.test(text)) return "not_found"
  return "operation_failed"
}

const boundString = (value: unknown, max: number): string =>
  typeof value === "string" ? value.slice(0, max) : ""

export const parseGhIssueList = (raw: string): ReadonlyArray<GitHubIssueRef> => {
  try {
    const value: unknown = JSON.parse(raw)
    if (!Array.isArray(value)) return []
    return value
      .slice(0, maxListItems)
      .filter((row): row is Record<string, unknown> => typeof row === "object" && row !== null)
      .map((row) => ({
        number: Number.isFinite(row["number"]) ? Math.floor(row["number"] as number) : 0,
        title: boundString(row["title"], maxTitleChars),
        url: boundString(row["url"], 400),
        state: boundString(row["state"], 40),
      }))
      .filter((row) => row.number > 0)
  } catch {
    return []
  }
}

export const parseGhPrList = (raw: string): ReadonlyArray<GitHubPrRef> => {
  try {
    const value: unknown = JSON.parse(raw)
    if (!Array.isArray(value)) return []
    return value
      .slice(0, maxListItems)
      .filter((row): row is Record<string, unknown> => typeof row === "object" && row !== null)
      .map((row) => ({
        number: Number.isFinite(row["number"]) ? Math.floor(row["number"] as number) : 0,
        title: boundString(row["title"], maxTitleChars),
        url: boundString(row["url"], 400),
        state: boundString(row["state"], 40),
        headRefName: boundString(row["headRefName"], 240),
        baseRefName: boundString(row["baseRefName"], 240),
      }))
      .filter((row) => row.number > 0)
  } catch {
    return []
  }
}

/** gh create prints the created URL on stdout; the number is its last segment. */
export const numberFromUrl = (url: string): number | null => {
  const match = /\/(\d+)\s*$/.exec(url.trim())
  return match === null ? null : Math.floor(Number.parseInt(match[1]!, 10))
}

const validNumber = (value: number): boolean => Number.isInteger(value) && value > 0 && value < 100_000_000

// ---------------------------------------------------------------------------
// Operation dispatch
// ---------------------------------------------------------------------------

const runGitGithub = (rawRoot: string | null, request: GitGithubRequest): GitGithubResult => {
  const resolved = resolveRepoRoot(rawRoot)
  if (!resolved.ok) {
    return gitGithubError(request.op, resolved.error, repoErrorMessage(resolved.error))
  }
  const root = resolved.root

  switch (request.op) {
    case "status": {
      const status = readStatus(root)
      if (status === null) return gitGithubError("status", "operation_failed", "Git status is unavailable for this workspace.")
      return { ok: true, op: "status", ...status }
    }

    case "diff": {
      const status = currentSnapshot(root, request.repositoryRef, request.statusRef)
      if (status === null) return gitGithubError("diff", "stale_status", "Repository changes moved. Refresh before reviewing this diff.")
      const relative = safeRepoRelativePath(root, request.path)
      if (relative === null) return gitGithubError("diff", "invalid_path", "That review path is outside the workspace.")
      const entries = request.source === "staged" ? status.staged : status.unstaged
      if (!entries.some(entry => entry.path === relative)) {
        return gitGithubError("diff", "stale_status", "That change is no longer present. Refresh the review.")
      }
      const baseArgs = ["-C", root, "diff", "--no-ext-diff", "--no-textconv", ...(request.source === "staged" ? ["--cached"] : [])]
      const numstat = runBinary("git", [...baseArgs, "--numstat", "--", relative], root, gitTimeoutMs)
      if (!numstat.ok) return gitGithubError("diff", "operation_failed", "That diff is unavailable for review.")
      if (/^-\t-/mu.test(numstat.stdout)) return gitGithubError("diff", "binary_diff", "Binary changes cannot enter the review or composer context surface.")
      const output = runBinary("git", [...baseArgs, "--no-color", "--unified=3", "--", relative], root, gitTimeoutMs)
      if (!output.ok) return gitGithubError("diff", "operation_failed", "That diff is unavailable for review.")
      if (Buffer.byteLength(output.stdout, "utf8") > maxReviewDiffBytes) {
        return gitGithubError("diff", "diff_too_large", "That diff exceeds the 120 KB review and context limit.")
      }
      if (privateGitDiffOutput(output.stdout)) {
        return gitGithubError("diff", "secret_diff", "Secret-shaped diff content is withheld from review and provider context.")
      }
      return {
        ok: true,
        op: "diff",
        repositoryRef: status.repositoryRef,
        statusRef: status.statusRef,
        path: relative,
        source: request.source,
        content: output.stdout,
        hunks: parseUnifiedDiffHunks(output.stdout),
        truncated: false,
      }
    }

    case "discard": {
      const status = currentSnapshot(root, request.repositoryRef, request.statusRef)
      if (status === null) return gitGithubError("discard", "stale_status", "Repository changes moved. Refresh before discarding anything.")
      const relative = safeRepoRelativePath(root, request.path)
      if (relative === null) return gitGithubError("discard", "invalid_path", "That discard path is outside the workspace.")
      const unstaged = status.unstaged.find(entry => entry.path === relative)
      const staged = status.staged.some(entry => entry.path === relative)
      if (unstaged === undefined || staged || unstaged.status === "unmerged") {
        return gitGithubError("discard", "unsafe_state", "Only an unstaged, tracked, non-conflicted change can be discarded here.")
      }
      const output = runBinary("git", ["-C", root, "restore", "--worktree", "--", relative], root, gitTimeoutMs)
      if (!output.ok) return gitGithubError("discard", "operation_failed", "That worktree change could not be discarded.")
      const next = readStatus(root)
      if (next === null) return gitGithubError("discard", "operation_failed", "The repository could not be refreshed after discard.")
      return { ok: true, op: "discard", repositoryRef: next.repositoryRef, path: relative, statusRef: next.statusRef }
    }

    case "stage":
    case "unstage": {
      const safePaths: string[] = []
      for (const candidate of request.paths.slice(0, maxStatusEntries)) {
        const relative = safeRepoRelativePath(root, candidate)
        if (relative === null) return gitGithubError(request.op, "invalid_path", "A requested path is outside the workspace.")
        safePaths.push(relative)
      }
      if (safePaths.length === 0) return gitGithubError(request.op, "invalid_request", "No paths were provided.")
      const args = request.op === "stage"
        ? ["-C", root, "add", "--", ...safePaths]
        : ["-C", root, "restore", "--staged", "--", ...safePaths]
      const out = runBinary("git", args, root, gitTimeoutMs)
      if (!out.ok) return gitGithubError(request.op, "operation_failed", `The paths could not be ${request.op === "stage" ? "staged" : "unstaged"}.`)
      return { ok: true, op: request.op, paths: safePaths }
    }

    case "commit": {
      const message = request.message.trim()
      if (message === "") return gitGithubError("commit", "empty_message", "A commit needs a non-empty message.")
      const status = readStatus(root)
      if (status !== null && status.staged.length === 0) {
        return gitGithubError("commit", "nothing_staged", "Nothing is staged to commit.")
      }
      const out = runBinary("git", ["-C", root, "commit", "-m", message.slice(0, maxMessageChars)], root, gitTimeoutMs)
      if (!out.ok) {
        const text = out.stderr.toLowerCase()
        if (/nothing to commit|no changes added/.test(text)) return gitGithubError("commit", "nothing_staged", "Nothing is staged to commit.")
        if (/pre-commit hook|hook declined|hook failed/.test(text)) return gitGithubError("commit", "blocked_by_hook", "A pre-commit hook blocked this commit.")
        return gitGithubError("commit", "operation_failed", "The commit could not be created.")
      }
      const sha = runBinary("git", ["-C", root, "rev-parse", "HEAD"], root, gitTimeoutMs)
      const shortSha = runBinary("git", ["-C", root, "rev-parse", "--short", "HEAD"], root, gitTimeoutMs)
      const full = sha.ok ? sha.stdout.trim() : ""
      return {
        ok: true,
        op: "commit",
        sha: full,
        shortSha: shortSha.ok ? shortSha.stdout.trim() : full.slice(0, 9),
        summary: message.split("\n")[0]!.slice(0, maxTitleChars),
      }
    }

    case "push": {
      const branch = runBinary("git", ["-C", root, "symbolic-ref", "--quiet", "--short", "HEAD"], root, gitTimeoutMs)
      if (!branch.ok) return gitGithubError("push", "no_upstream", "A detached HEAD has no branch to push.")
      const branchName = branch.stdout.trim()
      const upstream = runBinary("git", ["-C", root, "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"], root, gitTimeoutMs)
      if (!upstream.ok) return gitGithubError("push", "no_upstream", "This branch has no upstream to push to.")
      const upstreamRef = upstream.stdout.trim()
      const remote = upstreamRef.includes("/") ? upstreamRef.slice(0, upstreamRef.indexOf("/")) : "origin"

      const attempt = (): ExecResult => runBinary("git", ["-C", root, "push"], root, ghTimeoutMs)
      let out = attempt()
      if (!out.ok) {
        const failure = classifyPushError(out.stderr)
        // fetch → rebase → push retry, once (audit §3.3 structural habit).
        if (failure === "non_fast_forward") {
          const fetched = runBinary("git", ["-C", root, "fetch", remote], root, ghTimeoutMs)
          if (fetched.ok) {
            const rebased = runBinary("git", ["-C", root, "rebase", upstreamRef], root, ghTimeoutMs)
            if (!rebased.ok) {
              runBinary("git", ["-C", root, "rebase", "--abort"], root, gitTimeoutMs)
              return gitGithubError("push", "non_fast_forward", "Remote has diverged; automatic rebase hit a conflict. Resolve locally, then push.")
            }
            out = attempt()
          }
        }
        if (!out.ok) {
          const finalFailure = classifyPushError(out.stderr)
          return gitGithubError("push", finalFailure, pushErrorMessage(finalFailure))
        }
      }
      const sha = runBinary("git", ["-C", root, "rev-parse", "HEAD"], root, gitTimeoutMs)
      return {
        ok: true,
        op: "push",
        ref: branchName,
        remote,
        sha: sha.ok ? sha.stdout.trim() : "",
      }
    }

    case "branchList": {
      const out = runBinary(
        "git",
        ["-C", root, "for-each-ref", "--format=%(refname:short)%00%(HEAD)%00%(upstream:short)", "refs/heads/"],
        root,
        gitTimeoutMs,
      )
      if (!out.ok) return gitGithubError("branchList", "operation_failed", "The branch list is unavailable.")
      const branches: GitBranch[] = []
      let current: string | null = null
      let truncated = false
      for (const line of out.stdout.split("\n")) {
        if (line.trim() === "") continue
        if (branches.length >= maxBranches) { truncated = true; break }
        const [name, head, upstream] = line.split("\0")
        if (name === undefined || name === "") continue
        const isCurrent = head === "*"
        if (isCurrent) current = name
        branches.push({ name, current: isCurrent, upstream: upstream === undefined || upstream === "" ? null : upstream })
      }
      return { ok: true, op: "branchList", current, branches, truncated }
    }

    case "branchCreate": {
      if (!validBranchName(request.name)) return gitGithubError("branchCreate", "invalid_branch_name", "That branch name is not valid.")
      const args = request.checkout
        ? ["-C", root, "switch", "-c", request.name]
        : ["-C", root, "branch", request.name]
      const out = runBinary("git", args, root, gitTimeoutMs)
      if (!out.ok) {
        const text = out.stderr.toLowerCase()
        if (/already exists/.test(text)) return gitGithubError("branchCreate", "branch_exists", "A branch with that name already exists.")
        return gitGithubError("branchCreate", "operation_failed", "The branch could not be created.")
      }
      return { ok: true, op: "branchCreate", name: request.name, checkedOut: request.checkout }
    }

    case "checkout": {
      if (!validBranchName(request.name)) return gitGithubError("checkout", "invalid_branch_name", "That branch name is not valid.")
      const status = readStatus(root)
      // Match the workspace's existing safety: refuse a checkout with tracked
      // changes (staged or unstaged). Untracked files do not block it.
      if (status !== null && (status.staged.length > 0 || status.unstaged.length > 0)) {
        return gitGithubError("checkout", "dirty_tree", "Commit or stash your changes before switching branches.")
      }
      const out = runBinary("git", ["-C", root, "switch", "--", request.name], root, gitTimeoutMs)
      if (!out.ok) {
        const text = out.stderr.toLowerCase()
        if (/did not match|invalid reference|unknown revision|no such/.test(text)) {
          return gitGithubError("checkout", "not_found", "No branch with that name exists.")
        }
        return gitGithubError("checkout", "operation_failed", "The branch could not be checked out.")
      }
      return { ok: true, op: "checkout", name: request.name }
    }

    case "issueList": {
      const gate = ghGate(root)
      if (gate !== null) return gitGithubError("issueList", gate, ghErrorMessage(gate))
      const limit = Math.min(maxListItems, Math.max(1, Math.floor(request.limit ?? 20)))
      const out = runBinary("gh", ["issue", "list", "--json", "number,title,url,state", "--limit", String(limit)], root, ghTimeoutMs)
      if (!out.ok) return gitGithubError("issueList", classifyGhError(out.stderr), "The issue list could not be fetched.")
      return { ok: true, op: "issueList", issues: parseGhIssueList(out.stdout) }
    }

    case "issueView": {
      if (!validNumber(request.number)) return gitGithubError("issueView", "invalid_request", "That issue number is not valid.")
      const gate = ghGate(root)
      if (gate !== null) return gitGithubError("issueView", gate, ghErrorMessage(gate))
      const out = runBinary("gh", ["issue", "view", String(request.number), "--json", "number,title,url,state,body"], root, ghTimeoutMs)
      if (!out.ok) return gitGithubError("issueView", classifyGhError(out.stderr), "That issue could not be fetched.")
      try {
        const row = JSON.parse(out.stdout) as Record<string, unknown>
        return {
          ok: true,
          op: "issueView",
          issue: {
            number: Number.isFinite(row["number"]) ? Math.floor(row["number"] as number) : request.number,
            title: boundString(row["title"], maxTitleChars),
            url: boundString(row["url"], 400),
            state: boundString(row["state"], 40),
            body: boundString(row["body"], maxBodyChars),
          },
        }
      } catch {
        return gitGithubError("issueView", "operation_failed", "That issue response could not be read.")
      }
    }

    case "issueCreate": {
      const title = request.title.trim()
      if (title === "") return gitGithubError("issueCreate", "invalid_request", "An issue needs a title.")
      const gate = ghGate(root)
      if (gate !== null) return gitGithubError("issueCreate", gate, ghErrorMessage(gate))
      const out = runBinary(
        "gh",
        ["issue", "create", "--title", title.slice(0, maxTitleChars), "--body", request.body.slice(0, maxBodyChars)],
        root,
        ghTimeoutMs,
      )
      if (!out.ok) return gitGithubError("issueCreate", classifyGhError(out.stderr), "The issue could not be created.")
      const url = out.stdout.trim().split("\n").filter((line) => line.startsWith("http")).pop() ?? out.stdout.trim()
      const number = numberFromUrl(url)
      if (number === null) return gitGithubError("issueCreate", "operation_failed", "The issue was created but its number could not be read.")
      return { ok: true, op: "issueCreate", number, url: url.slice(0, 400) }
    }

    case "prList": {
      const gate = ghGate(root)
      if (gate !== null) return gitGithubError("prList", gate, ghErrorMessage(gate))
      const limit = Math.min(maxListItems, Math.max(1, Math.floor(request.limit ?? 20)))
      const out = runBinary(
        "gh",
        ["pr", "list", "--json", "number,title,url,state,headRefName,baseRefName", "--limit", String(limit)],
        root,
        ghTimeoutMs,
      )
      if (!out.ok) return gitGithubError("prList", classifyGhError(out.stderr), "The pull-request list could not be fetched.")
      return { ok: true, op: "prList", prs: parseGhPrList(out.stdout) }
    }

    case "prView": {
      if (!validNumber(request.number)) return gitGithubError("prView", "invalid_request", "That pull-request number is not valid.")
      const gate = ghGate(root)
      if (gate !== null) return gitGithubError("prView", gate, ghErrorMessage(gate))
      const out = runBinary(
        "gh",
        ["pr", "view", String(request.number), "--json", "number,title,url,state,headRefName,baseRefName,body"],
        root,
        ghTimeoutMs,
      )
      if (!out.ok) return gitGithubError("prView", classifyGhError(out.stderr), "That pull request could not be fetched.")
      try {
        const row = JSON.parse(out.stdout) as Record<string, unknown>
        return {
          ok: true,
          op: "prView",
          pr: {
            number: Number.isFinite(row["number"]) ? Math.floor(row["number"] as number) : request.number,
            title: boundString(row["title"], maxTitleChars),
            url: boundString(row["url"], 400),
            state: boundString(row["state"], 40),
            headRefName: boundString(row["headRefName"], 240),
            baseRefName: boundString(row["baseRefName"], 240),
            body: boundString(row["body"], maxBodyChars),
          },
        }
      } catch {
        return gitGithubError("prView", "operation_failed", "That pull-request response could not be read.")
      }
    }

    case "prCreate": {
      const title = request.title.trim()
      if (title === "") return gitGithubError("prCreate", "invalid_request", "A pull request needs a title.")
      if (request.base !== undefined && !validBranchName(request.base)) return gitGithubError("prCreate", "invalid_branch_name", "That base branch is not valid.")
      if (request.head !== undefined && !validBranchName(request.head)) return gitGithubError("prCreate", "invalid_branch_name", "That head branch is not valid.")
      const gate = ghGate(root)
      if (gate !== null) return gitGithubError("prCreate", gate, ghErrorMessage(gate))
      const args = [
        "pr",
        "create",
        "--title",
        title.slice(0, maxTitleChars),
        "--body",
        request.body.slice(0, maxBodyChars),
        ...(request.base !== undefined ? ["--base", request.base] : []),
        ...(request.head !== undefined ? ["--head", request.head] : []),
      ]
      const out = runBinary("gh", args, root, ghTimeoutMs)
      if (!out.ok) return gitGithubError("prCreate", classifyGhError(out.stderr), "The pull request could not be created.")
      const url = out.stdout.trim().split("\n").filter((line) => line.startsWith("http")).pop() ?? out.stdout.trim()
      const number = numberFromUrl(url)
      if (number === null) return gitGithubError("prCreate", "operation_failed", "The pull request was created but its number could not be read.")
      return { ok: true, op: "prCreate", number, url: url.slice(0, 400) }
    }
  }
}

// ---------------------------------------------------------------------------
// Public-safe messages
// ---------------------------------------------------------------------------

const repoErrorMessage = (error: GitGithubErrorCode): string => {
  switch (error) {
    case "no_workspace": return "Choose a workspace folder first."
    case "not_a_repo": return "The selected workspace is not a Git repository."
    case "git_unavailable": return "Git is not available on this machine."
    default: return "This workspace is unavailable for Git operations."
  }
}

const pushErrorMessage = (error: GitGithubErrorCode): string => {
  switch (error) {
    case "non_fast_forward": return "The remote has changes this branch does not. Fetch and rebase, then push."
    case "auth_failed": return "Push was rejected: Git could not authenticate to the remote."
    case "blocked_by_hook": return "A pre-push hook blocked this push."
    default: return "The push could not be completed."
  }
}

const ghErrorMessage = (error: GitGithubErrorCode): string => {
  switch (error) {
    case "gh_unavailable": return "The GitHub CLI (gh) is not installed."
    case "gh_unauthenticated": return "The GitHub CLI is not authenticated. Run `gh auth login` in a terminal."
    case "not_found": return "That item was not found."
    default: return "The GitHub operation could not be completed."
  }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

export type DesktopGitGithubService = Readonly<{
  run: (value: unknown) => GitGithubResult
}>

/**
 * One typed Git/GitHub service over an injected root resolver. The resolver is
 * re-read per call so the service tracks the active workspace without holding a
 * stale root; a null root yields the typed `no_workspace` error.
 */
export const openGitGithubService = (resolveRoot: () => string | null): DesktopGitGithubService => ({
  run: (value) => {
    const request = decodeGitGithubRequest(value)
    if (request === null) {
      return gitGithubError("status", "invalid_request", "The Git request could not be decoded.")
    }
    return runGitGithub(resolveRoot(), request)
  },
})
