import { khalaCodeDesktopUpdaterReleaseNotesUrl } from "../shared/updater.js"
import type {
  KhalaCodeDesktopUpdaterActionResult,
  KhalaCodeDesktopUpdaterState,
  KhalaCodeDesktopUpdaterStatus,
} from "../shared/rpc.js"

/**
 * Backend-agnostic in-app updater controller for #8440.
 *
 * The state machine is intentionally small and mirrors OpenCode's desktop
 * updater controller (`projects/repos/opencode/packages/desktop/src/main/updater-controller.ts`):
 * `check()` only ever reads remote feed metadata, `download()` fetches and
 * stages the update, and `install()` is the ONLY transition that mutates the
 * running app — it must always be triggered by an explicit caller (RPC
 * request from a settings-row button click or a native menu action), never
 * automatically. Periodic checks call `check()` only, so the app can never
 * silently install an update.
 */

export type KhalaCodeDesktopUpdaterBackend = {
  readonly checkForUpdates: () => Promise<{
    readonly error: string
    readonly updateAvailable: boolean
    readonly version: string
  }>
  readonly downloadUpdate: () => Promise<{ readonly error?: string; readonly ok: boolean }>
  readonly install: () => Promise<void>
}

export type KhalaCodeDesktopUpdaterController = {
  readonly check: () => Promise<KhalaCodeDesktopUpdaterState>
  readonly download: () => Promise<KhalaCodeDesktopUpdaterState>
  readonly getState: () => KhalaCodeDesktopUpdaterState
  readonly install: () => Promise<KhalaCodeDesktopUpdaterState>
  readonly startPeriodicChecks: (intervalMs: number) => () => void
  readonly status: () => KhalaCodeDesktopUpdaterStatus
  readonly subscribe: (listener: (state: KhalaCodeDesktopUpdaterState) => void) => () => void
}

export function createKhalaCodeDesktopUpdaterController(input: {
  readonly backend: KhalaCodeDesktopUpdaterBackend
  readonly channel: string
  readonly currentVersion: string
  readonly enabled: boolean
  readonly log?: (message: string, data?: Record<string, unknown>) => void
  readonly now?: () => Date
}): KhalaCodeDesktopUpdaterController {
  const now = input.now ?? (() => new Date())
  let state: KhalaCodeDesktopUpdaterState = input.enabled ? { status: "idle" } : { status: "idle" }
  let pendingCheck: Promise<KhalaCodeDesktopUpdaterState> | undefined
  let pendingDownload: Promise<KhalaCodeDesktopUpdaterState> | undefined
  const listeners = new Set<(state: KhalaCodeDesktopUpdaterState) => void>()

  const transition = (next: KhalaCodeDesktopUpdaterState): KhalaCodeDesktopUpdaterState => {
    input.log?.("khala_code_desktop.updater.state_changed", {
      from: state.status,
      to: next.status,
    })
    state = next
    for (const listener of listeners) listener(state)
    return state
  }

  const check = (): Promise<KhalaCodeDesktopUpdaterState> => {
    if (!input.enabled) return Promise.resolve(state)
    if (state.status === "checking" || state.status === "downloading" || state.status === "installing") {
      return pendingCheck ?? pendingDownload ?? Promise.resolve(state)
    }
    if (state.status === "ready" || state.status === "available") return Promise.resolve(state)

    pendingCheck = (async () => {
      transition({ status: "checking" })
      try {
        const result = await input.backend.checkForUpdates()
        if (result.error.length > 0) {
          return transition({ message: result.error, retryable: true, status: "error" })
        }
        const checkedAt = now().toISOString()
        if (!result.updateAvailable || result.version.length === 0) {
          return transition({ checkedAt, status: "up_to_date", version: input.currentVersion })
        }
        return transition({ checkedAt, status: "available", version: result.version })
      } catch (error) {
        return transition({
          message: error instanceof Error ? error.message : String(error),
          retryable: true,
          status: "error",
        })
      }
    })().finally(() => {
      pendingCheck = undefined
    })
    return pendingCheck
  }

  const download = (): Promise<KhalaCodeDesktopUpdaterState> => {
    if (state.status !== "available") return Promise.resolve(state)
    if (pendingDownload !== undefined) return pendingDownload

    const version = state.version
    pendingDownload = (async () => {
      transition({ progressPercent: null, status: "downloading", version })
      try {
        const result = await input.backend.downloadUpdate()
        if (!result.ok) {
          return transition({
            message: result.error ?? "Update download failed.",
            retryable: true,
            status: "error",
          })
        }
        return transition({ status: "ready", version })
      } catch (error) {
        return transition({
          message: error instanceof Error ? error.message : String(error),
          retryable: true,
          status: "error",
        })
      }
    })().finally(() => {
      pendingDownload = undefined
    })
    return pendingDownload
  }

  const install = async (): Promise<KhalaCodeDesktopUpdaterState> => {
    if (state.status !== "ready") {
      throw new Error("Khala Code update is not ready to install.")
    }
    const version = state.version
    transition({ status: "installing", version })
    try {
      // Real backends quit and relaunch the app here and this never resolves
      // in production; fixture/test backends may resolve normally.
      await input.backend.install()
      return transition({ status: "ready", version })
    } catch (error) {
      transition({ status: "ready", version })
      throw error
    }
  }

  return {
    check,
    download,
    getState: () => state,
    install,
    startPeriodicChecks(intervalMs: number) {
      if (!input.enabled || intervalMs <= 0) return () => {}
      const timer = setInterval(() => void check(), intervalMs)
      if (typeof timer === "object" && "unref" in timer) (timer as { unref: () => void }).unref()
      return () => clearInterval(timer)
    },
    status(): KhalaCodeDesktopUpdaterStatus {
      const version = "version" in state ? state.version : input.currentVersion
      return {
        app: "Khala Code Desktop",
        capability: "in_app_updater",
        channel: input.channel,
        currentVersion: input.currentVersion,
        enabled: input.enabled,
        observedAt: now().toISOString(),
        ok: true,
        releaseNotesUrl: khalaCodeDesktopUpdaterReleaseNotesUrl(version),
        state,
      }
    },
    subscribe(listener) {
      listeners.add(listener)
      listener(state)
      return () => listeners.delete(listener)
    },
  }
}

export const khalaCodeDesktopUpdaterActionResult = (
  status: KhalaCodeDesktopUpdaterStatus,
): KhalaCodeDesktopUpdaterActionResult =>
  status.state.status === "error"
    ? { error: status.state.message, ok: false, status }
    : { ok: true, status }
