import { mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"
import { CODEX_HOST_METHODS, CodexHostServiceError, makeCodexHostServices } from "./codex-host-services.ts"

const roots: string[] = []
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-host-root-")); roots.push(root)
  const outside = mkdtempSync(join(tmpdir(), "oa-host-outside-")); roots.push(outside)
  const spoolRoot = join(root, ".spool"); const receiptPath = join(root, ".receipts", "host.json")
  writeFileSync(join(root, "small.txt"), "small")
  writeFileSync(join(root, "large.bin"), Buffer.alloc(4_096, 7))
  writeFileSync(join(outside, "secret.txt"), "secret")
  symlinkSync(outside, join(root, "escape"), process.platform === "win32" ? "junction" : "dir")
  const requests: Array<{ method: string; params: unknown }> = []
  const notifications = new Set<(value: CodexAppServerNotification) => void>()
  let generation = 1
  const commandResolvers = new Map<string, (value: unknown) => void>()
  const lease = {
    state: () => ({ status: "ready" as const, generation }),
    request: async (method: string, params: unknown) => {
      requests.push({ method, params }); const row = params as Record<string, unknown>
      if (method === "fs/readFile") return { dataBase64: readFileSync(row.path as string).toString("base64") }
      if (method === "fs/readDirectory") return { entries: [] }
      if (method === "fs/getMetadata") return { isFile: true, isDirectory: false, isSymlink: false, createdAtMs: 0, modifiedAtMs: 0 }
      if (method === "command/exec") return new Promise(resolve => commandResolvers.set(row.processId as string, resolve))
      if (method === "fuzzyFileSearch") return { files: [{ path: join(root, "small.txt"), root, score: 1, match_type: "exact" }, { path: join(outside, "secret.txt"), root: outside, score: 1, match_type: "exact" }] }
      if (method === "externalAgentConfig/import") return { importId: "import-1" }
      if (method === "windowsSandbox/readiness") return { status: "ready" }
      if (method === "feedback/upload") return { threadId: "feedback-1" }
      return {}
    },
    subscribe: (listener: (value: CodexAppServerNotification) => void) => { notifications.add(listener); return () => notifications.delete(listener) },
    release: () => undefined,
  } as unknown as CodexAppServerLease
  return {
    root, outside, spoolRoot, receiptPath, lease, requests, commandResolvers,
    notify: (method: string, params: unknown, nextGeneration = generation) => { generation = nextGeneration; for (const listener of notifications) listener({ generation, message: { method, params } }) },
  }
}

describe("Codex bounded host services", () => {
  test("gives every generated host method/event an explicit capability policy", () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot })
    expect(Object.keys(service.snapshot().policies).sort()).toEqual(CODEX_HOST_METHODS.map(value => value.method).sort())
    expect(service.snapshot().policies["feedback/upload"]).toBe("review_required")
    expect(service.snapshot().policies["windowsSandbox/setupStart"]).toBe(process.platform === "win32" ? "available" : "blocked_platform")
    service.close()
  })

  test("fails path, symlink, absolute, and cross-workspace escapes closed", async () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot })
    await expect(service.readFile("../secret")).rejects.toMatchObject({ reason: "path_escape" })
    await expect(service.readFile(h.outside)).rejects.toMatchObject({ reason: "path_escape" })
    await expect(service.readFile("escape/secret.txt")).rejects.toMatchObject({ reason: "symlink_escape" })
    const payload = { relativePath: "escape/new.txt", recursive: true }
    const authority = service.authorize("fs_mutation", payload, service.snapshot().revision)
    await expect(service.createDirectory(payload.relativePath, payload.recursive, authority)).rejects.toMatchObject({ reason: "symlink_escape" })
    await expect(service.createDirectory("safe", true, authority)).rejects.toMatchObject({ reason: "authority_required" })
    service.close()
  })

  test("bounds and privately spools large reads with mode-0600 receipts", async () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot, receiptPath: h.receiptPath, maxReadBytes: 1_024 })
    const read = await service.readFile("large.bin")
    expect(read).toMatchObject({ spooled: true, spoolRef: expect.stringMatching(/^[a-f0-9]{32}$/) })
    expect(Buffer.from(read.dataBase64, "base64")).toHaveLength(1_024)
    expect(statSync(h.receiptPath).mode & 0o777).toBe(0o600)
    expect(readFileSync(h.receiptPath, "utf8")).not.toContain(h.root)
    expect(readFileSync(h.receiptPath, "utf8")).not.toContain("small")
    service.close()
  })

  test("owns watches by connection generation and makes overflow/disconnect explicit", async () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot })
    const watchId = await service.watch(".")
    h.notify("fs/changed", { watchId, changedPaths: [join(h.root, "small.txt"), join(h.outside, "secret.txt")] })
    expect(service.snapshot().watches[0]).toMatchObject({ state: "active", changes: ["small.txt"] })
    h.notify("fs/changed", { watchId, changedPaths: [] }, 2)
    expect(service.snapshot().watches[0]?.state).toBe("disconnected")
    await expect(service.unwatch("missing")).rejects.toBeInstanceOf(CodexHostServiceError)
    service.close()
  })

  test("streams bounded command output and settles resize/termination exactly once", async () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot, maxReadBytes: 1_024, maxStreamBytes: 2_048 })
    const input = { command: ["printf", "ok"], cwd: ".", tty: true, rows: 24, cols: 80 }
    const authority = service.authorize("command", input, service.snapshot().revision)
    const processId = await service.exec(input, authority)
    h.notify("command/exec/outputDelta", { processId, stream: "stdout", deltaBase64: Buffer.alloc(4_096, 65).toString("base64"), capReached: false })
    expect(service.snapshot().commands[0]).toMatchObject({ state: "running", capReached: true, spoolRef: expect.stringMatching(/^[a-f0-9]{32}$/) })
    await service.resizeCommand(processId, 9_999, 9_999)
    await service.writeCommand(processId, Buffer.from("x").toString("base64"))
    await service.terminateCommand(processId)
    h.commandResolvers.get(processId)?.({ exitCode: 0, stdout: "late", stderr: "" })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(service.snapshot().commands[0]?.state).toBe("terminated")
    await expect(service.terminateCommand(processId)).rejects.toMatchObject({ reason: "already_settled" })
    expect(h.requests.find(value => value.method === "command/exec/resize")?.params).toMatchObject({ size: { rows: 200, cols: 500 } })
    service.close()
  })

  test("bounds fuzzy search, owns stateful sessions, and makes interrupted imports restartable", async () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot })
    expect(await service.fuzzySearch("small")).toEqual(["small.txt"])
    const sessionId = await service.startSearch(); await service.updateSearch(sessionId, "small")
    h.notify("fuzzyFileSearch/sessionUpdated", { sessionId, query: "small", files: [{ path: join(h.root, "small.txt"), root: h.root, score: 1, match_type: "exact" }] })
    expect(service.snapshot().searches[0]).toMatchObject({ query: "small", files: ["small.txt"] })
    h.notify("fuzzyFileSearch/sessionCompleted", { sessionId }); await expect(service.stopSearch(sessionId)).rejects.toMatchObject({ reason: "already_settled" })
    const migrationItems = [{ itemType: "skills" }]; const payload = { migrationItems, source: "claude" }
    const authority = service.authorize("external_import", payload, service.snapshot().revision)
    expect(await service.importExternalConfig(migrationItems, "claude", authority)).toBe("import-1")
    h.notify("fs/changed", { changedPaths: [] }, 2)
    expect(service.snapshot().imports[0]?.state).toBe("disconnected")
    const restarted = service.authorize("external_import", payload, service.snapshot().revision)
    expect(await service.importExternalConfig(migrationItems, "claude", restarted)).toBe("import-1")
    h.notify("externalAgentConfig/import/completed", { importId: "import-1", itemTypeResults: [{ itemType: "skills", successes: [{ name: "one" }], failures: [{ message: "bad" }] }] })
    expect(service.snapshot().imports[0]).toMatchObject({ state: "failed", successes: 1, failures: 1 })
    service.close()
  })

  test("requires reviewed feedback attachments and gates Windows setup by host", async () => {
    const h = fixture(); const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot })
    const input = { classification: "bug", reason: "bounded", attachments: ["small.txt"], includeLogs: false }
    const authority = service.authorize("feedback", input, service.snapshot().revision)
    await service.uploadFeedback(input, authority)
    expect(h.requests.find(value => value.method === "feedback/upload")?.params).toMatchObject({ extraLogFiles: [realpathSync(resolve(h.root, "small.txt"))], tags: { reviewed: "true" } })
    if (process.platform === "win32") await expect(service.windowsReadiness()).resolves.toEqual({ status: "ready" })
    else await expect(service.windowsReadiness()).rejects.toMatchObject({ reason: "blocked_platform" })
    service.close()

    const win = fixture(); const windows = makeCodexHostServices({ lease: win.lease, workspaceRoot: win.root, spoolRoot: win.spoolRoot, platform: "win32" })
    await expect(windows.windowsReadiness()).resolves.toEqual({ status: "ready" })
    const setup = windows.authorize("windows_setup", { mode: "elevated" }, windows.snapshot().revision)
    await windows.startWindowsSetup("elevated", setup)
    win.notify("windowsSandbox/setupCompleted", { mode: "elevated", success: false, error: "private" })
    expect(windows.snapshot().windowsSandbox).toEqual({ state: "failed", mode: "elevated" })
    windows.close()
  })
})
