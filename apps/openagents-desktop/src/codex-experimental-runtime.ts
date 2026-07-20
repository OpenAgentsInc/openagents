import { createHash, randomUUID } from "node:crypto"
import { appendFileSync, mkdirSync, writeFileSync } from "node:fs"
import { resolve } from "node:path"

import { bundledCodex01441ProtocolManifest } from "@openagentsinc/codex-app-server-protocol/parity"

import { codexAppServerPoolKey, type CodexAppServerLease, type CodexAppServerNotification, type CodexAppServerPoolTarget, type CodexAppServerSupervisor } from "./codex-app-server-supervisor.ts"
import type { IdePortableMutationAuthority, IdePortableMutationPermit } from "./ide/portable-mutation-authority.ts"

type Row = Readonly<Record<string, unknown>>
const row = (value: unknown): Row | null => typeof value === "object" && value !== null && !Array.isArray(value) ? value as Row : null
const array = (value: unknown): ReadonlyArray<unknown> => Array.isArray(value) ? value : []
const string = (value: unknown): string | null => typeof value === "string" ? value : null
const number = (value: unknown): number | null => typeof value === "number" && Number.isFinite(value) ? value : null
const hash = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex")
const ref = (kind: string, value: string): string => `${kind}.${hash(value).slice(0, 24)}`

const family = /^(?:environment\/|process\/|thread\/backgroundTerminals\/|thread\/realtime\/|remoteControl\/|memory\/reset$)/u
export const CODEX_EXPERIMENTAL_METHODS = Object.freeze(bundledCodex01441ProtocolManifest.members.filter(member => member.method !== "mock/experimentalMethod" && (member.stability === "experimental-gated" || member.direction === "server-notification" && family.test(member.method))).map(member => member.method).sort())
const handlers = Object.freeze([
  "environment/add", "environment/info", "memory/reset", "process/kill", "process/resizePty", "process/spawn", "process/writeStdin",
  "remoteControl/client/list", "remoteControl/client/revoke", "remoteControl/disable", "remoteControl/enable", "remoteControl/pairing/start", "remoteControl/pairing/status", "remoteControl/status/changed", "remoteControl/status/read",
  "thread/backgroundTerminals/clean", "thread/backgroundTerminals/list", "thread/backgroundTerminals/terminate",
  "thread/realtime/appendAudio", "thread/realtime/appendSpeech", "thread/realtime/appendText", "thread/realtime/closed", "thread/realtime/error", "thread/realtime/itemAdded", "thread/realtime/listVoices", "thread/realtime/outputAudio/delta", "thread/realtime/sdp", "thread/realtime/start", "thread/realtime/started", "thread/realtime/stop", "thread/realtime/transcript/delta", "thread/realtime/transcript/done",
  "process/exited", "process/outputDelta",
  "collaborationMode/list", "fuzzyFileSearch/sessionStart", "fuzzyFileSearch/sessionStop", "fuzzyFileSearch/sessionUpdate",
  "thread/decrement_elicitation", "thread/increment_elicitation", "thread/items/list", "thread/memoryMode/set", "thread/search", "thread/settings/update", "thread/turns/list",
].sort())
export const codexExperimentalManifestGate = (() => {
  const missing = CODEX_EXPERIMENTAL_METHODS.filter(method => !handlers.includes(method))
  const extra = handlers.filter(method => !CODEX_EXPERIMENTAL_METHODS.includes(method))
  return Object.freeze({ enabled: missing.length === 0 && extra.length === 0, expected: CODEX_EXPERIMENTAL_METHODS.length, covered: handlers.length, missing, extra })
})()

export type CodexExperimentalSnapshot = Readonly<{
  revision: number
  generation: number
  manifest: typeof codexExperimentalManifestGate
  environments: ReadonlyArray<Readonly<{ environmentRef: string; state: "connected" | "disconnected" | "failed"; shell: string | null; cwdRef: string | null }>>
  processes: ReadonlyArray<Readonly<{ processRef: string; environmentRef: string | null; state: "running" | "exited" | "killed" | "disconnected"; exitCode: number | null; stdoutBytes: number; stderrBytes: number; capReached: boolean; spoolRef: string }>>
  terminals: ReadonlyArray<Readonly<{ threadRef: string; processRef: string; commandRef: string; cwdRef: string; state: "running" | "terminated" | "disconnected" }>>
  realtime: ReadonlyArray<Readonly<{ threadRef: string; sessionRef: string | null; state: "starting" | "active" | "stopped" | "errored" | "closed" | "disconnected"; transport: "websocket" | "webrtc"; transcriptChars: number; audioBytes: number; itemCount: number; sdpReady: boolean; capReached: boolean }>>
  remoteControl: Readonly<{ state: "disabled" | "connecting" | "connected" | "errored"; environmentRef: string | null; installationRef: string | null; pairing: Readonly<{ pairingRef: string; state: "pending" | "claimed" | "expired" | "revoked"; expiresAt: number }> | null; clients: ReadonlyArray<Readonly<{ clientRef: string; displayName: string | null; platform: string | null; state: "granted" | "revoked" }>> }>
  receipts: ReadonlyArray<Readonly<{ operation: string; causalHash: string; outcome: "accepted" | "failed" | "revoked"; observedAt: string }>>
}>

export type CodexExperimentalAuthorityKind = "environment_add" | "process_spawn" | "process_control" | "terminal_mutation" | "thread_control" | "realtime_start" | "realtime_audio" | "remote_control" | "remote_revoke" | "memory_reset"
export type CodexExperimentalAuthority = Readonly<{ token: string; kind: CodexExperimentalAuthorityKind; payloadHash: string; expectedRevision: number; expiresAt: number }>
export class CodexExperimentalError extends Error {
  readonly _tag = "CodexExperimentalError"
  override readonly name = "CodexExperimentalError"
  constructor(readonly reason: "manifest_incomplete" | "authority_required" | "stale" | "expired" | "not_found" | "already_settled" | "oversized" | "closed" | "revoked" | "quiesced" | "cleanup_failed", message: string) { super(message) }
}

export type CodexExperimentalRuntime = Readonly<{
  snapshot: () => CodexExperimentalSnapshot
  subscribe: (listener: (snapshot: CodexExperimentalSnapshot) => void) => () => void
  authorize: (kind: CodexExperimentalAuthorityKind, payload: unknown, revision: number) => CodexExperimentalAuthority
  addEnvironment: (input: Readonly<{ environmentId: string; execServerUrl: string; connectTimeoutMs?: number }>, authority: CodexExperimentalAuthority) => Promise<void>
  reconnectEnvironment: (environmentId: string, execServerUrl: string, authority: CodexExperimentalAuthority) => Promise<void>
  environmentInfo: (environmentId: string) => Promise<unknown>
  turnEnvironment: (environmentRef: string, cwd: string) => Readonly<{ environmentId: string; cwd: string }>
  spawnProcess: (input: Readonly<{ command: ReadonlyArray<string>; cwd: string; tty?: boolean; rows?: number; cols?: number; timeoutMs?: number }>, authority: CodexExperimentalAuthority) => Promise<string>
  writeProcess: (processRef: string, dataBase64: string, closeStdin?: boolean) => Promise<void>
  resizeProcess: (processRef: string, rows: number, cols: number) => Promise<void>
  killProcess: (processRef: string, authority: CodexExperimentalAuthority) => Promise<void>
  listBackgroundTerminals: (threadId: string) => Promise<void>
  cleanBackgroundTerminals: (threadId: string, authority: CodexExperimentalAuthority) => Promise<void>
  terminateBackgroundTerminal: (threadId: string, processRef: string, authority: CodexExperimentalAuthority) => Promise<boolean>
  startRealtime: (input: Readonly<{ threadId: string; outputModality: "text" | "audio"; transport: Readonly<{ type: "websocket" } | { type: "webrtc"; sdp: string }>; voice?: string }>, authority: CodexExperimentalAuthority) => Promise<void>
  appendRealtimeAudio: (threadId: string, audio: Readonly<{ data: string; numChannels: number; sampleRate: number; samplesPerChannel?: number }>, authority: CodexExperimentalAuthority) => Promise<void>
  appendRealtimeText: (threadId: string, text: string, role?: "user" | "developer" | "assistant") => Promise<void>
  appendRealtimeSpeech: (threadId: string, text: string) => Promise<void>
  stopRealtime: (threadId: string) => Promise<void>
  listVoices: () => Promise<unknown>
  enableRemoteControl: (authority: CodexExperimentalAuthority) => Promise<void>
  disableRemoteControl: (authority: CodexExperimentalAuthority) => Promise<void>
  remoteStatus: () => Promise<void>
  startPairing: (manualCode: boolean, authority: CodexExperimentalAuthority) => Promise<string>
  pairingStatus: (pairingRef: string) => Promise<boolean>
  listRemoteClients: (environmentRef: string) => Promise<void>
  revokeRemoteClient: (environmentRef: string, clientRef: string, authority: CodexExperimentalAuthority) => Promise<void>
  resetMemory: (confirmation: "RESET", authority: CodexExperimentalAuthority) => Promise<void>
  adjustElicitation: (threadId: string, direction: "increment" | "decrement", authority: CodexExperimentalAuthority) => Promise<void>
  quiesce: () => Promise<void>
  dispose: () => Promise<void>
  close: () => void
}>

type ExecutionPermit = Readonly<{ portable: IdePortableMutationPermit | null; generation: number }>
type CleanupIntent = Readonly<{ key: string; method: string; params: unknown }>
type Environment = CodexExperimentalSnapshot["environments"][number] & { id: string; url: string; permit: ExecutionPermit }
type Process = CodexExperimentalSnapshot["processes"][number] & { handle: string; permit: ExecutionPermit }
type Terminal = CodexExperimentalSnapshot["terminals"][number] & { threadId: string; processId: string; permit: ExecutionPermit }
type Realtime = CodexExperimentalSnapshot["realtime"][number] & { threadId: string; permit: ExecutionPermit }
type Pairing = NonNullable<CodexExperimentalSnapshot["remoteControl"]["pairing"]> & { code: string; manualCode: string | null; permit: ExecutionPermit }
type Client = CodexExperimentalSnapshot["remoteControl"]["clients"][number] & { id: string; permit: ExecutionPermit }

export const makeCodexExperimentalRuntime = (options: Readonly<{
  lease: CodexAppServerLease
  spoolRoot: string
  receiptPath?: string
  now?: () => Date
  maxProcessBytes?: number
  maxRealtimeBytes?: number
  mutationAuthority?: IdePortableMutationAuthority
  grantRef?: string
  quiesceTimeoutMs?: number
}>): CodexExperimentalRuntime => {
  if (!codexExperimentalManifestGate.enabled) throw new CodexExperimentalError("manifest_incomplete", "Experimental API manifest is incomplete")
  if ((options.mutationAuthority === undefined) !== (options.grantRef === undefined)) {
    throw new CodexExperimentalError("authority_required", "Portable authority and its workspace grant must be configured together")
  }
  mkdirSync(options.spoolRoot, { recursive: true, mode: 0o700 })
  const now = () => options.now?.() ?? new Date()
  const maxProcessBytes = options.maxProcessBytes ?? 33_554_432
  const maxRealtimeBytes = options.maxRealtimeBytes ?? 67_108_864
  const quiesceTimeoutMs = options.quiesceTimeoutMs ?? 2_000
  let disposed = false; let quiesced = false; let revision = 0; let generation = options.lease.state().generation
  const environments = new Map<string, Environment>(); const processes = new Map<string, Process>(); const terminals = new Map<string, Terminal>(); const realtime = new Map<string, Realtime>()
  const pendingProcessEvents = new Map<string, CodexAppServerNotification[]>()
  const pendingProcessPermits = new Map<string, ExecutionPermit>()
  const authorities = new Map<string, CodexExperimentalAuthority>(); const listeners = new Set<(snapshot: CodexExperimentalSnapshot) => void>(); const pairingCodes = new Map<string, Pairing>(); const clientIds = new Map<string, Client>()
  const receipts: CodexExperimentalSnapshot["receipts"][number][] = []
  let remote: { state: CodexExperimentalSnapshot["remoteControl"]["state"]; environmentRef: string | null; installationRef: string | null; pairing: Pairing | null; clients: ReadonlyArray<Client> } = { state: "disabled", environmentRef: null, installationRef: null, pairing: null, clients: [] }
  let remoteEnvironmentId: string | null = null
  let remotePermit: ExecutionPermit | null = null
  let removeNotifications: (() => void) | null = null
  let quiescePromise: Promise<void> | null = null
  let disposePromise: Promise<void> | null = null
  let resourceMonitor: NodeJS.Timeout | null = null
  const active = new Set<Readonly<{ permit: ExecutionPermit; controller: AbortController; settled: Promise<void>; cleanups: Map<string, CleanupIntent> }>>()
  const publicRemote = (): CodexExperimentalSnapshot["remoteControl"] => ({ ...remote, pairing: remote.pairing === null ? null : { pairingRef: remote.pairing.pairingRef, state: remote.pairing.state, expiresAt: remote.pairing.expiresAt }, clients: remote.clients.map(({ id: _, permit: __, ...client }) => client) })
  const snapshot = (): CodexExperimentalSnapshot => ({ revision, generation, manifest: codexExperimentalManifestGate, environments: [...environments.values()].map(({ id: _, url: __, permit: ___, ...value }) => value), processes: [...processes.values()].map(({ handle: _, permit: __, ...value }) => value), terminals: [...terminals.values()].map(({ threadId: _, processId: __, permit: ___, ...value }) => value), realtime: [...realtime.values()].map(({ threadId: _, permit: __, ...value }) => value), remoteControl: publicRemote(), receipts: [...receipts] })
  const publish = () => { revision += 1; const value = snapshot(); for (const listener of listeners) listener(value) }
  const consume = (kind: CodexExperimentalAuthorityKind, payload: unknown, authority: CodexExperimentalAuthority) => { const stored = authorities.get(authority.token); authorities.delete(authority.token); if (stored === undefined || stored.kind !== kind || stored.payloadHash !== hash(JSON.stringify(payload))) throw new CodexExperimentalError("authority_required", "Exact reviewed authority is required"); if (stored.expectedRevision !== revision) throw new CodexExperimentalError("stale", "Experimental runtime state changed"); if (stored.expiresAt < now().getTime()) throw new CodexExperimentalError("expired", "Experimental authority expired") }
  const portablePermitCurrent = (permit: IdePortableMutationPermit | null): boolean => {
    if (permit === null) return true
    try { return options.mutationAuthority?.reauthorize(permit) === true } catch { return false }
  }
  const permitCurrent = (permit: ExecutionPermit): boolean =>
    !quiesced && !disposed && options.lease.state().generation === permit.generation && portablePermitCurrent(permit.portable)
  const samePermit = (left: ExecutionPermit, right: ExecutionPermit): boolean =>
    left.generation === right.generation && left.portable?.key === right.portable?.key
  const ensurePermit = (permit: ExecutionPermit): void => {
    if (quiesced || disposed) throw new CodexExperimentalError("quiesced", "Experimental runtime is quiesced")
    if (!permitCurrent(permit)) throw new CodexExperimentalError("revoked", "Portable placement or app-server generation was revoked")
  }
  const capturePermit = (): ExecutionPermit => {
    if (quiesced || disposed) throw new CodexExperimentalError("quiesced", "Experimental runtime is quiesced")
    const state = options.lease.state()
    if (state.generation !== generation) invalidateGeneration(state.generation)
    let portable: IdePortableMutationPermit | null = null
    if (options.mutationAuthority !== undefined && options.grantRef !== undefined) {
      const authorization = options.mutationAuthority.authorize(options.grantRef)
      if (authorization._tag === "Refused") throw new CodexExperimentalError("revoked", `Portable mutation authority refused: ${authorization.reason}`)
      portable = authorization.permit
    }
    const permit = { portable, generation: state.generation }
    ensurePermit(permit)
    return permit
  }
  const record = (permit: ExecutionPermit, operation: string, causal: unknown, outcome: "accepted" | "failed" | "revoked") => {
    ensurePermit(permit)
    receipts.push({ operation, causalHash: hash(JSON.stringify(causal)), outcome, observedAt: now().toISOString() })
    if (options.receiptPath !== undefined) {
      mkdirSync(resolve(options.receiptPath, ".."), { recursive: true, mode: 0o700 })
      writeFileSync(options.receiptPath, `${JSON.stringify({ schema: "openagents.codex_experimental_receipts.v1", receipts: receipts.slice(-4096) })}\n`, { mode: 0o600 })
    }
  }
  const privateSpool = (permit: ExecutionPermit, kind: string, identity: string, bytes: Buffer): string => {
    ensurePermit(permit)
    const path = resolve(options.spoolRoot, `${hash(`${kind}\0${identity}`)}.bin`)
    appendFileSync(path, bytes, { mode: 0o600 })
    return ref("spool", path)
  }
  const findProcess = (processRef: string) => { const value = [...processes.values()].find(process => process.processRef === processRef); if (value === undefined) throw new CodexExperimentalError("not_found", "Unknown process"); return value }
  const findRealtime = (threadId: string) => { const value = realtime.get(threadId); if (value === undefined) throw new CodexExperimentalError("not_found", "Unknown realtime session"); return value }
  const applyRemote = (permit: ExecutionPermit, value: unknown) => { ensurePermit(permit); const data = row(value) ?? {}; remoteEnvironmentId = string(data.environmentId); remotePermit = permit; remote = { ...remote, state: ["disabled", "connecting", "connected", "errored"].includes(string(data.status) ?? "") ? string(data.status) as typeof remote.state : "errored", environmentRef: remoteEnvironmentId === null ? null : ref("environment", remoteEnvironmentId), installationRef: string(data.installationId) === null ? null : ref("installation", string(data.installationId)!) }; publish() }
  const abortError = (): CodexExperimentalError => quiesced || disposed
    ? new CodexExperimentalError("quiesced", "Experimental runtime is quiesced")
    : new CodexExperimentalError("revoked", "Portable placement or app-server generation was revoked")
  const runOperation = async <A>(body: (operation: Readonly<{
    permit: ExecutionPermit
    request: (method: string, params: unknown) => Promise<unknown>
    registerCleanup: (intent: CleanupIntent) => void
  }>) => Promise<A>): Promise<A> => {
    const permit = capturePermit()
    const controller = new AbortController()
    let settle!: () => void
    const settled = new Promise<void>(resolveSettled => { settle = resolveSettled })
    const cleanups = new Map<string, CleanupIntent>()
    const operation = { permit, controller, settled, cleanups }
    active.add(operation)
    const timer = setInterval(() => {
      if (!permitCurrent(permit)) {
        controller.abort(abortError())
        void runtime.quiesce().catch(() => undefined)
      }
    }, 10)
    timer.unref()
    const request = async (method: string, params: unknown): Promise<unknown> => {
      ensurePermit(permit)
      const aborted = new Promise<never>((_, reject) => {
        if (controller.signal.aborted) reject(abortError())
        else controller.signal.addEventListener("abort", () => reject(abortError()), { once: true })
      })
      const response = await Promise.race([
        options.lease.request(method, params, { signal: controller.signal }),
        aborted,
      ])
      ensurePermit(permit)
      return response
    }
    try {
      return await body({ permit, request, registerCleanup: intent => cleanups.set(intent.key, intent) })
    } finally {
      clearInterval(timer)
      active.delete(operation)
      settle()
    }
  }
  function invalidateGeneration(next: number): void {
    if (next === generation) return
    generation = next
    for (const operation of active) operation.controller.abort(new CodexExperimentalError("revoked", "App-server generation changed"))
    pendingProcessEvents.clear(); pendingProcessPermits.clear()
    for (const [id, value] of environments) environments.set(id, { ...value, state: "disconnected" })
    for (const [id, value] of processes) if (value.state === "running") processes.set(id, { ...value, state: "disconnected" })
    for (const [id, value] of terminals) if (value.state === "running") terminals.set(id, { ...value, state: "disconnected" })
    for (const [id, value] of realtime) if (value.state === "active" || value.state === "starting") realtime.set(id, { ...value, state: "disconnected" })
    remote = { ...remote, state: "errored", pairing: remote.pairing === null ? null : { ...remote.pairing, state: "revoked" }, clients: remote.clients.map(client => ({ ...client, state: "revoked" })) }
    remotePermit = null
    publish()
  }
  const onNotification = ({ generation: next, message }: CodexAppServerNotification) => {
    if (quiesced || disposed) return
    if (next !== generation) { invalidateGeneration(next); return }
    const data = row(message.params) ?? {}; const method = string(message.method); if (method === null) return
    if (method === "process/outputDelta" || method === "process/exited") {
      const handle = string(data.processHandle); const value = handle === null ? undefined : processes.get(handle); const pendingPermit = handle === null ? undefined : pendingProcessPermits.get(handle)
      if (handle !== null && value === undefined) { if (pendingPermit !== undefined && permitCurrent(pendingPermit) && pendingPermit.generation === next) pendingProcessEvents.set(handle, [...(pendingProcessEvents.get(handle) ?? []), { generation: next, message }].slice(-1_024)); return }
      if (handle === null || value === undefined || value.state !== "running" || value.permit.generation !== next || !permitCurrent(value.permit)) return
      if (method === "process/outputDelta") { const bytes = Buffer.from(string(data.deltaBase64) ?? "", "base64"); const total = value.stdoutBytes + value.stderrBytes; const accepted = bytes.subarray(0, Math.max(0, maxProcessBytes - total)); privateSpool(value.permit, `process-${string(data.stream)}`, handle, accepted); const stdout = string(data.stream) === "stdout"; processes.set(handle, { ...value, stdoutBytes: value.stdoutBytes + (stdout ? accepted.length : 0), stderrBytes: value.stderrBytes + (stdout ? 0 : accepted.length), capReached: data.capReached === true || accepted.length < bytes.length }); publish(); return }
      const stdout = Buffer.from(string(data.stdout) ?? ""); const stderr = Buffer.from(string(data.stderr) ?? ""); privateSpool(value.permit, "process-exit", handle, Buffer.concat([stdout, stderr]).subarray(0, maxProcessBytes)); processes.set(handle, { ...value, state: "exited", exitCode: number(data.exitCode), stdoutBytes: value.stdoutBytes + stdout.length, stderrBytes: value.stderrBytes + stderr.length, capReached: value.capReached || data.stdoutCapReached === true || data.stderrCapReached === true }); record(value.permit, method, { handle: value.processRef, exitCode: number(data.exitCode) }, "accepted"); publish(); return
    }
    if (method === "remoteControl/status/changed") { if (remotePermit !== null && remotePermit.generation === next && permitCurrent(remotePermit)) applyRemote(remotePermit, data); return }
    const threadId = string(data.threadId); if (method.startsWith("thread/realtime/") && threadId !== null) { const value = realtime.get(threadId); if (value === undefined || value.permit.generation !== next || !permitCurrent(value.permit)) return; if (method === "thread/realtime/started") realtime.set(threadId, { ...value, state: "active", sessionRef: string(data.realtimeSessionId) === null ? null : ref("realtime", string(data.realtimeSessionId)!) }); else if (method === "thread/realtime/transcript/delta" || method === "thread/realtime/transcript/done") { const text = string(data.delta) ?? string(data.text) ?? ""; privateSpool(value.permit, "transcript", threadId, Buffer.from(text)); realtime.set(threadId, { ...value, transcriptChars: value.transcriptChars + text.length }) } else if (method === "thread/realtime/outputAudio/delta") { const bytes = Buffer.from(string(row(data.audio)?.data) ?? "", "base64"); const accepted = bytes.subarray(0, Math.max(0, maxRealtimeBytes - value.audioBytes)); privateSpool(value.permit, "realtime-audio", threadId, accepted); realtime.set(threadId, { ...value, audioBytes: value.audioBytes + accepted.length, capReached: accepted.length < bytes.length || value.audioBytes + accepted.length >= maxRealtimeBytes }) } else if (method === "thread/realtime/itemAdded") realtime.set(threadId, { ...value, itemCount: value.itemCount + 1 }); else if (method === "thread/realtime/sdp") { privateSpool(value.permit, "realtime-sdp", threadId, Buffer.from(string(data.sdp) ?? "")); realtime.set(threadId, { ...value, sdpReady: true }) } else if (method === "thread/realtime/error") realtime.set(threadId, { ...value, state: "errored" }); else if (method === "thread/realtime/closed") realtime.set(threadId, { ...value, state: "closed" }); record(value.permit, method, { threadRef: value.threadRef }, method.endsWith("error") ? "failed" : "accepted"); publish() }
  }
  removeNotifications = options.lease.subscribe(onNotification)
  const runtime: CodexExperimentalRuntime = {
    snapshot,
    subscribe: listener => { listeners.add(listener); return () => listeners.delete(listener) },
    authorize: (kind, payload, expectedRevision) => { if (disposed) throw new CodexExperimentalError("closed", "Experimental runtime is closed"); if (quiesced) throw new CodexExperimentalError("quiesced", "Experimental runtime is quiesced"); if (expectedRevision !== revision) throw new CodexExperimentalError("stale", "Cannot authorize stale state"); const authority = { token: randomUUID(), kind, payloadHash: hash(JSON.stringify(payload)), expectedRevision, expiresAt: now().getTime() + 60_000 }; authorities.set(authority.token, authority); return authority },
    addEnvironment: (input, authority) => runOperation(async operation => { consume("environment_add", input, authority); await operation.request("environment/add", input); const info = row(await operation.request("environment/info", { environmentId: input.environmentId })); environments.set(input.environmentId, { id: input.environmentId, url: input.execServerUrl, permit: operation.permit, environmentRef: ref("environment", input.environmentId), state: "connected", shell: string(row(info?.shell)?.name), cwdRef: string(info?.cwd) === null ? null : ref("cwd", string(info?.cwd)!) }); record(operation.permit, "environment/add", { environmentRef: ref("environment", input.environmentId), urlHash: hash(input.execServerUrl) }, "accepted"); publish() }),
    reconnectEnvironment: (environmentId, execServerUrl, authority) => runOperation(async operation => { const input = { environmentId, execServerUrl }; consume("environment_add", input, authority); const current = environments.get(environmentId); if (current?.state === "connected" && !samePermit(current.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Environment belongs to a revoked placement"); await operation.request("environment/add", input); environments.set(environmentId, { id: environmentId, url: execServerUrl, permit: operation.permit, environmentRef: ref("environment", environmentId), state: "connected", shell: current?.shell ?? null, cwdRef: current?.cwdRef ?? null }); record(operation.permit, "environment/reconnect", { environmentRef: ref("environment", environmentId) }, "accepted"); publish() }),
    environmentInfo: environmentId => runOperation(async operation => { const environment = environments.get(environmentId); if (environment !== undefined && !samePermit(environment.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Environment belongs to a revoked placement"); return operation.request("environment/info", { environmentId }) }),
    turnEnvironment: (environmentRef, cwd) => { const environment = [...environments.values()].find(value => value.environmentRef === environmentRef); if (environment?.state !== "connected") throw new CodexExperimentalError("not_found", "Remote environment is not connected"); ensurePermit(environment.permit); return { environmentId: environment.id, cwd } },
    spawnProcess: (input, authority) => runOperation(async operation => { consume("process_spawn", input, authority); if (input.command.length === 0 || input.command.length > 128) throw new CodexExperimentalError("oversized", "Command argv is invalid"); const handle = `process.${randomUUID()}`; operation.registerCleanup({ key: `process:${handle}`, method: "process/kill", params: { processHandle: handle } }); pendingProcessPermits.set(handle, operation.permit); try { await operation.request("process/spawn", { command: input.command, cwd: input.cwd, processHandle: handle, outputBytesCap: Math.min(maxProcessBytes, 8_388_608), timeoutMs: Math.min(3_600_000, Math.max(1_000, input.timeoutMs ?? 120_000)), streamStdin: true, streamStdoutStderr: true, tty: input.tty === true, ...(input.tty === true ? { size: { rows: Math.min(200, Math.max(1, input.rows ?? 24)), cols: Math.min(500, Math.max(1, input.cols ?? 80)) } } : {}) }) } catch (error) { pendingProcessEvents.delete(handle); throw error } finally { pendingProcessPermits.delete(handle) }; const value: Process = { handle, permit: operation.permit, processRef: ref("process", handle), environmentRef: null, state: "running", exitCode: null, stdoutBytes: 0, stderrBytes: 0, capReached: false, spoolRef: ref("spool", handle) }; processes.set(handle, value); record(operation.permit, "process/spawn", { processRef: value.processRef, argvHash: hash(JSON.stringify(input.command)) }, "accepted"); publish(); const pending = pendingProcessEvents.get(handle) ?? []; pendingProcessEvents.delete(handle); for (const notification of pending) onNotification(notification); return value.processRef }),
    writeProcess: (processRef, dataBase64, closeStdin = false) => runOperation(async operation => { const value = findProcess(processRef); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Process belongs to a revoked placement"); if (value.state !== "running") throw new CodexExperimentalError("already_settled", "Process is not running"); if (Buffer.from(dataBase64, "base64").length > 1_048_576) throw new CodexExperimentalError("oversized", "Process input exceeds 1 MiB"); await operation.request("process/writeStdin", { processHandle: value.handle, deltaBase64: dataBase64, closeStdin }) }),
    resizeProcess: (processRef, rows, cols) => runOperation(async operation => { const value = findProcess(processRef); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Process belongs to a revoked placement"); if (value.state !== "running") throw new CodexExperimentalError("already_settled", "Process is not running"); await operation.request("process/resizePty", { processHandle: value.handle, size: { rows: Math.min(200, Math.max(1, rows)), cols: Math.min(500, Math.max(1, cols)) } }) }),
    killProcess: (processRef, authority) => runOperation(async operation => { consume("process_control", { processRef, operation: "kill" }, authority); const value = findProcess(processRef); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Process belongs to a revoked placement"); if (value.state !== "running") throw new CodexExperimentalError("already_settled", "Process is not running"); await operation.request("process/kill", { processHandle: value.handle }); processes.set(value.handle, { ...value, state: "killed" }); record(operation.permit, "process/kill", { processRef }, "revoked"); publish() }),
    listBackgroundTerminals: threadId => runOperation(async operation => { const response = row(await operation.request("thread/backgroundTerminals/list", { threadId, limit: 1_000 })); for (const item of array(response?.data)) { const value = row(item); const processId = string(value?.processId); if (processId !== null) terminals.set(`${threadId}:${processId}`, { threadId, processId, permit: operation.permit, threadRef: ref("thread", threadId), processRef: ref("terminal", processId), commandRef: ref("command", string(value?.command) ?? ""), cwdRef: ref("cwd", string(value?.cwd) ?? ""), state: "running" }) }; publish() }),
    cleanBackgroundTerminals: (threadId, authority) => runOperation(async operation => { consume("terminal_mutation", { threadId, operation: "clean" }, authority); const owned = [...terminals.values()].filter(value => value.threadId === threadId); if (owned.some(value => !samePermit(value.permit, operation.permit))) throw new CodexExperimentalError("revoked", "Terminal belongs to a revoked placement"); await operation.request("thread/backgroundTerminals/clean", { threadId }); for (const [id, value] of terminals) if (value.threadId === threadId) terminals.set(id, { ...value, state: "terminated" }); record(operation.permit, "thread/backgroundTerminals/clean", { threadRef: ref("thread", threadId) }, "revoked"); publish() }),
    terminateBackgroundTerminal: (threadId, processRef, authority) => runOperation(async operation => { consume("terminal_mutation", { threadId, processRef, operation: "terminate" }, authority); const value = [...terminals.values()].find(terminal => terminal.threadId === threadId && terminal.processRef === processRef); if (value === undefined) throw new CodexExperimentalError("not_found", "Unknown background terminal"); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Terminal belongs to a revoked placement"); const response = row(await operation.request("thread/backgroundTerminals/terminate", { threadId, processId: value.processId })); terminals.set(`${threadId}:${value.processId}`, { ...value, state: "terminated" }); record(operation.permit, "thread/backgroundTerminals/terminate", { processRef }, "revoked"); publish(); return response?.terminated === true }),
    startRealtime: (input, authority) => runOperation(async operation => { consume("realtime_start", input, authority); if (input.transport.type === "webrtc" && input.transport.sdp.length > 1_048_576) throw new CodexExperimentalError("oversized", "SDP exceeds 1 MiB"); operation.registerCleanup({ key: `realtime:${input.threadId}`, method: "thread/realtime/stop", params: { threadId: input.threadId } }); await operation.request("thread/realtime/start", input); realtime.set(input.threadId, { threadId: input.threadId, permit: operation.permit, threadRef: ref("thread", input.threadId), sessionRef: null, state: "starting", transport: input.transport.type, transcriptChars: 0, audioBytes: 0, itemCount: 0, sdpReady: false, capReached: false }); record(operation.permit, "thread/realtime/start", { threadRef: ref("thread", input.threadId), transport: input.transport.type }, "accepted"); publish() }),
    appendRealtimeAudio: (threadId, audio, authority) => runOperation(async operation => { const value = findRealtime(threadId); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Realtime session belongs to a revoked placement"); consume("realtime_audio", { threadId, audio }, authority); const bytes = Buffer.from(audio.data, "base64"); if (bytes.length > 2_097_152 || value.audioBytes + bytes.length > maxRealtimeBytes) throw new CodexExperimentalError("oversized", "Realtime audio budget exceeded"); await operation.request("thread/realtime/appendAudio", { threadId, audio }); record(operation.permit, "thread/realtime/appendAudio", { threadRef: value.threadRef, bytes: bytes.length }, "accepted") }),
    appendRealtimeText: (threadId, text, role = "user") => runOperation(async operation => { const value = findRealtime(threadId); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Realtime session belongs to a revoked placement"); if (text.length > 64_000) throw new CodexExperimentalError("oversized", "Realtime text exceeds 64k"); await operation.request("thread/realtime/appendText", { threadId, text, role }) }),
    appendRealtimeSpeech: (threadId, text) => runOperation(async operation => { const value = findRealtime(threadId); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Realtime session belongs to a revoked placement"); if (text.length > 64_000) throw new CodexExperimentalError("oversized", "Realtime speech exceeds 64k"); await operation.request("thread/realtime/appendSpeech", { threadId, text }) }),
    stopRealtime: threadId => runOperation(async operation => { const value = findRealtime(threadId); if (!samePermit(value.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Realtime session belongs to a revoked placement"); if (["closed", "stopped"].includes(value.state)) throw new CodexExperimentalError("already_settled", "Realtime is already stopped"); await operation.request("thread/realtime/stop", { threadId }); realtime.set(threadId, { ...value, state: "stopped" }); record(operation.permit, "thread/realtime/stop", { threadRef: value.threadRef }, "revoked"); publish() }),
    listVoices: () => runOperation(operation => operation.request("thread/realtime/listVoices", {})),
    enableRemoteControl: authority => runOperation(async operation => { consume("remote_control", { operation: "enable" }, authority); operation.registerCleanup({ key: "remote-control", method: "remoteControl/disable", params: null }); applyRemote(operation.permit, await operation.request("remoteControl/enable", null)); record(operation.permit, "remoteControl/enable", {}, "accepted") }),
    disableRemoteControl: authority => runOperation(async operation => { consume("remote_control", { operation: "disable" }, authority); if (remotePermit !== null && !samePermit(remotePermit, operation.permit)) throw new CodexExperimentalError("revoked", "Remote control belongs to a revoked placement"); applyRemote(operation.permit, await operation.request("remoteControl/disable", null)); if (remote.pairing !== null) remote = { ...remote, pairing: { ...remote.pairing, state: "revoked" } }; record(operation.permit, "remoteControl/disable", {}, "revoked"); publish() }),
    remoteStatus: () => runOperation(async operation => { if (remotePermit !== null && !samePermit(remotePermit, operation.permit)) throw new CodexExperimentalError("revoked", "Remote control belongs to a revoked placement"); applyRemote(operation.permit, await operation.request("remoteControl/status/read", undefined)) }),
    startPairing: (manualCode, authority) => runOperation(async operation => { consume("remote_control", { operation: "pair", manualCode }, authority); if (remotePermit !== null && !samePermit(remotePermit, operation.permit)) throw new CodexExperimentalError("revoked", "Remote control belongs to a revoked placement"); operation.registerCleanup({ key: "remote-control", method: "remoteControl/disable", params: null }); const response = row(await operation.request("remoteControl/pairing/start", { manualCode })); const code = string(response?.pairingCode); if (code === null) throw new CodexExperimentalError("not_found", "Pairing response omitted identity"); const pairing: Pairing = { code, manualCode: string(response?.manualPairingCode), permit: operation.permit, pairingRef: ref("pairing", code), state: "pending", expiresAt: number(response?.expiresAt) ?? now().getTime() }; pairingCodes.set(pairing.pairingRef, pairing); remote = { ...remote, pairing }; record(operation.permit, "remoteControl/pairing/start", { pairingRef: pairing.pairingRef }, "accepted"); publish(); return pairing.pairingRef }),
    pairingStatus: pairingRef => runOperation(async operation => { const pairing = pairingCodes.get(pairingRef); if (pairing === undefined) throw new CodexExperimentalError("not_found", "Unknown pairing"); if (!samePermit(pairing.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Pairing belongs to a revoked placement"); if (pairing.expiresAt < now().getTime()) { const expired = { ...pairing, state: "expired" as const }; pairingCodes.set(pairingRef, expired); remote = { ...remote, pairing: expired }; publish(); return false }; const response = row(await operation.request("remoteControl/pairing/status", { pairingCode: pairing.code, manualPairingCode: pairing.manualCode })); if (response?.claimed === true) { const claimed = { ...pairing, state: "claimed" as const }; pairingCodes.set(pairingRef, claimed); remote = { ...remote, pairing: claimed }; publish(); return true }; return false }),
    listRemoteClients: environmentRef => runOperation(async operation => { if (remoteEnvironmentId === null || remote.environmentRef !== environmentRef) throw new CodexExperimentalError("not_found", "Unknown remote environment"); if (remotePermit !== null && !samePermit(remotePermit, operation.permit)) throw new CodexExperimentalError("revoked", "Remote control belongs to a revoked placement"); const response = row(await operation.request("remoteControl/client/list", { environmentId: remoteEnvironmentId, limit: 1_000, order: "desc" })); const clients = array(response?.data).flatMap(value => { const data = row(value); const id = string(data?.clientId); if (id === null) return []; const client: Client = { id, permit: operation.permit, clientRef: ref("client", id), displayName: string(data?.displayName), platform: string(data?.platform), state: "granted" }; clientIds.set(client.clientRef, client); return [client] }); remote = { ...remote, clients }; publish() }),
    revokeRemoteClient: (environmentRef, clientRef, authority) => runOperation(async operation => { const payload = { environmentRef, clientRef }; consume("remote_revoke", payload, authority); if (remoteEnvironmentId === null || remote.environmentRef !== environmentRef) throw new CodexExperimentalError("not_found", "Unknown remote environment"); const client = clientIds.get(clientRef); if (client === undefined) throw new CodexExperimentalError("not_found", "Unknown remote client"); if (!samePermit(client.permit, operation.permit)) throw new CodexExperimentalError("revoked", "Remote client belongs to a revoked placement"); await operation.request("remoteControl/client/revoke", { environmentId: remoteEnvironmentId, clientId: client.id }); const revoked = { ...client, state: "revoked" as const }; clientIds.set(clientRef, revoked); remote = { ...remote, clients: remote.clients.map(value => value.clientRef === clientRef ? revoked : value) }; record(operation.permit, "remoteControl/client/revoke", { clientRef }, "revoked"); publish() }),
    resetMemory: (confirmation, authority) => runOperation(async operation => { consume("memory_reset", { confirmation }, authority); if (confirmation !== "RESET") throw new CodexExperimentalError("authority_required", "Memory reset requires exact confirmation"); await operation.request("memory/reset", undefined); record(operation.permit, "memory/reset", {}, "revoked"); publish() }),
    adjustElicitation: (threadId, direction, authority) => runOperation(async operation => { const payload = { threadId, direction }; consume("thread_control", payload, authority); const method = direction === "increment" ? "thread/increment_elicitation" : "thread/decrement_elicitation"; await operation.request(method, { threadId }); record(operation.permit, method, { threadRef: ref("thread", threadId) }, "accepted"); publish() }),
    quiesce: () => {
      if (quiescePromise !== null) return quiescePromise
      quiesced = true
      const inFlightCleanup = new Map<string, CleanupIntent>()
      for (const operation of active) for (const intent of operation.cleanups.values()) inFlightCleanup.set(intent.key, intent)
      authorities.clear(); pendingProcessEvents.clear(); pendingProcessPermits.clear()
      removeNotifications?.(); removeNotifications = null
      for (const operation of active) operation.controller.abort(new CodexExperimentalError("quiesced", "Experimental runtime is quiesced"))
      resourceMonitor !== null && clearInterval(resourceMonitor); resourceMonitor = null
      const runningProcesses = [...processes.values()].filter(value => value.state === "running")
      const runningRealtime = [...realtime.values()].filter(value => value.state === "active" || value.state === "starting")
      const runningTerminals = [...terminals.values()].filter(value => value.state === "running")
      const connectedEnvironments = [...environments.values()].filter(value => value.state === "connected")
      const disableRemote = remote.state !== "disabled"
      for (const value of runningProcesses) processes.set(value.handle, { ...value, state: "disconnected" })
      for (const value of runningRealtime) realtime.set(value.threadId, { ...value, state: "disconnected" })
      for (const value of runningTerminals) terminals.set(`${value.threadId}:${value.processId}`, { ...value, state: "disconnected" })
      for (const value of connectedEnvironments) environments.set(value.id, { ...value, state: "disconnected" })
      remote = { ...remote, state: disableRemote ? "errored" : "disabled", pairing: remote.pairing === null ? null : { ...remote.pairing, state: "revoked" }, clients: remote.clients.map(value => ({ ...value, state: "revoked" })) }
      remotePermit = null
      publish()
      const cleanup: Promise<unknown>[] = [
        ...[...inFlightCleanup.values()].map(intent => options.lease.request(intent.method, intent.params, { signal: AbortSignal.timeout(quiesceTimeoutMs) })),
        ...runningProcesses.map(value => options.lease.request("process/kill", { processHandle: value.handle }, { signal: AbortSignal.timeout(quiesceTimeoutMs) }).then(() => { const current = processes.get(value.handle); if (current !== undefined) processes.set(value.handle, { ...current, state: "killed" }) })),
        ...runningRealtime.map(value => options.lease.request("thread/realtime/stop", { threadId: value.threadId }, { signal: AbortSignal.timeout(quiesceTimeoutMs) }).then(() => { const current = realtime.get(value.threadId); if (current !== undefined) realtime.set(value.threadId, { ...current, state: "stopped" }) })),
        ...runningTerminals.map(value => options.lease.request("thread/backgroundTerminals/terminate", { threadId: value.threadId, processId: value.processId }, { signal: AbortSignal.timeout(quiesceTimeoutMs) }).then(() => { const key = `${value.threadId}:${value.processId}`; const current = terminals.get(key); if (current !== undefined) terminals.set(key, { ...current, state: "terminated" }) })),
        ...(disableRemote ? [options.lease.request("remoteControl/disable", null, { signal: AbortSignal.timeout(quiesceTimeoutMs) }).then(() => { remote = { ...remote, state: "disabled" } })] : []),
      ]
      const unwind = Promise.allSettled([...[...active].map(value => value.settled), ...cleanup]).then(results => {
        publish()
        if (results.some(result => result.status === "rejected")) throw new CodexExperimentalError("cleanup_failed", "Experimental runtime cleanup did not complete")
      })
      let timeoutHandle: NodeJS.Timeout
      const timeout = new Promise<never>((_, reject) => { timeoutHandle = setTimeout(() => reject(new CodexExperimentalError("cleanup_failed", "Experimental runtime cleanup timed out")), quiesceTimeoutMs) })
      quiescePromise = Promise.race([unwind, timeout]).finally(() => clearTimeout(timeoutHandle))
      return quiescePromise
    },
    dispose: () => {
      if (disposePromise !== null) return disposePromise
      disposePromise = runtime.quiesce().finally(() => { if (disposed) return; disposed = true; listeners.clear(); options.lease.release() })
      return disposePromise
    },
    close: () => { void runtime.dispose().catch(() => undefined) },
  }
  resourceMonitor = setInterval(() => {
    if (quiesced || disposed) return
    const livePermits = [
      ...[...processes.values()].filter(value => value.state === "running").map(value => value.permit),
      ...[...realtime.values()].filter(value => value.state === "active" || value.state === "starting").map(value => value.permit),
      ...[...terminals.values()].filter(value => value.state === "running").map(value => value.permit),
      ...[...environments.values()].filter(value => value.state === "connected").map(value => value.permit),
      ...(remotePermit === null || remote.state === "disabled" ? [] : [remotePermit]),
    ]
    if (livePermits.some(permit => !permitCurrent(permit))) void runtime.quiesce().catch(() => undefined)
  }, 10)
  resourceMonitor.unref()
  return runtime
}

export type CodexExperimentalRuntimeRegistry = Readonly<{
  forTarget: (
    target: CodexAppServerPoolTarget,
    portable?: Readonly<{ mutationAuthority: IdePortableMutationAuthority; grantRef: string }>,
  ) => Promise<CodexExperimentalRuntime>
  close: () => void
}>
export const makeCodexExperimentalRuntimeRegistry = (options: Readonly<{ supervisor: CodexAppServerSupervisor; spoolRoot: string; receiptRoot: string }>): CodexExperimentalRuntimeRegistry => {
  const entries = new Map<string, Promise<CodexExperimentalRuntime>>(); let closed = false
  return { forTarget: (target, portable) => { if (closed) return Promise.reject(new CodexExperimentalError("closed", "Experimental registry is closed")); if (!codexExperimentalManifestGate.enabled) return Promise.reject(new CodexExperimentalError("manifest_incomplete", "Experimental manifest is incomplete")); const experimentalTarget = { ...target, experimentalApi: true }; const key = `${codexAppServerPoolKey(experimentalTarget)}\0${portable?.grantRef ?? "unbound"}`; const existing = entries.get(key); if (existing !== undefined) return existing; const created = options.supervisor.acquire(experimentalTarget).then(lease => makeCodexExperimentalRuntime({ lease, spoolRoot: resolve(options.spoolRoot, hash(key)), receiptPath: resolve(options.receiptRoot, `${hash(key)}.json`), ...(portable ?? {}) }), error => { entries.delete(key); throw error }); entries.set(key, created); return created }, close: () => { if (closed) return; closed = true; for (const entry of entries.values()) void entry.then(value => value.close(), () => undefined); entries.clear() } }
}
