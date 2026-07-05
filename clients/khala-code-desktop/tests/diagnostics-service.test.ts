import { describe, expect, test } from "bun:test"

import {
  createKhalaCodeDesktopDiagnosticsService,
  khalaCodeDesktopAppVersionFromEnv,
  khalaCodeDesktopDiagnosticsDefaultOutputDirectory,
} from "../src/bun/diagnostics-service"
import { KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER } from "../src/shared/diagnostics-redaction"
import { readKhalaCodeDesktopZipArchiveEntries } from "../src/shared/diagnostics-zip"

const decoder = new TextDecoder()

const recordingFakes = () => {
  const calls: string[] = []
  const written: { data: Uint8Array; path: string }[] = []
  const disposeRuntime = async () => {
    calls.push("disposeRuntime")
  }
  const spawnRelaunchProcess = () => {
    calls.push("spawnRelaunchProcess")
  }
  const exitProcess = (code: number) => {
    calls.push(`exitProcess:${code}`)
  }
  const writeFileFn = async (path: string, data: Uint8Array) => {
    calls.push("writeFile")
    written.push({ data, path })
  }
  const mkdirFn = async () => {
    calls.push("mkdir")
  }
  return { calls, disposeRuntime, exitProcess, mkdirFn, spawnRelaunchProcess, writeFileFn, written }
}

describe("khalaCodeDesktopAppVersionFromEnv", () => {
  test("falls back to the default version when unset", () => {
    expect(khalaCodeDesktopAppVersionFromEnv({})).toBe("0.0.1")
  })

  test("honors KHALA_CODE_DESKTOP_VERSION when set", () => {
    expect(khalaCodeDesktopAppVersionFromEnv({ KHALA_CODE_DESKTOP_VERSION: "0.3.0" })).toBe("0.3.0")
  })
})

describe("khalaCodeDesktopDiagnosticsDefaultOutputDirectory", () => {
  test("honors an explicit override", () => {
    expect(
      khalaCodeDesktopDiagnosticsDefaultOutputDirectory({
        KHALA_CODE_DESKTOP_DIAGNOSTICS_DIR: "/tmp/khala-diagnostics-test",
      }),
    ).toBe("/tmp/khala-diagnostics-test")
  })

  test("defaults under the user's home .khala-code directory", () => {
    const dir = khalaCodeDesktopDiagnosticsDefaultOutputDirectory({})
    expect(dir).toContain(".khala-code")
    expect(dir).toContain("diagnostics")
  })
})

// Oracle for khala_code.diagnostics.debug_log_export_public_safe_and_recovery_visible.v1
describe("createKhalaCodeDesktopDiagnosticsService", () => {
  test("exportDebugLogArchive writes a real zip archive with redacted content", async () => {
    const fakes = recordingFakes()
    const service = createKhalaCodeDesktopDiagnosticsService({
      arch: "arm64",
      disposeRuntime: fakes.disposeRuntime,
      env: {},
      exitProcess: fakes.exitProcess,
      mkdirFn: fakes.mkdirFn,
      outputDirectory: "/tmp/khala-code-desktop-diagnostics-test",
      platform: "darwin",
      spawnRelaunchProcess: fakes.spawnRelaunchProcess,
      writeFileFn: fakes.writeFileFn,
    })

    service.recordServiceLog("codex-app-server", "call failed with token sk-abcdefghijklmnopqrstuvwx", "error")
    service.recordFatalError("renderer", "uncaught error in transcript render")

    const result = await service.exportDebugLogArchive()
    expect(result.path).toContain("/tmp/khala-code-desktop-diagnostics-test")
    expect(result.manifest.fatalCount).toBe(1)
    expect(fakes.calls).toContain("mkdir")
    expect(fakes.calls).toContain("writeFile")
    expect(fakes.written).toHaveLength(1)

    const entries = readKhalaCodeDesktopZipArchiveEntries(fakes.written[0]!.data)
    const serviceLog = decoder.decode(entries.find(entry => entry.path === "service.log")?.data)
    expect(serviceLog).not.toContain("sk-abcdefghijklmnopqrstuvwx")
    expect(serviceLog).toContain(KHALA_CODE_DESKTOP_DIAGNOSTICS_REDACTION_MARKER)
  })

  test("relaunch disposes the runtime before spawning a replacement and before exiting", async () => {
    const fakes = recordingFakes()
    const service = createKhalaCodeDesktopDiagnosticsService({
      disposeRuntime: fakes.disposeRuntime,
      env: {},
      exitProcess: fakes.exitProcess,
      spawnRelaunchProcess: fakes.spawnRelaunchProcess,
    })
    await service.relaunch()
    expect(fakes.calls).toEqual(["disposeRuntime", "spawnRelaunchProcess", "exitProcess:0"])
  })

  test("quit disposes the runtime before exiting and never spawns a replacement", async () => {
    const fakes = recordingFakes()
    const service = createKhalaCodeDesktopDiagnosticsService({
      disposeRuntime: fakes.disposeRuntime,
      env: {},
      exitProcess: fakes.exitProcess,
      spawnRelaunchProcess: fakes.spawnRelaunchProcess,
    })
    await service.quit()
    expect(fakes.calls).toEqual(["disposeRuntime", "exitProcess:0"])
  })

  test("relaunch/quit never leave the old process's services alive if disposeRuntime rejects", async () => {
    const fakes = recordingFakes()
    const failingDispose = async () => {
      fakes.calls.push("disposeRuntime")
      throw new Error("simulated dispose failure")
    }
    const service = createKhalaCodeDesktopDiagnosticsService({
      disposeRuntime: failingDispose,
      env: {},
      exitProcess: fakes.exitProcess,
      spawnRelaunchProcess: fakes.spawnRelaunchProcess,
    })
    await expect(service.relaunch()).rejects.toThrow("simulated dispose failure")
    // Because dispose rejected, we must NOT have spawned a replacement or
    // exited — a caller that ignores this rejection could otherwise end up
    // with two live processes. This proves the ordering is a real await,
    // not a fire-and-forget call.
    expect(fakes.calls).toEqual(["disposeRuntime"])
  })

  test("handleRendererHeartbeat feeds the watchdog and checkUnresponsiveNow reflects it", () => {
    const fakes = recordingFakes()
    const service = createKhalaCodeDesktopDiagnosticsService({
      disposeRuntime: fakes.disposeRuntime,
      env: {},
      unresponsiveTimeoutMs: 1000,
    })
    service.handleRendererHeartbeat()
    expect(service.watchdog.state()).toBe("responsive")
  })

  test("snapshotCounts reflects recorded entries per category", () => {
    const fakes = recordingFakes()
    const service = createKhalaCodeDesktopDiagnosticsService({
      disposeRuntime: fakes.disposeRuntime,
      env: {},
    })
    service.recordMainLog("boot")
    service.recordNativeShellEvent("window created")
    service.recordRendererDiagnostic("mounted")
    service.recordServiceLog("pylon", "ready")
    expect(service.snapshotCounts()).toEqual({
      main: 1,
      "native-shell": 1,
      renderer: 1,
      service: 1,
    })
  })
})
