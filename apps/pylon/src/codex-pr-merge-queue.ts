export type SupervisorPrMergeCommandResult = {
  exitCode: number
  stdout: string
  stderr: string
  timedOut: boolean
}

export type SupervisorPrMergeCommandRunner = (input: {
  args: string[]
  cwd: string
  timeoutMs?: number
}) => Promise<SupervisorPrMergeCommandResult>

export type SupervisorPrFastForwardMergeInput = {
  repository: {
    fullName: string
    baseBranch: string
  }
  prNumber: number
  workingDirectory: string
  verifyCommand?: string[]
  runner?: SupervisorPrMergeCommandRunner
}

export type SupervisorPrFastForwardMergeResult =
  | {
      state: "merged"
      prNumber: number
      prUrl: string | null
      headSha: string
      verifyExitCode: number | null
    }
  | { state: "skipped"; reasonRef: string; prNumber: number; prUrl?: string | null }
  | { state: "failed"; reasonRef: string; prNumber: number; prUrl?: string | null }

type PullRequestView = {
  number?: unknown
  state?: unknown
  isDraft?: unknown
  url?: unknown
  baseRefName?: unknown
  headRefOid?: unknown
}

const GIT_TIMEOUT_MS = 60 * 1000
const GH_TIMEOUT_MS = 2 * 60 * 1000
const VERIFY_TIMEOUT_MS = 10 * 60 * 1000

const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitRefNamePattern = /^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i

async function defaultRunner(input: {
  args: string[]
  cwd: string
  timeoutMs?: number
}): Promise<SupervisorPrMergeCommandResult> {
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

function githubRemoteUrl(fullName: string): string {
  return `https://github.com/${fullName}.git`
}

function safeRefSegment(value: string): string | null {
  if (!gitRefNamePattern.test(value) || value.includes("..") || value.endsWith(".lock")) {
    return null
  }
  return value
}

function parsePullRequestView(stdout: string): PullRequestView | null {
  try {
    const parsed = JSON.parse(stdout) as PullRequestView
    return parsed !== null && typeof parsed === "object" ? parsed : null
  } catch {
    return null
  }
}

async function runOk(
  runner: SupervisorPrMergeCommandRunner,
  input: { args: string[]; cwd: string; timeoutMs?: number },
): Promise<boolean> {
  const result = await runner(input)
  return result.exitCode === 0 && !result.timedOut
}

/**
 * Supervisor-side virtual merge queue primitive for Pylon/Codex PRs.
 *
 * The API never pushes directly to the protected base branch. It first builds
 * the projected post-merge tree locally (`base` + PR head via `--ff-only`),
 * optionally runs the supplied verifier there, then asks GitHub to merge that
 * exact PR head with `--match-head-commit`. This gives the supervisor a
 * deterministic fast-forward eligibility gate without turning GitHub's native
 * merge queue into the fleet's hot coordination path.
 */
export async function fastForwardMergePullRequest(
  input: SupervisorPrFastForwardMergeInput,
): Promise<SupervisorPrFastForwardMergeResult> {
  const runner = input.runner ?? defaultRunner
  const prNumber = input.prNumber
  const fullName = input.repository.fullName
  const baseBranch = safeRefSegment(input.repository.baseBranch)

  if (!Number.isInteger(prNumber) || prNumber <= 0) {
    return { state: "skipped", reasonRef: "merge_queue.skipped_invalid_pr_number", prNumber }
  }
  if (!githubFullNamePattern.test(fullName) || baseBranch === null) {
    return { state: "skipped", reasonRef: "merge_queue.skipped_unsupported_repository", prNumber }
  }

  const prView = await runner({
    args: [
      "gh",
      "pr",
      "view",
      String(prNumber),
      "--repo",
      fullName,
      "--json",
      "number,state,isDraft,url,baseRefName,headRefOid",
    ],
    cwd: input.workingDirectory,
    timeoutMs: GH_TIMEOUT_MS,
  })
  if (prView.exitCode !== 0 || prView.timedOut) {
    return { state: "failed", reasonRef: "merge_queue.pr_view_failed", prNumber }
  }

  const pr = parsePullRequestView(prView.stdout)
  const prUrl = typeof pr?.url === "string" ? pr.url : null
  const headSha = typeof pr?.headRefOid === "string" ? pr.headRefOid : ""
  if (pr === null || pr.number !== prNumber || !gitCommitShaPattern.test(headSha)) {
    return { state: "failed", reasonRef: "merge_queue.pr_view_invalid", prNumber, prUrl }
  }
  if (pr.state !== "OPEN") {
    return { state: "skipped", reasonRef: "merge_queue.skipped_pr_not_open", prNumber, prUrl }
  }
  if (pr.isDraft === true) {
    return { state: "skipped", reasonRef: "merge_queue.skipped_pr_draft", prNumber, prUrl }
  }
  if (pr.baseRefName !== baseBranch) {
    return { state: "skipped", reasonRef: "merge_queue.skipped_base_mismatch", prNumber, prUrl }
  }

  const remote = githubRemoteUrl(fullName)
  const localBaseRef = `refs/remotes/pylon-merge/${baseBranch}`
  const localPrRef = `refs/remotes/pylon-merge/pr-${prNumber}`
  const virtualBranch = `pylon/virtual-merge/pr-${prNumber}`

  const fetched = await runOk(runner, {
    args: [
      "git",
      "fetch",
      "--no-tags",
      remote,
      `refs/heads/${baseBranch}:${localBaseRef}`,
      `refs/pull/${prNumber}/head:${localPrRef}`,
    ],
    cwd: input.workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (!fetched) {
    return { state: "failed", reasonRef: "merge_queue.fetch_failed", prNumber, prUrl }
  }

  const checkedOut = await runOk(runner, {
    args: ["git", "checkout", "-B", virtualBranch, localBaseRef],
    cwd: input.workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (!checkedOut) {
    return { state: "failed", reasonRef: "merge_queue.virtual_checkout_failed", prNumber, prUrl }
  }

  const mergedLocally = await runOk(runner, {
    args: ["git", "merge", "--ff-only", localPrRef],
    cwd: input.workingDirectory,
    timeoutMs: GIT_TIMEOUT_MS,
  })
  if (!mergedLocally) {
    return { state: "skipped", reasonRef: "merge_queue.skipped_not_fast_forward", prNumber, prUrl }
  }

  let verifyExitCode: number | null = null
  if (input.verifyCommand !== undefined && input.verifyCommand.length > 0) {
    const verify = await runner({
      args: input.verifyCommand,
      cwd: input.workingDirectory,
      timeoutMs: VERIFY_TIMEOUT_MS,
    })
    verifyExitCode = verify.exitCode
    if (verify.exitCode !== 0 || verify.timedOut) {
      return { state: "skipped", reasonRef: "merge_queue.skipped_verification_failed", prNumber, prUrl }
    }
  }

  const merged = await runOk(runner, {
    args: [
      "gh",
      "pr",
      "merge",
      String(prNumber),
      "--repo",
      fullName,
      "--rebase",
      "--delete-branch",
      "--match-head-commit",
      headSha,
    ],
    cwd: input.workingDirectory,
    timeoutMs: GH_TIMEOUT_MS,
  })
  if (!merged) {
    return { state: "failed", reasonRef: "merge_queue.github_merge_failed", prNumber, prUrl }
  }

  return { state: "merged", prNumber, prUrl, headSha, verifyExitCode }
}
