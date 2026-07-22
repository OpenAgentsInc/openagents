/**
 * Adversarial PTY suite + built-host receipts (CUT-20, #8700).
 *
 * The deterministic cases drive an injected spy backend; the RECEIPTS drive the
 * real `childProcessTerminalBackend` under bun (real child-process trees, real
 * stdin steering, real exit codes, real process-tree disposal) — the evidence
 * the #8700 close rule names ("merge with process-tree disposal evidence").
 */
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  buildRedactionMap,
  childProcessTerminalBackend,
  detectAnnouncedPorts,
  makeRing,
  makeTerminalHost as makeGuardedTerminalHost,
  redactChunk,
  type TerminalBackend,
  type TerminalBackendProcess,
  type TerminalHostOptions,
} from "./terminal-host.ts"
import {
  decodeTerminalCreateRequest,
  decodeTerminalInputRequest,
  type TerminalEvent,
} from "./terminal-contract.ts"
import type { IdePortableMutationAuthority } from "./ide/portable-mutation-authority.ts"

const testMutationAuthority: IdePortableMutationAuthority = {
  authorize: grantRef => ({
    _tag: "Permitted",
    permit: Object.freeze({
      _tag: "LocalOnly",
      key: `local:${grantRef}`,
      grantRef,
      sessionRef: `session.${grantRef}`,
      workContextRef: `work-context.${grantRef}`,
      attachmentRef: null,
      generation: null,
      targetRef: null,
    }),
  }),
  reauthorize: () => true,
}

const makeTerminalHost = (
  options: Omit<TerminalHostOptions, "mutationAuthority">,
) => makeGuardedTerminalHost({ ...options, mutationAuthority: testMutationAuthority })

const portablePermit = (generation: number) => Object.freeze({
  _tag: "Portable" as const,
  key: `portable:attachment.${generation}:${generation}`,
  grantRef: "g-portable",
  sessionRef: "session.portable",
  workContextRef: "work-context.portable",
  attachmentRef: `attachment.${generation}`,
  generation,
  targetRef: "target.local",
})

// ---------------------------------------------------------------------------
// Spy backend for deterministic adversarial cases.
// ---------------------------------------------------------------------------

type SpyProcess = TerminalBackendProcess & {
  writes: string[]
  interrupts: number
  kills: number
  spawnInput: Parameters<TerminalBackend["spawn"]>[0]
  emitData: (chunk: string) => void
  emitExit: (code: number | null, signal: string | null) => void
}

const makeSpyBackend = () => {
  const spawned: SpyProcess[] = []
  const backend: TerminalBackend = {
    spawn: (spawnInput) => {
      let dataListener: ((chunk: string) => void) | null = null
      let exitListener: ((code: number | null, signal: string | null) => void) | null = null
      let settle!: () => void; const settled = new Promise<void>(resolve => { settle = resolve })
      const proc: SpyProcess = {
        pid: 4242,
        settled,
        writes: [],
        interrupts: 0,
        kills: 0,
        spawnInput,
        write: (data) => { proc.writes.push(data) },
        resize: () => undefined,
        interrupt: () => { proc.interrupts += 1 },
        kill: () => { proc.kills += 1 },
        onData: (listener) => { dataListener = listener },
        onExit: (listener) => { exitListener = listener },
        emitData: (chunk) => dataListener?.(chunk),
        emitExit: (code, signal) => { exitListener?.(code, signal); settle() },
      }
      spawned.push(proc)
      return proc
    },
  }
  return { backend, spawned }
}

const grant = (grantRef: string, root: string) => () => ({ root, grantRef })

const collector = () => {
  const events: TerminalEvent[] = []
  return { emit: (event: TerminalEvent) => { events.push(event) }, events }
}

const waitFor = async (
  predicate: () => boolean | Promise<boolean>,
  timeoutMs = 8_000,
): Promise<void> => {
  const start = Date.now()
  while (!(await predicate())) {
    if (Date.now() - start > timeoutMs) throw new Error("waitFor timed out")
    await new Promise((resolve) => setTimeout(resolve, 25))
  }
}

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

// ---------------------------------------------------------------------------
// 1. Shell injection: renderer input never becomes argv; spawn argv is fixed.
// ---------------------------------------------------------------------------

describe("adversarial: shell injection", () => {
  test("the create request schema carries no shell/argv/cwd/env — a malicious payload is stripped", () => {
    const decoded = decodeTerminalCreateRequest({
      sessionRef: "terminal.abc123xyz",
      cols: 80,
      rows: 24,
      argv: ["rm", "-rf", "/"],
      cwd: "/etc",
      env: { EVIL: "1" },
      shell: "/bin/evil",
    }) as Record<string, unknown> | null
    expect(decoded).not.toBeNull()
    expect(decoded).not.toHaveProperty("argv")
    expect(decoded).not.toHaveProperty("cwd")
    expect(decoded).not.toHaveProperty("env")
    expect(decoded).not.toHaveProperty("shell")
  })

  test("input is written to STDIN verbatim and the spawn argv stays fixed and workspace-bound", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws-a"),
      shell: { command: "/bin/sh", args: [] },
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin" }),
    })
    const created = host.create({ sessionRef: "terminal.inject01" })
    expect(created.ok).toBe(true)
    // The classic injection string is passed as STDIN data — never argv.
    const payload = "$(rm -rf /); `curl evil`\n"
    const decodedInput = decodeTerminalInputRequest({ sessionRef: "terminal.inject01", data: payload })
    expect(decodedInput).not.toBeNull()
    host.input("terminal.inject01", payload)
    expect(spawned).toHaveLength(1)
    expect(spawned[0]!.spawnInput.shell).toBe("/bin/sh")
    expect(spawned[0]!.spawnInput.args).toEqual([])
    expect(spawned[0]!.spawnInput.cwd).toBe("/tmp/ws-a")
    expect(spawned[0]!.writes).toEqual([payload])
    host.dispose()
  })
})

// ---------------------------------------------------------------------------
// 2. Secret environment: values never reach the renderer projection.
// ---------------------------------------------------------------------------

describe("adversarial: secret environment", () => {
  test("buildRedactionMap flags secret-named and secret-shaped values, ignores plain ones", () => {
    const map = buildRedactionMap({
      PATH: "/usr/bin",
      SECRET_TOKEN: "super-secret-value-123",
      OPENAI_API_KEY: "sk-abcdefgh12345678",
      HOME: "/home/x",
      GREETING: "hello",
    })
    expect(map.has("super-secret-value-123")).toBe(true)
    expect(map.has("sk-abcdefgh12345678")).toBe(true)
    expect(map.has("/usr/bin")).toBe(false)
    expect(map.has("hello")).toBe(false)
  })

  test("a secret env value echoed to output is redacted in the streamed chunk AND the tail", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin", DEPLOY_SECRET: "hunter2hunter2xyz" }),
    })
    host.create({ sessionRef: "terminal.secret01" })
    spawned[0]!.emitData("echo done: hunter2hunter2xyz and sk-livetoken12345678\n")
    const outputs = sink.events.filter((event): event is Extract<TerminalEvent, { kind: "output" }> =>
      event.kind === "output")
    expect(outputs).toHaveLength(1)
    expect(outputs[0]!.chunk).not.toContain("hunter2hunter2xyz")
    expect(outputs[0]!.chunk).toContain("«redacted:DEPLOY_SECRET»")
    expect(outputs[0]!.chunk).not.toContain("sk-livetoken12345678")
    const snapshot = host.snapshot()
    expect(snapshot.sessions[0]!.tail).not.toContain("hunter2hunter2xyz")
    expect(snapshot.sessions[0]!.tail).not.toContain("sk-livetoken12345678")
    host.dispose()
  })
})

// ---------------------------------------------------------------------------
// 3. Runaway output: the ring buffer caps memory and marks the gap.
// ---------------------------------------------------------------------------

describe("adversarial: runaway output", () => {
  test("makeRing holds the byte cap and reports it dropped", () => {
    const ring = makeRing(100)
    for (let index = 0; index < 1_000; index += 1) ring.append("0123456789")
    expect(ring.tail(1_000).length).toBeLessThanOrEqual(100)
    expect(ring.dropped()).toBe(true)
  })

  test("a flooding session stays bounded and the snapshot marks gap=true", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: sink.emit,
      ringCapBytes: 1_024,
      tailBytes: 1_024,
      env: () => ({ PATH: "/usr/bin" }),
    })
    host.create({ sessionRef: "terminal.flood01" })
    for (let index = 0; index < 5_000; index += 1) spawned[0]!.emitData("flood-line-of-output\n")
    const snapshot = host.snapshot()
    expect(snapshot.sessions[0]!.tail.length).toBeLessThanOrEqual(1_024)
    expect(snapshot.sessions[0]!.gap).toBe(true)
    host.dispose()
  })
})

// ---------------------------------------------------------------------------
// 5. Duplicate start.
// ---------------------------------------------------------------------------

describe("adversarial: duplicate start", () => {
  test("a second create with the same sessionRef is a typed duplicate rejection", () => {
    const { backend } = makeSpyBackend()
    const sink = collector()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin" }),
    })
    expect(host.create({ sessionRef: "terminal.dup01" }).ok).toBe(true)
    const second = host.create({ sessionRef: "terminal.dup01" })
    expect(second).toEqual({ ok: false, reason: "duplicate", message: "That terminal is already open." })
    host.dispose()
  })

  test("create is refused at capacity with a typed reason", () => {
    const { backend } = makeSpyBackend()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: () => undefined,
      maxSessions: 1,
      env: () => ({ PATH: "/usr/bin" }),
    })
    expect(host.create({}).ok).toBe(true)
    const second = host.create({})
    expect(second.ok).toBe(false)
    if (!second.ok) expect(second.reason).toBe("at_capacity")
    host.dispose()
  })
})

describe("source safe-point quiescence", () => {
  test("waits for every owned process tree and permanently refuses new work", async () => {
    const { backend, spawned } = makeSpyBackend(); const sink = collector()
    const host = makeTerminalHost({ backend, workspace: grant("g1", "/tmp/ws"), emit: sink.emit, env: () => ({ PATH: "/usr/bin" }), quiesceTimeoutMs: 1_000 })
    expect(host.create({ sessionRef: "terminal.quiesce01" }).ok).toBe(true)
    const lateEventsAtStart = sink.events.length; let completed = false
    const first = host.quiesce().then(result => { completed = true; return result })
    expect(spawned[0]?.kills).toBe(1); await Promise.resolve(); expect(completed).toBe(false)
    spawned[0]?.emitData("late output"); expect(sink.events).toHaveLength(lateEventsAtStart + 1)
    spawned[0]?.emitExit(null, "SIGTERM")
    await expect(first).resolves.toEqual({ state: "quiesced" }); await expect(host.quiesce()).resolves.toEqual({ state: "quiesced" })
    expect(host.create({ sessionRef: "terminal.quiesce02" })).toMatchObject({ ok: false })
  })

  test("reports a blocked process tree as timed out", async () => {
    const { backend } = makeSpyBackend()
    const host = makeTerminalHost({ backend, workspace: grant("g1", "/tmp/ws"), emit: () => undefined, env: () => ({ PATH: "/usr/bin" }), quiesceTimeoutMs: 10 })
    host.create({ sessionRef: "terminal.quiesce03" })
    await expect(host.quiesce()).resolves.toEqual({ state: "timed_out", detailRef: "desktop.terminal.process-tree-settlement-timeout" })
  })
})

// ---------------------------------------------------------------------------
// 6. Port collision.
// ---------------------------------------------------------------------------

describe("adversarial: port collision", () => {
  test("detectAnnouncedPorts parses only the process's own announced ports", () => {
    expect([...detectAnnouncedPorts("Local:   http://localhost:5173/")].sort()).toEqual([5173])
    expect([...detectAnnouncedPorts("ready - started server on port 3000")].sort()).toEqual([3000])
    expect(detectAnnouncedPorts("no port here")).toEqual([])
  })

  test("two live sessions announcing the same port yields a typed collision error on the second", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin" }),
    })
    host.create({ sessionRef: "terminal.portA" })
    host.create({ sessionRef: "terminal.portB" })
    spawned[0]!.emitData("Local: http://localhost:5173/\n")
    spawned[1]!.emitData("Local: http://localhost:5173/\n")
    const previews = sink.events.filter((event) => event.kind === "preview")
    expect(previews).toHaveLength(1)
    expect(previews[0]!.sessionRef).toBe("terminal.portA")
    const errors = sink.events.filter((event): event is Extract<TerminalEvent, { kind: "error" }> =>
      event.kind === "error")
    expect(errors).toHaveLength(1)
    expect(errors[0]!.sessionRef).toBe("terminal.portB")
    expect(errors[0]!.message).toContain("already owned")
    host.dispose()
  })
})

// ---------------------------------------------------------------------------
// 7. Revoked grants.
// ---------------------------------------------------------------------------

describe("adversarial: revoked grants", () => {
  test("ops on a session whose workspace grant changed are denied; revokeWorkspace kills it", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    let currentGrant: string | null = "g1"
    const host = makeTerminalHost({
      backend,
      workspace: () => (currentGrant === null ? null : { root: "/tmp/ws", grantRef: currentGrant }),
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin" }),
    })
    host.create({ sessionRef: "terminal.rev01" })
    // The workspace is replaced (a different grant is now authoritative).
    currentGrant = "g2"
    expect(host.input("terminal.rev01", "ls\n")).toEqual({ ok: false, reason: "grant_revoked" })
    expect(host.interrupt("terminal.rev01")).toEqual({ ok: false, reason: "not_found" })
    expect(host.resize("terminal.rev01", 100, 40)).toEqual({ ok: false, reason: "not_found" })
    // The first stale command already revoked the old grant and killed its tree.
    host.revokeWorkspace("g1")
    expect(spawned[0]!.kills).toBe(1)
    expect(host.liveSessionCount()).toBe(0)
    host.dispose()
  })

  test("refuses before spawn when portable authority is absent or changes after admission", () => {
    const refusedBackend = makeSpyBackend()
    const refused = makeGuardedTerminalHost({
      backend: refusedBackend.backend,
      workspace: grant("g-portable", "/tmp/ws"),
      mutationAuthority: {
        authorize: () => ({ _tag: "Refused", reason: "sync_unavailable" }),
        reauthorize: () => false,
      },
      emit: () => undefined,
    })
    expect(refused.create({ sessionRef: "terminal.authority01" })).toMatchObject({
      ok: false,
      reason: "no_workspace",
    })
    expect(refusedBackend.spawned).toHaveLength(0)

    const staleBackend = makeSpyBackend()
    const permit = portablePermit(7)
    const stale = makeGuardedTerminalHost({
      backend: staleBackend.backend,
      workspace: grant(permit.grantRef, "/tmp/ws"),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit }),
        reauthorize: () => false,
      },
      emit: () => undefined,
    })
    expect(stale.create({ sessionRef: "terminal.authority02" })).toMatchObject({
      ok: false,
      reason: "no_workspace",
    })
    expect(staleBackend.spawned).toHaveLength(0)

    const changedDuringSpawnBackend = makeSpyBackend()
    const currentPermit = portablePermit(8)
    let currentKey = currentPermit.key
    const changedDuringSpawn = makeGuardedTerminalHost({
      backend: {
        spawn: input => {
          const process = changedDuringSpawnBackend.backend.spawn(input)
          currentKey = portablePermit(9).key
          return process
        },
      },
      workspace: grant(currentPermit.grantRef, "/tmp/ws"),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit: currentPermit }),
        reauthorize: candidate => candidate.key === currentKey,
      },
      emit: () => undefined,
    })
    expect(changedDuringSpawn.create({ sessionRef: "terminal.authority03" })).toMatchObject({
      ok: false,
      reason: "no_workspace",
    })
    expect(changedDuringSpawnBackend.spawned).toHaveLength(1)
    expect(changedDuringSpawnBackend.spawned[0]!.kills).toBe(1)
    refused.dispose()
    stale.dispose()
    changedDuringSpawn.dispose()
  })

  test("tears down a stale generation before accepting commands or late output", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    const permit = portablePermit(7)
    let activeKey = permit.key
    const host = makeGuardedTerminalHost({
      backend,
      workspace: grant(permit.grantRef, "/tmp/ws"),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit }),
        reauthorize: candidate => candidate.key === activeKey,
      },
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin" }),
    })
    expect(host.create({ sessionRef: "terminal.generation01" }).ok).toBe(true)
    activeKey = portablePermit(8).key
    expect(host.input("terminal.generation01", "echo stale\n")).toEqual({
      ok: false,
      reason: "grant_revoked",
    })
    expect(spawned[0]!.writes).toEqual([])
    expect(spawned[0]!.kills).toBe(1)
    spawned[0]!.emitData("late stale output\n")
    expect(sink.events.some(event => event.kind === "output")).toBe(false)
    expect(sink.events.filter(event => event.kind === "closed")).toHaveLength(1)
    expect(host.liveSessionCount()).toBe(0)
    host.dispose()
  })

  test("checks the captured generation on output and each process command", async () => {
    const commands = ["resize", "interrupt", "restart"] as const
    for (const command of commands) {
      const { backend, spawned } = makeSpyBackend()
      const permit = portablePermit(9)
      let activeKey = permit.key
      const host = makeGuardedTerminalHost({
        backend,
        workspace: grant(permit.grantRef, "/tmp/ws"),
        mutationAuthority: {
          authorize: () => ({ _tag: "Permitted", permit }),
          reauthorize: candidate => candidate.key === activeKey,
        },
        emit: () => undefined,
      })
      const sessionRef = `terminal.stale-${command}`
      expect(host.create({ sessionRef }).ok).toBe(true)
      activeKey = portablePermit(10).key
      const result = command === "resize"
        ? host.resize(sessionRef, 100, 40)
        : command === "interrupt"
          ? host.interrupt(sessionRef)
          : host.restart(sessionRef)
      expect(result).toEqual({ ok: false, reason: "grant_revoked" })
      expect(spawned[0]!.kills).toBe(1)
      expect(spawned).toHaveLength(1)
      host.dispose()
    }

    const outputBackend = makeSpyBackend()
    const outputSink = collector()
    const outputPermit = portablePermit(11)
    let outputKey = outputPermit.key
    const outputHost = makeGuardedTerminalHost({
      backend: outputBackend.backend,
      workspace: grant(outputPermit.grantRef, "/tmp/ws"),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit: outputPermit }),
        reauthorize: candidate => candidate.key === outputKey,
      },
      emit: outputSink.emit,
    })
    outputHost.create({ sessionRef: "terminal.stale-output" })
    outputKey = portablePermit(12).key
    outputBackend.spawned[0]!.emitData("must be dropped\n")
    expect(outputSink.events.some(event => event.kind === "output")).toBe(false)
    expect(outputBackend.spawned[0]!.kills).toBe(1)
    expect(await outputHost.openPreview("terminal.stale-output", 3000)).toEqual({
      ok: false,
      reason: "not_found",
    })
    outputHost.dispose()

    const previewBackend = makeSpyBackend()
    const previewPermit = portablePermit(13)
    let previewKey = previewPermit.key
    let previewOpenCount = 0
    const previewHost = makeGuardedTerminalHost({
      backend: previewBackend.backend,
      workspace: grant(previewPermit.grantRef, "/tmp/ws"),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit: previewPermit }),
        reauthorize: candidate => candidate.key === previewKey,
      },
      emit: () => undefined,
      openPreview: (_url, authorize) => {
        previewOpenCount += 1
        previewKey = portablePermit(14).key
        return authorize()
      },
    })
    previewHost.create({ sessionRef: "terminal.stale-preview" })
    previewBackend.spawned[0]!.emitData("Local: http://localhost:3000/\n")
    expect(await previewHost.openPreview("terminal.stale-preview", 3000)).toEqual({
      ok: false,
      reason: "grant_revoked",
    })
    expect(previewOpenCount).toBe(1)
    expect(previewBackend.spawned[0]!.kills).toBe(1)
    previewHost.dispose()
  })
})

// ---------------------------------------------------------------------------
// Exactly-once disposal + restart.
// ---------------------------------------------------------------------------

describe("exactly-once disposal and restart", () => {
  test("close kills the tree exactly once; a second close is a no-op ok", () => {
    const { backend, spawned } = makeSpyBackend()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: () => undefined,
      env: () => ({ PATH: "/usr/bin" }),
    })
    host.create({ sessionRef: "terminal.close01" })
    expect(host.close("terminal.close01")).toEqual({ ok: true })
    expect(host.close("terminal.close01")).toEqual({ ok: true })
    expect(spawned[0]!.kills).toBe(1)
    host.dispose()
  })

  test("restart kills the old tree once and spawns a fresh process under the same ref", () => {
    const { backend, spawned } = makeSpyBackend()
    const sink = collector()
    const host = makeTerminalHost({
      backend,
      workspace: grant("g1", "/tmp/ws"),
      emit: sink.emit,
      env: () => ({ PATH: "/usr/bin" }),
    })
    host.create({ sessionRef: "terminal.restart01" })
    expect(host.restart("terminal.restart01")).toEqual({ ok: true })
    expect(spawned).toHaveLength(2)
    expect(spawned[0]!.kills).toBe(1)
    const readyEvents = sink.events.filter((event) => event.kind === "ready")
    expect(readyEvents).toHaveLength(2)
    host.dispose()
  })
})

// ---------------------------------------------------------------------------
// Restart recovery (loss-accounted persistence across an app restart).
// ---------------------------------------------------------------------------

describe("restart recovery", () => {
  test("a persisted bounded tail is reloaded as a recovered, gap-marked session", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "cut20-recover-"))
    const persistencePath = path.join(dir, "terminals.json")
    try {
      const first = makeTerminalHost({
        backend: makeSpyBackend().backend,
        workspace: grant("g1", "/tmp/ws"),
        emit: () => undefined,
        persistencePath,
        env: () => ({ PATH: "/usr/bin" }),
      })
      first.create({ sessionRef: "terminal.persist01" })
      first.dispose()

      const second = makeTerminalHost({
        backend: makeSpyBackend().backend,
        workspace: grant("g1", "/tmp/ws"),
        emit: () => undefined,
        persistencePath,
        env: () => ({ PATH: "/usr/bin" }),
      })
      const recovered = second.snapshot().sessions.find((session) =>
        session.sessionRef === "terminal.persist01")
      expect(recovered).toBeDefined()
      expect(recovered!.recovered).toBe(true)
      expect(recovered!.gap).toBe(true)
      expect(recovered!.status).toBe("recovered")
      second.dispose()
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  })
})

// ---------------------------------------------------------------------------
// 4 + BUILT-HOST RECEIPT: a REAL process tree, real stdin, real disposal.
// ---------------------------------------------------------------------------

describe("built-host receipt (real child-process backend)", () => {
  test("a real shell runs a stdin command, output is captured, exit code observed, tree disposed", async () => {
    const dir = mkdtempSync(path.join(tmpdir(), "cut20-receipt-"))
    const sink = collector()
    const host = makeTerminalHost({
      backend: childProcessTerminalBackend(500),
      workspace: grant("g-receipt", dir),
      shell: { command: "/bin/sh", args: [] },
      emit: sink.emit,
      env: () => ({ PATH: process.env.PATH ?? "/usr/bin:/bin" }),
    })
    const created = host.create({ sessionRef: "terminal.receipt01" })
    expect(created.ok).toBe(true)
    // xterm sends Enter as CR. The pipe-backed fallback must adapt that to LF
    // just as a real PTY's line discipline would.
    host.input("terminal.receipt01", "echo hello-cut20-receipt\r")
    await waitFor(() =>
      sink.events.some((event) => event.kind === "output" && event.chunk.includes("hello-cut20-receipt")))
    host.input("terminal.receipt01", "exit 3\n")
    await waitFor(() => sink.events.some((event) => event.kind === "exit"))
    const exit = sink.events.find((event): event is Extract<TerminalEvent, { kind: "exit" }> =>
      event.kind === "exit")
    expect(exit!.exitCode).toBe(3)
    expect(host.liveSessionCount()).toBe(0)
    host.dispose()
    rmSync(dir, { recursive: true, force: true })
  })

  test("PROCESS-TREE DISPOSAL EVIDENCE: closing a session reaps a backgrounded grandchild (no orphan)", async () => {
    const dir = mkdtempSync(path.join(tmpdir(), "cut20-orphan-"))
    const sink = collector()
    const host = makeTerminalHost({
      backend: childProcessTerminalBackend(300),
      workspace: grant("g-orphan", dir),
      shell: { command: "/bin/sh", args: [] },
      emit: sink.emit,
      env: () => ({ PATH: process.env.PATH ?? "/usr/bin:/bin" }),
    })
    host.create({ sessionRef: "terminal.orphan01" })
    // The shell backgrounds a long sleep (a grandchild in the same group) and
    // prints its PID.
    host.input("terminal.orphan01", "sleep 30 & echo GRANDCHILD:$!\n")
    await waitFor(() => sink.events.some((event) =>
      event.kind === "output" && /GRANDCHILD:\d+/.test(event.chunk)))
    const chunk = sink.events
      .filter((event): event is Extract<TerminalEvent, { kind: "output" }> => event.kind === "output")
      .map((event) => event.chunk)
      .join("")
    const grandchildPid = Number(/GRANDCHILD:(\d+)/.exec(chunk)![1])
    expect(alive(grandchildPid)).toBe(true)
    // Close the session -> SIGTERM the WHOLE process group -> the grandchild dies.
    host.close("terminal.orphan01")
    await waitFor(() => !alive(grandchildPid), 6_000)
    expect(alive(grandchildPid)).toBe(false)
    host.dispose()
    rmSync(dir, { recursive: true, force: true })
  })
})

// ---------------------------------------------------------------------------
// DEV-PREVIEW RECEIPT: a real bound port is detected, ready, then freed on stop.
// ---------------------------------------------------------------------------

describe("dev-preview receipt (real bound port lifecycle)", () => {
  test("a real server's announced port is detected + reachable, and freed when the session stops", async () => {
    const dir = mkdtempSync(path.join(tmpdir(), "cut20-preview-"))
    const sink = collector()
    const host = makeTerminalHost({
      backend: childProcessTerminalBackend(500),
      workspace: grant("g-preview", dir),
      shell: { command: "/bin/sh", args: [] },
      emit: sink.emit,
      env: () => ({ PATH: process.env.PATH ?? "/usr/bin:/bin" }),
      openPreview: () => true,
    })
    host.create({ sessionRef: "terminal.preview01" })
    // A trivial real HTTP server on an OS-assigned port, announced in the
    // dev-server "Local:" shape the detector parses (no port scanning).
    const server =
      "const s=require('http').createServer((_q,r)=>r.end('ok'));" +
      "s.listen(0,()=>console.log('Local: http://localhost:'+s.address().port+'/'))"
    host.input("terminal.preview01", `node -e "${server}"\n`)
    await waitFor(() => sink.events.some((event) => event.kind === "preview"), 12_000)
    const preview = sink.events.find((event): event is Extract<TerminalEvent, { kind: "preview" }> =>
      event.kind === "preview")!
    expect(preview.ready).toBe(true)
    expect(preview.url).toBe(`http://localhost:${preview.port}/`)
    // Readiness: the announced port actually serves.
    const response = await fetch(preview.url)
    expect(await response.text()).toBe("ok")
    // openPreview resolves the announced (not arbitrary) port.
    const opened = await host.openPreview("terminal.preview01", preview.port)
    expect(opened.ok).toBe(true)
    const rejected = await host.openPreview("terminal.preview01", 1)
    expect(rejected.ok).toBe(false)
    if (!rejected.ok) expect(rejected.reason).toBe("unknown_port")
    // Stop the owning session -> the server's process tree dies -> port frees.
    host.close("terminal.preview01")
    await waitFor(async () => {
      try {
        await fetch(preview.url)
        return false
      } catch {
        return true
      }
    }, 8_000)
    host.dispose()
    rmSync(dir, { recursive: true, force: true })
  })
})
