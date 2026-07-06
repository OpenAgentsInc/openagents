import { describe, expect, test } from "bun:test"

import { decideOtaGateAction, otaGateVisibleState } from "../src/updates/ota-update-gate-core"

const idle = {
  isChecking: false,
  isDownloading: false,
  isRestarting: false,
  isUpdateAvailable: false,
  isUpdatePending: false,
}

describe("decideOtaGateAction", () => {
  test("nothing available: none", () => {
    expect(decideOtaGateAction(idle)).toBe("none")
  })

  test("an update is available and not yet downloading: fetch", () => {
    expect(decideOtaGateAction({ ...idle, isUpdateAvailable: true })).toBe("fetch")
  })

  test("already downloading: none (don't double-fetch)", () => {
    expect(
      decideOtaGateAction({ ...idle, isDownloading: true, isUpdateAvailable: true }),
    ).toBe("none")
  })

  test("an update has been fetched and is pending: reload, even if isUpdateAvailable is stale-true", () => {
    expect(
      decideOtaGateAction({ ...idle, isUpdateAvailable: true, isUpdatePending: true }),
    ).toBe("reload")
  })

  test("already restarting: none, regardless of other flags (avoid double reloadAsync calls)", () => {
    expect(
      decideOtaGateAction({
        ...idle,
        isRestarting: true,
        isUpdateAvailable: true,
        isUpdatePending: true,
      }),
    ).toBe("none")
  })
})

describe("otaGateVisibleState", () => {
  test("idle: hidden", () => {
    expect(otaGateVisibleState(idle)).toBe("hidden")
  })

  test("checking: hidden — routine checks must never show visible chrome", () => {
    expect(otaGateVisibleState({ ...idle, isChecking: true })).toBe("hidden")
  })

  test("downloading: downloading (even if isChecking is also still true)", () => {
    expect(
      otaGateVisibleState({ ...idle, isChecking: true, isDownloading: true }),
    ).toBe("downloading")
  })

  test("restarting: reloading, takes priority over every other flag", () => {
    expect(
      otaGateVisibleState({
        ...idle,
        isChecking: true,
        isDownloading: true,
        isRestarting: true,
      }),
    ).toBe("reloading")
  })
})
