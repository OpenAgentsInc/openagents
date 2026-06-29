import { describe, expect, test } from "bun:test"

import {
  autoUpdateDisabledReason,
  autoUpdateIntervalMs,
  DEFAULT_AUTO_UPDATE_INTERVAL_MS,
  runAutoUpdateOnce,
  type UpdaterLike,
} from "../src/bun/auto-update"

describe("autoUpdateDisabledReason (default on)", () => {
  test("on by default; off only when explicitly disabled", () => {
    expect(autoUpdateDisabledReason({})).toBeNull()
    expect(autoUpdateDisabledReason({ AUTOPILOT_DISABLE_AUTOUPDATE: "1" })).toContain("DISABLE")
    expect(autoUpdateDisabledReason({ AUTOPILOT_AUTOUPDATE: "0" })).toContain("disabled")
    expect(autoUpdateDisabledReason({ AUTOPILOT_DISABLE_AUTOUPDATE: "0" })).toBeNull()
  })
})

describe("autoUpdateIntervalMs", () => {
  test("defaults to 6h, overridable", () => {
    expect(autoUpdateIntervalMs({})).toBe(DEFAULT_AUTO_UPDATE_INTERVAL_MS)
    expect(autoUpdateIntervalMs({ AUTOPILOT_DESKTOP_UPDATE_POLL_MS: "1000" })).toBe(1000)
    expect(autoUpdateIntervalMs({ AUTOPILOT_DESKTOP_UPDATE_POLL_MS: "bad" })).toBe(
      DEFAULT_AUTO_UPDATE_INTERVAL_MS,
    )
  })
})

describe("runAutoUpdateOnce", () => {
  const updater = (overrides: Partial<UpdaterLike> & { available: boolean }): UpdaterLike => ({
    checkForUpdate: async () => ({ updateAvailable: overrides.available }),
    downloadUpdate: async () => undefined,
    applyUpdate: async () => undefined,
    ...overrides,
  })

  test("opted out => disabled, never checks", async () => {
    let checked = false
    const result = await runAutoUpdateOnce({
      env: { AUTOPILOT_DISABLE_AUTOUPDATE: "1" },
      updater: {
        checkForUpdate: async () => {
          checked = true
          return { updateAvailable: true }
        },
        downloadUpdate: async () => undefined,
        applyUpdate: async () => undefined,
      },
    })
    expect(result).toBe("disabled")
    expect(checked).toBe(false)
  })

  test("no update => up-to-date", async () => {
    const result = await runAutoUpdateOnce({ env: {}, updater: updater({ available: false }) })
    expect(result).toBe("up-to-date")
  })

  test("update available => download + apply", async () => {
    let downloaded = false
    let applied = false
    const result = await runAutoUpdateOnce({
      env: {},
      updater: {
        checkForUpdate: async () => ({ updateAvailable: true }),
        downloadUpdate: async () => {
          downloaded = true
        },
        applyUpdate: async () => {
          applied = true
        },
      },
    })
    expect(result).toBe("applied")
    expect(downloaded).toBe(true)
    expect(applied).toBe(true)
  })

  test("check error => fail-soft (error, app keeps running)", async () => {
    const result = await runAutoUpdateOnce({
      env: {},
      updater: {
        checkForUpdate: async () => {
          throw new Error("network down")
        },
        downloadUpdate: async () => undefined,
        applyUpdate: async () => undefined,
      },
    })
    expect(result).toBe("error")
  })
})
