import { afterEach, describe, expect, test } from "vite-plus/test"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import { execFileSync } from "node:child_process"
import { tmpdir } from "node:os"
import path from "node:path"

import { attachedDiskDevice, MACOS_ATOMIC_REPLACE_JXA, MACOS_FSYNC_JXA, MACOS_UPDATE_TRANSACTION_SCHEMA, openMacOSUpdateApplier, type UpdateCommandRunner } from "./macos-update-applier.ts"

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
  let transactionAtAtomicReplace: unknown = null
  const run: UpdateCommandRunner = async (executable, args) => {
    const name = path.basename(executable)
    calls.push(`${name} ${args.join(" ")}`)
    if (fail !== null && ((name === fail && !(name === "osascript" && args[3] !== MACOS_ATOMIC_REPLACE_JXA)) || (fail === "syspolicy_check" && args[0] === "syspolicy_check"))) {
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
      return { exitCode: 0, stdout: args[1]?.includes("CFBundleIdentifier") ? "com.openagents.desktop\n" : args[1]?.includes("CFBundleExecutable") ? "OpenAgents\n" : `${existsSync(path.join(app, "Contents", "candidate")) ? "0.1.0-rc.6" : "0.1.0-rc.5"}\n`, stderr: "" }
    }
    if (name === "lipo") return { exitCode: 0, stdout: "arm64\n", stderr: "" }
    if (name === "codesign" && args[0] === "-dv") return { exitCode: 0, stdout: "", stderr: "Authority=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)\nTeamIdentifier=HQWSG26L43\n" }
    if (name === "osascript") {
      if (args[3] === MACOS_FSYNC_JXA) return { exitCode: 0, stdout: "", stderr: "" }
      const transactionFile = ["updates", "fallback-updates"].map(directory => path.join(root, directory, "apply-transaction.json")).find(existsSync)
      transactionAtAtomicReplace = transactionFile === undefined ? null : JSON.parse(readFileSync(transactionFile, "utf8"))
      const target = args.at(-2)!
      const incoming = args.at(-1)!
      rmSync(target, { recursive: true, force: true })
      renameSync(incoming, target)
    }
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
  return { root, installed, artifact, calls, applier, run, transactionAtAtomicReplace: () => transactionAtAtomicReplace }
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
    const atomicIndex = h.calls.findIndex(call => call.includes(MACOS_ATOMIC_REPLACE_JXA))
    const transactionFsyncIndex = h.calls.findIndex(call => call.includes(MACOS_FSYNC_JXA) && call.includes("apply-transaction.json"))
    const parentFsyncIndex = h.calls.findIndex((call, index) => index > transactionFsyncIndex && call.includes(MACOS_FSYNC_JXA) && call.endsWith(path.join(h.root, "updates")))
    expect(transactionFsyncIndex).toBeGreaterThan(-1)
    expect(parentFsyncIndex).toBeGreaterThan(transactionFsyncIndex)
    expect(parentFsyncIndex).toBeLessThan(atomicIndex)
    const postReplaceParentFsyncIndex = h.calls.findIndex((call, index) => index > atomicIndex && call.includes(MACOS_FSYNC_JXA) && call.endsWith(path.dirname(h.installed)))
    expect(postReplaceParentFsyncIndex).toBeGreaterThan(atomicIndex)
    expect(h.calls[atomicIndex]).toContain("undefined, 0, undefined")
    expect(h.transactionAtAtomicReplace()).toMatchObject({ status: "prepared", previousVersion: "0.1.0-rc.5", installedVersion: "0.1.0-rc.6" })
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

    const swap = fixture("osascript")
    expect(await swap.applier().install(swap.artifact, "0.1.0-rc.6")).toEqual({ ok: false, reason: "install_failed" })
    expect(existsSync(path.join(swap.installed, "Contents", "current"))).toBe(true)
    expect(existsSync(path.join(swap.root, "updates", "rollback", "OpenAgents.app"))).toBe(true)
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

  test("arms a detached first-launch watchdog for the exact retained slot", async () => {
    const h = fixture()
    expect((await h.applier().install(h.artifact, "0.1.0-rc.6")).ok).toBe(true)
    const spawns: Array<{ executable: string; args: ReadonlyArray<string> }> = []
    const restarted = openMacOSUpdateApplier({ root: path.join(h.root, "updates"), installedAppPath: h.installed, installedVersion: "0.1.0-rc.6", channel: "rc", platform: "darwin", packaged: true, run: h.run, spawnWatchdog: (executable, args) => { spawns.push({ executable, args }); return true } })
    const receiptPath = path.join(h.root, "updates", "launch-receipt.json")
    expect(await restarted.armFirstLaunchRollback({ receiptPath, expectedVersion: "0.1.0-rc.6", transactionRef: "a".repeat(32), previousVersion: "0.1.0-rc.5", previousArchitecture: "arm64", deadlineMs: 1_000_000 })).toBe(true)
    expect(spawns[0]).toMatchObject({ executable: "/bin/sh" })
    expect(spawns[0]!.args).toContain(receiptPath)
    expect(readFileSync(path.join(h.root, "updates", "first-launch-watchdog.sh"), "utf8")).toContain("rolled_back")
    const script = readFileSync(path.join(h.root, "updates", "first-launch-watchdog.sh"), "utf8")
    expect(script).toContain("plutil -extract transactionRef")
    expect(script).toContain("codesign --verify --deep --strict")
    expect(script).toContain('"status":"rollback_prepared"')
    expect(script).toContain("replaceItemAtURLWithItemAtURLBackupItemNameOptionsResultingItemURLError")
    expect(script).toContain('fsync_native "$transaction_tmp"')
    expect(script).toContain('fsync_native "$(/usr/bin/dirname "$transaction_file")"')
    expect(script).toContain("synchronizeFile")
    expect(script.indexOf('fsync_native "$(/usr/bin/dirname "$installed_app")"', script.indexOf(MACOS_ATOMIC_REPLACE_JXA))).toBeLessThan(script.indexOf('"status":"rolled_back"'))
    expect(script.indexOf('fsync_native "$diagnostic_tmp"')).toBeLessThan(script.indexOf('/bin/rm -rf "$rollback_app"'))
    expect(script.indexOf('fsync_native "$(/usr/bin/dirname "$diagnostic")"')).toBeLessThan(script.indexOf('/bin/rm -rf "$rollback_app"'))
    expect(script).not.toContain('/bin/mv "$installed_app"')
    expect(script).not.toContain('grep -Fq "\\\\\"version')
    writeFileSync(receiptPath, JSON.stringify({ schema: "openagents.desktop.launch_health.v1", app: "openagents-desktop", version: "0.1.0-rc.6", transactionRef: "a".repeat(32), rendererReadyAt: "not-an-instant", providerReadyAt: "2026-07-16T10:00:01.000Z", cleanShutdownAt: "2026-07-16T10:00:02.000Z" }))
    const malformedArgs = [...spawns[0]!.args]
    malformedArgs[3] = String(Math.floor(Date.now() / 1000) + 1)
    expect(() => execFileSync("/bin/sh", malformedArgs, { stdio: "ignore" })).toThrow()
    expect(readFileSync(path.join(h.root, "updates", "first-launch-watchdog.result"), "utf8").trim()).toBe("rollback_failed")
  })

  test("refuses to arm when the retained slot fails immediate native verification", async () => {
    const h = fixture()
    expect((await h.applier().install(h.artifact, "0.1.0-rc.6")).ok).toBe(true)
    let spawned = false
    const tampered = openMacOSUpdateApplier({ root: path.join(h.root, "updates"), installedAppPath: h.installed, installedVersion: "0.1.0-rc.6", channel: "rc", platform: "darwin", packaged: true,
      run: async (executable, args) => path.basename(executable) === "codesign" && args[0] === "--verify" ? { exitCode: 1, stdout: "", stderr: "tampered" } : h.run(executable, args),
      spawnWatchdog: () => { spawned = true; return true },
    })
    expect(await tampered.armFirstLaunchRollback({ receiptPath: path.join(h.root, "receipt.json"), expectedVersion: "0.1.0-rc.6", transactionRef: "b".repeat(32), previousVersion: "0.1.0-rc.5", previousArchitecture: "arm64", deadlineMs: 1_000_000 })).toBe(false)
    expect(spawned).toBe(false)
  })

  test("does not mutate live app paths while reopening durable prepared state", async () => {
    const interrupted = fixture()
    expect((await interrupted.applier().install(interrupted.artifact, "0.1.0-rc.6")).ok).toBe(true)
    const updates = path.join(interrupted.root, "updates")
    writeFileSync(path.join(updates, "apply-transaction.json"), JSON.stringify({ schema: MACOS_UPDATE_TRANSACTION_SCHEMA, status: "prepared", previousVersion: "0.1.0-rc.5", installedVersion: "0.1.0-rc.6", channel: "rc" }))
    const recovered = openMacOSUpdateApplier({ root: updates, installedAppPath: interrupted.installed, installedVersion: "0.1.0-rc.6", channel: "rc", platform: "darwin", packaged: true, run: interrupted.run })
    expect(existsSync(interrupted.installed)).toBe(true)
    expect(recovered.rollbackAvailable()).toBe(true)

    const unbound = fixture()
    expect((await unbound.applier().install(unbound.artifact, "0.1.0-rc.6")).ok).toBe(true)
    const unboundUpdates = path.join(unbound.root, "updates")
    rmSync(path.join(unboundUpdates, "apply-transaction.json"))
    const unboundDisplaced = path.join(path.dirname(unbound.installed), ".OpenAgents.displaced.app")
    renameSync(unbound.installed, unboundDisplaced)
    const refused = openMacOSUpdateApplier({ root: unboundUpdates, installedAppPath: unbound.installed, installedVersion: "0.1.0-rc.6", channel: "rc", platform: "darwin", packaged: true, run: unbound.run })
    expect(existsSync(unbound.installed)).toBe(false)
    expect(existsSync(unboundDisplaced)).toBe(true)
    expect(refused.rollbackAvailable()).toBe(false)
  })

  test("executes exact native file/parent fsyncs before Foundation atomic replacement on macOS", () => {
    if (process.platform !== "darwin") return
    const root = mkdtempSync(path.join(tmpdir(), "oa-native-replace-")); roots.push(root)
    const target = path.join(root, "OpenAgents.app")
    const incoming = path.join(root, "OpenAgents.update.app")
    mkdirSync(target); mkdirSync(incoming)
    writeFileSync(path.join(target, "old"), "old"); writeFileSync(path.join(incoming, "new"), "new")
    execFileSync("/usr/bin/osascript", ["-l", "JavaScript", "-e", MACOS_FSYNC_JXA, path.join(incoming, "new")])
    execFileSync("/usr/bin/osascript", ["-l", "JavaScript", "-e", MACOS_FSYNC_JXA, root])
    execFileSync("/usr/bin/osascript", ["-l", "JavaScript", "-e", MACOS_ATOMIC_REPLACE_JXA, target, incoming])
    execFileSync("/usr/bin/osascript", ["-l", "JavaScript", "-e", MACOS_FSYNC_JXA, root])
    expect(existsSync(path.join(target, "new"))).toBe(true)
    expect(existsSync(path.join(target, "old"))).toBe(false)
    expect(existsSync(incoming)).toBe(false)
  })
})
