import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"

import { isMonotonicUpgrade, type UpdateChannel } from "./update-contract.ts"

export const MACOS_UPDATE_TRANSACTION_SCHEMA = "openagents.desktop.macos_update_transaction.v1" as const

export type MacOSUpdateFailureReason =
  | "unsupported_platform"
  | "not_packaged"
  | "artifact_missing"
  | "mount_failed"
  | "candidate_missing"
  | "identity_mismatch"
  | "version_mismatch"
  | "candidate_not_monotonic"
  | "signature_invalid"
  | "notarization_invalid"
  | "backup_failed"
  | "install_failed"
  | "rollback_unavailable"
  | "rollback_failed"

export type MacOSUpdateResult =
  | Readonly<{ ok: true; action: "installed" | "rolled_back"; installedVersion: string; previousVersion: string | null }>
  | Readonly<{ ok: false; reason: MacOSUpdateFailureReason }>

type TransactionDocument = Readonly<{
  schema: typeof MACOS_UPDATE_TRANSACTION_SCHEMA
  status: "installed" | "rolled_back"
  previousVersion: string | null
  installedVersion: string
  channel: UpdateChannel
}>

export type UpdateCommandResult = Readonly<{ exitCode: number; stdout: string; stderr: string }>
export type UpdateCommandRunner = (executable: string, args: ReadonlyArray<string>) => Promise<UpdateCommandResult>

const defaultRunner: UpdateCommandRunner = async (executable, args) => {
  const child = Bun.spawn([executable, ...args], { stdout: "pipe", stderr: "pipe" })
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ])
  return { exitCode, stdout, stderr }
}

const safeVersion = (value: unknown): value is string =>
  typeof value === "string" && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/.test(value)

const readTransaction = (file: string): TransactionDocument | null => {
  try {
    const value = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>
    if (value.schema !== MACOS_UPDATE_TRANSACTION_SCHEMA ||
      (value.status !== "installed" && value.status !== "rolled_back") ||
      (value.channel !== "stable" && value.channel !== "rc") ||
      !safeVersion(value.installedVersion) ||
      (value.previousVersion !== null && !safeVersion(value.previousVersion))) return null
    return value as TransactionDocument
  } catch {
    return null
  }
}

const writeTransaction = (file: string, value: TransactionDocument): void => {
  mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
  const temporary = `${file}.tmp`
  writeFileSync(temporary, `${JSON.stringify(value)}\n`, { encoding: "utf8", mode: 0o600 })
  if (process.platform !== "win32") chmodSync(temporary, 0o600)
  renameSync(temporary, file)
}

const output = (result: UpdateCommandResult): string => `${result.stdout}\n${result.stderr}`

export type MacOSUpdateApplier = Readonly<{
  rollbackAvailable: () => boolean
  install: (artifactPath: string, candidateVersion: string) => Promise<MacOSUpdateResult>
  rollback: () => Promise<MacOSUpdateResult>
}>

export const openMacOSUpdateApplier = (input: Readonly<{
  root: string
  installedAppPath: string
  installedVersion: string
  channel: UpdateChannel
  platform?: NodeJS.Platform
  packaged: boolean
  run?: UpdateCommandRunner
  bundleId?: string
  teamId?: string
}>): MacOSUpdateApplier => {
  const run = input.run ?? defaultRunner
  const platform = input.platform ?? process.platform
  const bundleId = input.bundleId ?? "com.openagents.desktop"
  const teamId = input.teamId ?? "HQWSG26L43"
  const transactionFile = path.join(input.root, "apply-transaction.json")
  const rollbackApp = path.join(input.root, "rollback", "OpenAgents.app")
  const mountpoint = path.join(input.root, "mount")
  let transaction = readTransaction(transactionFile)

  const commandOk = async (executable: string, args: ReadonlyArray<string>): Promise<UpdateCommandResult | null> => {
    try {
      const result = await run(executable, args)
      return result.exitCode === 0 ? result : null
    } catch {
      return null
    }
  }

  const verifyApp = async (
    appPath: string,
    expectedVersion: string,
  ): Promise<MacOSUpdateFailureReason | null> => {
    if (!existsSync(appPath)) return "candidate_missing"
    const identifier = await commandOk("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleIdentifier", path.join(appPath, "Contents", "Info.plist")])
    const version = await commandOk("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleShortVersionString", path.join(appPath, "Contents", "Info.plist")])
    if (identifier === null || identifier.stdout.trim() !== bundleId) return "identity_mismatch"
    if (version === null || version.stdout.trim() !== expectedVersion) return "version_mismatch"
    const signature = await commandOk("/usr/bin/codesign", ["--verify", "--deep", "--strict", "--verbose=2", appPath])
    if (signature === null) return "signature_invalid"
    const identity = await commandOk("/usr/bin/codesign", ["-dv", "--verbose=4", appPath])
    const identityText = identity === null ? "" : output(identity)
    if (!identityText.includes(`TeamIdentifier=${teamId}`) ||
      !identityText.includes(`Authority=Developer ID Application: OpenAgents, Inc. (${teamId})`)) return "identity_mismatch"
    if (await commandOk("/usr/sbin/spctl", ["--assess", "--type", "execute", "--verbose=2", appPath]) === null ||
      await commandOk("/usr/bin/xcrun", ["stapler", "validate", appPath]) === null) return "notarization_invalid"
    return null
  }

  const copyApp = async (source: string, destination: string): Promise<boolean> => {
    rmSync(destination, { recursive: true, force: true })
    mkdirSync(path.dirname(destination), { recursive: true, mode: 0o700 })
    return await commandOk("/usr/bin/ditto", ["--rsrc", "--extattr", source, destination]) !== null && existsSync(destination)
  }

  const swapIntoPlace = async (source: string, expectedVersion: string): Promise<boolean> => {
    const target = path.resolve(input.installedAppPath)
    const parent = path.dirname(target)
    const incoming = path.join(parent, ".OpenAgents.update.app")
    const displaced = path.join(parent, ".OpenAgents.displaced.app")
    rmSync(incoming, { recursive: true, force: true })
    rmSync(displaced, { recursive: true, force: true })
    if (!await copyApp(source, incoming) || await verifyApp(incoming, expectedVersion) !== null) {
      rmSync(incoming, { recursive: true, force: true })
      return false
    }
    try {
      renameSync(target, displaced)
      try {
        renameSync(incoming, target)
        if (await verifyApp(target, expectedVersion) !== null) {
          rmSync(target, { recursive: true, force: true })
          renameSync(displaced, target)
          return false
        }
      } catch {
        renameSync(displaced, target)
        throw new Error("swap_failed")
      }
      rmSync(displaced, { recursive: true, force: true })
      return true
    } catch {
      rmSync(incoming, { recursive: true, force: true })
      return false
    }
  }

  const supported = (): MacOSUpdateFailureReason | null =>
    platform !== "darwin" ? "unsupported_platform" : !input.packaged ? "not_packaged" : null

  const rollbackAvailable = (): boolean =>
    supported() === null && transaction?.status === "installed" &&
    transaction.installedVersion === input.installedVersion &&
    transaction.channel === input.channel &&
    transaction.previousVersion !== null && existsSync(rollbackApp)

  const install = async (artifactPath: string, candidateVersion: string): Promise<MacOSUpdateResult> => {
    const unsupported = supported()
    if (unsupported !== null) return { ok: false, reason: unsupported }
    if (!existsSync(artifactPath)) return { ok: false, reason: "artifact_missing" }
    if (!safeVersion(candidateVersion)) return { ok: false, reason: "version_mismatch" }
    if (!isMonotonicUpgrade(input.installedVersion, candidateVersion, input.channel).admissible) {
      return { ok: false, reason: "candidate_not_monotonic" }
    }
    rmSync(mountpoint, { recursive: true, force: true })
    mkdirSync(path.dirname(mountpoint), { recursive: true, mode: 0o700 })
    const mounted = await commandOk("/usr/bin/hdiutil", ["attach", "-readonly", "-nobrowse", "-mountpoint", mountpoint, artifactPath])
    if (mounted === null) return { ok: false, reason: "mount_failed" }
    try {
      const candidateApp = path.join(mountpoint, "OpenAgents.app")
      const verification = await verifyApp(candidateApp, candidateVersion)
      if (verification !== null) return { ok: false, reason: verification }
      const currentVerification = await verifyApp(input.installedAppPath, input.installedVersion)
      if (currentVerification !== null) return { ok: false, reason: currentVerification }
      if (!await copyApp(input.installedAppPath, rollbackApp)) return { ok: false, reason: "backup_failed" }
      if (!await swapIntoPlace(candidateApp, candidateVersion)) {
        rmSync(rollbackApp, { recursive: true, force: true })
        return { ok: false, reason: "install_failed" }
      }
      transaction = {
        schema: MACOS_UPDATE_TRANSACTION_SCHEMA,
        status: "installed",
        previousVersion: input.installedVersion,
        installedVersion: candidateVersion,
        channel: input.channel,
      }
      writeTransaction(transactionFile, transaction)
      return { ok: true, action: "installed", installedVersion: candidateVersion, previousVersion: input.installedVersion }
    } finally {
      await commandOk("/usr/bin/hdiutil", ["detach", mountpoint])
      rmSync(mountpoint, { recursive: true, force: true })
    }
  }

  const rollback = async (): Promise<MacOSUpdateResult> => {
    const unsupported = supported()
    if (unsupported !== null) return { ok: false, reason: unsupported }
    if (!rollbackAvailable() || transaction?.previousVersion === null || transaction === null) {
      return { ok: false, reason: "rollback_unavailable" }
    }
    const previousVersion = transaction.previousVersion
    const verification = await verifyApp(rollbackApp, previousVersion)
    if (verification !== null) return { ok: false, reason: "rollback_failed" }
    if (!await swapIntoPlace(rollbackApp, previousVersion)) return { ok: false, reason: "rollback_failed" }
    rmSync(rollbackApp, { recursive: true, force: true })
    transaction = {
      schema: MACOS_UPDATE_TRANSACTION_SCHEMA,
      status: "rolled_back",
      previousVersion: null,
      installedVersion: previousVersion,
      channel: input.channel,
    }
    writeTransaction(transactionFile, transaction)
    return { ok: true, action: "rolled_back", installedVersion: previousVersion, previousVersion: null }
  }

  return { rollbackAvailable, install, rollback }
}
