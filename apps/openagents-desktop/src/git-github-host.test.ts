/**
 * Typed Git/GitHub host tests (EP250 E2–E5, #8712).
 *
 * Every git-facing operation runs against a REAL temporary git repository
 * (and, for push, a REAL local bare remote) — no mocks on the git path. The
 * gh path is tested two ways: the parse/classify boundary is unit-tested
 * directly, and one guarded real `gh --version`/`gh auth status` test skips
 * with a printed reason when gh is unavailable, so CI stays honest.
 */
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"
import { spawnSync } from "node:child_process"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  classifyGhError,
  classifyPushError,
  numberFromUrl,
  openGitGithubService,
  parseGhIssueList,
  parseGhPrList,
  parsePorcelainV2,
  validBranchName,
} from "./git-github-host.ts"
import type { GitGithubResult } from "./git-github-contract.ts"
import { isolatedGitEnvironment } from "../tests/git-fixture.ts"

const gitAvailable = spawnSync("git", ["--version"], { encoding: "utf8" }).status === 0

const git = (repo: string, ...args: string[]): { ok: boolean; stdout: string; stderr: string } => {
  const proc = spawnSync("git", ["-C", repo, ...args], {
    encoding: "utf8",
    env: {
      ...isolatedGitEnvironment(),
      GIT_AUTHOR_NAME: "Test",
      GIT_AUTHOR_EMAIL: "test@example.com",
      GIT_COMMITTER_NAME: "Test",
      GIT_COMMITTER_EMAIL: "test@example.com",
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_SYSTEM: "/dev/null",
    },
  })
  return { ok: proc.status === 0, stdout: (proc.stdout ?? "").toString(), stderr: (proc.stderr ?? "").toString() }
}

const tmpRoots: string[] = []
const makeRepo = (bare = false): string => {
  const dir = mkdtempSync(path.join(tmpdir(), "gitgh-"))
  tmpRoots.push(dir)
  if (bare) {
    git(dir, "init", "--bare", "--initial-branch=main")
  } else {
    git(dir, "init", "--initial-branch=main")
    git(dir, "config", "user.email", "test@example.com")
    git(dir, "config", "user.name", "Test")
    git(dir, "config", "commit.gpgsign", "false")
  }
  return dir
}
const write = (repo: string, rel: string, content: string): void => writeFileSync(path.join(repo, rel), content)
const service = (repo: string) => {
  const host = openGitGithubService(() => repo)
  return {
    run: (request: Record<string, unknown>) => {
      if (["stage", "unstage", "commit", "push", "branchCreate", "checkout"].includes(String(request["op"])) &&
          (request["repositoryRef"] === undefined || request["statusRef"] === undefined)) {
        const status = host.run({ op: "status" })
        if (status.ok && status.op === "status") {
          return host.run({ ...request, repositoryRef: status.repositoryRef, statusRef: status.statusRef })
        }
      }
      return host.run(request)
    },
  }
}

afterAll(() => {
  for (const dir of tmpRoots) {
    try { rmSync(dir, { recursive: true, force: true }) } catch { /* best effort */ }
  }
})

// ---------------------------------------------------------------------------
// Pure parsing / validation
// ---------------------------------------------------------------------------

describe("porcelain v2 parsing", () => {
  test("branch header, ahead/behind, and staged vs unstaged split", () => {
    const raw = [
      "# branch.head main",
      "# branch.upstream origin/main",
      "# branch.ab +2 -1",
      "1 M. N... 100644 100644 100644 aaa bbb staged.txt",
      "1 .M N... 100644 100644 100644 ccc ddd worktree.txt",
      "1 MM N... 100644 100644 100644 eee fff both.txt",
      "? untracked.txt",
      "! ignored.txt",
      "",
    ].join("\0")
    const parsed = parsePorcelainV2(raw)
    expect(parsed.branch).toBe("main")
    expect(parsed.upstream).toBe("origin/main")
    expect(parsed.ahead).toBe(2)
    expect(parsed.behind).toBe(1)
    expect(parsed.staged.map((entry) => entry.path).sort()).toEqual(["both.txt", "staged.txt"])
    expect(parsed.unstaged.map((entry) => entry.path).sort()).toEqual(["both.txt", "worktree.txt"])
    expect(parsed.untracked.map((entry) => entry.path)).toEqual(["untracked.txt"])
  })

  test("detached HEAD sets detached with no branch", () => {
    const parsed = parsePorcelainV2("# branch.head (detached)\0")
    expect(parsed.detached).toBe(true)
    expect(parsed.branch).toBeNull()
  })
})

describe("input validation and classification", () => {
  test("branch names", () => {
    expect(validBranchName("feature/x")).toBe(true)
    expect(validBranchName("main")).toBe(true)
    expect(validBranchName("-bad")).toBe(false)
    expect(validBranchName("a..b")).toBe(false)
    expect(validBranchName("trailing/")).toBe(false)
    expect(validBranchName("has space")).toBe(false)
  })

  test("push error classes", () => {
    expect(classifyPushError("! [rejected] main -> main (non-fast-forward)")).toBe("non_fast_forward")
    expect(classifyPushError("fatal: Authentication failed for 'https://...'")).toBe("auth_failed")
    expect(classifyPushError("error: failed to push some refs (pre-push hook declined)")).toBe("blocked_by_hook")
    expect(classifyPushError("some other error")).toBe("operation_failed")
  })

  test("gh error classes", () => {
    expect(classifyGhError("You are not logged into any GitHub hosts. Run gh auth login")).toBe("gh_unauthenticated")
    expect(classifyGhError("Could not resolve to an Issue with the number of 9999999")).toBe("not_found")
  })

  test("gh list parsing bounds and drops malformed rows", () => {
    const issues = parseGhIssueList(JSON.stringify([
      { number: 1, title: "a", url: "https://x/1", state: "OPEN" },
      { number: 0, title: "bad" },
      { nope: true },
    ]))
    expect(issues).toEqual([{ number: 1, title: "a", url: "https://x/1", state: "OPEN" }])
    const prs = parseGhPrList(JSON.stringify([
      { number: 5, title: "pr", url: "https://x/5", state: "OPEN", headRefName: "feat", baseRefName: "main" },
    ]))
    expect(prs[0]?.headRefName).toBe("feat")
    expect(parseGhIssueList("not json")).toEqual([])
  })

  test("numberFromUrl extracts the trailing id", () => {
    expect(numberFromUrl("https://github.com/o/r/issues/8712")).toBe(8712)
    expect(numberFromUrl("https://github.com/o/r/pull/42\n")).toBe(42)
    expect(numberFromUrl("no number here")).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Real repo operations
// ---------------------------------------------------------------------------

const guardGit = (fn: () => void): (() => void) => () => {
  if (!gitAvailable) {
    console.log("[git-github-host.test] SKIP: git is not available")
    return
  }
  fn()
}

describe("real repo: status, stage, commit", () => {
  test("no_workspace when the root is null", () => {
    const result = openGitGithubService(() => null).run({ op: "status" })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("no_workspace")
  })

  test("not_a_repo for a plain directory", guardGit(() => {
    const dir = mkdtempSync(path.join(tmpdir(), "gitgh-plain-"))
    tmpRoots.push(dir)
    const result = service(dir).run({ op: "status" })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("not_a_repo")
  }))

  test("status → stage → status → commit returns a real SHA receipt", guardGit(() => {
    const repo = makeRepo()
    write(repo, "a.txt", "hello\n")

    const before = service(repo).run({ op: "status" })
    expect(before.ok).toBe(true)
    if (before.ok && before.op === "status") {
      expect(before.untracked.map((entry) => entry.path)).toContain("a.txt")
      expect(before.staged).toHaveLength(0)
    }

    const staged = service(repo).run({ op: "stage", paths: ["a.txt"] })
    expect(staged.ok).toBe(true)

    const afterStage = service(repo).run({ op: "status" })
    if (afterStage.ok && afterStage.op === "status") {
      expect(afterStage.staged.map((entry) => entry.path)).toContain("a.txt")
    }

    const commit = service(repo).run({ op: "commit", message: "feat: add a" })
    expect(commit.ok).toBe(true)
    if (commit.ok && commit.op === "commit") {
      expect(commit.sha).toMatch(/^[0-9a-f]{40}$/)
      expect(commit.summary).toBe("feat: add a")
      // The SHA is the real HEAD.
      expect(git(repo, "rev-parse", "HEAD").stdout.trim()).toBe(commit.sha)
    }
  }))

  test("empty message and nothing-staged are typed refusals", guardGit(() => {
    const repo = makeRepo()
    write(repo, "seed.txt", "x\n")
    service(repo).run({ op: "stage", paths: ["seed.txt"] })
    service(repo).run({ op: "commit", message: "seed" })

    const empty = service(repo).run({ op: "commit", message: "   " })
    expect(empty.ok).toBe(false)
    if (!empty.ok) expect(empty.error).toBe("empty_message")

    const nothing = service(repo).run({ op: "commit", message: "nothing here" })
    expect(nothing.ok).toBe(false)
    if (!nothing.ok) expect(nothing.error).toBe("nothing_staged")
  }))

  test("unstage moves a staged path back out of the index", guardGit(() => {
    const repo = makeRepo()
    write(repo, "seed.txt", "x\n")
    service(repo).run({ op: "stage", paths: ["seed.txt"] })
    service(repo).run({ op: "commit", message: "seed" })
    write(repo, "seed.txt", "y\n")
    service(repo).run({ op: "stage", paths: ["seed.txt"] })
    const unstaged = service(repo).run({ op: "unstage", paths: ["seed.txt"] })
    expect(unstaged.ok).toBe(true)
    const status = service(repo).run({ op: "status" })
    if (status.ok && status.op === "status") {
      expect(status.staged).toHaveLength(0)
      expect(status.unstaged.map((entry) => entry.path)).toContain("seed.txt")
    }
  }))

  test("a path outside the workspace is refused", guardGit(() => {
    const repo = makeRepo()
    const result = service(repo).run({ op: "stage", paths: ["../escape.txt"] })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("invalid_path")
  }))
})

describe("real repo: branch list, create, checkout", () => {
  const seed = (repo: string): void => {
    write(repo, "seed.txt", "x\n")
    service(repo).run({ op: "stage", paths: ["seed.txt"] })
    service(repo).run({ op: "commit", message: "seed" })
  }

  test("branchList shows the current branch; branchCreate switches", guardGit(() => {
    const repo = makeRepo()
    seed(repo)
    const created = service(repo).run({ op: "branchCreate", name: "feature/x", checkout: true })
    expect(created.ok).toBe(true)
    if (created.ok && created.op === "branchCreate") expect(created.checkedOut).toBe(true)

    const list = service(repo).run({ op: "branchList" })
    expect(list.ok).toBe(true)
    if (list.ok && list.op === "branchList") {
      expect(list.current).toBe("feature/x")
      expect(list.branches.map((branch) => branch.name).sort()).toEqual(["feature/x", "main"])
    }
  }))

  test("branchCreate on an existing name is a typed refusal", guardGit(() => {
    const repo = makeRepo()
    seed(repo)
    service(repo).run({ op: "branchCreate", name: "dup", checkout: false })
    const again = service(repo).run({ op: "branchCreate", name: "dup", checkout: false })
    expect(again.ok).toBe(false)
    if (!again.ok) expect(again.error).toBe("branch_exists")
  }))

  test("checkout refuses a dirty tree, then succeeds when clean", guardGit(() => {
    const repo = makeRepo()
    seed(repo)
    service(repo).run({ op: "branchCreate", name: "other", checkout: false })
    // Dirty (tracked change staged): checkout refused.
    write(repo, "seed.txt", "changed\n")
    service(repo).run({ op: "stage", paths: ["seed.txt"] })
    const dirty = service(repo).run({ op: "checkout", name: "other" })
    expect(dirty.ok).toBe(false)
    if (!dirty.ok) expect(dirty.error).toBe("dirty_tree")
    // Clean it, then checkout succeeds.
    service(repo).run({ op: "unstage", paths: ["seed.txt"] })
    write(repo, "seed.txt", "x\n")
    const clean = service(repo).run({ op: "checkout", name: "other" })
    expect(clean.ok).toBe(true)
    if (clean.ok && clean.op === "checkout") expect(clean.name).toBe("other")
  }))

  test("checkout of an unknown branch is not_found", guardGit(() => {
    const repo = makeRepo()
    seed(repo)
    const result = service(repo).run({ op: "checkout", name: "ghost" })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("not_found")
  }))
})

describe("real repo: push to a local bare remote", () => {
  const seedAndClone = (): { work: string; remote: string } => {
    const remote = makeRepo(true)
    const work = makeRepo()
    write(work, "a.txt", "one\n")
    service(work).run({ op: "stage", paths: ["a.txt"] })
    service(work).run({ op: "commit", message: "one" })
    git(work, "remote", "add", "origin", remote)
    git(work, "push", "-u", "origin", "main")
    return { work, remote }
  }

  test("no upstream is a typed refusal", guardGit(() => {
    const repo = makeRepo()
    write(repo, "a.txt", "x\n")
    service(repo).run({ op: "stage", paths: ["a.txt"] })
    service(repo).run({ op: "commit", message: "one" })
    const result = service(repo).run({ op: "push" })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("no_upstream")
  }))

  test("push advances the remote ref and returns a ref/sha receipt", guardGit(() => {
    const { work, remote } = seedAndClone()
    write(work, "b.txt", "two\n")
    service(work).run({ op: "stage", paths: ["b.txt"] })
    service(work).run({ op: "commit", message: "two" })
    const result = service(work).run({ op: "push" })
    expect(result.ok).toBe(true)
    if (result.ok && result.op === "push") {
      expect(result.ref).toBe("main")
      expect(result.remote).toBe("origin")
      expect(result.sha).toMatch(/^[0-9a-f]{40}$/)
      // The bare remote now points at the pushed SHA.
      expect(git(remote, "rev-parse", "refs/heads/main").stdout.trim()).toBe(result.sha)
    }
  }))

  test("non-fast-forward stays explicit and never rewrites local history", guardGit(() => {
    const { work, remote } = seedAndClone()
    // A second clone advances the remote on a DIFFERENT file.
    const other = makeRepo()
    git(other, "remote", "add", "origin", remote)
    git(other, "fetch", "origin")
    git(other, "checkout", "-B", "main", "origin/main")
    write(other, "remote-change.txt", "remote\n")
    service(other).run({ op: "stage", paths: ["remote-change.txt"] })
    service(other).run({ op: "commit", message: "remote change" })
    git(other, "push", "origin", "main")

    // The original workspace commits a non-conflicting change and pushes.
    write(work, "local-change.txt", "local\n")
    service(work).run({ op: "stage", paths: ["local-change.txt"] })
    service(work).run({ op: "commit", message: "local change" })
    const localHead = git(work, "rev-parse", "HEAD").stdout.trim()
    const result = service(work).run({ op: "push" })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("non_fast_forward")
    expect(git(work, "rev-parse", "HEAD").stdout.trim()).toBe(localHead)
    expect(git(work, "status", "--porcelain").stdout).toBe("")
  }))

  test("conflicting non-fast-forward returns the same typed refusal without starting rebase", guardGit(() => {
    const { work, remote } = seedAndClone()
    const other = makeRepo()
    git(other, "remote", "add", "origin", remote)
    git(other, "fetch", "origin")
    git(other, "checkout", "-B", "main", "origin/main")
    write(other, "a.txt", "remote-edit\n")
    service(other).run({ op: "stage", paths: ["a.txt"] })
    service(other).run({ op: "commit", message: "remote edit a" })
    git(other, "push", "origin", "main")

    // The workspace edits the SAME file → rebase conflict.
    write(work, "a.txt", "local-edit\n")
    service(work).run({ op: "stage", paths: ["a.txt"] })
    service(work).run({ op: "commit", message: "local edit a" })
    const result = service(work).run({ op: "push" })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("non_fast_forward")
    // The failed rebase was aborted: the tree is clean and usable.
    const status = service(work).run({ op: "status" })
    if (status.ok && status.op === "status") {
      expect(status.unstaged).toHaveLength(0)
    }
  }))
})

// ---------------------------------------------------------------------------
// gh path: guarded real availability probe (honest skip)
// ---------------------------------------------------------------------------

describe("gh path: guarded real availability", () => {
  test("gh --version + gh auth status; issueList returns a typed shape or a typed gh error", () => {
    const version = spawnSync("gh", ["--version"], { encoding: "utf8" })
    if (version.status !== 0) {
      console.log("[git-github-host.test] SKIP gh live: gh CLI is not installed")
      return
    }
    const auth = spawnSync("gh", ["auth", "status"], { encoding: "utf8" })
    if (!gitAvailable) {
      console.log("[git-github-host.test] SKIP gh live: git unavailable")
      return
    }
    // Run against the app's own repo (this checkout) so gh can infer a repo.
    const result: GitGithubResult = service(process.cwd()).run({ op: "issueList", limit: 1 })
    if (auth.status !== 0) {
      // Unauthenticated is the honest outcome — a typed class, never a throw.
      expect(result.ok).toBe(false)
      if (!result.ok) expect(["gh_unauthenticated", "gh_unavailable"]).toContain(result.error)
      console.log("[git-github-host.test] gh present but not authenticated — typed refusal asserted")
      return
    }
    // Authenticated: either a decoded issue list, or a typed gh/repo error.
    if (result.ok) {
      expect(result.op).toBe("issueList")
    } else {
      expect(["gh_unauthenticated", "gh_unavailable", "not_found", "operation_failed", "not_a_repo"]).toContain(result.error)
    }
  })
})
