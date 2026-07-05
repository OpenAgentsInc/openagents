import { afterEach, describe, expect, test } from "bun:test"

import {
  createKhalaCodeDesktopUpdaterController,
  khalaCodeDesktopUpdaterActionResult,
  type KhalaCodeDesktopUpdaterBackend,
} from "../src/bun/khala-code-updater-controller"
import { createKhalaCodeDesktopElectrobunUpdaterBackend } from "../src/bun/khala-code-updater-electrobun-backend"
import {
  khalaCodeUpdateFeedFixtureRouteKey,
  startKhalaCodeUpdateFeedFixtureServer,
  type KhalaCodeUpdateFeedFixtureServer,
} from "../src/bun/khala-code-update-feed-fixture-server"
import {
  fetchKhalaCodeDesktopUpdateFeedInfo,
  khalaCodeDesktopUpdaterReleaseNotesUrl,
  type KhalaCodeDesktopUpdaterLocalInfo,
} from "../src/shared/updater"

const platform = { arch: "arm64" as const, os: "macos" as const }

const servers: KhalaCodeUpdateFeedFixtureServer[] = []
afterEach(() => {
  servers.splice(0).forEach(server => server.stop())
})

const localInfo = (overrides: Partial<KhalaCodeDesktopUpdaterLocalInfo> = {}): KhalaCodeDesktopUpdaterLocalInfo => ({
  baseUrl: "http://127.0.0.1:0",
  channel: "stable",
  hash: "current-hash",
  identifier: "com.openagents.khala.code.desktop",
  name: "Khala Code",
  version: "0.1.0",
  ...overrides,
})

describe("Khala Code updater feed client (#8440)", () => {
  test("available: fixture server reports a newer hash", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({
      routes: new Map([
        [
          khalaCodeUpdateFeedFixtureRouteKey("stable", platform),
          { kind: "json", body: { hash: "next-hash", version: "0.2.0" } },
        ],
      ]),
    })
    servers.push(server)

    const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
      localInfo: localInfo({ baseUrl: server.baseUrl }),
      platform,
    })

    expect(feed).toEqual({ error: "", hash: "next-hash", updateAvailable: true, version: "0.2.0" })
    expect(server.requestCount()).toBe(1)
  })

  test("unavailable: fixture server reports the same hash", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({
      routes: new Map([
        [
          khalaCodeUpdateFeedFixtureRouteKey("stable", platform),
          { kind: "json", body: { hash: "current-hash", version: "0.1.0" } },
        ],
      ]),
    })
    servers.push(server)

    const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
      localInfo: localInfo({ baseUrl: server.baseUrl }),
      platform,
    })

    expect(feed.updateAvailable).toBe(false)
    expect(feed.error).toBe("")
  })

  test("failed: fixture server 500s", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({
      routes: new Map([[khalaCodeUpdateFeedFixtureRouteKey("stable", platform), { kind: "status", status: 500 }]]),
    })
    servers.push(server)

    const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
      localInfo: localInfo({ baseUrl: server.baseUrl }),
      platform,
    })

    expect(feed.updateAvailable).toBe(false)
    expect(feed.error).toContain("HTTP 500")
  })

  test("failed: fixture server returns malformed JSON", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({
      routes: new Map([[khalaCodeUpdateFeedFixtureRouteKey("stable", platform), { kind: "malformed" }]]),
    })
    servers.push(server)

    const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
      localInfo: localInfo({ baseUrl: server.baseUrl }),
      platform,
    })

    expect(feed.error).toContain("failed to parse JSON")
  })

  test("failed: fixture server returns JSON missing a hash", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({
      routes: new Map([
        [khalaCodeUpdateFeedFixtureRouteKey("stable", platform), { kind: "json", body: { version: "0.2.0" } }],
      ]),
    })
    servers.push(server)

    const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
      localInfo: localInfo({ baseUrl: server.baseUrl }),
      platform,
    })

    expect(feed.error).toContain("missing hash")
  })

  test("dev channel short-circuits without a network call", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({ routes: new Map() })
    servers.push(server)

    const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
      localInfo: localInfo({ baseUrl: server.baseUrl, channel: "dev" }),
      platform,
    })

    expect(feed).toEqual({ error: "", hash: "current-hash", updateAvailable: false, version: "0.1.0" })
    expect(server.requestCount()).toBe(0)
  })
})

describe("Khala Code updater controller (#8440)", () => {
  const fakeBackend = (
    overrides: Partial<KhalaCodeDesktopUpdaterBackend> = {},
  ): KhalaCodeDesktopUpdaterBackend => ({
    checkForUpdates: async () => ({ error: "", updateAvailable: false, version: "" }),
    downloadUpdate: async () => ({ ok: true }),
    install: async () => {},
    ...overrides,
  })

  test("available: check() transitions idle -> checking -> available", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }),
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    expect(controller.getState()).toEqual({ status: "idle" })
    const seen: string[] = []
    const unsubscribe = controller.subscribe(state => seen.push(state.status))
    const state = await controller.check()
    unsubscribe()

    expect(state).toEqual({ checkedAt: expect.any(String), status: "available", version: "0.2.0" })
    expect(seen).toEqual(["idle", "checking", "available"])
  })

  test("unavailable: check() transitions to up_to_date when no newer version exists", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => ({ error: "", updateAvailable: false, version: "" }),
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    const state = await controller.check()
    expect(state).toEqual({ checkedAt: expect.any(String), status: "up_to_date", version: "0.1.0" })
  })

  // Oracle for khala_code.desktop.updater_error_states_legible_and_retryable.v1
  test("failed: check() surfaces backend errors as a retryable error state", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => ({ error: "network unreachable", updateAvailable: false, version: "" }),
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    const state = await controller.check()
    expect(state).toEqual({ message: "network unreachable", retryable: true, status: "error" })

    const status = controller.status()
    const actionResult = khalaCodeDesktopUpdaterActionResult(status)
    expect(actionResult.ok).toBe(false)
    expect(actionResult.error).toBe("network unreachable")
  })

  test("failed: check() surfaces a thrown backend error as a retryable error state", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => {
          throw new Error("boom")
        },
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    const state = await controller.check()
    expect(state).toEqual({ message: "boom", retryable: true, status: "error" })
  })

  test("downloaded: available -> downloading -> ready, and install is never called by download()", async () => {
    let installCalls = 0
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }),
        downloadUpdate: async () => ({ ok: true }),
        install: async () => {
          installCalls += 1
        },
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    await controller.check()
    const seen: string[] = []
    const unsubscribe = controller.subscribe(state => seen.push(state.status))
    const state = await controller.download()
    unsubscribe()

    expect(state).toEqual({ status: "ready", version: "0.2.0" })
    expect(seen).toEqual(["available", "downloading", "ready"])
    // Downloading a ready update must never itself trigger an install.
    expect(installCalls).toBe(0)
  })

  // Oracle for khala_code.desktop.updater_never_silently_installs.v1
  test("install-ready: install() is only reachable from the ready state and is never automatic", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({ checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }) }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    // install() before a download has happened must refuse rather than
    // silently installing whatever backend state currently exists.
    await expect(controller.install()).rejects.toThrow(/not ready to install/i)

    await controller.check()
    await expect(controller.install()).rejects.toThrow(/not ready to install/i)

    await controller.download()
    expect(controller.getState()).toEqual({ status: "ready", version: "0.2.0" })

    let installCalled = false
    const readyController = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }),
        install: async () => {
          installCalled = true
        },
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })
    await readyController.check()
    await readyController.download()
    expect(installCalled).toBe(false)
    await readyController.install()
    expect(installCalled).toBe(true)
  })

  test("a failed install returns the controller to the ready state instead of leaving it stuck installing", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }),
        install: async () => {
          throw new Error("install failed")
        },
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })
    await controller.check()
    await controller.download()

    await expect(controller.install()).rejects.toThrow("install failed")
    expect(controller.getState()).toEqual({ status: "ready", version: "0.2.0" })
  })

  test("a disabled controller never transitions out of idle", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({ checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }) }),
      channel: "dev",
      currentVersion: "0.1.0",
      enabled: false,
    })
    const state = await controller.check()
    expect(state).toEqual({ status: "idle" })
  })

  test("status() reports channel/version metadata and a release-notes URL tied to the current build", async () => {
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({ checkForUpdates: async () => ({ error: "", updateAvailable: true, version: "0.2.0" }) }),
      channel: "rc",
      currentVersion: "0.1.0",
      enabled: true,
    })
    expect(controller.status().releaseNotesUrl).toBe(khalaCodeDesktopUpdaterReleaseNotesUrl("0.1.0"))
    await controller.check()
    expect(controller.status().releaseNotesUrl).toBe(khalaCodeDesktopUpdaterReleaseNotesUrl("0.2.0"))
    expect(controller.status().channel).toBe("rc")
  })

  // Oracle for khala_code.desktop.updater_never_silently_installs.v1
  test("startPeriodicChecks only ever calls check(), never download() or install()", async () => {
    let checkCalls = 0
    let downloadCalls = 0
    let installCalls = 0
    const controller = createKhalaCodeDesktopUpdaterController({
      backend: fakeBackend({
        checkForUpdates: async () => {
          checkCalls += 1
          return { error: "", updateAvailable: false, version: "" }
        },
        downloadUpdate: async () => {
          downloadCalls += 1
          return { ok: true }
        },
        install: async () => {
          installCalls += 1
        },
      }),
      channel: "stable",
      currentVersion: "0.1.0",
      enabled: true,
    })

    const stop = controller.startPeriodicChecks(1)
    await Bun.sleep(15)
    stop()

    expect(checkCalls).toBeGreaterThan(0)
    expect(downloadCalls).toBe(0)
    expect(installCalls).toBe(0)
  })
})

describe("Khala Code Electrobun updater backend (#8440)", () => {
  test("checkForUpdates talks to the fixture feed server through the injected local-info reader", async () => {
    const server = startKhalaCodeUpdateFeedFixtureServer({
      routes: new Map([
        [
          khalaCodeUpdateFeedFixtureRouteKey("rc", platform),
          { kind: "json", body: { hash: "next-hash", version: "0.2.0-rc.1" } },
        ],
      ]),
    })
    servers.push(server)

    const backend = createKhalaCodeDesktopElectrobunUpdaterBackend({
      currentVersion: "0.1.0",
      updater: {
        applyUpdate: async () => {},
        downloadUpdate: async () => {},
        getLocalInfo: async () => localInfo({ baseUrl: server.baseUrl, channel: "rc" }),
      },
    })

    const result = await backend.checkForUpdates()
    expect(result).toEqual({ error: "", updateAvailable: true, version: "0.2.0-rc.1" })
  })

  test("falls back to the disabled local info when Electrobun reports an empty baseUrl (unpackaged/dev)", async () => {
    const backend = createKhalaCodeDesktopElectrobunUpdaterBackend({
      currentVersion: "0.1.0",
      updater: {
        applyUpdate: async () => {},
        downloadUpdate: async () => {},
        getLocalInfo: async () => localInfo({ baseUrl: "" }),
      },
    })

    const result = await backend.checkForUpdates()
    expect(result.updateAvailable).toBe(false)
    expect(result.error).toBe("")
  })

  test("downloadUpdate and install delegate to the real Updater methods", async () => {
    let downloadCalls = 0
    let applyCalls = 0
    const backend = createKhalaCodeDesktopElectrobunUpdaterBackend({
      currentVersion: "0.1.0",
      updater: {
        applyUpdate: async () => {
          applyCalls += 1
        },
        downloadUpdate: async () => {
          downloadCalls += 1
        },
        getLocalInfo: async () => localInfo(),
      },
    })

    expect(await backend.downloadUpdate()).toEqual({ ok: true })
    await backend.install()
    expect(downloadCalls).toBe(1)
    expect(applyCalls).toBe(1)
  })

  test("downloadUpdate reports a failure instead of throwing", async () => {
    const backend = createKhalaCodeDesktopElectrobunUpdaterBackend({
      currentVersion: "0.1.0",
      updater: {
        applyUpdate: async () => {},
        downloadUpdate: async () => {
          throw new Error("disk full")
        },
        getLocalInfo: async () => localInfo(),
      },
    })

    expect(await backend.downloadUpdate()).toEqual({ error: "disk full", ok: false })
  })
})
