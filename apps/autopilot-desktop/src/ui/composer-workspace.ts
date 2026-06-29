// #5471 (EPIC #5461): composer repo / worktree picker.
//
// "Give the agent a repo + a task." The composer spawn form already carried an
// objective + an optional worktree-path text field that mapped to
// session.spawn's `worktreePath`. The control protocol ALSO accepts a managed
// worktree (`repoRef = { provider:"github", visibility:"public", fullName,
// branch, commitSha }`) that Pylon's workspace-materializer checks out — but the
// desktop never exposed it.
//
// This sibling module owns the PURE logic for a compact two-mode picker so the
// central composer files (view/update/model/message/commands) only gain thin,
// append-style hooks:
//   - mode "worktree" — point at an existing local worktree path (unchanged
//     behavior; threaded through `worktreePath`).
//   - mode "managed"  — request a Pylon-managed worktree for a GitHub repo
//     (`owner/name`) + a base ref. The 40-char commit SHA is resolved node-side
//     in the Bun layer via `git ls-remote` (the same shape the Pylon CLI's
//     `--managed-worktree` builds), then sent as `repoRef`. No new control verb.
//
// Everything here is a pure function over plain values so it is covered by the
// proper-runner reducer tests without a DOM or a live node.

// The repo/worktree selection mode the composer spawn form is in.
export type ComposerWorkspaceMode = "worktree" | "managed"

// A validated managed-worktree request the Bun layer can resolve to a repoRef.
// The branch is the base ref with any `origin/` prefix stripped (matching the
// Pylon CLI's `managedWorktreeRepoRef`), so a checkout off `origin/main`
// records branch `main`.
export type ManagedWorktreeRequest = {
  readonly fullName: string
  readonly baseRef: string
  readonly branch: string
}

export type ManagedWorktreeParse =
  | { readonly ok: true; readonly request: ManagedWorktreeRequest }
  | { readonly ok: false; readonly error: string }

// GitHub `owner/name`. Mirrors `gitHubFullNamePattern` in apps/pylon and
// `repositoryRefFrom` in control-sessions.ts so a request the desktop accepts is
// one the node will accept.
const GITHUB_FULL_NAME = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/

// A git ref the node will accept (`repositoryRefFrom` rejects `..`, a leading
// `-`, and any char outside this class). We validate the same way up front so a
// bad ref fails in the picker rather than after a round-trip.
const SAFE_REF = /^[A-Za-z0-9_./-]+$/

export const DEFAULT_MANAGED_BASE_REF = "origin/main"

// Normalize a GitHub repo entry to `owner/name`. Accepts a bare `owner/name`,
// an `https://github.com/owner/name(.git)` URL, or a `git@github.com:owner/name`
// SSH remote — the forms a user is likely to paste — and rejects anything else.
export const normalizeGitHubFullName = (raw: string): string | null => {
  const trimmed = raw.trim().replace(/\/+$/, "")
  if (trimmed === "") return null
  const fromPath = (path: string): string | null => {
    const fullName = path.replace(/^\/+/, "").replace(/\.git$/, "")
    return GITHUB_FULL_NAME.test(fullName) ? fullName : null
  }
  if (GITHUB_FULL_NAME.test(trimmed)) return trimmed
  try {
    const url = new URL(trimmed)
    if (url.hostname !== "github.com") return null
    return fromPath(url.pathname)
  } catch {
    const ssh = /^git@github\.com:([^#?]+)$/.exec(trimmed)
    if (ssh !== null) return fromPath(ssh[1] ?? "")
    return null
  }
}

// Validate a managed-worktree request (repo + base ref) before it leaves the
// picker. Returns a typed request the resolver command can consume, or a
// single human-readable error for the composer status line.
export const parseManagedWorktreeRequest = (input: {
  repo: string
  baseRef: string
}): ManagedWorktreeParse => {
  const fullName = normalizeGitHubFullName(input.repo)
  if (fullName === null) {
    return {
      ok: false,
      error: "repo must be a GitHub owner/name (or github.com URL)",
    }
  }
  const baseRef =
    input.baseRef.trim() === "" ? DEFAULT_MANAGED_BASE_REF : input.baseRef.trim()
  if (!SAFE_REF.test(baseRef) || baseRef.includes("..") || baseRef.startsWith("-")) {
    return { ok: false, error: "base ref is invalid" }
  }
  return {
    ok: true,
    request: { fullName, baseRef, branch: baseRef.replace(/^origin\//, "") },
  }
}

// Short provenance label for a managed request, shown in the picker and (after
// spawn) carried into the composer status so the chosen repo/worktree is
// visible. Example: "OpenAgentsInc/openagents @ origin/main".
export const managedWorktreeLabel = (request: ManagedWorktreeRequest): string =>
  `${request.fullName} @ ${request.baseRef}`

// The worktree-mode provenance label (the path itself, or a hint when empty —
// an empty path means the node's built-in agent worktree).
export const worktreePathLabel = (path: string): string => {
  const trimmed = path.trim()
  return trimmed === "" ? "node default worktree" : trimmed
}
