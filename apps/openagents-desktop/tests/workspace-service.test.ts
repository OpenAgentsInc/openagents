import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { execFileSync } from "node:child_process"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  openWorkspaceService,
  readWorkspaceFile,
  saveWorkspaceFile,
  workspaceGitDiff,
  workspaceGitStatus,
} from "../src/workspace-service.ts"

const roots: string[] = []
const makeRoot = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-workspace-"))
  roots.push(root)
  return root
}

afterEach(() => {
  while (roots.length > 0) rmSync(roots.pop()!, { recursive: true, force: true })
})

describe("Desktop bounded workspace service", () => {
  test("binds one selected root into an explicit WorkContext service", () => {
    const root = makeRoot()
    const outside = makeRoot()
    const file = path.join(root, "README.md")
    const outsideFile = path.join(outside, "private.txt")
    execFileSync("git", ["init", "--quiet"], { cwd: root })
    writeFileSync(file, "inside")
    writeFileSync(outsideFile, "outside")

    const workspace = openWorkspaceService(root)
    expect(workspace.summary().root).toBe(root)
    expect(workspace.read(file)?.content).toBe("inside")
    expect(workspace.read(outsideFile)).toBeNull()
    workspace.dispose()
    workspace.dispose()
    expect(() => workspace.summary()).toThrow("workspace_disposed")
    expect(workspace.read(file)).toBeNull()
    expect(workspace.gitStatus()).toEqual({ state: "unavailable" })
  })

  test("reads and atomically saves an existing bounded text file with a new revision", () => {
    const root = makeRoot()
    const file = path.join(root, "README.md")
    writeFileSync(file, "before")

    const initial = readWorkspaceFile(root, file)
    expect(initial).not.toBeNull()
    if (initial === null) throw new Error("expected workspace file")

    const result = saveWorkspaceFile(root, {
      path: file,
      content: "after",
      expectedRevision: initial.revision,
    })
    expect(result.state).toBe("saved")
    expect(readFileSync(file, "utf8")).toBe("after")
    if (result.state !== "saved") throw new Error("expected saved result")
    expect(result.file.content).toBe("after")
    expect(result.file.revision).not.toBe(initial.revision)
    expect(result.file.truncated).toBe(false)
  })

  test("returns the current bounded file on a stale revision and never overwrites it", () => {
    const root = makeRoot()
    const file = path.join(root, "notes.txt")
    writeFileSync(file, "first")
    const initial = readWorkspaceFile(root, file)
    if (initial === null) throw new Error("expected workspace file")

    writeFileSync(file, "changed elsewhere")
    const result = saveWorkspaceFile(root, {
      path: file,
      content: "stale editor content",
      expectedRevision: initial.revision,
    })
    expect(result.state).toBe("conflict")
    if (result.state !== "conflict") throw new Error("expected conflict result")
    expect(result.file.content).toBe("changed elsewhere")
    expect(readFileSync(file, "utf8")).toBe("changed elsewhere")
  })

  test("rejects traversal and symlink escapes before read or save", () => {
    const root = makeRoot()
    const outside = makeRoot()
    const outsideFile = path.join(outside, "private.txt")
    writeFileSync(outsideFile, "outside")
    const linked = path.join(root, "linked-private.txt")
    symlinkSync(outsideFile, linked)
    mkdirSync(path.join(root, "nested"))
    const nestedLink = path.join(root, "nested", "elsewhere")
    symlinkSync(outside, nestedLink)

    expect(readWorkspaceFile(root, outsideFile)).toBeNull()
    expect(readWorkspaceFile(root, linked)).toBeNull()
    expect(readWorkspaceFile(root, path.join(nestedLink, "private.txt"))).toBeNull()
    expect(saveWorkspaceFile(root, {
      path: linked,
      content: "overwrite",
      expectedRevision: "any",
    }).state).toBe("unavailable")
    expect(readFileSync(outsideFile, "utf8")).toBe("outside")
  })

  test("rejects binary, truncated, and oversized edits rather than writing lossy bytes", () => {
    const root = makeRoot()
    const binary = path.join(root, "image.bin")
    writeFileSync(binary, Buffer.from([0, 1, 2]))
    expect(readWorkspaceFile(root, binary)).toBeNull()

    const large = path.join(root, "large.txt")
    writeFileSync(large, "x".repeat(240_001))
    const largeFile = readWorkspaceFile(root, large)
    expect(largeFile?.truncated).toBe(true)
    if (largeFile === null) throw new Error("expected large projection")
    expect(saveWorkspaceFile(root, {
      path: large,
      content: "short",
      expectedRevision: largeFile.revision,
    }).state).toBe("unavailable")

    const small = path.join(root, "small.txt")
    writeFileSync(small, "small")
    const smallFile = readWorkspaceFile(root, small)
    if (smallFile === null) throw new Error("expected small projection")
    expect(saveWorkspaceFile(root, {
      path: small,
      content: "x".repeat(240_001),
      expectedRevision: smallFile.revision,
    }).state).toBe("unavailable")
  })

  test("projects only bounded typed Git status and a selected non-binary diff", () => {
    const root = makeRoot()
    execFileSync("git", ["init", "--quiet"], { cwd: root })
    execFileSync("git", ["config", "user.email", "fixture@example.test"], { cwd: root })
    execFileSync("git", ["config", "user.name", "Fixture"], { cwd: root })
    const file = path.join(root, "README.md")
    writeFileSync(file, "before\n")
    execFileSync("git", ["add", "README.md"], { cwd: root })
    execFileSync("git", ["commit", "--quiet", "-m", "initial"], { cwd: root })
    writeFileSync(file, "after\n")
    writeFileSync(path.join(root, "new.txt"), "new")

    expect(workspaceGitStatus(root)).toEqual({
      state: "available",
      changes: [
        { path: "README.md", kind: "modified" },
        { path: "new.txt", kind: "untracked" },
      ],
      truncated: false,
    })
    const diff = workspaceGitDiff(root, file)
    expect(diff.state).toBe("available")
    if (diff.state !== "available") throw new Error("expected diff")
    expect(diff.path).toBe("README.md")
    expect(diff.content).toContain("-before")
    expect(diff.content).toContain("+after")
    expect(diff.content).not.toContain(root)
  })

  test("never projects binary, secret-shaped, escape, or non-Git diff output", () => {
    const root = makeRoot()
    execFileSync("git", ["init", "--quiet"], { cwd: root })
    execFileSync("git", ["config", "user.email", "fixture@example.test"], { cwd: root })
    execFileSync("git", ["config", "user.name", "Fixture"], { cwd: root })
    const file = path.join(root, "config.txt")
    writeFileSync(file, "safe\n")
    execFileSync("git", ["add", "config.txt"], { cwd: root })
    execFileSync("git", ["commit", "--quiet", "-m", "initial"], { cwd: root })

    writeFileSync(file, "token=sk-abcdefghijklmnop\n")
    expect(workspaceGitDiff(root, file).state).toBe("unavailable")
    writeFileSync(file, Buffer.from([0, 1, 2]))
    expect(workspaceGitDiff(root, file).state).toBe("unavailable")
    expect(workspaceGitDiff(root, path.join(root, "..", "outside.txt")).state).toBe("unavailable")
  })
})
