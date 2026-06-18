import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { Effect, Exit, Scope } from "effect"
import { makePylonNodeRuntime, setWalletStatus } from "../src/node/runtime"
import { startControlServer } from "../src/node/control-server"

// Bug 2 (the Orwell report on v1.0.x): `pylon status` (no --json) and
// `pylon doctor` had NO read-only/remote path, so they fell through to the
// default node boot — which binds the control port (4716). When the
// Autopilot/GUI bun node already held 4716, they crashed with EADDRINUSE.
//
// These tests start a REAL control server holding a test port, then run the
// `status`/`doctor` CLI as subprocesses against that already-bound port. If the
// CLI still tried to bind, it would crash with EADDRINUSE. Instead it must
// detect the running node and report it READ-ONLY.

const CLI_ENTRY = join(import.meta.dir, "..", "src", "index.ts")

const dirs: string[] = []
function tempHome(token: string): string {
  const dir = mkdtempSync(join(tmpdir(), "pylon-status-doctor-"))
  dirs.push(dir)
  // The per-home control token the CLI reads (never writes) to authenticate a
  // read-only command to the running node. >= 16 chars to pass the guard.
  writeFileSync(join(dir, "control-token"), `${token}\n`, { mode: 0o600 })
  return dir
}

afterEach(() => {
  while (dirs.length > 0) {
    try {
      rmSync(dirs.pop()!, { recursive: true, force: true })
    } catch {
      // best-effort
    }
  }
})

// Start a real control server bound to `port` with a stub read-only
// `walletStatus`. Returns a closer that tears the Scope down.
async function startStubNode(input: { port: number; token: string }): Promise<() => Promise<void>> {
  const scope = Effect.runSync(Scope.make())
  await Effect.runPromise(
    Effect.gen(function* () {
      const runtime = yield* makePylonNodeRuntime
      // A daemon-online-shaped wallet pane so the snapshot is populated; the
      // read-only command returns the projection below.
      yield* setWalletStatus(runtime, { daemonOnline: true, balanceSats: 5_672, readiness: "ready" })
      yield* startControlServer(runtime, {
        token: input.token,
        port: input.port,
        hostname: "127.0.0.1",
        actions: {
          walletSend: async () => ({ dispatched: false }),
          walletReceive: async () => ({ invoice: "lnbc-test" }),
          walletAdmitPayoutTarget: async () => ({ admitted: false }),
          // Read-only projection the `status`/`doctor` remote read fetches.
          walletStatus: async () => ({
            schema: "openagents.pylon.wallet_status.v0.1",
            configured: true,
            daemonOnline: true,
            balanceSats: 5_672,
            blockerRefs: [],
          }),
        },
      })
    }).pipe(Effect.provideService(Scope.Scope, scope)),
  )
  return () => Effect.runPromise(Scope.close(scope, Exit.void))
}

function runCli(args: string[], env: Record<string, string>) {
  const proc = Bun.spawn(["bun", CLI_ENTRY, ...args], {
    env: { ...Bun.env, ...env, PYLON_DISABLE_OPENCODE_STARTUP: "1" },
    stdout: "pipe",
    stderr: "pipe",
  })
  return proc
}

describe("status/doctor query a running node read-only (Bug 2: no bind conflict)", () => {
  test("`pylon status` reports the live node WITHOUT binding the already-held control port", async () => {
    const port = 4831
    const token = "status-test-token-0123456789"
    const home = tempHome(token)
    const close = await startStubNode({ port, token })
    try {
      const proc = runCli(["status"], {
        PYLON_HOME: home,
        PYLON_CONTROL_PORT: String(port),
      })
      const exitCode = await proc.exited
      const stdout = await new Response(proc.stdout).text()
      const stderr = await new Response(proc.stderr).text()
      // It must NOT crash on the bind (no EADDRINUSE), and must exit cleanly.
      expect(stderr).not.toMatch(/EADDRINUSE|address (already )?in use|could not start/i)
      expect(exitCode).toBe(0)
      const json = JSON.parse(stdout)
      expect(json.node.running).toBe(true)
      // The live wallet read came back through the control API, read-only.
      expect(json.node.wallet.daemonOnline).toBe(true)
      expect(json.node.wallet.balanceSats).toBe(5_672)
    } finally {
      await close()
    }
  }, 30_000)

  test("`pylon doctor` reports the resolved home + live node read-only WITHOUT binding the port", async () => {
    const port = 4832
    const token = "doctor-test-token-0123456789"
    const home = tempHome(token)
    const close = await startStubNode({ port, token })
    try {
      const proc = runCli(["doctor"], {
        PYLON_HOME: home,
        PYLON_CONTROL_PORT: String(port),
      })
      const exitCode = await proc.exited
      const stdout = await new Response(proc.stdout).text()
      const stderr = await new Response(proc.stderr).text()
      expect(stderr).not.toMatch(/EADDRINUSE|address (already )?in use|could not start/i)
      expect(exitCode).toBe(0)
      const json = JSON.parse(stdout)
      expect(json.ok).toBe(true)
      expect(json.schema).toBe("openagents.pylon.doctor.v0.1")
      // Public-safe home label + seed presence, never the seed itself.
      expect(json.home.path).toBe(home)
      expect(json.home.source).toBe("explicit_pylon_home")
      expect(json.node.running).toBe(true)
      expect(json.node.wallet.daemonOnline).toBe(true)
    } finally {
      await close()
    }
  }, 30_000)

  test("`pylon doctor --remote` errors cleanly (no bind) when NO node is running", async () => {
    const home = tempHome("no-node-token-0123456789ab")
    const proc = runCli(["doctor", "--remote"], {
      PYLON_HOME: home,
      // A port nothing is listening on.
      PYLON_CONTROL_PORT: "4839",
    })
    const exitCode = await proc.exited
    const stdout = await new Response(proc.stdout).text()
    const stderr = await new Response(proc.stderr).text()
    expect(stderr).not.toMatch(/EADDRINUSE|address (already )?in use|could not start/i)
    // --remote with no reachable node is a clean nonzero, not a crash.
    expect(exitCode).toBe(1)
    const json = JSON.parse(stdout)
    expect(json.ok).toBe(false)
    expect(json.error).toMatch(/no Pylon node reachable/i)
  }, 30_000)
})
