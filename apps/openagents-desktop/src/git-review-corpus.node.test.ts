import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { afterAll, test } from "vite-plus/test"

import { openGitGithubService } from "./git-github-host.ts"
import { runGitFixture } from "../tests/git-fixture.ts"

const roots: string[] = []
const temp = (prefix: string): string => {
  const root = mkdtempSync(path.join(tmpdir(), prefix))
  roots.push(root)
  return root
}
const git = (root: string, args: ReadonlyArray<string>): string =>
  runGitFixture(root, args)
const write = (root: string, relative: string, content: string | Buffer): void =>
  writeFileSync(path.join(root, relative), content)
const repo = (): string => {
  const root = temp("openagents-cut19-")
  git(root, ["init", "--quiet", "-b", "main"])
  git(root, ["config", "user.email", "cut19@openagents.test"])
  git(root, ["config", "user.name", "CUT-19"])
  write(root, "review.txt", "base\n")
  git(root, ["add", "review.txt"])
  git(root, ["commit", "--quiet", "-m", "base"])
  return root
}
const service = (root: string) => openGitGithubService(() => root)
const status = (root: string) => {
  const value = service(root).run({ op: "status" })
  assert.equal(value.ok, true)
  assert.equal(value.op, "status")
  if (!value.ok || value.op !== "status") throw new Error("status unavailable")
  return value
}
const review = (root: string, pathRef: string, source: "staged" | "unstaged" = "unstaged") => {
  const snapshot = status(root)
  return service(root).run({
    op: "diff",
    repositoryRef: snapshot.repositoryRef,
    statusRef: snapshot.statusRef,
    path: pathRef,
    source,
    causalItemRef: null,
  })
}

afterAll(() => {
  for (const root of roots) rmSync(root, { recursive: true, force: true })
})

test("bounded diff exposes typed hunks and discard is exact-snapshot fenced", () => {
  const root = repo()
  write(root, "review.txt", "base\nchanged\n")
  const before = status(root)
  const diff = service(root).run({
    op: "diff",
    repositoryRef: before.repositoryRef,
    statusRef: before.statusRef,
    path: "review.txt",
    source: "unstaged",
    causalItemRef: "timeline.item.file-change.review",
  })
  assert.equal(diff.ok, true)
  assert.equal(diff.op, "diff")
  if (diff.ok && diff.op === "diff") {
    assert.equal(diff.path, "review.txt")
    assert.equal(diff.causalItemRef, "timeline.item.file-change.review")
    assert.ok(diff.hunks.length >= 1)
    assert.match(diff.content, /\+changed/u)
  }

  write(root, "review.txt", "base\nchanged again\n")
  const stale = service(root).run({
    op: "discard",
    repositoryRef: before.repositoryRef,
    statusRef: before.statusRef,
    path: "review.txt",
  })
  assert.equal(stale.ok, false)
  if (!stale.ok) assert.equal(stale.error, "stale_status")

  const current = status(root)
  const discarded = service(root).run({
    op: "discard",
    repositoryRef: current.repositoryRef,
    statusRef: current.statusRef,
    path: "review.txt",
  })
  assert.equal(discarded.ok, true)
  assert.equal(readFileSync(path.join(root, "review.txt"), "utf8"), "base\n")
})

test("binary, secret-shaped, and oversized diffs are refused before projection", () => {
  const root = repo()
  write(root, "binary.bin", Buffer.from([1, 0, 2]))
  git(root, ["add", "binary.bin"])
  git(root, ["commit", "--quiet", "-m", "binary base"])
  write(root, "binary.bin", Buffer.from([3, 0, 4]))
  const binary = review(root, "binary.bin")
  assert.equal(binary.ok, false)
  if (!binary.ok) assert.equal(binary.error, "binary_diff")

  write(root, "review.txt", "token=sk-abcdefghijklmnop\n")
  const secret = review(root, "review.txt")
  assert.equal(secret.ok, false)
  if (!secret.ok) assert.equal(secret.error, "secret_diff")

  write(root, "review.txt", `base\n${"changed line\n".repeat(20_000)}`)
  const oversized = review(root, "review.txt")
  assert.equal(oversized.ok, false)
  if (!oversized.ok) assert.equal(oversized.error, "diff_too_large")
})

test("rename, submodule, detached, conflict, and no-repository states remain explicit", { timeout: 120_000 }, () => {
  const root = repo()
  git(root, ["mv", "review.txt", "renamed.txt"])
  let snapshot = status(root)
  assert.ok(snapshot.staged.some(entry => entry.path === "renamed.txt" && entry.status === "renamed"))
  git(root, ["commit", "--quiet", "-m", "rename"])

  const sub = repo()
  const subHead = git(sub, ["rev-parse", "HEAD"]).trim()
  mkdirSync(path.join(root, "vendor"), { recursive: true })
  renameSync(sub, path.join(root, "vendor/sub"))
  git(root, ["update-index", "--add", "--cacheinfo", `160000,${subHead},vendor/sub`])
  git(root, ["commit", "--quiet", "-m", "submodule"])
  write(path.join(root, "vendor/sub"), "review.txt", "submodule dirty\n")
  snapshot = status(root)
  assert.ok(snapshot.unstaged.some(entry => entry.path === "vendor/sub"))

  git(root, ["switch", "--detach", "--quiet", "HEAD"])
  assert.equal(status(root).detached, true)

  git(root, ["switch", "--quiet", "-c", "conflict-left"])
  write(root, "renamed.txt", "left\n")
  git(root, ["add", "renamed.txt"])
  git(root, ["commit", "--quiet", "-m", "left"])
  git(root, ["switch", "--quiet", "main"])
  write(root, "renamed.txt", "right\n")
  git(root, ["add", "renamed.txt"])
  git(root, ["commit", "--quiet", "-m", "right"])
  try { git(root, ["merge", "conflict-left"]) } catch { /* expected conflict */ }
  snapshot = status(root)
  assert.ok(snapshot.staged.some(entry => entry.status === "unmerged") || snapshot.unstaged.some(entry => entry.status === "unmerged"))
  git(root, ["merge", "--abort"])

  const plain = temp("openagents-cut19-plain-")
  const unavailable = service(plain).run({ op: "status" })
  assert.equal(unavailable.ok, false)
  if (!unavailable.ok) assert.equal(unavailable.error, "not_a_repo")
})
