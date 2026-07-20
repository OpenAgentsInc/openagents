import { afterEach, describe, expect, test } from "vite-plus/test"
import { existsSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  decodeWorkspaceChange,
  decodeWorkspaceCreateRequest,
  decodeWorkspaceDeleteRequest,
  decodeWorkspaceMoveRequest,
  decodeWorkspaceCopyRequest,
  decodeWorkspaceDuplicateRequest,
  decodeWorkspaceDocumentRequest,
  decodeWorkspaceDocumentResult,
  decodeWorkspaceDocumentSaveRequest,
  decodeWorkspaceDocumentSaveAsRequest,
  decodeWorkspaceOperationResult,
  decodeWorkspaceRenameRequest,
  decodeWorkspaceRevealRequest,
  decodeWorkspaceSearchPage,
  decodeWorkspaceSearchRequest,
  decodeWorkspaceTreePage,
  decodeWorkspaceTreeRequest,
  decodeWorkspaceWatchRequest,
  type DesktopWorkspaceChange,
} from "../src/workspace-contract.ts"

import {
  openWorkspaceService,
  openWorkspaceDocument,
  createWorkspaceEntry,
  deleteWorkspaceEntry,
  moveWorkspaceEntry,
  copyWorkspaceEntry,
  duplicateWorkspaceEntry,
  readWorkspaceFile,
  saveWorkspaceFile,
  saveWorkspaceDocument,
  saveWorkspaceDocumentAs,
  renameWorkspaceEntry,
  revealWorkspaceEntry,
  searchWorkspace,
  workspaceGitDiff,
  workspaceGitStatus,
  workspaceTreePage,
  type WorkspaceMutationIo,
  type WorkspaceDocumentIo,
} from "../src/workspace-service.ts"
import type {
  WorkspaceSearchHost,
  WorkspaceSearchRequest,
} from "../src/workspace-search-host.ts"
import type { IdePortableMutationAuthority } from "../src/ide/portable-mutation-authority.ts"
import { isolatedGitEnvironment, runGitFixture } from "./git-fixture.ts"

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
  test("strips inherited Git hook context from fixture repositories", () => {
    const environment = isolatedGitEnvironment({
      PATH: process.env.PATH,
      GIT_DIR: "/private/real-repository/.git",
      GIT_WORK_TREE: "/private/real-repository",
      GIT_INDEX_FILE: "/private/real-repository/.git/index",
    })
    expect(environment.PATH).toBe(process.env.PATH)
    expect(environment.GIT_DIR).toBeUndefined()
    expect(environment.GIT_WORK_TREE).toBeUndefined()
    expect(environment.GIT_INDEX_FILE).toBeUndefined()
  })
  test("decodes only grant-scoped relative document requests and bounded results", () => {
    expect(decodeWorkspaceDocumentRequest({
      grantRef: "workspace.grant.docs",
      pathRef: "src/index.ts",
      root: "/private/root",
    })).toEqual({ grantRef: "workspace.grant.docs", pathRef: "src/index.ts" })
    expect(decodeWorkspaceDocumentRequest({ grantRef: "workspace.grant.docs", pathRef: "../private" })).toBeNull()
    expect(decodeWorkspaceDocumentSaveRequest({
      grantRef: "workspace.grant.docs",
      pathRef: "src/index.ts",
      content: "next",
      expectedRevisionRef: "workspace.document.revision",
    })).toEqual({
      grantRef: "workspace.grant.docs",
      pathRef: "src/index.ts",
      content: "next",
      expectedRevisionRef: "workspace.document.revision",
    })
    expect(decodeWorkspaceDocumentSaveAsRequest({
      grantRef: "workspace.grant.docs",
      pathRef: "src/copy.ts",
      content: "copy",
      expectedRevisionRef: "must-not-cross",
    })).toEqual({
      grantRef: "workspace.grant.docs",
      pathRef: "src/copy.ts",
      content: "copy",
    })
    expect(decodeWorkspaceDocumentSaveAsRequest({
      grantRef: "workspace.grant.docs",
      pathRef: "../copy.ts",
      content: "copy",
    })).toBeNull()
    expect(decodeWorkspaceDocumentResult({
      state: "unavailable",
      reason: "binary",
      message: "Binary files cannot be opened.",
      root: "/private/root",
    })).toEqual({ state: "unavailable", reason: "binary", message: "Binary files cannot be opened." })
  })

  test("opens UTF-8 documents through an exact grant and projects no absolute root", () => {
    const root = makeRoot()
    mkdirSync(path.join(root, "src"))
    writeFileSync(path.join(root, "src", "index.ts"), "const ready = true\r\n")
    const result = openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "src/index.ts",
    })
    expect(result).toMatchObject({
      state: "available",
      document: {
        grantRef: "workspace.grant.docs",
        pathRef: "src/index.ts",
        content: "const ready = true\r\n",
        languageMode: "typescript",
        encoding: "utf-8",
        lineEnding: "crlf",
      },
    })
    expect(JSON.stringify(result)).not.toContain(root)
    expect(openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.stale",
      pathRef: "src/index.ts",
    })).toMatchObject({ state: "unavailable", reason: "grant_revoked" })

    const bomFile = path.join(root, "README.md")
    writeFileSync(bomFile, Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("hello\n")]))
    const bom = openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "README.md",
    })
    expect(bom).toMatchObject({ state: "available", document: { content: "hello\n", encoding: "utf-8-bom" } })
    if (bom.state !== "available") throw new Error("expected BOM document")
    const savedBom = saveWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "README.md",
      content: "updated\n",
      expectedRevisionRef: bom.document.revisionRef,
    })
    expect(savedBom).toMatchObject({ state: "saved", document: { encoding: "utf-8-bom" } })
    expect(readFileSync(bomFile).subarray(0, 3)).toEqual(Buffer.from([0xef, 0xbb, 0xbf]))
  })

  test("classifies missing, directory, binary, oversized, encoding, ignored, and escape outcomes", () => {
    const root = makeRoot()
    const outside = makeRoot()
    runGitFixture(root, ["init", "--quiet"])
    writeFileSync(path.join(root, ".gitignore"), "ignored.txt\n")
    mkdirSync(path.join(root, "src"))
    writeFileSync(path.join(root, "binary.bin"), Buffer.from([0, 1, 2]))
    writeFileSync(path.join(root, "large.txt"), "x".repeat(1_000_001))
    writeFileSync(path.join(root, "latin.txt"), Buffer.from([0xff, 0xfe, 0x61]))
    writeFileSync(path.join(root, "ignored.txt"), "hidden")
    writeFileSync(path.join(root, ".env"), "SECRET=value")
    writeFileSync(path.join(outside, "private.txt"), "outside")
    symlinkSync(path.join(outside, "private.txt"), path.join(root, "linked.txt"))
    const open = (pathRef: string) => openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef,
    })
    expect(open("missing.txt")).toMatchObject({ state: "unavailable", reason: "missing" })
    expect(open("src")).toMatchObject({ state: "unavailable", reason: "directory" })
    expect(open("binary.bin")).toMatchObject({ state: "unavailable", reason: "binary" })
    expect(open("large.txt")).toMatchObject({ state: "unavailable", reason: "too_large" })
    expect(open("latin.txt")).toMatchObject({ state: "unavailable", reason: "unsupported_encoding" })
    expect(open("ignored.txt")).toMatchObject({ state: "unavailable", reason: "unavailable" })
    expect(open(".env")).toMatchObject({ state: "unavailable", reason: "unavailable" })
    expect(open("linked.txt")).toMatchObject({ state: "unavailable", reason: "unavailable" })
    expect(open("../private.txt")).toMatchObject({ state: "unavailable", reason: "invalid_ref" })
  })

  test("saves atomically with a revision receipt and refuses stale editors", () => {
    const root = makeRoot()
    const pathRef = "README.md"
    const file = path.join(root, pathRef)
    writeFileSync(file, "before\n")
    const opened = openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef,
    })
    if (opened.state !== "available") throw new Error("expected document")
    writeFileSync(file, "changed elsewhere\n")
    const conflict = saveWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef,
      content: "stale editor\n",
      expectedRevisionRef: opened.document.revisionRef,
    })
    expect(conflict).toMatchObject({ state: "conflict", current: { content: "changed elsewhere\n" } })
    expect(readFileSync(file, "utf8")).toBe("changed elsewhere\n")
    if (conflict.state !== "conflict") throw new Error("expected conflict")
    const saved = saveWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef,
      content: "resolved\n",
      expectedRevisionRef: conflict.current.revisionRef,
    })
    expect(saved).toMatchObject({ state: "saved", document: { content: "resolved\n", pathRef } })
    expect(readFileSync(file, "utf8")).toBe("resolved\n")
    if (saved.state !== "saved") throw new Error("expected saved")
    expect(saved.document.revisionRef).not.toBe(conflict.current.revisionRef)
  })

  test("Save As creates a new relative UTF-8 document and never overwrites", () => {
    const root = makeRoot()
    mkdirSync(path.join(root, "src"))
    const saved = saveWorkspaceDocumentAs(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "src/copy.ts",
      content: "export const copy = true\n",
    })
    expect(saved).toMatchObject({
      state: "saved",
      document: {
        grantRef: "workspace.grant.docs",
        pathRef: "src/copy.ts",
        content: "export const copy = true\n",
        languageMode: "typescript",
        encoding: "utf-8",
      },
    })
    expect(readFileSync(path.join(root, "src/copy.ts"), "utf8")).toBe("export const copy = true\n")

    writeFileSync(path.join(root, "src/existing.ts"), "keep me\n")
    const conflict = saveWorkspaceDocumentAs(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "src/existing.ts",
      content: "overwrite attempt\n",
    })
    expect(conflict).toMatchObject({ state: "conflict", current: { content: "keep me\n" } })
    expect(readFileSync(path.join(root, "src/existing.ts"), "utf8")).toBe("keep me\n")
    expect(JSON.stringify([saved, conflict])).not.toContain(root)
  })

  test("Save As fails closed for revoked, hidden, ignored, missing-parent, race, and permission targets", () => {
    const root = makeRoot()
    runGitFixture(root, ["init", "--quiet"])
    writeFileSync(path.join(root, ".gitignore"), "ignored.ts\n")
    const input = { grantRef: "workspace.grant.docs", pathRef: "copy.ts", content: "copy" }
    expect(saveWorkspaceDocumentAs(root, "workspace.grant.other", input))
      .toMatchObject({ state: "unavailable", reason: "grant_revoked" })
    expect(saveWorkspaceDocumentAs(root, "workspace.grant.docs", { ...input, pathRef: ".hidden" }))
      .toMatchObject({ state: "unavailable", reason: "unavailable" })
    expect(saveWorkspaceDocumentAs(root, "workspace.grant.docs", { ...input, pathRef: "ignored.ts" }))
      .toMatchObject({ state: "unavailable", reason: "unavailable" })
    expect(saveWorkspaceDocumentAs(root, "workspace.grant.docs", { ...input, pathRef: "missing/copy.ts" }))
      .toMatchObject({ state: "unavailable", reason: "missing" })

    const raceIo: WorkspaceDocumentIo = {
      read: absolutePath => readFileSync(absolutePath),
      replace: () => undefined,
      create: absolutePath => {
        writeFileSync(absolutePath, "race winner")
        throw Object.assign(new Error("exists"), { code: "EEXIST" })
      },
    }
    const raced = saveWorkspaceDocumentAs(root, "workspace.grant.docs", input, raceIo)
    expect(raced).toMatchObject({ state: "conflict", current: { content: "race winner" } })
    expect(readFileSync(path.join(root, "copy.ts"), "utf8")).toBe("race winner")

    const denied = Object.assign(new Error("private detail"), { code: "EACCES" })
    const deniedResult = saveWorkspaceDocumentAs(root, "workspace.grant.docs", {
      ...input,
      pathRef: "denied.ts",
    }, {
      read: absolutePath => readFileSync(absolutePath),
      replace: () => undefined,
      create: () => { throw denied },
    })
    expect(deniedResult).toMatchObject({ state: "unavailable", reason: "permission_denied" })
    expect(JSON.stringify(deniedResult)).not.toContain("private detail")
  })

  test("surfaces injected document permission loss and WorkContext disposal without root leakage", () => {
    const root = makeRoot()
    writeFileSync(path.join(root, "README.md"), "before")
    const denied = Object.assign(new Error("private operating-system detail"), { code: "EACCES" })
    const deniedIo: WorkspaceDocumentIo = {
      read: () => { throw denied },
      replace: () => { throw denied },
      create: () => { throw denied },
    }
    const deniedOpen = openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "README.md",
    }, deniedIo)
    expect(deniedOpen).toMatchObject({ state: "unavailable", reason: "permission_denied" })
    expect(JSON.stringify(deniedOpen)).not.toContain(root)
    expect(JSON.stringify(deniedOpen)).not.toContain("private operating-system detail")

    const opened = openWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "README.md",
    })
    if (opened.state !== "available") throw new Error("expected document")
    const deniedSave = saveWorkspaceDocument(root, "workspace.grant.docs", {
      grantRef: "workspace.grant.docs",
      pathRef: "README.md",
      content: "after",
      expectedRevisionRef: opened.document.revisionRef,
    }, {
      read: absolutePath => readFileSync(absolutePath),
      replace: () => { throw denied },
      create: () => { throw denied },
    })
    expect(deniedSave).toMatchObject({ state: "unavailable", reason: "permission_denied" })
    expect(readFileSync(path.join(root, "README.md"), "utf8")).toBe("before")

    const workspace = openWorkspaceService(root, { grantRef: "workspace.grant.docs" })
    workspace.dispose()
    expect(workspace.openDocument({ grantRef: "workspace.grant.docs", pathRef: "README.md" }))
      .toMatchObject({ state: "unavailable", reason: "grant_revoked" })
    expect(workspace.saveDocument({
      grantRef: "workspace.grant.docs",
      pathRef: "README.md",
      content: "after",
      expectedRevisionRef: "workspace.document.stale",
    })).toMatchObject({ state: "unavailable", reason: "grant_revoked" })
  })

  test("binds one selected root into an explicit WorkContext service", () => {
    const root = makeRoot()
    const outside = makeRoot()
    const file = path.join(root, "README.md")
    const outsideFile = path.join(outside, "private.txt")
    runGitFixture(root, ["init", "--quiet"])
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

  test("keeps reads available and performs no mutation IO when portable authority refuses", () => {
    const root = makeRoot()
    const file = path.join(root, "README.md")
    writeFileSync(file, "before")
    mkdirSync(path.join(root, "archive"))
    const mutationCalls: string[] = []
    const documentCalls: string[] = []
    const authority: IdePortableMutationAuthority = {
      authorize: () => ({ _tag: "Refused", reason: "sync_unavailable" }),
      reauthorize: () => false,
    }
    const workspace = openWorkspaceService(root, {
      grantRef: "workspace.grant.portable",
      mutationAuthority: authority,
      mutationIo: {
        createFile: () => mutationCalls.push("createFile"),
        createDirectory: () => mutationCalls.push("createDirectory"),
        rename: () => mutationCalls.push("rename"),
        copyFile: () => mutationCalls.push("copyFile"),
        deleteFile: () => mutationCalls.push("deleteFile"),
        deleteDirectory: () => mutationCalls.push("deleteDirectory"),
      },
      documentIo: {
        read: absolutePath => {
          documentCalls.push("read")
          return readFileSync(absolutePath)
        },
        replace: () => documentCalls.push("replace"),
        create: () => documentCalls.push("create"),
      },
    })

    const opened = workspace.openDocument({ grantRef: workspace.grantRef, pathRef: "README.md" })
    expect(opened.state).toBe("available")
    expect(workspace.read(file)?.content).toBe("before")
    expect(workspace.createEntry({ parentRef: "", name: "new.txt", kind: "file" }).state).toBe("unavailable")
    expect(workspace.renameEntry({ pathRef: "README.md", name: "renamed.md", expectedRevisionRef: "unused" }).state).toBe("unavailable")
    expect(workspace.moveEntry({ pathRef: "README.md", destinationParentRef: "", expectedRevisionRef: "unused" }).state).toBe("unavailable")
    expect(workspace.copyEntry({ pathRef: "README.md", destinationParentRef: "", expectedRevisionRef: "unused" }).state).toBe("unavailable")
    expect(workspace.duplicateEntry({ pathRef: "README.md", expectedRevisionRef: "unused" }).state).toBe("unavailable")
    expect(workspace.deleteEntry({ pathRef: "README.md", expectedRevisionRef: "unused" }).state).toBe("unavailable")
    expect(workspace.saveDocument({
      grantRef: workspace.grantRef,
      pathRef: "README.md",
      content: "after",
      expectedRevisionRef: "unused",
    })).toMatchObject({ state: "unavailable", reason: "grant_revoked" })
    expect(workspace.saveDocumentAs({
      grantRef: workspace.grantRef,
      pathRef: "new.md",
      content: "after",
    })).toMatchObject({ state: "unavailable", reason: "grant_revoked" })
    expect(workspace.save({ path: file, content: "after", expectedRevision: "unused" }).state).toBe("unavailable")
    expect(mutationCalls).toEqual([])
    expect(documentCalls).toEqual(["read"])
    expect(readFileSync(file, "utf8")).toBe("before")
    workspace.dispose()
  })

  test("re-reads the portable permit immediately before each write boundary", () => {
    const root = makeRoot()
    const file = path.join(root, "README.md")
    writeFileSync(file, "before")
    mkdirSync(path.join(root, "archive"))
    const writes: string[] = []
    let reauthorizationCount = 0
    const permit = {
      _tag: "Portable" as const,
      key: "portable:attachment.1:1",
      grantRef: "workspace.grant.portable",
      sessionRef: "session.1",
      workContextRef: "work-context.1",
      attachmentRef: "attachment.1",
      generation: 1,
      targetRef: "target.local.1",
    }
    const authority: IdePortableMutationAuthority = {
      authorize: () => ({ _tag: "Permitted", permit }),
      reauthorize: () => {
        reauthorizationCount += 1
        return false
      },
    }
    const workspace = openWorkspaceService(root, {
      grantRef: permit.grantRef,
      mutationAuthority: authority,
      mutationIo: {
        createFile: () => writes.push("createFile"),
        createDirectory: () => writes.push("createDirectory"),
        rename: () => writes.push("rename"),
        copyFile: () => writes.push("copyFile"),
        deleteFile: () => writes.push("deleteFile"),
        deleteDirectory: () => writes.push("deleteDirectory"),
      },
      documentIo: {
        read: absolutePath => readFileSync(absolutePath),
        replace: () => writes.push("replace"),
        create: () => writes.push("create"),
      },
    })

    expect(workspace.createEntry({ parentRef: "", name: "new.txt", kind: "file" }).state).toBe("unavailable")
    const tree = workspace.tree({ directoryRef: "" })
    if (tree.state !== "available") throw new Error("expected tree")
    const readme = tree.entries.find(entry => entry.pathRef === "README.md")
    if (readme === undefined) throw new Error("expected README entry")
    const revisionInput = { pathRef: readme.pathRef, expectedRevisionRef: readme.revisionRef }
    expect(workspace.renameEntry({ ...revisionInput, name: "renamed.md" }).state).toBe("unavailable")
    expect(workspace.moveEntry({ ...revisionInput, destinationParentRef: "archive" }).state).toBe("unavailable")
    expect(workspace.copyEntry({ ...revisionInput, destinationParentRef: "archive" }).state).toBe("unavailable")
    expect(workspace.duplicateEntry(revisionInput).state).toBe("unavailable")
    expect(workspace.deleteEntry(revisionInput).state).toBe("unavailable")
    const opened = workspace.openDocument({ grantRef: permit.grantRef, pathRef: "README.md" })
    if (opened.state !== "available") throw new Error("expected document")
    expect(workspace.saveDocument({
      grantRef: permit.grantRef,
      pathRef: "README.md",
      content: "after",
      expectedRevisionRef: opened.document.revisionRef,
    })).toMatchObject({ state: "unavailable", reason: "grant_revoked" })
    expect(workspace.saveDocumentAs({
      grantRef: permit.grantRef,
      pathRef: "new.md",
      content: "after",
    })).toMatchObject({ state: "unavailable", reason: "grant_revoked" })
    const current = workspace.read(file)
    if (current === null) throw new Error("expected file")
    expect(workspace.save({ path: file, content: "after", expectedRevision: current.revision }).state).toBe("unavailable")
    expect(writes).toEqual([])
    expect(reauthorizationCount).toBe(9)
    expect(readFileSync(file, "utf8")).toBe("before")
    expect(existsSync(path.join(root, "new.txt"))).toBe(false)
    workspace.dispose()
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
    runGitFixture(root, ["init", "--quiet"])
    runGitFixture(root, ["config", "user.email", "fixture@example.test"])
    runGitFixture(root, ["config", "user.name", "Fixture"])
    const file = path.join(root, "README.md")
    writeFileSync(file, "before\n")
    runGitFixture(root, ["add", "README.md"])
    runGitFixture(root, ["commit", "--quiet", "-m", "initial"])
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
    runGitFixture(root, ["init", "--quiet"])
    runGitFixture(root, ["config", "user.email", "fixture@example.test"])
    runGitFixture(root, ["config", "user.name", "Fixture"])
    const file = path.join(root, "config.txt")
    writeFileSync(file, "safe\n")
    runGitFixture(root, ["add", "config.txt"])
    runGitFixture(root, ["commit", "--quiet", "-m", "initial"])

    writeFileSync(file, "token=sk-abcdefghijklmnop\n")
    expect(workspaceGitDiff(root, file).state).toBe("unavailable")
    writeFileSync(file, Buffer.from([0, 1, 2]))
    expect(workspaceGitDiff(root, file).state).toBe("unavailable")
    expect(workspaceGitDiff(root, path.join(root, "..", "outside.txt")).state).toBe("unavailable")
  })

  test("pages a lazy relative tree while omitting ignored, secret, hidden, and symlink entries", () => {
    const root = makeRoot()
    const outside = makeRoot()
    runGitFixture(root, ["init", "--quiet"])
    writeFileSync(path.join(root, ".gitignore"), "ignored/\n*.log\n")
    mkdirSync(path.join(root, "src"))
    mkdirSync(path.join(root, "ignored"))
    mkdirSync(path.join(root, "node_modules"))
    writeFileSync(path.join(root, "README.md"), "hello")
    writeFileSync(path.join(root, "debug.log"), "ignored")
    writeFileSync(path.join(root, ".env"), "TOKEN=private")
    writeFileSync(path.join(root, "client.pem"), "private")
    writeFileSync(path.join(root, "ignored", "ignored.ts"), "ignored")
    writeFileSync(path.join(root, "src", "index.ts"), "export const ready = true")
    symlinkSync(outside, path.join(root, "outside-link"))

    const page = workspaceTreePage({
      root,
      grantRef: "workspace.grant.fixture",
      directoryRef: "",
      limit: 2,
    })
    expect(page.state).toBe("available")
    if (page.state !== "available") throw new Error("expected tree page")
    expect(page.entries.map(entry => [entry.pathRef, entry.kind])).toEqual([
      ["src", "directory"],
      ["README.md", "file"],
    ])
    expect(page.nextOffset).toBeNull()
    expect(JSON.stringify(page)).not.toContain(root)
    expect(JSON.stringify(page)).not.toContain(".env")
    expect(JSON.stringify(page)).not.toContain("client.pem")
    expect(JSON.stringify(page)).not.toContain("outside-link")

    const nested = workspaceTreePage({
      root,
      grantRef: "workspace.grant.fixture",
      directoryRef: "src",
    })
    expect(nested).toMatchObject({
      state: "available",
      directoryRef: "src",
      entries: [{ pathRef: "src/index.ts", kind: "file" }],
    })
    expect(workspaceTreePage({
      root,
      grantRef: "workspace.grant.fixture",
      directoryRef: "../",
    }).state).toBe("unavailable")
    expect(workspaceTreePage({
      root,
      grantRef: "workspace.grant.fixture",
      directoryRef: "outside-link",
    }).state).toBe("unavailable")
  })

  test("keeps literal leading-colon filenames from emptying their workspace siblings", () => {
    if (process.platform === "win32") return
    const root = makeRoot()
    runGitFixture(root, ["init", "--quiet"])
    writeFileSync(path.join(root, ".gitignore"), "ignored.txt\n")
    writeFileSync(path.join(root, ":-"), "literal pathspec-shaped filename")
    writeFileSync(path.join(root, "README.md"), "visible")
    writeFileSync(path.join(root, "ignored.txt"), "hidden")

    const page = workspaceTreePage({
      root,
      grantRef: "workspace.grant.literal-pathspec",
      directoryRef: "",
    })

    expect(page.state === "available" ? page.entries.map(entry => entry.pathRef) : []).toEqual([
      ":-",
      "README.md",
    ])
  })

  test("bounds path/content search and withholds binary, secret, ignored, and raw-root data", () => {
    const root = makeRoot()
    runGitFixture(root, ["init", "--quiet"])
    writeFileSync(path.join(root, ".gitignore"), "ignored.txt\n")
    mkdirSync(path.join(root, "src"))
    writeFileSync(path.join(root, "src", "alpha.ts"), "first\nneedle in safe content\nlast")
    writeFileSync(path.join(root, "src", "beta.ts"), "needle token=sk-abcdefghijklmnop")
    writeFileSync(path.join(root, "ignored.txt"), "needle")
    writeFileSync(path.join(root, "image.bin"), Buffer.from([0, 110, 101, 101, 100, 108, 101]))

    const content = searchWorkspace({
      root,
      grantRef: "workspace.grant.search",
      query: "needle",
      mode: "content",
    })
    expect(content).toMatchObject({
      state: "available",
      matches: [{
        pathRef: "src/alpha.ts",
        kind: "content",
        line: 2,
        preview: "needle in safe content",
      }],
    })
    expect(JSON.stringify(content)).not.toContain(root)
    expect(JSON.stringify(content)).not.toContain("sk-")
    expect(JSON.stringify(content)).not.toContain("ignored.txt")

    const byPath = searchWorkspace({
      root,
      grantRef: "workspace.grant.search",
      query: "alpha",
      mode: "path",
    })
    expect(byPath).toMatchObject({
      state: "available",
      matches: [{ pathRef: "src/alpha.ts", kind: "path" }],
    })
  })

  test("creates, revision-renames, reveals, and non-recursively deletes only visible relative entries", async () => {
    const root = makeRoot()
    const outside = makeRoot()
    runGitFixture(root, ["init", "--quiet"])
    writeFileSync(path.join(root, ".gitignore"), "ignored.txt\n")
    const revealed: string[] = []
    const changes: unknown[] = []
    const workspace = openWorkspaceService(root, {
      grantRef: "workspace.grant.mutations",
      watchFactory: () => ({ close: () => undefined }),
      reveal: absolutePath => {
        revealed.push(absolutePath)
        return true
      },
    })
    const subscription = workspace.subscribe(change => changes.push(change))

    const createdDirectory = workspace.createEntry({ parentRef: "", name: "docs", kind: "directory" })
    expect(createdDirectory.state).toBe("created")
    const createdFile = workspace.createEntry({ parentRef: "docs", name: "README.md", kind: "file" })
    expect(createdFile.state).toBe("created")
    if (createdFile.state !== "created") throw new Error("expected created file")
    expect(JSON.stringify([createdDirectory, createdFile])).not.toContain(root)
    expect(workspace.createEntry({ parentRef: "docs", name: "README.md", kind: "file" }).state).toBe("conflict")
    expect(workspace.createEntry({ parentRef: "", name: ".hidden", kind: "file" }).state).toBe("unavailable")
    expect(workspace.createEntry({ parentRef: "", name: "client.pem", kind: "file" }).state).toBe("unavailable")
    expect(workspace.createEntry({ parentRef: "", name: "ignored.txt", kind: "file" }).state).toBe("unavailable")
    expect(workspace.createEntry({ parentRef: "../", name: "escape.txt", kind: "file" }).state).toBe("unavailable")

    expect(workspace.renameEntry({
      pathRef: "docs/README.md",
      name: "guide.md",
      expectedRevisionRef: "workspace.entry.stale",
    }).state).toBe("conflict")
    expect(workspace.createEntry({ parentRef: "docs", name: "existing.md", kind: "file" }).state).toBe("created")
    expect(workspace.renameEntry({
      pathRef: "docs/README.md",
      name: "existing.md",
      expectedRevisionRef: createdFile.entry.revisionRef,
    }).state).toBe("conflict")
    const renamed = workspace.renameEntry({
      pathRef: "docs/README.md",
      name: "guide.md",
      expectedRevisionRef: createdFile.entry.revisionRef,
    })
    expect(renamed.state).toBe("renamed")
    if (renamed.state !== "renamed") throw new Error("expected renamed entry")
    expect(renamed.entry.pathRef).toBe("docs/guide.md")
    expect(await workspace.revealEntry({ pathRef: "docs/guide.md" })).toEqual({
      state: "revealed",
      pathRef: "docs/guide.md",
    })
    expect(revealed).toEqual([path.join(realpathSync(root), "docs", "guide.md")])
    symlinkSync(outside, path.join(root, "outside-link"))
    expect((await workspace.revealEntry({ pathRef: "outside-link" })).state).toBe("unavailable")

    const rootPage = workspace.tree({ directoryRef: "" })
    if (rootPage.state !== "available") throw new Error("expected root tree")
    const docs = rootPage.entries.find(entry => entry.pathRef === "docs")!
    expect(workspace.deleteEntry({ pathRef: "docs", expectedRevisionRef: docs.revisionRef }).state).toBe("conflict")
    expect(workspace.deleteEntry({ pathRef: "docs/guide.md", expectedRevisionRef: "workspace.entry.stale" }).state).toBe("conflict")
    expect(workspace.deleteEntry({ pathRef: "docs/guide.md", expectedRevisionRef: renamed.entry.revisionRef })).toEqual({
      state: "deleted",
      pathRef: "docs/guide.md",
    })
    const docsPage = workspace.tree({ directoryRef: "docs" })
    if (docsPage.state !== "available") throw new Error("expected docs tree")
    const existing = docsPage.entries.find(entry => entry.pathRef === "docs/existing.md")!
    expect(workspace.deleteEntry({ pathRef: existing.pathRef, expectedRevisionRef: existing.revisionRef }).state).toBe("deleted")
    const refreshedRoot = workspace.tree({ directoryRef: "" })
    if (refreshedRoot.state !== "available") throw new Error("expected refreshed root")
    const emptyDocs = refreshedRoot.entries.find(entry => entry.pathRef === "docs")!
    expect(workspace.deleteEntry({ pathRef: "docs", expectedRevisionRef: emptyDocs.revisionRef })).toEqual({
      state: "deleted",
      pathRef: "docs",
    })
    expect(changes).toHaveLength(7)
    expect(changes.every(change => (change as { kind?: unknown }).kind === "changed")).toBe(true)
    subscription.close()
    workspace.dispose()
  })

  test("moves, copies, and duplicates files only through expected revisions and admitted destinations", () => {
    const root = makeRoot()
    mkdirSync(path.join(root, "src"))
    mkdirSync(path.join(root, "archive"))
    writeFileSync(path.join(root, "src", "index.ts"), "export const value = 1\n")
    const sourcePage = workspaceTreePage({ root, grantRef: "workspace.grant.operations", directoryRef: "src" })
    if (sourcePage.state !== "available") throw new Error("expected source page")
    const source = sourcePage.entries.find(entry => entry.pathRef === "src/index.ts")!
    expect(moveWorkspaceEntry(root, {
      pathRef: source.pathRef,
      destinationParentRef: "archive",
      expectedRevisionRef: "revision.stale",
    }).state).toBe("conflict")
    const moved = moveWorkspaceEntry(root, {
      pathRef: source.pathRef,
      destinationParentRef: "archive",
      expectedRevisionRef: source.revisionRef,
    })
    expect(moved.state).toBe("renamed")
    if (moved.state !== "renamed") throw new Error("expected move")
    const copied = copyWorkspaceEntry(root, {
      pathRef: moved.entry.pathRef,
      destinationParentRef: "src",
      expectedRevisionRef: moved.entry.revisionRef,
    })
    expect(copied.state).toBe("created")
    if (copied.state !== "created") throw new Error("expected copy")
    const duplicated = duplicateWorkspaceEntry(root, {
      pathRef: copied.entry.pathRef,
      expectedRevisionRef: copied.entry.revisionRef,
    })
    expect(duplicated.state).toBe("created")
    if (duplicated.state !== "created") throw new Error("expected duplicate")
    expect(duplicated.entry.pathRef).toBe("src/index copy.ts")
    expect(readFileSync(path.join(root, "src", "index copy.ts"), "utf8")).toContain("value = 1")
    const rootPage = workspaceTreePage({ root, grantRef: "workspace.grant.operations", directoryRef: "" })
    if (rootPage.state !== "available") throw new Error("expected root page")
    const archive = rootPage.entries.find(entry => entry.pathRef === "archive")!
    expect(moveWorkspaceEntry(root, {
      pathRef: "archive",
      destinationParentRef: "archive",
      expectedRevisionRef: archive.revisionRef,
    }).state).toBe("conflict")
    expect(decodeWorkspaceMoveRequest({ pathRef: "src/index.ts", destinationParentRef: "archive", expectedRevisionRef: "revision" })).not.toBeNull()
    expect(decodeWorkspaceCopyRequest({ pathRef: "src/index.ts", destinationParentRef: "archive", expectedRevisionRef: "revision" })).not.toBeNull()
    expect(decodeWorkspaceDuplicateRequest({ pathRef: "src/index.ts", expectedRevisionRef: "revision" })).not.toBeNull()
  })

  test("classifies permission loss for every mutation and reveal without leaking the root", async () => {
    const root = makeRoot()
    writeFileSync(path.join(root, "source.txt"), "safe")
    const page = workspaceTreePage({ root, grantRef: "workspace.grant.permission", directoryRef: "" })
    if (page.state !== "available") throw new Error("expected root tree")
    const source = page.entries.find(entry => entry.pathRef === "source.txt")!
    mkdirSync(path.join(root, "destination"))
    const denied = Object.assign(new Error("private operating-system detail"), { code: "EACCES" })
    const deny = (): never => { throw denied }
    const deniedIo: WorkspaceMutationIo = {
      createFile: deny,
      createDirectory: deny,
      rename: deny,
      copyFile: deny,
      deleteFile: deny,
      deleteDirectory: deny,
    }
    const results = [
      createWorkspaceEntry(root, { parentRef: "", name: "new.txt", kind: "file" }, deniedIo),
      renameWorkspaceEntry(root, {
        pathRef: "source.txt",
        name: "renamed.txt",
        expectedRevisionRef: source.revisionRef,
      }, deniedIo),
      moveWorkspaceEntry(root, {
        pathRef: "source.txt",
        destinationParentRef: "destination",
        expectedRevisionRef: source.revisionRef,
      }, deniedIo),
      copyWorkspaceEntry(root, {
        pathRef: "source.txt",
        destinationParentRef: "destination",
        expectedRevisionRef: source.revisionRef,
      }, deniedIo),
      duplicateWorkspaceEntry(root, {
        pathRef: "source.txt",
        expectedRevisionRef: source.revisionRef,
      }, deniedIo),
      deleteWorkspaceEntry(root, {
        pathRef: "source.txt",
        expectedRevisionRef: source.revisionRef,
      }, deniedIo),
      await revealWorkspaceEntry(root, { pathRef: "source.txt" }, deny),
    ]
    expect(results.map(result => result.state)).toEqual([
      "permission_denied",
      "permission_denied",
      "permission_denied",
      "permission_denied",
      "permission_denied",
      "permission_denied",
      "permission_denied",
    ])
    expect(JSON.stringify(results)).not.toContain(root)
    expect(JSON.stringify(results)).not.toContain("private operating-system detail")
    expect(readFileSync(path.join(root, "source.txt"), "utf8")).toBe("safe")
  })

  test("declares cache epochs, invalidates once per watcher event, and disposes watchers exactly once", async () => {
    const root = makeRoot()
    writeFileSync(path.join(root, "README.md"), "before")
    const watcherCallbacks: Array<(pathRef: string | null) => void> = []
    let watchStarts = 0
    let watchCloses = 0
    const scheduled: Array<() => void> = []
    const workspace = openWorkspaceService(root, {
      grantRef: "workspace.grant.lifecycle",
      watchFactory: (_root, listener) => {
        watchStarts += 1
        watcherCallbacks.push(listener)
        let closed = false
        return { close: () => { if (!closed) { closed = true; watchCloses += 1 } } }
      },
      watchScheduler: flush => {
        let active = true
        scheduled.push(() => { if (active) flush() })
        return { cancel: () => { active = false } }
      },
    })
    const changesA: unknown[] = []
    const changesB: unknown[] = []
    const first = workspace.subscribe(change => changesA.push(change))
    const second = workspace.subscribe(change => changesB.push(change))
    expect(watchStarts).toBe(1)
    const pageA = workspace.tree({ directoryRef: "" })
    const pageB = workspace.tree({ directoryRef: "" })
    expect(pageB).toBe(pageA)
    expect(pageA.state === "available" ? pageA.cache.epoch : null).toBe(0)

    watcherCallbacks[0]?.("README.md")
    watcherCallbacks[0]?.("src/index.ts")
    watcherCallbacks[0]?.("README.md")
    scheduled.shift()?.()
    expect(changesA).toEqual([{
      kind: "changed", pathRef: null, pathRefs: ["README.md", "src/index.ts"], epoch: 1,
    }])
    expect(changesB).toEqual(changesA)
    const pageAfterChange = workspace.tree({ directoryRef: "" })
    expect(pageAfterChange).not.toBe(pageA)
    expect(pageAfterChange.state === "available" ? pageAfterChange.cache.epoch : null).toBe(1)

    watcherCallbacks[0]?.(null)
    scheduled.shift()?.()
    expect(changesA.at(-1)).toEqual({ kind: "overflow", pathRef: null, epoch: 2 })
    workspace.refresh()
    expect(changesA.at(-1)).toEqual({ kind: "refresh", pathRef: null, epoch: 3 })

    first.close()
    first.close()
    expect(watchCloses).toBe(0)
    second.close()
    second.close()
    expect(watchCloses).toBe(1)
    workspace.dispose()
    workspace.dispose()
    expect(watchCloses).toBe(1)
    expect(workspace.tree({ directoryRef: "" }).state).toBe("unavailable")
    expect((await workspace.search({ query: "readme", mode: "path" }).result).state).toBe("unavailable")
  })

  test("drops generated-tree churn and bounds a 10,000-event watcher burst", () => {
    const root = makeRoot()
    const watcherCallbacks: Array<(pathRef: string | null) => void> = []
    const scheduled: Array<() => void> = []
    const workspace = openWorkspaceService(root, {
      watchFactory: (_root, listener) => {
        watcherCallbacks.push(listener)
        return { close: () => undefined }
      },
      watchScheduler: flush => {
        let active = true
        scheduled.push(() => { if (active) flush() })
        return { cancel: () => { active = false } }
      },
    })
    const changes: DesktopWorkspaceChange[] = []
    const subscription = workspace.subscribe(change => changes.push(change))
    for (let index = 0; index < 10_000; index += 1) {
      watcherCallbacks[0]?.(`node_modules/pkg-${index}/index.js`)
      watcherCallbacks[0]?.(`.git/objects/${index}`)
      watcherCallbacks[0]?.(`dist/chunk-${index}.js`)
    }
    expect(scheduled).toHaveLength(0)
    expect(changes).toEqual([])
    for (let index = 0; index < 256; index += 1) watcherCallbacks[0]?.(`src/generated-${index}.ts`)
    watcherCallbacks[0]?.("src/generated-255.ts")
    scheduled.shift()?.()
    expect(changes[0]?.kind).toBe("changed")
    expect(changes[0]?.pathRefs).toHaveLength(256)
    for (let index = 0; index < 10_000; index += 1) watcherCallbacks[0]?.(`src/overflow-${index}.ts`)
    scheduled.shift()?.()
    expect(changes.at(-1)).toEqual({ kind: "overflow", pathRef: null, epoch: 2 })
    subscription.close()
    workspace.dispose()
  })

  test("owns asynchronous search tasks, caches only current results, and cancels them on epoch change or close", async () => {
    const root = makeRoot()
    const started: Array<Readonly<{
      request: WorkspaceSearchRequest
      resolve: (page: ReturnType<typeof searchWorkspace>) => void
    }>> = []
    let cancelAlls = 0
    let disposals = 0
    let disposed = false
    const active = new Set<(page: ReturnType<typeof searchWorkspace>) => void>()
    const searchHost: WorkspaceSearchHost = {
      start: request => {
        if (disposed) {
          return {
            taskRef: "workspace.search.task.disposed",
            result: Promise.resolve({ state: "unavailable", message: "The selected workspace has been disposed." }),
            cancel: () => undefined,
          }
        }
        let resolveResult!: (page: ReturnType<typeof searchWorkspace>) => void
        const result = new Promise<ReturnType<typeof searchWorkspace>>(resolve => {
          resolveResult = page => {
            active.delete(resolveResult)
            resolve(page)
          }
        })
        active.add(resolveResult)
        started.push({ request, resolve: resolveResult })
        return {
          taskRef: `workspace.search.task.${started.length}`,
          result,
          cancel: () => resolveResult({ state: "unavailable", message: "Workspace search was cancelled." }),
        }
      },
      cancelAll: () => {
        cancelAlls += 1
        for (const resolve of [...active]) resolve({ state: "unavailable", message: "Workspace search was cancelled." })
      },
      activeCount: () => active.size,
      dispose: () => {
        if (disposed) return
        disposed = true
        disposals += 1
        for (const resolve of [...active]) resolve({ state: "unavailable", message: "The selected workspace has been disposed." })
      },
    }
    const workspace = openWorkspaceService(root, {
      grantRef: "workspace.grant.async",
      searchHostFactory: (selectedRoot, grantRef) => {
        expect(selectedRoot).toBe(root)
        expect(grantRef).toBe("workspace.grant.async")
        return searchHost
      },
    })

    const stale = workspace.search({ query: "needle", mode: "content", limit: 12 })
    expect(started[0]?.request).toEqual({ query: "needle", mode: "content", offset: 0, limit: 12, epoch: 0 })
    workspace.refresh()
    expect(await stale.result).toEqual({ state: "unavailable", message: "Workspace search was cancelled." })
    expect(cancelAlls).toBe(1)

    const current = workspace.search({ query: "needle", mode: "content", limit: 12 })
    const currentPage = searchWorkspace({
      root,
      grantRef: "workspace.grant.async",
      query: "needle",
      mode: "content",
      limit: 12,
      epoch: 1,
    })
    started[1]!.resolve(currentPage)
    expect(await current.result).toEqual(currentPage)
    const cached = workspace.search({ query: "needle", mode: "content", limit: 12 })
    expect(cached.taskRef.startsWith("workspace.search.cache.")).toBe(true)
    expect(await cached.result).toEqual(currentPage)
    expect(started).toHaveLength(2)

    const closing = workspace.search({ query: "other", mode: "path" })
    workspace.dispose()
    workspace.dispose()
    expect(await closing.result).toEqual({ state: "unavailable", message: "The selected workspace has been disposed." })
    expect(disposals).toBe(1)
    expect(searchHost.activeCount()).toBe(0)
  })

  test("paginates a large root behind the fixed tree bound", () => {
    const root = makeRoot()
    for (let index = 0; index < 250; index++) {
      writeFileSync(path.join(root, `file-${String(index).padStart(3, "0")}.txt`), "safe")
    }
    const first = workspaceTreePage({
      root,
      grantRef: "workspace.grant.large",
      directoryRef: "",
      limit: 200,
    })
    expect(first.state).toBe("available")
    if (first.state !== "available") throw new Error("expected first page")
    expect(first.entries).toHaveLength(200)
    expect(first.nextOffset).toBe(200)
    const second = workspaceTreePage({
      root,
      grantRef: "workspace.grant.large",
      directoryRef: "",
      offset: first.nextOffset ?? 0,
      limit: 200,
    })
    expect(second.state === "available" ? second.entries : []).toHaveLength(50)
    expect(second.state === "available" ? second.nextOffset : -1).toBeNull()
  })

  test("decodes only the fixed tree/search/watch request, page, and event shapes", () => {
    expect(decodeWorkspaceTreeRequest({ directoryRef: "src", offset: 0, limit: 80 })).toEqual({
      directoryRef: "src",
      offset: 0,
      limit: 80,
    })
    expect(decodeWorkspaceTreeRequest({ directoryRef: "src", extra: "ignored" })).toEqual({
      directoryRef: "src",
    })
    expect(decodeWorkspaceWatchRequest({ active: true })).toEqual({ active: true })
    expect(decodeWorkspaceWatchRequest({ active: "yes" })).toBeNull()
    expect(decodeWorkspaceChange({ kind: "overflow", pathRef: null, epoch: 2 })).toEqual({
      kind: "overflow",
      pathRef: null,
      epoch: 2,
    })
    expect(decodeWorkspaceChange({ kind: "changed", pathRef: null, epoch: "2" })).toBeNull()
    expect(decodeWorkspaceChange({
      kind: "changed", pathRef: null, pathRefs: ["README.md", "src/index.ts"], epoch: 3,
    })?.pathRefs).toEqual(["README.md", "src/index.ts"])
    expect(decodeWorkspaceChange({
      kind: "changed", pathRef: null,
      pathRefs: Array.from({ length: 257 }, (_, index) => `src/${index}.ts`), epoch: 3,
    })).toBeNull()
    expect(decodeWorkspaceChange({ kind: "changed", pathRef: null, pathRefs: ["../secret"], epoch: 3 })).toBeNull()
    expect(decodeWorkspaceChange({ kind: "changed", pathRef: null, pathRefs: ["/tmp/secret"], epoch: 3 })).toBeNull()
    const root = makeRoot()
    writeFileSync(path.join(root, "README.md"), "safe")
    const page = workspaceTreePage({
      root,
      grantRef: "workspace.grant.decode",
      directoryRef: "",
    })
    expect(decodeWorkspaceTreePage(page)).toEqual(page)
    expect(decodeWorkspaceTreePage({ state: "available", root })).toBeNull()
    expect(decodeWorkspaceSearchRequest({ query: "needle", mode: "content", limit: 40 })).toEqual({
      query: "needle",
      mode: "content",
      limit: 40,
    })
    const search = searchWorkspace({
      root,
      grantRef: "workspace.grant.decode",
      query: "readme",
      mode: "path",
    })
    expect(decodeWorkspaceSearchPage(search)).toEqual(search)
    expect(decodeWorkspaceSearchPage({ state: "available", root })).toBeNull()
    expect(decodeWorkspaceCreateRequest({ parentRef: "src", name: "new.ts", kind: "file", root })).toEqual({
      parentRef: "src",
      name: "new.ts",
      kind: "file",
    })
    expect(decodeWorkspaceRenameRequest({
      pathRef: "src/old.ts",
      name: "new.ts",
      expectedRevisionRef: "workspace.entry.fixture",
    })?.name).toBe("new.ts")
    expect(decodeWorkspaceDeleteRequest({
      pathRef: "src/new.ts",
      expectedRevisionRef: "workspace.entry.fixture",
    })?.pathRef).toBe("src/new.ts")
    expect(decodeWorkspaceRevealRequest({ pathRef: "src/new.ts" })).toEqual({ pathRef: "src/new.ts" })
    expect(decodeWorkspaceOperationResult({ state: "deleted", pathRef: "src/new.ts" })).toEqual({
      state: "deleted",
      pathRef: "src/new.ts",
    })
    expect(decodeWorkspaceOperationResult({ state: "deleted", pathRef: root })).toBeNull()
  })
})
