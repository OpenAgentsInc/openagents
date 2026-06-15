// #5040: Autopilot Desktop default-on auto-update.
//
// Uses Electrobun's built-in Updater (checkForUpdate -> downloadUpdate ->
// applyUpdate, BSDIFF patches + tarball, hash-verified) against our GCP feed
// (release.baseUrl = updates.openagents.com/desktop in electrobun.config.ts).
// Default ON: checks at launch and on an interval unless the operator opts out.
// Dev builds (channel "dev") are a no-op (the Updater itself disables there).
//
// The opt-out logic is pure + injectable so it is unit-tested without Electrobun.

export type AutoUpdateEnv = Readonly<Record<string, string | undefined>>

// Default-on: only disabled when the operator explicitly opts out, via env now
// (AUTOPILOT_DISABLE_AUTOUPDATE) or the Settings toggle later (same flag).
export function autoUpdateDisabledReason(env: AutoUpdateEnv): string | null {
  const disable = env.AUTOPILOT_DISABLE_AUTOUPDATE
  if (disable && disable !== "0" && disable !== "false") {
    return "AUTOPILOT_DISABLE_AUTOUPDATE is set"
  }
  if (env.AUTOPILOT_AUTOUPDATE === "0" || env.AUTOPILOT_AUTOUPDATE === "false") {
    return "AUTOPILOT_AUTOUPDATE is disabled"
  }
  return null
}

export const DEFAULT_AUTO_UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000 // 6h

export function autoUpdateIntervalMs(env: AutoUpdateEnv): number {
  const raw = Number(env.AUTOPILOT_DESKTOP_UPDATE_POLL_MS)
  return Number.isFinite(raw) && raw > 0 ? raw : DEFAULT_AUTO_UPDATE_INTERVAL_MS
}

// Minimal shape of Electrobun's Updater we depend on (injected for testing).
export type UpdaterLike = {
  checkForUpdate: () => Promise<{ updateAvailable: boolean; updateReady?: boolean; error?: string }>
  downloadUpdate: () => Promise<unknown>
  applyUpdate: () => Promise<unknown>
}

export type RunAutoUpdateDeps = {
  readonly updater: UpdaterLike
  readonly env: AutoUpdateEnv
  readonly log?: (message: string) => void
}

// One check cycle: check -> (if available) download -> apply (Electrobun relaunches
// into the new version). Fail-soft: any error is logged and swallowed so a failed
// update never blocks the running app. Returns what it did (for tests).
export async function runAutoUpdateOnce(
  deps: RunAutoUpdateDeps,
): Promise<"disabled" | "up-to-date" | "applied" | "error"> {
  const disabled = autoUpdateDisabledReason(deps.env)
  if (disabled !== null) {
    deps.log?.(`auto-update disabled: ${disabled}`)
    return "disabled"
  }
  try {
    const result = await deps.updater.checkForUpdate()
    if (!result.updateAvailable) return "up-to-date"
    deps.log?.("auto-update: downloading…")
    await deps.updater.downloadUpdate()
    deps.log?.("auto-update: applying + relaunching…")
    await deps.updater.applyUpdate()
    return "applied"
  } catch (error) {
    deps.log?.(`auto-update skipped: ${error instanceof Error ? error.message : String(error)}`)
    return "error"
  }
}
