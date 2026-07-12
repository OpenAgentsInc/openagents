/**
 * Adversarial PTY suite + built-host receipts (CUT-20, #8700).
 *
 * The deterministic cases drive an injected spy backend; the RECEIPTS drive the
 * real `childProcessTerminalBackend` under bun (real child-process trees, real
 * stdin steering, real exit codes, real process-tree disposal) — the evidence
 * the #8700 close rule names ("merge with process-tree disposal evidence").
 */
import { describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  buildRedactionMap,
  childProcessTerminalBackend,
  detectAnnouncedPorts,
  makeRing,
  makeTerminalHost,
  redactChunk,
  type TerminalBackend,
  type TerminalBackendProcess,
} from "./terminal-host.ts"
import {
  decodeTerminalCreateRequest,
  decodeTerminalInputRequest,
  type TerminalEvent,
} from "./terminal-contract.ts"

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
      const proc: SpyProcess = {
        pid: 4242,
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
        emitExit: (code, signal) => exitListener?.(code, signal),
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
    expect(host.interrupt("terminal.rev01")).toEqual({ ok: false, reason: "grant_revoked" })
    expect(host.resize("terminal.rev01", 100, 40)).toEqual({ ok: false, reason: "grant_revoked" })
    // Revoking the OLD grant kills the owned tree exactly once.
    host.revokeWorkspace("g1")
    expect(spawned[0]!.kills).toBe(1)
    expect(host.liveSessionCount()).toBe(0)
    host.dispose()
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
    host.input("terminal.receipt01", "echo hello-cut20-receipt\n")
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
