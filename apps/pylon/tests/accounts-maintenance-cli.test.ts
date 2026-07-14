/**
 * CLI parity for typed per-harness maintenance (MAINT-1, #8785):
 * `pylon accounts maintenance --json` (status) and
 * `pylon accounts maintenance --update --harness codex --json` drive the same
 * detect → pin → update → RE-PROBE → receipt engine as Desktop Settings.
 *
 * The spawned CLI runs against a fixture machine: fake `codex`/`npm` binaries
 * on a PATH shaped like an npm-global prefix, a fixture HOME with a
 * live-looking `~/.codex/auth.json` that must stay byte-identical, and a
 * local registry server standing in for registry.npmjs.org (no network).
 */
import { describe, expect, test } from "bun:test"
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { tmpdir } from "node:os"

import {
  harnessMaintenanceReceiptsPath,
  loadHarnessMaintenanceReceipts,
} from "@openagentsinc/pylon-core/custody/harness-maintenance"

const INDEX = join(import.meta.dirname, "..", "src", "index.ts")
const CWD = join(import.meta.dirname, "..")

async function runPylonCli(args: string[], env: Record<string, string | undefined>) {
  const proc = Bun.spawn(["bun", INDEX, ...args], {
    cwd: CWD,
    env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const timeout = setTimeout(() => proc.kill(), 20_000)
  try {
    const [exitCode, stdout, stderr] = await Promise.all([
      proc.exited,
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ])
    return { exitCode, stdout, stderr }
  } finally {
    clearTimeout(timeout)
  }
}

type Fixture = {
  root: string
  env: Record<string, string | undefined>
  pylonHome: string
  codexAuthFile: string
  stop: () => void
}

async function withFixture<T>(fn: (fixture: Fixture) => Promise<T>): Promise<T> {
  const root = await mkdtemp(join(tmpdir(), "pylon-maintenance-cli-"))
  const server = Bun.serve({
    port: 0,
    fetch(request) {
      if (new URL(request.url).pathname.endsWith("/latest")) {
        return Response.json({ version: "0.45.0" })
      }
      return new Response("not found", { status: 404 })
    },
  })
  try {
    const binDir = join(root, "prefix", "lib", "node_modules", ".bin")
    await mkdir(binDir, { recursive: true })
    const versionFile = join(root, "codex-version.txt")
    await writeFile(versionFile, "0.44.0", "utf8")
    const codexPath = join(binDir, "codex")
    await writeFile(
      codexPath,
      `#!/bin/sh\nif [ "$1" = "--version" ]; then\n  echo "codex-cli $(cat "${versionFile}")"\n  exit 0\nfi\nexit 1\n`,
      "utf8",
    )
    await chmod(codexPath, 0o755)
    const npmPath = join(binDir, "npm")
    await writeFile(npmPath, `#!/bin/sh\nprintf "0.45.0" > "${versionFile}"\nexit 0\n`, "utf8")
    await chmod(npmPath, 0o755)

    const home = join(root, "home")
    const codexHome = join(home, ".codex")
    await mkdir(codexHome, { recursive: true })
    const codexAuthFile = join(codexHome, "auth.json")
    await writeFile(codexAuthFile, JSON.stringify({ fixture: "live-session" }), "utf8")

    const pylonHome = join(root, "pylon-home")
    await mkdir(pylonHome, { recursive: true })

    const bunDir = dirname(Bun.which("bun") ?? "/usr/local/bin/bun")
    const env: Record<string, string | undefined> = {
      PATH: `${binDir}:${bunDir}:/usr/bin:/bin`,
      HOME: home,
      PYLON_HOME: pylonHome,
      PYLON_NPM_REGISTRY_BASE: `http://127.0.0.1:${server.port}`,
      // A leaked CODEX_HOME must never reach maintenance spawns.
      CODEX_HOME: codexHome,
    }
    return await fn({ root, env, pylonHome, codexAuthFile, stop: () => server.stop(true) })
  } finally {
    server.stop(true)
    await rm(root, { recursive: true, force: true })
  }
}

describe("pylon accounts maintenance CLI", () => {
  test("status projection reports version/channel/advisory per harness", async () => {
    await withFixture(async (fixture) => {
      const result = await runPylonCli(["accounts", "maintenance", "--json"], fixture.env)
      expect(result.exitCode).toBe(0)
      const projection = JSON.parse(result.stdout) as {
        schema: string
        harnesses: Array<{
          harness: string
          installed: boolean
          installedVersion: string | null
          latestVersion: string | null
          channel: string
          advisory: string
          updateSupported: boolean
        }>
      }
      expect(projection.schema).toBe("openagents.pylon.harness_maintenance_status.v0.1")
      const codex = projection.harnesses.find((entry) => entry.harness === "codex")
      expect(codex?.installed).toBe(true)
      expect(codex?.installedVersion).toBe("0.44.0")
      expect(codex?.latestVersion).toBe("0.45.0")
      expect(codex?.channel).toBe("npm-global")
      expect(codex?.advisory).toBe("behind_latest")
      expect(codex?.updateSupported).toBe(true)
      const opencode = projection.harnesses.find((entry) => entry.harness === "opencode")
      expect(opencode?.installed).toBe(false)
    })
  }, 30_000)

  test("update round trip persists a provenance receipt and leaves ~/.codex untouched", async () => {
    await withFixture(async (fixture) => {
      const authBefore = await readFile(fixture.codexAuthFile, "utf8")
      const result = await runPylonCli(
        ["accounts", "maintenance", "--update", "--harness", "codex", "--json"],
        fixture.env,
      )
      expect(result.stderr).toBe("")
      expect(result.exitCode).toBe(0)
      const receipt = JSON.parse(result.stdout) as {
        schema: string
        outcome: string
        pin: { expectedVersion: string; channel: string; targetVersion: string }
        before: { installedVersion: string }
        after: { installedVersion: string; probeOk: boolean }
        update: { exitCode: number }
      }
      expect(receipt.schema).toBe("openagents.pylon.harness_maintenance_receipt.v0.1")
      expect(receipt.outcome).toBe("updated")
      expect(receipt.pin.expectedVersion).toBe("0.44.0")
      expect(receipt.pin.targetVersion).toBe("0.45.0")
      expect(receipt.after.installedVersion).toBe("0.45.0")
      expect(receipt.after.probeOk).toBe(true)

      // Receipt persisted under the pylon home.
      const summary = { paths: { home: fixture.pylonHome, config: "", cache: "", releases: "" } }
      const receipts = await loadHarnessMaintenanceReceipts(summary, "codex")
      expect(receipts).toHaveLength(1)
      expect(receipts[0]!.outcome).toBe("updated")
      expect(harnessMaintenanceReceiptsPath(summary, "codex")).toContain(fixture.pylonHome)

      // The default ~/.codex login home is byte-identical.
      expect(await readFile(fixture.codexAuthFile, "utf8")).toBe(authBefore)
    })
  }, 30_000)

  test("channel jump is refused with a non-zero exit", async () => {
    await withFixture(async (fixture) => {
      const result = await runPylonCli(
        [
          "accounts",
          "maintenance",
          "--update",
          "--harness",
          "codex",
          "--channel",
          "homebrew",
          "--json",
        ],
        fixture.env,
      )
      expect(result.exitCode).toBe(1)
      const receipt = JSON.parse(result.stdout) as { outcome: string; update: unknown }
      expect(receipt.outcome).toBe("channel_jump_refused")
      expect(receipt.update).toBeNull()
    })
  }, 30_000)
})
