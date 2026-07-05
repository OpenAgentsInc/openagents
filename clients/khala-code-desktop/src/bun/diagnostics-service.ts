/**
 * Khala Code desktop diagnostics service (issue #8441).
 *
 * Composes the pure shared diagnostics modules (log store, bundle/zip
 * builder, unresponsive watchdog) with the process-level effects the
 * desktop main process needs: writing the exported debug-log archive to
 * disk, and coordinating a clean relaunch/quit (dispose every local service
 * process before spawning a replacement or exiting, so recovery actions
 * never leave a duplicate Codex/Pylon/Khala Sync process running).
 *
 * Every process-level effect (disk I/O, spawning a replacement process,
 * exiting) is injected so this can be fully unit tested without touching a
 * real filesystem or actually terminating the test runner.
 */

import { mkdir, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

import { buildKhalaCodeDesktopDiagnosticsArchive, buildKhalaCodeDesktopDiagnosticsBundle, type KhalaCodeDesktopDiagnosticsBundleManifest } from "../shared/diagnostics-bundle.js"
import {
  createKhalaCodeDesktopDiagnosticsLogStore,
  type KhalaCodeDesktopDiagnosticsCategory,
  type KhalaCodeDesktopDiagnosticsLogStore,
} from "../shared/diagnostics-log-store.js"
import {
  createKhalaCodeDesktopUnresponsiveWatchdog,
  type KhalaCodeDesktopUnresponsiveWatchdog,
  type KhalaCodeDesktopUnresponsiveWatchdogState,
} from "../shared/unresponsive-watchdog.js"

type ChatEnv = Readonly<Record<string, string | undefined>>

const DEFAULT_APP_VERSION = "0.0.1"
const DEFAULT_UNRESPONSIVE_TIMEOUT_MS = 15_000

export const khalaCodeDesktopAppVersionFromEnv = (env: ChatEnv): string => {
  const version = env.KHALA_CODE_DESKTOP_VERSION?.trim()
  return version !== undefined && version.length > 0 ? version : DEFAULT_APP_VERSION
}

export const khalaCodeDesktopDiagnosticsDefaultOutputDirectory = (
  env: ChatEnv,
): string => {
  const override = env.KHALA_CODE_DESKTOP_DIAGNOSTICS_DIR?.trim()
  if (override !== undefined && override.length > 0) return override
  return join(homedir(), ".khala-code", "diagnostics")
}

export type KhalaCodeDesktopDiagnosticsExportResult = Readonly<{
  archiveBytes: number
  manifest: KhalaCodeDesktopDiagnosticsBundleManifest
  path: string
}>

export type KhalaCodeDesktopDiagnosticsServiceOptions = Readonly<{
  arch?: string
  /**
   * Disposes every local service/subprocess this app started (Codex
   * app-server, token-usage sync, Khala Sync, fleet reporters, ...). Must be
   * awaited to completion before relaunch/quit spawn or exit, so no orphaned
   * process is left running.
   */
  disposeRuntime: () => void | Promise<void>
  env: ChatEnv
  exitProcess?: (code: number) => void
  fatalLimit?: number
  limitPerCategory?: number
  mkdirFn?: (path: string) => Promise<void>
  /** Called whenever the unresponsive watchdog transitions state (e.g. to show/hide a native recovery dialog). */
  onWatchdogStateChange?: (state: KhalaCodeDesktopUnresponsiveWatchdogState) => void
  outputDirectory?: string
  platform?: string
  /** Spawns a fresh copy of this app's own process. Only called by relaunch(). */
  spawnRelaunchProcess?: () => void
  unresponsiveTimeoutMs?: number
  writeFileFn?: (path: string, data: Uint8Array) => Promise<void>
}>

export type KhalaCodeDesktopDiagnosticsService = Readonly<{
  checkUnresponsiveNow: () => KhalaCodeDesktopUnresponsiveWatchdogState
  exportDebugLogArchive: () => Promise<KhalaCodeDesktopDiagnosticsExportResult>
  handleRendererHeartbeat: () => void
  logStore: KhalaCodeDesktopDiagnosticsLogStore
  quit: () => Promise<void>
  recordFatalError: (
    category: KhalaCodeDesktopDiagnosticsCategory,
    message: string,
    context?: Readonly<Record<string, unknown>>,
  ) => void
  recordMainLog: (message: string, level?: "debug" | "error" | "info" | "warn") => void
  recordNativeShellEvent: (message: string, context?: Readonly<Record<string, unknown>>) => void
  recordRendererDiagnostic: (
    message: string,
    level?: "debug" | "error" | "info" | "warn",
    context?: Readonly<Record<string, unknown>>,
  ) => void
  recordServiceLog: (
    source: string,
    message: string,
    level?: "debug" | "error" | "info" | "warn",
  ) => void
  relaunch: () => Promise<void>
  snapshotCounts: () => Readonly<Record<KhalaCodeDesktopDiagnosticsCategory, number>>
  watchdog: KhalaCodeDesktopUnresponsiveWatchdog
}>

const noopSpawnRelaunchProcess = (): void => {
  // Callers on real Electrobun builds must supply a real spawn function
  // (see index.ts). Absent one, relaunch degrades to "dispose and exit"
  // rather than throwing, which is the safer default for a diagnostics path.
}

export const createKhalaCodeDesktopDiagnosticsService = (
  options: KhalaCodeDesktopDiagnosticsServiceOptions,
): KhalaCodeDesktopDiagnosticsService => {
  const logStore = createKhalaCodeDesktopDiagnosticsLogStore({
    ...(options.fatalLimit === undefined ? {} : { fatalLimit: options.fatalLimit }),
    ...(options.limitPerCategory === undefined ? {} : { limitPerCategory: options.limitPerCategory }),
  })
  const watchdog = createKhalaCodeDesktopUnresponsiveWatchdog({
    ...(options.onWatchdogStateChange === undefined
      ? {}
      : { onStateChange: options.onWatchdogStateChange }),
    timeoutMs: options.unresponsiveTimeoutMs ?? DEFAULT_UNRESPONSIVE_TIMEOUT_MS,
  })
  const writeFileFn = options.writeFileFn ?? writeFile
  const mkdirFn = options.mkdirFn ?? (async (path: string) => {
    await mkdir(path, { recursive: true })
  })
  const exitProcess = options.exitProcess ?? ((code: number) => process.exit(code))
  const spawnRelaunchProcess = options.spawnRelaunchProcess ?? noopSpawnRelaunchProcess
  const outputDirectory = options.outputDirectory ??
    khalaCodeDesktopDiagnosticsDefaultOutputDirectory(options.env)

  const recordMainLog: KhalaCodeDesktopDiagnosticsService["recordMainLog"] = (
    message,
    level = "info",
  ) => {
    logStore.record({ category: "main", level, message })
  }

  const recordRendererDiagnostic: KhalaCodeDesktopDiagnosticsService["recordRendererDiagnostic"] = (
    message,
    level = "info",
    context,
  ) => {
    logStore.record({ category: "renderer", ...(context === undefined ? {} : { context }), level, message })
  }

  const recordServiceLog: KhalaCodeDesktopDiagnosticsService["recordServiceLog"] = (
    source,
    message,
    level = "info",
  ) => {
    logStore.record({ category: "service", level, message, source })
  }

  const recordNativeShellEvent: KhalaCodeDesktopDiagnosticsService["recordNativeShellEvent"] = (
    message,
    context,
  ) => {
    logStore.record({ category: "native-shell", ...(context === undefined ? {} : { context }), message })
  }

  const recordFatalError: KhalaCodeDesktopDiagnosticsService["recordFatalError"] = (
    category,
    message,
    context,
  ) => {
    logStore.record({
      category,
      ...(context === undefined ? {} : { context }),
      fatal: true,
      level: "error",
      message,
    })
  }

  const exportDebugLogArchive = async (): Promise<KhalaCodeDesktopDiagnosticsExportResult> => {
    const bundle = buildKhalaCodeDesktopDiagnosticsBundle({
      appVersion: khalaCodeDesktopAppVersionFromEnv(options.env),
      arch: options.arch ?? process.arch,
      platform: options.platform ?? process.platform,
      snapshot: logStore.snapshot(),
    })
    const archive = buildKhalaCodeDesktopDiagnosticsArchive(bundle)
    await mkdirFn(outputDirectory)
    const fileName = `khala-code-debug-logs-${bundle.manifest.generatedAt.replace(/[:.]/g, "-")}.zip`
    const path = join(outputDirectory, fileName)
    await writeFileFn(path, archive)
    recordNativeShellEvent("debug log archive exported", { fatalCount: bundle.manifest.fatalCount, path })
    return { archiveBytes: archive.length, manifest: bundle.manifest, path }
  }

  // Relaunch/quit must dispose every local service process BEFORE spawning a
  // replacement or exiting — otherwise a Codex app-server (or Pylon, Khala
  // Sync, token-usage sync) child process spawned by this process can
  // outlive it, leaving a duplicate running alongside the new instance.
  const relaunch = async (): Promise<void> => {
    recordNativeShellEvent("relaunch requested")
    await options.disposeRuntime()
    spawnRelaunchProcess()
    exitProcess(0)
  }

  const quit = async (): Promise<void> => {
    recordNativeShellEvent("quit requested")
    await options.disposeRuntime()
    exitProcess(0)
  }

  const handleRendererHeartbeat = (): void => {
    watchdog.recordHeartbeat()
  }

  const checkUnresponsiveNow = (): KhalaCodeDesktopUnresponsiveWatchdogState => watchdog.checkNow()

  const snapshotCounts = (): Readonly<Record<KhalaCodeDesktopDiagnosticsCategory, number>> => {
    const snapshot = logStore.snapshot()
    return {
      main: snapshot.entriesByCategory.main.length,
      "native-shell": snapshot.entriesByCategory["native-shell"].length,
      renderer: snapshot.entriesByCategory.renderer.length,
      service: snapshot.entriesByCategory.service.length,
    }
  }

  return {
    checkUnresponsiveNow,
    exportDebugLogArchive,
    handleRendererHeartbeat,
    logStore,
    quit,
    recordFatalError,
    recordMainLog,
    recordNativeShellEvent,
    recordRendererDiagnostic,
    recordServiceLog,
    relaunch,
    snapshotCounts,
    watchdog,
  }
}
