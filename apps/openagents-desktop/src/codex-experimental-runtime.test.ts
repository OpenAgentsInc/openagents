import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"
import { CODEX_EXPERIMENTAL_METHODS, codexExperimentalManifestGate, makeCodexExperimentalRuntime } from "./codex-experimental-runtime.ts"
import type { IdePortableMutationAuthority, IdePortableMutationPermit } from "./ide/portable-mutation-authority.ts"

const roots: string[] = []
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

type RequestOverride = (
  method: string,
  params: unknown,
  options?: Readonly<{ signal?: AbortSignal }>,
) => Promise<unknown> | undefined

const fixture = (earlyProcessExit = false, requestOverride?: RequestOverride) => {
  const root = mkdtempSync(join(tmpdir(), "oa-experimental-")); roots.push(root)
  const requests: Array<{ method: string; params: unknown }> = []; const listeners = new Set<(value: CodexAppServerNotification) => void>(); let generation = 1
  const lease = {
    state: () => ({ status: "ready" as const, generation }), release: () => undefined,
    subscribe: (listener: (value: CodexAppServerNotification) => void) => { listeners.add(listener); return () => listeners.delete(listener) },
    request: async (method: string, params: unknown, options?: Readonly<{ signal?: AbortSignal }>) => {
      requests.push({ method, params })
      const overridden = requestOverride?.(method, params, options)
      if (overridden !== undefined) return overridden
      if (method === "environment/info") return { cwd: "file:///remote/work", shell: { name: "zsh", path: "/bin/zsh" } }
      if (method === "thread/backgroundTerminals/list") return { data: [{ command: "private --token", cwd: "/private/work", itemId: "i", processId: "p", osPid: 5 }], nextCursor: null }
      if (method === "thread/backgroundTerminals/terminate") return { terminated: true }
      if (method === "process/spawn" && earlyProcessExit) { const handle = (params as { processHandle: string }).processHandle; for (const listener of listeners) { listener({ generation, message: { method: "process/outputDelta", params: { processHandle: handle, stream: "stdout", deltaBase64: Buffer.from("early").toString("base64"), capReached: false } } }); listener({ generation, message: { method: "process/exited", params: { processHandle: handle, exitCode: 0, stdout: "", stderr: "", stdoutCapReached: false, stderrCapReached: false } } }) }; return {} }
      if (method === "remoteControl/enable") return { status: "connected", environmentId: "env-1", installationId: "install-secret", serverName: "desktop" }
      if (method === "remoteControl/disable") return { status: "disabled", environmentId: "env-1", installationId: "install-secret", serverName: "desktop" }
      if (method === "remoteControl/pairing/start") return { environmentId: "env-1", pairingCode: "pair-secret", manualPairingCode: "manual-secret", expiresAt: Date.now() + 60_000 }
      if (method === "remoteControl/pairing/status") return { claimed: true }
      if (method === "remoteControl/client/list") return { data: [{ clientId: "client-secret", displayName: "Phone", platform: "ios" }], nextCursor: null }
      return {}
    },
  } as unknown as CodexAppServerLease
  return { root, requests, lease, notify: (method: string, params: unknown, next = generation) => { generation = next; for (const listener of listeners) listener({ generation, message: { method, params } }) } }
}

const portableAuthority = () => {
  let current = true
  const permit: IdePortableMutationPermit = {
    _tag: "Portable",
    key: "portable:grant.1:session.1:work.1:attachment.1:1:target.1",
    grantRef: "grant.1",
    sessionRef: "session.1",
    workContextRef: "work.1",
    attachmentRef: "attachment.1",
    generation: 1,
    targetRef: "target.1",
  }
  const authority: IdePortableMutationAuthority = {
    authorize: grantRef => current && grantRef === permit.grantRef
      ? { _tag: "Permitted", permit }
      : { _tag: "Refused", reason: "attachment_ambiguous" },
    reauthorize: candidate => current && candidate.key === permit.key,
  }
  return { authority, permit, revoke: () => { current = false } }
}

describe("Codex experimental runtime", () => {
  test("enables the experimental API only for the complete generated family", () => {
    expect(codexExperimentalManifestGate).toEqual({ enabled: true, expected: 45, covered: 45, missing: [], extra: [] })
    expect(new Set(CODEX_EXPERIMENTAL_METHODS).size).toBe(45)
  })

  test("reconnects remote environments deterministically and emits private turn targeting", async () => {
    const h = fixture(); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool") })
    const input = { environmentId: "env-1", execServerUrl: "wss://secret.example/token" }
    await runtime.addEnvironment(input, runtime.authorize("environment_add", input, runtime.snapshot().revision))
    const environmentRef = runtime.snapshot().environments[0]!.environmentRef
    expect(runtime.turnEnvironment(environmentRef, "/remote/work")).toEqual({ environmentId: "env-1", cwd: "/remote/work" })
    expect(JSON.stringify(runtime.snapshot())).not.toContain("secret.example")
    h.notify("process/outputDelta", {}, 2)
    expect(runtime.snapshot().environments[0]?.state).toBe("disconnected")
    expect(() => runtime.turnEnvironment(environmentRef, "/remote/work")).toThrow()
    const reconnect = { environmentId: "env-1", execServerUrl: input.execServerUrl }
    await runtime.reconnectEnvironment("env-1", input.execServerUrl, runtime.authorize("environment_add", reconnect, runtime.snapshot().revision))
    expect(runtime.snapshot().environments[0]?.state).toBe("connected")
    runtime.close()
  })

  test("owns unsandboxed process output, PTY control, caps, exit, and disconnect cleanup", async () => {
    const h = fixture(); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool"), maxProcessBytes: 1_024 })
    const input = { command: ["private-command", "--token"], cwd: h.root, tty: true, rows: 24, cols: 80 }
    const processRef = await runtime.spawnProcess(input, runtime.authorize("process_spawn", input, runtime.snapshot().revision))
    const handle = (h.requests.find(value => value.method === "process/spawn")?.params as { processHandle: string }).processHandle
    h.notify("process/outputDelta", { processHandle: handle, stream: "stdout", deltaBase64: Buffer.alloc(2_048, 65).toString("base64"), capReached: false })
    expect(runtime.snapshot().processes[0]).toMatchObject({ processRef, state: "running", stdoutBytes: 1_024, capReached: true })
    expect(JSON.stringify(runtime.snapshot())).not.toContain("private-command")
    await runtime.resizeProcess(processRef, 999, 999); await runtime.writeProcess(processRef, Buffer.from("x").toString("base64"))
    h.notify("process/exited", { processHandle: handle, exitCode: 0, stdout: "private-output", stderr: "", stdoutCapReached: false, stderrCapReached: false })
    expect(runtime.snapshot().processes[0]?.state).toBe("exited")
    const kill = { processRef, operation: "kill" }; await expect(runtime.killProcess(processRef, runtime.authorize("process_control", kill, runtime.snapshot().revision))).rejects.toMatchObject({ reason: "already_settled" })
    runtime.close()
  })

  test("replays output and exit that race ahead of process spawn acknowledgement", async () => {
    const h = fixture(true); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool") })
    const input = { command: ["fast"], cwd: h.root }
    const processRef = await runtime.spawnProcess(input, runtime.authorize("process_spawn", input, runtime.snapshot().revision))
    expect(runtime.snapshot().processes.find(value => value.processRef === processRef)).toMatchObject({ state: "exited", exitCode: 0, stdoutBytes: 5 })
    runtime.close()
  })

  test("suppresses output and exit from a replaced app-server generation", async () => {
    const h = fixture(); const receiptPath = join(h.root, "receipts", "experimental.json"); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool"), receiptPath })
    const input = { command: ["quiet"], cwd: h.root }
    await runtime.spawnProcess(input, runtime.authorize("process_spawn", input, runtime.snapshot().revision))
    const handle = (h.requests.find(value => value.method === "process/spawn")?.params as { processHandle: string }).processHandle
    const receiptBefore = readFileSync(receiptPath, "utf8")
    h.notify("process/outputDelta", { processHandle: handle, stream: "stdout", deltaBase64: Buffer.from("stale-secret").toString("base64") }, 2)
    h.notify("process/exited", { processHandle: handle, exitCode: 0, stdout: "stale-secret", stderr: "" })
    expect(readdirSync(join(h.root, "spool"))).toEqual([])
    expect(readFileSync(receiptPath, "utf8")).toBe(receiptBefore)
    expect(runtime.snapshot().processes[0]?.state).toBe("disconnected")
    await runtime.dispose()
  })

  test("reconciles and terminates background terminals without exposing command or cwd", async () => {
    const h = fixture(); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool") })
    await runtime.listBackgroundTerminals("thread-secret")
    const terminal = runtime.snapshot().terminals[0]!
    expect(JSON.stringify(terminal)).not.toContain("private --token"); expect(JSON.stringify(terminal)).not.toContain("/private/work")
    const payload = { threadId: "thread-secret", processRef: terminal.processRef, operation: "terminate" }
    await expect(runtime.terminateBackgroundTerminal("thread-secret", terminal.processRef, runtime.authorize("terminal_mutation", payload, runtime.snapshot().revision))).resolves.toBe(true)
    expect(runtime.snapshot().terminals[0]?.state).toBe("terminated")
    await runtime.listBackgroundTerminals("thread-secret"); h.notify("process/outputDelta", {}, 2)
    expect(runtime.snapshot().terminals[0]?.state).toBe("disconnected")
    runtime.close()
  })

  test("bounds realtime transport media and surfaces interruption, errors, and closure", async () => {
    const h = fixture(); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool"), maxRealtimeBytes: 1_024 })
    const input = { threadId: "thread-secret", outputModality: "audio" as const, transport: { type: "webrtc" as const, sdp: "private-offer" } }
    await runtime.startRealtime(input, runtime.authorize("realtime_start", input, runtime.snapshot().revision))
    h.notify("thread/realtime/started", { threadId: input.threadId, realtimeSessionId: "session-secret", version: "v2" })
    h.notify("thread/realtime/transcript/delta", { threadId: input.threadId, role: "user", delta: "private words" })
    h.notify("thread/realtime/outputAudio/delta", { threadId: input.threadId, audio: { data: Buffer.alloc(2_048).toString("base64"), numChannels: 1, sampleRate: 24_000 } })
    h.notify("thread/realtime/sdp", { threadId: input.threadId, sdp: "private-answer" })
    expect(runtime.snapshot().realtime[0]).toMatchObject({ state: "active", transcriptChars: 13, audioBytes: 1_024, sdpReady: true, capReached: true })
    expect(JSON.stringify(runtime.snapshot())).not.toContain("private words"); expect(JSON.stringify(runtime.snapshot())).not.toContain("private-answer")
    h.notify("thread/realtime/error", { threadId: input.threadId, message: "credential secret" }); expect(runtime.snapshot().realtime[0]?.state).toBe("errored")
    h.notify("thread/realtime/closed", { threadId: input.threadId, reason: "private" }); expect(runtime.snapshot().realtime[0]?.state).toBe("closed")
    runtime.close()
  })

  test("correlates pairing/client grants, revokes exactly, and receipts confirmed memory reset", async () => {
    const h = fixture(); const receiptPath = join(h.root, "receipts", "experimental.json"); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool"), receiptPath })
    const enable = { operation: "enable" }; await runtime.enableRemoteControl(runtime.authorize("remote_control", enable, runtime.snapshot().revision))
    const pair = { operation: "pair", manualCode: true }; const pairingRef = await runtime.startPairing(true, runtime.authorize("remote_control", pair, runtime.snapshot().revision))
    await expect(runtime.pairingStatus(pairingRef)).resolves.toBe(true)
    const environmentRef = runtime.snapshot().remoteControl.environmentRef!
    await runtime.listRemoteClients(environmentRef); const clientRef = runtime.snapshot().remoteControl.clients[0]!.clientRef
    const revoke = { environmentRef, clientRef }; await runtime.revokeRemoteClient(environmentRef, clientRef, runtime.authorize("remote_revoke", revoke, runtime.snapshot().revision))
    expect(runtime.snapshot().remoteControl.clients[0]?.state).toBe("revoked")
    const reset = { confirmation: "RESET" }; await runtime.resetMemory("RESET", runtime.authorize("memory_reset", reset, runtime.snapshot().revision))
    const control = { threadId: "thread-secret", direction: "increment" as const }; await runtime.adjustElicitation(control.threadId, control.direction, runtime.authorize("thread_control", control, runtime.snapshot().revision))
    const projection = JSON.stringify(runtime.snapshot()); expect(projection).not.toContain("pair-secret"); expect(projection).not.toContain("client-secret"); expect(projection).not.toContain("install-secret")
    expect(statSync(receiptPath).mode & 0o777).toBe(0o600); expect(readFileSync(receiptPath, "utf8")).not.toContain("pair-secret")
    runtime.close()
  })

  test("aborts a blocked request when its immutable portable permit is revoked", async () => {
    let started!: () => void
    const requestStarted = new Promise<void>(resolve => { started = resolve })
    const h = fixture(false, (method, _params, options) => {
      if (method !== "process/spawn") return undefined
      started()
      return new Promise((_resolve, reject) => {
        options?.signal?.addEventListener("abort", () => reject(options.signal?.reason), { once: true })
      })
    })
    const portable = portableAuthority()
    const receiptPath = join(h.root, "receipts", "experimental.json")
    const runtime = makeCodexExperimentalRuntime({
      lease: h.lease,
      spoolRoot: join(h.root, "spool"),
      receiptPath,
      mutationAuthority: portable.authority,
      grantRef: portable.permit.grantRef,
    })
    const input = { command: ["blocked"], cwd: h.root }
    const pending = runtime.spawnProcess(input, runtime.authorize("process_spawn", input, runtime.snapshot().revision))
    await requestStarted
    portable.revoke()
    await expect(pending).rejects.toMatchObject({ reason: "revoked" })
    await runtime.quiesce()
    expect(h.requests.map(value => value.method)).toContain("process/kill")
    expect(existsSync(receiptPath)).toBe(false)
    expect(readdirSync(join(h.root, "spool"))).toEqual([])
    await runtime.dispose()
  })

  test("quiesces quiet owned resources on revocation and suppresses late output and receipts", async () => {
    let cleanupStarted!: () => void
    const cleanup = new Promise<void>(resolve => { cleanupStarted = resolve })
    const h = fixture(false, method => {
      if (method === "process/kill") cleanupStarted()
      return undefined
    })
    const portable = portableAuthority()
    const receiptPath = join(h.root, "receipts", "experimental.json")
    const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool"), receiptPath, mutationAuthority: portable.authority, grantRef: portable.permit.grantRef })
    const processInput = { command: ["quiet"], cwd: h.root }
    await runtime.spawnProcess(processInput, runtime.authorize("process_spawn", processInput, runtime.snapshot().revision))
    await runtime.listBackgroundTerminals("thread.1")
    const realtimeInput = { threadId: "thread.1", outputModality: "audio" as const, transport: { type: "websocket" as const } }
    await runtime.startRealtime(realtimeInput, runtime.authorize("realtime_start", realtimeInput, runtime.snapshot().revision))
    const enable = { operation: "enable" }
    await runtime.enableRemoteControl(runtime.authorize("remote_control", enable, runtime.snapshot().revision))
    const handle = (h.requests.find(value => value.method === "process/spawn")?.params as { processHandle: string }).processHandle
    const receiptBefore = readFileSync(receiptPath, "utf8")
    portable.revoke()
    await cleanup
    await runtime.quiesce()
    expect(h.requests.map(value => value.method)).toEqual(expect.arrayContaining([
      "process/kill",
      "thread/realtime/stop",
      "thread/backgroundTerminals/terminate",
      "remoteControl/disable",
    ]))
    const spoolBefore = readdirSync(join(h.root, "spool"))
    h.notify("process/outputDelta", { processHandle: handle, stream: "stdout", deltaBase64: Buffer.from("late-secret").toString("base64") })
    h.notify("process/exited", { processHandle: handle, exitCode: 0, stdout: "late-secret", stderr: "" })
    expect(readdirSync(join(h.root, "spool"))).toEqual(spoolBefore)
    expect(readFileSync(receiptPath, "utf8")).toBe(receiptBefore)
    await expect(runtime.listVoices()).rejects.toMatchObject({ reason: "quiesced" })
    expect(() => runtime.authorize("memory_reset", { confirmation: "RESET" }, runtime.snapshot().revision)).toThrow(expect.objectContaining({ reason: "quiesced" }))
    await runtime.dispose()
  })

  test("reports cleanup timeout and keeps unconfirmed resources disconnected", async () => {
    const h = fixture(false, method => method === "process/kill" ? new Promise(() => undefined) : undefined)
    const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool"), quiesceTimeoutMs: 20 })
    const input = { command: ["quiet"], cwd: h.root }
    await runtime.spawnProcess(input, runtime.authorize("process_spawn", input, runtime.snapshot().revision))
    await expect(runtime.quiesce()).rejects.toMatchObject({ reason: "cleanup_failed" })
    expect(runtime.snapshot().processes[0]?.state).toBe("disconnected")
    await expect(runtime.dispose()).rejects.toMatchObject({ reason: "cleanup_failed" })
  })
})
