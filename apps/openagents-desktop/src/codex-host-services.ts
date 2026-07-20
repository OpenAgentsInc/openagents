import { createHash, randomUUID } from "node:crypto"
import { appendFileSync, existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, renameSync, statSync, writeFileSync } from "node:fs"
import { dirname, isAbsolute, relative, resolve } from "node:path"

import { bundledCodex01441ProtocolManifest } from "@openagentsinc/codex-app-server-protocol/parity"

import { codexAppServerPoolKey, type CodexAppServerLease, type CodexAppServerNotification, type CodexAppServerPoolTarget, type CodexAppServerSupervisor } from "./codex-app-server-supervisor.ts"
import type { IdePortableMutationAuthority, IdePortableMutationPermit } from "./ide/portable-mutation-authority.ts"

type ObjectValue = Readonly<Record<string, unknown>>
const object = (value: unknown): ObjectValue | null => typeof value === "object" && value !== null && !Array.isArray(value) ? value as ObjectValue : null
const array = (value: unknown): ReadonlyArray<unknown> => Array.isArray(value) ? value : []
const string = (value: unknown): string | null => typeof value === "string" ? value : null
const number = (value: unknown): number | null => typeof value === "number" && Number.isFinite(value) ? value : null
const hash = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex")
const bounded = (value: string, limit: number): string => value.length <= limit ? value : `${value.slice(0, Math.max(0, limit - 1))}…`

export const CODEX_HOST_METHODS = Object.freeze(bundledCodex01441ProtocolManifest.members
  .filter(member => /^(?:fs\/|command\/|fuzzyFileSearch|externalAgentConfig\/|windows(?:Sandbox|\/)|feedback\/)/u.test(member.method))
  .map(member => ({ method: member.method, direction: member.direction })))

export type CodexHostPolicy = "available" | "blocked_platform" | "review_required"
export type CodexHostReceipt = Readonly<{ method: string; causalHash: string; outcome: "accepted" | "failed" | "blocked"; generation: number; observedAt: string }>
export type CodexHostSnapshot = Readonly<{
  revision: number
  generation: number
  watches: ReadonlyArray<Readonly<{ watchId: string; rootRelativePath: string; state: "active" | "overflow" | "disconnected" | "stopped"; changes: ReadonlyArray<string> }>>
  commands: ReadonlyArray<Readonly<{ processId: string; state: "running" | "settled" | "terminated" | "disconnected"; exitCode: number | null; stdoutPreview: string; stderrPreview: string; spoolRef: string; capReached: boolean }>>
  searches: ReadonlyArray<Readonly<{ sessionId: string; state: "active" | "completed" | "stopped" | "disconnected"; query: string; files: ReadonlyArray<string> }>>
  imports: ReadonlyArray<Readonly<{ importId: string; state: "running" | "completed" | "failed" | "disconnected"; successes: number; failures: number }>>
  windowsSandbox: Readonly<{ state: "unsupported" | "not_ready" | "setting_up" | "ready" | "failed"; mode: string | null }>
  policies: Readonly<Record<string, CodexHostPolicy>>
}>

export type CodexHostAuthorityKind = "fs_mutation" | "command" | "external_import" | "windows_setup" | "feedback"
export type CodexHostAuthority = Readonly<{ token: string; kind: CodexHostAuthorityKind; expectedRevision: number; payloadHash: string; expiresAt: string }>

export class CodexHostServiceError extends Error {
  readonly _tag = "CodexHostServiceError"
  override readonly name = "CodexHostServiceError"
  constructor(readonly reason: "closed" | "path_escape" | "symlink_escape" | "authority_required" | "grant_revoked" | "stale" | "expired" | "reused" | "oversized" | "not_found" | "already_settled" | "blocked_platform" | "partial_failure", message: string) { super(message) }
}

export type CodexHostServices = Readonly<{
  snapshot: () => CodexHostSnapshot
  subscribe: (listener: (snapshot: CodexHostSnapshot) => void) => () => void
  authorize: (kind: CodexHostAuthorityKind, payload: unknown, expectedRevision: number) => CodexHostAuthority
  readFile: (relativePath: string) => Promise<Readonly<{ dataBase64: string; spooled: boolean; spoolRef: string | null }>>
  writeFile: (relativePath: string, dataBase64: string, authority: CodexHostAuthority) => Promise<void>
  createDirectory: (relativePath: string, recursive: boolean, authority: CodexHostAuthority) => Promise<void>
  readDirectory: (relativePath: string) => Promise<unknown>
  metadata: (relativePath: string) => Promise<unknown>
  remove: (relativePath: string, recursive: boolean, authority: CodexHostAuthority) => Promise<void>
  copy: (sourceRelativePath: string, destinationRelativePath: string, recursive: boolean, authority: CodexHostAuthority) => Promise<void>
  watch: (relativePath: string) => Promise<string>
  unwatch: (watchId: string) => Promise<void>
  exec: (input: Readonly<{ command: ReadonlyArray<string>; cwd?: string; timeoutMs?: number; tty?: boolean; rows?: number; cols?: number }>, authority: CodexHostAuthority) => Promise<string>
  writeCommand: (processId: string, deltaBase64: string, closeStdin?: boolean) => Promise<void>
  resizeCommand: (processId: string, rows: number, cols: number) => Promise<void>
  terminateCommand: (processId: string) => Promise<void>
  fuzzySearch: (query: string) => Promise<ReadonlyArray<string>>
  startSearch: () => Promise<string>
  updateSearch: (sessionId: string, query: string) => Promise<void>
  stopSearch: (sessionId: string) => Promise<void>
  detectExternalConfig: () => Promise<unknown>
  readExternalHistories: () => Promise<unknown>
  importExternalConfig: (migrationItems: ReadonlyArray<unknown>, source: string | null, authority: CodexHostAuthority) => Promise<string>
  windowsReadiness: () => Promise<unknown>
  startWindowsSetup: (mode: string, authority: CodexHostAuthority) => Promise<void>
  uploadFeedback: (input: Readonly<{ classification: string; reason: string | null; attachments: ReadonlyArray<string>; includeLogs: boolean }>, authority: CodexHostAuthority) => Promise<void>
  receipts: () => ReadonlyArray<CodexHostReceipt>
  quiesce: () => Promise<void>
  dispose: () => Promise<void>
  close: () => void
}>

export type CodexHostServiceRegistry = Readonly<{
  forTarget: (
    target: CodexAppServerPoolTarget,
    workspaceRoot: string,
    portable?: Readonly<{
      workspaceGrantRef: string
      mutationAuthority: IdePortableMutationAuthority
    }>,
  ) => Promise<CodexHostServices>
  quiesce: () => Promise<CodexHostRegistryQuiescence>
  close: () => void
}>

export type CodexHostRegistryQuiescence =
  | Readonly<{ state: "quiesced" }>
  | Readonly<{ state: "timed_out" | "failed"; detailRef: string }>

const policies = (platform: NodeJS.Platform): Readonly<Record<string, CodexHostPolicy>> => Object.fromEntries(CODEX_HOST_METHODS.map(member => [
  member.method,
  member.method.startsWith("windows") && platform !== "win32" ? "blocked_platform" : member.method === "feedback/upload" || member.method === "externalAgentConfig/import" ? "review_required" : "available",
]))

export const makeCodexHostServices = (options: Readonly<{
  lease: CodexAppServerLease
  workspaceRoot: string
  spoolRoot: string
  receiptPath?: string
  platform?: NodeJS.Platform
  now?: () => Date
  authorityTtlMs?: number
  maxReadBytes?: number
  maxStreamBytes?: number
  workspaceGrantRef?: string
  mutationAuthority?: IdePortableMutationAuthority
  revocationPollMs?: number
  quiesceTimeoutMs?: number
}>): CodexHostServices => {
  const root = realpathSync(options.workspaceRoot)
  const spoolRoot = resolve(options.spoolRoot); mkdirSync(spoolRoot, { recursive: true, mode: 0o700 })
  const platform = options.platform ?? process.platform
  const now = () => options.now?.() ?? new Date()
  const maxReadBytes = Math.max(1_024, options.maxReadBytes ?? 1_048_576)
  const maxStreamBytes = Math.max(maxReadBytes, options.maxStreamBytes ?? 33_554_432)
  let closed = false
  let revision = 0
  let generation = options.lease.state().generation
  const listeners = new Set<(snapshot: CodexHostSnapshot) => void>()
  const authorities = new Map<string, CodexHostAuthority>()
  const watches = new Map<string, CodexHostSnapshot["watches"][number]>()
  const commands = new Map<string, CodexHostSnapshot["commands"][number] & { bytes: number }>()
  const searches = new Map<string, CodexHostSnapshot["searches"][number]>()
  const imports = new Map<string, CodexHostSnapshot["imports"][number]>()
  const watchPermits = new Map<string, IdePortableMutationPermit | null>()
  const commandPermits = new Map<string, IdePortableMutationPermit | null>()
  const searchPermits = new Map<string, IdePortableMutationPermit | null>()
  const importPermits = new Map<string, IdePortableMutationPermit | null>()
  const activeRequests = new Set<Readonly<{ controller: AbortController; settled: Promise<void> }>>()
  const cleanupRequests = new Set<Promise<void>>()
  let windowsPermit: IdePortableMutationPermit | null = null
  let revocationInProgress = false
  let quiescing = false
  let disposePromise: Promise<void> | null = null
  let windowsSandbox: CodexHostSnapshot["windowsSandbox"] = { state: platform === "win32" ? "not_ready" : "unsupported", mode: null }
  const readReceipts = (): CodexHostReceipt[] => {
    if (options.receiptPath === undefined) return []
    try { const parsed = JSON.parse(readFileSync(options.receiptPath, "utf8")); return Array.isArray(parsed.receipts) ? parsed.receipts : [] } catch { return [] }
  }
  const receiptLog = readReceipts()
  const persistReceipts = () => {
    if (options.receiptPath === undefined) return
    mkdirSync(dirname(options.receiptPath), { recursive: true, mode: 0o700 }); const temp = `${options.receiptPath}.tmp`
    writeFileSync(temp, `${JSON.stringify({ schema: "openagents.desktop.codex_host_receipts.v1", receipts: receiptLog.slice(-4_096) })}\n`, { mode: 0o600 }); renameSync(temp, options.receiptPath)
  }
  const snapshot = (): CodexHostSnapshot => ({ revision, generation, watches: [...watches.values()], commands: [...commands.values()].map(({ bytes: _, ...command }) => command), searches: [...searches.values()], imports: [...imports.values()], windowsSandbox, policies: policies(platform) })
  const publish = () => { revision += 1; const value = snapshot(); for (const listener of listeners) listener(value) }
  const record = (method: string, causal: unknown, outcome: CodexHostReceipt["outcome"]) => { receiptLog.push({ method, causalHash: hash(JSON.stringify(causal)), outcome, generation, observedAt: now().toISOString() }); persistReceipts() }
  const assertOpen = () => { if (closed || quiescing) throw new CodexHostServiceError("closed", "Codex host services are closed or quiescing") }
  if ((options.mutationAuthority === undefined) !== (options.workspaceGrantRef === undefined)) throw new CodexHostServiceError("authority_required", "Portable mutation authority and workspace grant must be configured together")
  const capturePermit = (): IdePortableMutationPermit | null => {
    assertOpen()
    if (options.mutationAuthority === undefined || options.workspaceGrantRef === undefined) return null
    const authorized = options.mutationAuthority.authorize(options.workspaceGrantRef)
    if (authorized._tag === "Refused") throw new CodexHostServiceError("grant_revoked", `Portable mutation authority is unavailable (${authorized.reason})`)
    if (!options.mutationAuthority.reauthorize(authorized.permit)) throw new CodexHostServiceError("grant_revoked", "Portable mutation authority changed before dispatch")
    return authorized.permit
  }
  const permitCurrent = (permit: IdePortableMutationPermit | null): boolean => permit === null ? options.mutationAuthority === undefined : options.mutationAuthority?.reauthorize(permit) === true
  const assertPermit = (permit: IdePortableMutationPermit | null, operationGeneration: number): void => {
    if (closed || quiescing || !permitCurrent(permit) || options.lease.state().generation !== operationGeneration) throw new CodexHostServiceError("grant_revoked", "Portable mutation authority changed during the host operation")
  }
  const trackCleanup = (request: Promise<unknown>): void => {
    const settled = request.then(() => undefined, () => undefined); cleanupRequests.add(settled); void settled.finally(() => cleanupRequests.delete(settled))
  }
  const stopOwnedResources = (): void => {
    let changed = false
    for (const [watchId, value] of watches) if (value.state === "active") { watches.set(watchId, { ...value, state: "disconnected" }); trackCleanup(options.lease.request("fs/unwatch", { watchId })); changed = true }
    for (const [processId, value] of commands) if (value.state === "running") { commands.set(processId, { ...value, state: "disconnected" }); trackCleanup(options.lease.request("command/exec/terminate", { processId })); changed = true }
    for (const [sessionId, value] of searches) if (value.state === "active") { searches.set(sessionId, { ...value, state: "disconnected" }); trackCleanup(options.lease.request("fuzzyFileSearch/sessionStop", { sessionId })); changed = true }
    for (const [importId, value] of imports) if (value.state === "running") { imports.set(importId, { ...value, state: "disconnected" }); changed = true }
    if (windowsSandbox.state === "setting_up") { windowsSandbox = { ...windowsSandbox, state: "failed" }; changed = true }
    if (changed) publish()
  }
  const revokeActiveOperations = (): void => {
    if (revocationInProgress) return
    revocationInProgress = true
    for (const operation of activeRequests) operation.controller.abort()
    stopOwnedResources()
    if (activeRequests.size === 0) revocationInProgress = false
  }
  const requestWithPermit = async (method: string, params: unknown, permit: IdePortableMutationPermit | null): Promise<unknown> => {
    assertOpen(); const operationGeneration = options.lease.state().generation; assertPermit(permit, operationGeneration)
    const controller = new AbortController(); let complete!: () => void
    const settled = new Promise<void>(resolveSettled => { complete = resolveSettled }); const active = { controller, settled }; activeRequests.add(active)
    const poll = setInterval(() => { if (!permitCurrent(permit) || options.lease.state().generation !== operationGeneration) revokeActiveOperations() }, Math.max(1, options.revocationPollMs ?? 10)); poll.unref?.()
    try { const result = await options.lease.request(method, params, { signal: controller.signal }); assertPermit(permit, operationGeneration); return result }
    finally { clearInterval(poll); activeRequests.delete(active); complete(); if (activeRequests.size === 0) revocationInProgress = false }
  }
  const within = (candidate: string) => { const rel = relative(root, candidate); return rel === "" || (!rel.startsWith("..") && !isAbsolute(rel)) }
  const resolvePath = (input: string, mayCreate = false): string => {
    assertOpen(); if (input === "" || isAbsolute(input)) throw new CodexHostServiceError("path_escape", "Only WorkContext-relative paths are accepted")
    const candidate = resolve(root, input); if (!within(candidate)) throw new CodexHostServiceError("path_escape", "Path escapes WorkContext")
    let existing = candidate
    while (!existsSync(existing) && mayCreate && existing !== root) existing = dirname(existing)
    if (!existsSync(existing)) throw new CodexHostServiceError("not_found", "Path does not exist")
    const canonical = realpathSync(existing); if (!within(canonical)) throw new CodexHostServiceError("symlink_escape", "Canonical path escapes WorkContext")
    if (!mayCreate && canonical !== candidate && lstatSync(candidate).isSymbolicLink() && !within(canonical)) throw new CodexHostServiceError("symlink_escape", "Symlink escapes WorkContext")
    return candidate
  }
  const consume = (kind: CodexHostAuthorityKind, payload: unknown, authority: CodexHostAuthority) => {
    const stored = authorities.get(authority.token); if (stored === undefined) throw new CodexHostServiceError("authority_required", "Reviewed authority is required")
    authorities.delete(authority.token)
    if (stored.kind !== kind || stored.payloadHash !== hash(JSON.stringify(payload))) throw new CodexHostServiceError("authority_required", "Authority does not match this operation")
    if (stored.expectedRevision !== revision) throw new CodexHostServiceError("stale", "Host state changed")
    if (Date.parse(stored.expiresAt) < now().getTime()) throw new CodexHostServiceError("expired", "Authority expired")
  }
  const spool = (ref: string, bytes: Buffer) => { const path = resolve(spoolRoot, `${hash(ref)}.bin`); appendFileSync(path, bytes, { mode: 0o600 }); return hash(path).slice(0, 32) }
  const relativeResult = (pathValue: unknown): string | null => {
    const value = string(pathValue); if (value === null) return null
    try { const canonical = realpathSync(value); return within(canonical) ? relative(root, canonical) || "." : null } catch { return null }
  }
  const onNotification = ({ generation: nextGeneration, message }: CodexAppServerNotification) => {
    if (nextGeneration !== generation) {
      generation = nextGeneration
      revokeActiveOperations()
      return
    }
    const params = object(message.params) ?? {}
    if (message.method === "fs/changed") {
      const watchId = string(params.watchId); const current = watchId === null ? undefined : watches.get(watchId); if (watchId === null || current === undefined || current.state !== "active") return
      if (!permitCurrent(watchPermits.get(watchId) ?? null)) { revokeActiveOperations(); return }
      const changes = array(params.changedPaths).map(relativeResult).filter((value): value is string => value !== null).slice(0, 1_024)
      watches.set(watchId, { ...current, changes: [...current.changes, ...changes].slice(-1_024), state: array(params.changedPaths).length > 1_024 ? "overflow" : "active" }); record(message.method, { watchId, count: changes.length }, "accepted"); publish(); return
    }
    if (message.method === "command/exec/outputDelta") {
      const processId = string(params.processId); const current = processId === null ? undefined : commands.get(processId); if (processId === null || current === undefined || current.state !== "running") return
      if (!permitCurrent(commandPermits.get(processId) ?? null)) { revokeActiveOperations(); return }
      const bytes = Buffer.from(string(params.deltaBase64) ?? "", "base64"); const allowed = bytes.subarray(0, Math.max(0, maxStreamBytes - current.bytes)); const spoolRef = spool(`command:${processId}`, allowed)
      const text = allowed.toString("utf8"); const stream = params.stream === "stderr" ? "stderr" : "stdout"
      commands.set(processId, { ...current, bytes: current.bytes + allowed.length, spoolRef, capReached: params.capReached === true || allowed.length < bytes.length || current.bytes + allowed.length >= maxStreamBytes, ...(stream === "stdout" ? { stdoutPreview: bounded(current.stdoutPreview + text, 65_536) } : { stderrPreview: bounded(current.stderrPreview + text, 65_536) }) }); record(message.method, { processId, stream, bytes: allowed.length }, "accepted"); publish(); return
    }
    if (message.method === "fuzzyFileSearch/sessionUpdated") {
      const sessionId = string(params.sessionId); const current = sessionId === null ? undefined : searches.get(sessionId); if (sessionId === null || current === undefined || current.state !== "active") return
      if (!permitCurrent(searchPermits.get(sessionId) ?? null)) { revokeActiveOperations(); return }
      searches.set(sessionId, { ...current, query: string(params.query) ?? current.query, files: array(params.files).map(value => relativeResult(object(value)?.path)).filter((value): value is string => value !== null).slice(0, 1_000) }); record(message.method, { sessionId }, "accepted"); publish(); return
    }
    if (message.method === "fuzzyFileSearch/sessionCompleted") { const sessionId = string(params.sessionId); const current = sessionId === null ? undefined : searches.get(sessionId); if (sessionId !== null && current !== undefined && permitCurrent(searchPermits.get(sessionId) ?? null)) { searches.set(sessionId, { ...current, state: "completed" }); publish() }; return }
    if (message.method === "externalAgentConfig/import/progress" || message.method === "externalAgentConfig/import/completed") {
      const importId = string(params.importId); if (importId === null) return
      if (!permitCurrent(importPermits.get(importId) ?? null)) { revokeActiveOperations(); return }
      const results = array(params.itemTypeResults); const successes = results.reduce<number>((sum, value) => sum + array(object(value)?.successes).length, 0); const failures = results.reduce<number>((sum, value) => sum + array(object(value)?.failures).length, 0)
      imports.set(importId, { importId, state: message.method.endsWith("completed") ? failures > 0 ? "failed" : "completed" : "running", successes, failures }); record(message.method, { importId, successes, failures }, failures > 0 ? "failed" : "accepted"); publish(); return
    }
    if (message.method === "windows/worldWritableWarning") { if (!permitCurrent(windowsPermit)) { revokeActiveOperations(); return }; windowsSandbox = { state: "failed", mode: windowsSandbox.mode }; record(message.method, {}, "failed"); publish(); return }
    if (message.method === "windowsSandbox/setupCompleted") { if (!permitCurrent(windowsPermit)) { revokeActiveOperations(); return }; windowsSandbox = { state: params.success === true ? "ready" : "failed", mode: string(params.mode) }; record(message.method, { mode: string(params.mode) }, params.success === true ? "accepted" : "failed"); publish() }
  }
  const removeNotification = options.lease.subscribe(onNotification)
  const authorityMonitor = options.mutationAuthority === undefined ? null : setInterval(() => {
    const staleWatch = [...watches].some(([id, value]) => value.state === "active" && !permitCurrent(watchPermits.get(id) ?? null))
    const staleCommand = [...commands].some(([id, value]) => value.state === "running" && !permitCurrent(commandPermits.get(id) ?? null))
    const staleSearch = [...searches].some(([id, value]) => value.state === "active" && !permitCurrent(searchPermits.get(id) ?? null))
    const staleImport = [...imports].some(([id, value]) => value.state === "running" && !permitCurrent(importPermits.get(id) ?? null))
    const staleSetup = windowsSandbox.state === "setting_up" && !permitCurrent(windowsPermit)
    if (staleWatch || staleCommand || staleSearch || staleImport || staleSetup) revokeActiveOperations()
  }, Math.max(1, options.revocationPollMs ?? 10)); authorityMonitor?.unref?.()
  const fsMutation = async (method: string, params: ObjectValue, payload: unknown, permit: IdePortableMutationPermit | null) => { try { await requestWithPermit(method, params, permit); record(method, payload, "accepted") } catch (error) { if (!closed && !quiescing && permitCurrent(permit)) record(method, payload, "failed"); throw error } }
  const drainActive = async (): Promise<void> => {
    if (activeRequests.size === 0 && cleanupRequests.size === 0) return
    await Promise.allSettled([...activeRequests].map(operation => operation.settled).concat([...cleanupRequests]))
    return drainActive()
  }
  const boundedDrain = async (): Promise<void> => {
    const timeoutMs = Math.max(1, options.quiesceTimeoutMs ?? 5_000); let timeout: ReturnType<typeof setTimeout> | undefined
    const timedOut = new Promise<never>((_, reject) => { timeout = setTimeout(() => reject(new CodexHostServiceError("partial_failure", "Codex host services did not reach a safe point before the quiesce deadline")), timeoutMs) })
    try { await Promise.race([drainActive(), timedOut]) } finally { if (timeout !== undefined) clearTimeout(timeout) }
  }
  const quiesce = async (): Promise<void> => { if (closed) return disposePromise ?? Promise.resolve(); quiescing = true; revokeActiveOperations(); await boundedDrain() }
  const dispose = (): Promise<void> => {
    if (disposePromise !== null) return disposePromise
    closed = true; quiescing = true; removeNotification(); if (authorityMonitor !== null) clearInterval(authorityMonitor); authorities.clear(); listeners.clear(); revokeActiveOperations()
    disposePromise = boundedDrain().finally(() => options.lease.release()); return disposePromise
  }
  return {
    snapshot,
    subscribe: listener => { listeners.add(listener); return () => listeners.delete(listener) },
    authorize: (kind, payload, expectedRevision) => { assertOpen(); if (expectedRevision !== revision) throw new CodexHostServiceError("stale", "Cannot authorize stale host state"); const authority = { token: randomUUID(), kind, expectedRevision, payloadHash: hash(JSON.stringify(payload)), expiresAt: new Date(now().getTime() + (options.authorityTtlMs ?? 60_000)).toISOString() }; authorities.set(authority.token, authority); return authority },
    readFile: async relativePath => { const permit = capturePermit(); const path = resolvePath(relativePath); const response = object(await requestWithPermit("fs/readFile", { path }, permit)); const dataBase64 = string(response?.dataBase64) ?? ""; const bytes = Buffer.from(dataBase64, "base64"); record("fs/readFile", { relativePath, bytes: bytes.length }, "accepted"); if (bytes.length <= maxReadBytes) return { dataBase64, spooled: false, spoolRef: null }; const spoolRef = spool(`read:${relativePath}:${now().toISOString()}`, bytes); return { dataBase64: bytes.subarray(0, maxReadBytes).toString("base64"), spooled: true, spoolRef } },
    writeFile: async (relativePath, dataBase64, authority) => { const permit = capturePermit(); const payload = { relativePath, dataBase64 }; consume("fs_mutation", payload, authority); const bytes = Buffer.from(dataBase64, "base64"); if (bytes.length > 8_388_608) throw new CodexHostServiceError("oversized", "Write exceeds 8 MiB"); await fsMutation("fs/writeFile", { path: resolvePath(relativePath, true), dataBase64 }, payload, permit) },
    createDirectory: async (relativePath, recursive, authority) => { const permit = capturePermit(); const payload = { relativePath, recursive }; consume("fs_mutation", payload, authority); await fsMutation("fs/createDirectory", { path: resolvePath(relativePath, true), recursive }, payload, permit) },
    readDirectory: async relativePath => { const permit = capturePermit(); const result = await requestWithPermit("fs/readDirectory", { path: resolvePath(relativePath) }, permit); record("fs/readDirectory", { relativePath }, "accepted"); return result },
    metadata: async relativePath => { const permit = capturePermit(); const result = await requestWithPermit("fs/getMetadata", { path: resolvePath(relativePath) }, permit); record("fs/getMetadata", { relativePath }, "accepted"); return result },
    remove: async (relativePath, recursive, authority) => { const permit = capturePermit(); const payload = { relativePath, recursive }; consume("fs_mutation", payload, authority); await fsMutation("fs/remove", { path: resolvePath(relativePath), recursive, force: false }, payload, permit) },
    copy: async (sourceRelativePath, destinationRelativePath, recursive, authority) => { const permit = capturePermit(); const payload = { sourceRelativePath, destinationRelativePath, recursive }; consume("fs_mutation", payload, authority); await fsMutation("fs/copy", { sourcePath: resolvePath(sourceRelativePath), destinationPath: resolvePath(destinationRelativePath, true), recursive }, payload, permit) },
    watch: async relativePath => { const permit = capturePermit(); const watchId = `watch.${randomUUID()}`; await requestWithPermit("fs/watch", { path: resolvePath(relativePath), watchId }, permit); watches.set(watchId, { watchId, rootRelativePath: relativePath, state: "active", changes: [] }); watchPermits.set(watchId, permit); record("fs/watch", { watchId, relativePath }, "accepted"); publish(); return watchId },
    unwatch: async watchId => { const current = watches.get(watchId); if (current === undefined) throw new CodexHostServiceError("not_found", "Unknown watch"); const permit = watchPermits.get(watchId) ?? capturePermit(); await requestWithPermit("fs/unwatch", { watchId }, permit); watches.set(watchId, { ...current, state: "stopped" }); record("fs/unwatch", { watchId }, "accepted"); publish() },
    exec: async (input, authority) => { const permit = capturePermit(); consume("command", input, authority); if (input.command.length === 0 || input.command.length > 128 || input.command.some(value => value.length > 8_192)) throw new CodexHostServiceError("oversized", "Command argv is invalid"); const processId = `process.${randomUUID()}`; commands.set(processId, { processId, state: "running", exitCode: null, stdoutPreview: "", stderrPreview: "", spoolRef: hash(processId).slice(0, 32), capReached: false, bytes: 0 }); commandPermits.set(processId, permit); publish(); void requestWithPermit("command/exec", { command: input.command, processId, cwd: resolvePath(input.cwd ?? "."), timeoutMs: Math.min(3_600_000, Math.max(1_000, input.timeoutMs ?? 120_000)), outputBytesCap: maxReadBytes, disableOutputCap: false, disableTimeout: false, streamStdoutStderr: true, streamStdin: true, tty: input.tty === true, ...(input.tty === true ? { size: { rows: Math.min(200, Math.max(1, input.rows ?? 24)), cols: Math.min(500, Math.max(1, input.cols ?? 80)) } } : {}) }, permit).then(responseValue => { const current = commands.get(processId); if (current === undefined || current.state !== "running" || !permitCurrent(permit)) return; const response = object(responseValue); const stdout = string(response?.stdout) ?? ""; const stderr = string(response?.stderr) ?? ""; spool(`command:${processId}`, Buffer.from(`${stdout}${stderr}`)); commands.set(processId, { ...current, state: "settled", exitCode: number(response?.exitCode), stdoutPreview: bounded(current.stdoutPreview + stdout, 65_536), stderrPreview: bounded(current.stderrPreview + stderr, 65_536) }); record("command/exec", { processId, commandHash: hash(JSON.stringify(input.command)) }, "accepted"); publish() }, () => { const current = commands.get(processId); if (current === undefined || current.state !== "running" || !permitCurrent(permit)) return; commands.set(processId, { ...current, state: "settled" }); record("command/exec", { processId }, "failed"); publish() }); return processId },
    writeCommand: async (processId, deltaBase64, closeStdin = false) => { const current = commands.get(processId); if (current?.state !== "running") throw new CodexHostServiceError("already_settled", "Command is not running"); const permit = commandPermits.get(processId) ?? capturePermit(); if (Buffer.from(deltaBase64, "base64").length > 1_048_576) throw new CodexHostServiceError("oversized", "Command input exceeds 1 MiB"); await requestWithPermit("command/exec/write", { processId, deltaBase64, closeStdin }, permit); record("command/exec/write", { processId, bytes: deltaBase64.length }, "accepted") },
    resizeCommand: async (processId, rows, cols) => { const current = commands.get(processId); if (current?.state !== "running") throw new CodexHostServiceError("already_settled", "Command is not running"); const permit = commandPermits.get(processId) ?? capturePermit(); await requestWithPermit("command/exec/resize", { processId, size: { rows: Math.min(200, Math.max(1, rows)), cols: Math.min(500, Math.max(1, cols)) } }, permit); record("command/exec/resize", { processId, rows, cols }, "accepted") },
    terminateCommand: async processId => { const current = commands.get(processId); if (current?.state !== "running") throw new CodexHostServiceError("already_settled", "Command already settled"); const permit = commandPermits.get(processId) ?? capturePermit(); await requestWithPermit("command/exec/terminate", { processId }, permit); commands.set(processId, { ...current, state: "terminated" }); record("command/exec/terminate", { processId }, "accepted"); publish() },
    fuzzySearch: async query => { const permit = capturePermit(); const response = object(await requestWithPermit("fuzzyFileSearch", { query: bounded(query, 1_024), roots: [root] }, permit)); const files = array(response?.files).map(value => relativeResult(object(value)?.path)).filter((value): value is string => value !== null).slice(0, 1_000); record("fuzzyFileSearch", { queryHash: hash(query), count: files.length }, "accepted"); return files },
    startSearch: async () => { const permit = capturePermit(); const sessionId = `search.${randomUUID()}`; await requestWithPermit("fuzzyFileSearch/sessionStart", { sessionId, roots: [root] }, permit); searches.set(sessionId, { sessionId, state: "active", query: "", files: [] }); searchPermits.set(sessionId, permit); record("fuzzyFileSearch/sessionStart", { sessionId }, "accepted"); publish(); return sessionId },
    updateSearch: async (sessionId, query) => { const current = searches.get(sessionId); if (current?.state !== "active") throw new CodexHostServiceError("already_settled", "Search is not active"); const permit = searchPermits.get(sessionId) ?? capturePermit(); await requestWithPermit("fuzzyFileSearch/sessionUpdate", { sessionId, query: bounded(query, 1_024) }, permit); searches.set(sessionId, { ...current, query: bounded(query, 1_024) }); record("fuzzyFileSearch/sessionUpdate", { sessionId, queryHash: hash(query) }, "accepted"); publish() },
    stopSearch: async sessionId => { const current = searches.get(sessionId); if (current?.state !== "active") throw new CodexHostServiceError("already_settled", "Search is not active"); const permit = searchPermits.get(sessionId) ?? capturePermit(); await requestWithPermit("fuzzyFileSearch/sessionStop", { sessionId }, permit); searches.set(sessionId, { ...current, state: "stopped" }); record("fuzzyFileSearch/sessionStop", { sessionId }, "accepted"); publish() },
    detectExternalConfig: async () => { const permit = capturePermit(); const response = await requestWithPermit("externalAgentConfig/detect", { cwds: [root], includeHome: false }, permit); record("externalAgentConfig/detect", { rootHash: hash(root) }, "accepted"); return response },
    readExternalHistories: async () => { const permit = capturePermit(); const response = await requestWithPermit("externalAgentConfig/import/readHistories", undefined, permit); record("externalAgentConfig/import/readHistories", {}, "accepted"); return response },
    importExternalConfig: async (migrationItems, source, authority) => { const permit = capturePermit(); const payload = { migrationItems, source }; consume("external_import", payload, authority); const response = object(await requestWithPermit("externalAgentConfig/import", payload, permit)); const importId = string(response?.importId); if (importId === null) throw new CodexHostServiceError("partial_failure", "Import omitted identity"); imports.set(importId, { importId, state: "running", successes: 0, failures: 0 }); importPermits.set(importId, permit); record("externalAgentConfig/import", { importId }, "accepted"); publish(); return importId },
    windowsReadiness: async () => { if (platform !== "win32") throw new CodexHostServiceError("blocked_platform", "Windows sandbox is unavailable on this host"); const permit = capturePermit(); const result = await requestWithPermit("windowsSandbox/readiness", undefined, permit); const status = string(object(result)?.status) ?? "not_ready"; windowsSandbox = { state: /ready/iu.test(status) ? "ready" : "not_ready", mode: null }; record("windowsSandbox/readiness", { status }, "accepted"); publish(); return result },
    startWindowsSetup: async (mode, authority) => { if (platform !== "win32") throw new CodexHostServiceError("blocked_platform", "Windows sandbox is unavailable on this host"); const permit = capturePermit(); consume("windows_setup", { mode }, authority); windowsSandbox = { state: "setting_up", mode }; windowsPermit = permit; publish(); await requestWithPermit("windowsSandbox/setupStart", { mode, cwd: root }, permit); record("windowsSandbox/setupStart", { mode }, "accepted") },
    uploadFeedback: async (input, authority) => { if (input.attachments.length > 8) throw new CodexHostServiceError("oversized", "Feedback has too many attachments"); const permit = capturePermit(); consume("feedback", input, authority); const reviewed = input.attachments.map(path => ({ path, absolute: resolvePath(path), digest: hash(readFileSync(resolvePath(path))) })); const extraLogFiles = reviewed.map(value => { if (statSync(value.absolute).size > 5_242_880) throw new CodexHostServiceError("oversized", "Feedback attachment exceeds 5 MiB"); return value.absolute }); await requestWithPermit("feedback/upload", { classification: bounded(input.classification, 120), reason: input.reason === null ? null : bounded(input.reason, 4_000), includeLogs: input.includeLogs, extraLogFiles, tags: { reviewed: "true" } }, permit); record("feedback/upload", { attachmentDigests: reviewed.map(value => value.digest) }, "accepted") },
    receipts: () => [...receiptLog],
    quiesce,
    dispose,
    close: () => { void dispose().catch(() => undefined) },
  }
}

export const makeCodexHostServiceRegistry = (options: Readonly<{
  supervisor: CodexAppServerSupervisor
  spoolRoot: string
  receiptRoot: string
  quiesceTimeoutMs?: number
}>): CodexHostServiceRegistry => {
  const entries = new Map<string, Promise<CodexHostServices>>()
  let closed = false
  let quiescePromise: Promise<CodexHostRegistryQuiescence> | null = null
  const quiesce = (): Promise<CodexHostRegistryQuiescence> => {
    if (quiescePromise !== null) return quiescePromise
    closed = true
    const settlement = Promise.allSettled([...entries.values()]).then(async acquired => {
      if (acquired.some(result => result.status === "rejected")) return { state: "failed" as const, detailRef: "desktop.codex-host-services.registry-acquisition-failed" }
      const services = acquired.flatMap(result => result.status === "fulfilled" ? [result.value] : [])
      const disposed = await Promise.allSettled(services.map(service => service.dispose()))
      return disposed.some(result => result.status === "rejected")
        ? { state: "failed" as const, detailRef: "desktop.codex-host-services.registry-cleanup-failed" }
        : { state: "quiesced" as const }
    })
    let timer: ReturnType<typeof setTimeout> | undefined
    const deadline = new Promise<CodexHostRegistryQuiescence>(resolve => { timer = setTimeout(() => resolve({ state: "timed_out", detailRef: "desktop.codex-host-services.registry-cleanup-timeout" }), Math.max(1, options.quiesceTimeoutMs ?? 5_000)); timer.unref?.() })
    quiescePromise = Promise.race([settlement, deadline]).finally(() => { if (timer !== undefined) clearTimeout(timer) })
    return quiescePromise
  }
  return {
    forTarget: (target, workspaceRoot, portable) => {
      if (closed) return Promise.reject(new CodexHostServiceError("closed", "Codex host service registry is closed"))
      const canonicalRoot = realpathSync(workspaceRoot)
      const identity = hash(`${codexAppServerPoolKey(target)}\0${canonicalRoot}\0${portable?.workspaceGrantRef ?? "unbound"}`)
      const existing = entries.get(identity); if (existing !== undefined) return existing
      const created = options.supervisor.acquire(target).then(lease => makeCodexHostServices({
        lease,
        workspaceRoot: canonicalRoot,
        spoolRoot: resolve(options.spoolRoot, identity),
        receiptPath: resolve(options.receiptRoot, `${identity}.json`),
        ...(portable === undefined ? {} : portable),
      }), error => { entries.delete(identity); throw error })
      entries.set(identity, created)
      return created
    },
    quiesce,
    close: () => { void quiesce() },
  }
}
