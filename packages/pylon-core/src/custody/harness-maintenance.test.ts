/**
 * Harness maintenance (MAINT-1, #8785) — fixture-harness proofs.
 *
 * The round-trip tests spawn REAL fixture binaries (a fake `codex` whose
 * version comes from a state file, and a fake `npm` that flips the state
 * file), so detect → update → re-probe → receipt runs through the default
 * command runner and PATH resolver, not through mocks.
 */
import { afterEach, describe, expect, test } from "bun:test"
import { chmod, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  assertHarnessMaintenanceCommandSafe,
  classifyHarnessInstallChannel,
  collectHarnessMaintenanceStatus,
  compareHarnessVersions,
  detectHarnessInstall,
  HARNESS_MAINTENANCE_DEFINITIONS,
  harnessMaintenanceEnvironment,
  harnessMaintenanceReceiptsPath,
  harnessUpdateCommandForChannel,
  loadHarnessMaintenancePin,
  loadHarnessMaintenanceReceipts,
  normalizeMaintenanceHarness,
  parseHarnessVersionOutput,
  persistHarnessMaintenanceReceipt,
  projectPublicHarnessMaintenanceReceipt,
  PYLON_HARNESS_MAINTENANCE_RECEIPT_SCHEMA,
  runHarnessMaintenanceUpdate,
  type HarnessMaintenanceDeps,
  type PylonHarnessMaintenanceReceipt,
} from "./harness-maintenance.js"

const cleanups: Array<() => Promise<void>> = []

afterEach(async () => {
  while (cleanups.length > 0) {
    await cleanups.pop()!()
  }
})

async function makeFixtureRoot(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "pylon-harness-maintenance-"))
  cleanups.push(() => rm(root, { recursive: true, force: true }))
  return root
}

type FixtureHarness = {
  root: string
  /** PATH-visible bin dir shaped like an npm-global prefix. */
  binDir: string
  versionFile: string
  codexAuthFile: string
  env: Record<string, string | undefined>
  deps: HarnessMaintenanceDeps
  summary: { paths: { home: string; config: string; cache: string; releases: string } }
}

/**
 * Builds a fixture machine: fake `codex` + fake `npm` under a path that
 * classifies as npm-global, a fixture HOME containing a live-looking
 * `~/.codex/auth.json`, and an isolated pylon home for receipts. The latest
 * version is injected (no network).
 */
async function makeFixtureHarness(options: {
  installedVersion: string
  latestVersion: string | null
  updateBehavior?: "succeed" | "fail" | "break-binary" | "noop"
}): Promise<FixtureHarness> {
  const root = await makeFixtureRoot()
  // "/lib/node_modules/.bin/" makes the channel classifier see npm-global.
  const binDir = join(root, "prefix", "lib", "node_modules", ".bin")
  await mkdir(binDir, { recursive: true })
  const versionFile = join(root, "codex-version.txt")
  await writeFile(versionFile, `${options.installedVersion}\n`, "utf8")

  const codexPath = join(binDir, "codex")
  await writeFile(
    codexPath,
    `#!/bin/sh\nif [ "$1" = "--version" ]; then\n  v=$(cat "${versionFile}")\n  if [ "$v" = "BROKEN" ]; then echo "exec format error" >&2; exit 86; fi\n  echo "codex-cli $v"\n  exit 0\nfi\nexit 1\n`,
    "utf8",
  )
  await chmod(codexPath, 0o755)

  const behavior = options.updateBehavior ?? "succeed"
  const npmPath = join(binDir, "npm")
  const updateScript =
    behavior === "fail"
      ? `echo "npm ERR! network tunneling socket could not be established" >&2\nexit 1`
      : behavior === "break-binary"
        ? `printf "BROKEN" > "${versionFile}"\nexit 0`
        : behavior === "noop"
          ? `exit 0`
          : `printf "%s" "${options.latestVersion ?? ""}" > "${versionFile}"\nexit 0`
  await writeFile(npmPath, `#!/bin/sh\n${updateScript}\n`, "utf8")
  await chmod(npmPath, 0o755)

  // Fixture HOME with a live-looking default ~/.codex login home. The whole
  // maintenance flow must leave it byte-identical.
  const home = join(root, "home")
  const codexHome = join(home, ".codex")
  await mkdir(codexHome, { recursive: true })
  const codexAuthFile = join(codexHome, "auth.json")
  await writeFile(codexAuthFile, JSON.stringify({ fixture: "live-session-do-not-touch" }), "utf8")

  const pylonHome = join(root, "pylon-home")
  await mkdir(pylonHome, { recursive: true })

  const env: Record<string, string | undefined> = {
    // Fixture bin dir first (fake codex/npm win); system dirs so the fixture
    // shell scripts can use cat/printf.
    PATH: `${binDir}:/usr/bin:/bin`,
    HOME: home,
    // A leaked CODEX_HOME in the caller environment must be scrubbed before
    // any maintenance spawn.
    CODEX_HOME: codexHome,
  }
  return {
    root,
    binDir,
    versionFile,
    codexAuthFile,
    env,
    deps: {
      env,
      fetchLatestVersion: async () => options.latestVersion,
    },
    summary: {
      paths: {
        home: pylonHome,
        config: join(pylonHome, "config.json"),
        cache: join(pylonHome, "cache"),
        releases: join(pylonHome, "cache", "releases"),
      },
    },
  }
}

async function snapshotCodexHome(fixture: FixtureHarness): Promise<string> {
  const content = await readFile(fixture.codexAuthFile, "utf8")
  const info = await stat(fixture.codexAuthFile)
  return `${content}:${info.mtimeMs}`
}

describe("channel classification and update commands", () => {
  test("classifies npm/bun/pnpm/homebrew/native paths per harness definition", () => {
    const claude = HARNESS_MAINTENANCE_DEFINITIONS.claude_code
    expect(classifyHarnessInstallChannel(claude, ["/usr/local/lib/node_modules/.bin/claude"])).toBe(
      "npm-global",
    )
    expect(classifyHarnessInstallChannel(claude, ["/Users/x/.bun/bin/claude"])).toBe("bun-global")
    expect(classifyHarnessInstallChannel(claude, ["/Users/x/.local/share/pnpm/claude"])).toBe(
      "pnpm-global",
    )
    expect(classifyHarnessInstallChannel(claude, ["/opt/homebrew/bin/claude"])).toBe("homebrew")
    // The native install wins over generic prefixes: Claude knows BOTH the
    // npm package and its own `claude update` path.
    expect(classifyHarnessInstallChannel(claude, ["/Users/x/.local/bin/claude"])).toBe("native")
    expect(classifyHarnessInstallChannel(claude, ["/somewhere/else/claude"])).toBe("unknown")

    const opencode = HARNESS_MAINTENANCE_DEFINITIONS.opencode
    expect(classifyHarnessInstallChannel(opencode, ["/Users/x/.opencode/bin/opencode"])).toBe(
      "native",
    )
    const codex = HARNESS_MAINTENANCE_DEFINITIONS.codex
    expect(classifyHarnessInstallChannel(codex, ["/opt/homebrew/bin/codex"])).toBe("homebrew")
  })

  test("update command follows the channel's native path", () => {
    const claude = HARNESS_MAINTENANCE_DEFINITIONS.claude_code
    expect(harnessUpdateCommandForChannel(claude, "npm-global")).toEqual({
      executable: "npm",
      args: ["install", "-g", "@anthropic-ai/claude-code@latest"],
    })
    expect(harnessUpdateCommandForChannel(claude, "native")).toEqual({
      executable: "claude",
      args: ["update"],
    })
    expect(harnessUpdateCommandForChannel(claude, "homebrew")).toEqual({
      executable: "brew",
      args: ["upgrade", "claude-code"],
    })
    // Unknown channel = no one-click update, ever. Guessing is the failure.
    expect(harnessUpdateCommandForChannel(claude, "unknown")).toBeNull()
    // Codex has no native updater.
    expect(harnessUpdateCommandForChannel(HARNESS_MAINTENANCE_DEFINITIONS.codex, "native")).toBeNull()
  })

  test("normalizeMaintenanceHarness accepts aliases", () => {
    expect(normalizeMaintenanceHarness("codex")).toBe("codex")
    expect(normalizeMaintenanceHarness("claude")).toBe("claude_code")
    expect(normalizeMaintenanceHarness("claude-code")).toBe("claude_code")
    expect(normalizeMaintenanceHarness("Claude_Agent")).toBe("claude_code")
    expect(normalizeMaintenanceHarness("opencode")).toBe("opencode")
    expect(normalizeMaintenanceHarness("grok")).toBeNull()
  })

  test("version parsing and comparison", () => {
    expect(parseHarnessVersionOutput("codex-cli 0.144.1")).toBe("0.144.1")
    expect(parseHarnessVersionOutput("1.0.63 (Claude Code)")).toBe("1.0.63")
    expect(parseHarnessVersionOutput("no version here")).toBeNull()
    expect(compareHarnessVersions("0.9.9", "0.10.0")).toBe(-1)
    expect(compareHarnessVersions("1.2.3", "1.2.3")).toBe(0)
  })
})

describe("auth-state guards", () => {
  test("refuses login/auth-flow command arguments", () => {
    expect(() =>
      assertHarnessMaintenanceCommandSafe({ executable: "codex", args: ["login"] }),
    ).toThrow(/never auth state/)
    expect(() =>
      assertHarnessMaintenanceCommandSafe({ executable: "codex", args: ["login", "--device-auth"] }),
    ).toThrow()
    expect(() =>
      assertHarnessMaintenanceCommandSafe({ executable: "claude", args: ["update"] }),
    ).not.toThrow()
  })

  test("maintenance environment scrubs harness home isolation variables", () => {
    const env = harnessMaintenanceEnvironment({
      PATH: "/usr/bin",
      CODEX_HOME: "/tmp/evil",
      CLAUDE_CONFIG_DIR: "/tmp/evil2",
      CLAUDE_CODE_OAUTH_TOKEN: "tok",
      GROK_HOME: "/tmp/evil3",
    })
    expect(env.PATH).toBe("/usr/bin")
    expect(env.CODEX_HOME).toBeUndefined()
    expect(env.CLAUDE_CONFIG_DIR).toBeUndefined()
    expect(env.CLAUDE_CODE_OAUTH_TOKEN).toBeUndefined()
    expect(env.GROK_HOME).toBeUndefined()
  })
})

describe("fixture-harness round trip", () => {
  test("detect → update → re-probe → receipt, with pin and provenance", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
    })
    const codexHomeBefore = await snapshotCodexHome(fixture)

    const before = await detectHarnessInstall("codex", fixture.deps)
    expect(before.installed).toBe(true)
    expect(before.channel).toBe("npm-global")
    expect(before.installedVersion).toBe("0.44.0")
    expect(before.probeOk).toBe(true)
    expect(before.binarySha256).toMatch(/^[0-9a-f]{64}$/)

    const receipt = await runHarnessMaintenanceUpdate({ harness: "codex", deps: fixture.deps })
    expect(receipt.schema).toBe(PYLON_HARNESS_MAINTENANCE_RECEIPT_SCHEMA)
    expect(receipt.outcome).toBe("updated")
    expect(receipt.failureReason).toBeNull()
    // Pin recorded BEFORE the swap: expected version + checksum + channel.
    expect(receipt.pin?.expectedVersion).toBe("0.44.0")
    expect(receipt.pin?.expectedBinarySha256).toBe(before.binarySha256)
    expect(receipt.pin?.channel).toBe("npm-global")
    expect(receipt.pin?.targetVersion).toBe("0.45.0")
    // Provenance: what ran, from where.
    expect(receipt.update?.executable).toContain("npm")
    expect(receipt.update?.args).toEqual(["install", "-g", "@openai/codex@latest"])
    expect(receipt.update?.exitCode).toBe(0)
    expect(receipt.source?.kind).toBe("npm-registry")
    expect(receipt.source?.packageName).toBe("@openai/codex")
    // RE-PROBE result: the swapped binary answered with the new version.
    expect(receipt.after?.probeOk).toBe(true)
    expect(receipt.after?.installedVersion).toBe("0.45.0")
    expect(receipt.after?.channel).toBe("npm-global")

    // Receipt + pin persist and load back.
    await persistHarnessMaintenanceReceipt(fixture.summary, receipt)
    const receipts = await loadHarnessMaintenanceReceipts(fixture.summary, "codex")
    expect(receipts).toHaveLength(1)
    expect(receipts[0]!.receiptId).toBe(receipt.receiptId)
    const pin = await loadHarnessMaintenancePin(fixture.summary, "codex")
    expect(pin?.expectedVersion).toBe("0.44.0")

    // The default ~/.codex login home is byte-identical after the whole flow.
    expect(await snapshotCodexHome(fixture)).toBe(codexHomeBefore)

    // Public-safe projection: home prefix collapses to ~ and passes the guard.
    const projected = projectPublicHarnessMaintenanceReceipt(receipt, { home: fixture.root })
    expect(JSON.stringify(projected)).not.toContain(fixture.root)
  })

  test("already current: no update command runs, receipt says so", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.45.0",
      latestVersion: "0.45.0",
      updateBehavior: "break-binary",
    })
    const receipt = await runHarnessMaintenanceUpdate({ harness: "codex", deps: fixture.deps })
    expect(receipt.outcome).toBe("already_current")
    expect(receipt.update).toBeNull()
    // The destructive fake npm never ran: the binary still answers.
    const after = await detectHarnessInstall("codex", fixture.deps)
    expect(after.installedVersion).toBe("0.45.0")
  })

  test("failure path: post-update probe fails → maintenance failure, previous state in receipt", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
      updateBehavior: "break-binary",
    })
    const codexHomeBefore = await snapshotCodexHome(fixture)
    const receipt = await runHarnessMaintenanceUpdate({ harness: "codex", deps: fixture.deps })
    expect(receipt.outcome).toBe("failed")
    expect(receipt.failureReason).toBe("post_update_probe_failed")
    // The receipt records the intact previous state and the failing re-probe.
    expect(receipt.before.installedVersion).toBe("0.44.0")
    expect(receipt.before.probeOk).toBe(true)
    expect(receipt.after?.probeOk).toBe(false)
    expect(receipt.pin?.expectedVersion).toBe("0.44.0")
    await persistHarnessMaintenanceReceipt(fixture.summary, receipt)
    const receipts = await loadHarnessMaintenanceReceipts(fixture.summary, "codex")
    expect(receipts[0]!.outcome).toBe("failed")
    expect(await snapshotCodexHome(fixture)).toBe(codexHomeBefore)
  })

  test("failure path: update command fails → previous binary intact and still answering", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
      updateBehavior: "fail",
    })
    const receipt = await runHarnessMaintenanceUpdate({ harness: "codex", deps: fixture.deps })
    expect(receipt.outcome).toBe("failed")
    expect(receipt.failureReason).toBe("update_command_failed")
    expect(receipt.update?.exitCode).toBe(1)
    expect(receipt.update?.outputExcerpt).toContain("npm ERR!")
    const after = await detectHarnessInstall("codex", fixture.deps)
    expect(after.installedVersion).toBe("0.44.0")
    expect(after.probeOk).toBe(true)
  })

  test("failure path: update succeeds but version does not move → honest failure", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
      updateBehavior: "noop",
    })
    const receipt = await runHarnessMaintenanceUpdate({ harness: "codex", deps: fixture.deps })
    expect(receipt.outcome).toBe("failed")
    expect(receipt.failureReason).toBe("version_unchanged_after_update")
  })

  test("channel-jump refusal: explicit different channel is refused without allowChannelJump", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
      updateBehavior: "break-binary",
    })
    const receipt = await runHarnessMaintenanceUpdate({
      harness: "codex",
      channel: "homebrew",
      deps: fixture.deps,
    })
    expect(receipt.outcome).toBe("channel_jump_refused")
    expect(receipt.update).toBeNull()
    expect(receipt.after).toBeNull()
    // Nothing executed: binary intact.
    const after = await detectHarnessInstall("codex", fixture.deps)
    expect(after.installedVersion).toBe("0.44.0")
  })

  test("not installed and unknown channel are typed failures", async () => {
    const root = await makeFixtureRoot()
    const emptyBin = join(root, "empty-bin")
    await mkdir(emptyBin, { recursive: true })
    const missing = await runHarnessMaintenanceUpdate({
      harness: "opencode",
      deps: { env: { PATH: emptyBin }, fetchLatestVersion: async () => "1.0.0" },
    })
    expect(missing.outcome).toBe("failed")
    expect(missing.failureReason).toBe("not_installed")

    // A binary in an unclassifiable location refuses one-click updates.
    const strayBin = join(root, "stray")
    await mkdir(strayBin, { recursive: true })
    const strayCodex = join(strayBin, "codex")
    await writeFile(strayCodex, `#!/bin/sh\necho "codex-cli 0.44.0"\n`, "utf8")
    await chmod(strayCodex, 0o755)
    const unknown = await runHarnessMaintenanceUpdate({
      harness: "codex",
      deps: { env: { PATH: strayBin }, fetchLatestVersion: async () => "9.9.9" },
    })
    expect(unknown.outcome).toBe("failed")
    expect(unknown.failureReason).toBe("unsupported_channel")
  })

  test("status projection reports version, channel, advisory, and update support", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
    })
    const status = await collectHarnessMaintenanceStatus(fixture.deps, ["codex"])
    expect(status.harnesses).toHaveLength(1)
    const codex = status.harnesses[0]!
    expect(codex.installedVersion).toBe("0.44.0")
    expect(codex.latestVersion).toBe("0.45.0")
    expect(codex.advisory).toBe("behind_latest")
    expect(codex.channel).toBe("npm-global")
    expect(codex.updateSupported).toBe(true)
    expect(codex.updateCommand?.executable).toBe("npm")
  })

  test("receipts JSONL tolerates torn lines and appends across runs", async () => {
    const fixture = await makeFixtureHarness({
      installedVersion: "0.44.0",
      latestVersion: "0.45.0",
    })
    const first = await runHarnessMaintenanceUpdate({ harness: "codex", deps: fixture.deps })
    await persistHarnessMaintenanceReceipt(fixture.summary, first)
    const path = harnessMaintenanceReceiptsPath(fixture.summary, "codex")
    const raw = await readFile(path, "utf8")
    await writeFile(path, `${raw}{"torn`, "utf8")
    const second: PylonHarnessMaintenanceReceipt = { ...first, receiptId: "hmr-second" }
    await writeFile(path, `${await readFile(path, "utf8")}\n${JSON.stringify(second)}\n`, "utf8")
    const receipts = await loadHarnessMaintenanceReceipts(fixture.summary, "codex")
    expect(receipts.map((entry) => entry.receiptId)).toEqual([first.receiptId, "hmr-second"])
  })
})
