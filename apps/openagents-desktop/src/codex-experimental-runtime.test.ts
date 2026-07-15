import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"
import { CODEX_EXPERIMENTAL_METHODS, codexExperimentalManifestGate, makeCodexExperimentalRuntime } from "./codex-experimental-runtime.ts"

const roots: string[] = []
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

const fixture = (earlyProcessExit = false) => {
  const root = mkdtempSync(join(tmpdir(), "oa-experimental-")); roots.push(root)
  const requests: Array<{ method: string; params: unknown }> = []; const listeners = new Set<(value: CodexAppServerNotification) => void>(); let generation = 1
  const lease = {
    state: () => ({ status: "ready" as const, generation }), release: () => undefined,
    subscribe: (listener: (value: CodexAppServerNotification) => void) => { listeners.add(listener); return () => listeners.delete(listener) },
    request: async (method: string, params: unknown) => {
      requests.push({ method, params })
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

  test("reconciles and terminates background terminals without exposing command or cwd", async () => {
    const h = fixture(); const runtime = makeCodexExperimentalRuntime({ lease: h.lease, spoolRoot: join(h.root, "spool") })
    await runtime.listBackgroundTerminals("thread-secret")
    const terminal = runtime.snapshot().terminals[0]!
    expect(JSON.stringify(terminal)).not.toContain("private --token"); expect(JSON.stringify(terminal)).not.toContain("/private/work")
    const payload = { threadId: "thread-secret", processRef: terminal.processRef, operation: "terminate" }
    await expect(runtime.terminateBackgroundTerminal("thread-secret", terminal.processRef, runtime.authorize("terminal_mutation", payload, runtime.snapshot().revision))).resolves.toBe(true)
    expect(runtime.snapshot().terminals[0]?.state).toBe("terminated")
    await runtime.listBackgroundTerminals("thread-secret"); h.notify("process/outputDelta", {}, 2)
    expect(runtime.snapshot().terminals[0]?.state).toBe("terminated")
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
    await runtime.listRemoteClients("env-1"); const clientRef = runtime.snapshot().remoteControl.clients[0]!.clientRef
    const revoke = { environmentId: "env-1", clientRef }; await runtime.revokeRemoteClient("env-1", clientRef, runtime.authorize("remote_revoke", revoke, runtime.snapshot().revision))
    expect(runtime.snapshot().remoteControl.clients[0]?.state).toBe("revoked")
    const reset = { confirmation: "RESET" }; await runtime.resetMemory("RESET", runtime.authorize("memory_reset", reset, runtime.snapshot().revision))
    const control = { threadId: "thread-secret", direction: "increment" as const }; await runtime.adjustElicitation(control.threadId, control.direction, runtime.authorize("thread_control", control, runtime.snapshot().revision))
    const projection = JSON.stringify(runtime.snapshot()); expect(projection).not.toContain("pair-secret"); expect(projection).not.toContain("client-secret"); expect(projection).not.toContain("install-secret")
    expect(statSync(receiptPath).mode & 0o777).toBe(0o600); expect(readFileSync(receiptPath, "utf8")).not.toContain("pair-secret")
    runtime.close()
  })
})
