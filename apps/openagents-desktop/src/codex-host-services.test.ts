import { mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexAppServerLease, CodexAppServerNotification, CodexAppServerPoolTarget, CodexAppServerSupervisor } from "./codex-app-server-supervisor.ts"
import { CODEX_HOST_METHODS, CodexHostServiceError, makeCodexHostServiceRegistry, makeCodexHostServices } from "./codex-host-services.ts"
import type { IdePortableMutationAuthority, IdePortableMutationPermit } from "./ide/portable-mutation-authority.ts"

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
  let releases = 0
  const commandResolvers = new Map<string, (value: unknown) => void>()
  const blockers = new Map<string, { ignoreAbort: boolean; started: () => void; aborted: () => void; complete: (value: unknown) => void; fail: (error: unknown) => void }>()
  const lease = {
    state: () => ({ status: "ready" as const, generation }),
    request: async (method: string, params: unknown, options?: Readonly<{ signal?: AbortSignal }>) => {
      requests.push({ method, params }); const row = params as Record<string, unknown>
      const blocker = blockers.get(method)
      if (blocker !== undefined) {
        blockers.delete(method); blocker.started()
        return new Promise((resolveRequest, rejectRequest) => {
          const abort = () => { blocker.aborted(); if (!blocker.ignoreAbort) rejectRequest(new DOMException("Aborted", "AbortError")) }
          options?.signal?.addEventListener("abort", abort, { once: true })
          blocker.complete = value => { options?.signal?.removeEventListener("abort", abort); resolveRequest(value) }
          blocker.fail = error => { options?.signal?.removeEventListener("abort", abort); rejectRequest(error) }
        })
      }
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
    release: () => { releases += 1 },
  } as unknown as CodexAppServerLease
  return {
    root, outside, spoolRoot, receiptPath, lease, requests, commandResolvers, releases: () => releases,
    block: (method: string, ignoreAbort = false) => {
      let started!: () => void; let aborted!: () => void
      const didStart = new Promise<void>(resolveStarted => { started = resolveStarted })
      const didAbort = new Promise<void>(resolveAborted => { aborted = resolveAborted })
      const blocker = { ignoreAbort, started, aborted, complete: (_value: unknown) => undefined, fail: (_error: unknown) => undefined }
      blockers.set(method, blocker)
      return { started: didStart, aborted: didAbort, complete: (value: unknown) => blocker.complete(value), fail: (error: unknown) => blocker.fail(error) }
    },
    notify: (method: string, params: unknown, nextGeneration = generation) => { generation = nextGeneration; for (const listener of notifications) listener({ generation, message: { method, params } }) },
  }
}

const target: CodexAppServerPoolTarget = { binary: "/test/codex", binarySha256: "test", env: {}, cwd: "/tmp", accountRef: null, hostTarget: "local" }

const portableAuthority = () => {
  let live = true
  const permit: IdePortableMutationPermit = { _tag: "Portable", key: "portable:grant-1:attachment-1:7", grantRef: "grant-1", sessionRef: "session-1", workContextRef: "work-1", attachmentRef: "attachment-1", generation: 7, targetRef: "target-1" }
  const authority: IdePortableMutationAuthority = {
    authorize: grantRef => live && grantRef === permit.grantRef ? { _tag: "Permitted", permit } : { _tag: "Refused", reason: "attachment_ambiguous" },
    reauthorize: candidate => live && candidate.key === permit.key,
  }
  return { authority, revoke: () => { live = false } }
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

  test("withholds a blocked file-mutation result after portable revocation and keeps the host quiesced", async () => {
    const h = fixture(); const portable = portableAuthority(); const blocked = h.block("fs/writeFile", true)
    const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot, receiptPath: h.receiptPath, workspaceGrantRef: "grant-1", mutationAuthority: portable.authority, revocationPollMs: 1 })
    const payload = { relativePath: "new.txt", dataBase64: Buffer.from("new").toString("base64") }
    const reviewed = service.authorize("fs_mutation", payload, service.snapshot().revision)
    const write = service.writeFile(payload.relativePath, payload.dataBase64, reviewed); await blocked.started
    portable.revoke(); await blocked.aborted
    let quiesced = false; const quiesce = service.quiesce().then(() => { quiesced = true }); await Promise.resolve(); expect(quiesced).toBe(false)
    blocked.complete({}); await expect(write).rejects.toMatchObject({ reason: "grant_revoked" }); await quiesce
    expect(service.receipts().some(receipt => receipt.method === "fs/writeFile")).toBe(false)
    await expect(service.readDirectory(".")).rejects.toMatchObject({ reason: "closed" })
    const disposal = service.dispose(); expect(service.dispose()).toBe(disposal); await disposal
  })

  test("terminates a blocked command and suppresses its late result after portable revocation", async () => {
    const h = fixture(); const portable = portableAuthority(); const blocked = h.block("command/exec", true)
    const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot, workspaceGrantRef: "grant-1", mutationAuthority: portable.authority, revocationPollMs: 1 })
    const input = { command: ["node", "mutate.mjs"], cwd: "." }; const reviewed = service.authorize("command", input, service.snapshot().revision)
    const processId = await service.exec(input, reviewed); await blocked.started
    portable.revoke(); await blocked.aborted
    let quiesced = false; const quiesce = service.quiesce().then(() => { quiesced = true }); await Promise.resolve(); expect(quiesced).toBe(false)
    expect(service.snapshot().commands).toContainEqual(expect.objectContaining({ processId, state: "disconnected" }))
    expect(h.requests).toContainEqual({ method: "command/exec/terminate", params: { processId } })
    blocked.complete({ exitCode: 0, stdout: "late", stderr: "" }); await quiesce
    expect(service.snapshot().commands).toContainEqual(expect.objectContaining({ processId, state: "disconnected", stdoutPreview: "" }))
    expect(service.receipts().some(receipt => receipt.method === "command/exec")).toBe(false)
    await service.dispose()
  })

  test("withholds a read result from a stale app-server generation", async () => {
    const h = fixture(); const portable = portableAuthority(); const blocked = h.block("fs/readDirectory", true)
    const service = makeCodexHostServices({ lease: h.lease, workspaceRoot: h.root, spoolRoot: h.spoolRoot, workspaceGrantRef: "grant-1", mutationAuthority: portable.authority, revocationPollMs: 1 })
    const read = service.readDirectory("."); await blocked.started
    h.notify("fs/changed", { changedPaths: [] }, 2); await blocked.aborted; blocked.complete({ entries: ["late"] })
    await expect(read).rejects.toMatchObject({ reason: "grant_revoked" })
    expect(service.receipts().some(receipt => receipt.method === "fs/readDirectory")).toBe(false)
    await service.dispose()
  })
})

describe("Codex host service registry quiescence", () => {
  test("awaits a blocked child, suppresses its late result, and stays closed", async () => {
    const h = fixture(); const supervisor = { acquire: async () => h.lease } as unknown as CodexAppServerSupervisor
    const registry = makeCodexHostServiceRegistry({ supervisor, spoolRoot: join(h.root, "registry-spool"), receiptRoot: join(h.root, "registry-receipts"), quiesceTimeoutMs: 1_000 })
    const service = await registry.forTarget(target, h.root); const blocked = h.block("fs/readDirectory", true); const read = service.readDirectory("."); await blocked.started
    let completed = false; const first = registry.quiesce().then(result => { completed = true; return result }); await blocked.aborted; expect(completed).toBe(false)
    blocked.complete({ entries: ["late"] }); await expect(read).rejects.toMatchObject({ reason: "grant_revoked" })
    await expect(first).resolves.toEqual({ state: "quiesced" }); await expect(registry.quiesce()).resolves.toEqual({ state: "quiesced" })
    await expect(registry.forTarget(target, h.root)).rejects.toMatchObject({ reason: "closed" }); expect(h.releases()).toBe(1)
  })

  test("reports a blocked acquisition as timed out and disposes a late instance", async () => {
    const h = fixture(); let resolveAcquire!: (lease: CodexAppServerLease) => void
    const pending = new Promise<CodexAppServerLease>(resolve => { resolveAcquire = resolve }); const supervisor = { acquire: () => pending } as unknown as CodexAppServerSupervisor
    const registry = makeCodexHostServiceRegistry({ supervisor, spoolRoot: join(h.root, "registry-spool"), receiptRoot: join(h.root, "registry-receipts"), quiesceTimeoutMs: 10 })
    const late = registry.forTarget(target, h.root)
    await expect(registry.quiesce()).resolves.toEqual({ state: "timed_out", detailRef: "desktop.codex-host-services.registry-cleanup-timeout" })
    resolveAcquire(h.lease); const service = await late
    for (let index = 0; index < 20 && h.releases() === 0; index += 1) await Promise.resolve()
    expect(h.releases()).toBe(1); await expect(service.readDirectory(".")).rejects.toMatchObject({ reason: "closed" })
  })
})
