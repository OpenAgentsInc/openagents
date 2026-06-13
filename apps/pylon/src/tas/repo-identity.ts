export type RepoIdentity = {
  readonly repoFullName: string
  readonly commitSha: string
  readonly branch?: string | null
  readonly detached: boolean
  readonly worktreeRef: string
  readonly dirty: boolean
}

export type RepoIdentityInput = RepoIdentity & Record<string, unknown>

const COMMIT_SHA_PATTERN = /^[a-f0-9]{40}$/i

function assertNonEmptyRef(value: string, name: string): void {
  if (!value.trim()) {
    throw new Error(`repo identity ${name} is required`)
  }
}

function assertRepoIdentity(identity: RepoIdentity): RepoIdentity {
  assertNonEmptyRef(identity.repoFullName, "repoFullName")
  assertNonEmptyRef(identity.worktreeRef, "worktreeRef")

  if (!COMMIT_SHA_PATTERN.test(identity.commitSha)) {
    throw new Error("repo identity commitSha must be a 40-hex commit hash")
  }

  if (identity.branch !== null && identity.branch !== undefined && !identity.branch.trim()) {
    throw new Error("repo identity branch must be null or a non-empty branch ref")
  }

  if (identity.detached !== (identity.branch === null)) {
    throw new Error("repo identity detached must match whether branch is null")
  }

  return identity
}

export function buildRepoIdentity(input: RepoIdentityInput): RepoIdentity {
  const branch = input.branch ?? null
  return assertRepoIdentity({
    repoFullName: input.repoFullName,
    commitSha: input.commitSha.toLowerCase(),
    branch,
    detached: input.detached,
    worktreeRef: input.worktreeRef,
    dirty: input.dirty,
  })
}

const scopePart = (value: string): string => encodeURIComponent(value)

export function scopeKey(identity: RepoIdentity): string {
  const snapshot = assertRepoIdentity(identity)
  const branchRef = snapshot.branch === null ? "detached" : `branch:${snapshot.branch}`
  const dirtyRef = snapshot.dirty ? "dirty" : "clean"

  return [
    "repo-scope",
    scopePart(snapshot.repoFullName),
    scopePart(snapshot.worktreeRef),
    scopePart(snapshot.commitSha.toLowerCase()),
    scopePart(branchRef),
    dirtyRef,
  ].join(":")
}
