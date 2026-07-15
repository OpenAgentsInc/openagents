import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { attachedDiskDevice, MACOS_UPDATE_TRANSACTION_SCHEMA, openMacOSUpdateApplier, type UpdateCommandRunner } from "./macos-update-applier.ts"

const roots: string[] = []
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

const fixture = (fail: string | null = null) => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-update-apply-")); roots.push(root)
  const installed = path.join(root, "Applications", "OpenAgents.app")
  const artifact = path.join(root, "OpenAgents-0.1.0-rc.6-arm64.dmg")
  mkdirSync(path.join(installed, "Contents"), { recursive: true })
  writeFileSync(path.join(installed, "Contents", "current"), "rc5")
  writeFileSync(artifact, "verified-dmg")
  const calls: string[] = []
  let dittoCalls = 0
  const run: UpdateCommandRunner = async (executable, args) => {
    const name = path.basename(executable)
    calls.push(`${name} ${args.join(" ")}`)
    if (fail !== null && (name === fail || (fail === "syspolicy_check" && args[0] === "syspolicy_check"))) {
      return { exitCode: 1, stdout: "", stderr: "refused" }
    }
    if (name === "ditto" && fail === "second_ditto" && ++dittoCalls === 2) return { exitCode: 1, stdout: "", stderr: "refused" }
    if (name === "hdiutil" && args[0] === "attach") {
      const mountpoint = args[args.indexOf("-mountpoint") + 1]!
      mkdirSync(path.join(mountpoint, "OpenAgents.app", "Contents"), { recursive: true })
      writeFileSync(path.join(mountpoint, "OpenAgents.app", "Contents", "candidate"), "rc6")
      return { exitCode: 0, stdout: `/dev/disk42\tGUID_partition_scheme\n/dev/disk42s1\t${mountpoint}\n`, stderr: "" }
    }
    if (name === "PlistBuddy") {
      const app = path.dirname(path.dirname(args[2]!))
      return { exitCode: 0, stdout: args[1]?.includes("CFBundleIdentifier") ? "com.openagents.desktop\n" : `${existsSync(path.join(app, "Contents", "candidate")) ? "0.1.0-rc.6" : "0.1.0-rc.5"}\n`, stderr: "" }
    }
    if (name === "codesign" && args[0] === "-dv") return { exitCode: 0, stdout: "", stderr: "Authority=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)\nTeamIdentifier=HQWSG26L43\n" }
    if (name === "ditto") {
      const source = args.at(-2)!
      const destination = args.at(-1)!
      mkdirSync(destination, { recursive: true })
      const marker = existsSync(path.join(source, "Contents", "candidate")) ? "candidate" : "current"
      mkdirSync(path.join(destination, "Contents"), { recursive: true })
      writeFileSync(path.join(destination, "Contents", marker), marker)
    }
    return { exitCode: 0, stdout: "", stderr: "" }
  }
  const applier = () => openMacOSUpdateApplier({ root: path.join(root, "updates"), installedAppPath: installed, installedVersion: "0.1.0-rc.5", channel: "rc", platform: "darwin", packaged: true, run })
  return { root, installed, artifact, calls, applier, run }
}

const existsSync = (file: string): boolean => {
  try { readFileSync(file); return true } catch { return false }
}

describe("macOS signed update applier", () => {
  test("verifies, retains the current signed app, swaps the candidate, and persists a private rollback transaction", async () => {
    const h = fixture()
    const result = await h.applier().install(h.artifact, "0.1.0-rc.6")
    expect(result).toEqual({ ok: true, action: "installed", installedVersion: "0.1.0-rc.6", previousVersion: "0.1.0-rc.5" })
    expect(existsSync(path.join(h.installed, "Contents", "candidate"))).toBe(true)
    expect(existsSync(path.join(h.root, "updates", "rollback", "OpenAgents.app", "Contents", "current"))).toBe(true)
    expect(h.calls.some(call => call.startsWith("codesign --verify --deep --strict"))).toBe(true)
    expect(h.calls.some(call => call.startsWith("xcrun --find syspolicy_check"))).toBe(true)
    expect(h.calls.some(call => call.startsWith("xcrun syspolicy_check distribution"))).toBe(true)
    expect(h.calls.some(call => call.startsWith("spctl --assess"))).toBe(false)
    expect(h.calls.some(call => call.startsWith("xcrun stapler validate"))).toBe(true)
    expect(h.calls).toContain("hdiutil detach /dev/disk42")
    expect(JSON.parse(readFileSync(path.join(h.root, "updates", "apply-transaction.json"), "utf8"))).toEqual({ schema: MACOS_UPDATE_TRANSACTION_SCHEMA, status: "installed", previousVersion: "0.1.0-rc.5", installedVersion: "0.1.0-rc.6", channel: "rc" })
  })

  test("extracts the whole-disk detach target and refuses unrelated output", () => {
    expect(attachedDiskDevice({ exitCode: 0, stdout: "/dev/disk9\tGUID_partition_scheme\n/dev/disk9s1\t/private/tmp/mount\n", stderr: "" })).toBe("/dev/disk9")
    expect(attachedDiskDevice({ exitCode: 0, stdout: "attached without a device receipt", stderr: "" })).toBeNull()
  })

  test("restart discovers and consumes exactly one retained rollback slot", async () => {
    const h = fixture()
    expect((await h.applier().install(h.artifact, "0.1.0-rc.6")).ok).toBe(true)
    const restarted = openMacOSUpdateApplier({ root: path.join(h.root, "updates"), installedAppPath: h.installed, installedVersion: "0.1.0-rc.6", channel: "rc", platform: "darwin", packaged: true, run: async (executable, args) => {
      const name = path.basename(executable)
      h.calls.push(`${name} ${args.join(" ")}`)
      if (name === "PlistBuddy") return { exitCode: 0, stdout: args[1]?.includes("CFBundleIdentifier") ? "com.openagents.desktop\n" : "0.1.0-rc.5\n", stderr: "" }
      if (name === "codesign" && args[0] === "-dv") return { exitCode: 0, stdout: "", stderr: "Authority=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)\nTeamIdentifier=HQWSG26L43\n" }
      if (name === "ditto") {
        const destination = args.at(-1)!; mkdirSync(path.join(destination, "Contents"), { recursive: true }); writeFileSync(path.join(destination, "Contents", "current"), "rc5")
      }
      return { exitCode: 0, stdout: "", stderr: "" }
    } })
    expect(restarted.rollbackAvailable()).toBe(true)
    expect(restarted.rollbackVersion()).toBe("0.1.0-rc.5")
    expect(await restarted.rollback()).toEqual({ ok: true, action: "rolled_back", installedVersion: "0.1.0-rc.5", previousVersion: null })
    expect(restarted.rollbackAvailable()).toBe(false)
    expect(await restarted.rollback()).toEqual({ ok: false, reason: "rollback_unavailable" })
  })

  test("fails closed before mutation on identity/notarization failure and restores the running app on swap failure", async () => {
    const identity = fixture("syspolicy_check")
    expect(await identity.applier().install(identity.artifact, "0.1.0-rc.6")).toEqual({ ok: false, reason: "notarization_invalid" })
    expect(existsSync(path.join(identity.installed, "Contents", "current"))).toBe(true)
    expect(existsSync(path.join(identity.root, "updates", "rollback", "OpenAgents.app"))).toBe(false)

    const unsupported = openMacOSUpdateApplier({ root: identity.root, installedAppPath: identity.installed, installedVersion: "0.1.0-rc.5", channel: "rc", platform: "linux", packaged: true, run: async () => ({ exitCode: 0, stdout: "", stderr: "" }) })
    expect(await unsupported.install(identity.artifact, "0.1.0-rc.6")).toEqual({ ok: false, reason: "unsupported_platform" })
    expect(await identity.applier().install(identity.artifact, "0.1.0-rc.5")).toEqual({ ok: false, reason: "candidate_not_monotonic" })

    const swap = fixture("second_ditto")
    expect(await swap.applier().install(swap.artifact, "0.1.0-rc.6")).toEqual({ ok: false, reason: "install_failed" })
    expect(existsSync(path.join(swap.installed, "Contents", "current"))).toBe(true)
    expect(existsSync(path.join(swap.root, "updates", "rollback", "OpenAgents.app"))).toBe(false)
  })

  test("falls back to legacy Gatekeeper assessment only when syspolicy_check is unavailable", async () => {
    const h = fixture()
    const fallback = openMacOSUpdateApplier({
      root: path.join(h.root, "fallback-updates"),
      installedAppPath: h.installed,
      installedVersion: "0.1.0-rc.5",
      channel: "rc",
      platform: "darwin",
      packaged: true,
      run: async (executable, args) => {
        if (path.basename(executable) === "xcrun" && args[0] === "--find") {
          return { exitCode: 1, stdout: "", stderr: "unable to find utility" }
        }
        return await h.run(executable, args)
      },
    })
    expect((await fallback.install(h.artifact, "0.1.0-rc.6")).ok).toBe(true)
    expect(h.calls.some(call => call.startsWith("spctl --assess"))).toBe(true)
  })
})
