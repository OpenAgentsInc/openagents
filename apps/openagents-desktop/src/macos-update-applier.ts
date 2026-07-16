import { chmodSync, closeSync, existsSync, fsyncSync, mkdirSync, openSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"
import { spawn } from "node:child_process"

import { isMonotonicUpgrade, type UpdateChannel } from "./update-contract.ts"
import type { DesktopPlatformUpdateApplier } from "./update-platform-applier.ts"

export const MACOS_UPDATE_TRANSACTION_SCHEMA = "openagents.desktop.macos_update_transaction.v1" as const
export const MACOS_ATOMIC_REPLACE_JXA = "ObjC.import('Foundation'); function run(argv) { const manager = $.NSFileManager.defaultManager; const error = Ref(); const result = manager.replaceItemAtURLWithItemAtURLBackupItemNameOptionsResultingItemURLError($.NSURL.fileURLWithPath(argv[0]), $.NSURL.fileURLWithPath(argv[1]), undefined, 0, undefined, error); if (!result) throw new Error('atomic_replace_failed'); }"
export const MACOS_FSYNC_JXA = "ObjC.import('Foundation'); function run(argv) { const handle = $.NSFileHandle.fileHandleForReadingAtPath(argv[0]); if (!handle) throw new Error('fsync_open_failed'); handle.synchronizeFile; handle.closeFile; }"

export type MacOSUpdateFailureReason =
  | "unsupported_platform"
  | "not_packaged"
  | "artifact_missing"
  | "mount_failed"
  | "candidate_missing"
  | "identity_mismatch"
  | "version_mismatch"
  | "architecture_mismatch"
  | "candidate_not_monotonic"
  | "signature_invalid"
  | "notarization_invalid"
  | "backup_failed"
  | "install_failed"
  | "watchdog_failed"
  | "rollback_unavailable"
  | "rollback_failed"

export type MacOSUpdateResult =
  | Readonly<{ ok: true; action: "installed" | "rolled_back"; installedVersion: string; previousVersion: string | null }>
  | Readonly<{ ok: false; reason: MacOSUpdateFailureReason }>

type TransactionDocument = Readonly<{
  schema: typeof MACOS_UPDATE_TRANSACTION_SCHEMA
  status: "prepared" | "installed" | "rollback_prepared" | "rolled_back"
  previousVersion: string | null
  installedVersion: string
  channel: UpdateChannel
}>

export type UpdateCommandResult = Readonly<{ exitCode: number; stdout: string; stderr: string }>
export type UpdateCommandRunner = (executable: string, args: ReadonlyArray<string>) => Promise<UpdateCommandResult>

const defaultRunner: UpdateCommandRunner = async (executable, args) => {
  return await new Promise<UpdateCommandResult>((resolve, reject) => {
    const child = spawn(executable, [...args], { stdio: ["ignore", "pipe", "pipe"] })
    let stdout = ""
    let stderr = ""
    child.stdout.setEncoding("utf8")
    child.stderr.setEncoding("utf8")
    child.stdout.on("data", (chunk: string) => {
      stdout += chunk
    })
    child.stderr.on("data", (chunk: string) => {
      stderr += chunk
    })
    child.once("error", reject)
    child.once("close", (code) => resolve({ exitCode: code ?? 1, stdout, stderr }))
  })
}

const safeVersion = (value: unknown): value is string =>
  typeof value === "string" && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/.test(value)

const readTransaction = (file: string): TransactionDocument | null => {
  try {
    const value = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>
    if (
      value.schema !== MACOS_UPDATE_TRANSACTION_SCHEMA ||
      (value.status !== "prepared" && value.status !== "installed" && value.status !== "rollback_prepared" && value.status !== "rolled_back") ||
      (value.channel !== "stable" && value.channel !== "rc") ||
      !safeVersion(value.installedVersion) ||
      (value.previousVersion !== null && !safeVersion(value.previousVersion))
    )
      return null
    return value as TransactionDocument
  } catch {
    return null
  }
}

const fsyncPath = (value: string): void => {
  const descriptor = openSync(value, "r")
  try { fsyncSync(descriptor) } finally { closeSync(descriptor) }
}

const writeTransaction = (file: string, value: TransactionDocument): boolean => {
  try {
    const parent = path.dirname(file)
    mkdirSync(parent, { recursive: true, mode: 0o700 })
    if (process.platform !== "win32") chmodSync(parent, 0o700)
    const temporary = `${file}.tmp`
    writeFileSync(temporary, `${JSON.stringify(value)}\n`, { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(temporary, 0o600)
    fsyncPath(temporary)
    renameSync(temporary, file)
    fsyncPath(parent)
    return true
  } catch {
    return false
  }
}

const output = (result: UpdateCommandResult): string => `${result.stdout}\n${result.stderr}`

/**
 * `hdiutil attach` prints every device node in the attached image. Detaching
 * the whole-disk node is more reliable than addressing the temporary mount
 * directory after code-signature assessment and `ditto` have traversed it.
 */
export const attachedDiskDevice = (result: UpdateCommandResult): string | null => output(result).match(/^\/dev\/(disk\d+)\b/m)?.[0] ?? null

export type MacOSUpdateApplier = DesktopPlatformUpdateApplier & Readonly<{
  rollbackAvailable: () => boolean
  rollbackVersion: () => string | null
  rollbackCompletionStatus: () => "rolled_back" | null
  armFirstLaunchRollback: (input: Readonly<{ receiptPath: string; expectedVersion: string; transactionRef: string; previousVersion: string; previousArchitecture: "arm64" | "x64"; deadlineMs: number }>) => Promise<boolean>
  install: (artifactPath: string, candidateVersion: string, expectedApplicationArchitecture?: "arm64" | "x64") => Promise<MacOSUpdateResult>
  rollback: () => Promise<MacOSUpdateResult>
}>

export const openMacOSUpdateApplier = (
  input: Readonly<{
    root: string
    installedAppPath: string
    installedVersion: string
    channel: UpdateChannel
    platform?: NodeJS.Platform
    packaged: boolean
    run?: UpdateCommandRunner
    bundleId?: string
    teamId?: string
    targetArchitecture?: "arm64" | "x64"
    spawnWatchdog?: (executable: string, args: ReadonlyArray<string>) => boolean
  }>,
): MacOSUpdateApplier => {
  const run = input.run ?? defaultRunner
  const platform = input.platform ?? process.platform
  const bundleId = input.bundleId ?? "com.openagents.desktop"
  const teamId = input.teamId ?? "HQWSG26L43"
  const targetArchitecture = input.targetArchitecture ?? (process.arch === "arm64" ? "arm64" : "x64")
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

  const verifyApp = async (appPath: string, expectedVersion: string, expectedArchitecture?: "arm64" | "x64"): Promise<MacOSUpdateFailureReason | null> => {
    if (!existsSync(appPath)) return "candidate_missing"
    const identifier = await commandOk("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleIdentifier", path.join(appPath, "Contents", "Info.plist")])
    const version = await commandOk("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleShortVersionString", path.join(appPath, "Contents", "Info.plist")])
    if (identifier === null || identifier.stdout.trim() !== bundleId) return "identity_mismatch"
    if (version === null || version.stdout.trim() !== expectedVersion) return "version_mismatch"
    if (expectedArchitecture !== undefined) {
      const executable = await commandOk("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleExecutable", path.join(appPath, "Contents", "Info.plist")])
      if (executable === null || !/^[A-Za-z0-9._-]+$/.test(executable.stdout.trim())) {
        return "architecture_mismatch"
      }
      const architectures = await commandOk("/usr/bin/lipo", ["-archs", path.join(appPath, "Contents", "MacOS", executable.stdout.trim())])
      if (architectures === null || !architectures.stdout.trim().split(/\s+/).includes(expectedArchitecture)) {
        return "architecture_mismatch"
      }
    }
    const signature = await commandOk("/usr/bin/codesign", ["--verify", "--deep", "--strict", "--verbose=2", appPath])
    if (signature === null) return "signature_invalid"
    const identity = await commandOk("/usr/bin/codesign", ["-dv", "--verbose=4", appPath])
    const identityText = identity === null ? "" : output(identity)
    if (!identityText.includes(`TeamIdentifier=${teamId}`) || !identityText.includes(`Authority=Developer ID Application: OpenAgents, Inc. (${teamId})`))
      return "identity_mismatch"
    const modernDistributionVerifier = await commandOk("/usr/bin/xcrun", ["--find", "syspolicy_check"])
    const distribution =
      modernDistributionVerifier === null
        ? await commandOk("/usr/sbin/spctl", ["--assess", "--type", "execute", "--verbose=2", appPath])
        : await commandOk("/usr/bin/xcrun", ["syspolicy_check", "distribution", appPath])
    if (distribution === null || (await commandOk("/usr/bin/xcrun", ["stapler", "validate", appPath])) === null) return "notarization_invalid"
    return null
  }

  const copyApp = async (source: string, destination: string): Promise<boolean> => {
    rmSync(destination, { recursive: true, force: true })
    mkdirSync(path.dirname(destination), { recursive: true, mode: 0o700 })
    return (await commandOk("/usr/bin/ditto", ["--rsrc", "--extattr", source, destination])) !== null && existsSync(destination)
  }

  const fsyncNativePath = async (value: string): Promise<boolean> =>
    (await commandOk("/usr/bin/osascript", ["-l", "JavaScript", "-e", MACOS_FSYNC_JXA, value])) !== null

  const swapIntoPlace = async (source: string, expectedVersion: string, expectedArchitecture?: "arm64" | "x64"): Promise<boolean> => {
    const target = path.resolve(input.installedAppPath)
    const parent = path.dirname(target)
    const incoming = path.join(parent, ".OpenAgents.update.app")
    rmSync(incoming, { recursive: true, force: true })
    if (!(await copyApp(source, incoming)) || (await verifyApp(incoming, expectedVersion, expectedArchitecture)) !== null) {
      rmSync(incoming, { recursive: true, force: true })
      return false
    }
    if (!(await fsyncNativePath(incoming)) || !(await fsyncNativePath(parent))) return false
    const atomicReplace = await commandOk("/usr/bin/osascript", [
      "-l", "JavaScript", "-e",
      MACOS_ATOMIC_REPLACE_JXA,
      target,
      incoming,
    ])
    if (atomicReplace === null) {
      rmSync(incoming, { recursive: true, force: true })
      return false
    }
    // The selector mutates the app-parent namespace. Persist that directory
    // entry before any installed/rolled_back transaction can be published.
    if (!(await fsyncNativePath(parent))) return false
    return (await verifyApp(target, expectedVersion, expectedArchitecture)) === null
  }

  const supported = (): MacOSUpdateFailureReason | null => (platform !== "darwin" ? "unsupported_platform" : !input.packaged ? "not_packaged" : null)

  const rollbackAvailable = (): boolean =>
    supported() === null &&
    (transaction?.status === "installed" || transaction?.status === "prepared" || transaction?.status === "rollback_prepared") &&
    (transaction.installedVersion === input.installedVersion || transaction.previousVersion === input.installedVersion) &&
    transaction.channel === input.channel &&
    transaction.previousVersion !== null &&
    existsSync(rollbackApp)

  const rollbackVersion = (): string | null => (rollbackAvailable() ? (transaction?.previousVersion ?? null) : null)
  const rollbackCompletionStatus = (): "rolled_back" | null =>
    transaction?.status === "rolled_back" && transaction.previousVersion === null &&
      transaction.installedVersion === input.installedVersion && transaction.channel === input.channel
      ? "rolled_back"
      : null

  const install = async (
    artifactPath: string,
    candidateVersion: string,
    expectedApplicationArchitecture = input.targetArchitecture,
  ): Promise<MacOSUpdateResult> => {
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
    const detachTarget = attachedDiskDevice(mounted) ?? mountpoint
    try {
      const candidateApp = path.join(mountpoint, "OpenAgents.app")
      const verification = await verifyApp(candidateApp, candidateVersion, expectedApplicationArchitecture)
      if (verification !== null) return { ok: false, reason: verification }
      const currentVerification = await verifyApp(input.installedAppPath, input.installedVersion)
      if (currentVerification !== null) return { ok: false, reason: currentVerification }
      if (!(await copyApp(input.installedAppPath, rollbackApp))) return { ok: false, reason: "backup_failed" }
      if (!(await fsyncNativePath(rollbackApp)) || !(await fsyncNativePath(path.dirname(rollbackApp)))) return { ok: false, reason: "backup_failed" }
      transaction = {
        schema: MACOS_UPDATE_TRANSACTION_SCHEMA,
        status: "prepared",
        previousVersion: input.installedVersion,
        installedVersion: candidateVersion,
        channel: input.channel,
      }
      if (!writeTransaction(transactionFile, transaction) || !(await fsyncNativePath(transactionFile)) || !(await fsyncNativePath(path.dirname(transactionFile)))) {
        return { ok: false, reason: "install_failed" }
      }
      if (!(await swapIntoPlace(candidateApp, candidateVersion, expectedApplicationArchitecture))) {
        await rollback()
        return { ok: false, reason: "install_failed" }
      }
      transaction = {
        schema: MACOS_UPDATE_TRANSACTION_SCHEMA,
        status: "installed",
        previousVersion: input.installedVersion,
        installedVersion: candidateVersion,
        channel: input.channel,
      }
      if (!writeTransaction(transactionFile, transaction)) return { ok: false, reason: "install_failed" }
      return {
        ok: true,
        action: "installed",
        installedVersion: candidateVersion,
        previousVersion: input.installedVersion,
      }
    } finally {
      if ((await commandOk("/usr/bin/hdiutil", ["detach", detachTarget])) === null) {
        await commandOk("/usr/bin/hdiutil", ["detach", "-force", detachTarget])
      }
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
    transaction = { ...transaction, status: "rollback_prepared" }
    if (!writeTransaction(transactionFile, transaction) || !(await fsyncNativePath(transactionFile)) || !(await fsyncNativePath(path.dirname(transactionFile)))) {
      return { ok: false, reason: "rollback_failed" }
    }
    if (!(await swapIntoPlace(rollbackApp, previousVersion))) return { ok: false, reason: "rollback_failed" }
    transaction = {
      schema: MACOS_UPDATE_TRANSACTION_SCHEMA,
      status: "rolled_back",
      previousVersion: null,
      installedVersion: previousVersion,
      channel: input.channel,
    }
    if (!writeTransaction(transactionFile, transaction)) return { ok: false, reason: "rollback_failed" }
    rmSync(rollbackApp, { recursive: true, force: true })
    return {
      ok: true,
      action: "rolled_back",
      installedVersion: previousVersion,
      previousVersion: null,
    }
  }

  const armFirstLaunchRollback = async (
    watch: Readonly<{
      receiptPath: string
      expectedVersion: string
      transactionRef: string
      previousVersion: string
      previousArchitecture: "arm64" | "x64"
      deadlineMs: number
    }>,
  ): Promise<boolean> => {
    if (
      transaction?.status !== "installed" ||
      transaction.installedVersion !== watch.expectedVersion ||
      transaction.previousVersion === null ||
      transaction.previousVersion !== watch.previousVersion ||
      !existsSync(rollbackApp) ||
      !safeVersion(watch.expectedVersion) ||
      !/^[0-9a-f]{32}$/.test(watch.transactionRef) ||
      !Number.isFinite(watch.deadlineMs)
    )
      return false
    if ((await verifyApp(rollbackApp, watch.previousVersion, watch.previousArchitecture)) !== null) return false
    const watchdog = path.join(input.root, "first-launch-watchdog.sh")
    const diagnostic = path.join(input.root, "first-launch-watchdog.result")
    const script = `#!/bin/sh
set -eu
receipt="$1"
expected="$2"
transaction="$3"
deadline="$4"
rollback_app="$5"
installed_app="$6"
diagnostic="$7"
previous="$8"
architecture="$9"
iso_instant() { printf '%s' "$1" | /usr/bin/grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$'; }
ordered() { [ "$1" = "$2" ] || [ "$1" \< "$2" ]; }
while [ "$(date +%s)" -lt "$deadline" ]; do
  renderer="$(/usr/bin/plutil -extract rendererReadyAt raw -o - "$receipt" 2>/dev/null || true)"
  provider="$(/usr/bin/plutil -extract providerReadyAt raw -o - "$receipt" 2>/dev/null || true)"
  shutdown="$(/usr/bin/plutil -extract cleanShutdownAt raw -o - "$receipt" 2>/dev/null || true)"
  if [ -f "$receipt" ] && [ "$(/usr/bin/plutil -extract schema raw -o - "$receipt" 2>/dev/null || true)" = "openagents.desktop.launch_health.v1" ] && [ "$(/usr/bin/plutil -extract app raw -o - "$receipt" 2>/dev/null || true)" = "openagents-desktop" ] && [ "$(/usr/bin/plutil -extract version raw -o - "$receipt" 2>/dev/null || true)" = "$expected" ] && [ "$(/usr/bin/plutil -extract transactionRef raw -o - "$receipt" 2>/dev/null || true)" = "$transaction" ] && iso_instant "$renderer" && iso_instant "$provider" && iso_instant "$shutdown" && ordered "$renderer" "$provider" && ordered "$provider" "$shutdown"; then
    printf '%s\\n' healthy > "$diagnostic"
    exit 0
  fi
  sleep 2
done
info="$rollback_app/Contents/Info.plist"
executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$info" 2>/dev/null || true)"
identity="$(/usr/bin/codesign -dv --verbose=4 "$rollback_app" 2>&1 || true)"
if [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$info" 2>/dev/null || true)" != "${bundleId}" ] || [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$info" 2>/dev/null || true)" != "$previous" ] || ! /usr/bin/lipo -archs "$rollback_app/Contents/MacOS/$executable" 2>/dev/null | /usr/bin/grep -Eq "(^| )$architecture( |$)" || ! /usr/bin/codesign --verify --deep --strict --verbose=2 "$rollback_app" >/dev/null 2>&1 || ! printf '%s' "$identity" | /usr/bin/grep -Fq 'TeamIdentifier=${teamId}' || ! printf '%s' "$identity" | /usr/bin/grep -Fq 'Authority=Developer ID Application: OpenAgents, Inc. (${teamId})' || ! (/usr/bin/xcrun syspolicy_check distribution "$rollback_app" >/dev/null 2>&1 || /usr/sbin/spctl --assess --type execute "$rollback_app" >/dev/null 2>&1) || ! /usr/bin/xcrun stapler validate "$rollback_app" >/dev/null 2>&1; then
  printf '%s\n' rollback_failed > "$diagnostic"
  exit 1
fi
incoming="$installed_app.rollback-incoming"
/bin/rm -rf "$incoming"
/usr/bin/ditto --rsrc --extattr "$rollback_app" "$incoming"
fsync_native() { /usr/bin/osascript -l JavaScript -e "${MACOS_FSYNC_JXA}" "$1"; }
if ! fsync_native "$incoming" || ! fsync_native "$(/usr/bin/dirname "$installed_app")"; then
  printf '%s\\n' rollback_failed > "$diagnostic"
  exit 1
fi
transaction_file="$(/usr/bin/dirname "$diagnostic")/apply-transaction.json"
transaction_tmp="$transaction_file.watchdog.tmp"
printf '{"schema":"${MACOS_UPDATE_TRANSACTION_SCHEMA}","status":"rollback_prepared","previousVersion":"%s","installedVersion":"%s","channel":"${input.channel}"}\\n' "$previous" "$expected" > "$transaction_tmp"
/bin/chmod 600 "$transaction_tmp"
if ! fsync_native "$transaction_tmp" || ! /bin/mv "$transaction_tmp" "$transaction_file" || ! fsync_native "$(/usr/bin/dirname "$transaction_file")"; then
  printf '%s\\n' rollback_failed > "$diagnostic"
  exit 1
fi
if /usr/bin/osascript -l JavaScript -e "${MACOS_ATOMIC_REPLACE_JXA}" "$installed_app" "$incoming"; then
  if ! fsync_native "$(/usr/bin/dirname "$installed_app")"; then
    printf '%s\\n' rollback_failed > "$diagnostic"
    exit 1
  fi
  printf '{"schema":"${MACOS_UPDATE_TRANSACTION_SCHEMA}","status":"rolled_back","previousVersion":null,"installedVersion":"%s","channel":"${input.channel}"}\\n' "$previous" > "$transaction_tmp"
  /bin/chmod 600 "$transaction_tmp"
  if ! fsync_native "$transaction_tmp" || ! /bin/mv "$transaction_tmp" "$transaction_file" || ! fsync_native "$(/usr/bin/dirname "$transaction_file")"; then
    printf '%s\\n' rollback_failed > "$diagnostic"
    exit 1
  fi
  diagnostic_tmp="$diagnostic.watchdog.tmp"
  printf '%s\\n' rolled_back > "$diagnostic_tmp"
  /bin/chmod 600 "$diagnostic_tmp"
  if ! fsync_native "$diagnostic_tmp" || ! /bin/mv "$diagnostic_tmp" "$diagnostic" || ! fsync_native "$(/usr/bin/dirname "$diagnostic")"; then
    exit 1
  fi
  /bin/rm -rf "$rollback_app"
  /usr/bin/open "$installed_app" >/dev/null 2>&1 || true
  exit 0
fi
printf '%s\\n' rollback_failed > "$diagnostic"
exit 1
`
    try {
      writeFileSync(watchdog, script, { encoding: "utf8", mode: 0o700 })
      chmodSync(watchdog, 0o700)
      const args = [
        watchdog,
        path.resolve(watch.receiptPath),
        watch.expectedVersion,
        watch.transactionRef,
        String(Math.floor(watch.deadlineMs / 1000)),
        rollbackApp,
        path.resolve(input.installedAppPath),
        diagnostic,
        watch.previousVersion,
        watch.previousArchitecture,
      ]
      if (input.spawnWatchdog !== undefined) return input.spawnWatchdog("/bin/sh", args)
      const child = spawn("/bin/sh", args, { detached: true, stdio: "ignore" })
      child.unref()
      return true
    } catch {
      return false
    }
  }

  return { target: `darwin-${targetArchitecture}`, format: "dmg", rollbackClaim: "retained_slot", rollbackAvailable, rollbackVersion, rollbackCompletionStatus, armFirstLaunchRollback, install, rollback }
}
